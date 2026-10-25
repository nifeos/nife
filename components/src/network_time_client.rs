//! **The network time client** (§43 (reading the clock is a page), notes/ntp.md): turns
//! `network_time_protocol`'s wire format into a clock correction. It holds a network capability and
//! a capability to propose a time, and does not hold the clock: with no clock page, setting the
//! time is not an operation it can express, so a compromised client can lie inside the service's
//! bounds and do nothing else. `an_ntp_client_holds_no_writable_clock_page` proves the missing
//! mapping with `fixtures/src/unwritable_clock_witness.rs`, endowed by the same function.
//!
//! One-shot: up to [`ATTEMPTS`] requests, a fresh entropy-service nonce and a `Timer` sleep each,
//! one proposal, exit. No entropy, no request ([`RPT_NO_ENTROPY`]); a kiss-o'-death is not retried.
//!
//! Name: ratified 2026-09-14 (calef), replacing `ntp`. Refused keeping `ntp`: nothing types this
//! name, so a typed command's latitude does not reach it. `client` is NTP's own mode word.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use abi::rights;
use clock_contract::propose;
use network_time_protocol::{Query, Reject, Timestamp};
// The socket contract, verbatim from the file `net_stack` compiles, so this client and the test
// server cannot drift from the real server's idea of the wire format.
// An NTP client speaks the UDP half of the contract and never the TCP half, so the rest of the
// file is dead here. Allowed rather than trimmed: the value of compiling the *same file* net_stack
// does is that the two cannot drift, and a per-consumer subset would throw that away.
#[allow(dead_code)]
use socket_contract::*;
use user_mode_runtime::socket_frame::Frame;
use user_mode_runtime::{
    call, call_receiving, cntfrq, exit, map_page_frame, now, retype_into_page_frame,
    retype_sleeper, send, send_capability, sleep_until, yield_now,
};

// =================================================================================================
// The slots, and the words this program reports.
//
// The report vocabulary is one numbering space across all three programs milestone 290 split this
// file into, and each declares only the words it sends. The numbers did not move in that split, so
// a boot log from before it still reads. `kernel/src/user/ntp_service.rs`'s `rpt` module is the
// whole vocabulary in one place, as the kernel side of a wiring always is here.
// =================================================================================================

/// Slot 0: the report endpoint (WRITE).
const REPORT: u64 = 0;
/// Slot 1: the socket contract's endpoint. **This client's whole network authority.** The client
/// holds `WRITE`, the test server `READ`.
const STACK: u64 = 1;
/// Slot 2: an untyped budget, to mint and map the one shared frame.
const MEMORY_REGION: u64 = 2;
/// Slot 3: the clock service's **propose** endpoint (WRITE). Not a page, and not a way to set
/// anything.
const PROPOSE: u64 = 3;
/// Slot 4: the entropy service's endpoint (WRITE): "you may obtain randomness", naming no device.
const ENTROPY: u64 = 4;

/// A sample was accepted and proposed. `w1` is the `clock_contract::status` the service answered with,
/// `w2` the nanoseconds proposed.
pub(crate) const RPT_SYNCED: u64 = 1;
/// A reply arrived and `Query::accept` refused it. `w1` is a [`reject_code`], `w2` the number of
/// requests sent. **Nothing was proposed.**
pub(crate) const RPT_REJECTED: u64 = 2;
/// No reply after [`ATTEMPTS`] requests. `w1` is the number sent.
pub(crate) const RPT_NO_REPLY: u64 = 3;
/// **No unguessable bits, so no request.** `w1` is the entropy service's reply word (or the kernel
/// error from a `CALL` on an empty slot, which is what "no capability" looks like).
pub(crate) const RPT_NO_ENTROPY: u64 = 4;
/// The socket contract refused us. `w1` names the step, so a wiring mistake is not a mystery.
pub(crate) const RPT_NET_ERROR: u64 = 5;
/// The local wall clock is outside the range an NTP timestamp can hold, so no exchange is possible.
pub(crate) const RPT_BAD_LOCAL_TIME: u64 = 8;

/// How many requests one synchronisation makes before giving up. Three is a client's ordinary
/// behaviour on a lossy path, not a widened timeout; the DNS check in `socket_test_client` settled on the same
/// number for the same reason.
pub const ATTEMPTS: u32 = 3;

/// The gap between attempts. Milliseconds rather than NTP's 64 seconds, because this client is still
/// one-shot (see the module docs). Asleep on a timer since milestone 106, whose resolution is the
/// 10 ms scheduler tick, so the gap is in practice one tick.
const RETRY_GAP_NANOS: u64 = 2 * 1_000_000;

/// Where this client maps the shared socket frame in its own address space. Above the program's
/// segments; the same address `socket_test_client` uses, and each address space is its own. The
/// test server picks its own, and the two do not have to agree: what they share is the frame's
/// *layout*, which is `socket_contract`'s.
const PAGE_FRAME_VA: u64 = address_space_map::pair_page(0x0000_0000_00A0_0000);

/// The shared frame at that address, read and written through
/// `user_mode_runtime::socket_frame::Frame`.
// SAFETY: this program's own `PageFrame::MAP` (in `attach_page_frame`, before the frame is touched)
// mapped one page read/write at PAGE_FRAME_VA before any access through `SHARED_FRAME` (milestone 139 (drive the unsafe count down)).
static SHARED_FRAME: Frame = unsafe { Frame::new(PAGE_FRAME_VA) };

user_mode_runtime::program_entry!(start);

/// `a0` is the server's IPv4 packed big-endian, `a1` its UDP port.
fn start(server_ip: u64, server_port: u64, _arg2: u64) -> ! {
    client(server_ip, server_port)
}

/// Report and stop. One-shot, so it **exits** rather than parking: a role left spinning on a run
/// queue starves later tests on the same core, which is the finding `socket_test_client`'s `done` records.
fn done(w0: u64, w1: u64, w2: u64) -> ! {
    send(REPORT, w0, w1, w2);
    exit();
}

fn client(server_ip: u64, server_port: u64) -> ! {
    // The wall clock, by asking the service (`propose::STATE`): this process holds no mapping of
    // the clock page. Taken once; the ambient monotonic counter carries the time from here.
    let (_state, wall0) = call(PROPOSE, propose::req(propose::STATE), 0);
    let local = LocalClock {
        wall0,
        mono0: monotonic_nanos(),
    };

    // **The nonce before the network**: a client with no unguessable bits has nothing to send,
    // so it learns that before it exercises any network authority at all.
    let mut nonce = match nonce_bits() {
        Ok(bits) => bits,
        Err(word) => done(RPT_NO_ENTROPY, word, 0),
    };

    // Our one socket, a capability the stack hands back (§255 (each socket is its own
    // capability)), and then our page attached to it.
    let (REP_OK, _, Some(socket)) = call_receiving(STACK, OPERATION_OPEN_UDP, 0) else {
        done(RPT_NET_ERROR, 2, 0);
    };
    attach_page_frame(socket);

    let mut sent = 0u32;
    let mut last_reject = 0u64;
    while sent < ATTEMPTS {
        // A fresh nonce every attempt after the first. Reusing one would let a reply to an earlier
        // request answer a later one, which is the stale-reply half of the origin check's job.
        if sent > 0 {
            nonce = match nonce_bits() {
                Ok(bits) => bits,
                Err(word) => done(RPT_NO_ENTROPY, word, 0),
            };
        }

        // T1 as late as it can honestly be taken: the request carries the nonce, not the send
        // time, so only one IPC lies between this reading and the datagram leaving. The server is
        // the one the wiring named in `a0`/`a1`; which server to ask is an endowment.
        SHARED_FRAME.set_dst((server_ip as u32).to_be_bytes(), server_port as u16);
        let Some(t1) = stamp(local.now()) else {
            done(RPT_BAD_LOCAL_TIME, local.now(), 0);
        };
        let query = Query::with_nonce(t1, Timestamp::from_bits(nonce));
        SHARED_FRAME.payload_write(&query.request());
        sent += 1;
        if call(
            socket,
            OPERATION_SENDTO,
            network_time_protocol::PACKET_LEN as u64,
        )
        .0 != REP_OK
        {
            done(RPT_NET_ERROR, 3, sent as u64);
        }

        let (n, _) = call(socket, OPERATION_RECEIVE, 0);
        if n == REP_ERR || n == 0 {
            poll_gap();
            continue;
        }

        // T4, read as close to the arrival as this side can manage: the RECEIVE reply is the first
        // instruction after the bytes landed.
        let Some(t4) = stamp(local.now()) else {
            done(RPT_BAD_LOCAL_TIME, local.now(), 0);
        };
        let mut wire = [0u8; network_time_protocol::PACKET_LEN];
        let n = SHARED_FRAME.payload_read(n as usize, &mut wire);

        match query.accept(&wire[..n], t4) {
            Ok(sample) => {
                // Applied to the local clock as it is now, not as it was at T4: the proposal is a
                // claim about the present, and the service compares it against the present.
                let corrected = local.now() as i128 + sample.offset.nanos();
                let proposed = corrected.clamp(0, u64::MAX as i128) as u64;
                let (status, _wall_after) = call(PROPOSE, propose::req(propose::PROPOSE), proposed);
                let _ = call(socket, OPERATION_CLOSE, 0);
                done(RPT_SYNCED, status, proposed);
            }
            Err(reject) => {
                last_reject = reject_code(reject);
                // A kiss-o'-death is an instruction, and the instruction is to stop.
                if matches!(reject, Reject::KissOfDeath(_)) {
                    break;
                }
                poll_gap();
            }
        }
    }

    let _ = call(socket, OPERATION_CLOSE, 0);
    if last_reject != 0 {
        done(RPT_REJECTED, last_reject, sent as u64);
    }
    done(RPT_NO_REPLY, sent as u64, 0)
}

/// **Eight unguessable bytes, or an error word that says why not.**
///
/// `entropy_contract::delivered` is what makes the two failures distinguishable with no probe: a byte
/// count is `0..=8`, and every error a `CALL` can return is one of the kernel's small negatives,
/// which read as enormous `u64`s. So `Err(0)` is "the service has no entropy" and `Err(huge)` is
/// "there is no entropy service in slot 4". Both stop the client; only the report tells them apart.
fn nonce_bits() -> Result<u64, u64> {
    let (r0, r1) = call(ENTROPY, entropy_contract::req(entropy_contract::GET, 8), 0);
    let Some(n) = entropy_contract::delivered(r0) else {
        return Err(r0);
    };
    if n < 8 {
        return Err(r0);
    }
    let mut bytes = [0u8; 8];
    entropy_contract::take(n, r1, &mut bytes);
    Ok(u64::from_le_bytes(bytes))
}

/// The local wall clock, anchored once against the monotonic counter.
///
/// `wall0` is what the clock service said at `mono0`; everything after that is counter arithmetic,
/// which is ambient and costs one instruction. When the service says the clock is `UNKNOWN` it
/// answers 0, and the arithmetic still works out: T1 and T4 are then measured from 1970, the offset
/// comes out as the whole distance to the server's clock, and the proposal lands on the server's
/// time. That is the bootstrap case, and it is the case a machine with no RTC is in.
struct LocalClock {
    wall0: u64,
    mono0: u64,
}

impl LocalClock {
    fn now(&self) -> u64 {
        self.wall0
            .saturating_add(monotonic_nanos().saturating_sub(self.mono0))
    }
}

/// Monotonic nanoseconds since boot, from the ambient counter.
///
/// Whole seconds and a remainder, not `ticks * 1e9`: at 62.5 MHz the naive product overflows a
/// `u64` about five minutes into a boot, and the clock built on it jumps backwards by six centuries.
/// The clock service's copy of this arithmetic carries the same note.
fn monotonic_nanos() -> u64 {
    let freq = cntfrq();
    let ticks = now();
    let secs = ticks / freq;
    let rem = ticks % freq;
    secs * clock_contract::NANOS_PER_SEC + rem * clock_contract::NANOS_PER_SEC / freq
}

/// Unix nanoseconds to an NTP timestamp. `None` outside the crate's representable window, which the
/// clock service's 2100 ceiling keeps us well inside; it is checked rather than assumed because the
/// alternative to checking is a timestamp 136 years out.
fn stamp(unix_nanos: u64) -> Option<Timestamp> {
    Timestamp::from_unix(
        unix_nanos / clock_contract::NANOS_PER_SEC,
        (unix_nanos % clock_contract::NANOS_PER_SEC) as u32,
    )
}

/// The poll interval, asleep on a timer (milestone 106 (a wait that ends on either the interrupt or
/// the deadline)). The pair is retyped from this client's own untyped on the first gap; if the
/// untyped cannot pay for it, the gap falls back to the yield-spin it was until 2026-09-26.
fn poll_gap() {
    use core::sync::atomic::AtomicU64;
    use core::sync::atomic::Ordering::Relaxed;
    static TIMER: AtomicU64 = AtomicU64::new(0); // 0: not made yet; u64::MAX: refused
    static NOTIFICATION: AtomicU64 = AtomicU64::new(0);
    if TIMER.load(Relaxed) == 0 {
        // Up to the entropy slot, whose emptiness is how this client learns it has no entropy.
        let (t, n) = retype_sleeper(MEMORY_REGION, ENTROPY + 1).unwrap_or((u64::MAX, 0));
        NOTIFICATION.store(n, Relaxed);
        TIMER.store(t, Relaxed);
    }
    let timer = TIMER.load(Relaxed);
    if timer != u64::MAX {
        let ticks = abi::timer::counter_ticks_for(
            RETRY_GAP_NANOS / 1_000_000_000,
            (RETRY_GAP_NANOS % 1_000_000_000) as u32,
            cntfrq(),
        );
        if sleep_until(
            timer,
            NOTIFICATION.load(Relaxed),
            now().saturating_add(ticks),
        ) >= 0
        {
            return;
        }
    }
    let deadline = monotonic_nanos() + RETRY_GAP_NANOS;
    while monotonic_nanos() < deadline {
        yield_now();
    }
}

/// A stable small integer for each `Reject`, so a test (and one day a log) can name which check
/// refused a packet. The enum is the security story of unauthenticated NTP; collapsing it to a bool
/// at the first opportunity would throw that away.
fn reject_code(r: Reject) -> u64 {
    match r {
        Reject::Length(_) => 1,
        Reject::Version(_) => 2,
        Reject::Mode(_) => 3,
        Reject::KissOfDeath(_) => 4,
        Reject::Stratum(_) => 5,
        Reject::Unsynchronised => 6,
        Reject::OriginMismatch => 7,
        Reject::TransmitZero => 8,
        Reject::ServerOrderReversed => 9,
        Reject::ClientOrderReversed => 10,
        Reject::NegativeDelay => 11,
        Reject::RootDistance => 12,
    }
}

/// Mint one frame from our own budget, map it, and delegate it to `socket`. Exactly what
/// `socket_test_client` does, because it is exactly the same contract.
fn attach_page_frame(socket: u64) {
    // RETYPE answers with the new frame capability's slot, or a negative error.
    let frame = retype_into_page_frame(MEMORY_REGION);
    if frame < 0 {
        done(RPT_NET_ERROR, 0, 0);
    }
    let frame = frame as u64;
    // Map it writable; the page tables come from our untyped.
    if !map_page_frame(frame, PAGE_FRAME_VA, true, MEMORY_REGION) {
        done(RPT_NET_ERROR, 1, 0);
    }
    // Delegate it, narrowed to read/write, with the ATTACH request.
    if send_capability(
        socket,
        frame,
        rights::READ | rights::WRITE,
        OPERATION_ATTACH_PAGE_FRAME,
    ) < 0
    {
        done(RPT_NET_ERROR, 1, 0);
    }
}

user_mode_runtime::panic_handler!();
