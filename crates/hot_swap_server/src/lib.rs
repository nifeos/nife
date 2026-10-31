#![no_std]
//! **The hot-swap server: the runtime half of `hot_swap_contract`.** One serving loop with two
//! implementations, which is what makes "the replacement is a different program" a claim about the
//! program and not about the harness: the fixtures' two `_start` functions both call [`start`], and
//! everything else, including the capability layout and the wire format, is one function.
//!
//! It lives here rather than in the contract so the contract carries no `user_mode_runtime` edge
//! (DECISIONS §235 (the OS is built and updated from packages, and the tree divides by what releases together) P3: a contract never links the runtime), the same split as
//! `supervision_contract` and `process_builder`. See notes/live-replacement.md and
//! notes/state-handoff.md.
//!
//! # Examples
//!
//! ```no_run
//! use hot_swap_server::start;
//!
//! // A fixture's `_start` is this call and nothing else. `a0` picks the system: a device flag for
//! // the three stateless ones, `START_FRESH` or `START_ABSORB` for the stateful one. The operator
//! // chooses what a binary serves by the argument, not by the program, which is why the same
//! // function is both instances' entry point. `xform` is the one thing that differs between them.
//! fn serve_v1() -> ! {
//!     start(1, |arg| arg.wrapping_mul(2), 0, 0x1000_0000, 0)
//! }
//! ```
//!
//! Every entry point here is a syscall loop from EL0 and returns `!`, so the example has nothing to
//! assert. It runs under `cargo test --doc -p hot_swap_server` on an aarch64 host.
//!
//! Name: provisional, minted 2026-10-29 (UTC) by lane
//! `milestone/607-item-3-contracts-drop-the-runtime`. calef names it.

use hot_swap_contract::{
    BAD_REQUEST, NOTE, NOTE_ABSORBED, NOTE_NOT_ABSORBED, NOTE_SWAP_NOW, OPERATION_PUT,
    OPERATION_QUIESCE, PAGE, POKE, POKE_PROBE, POKE_RESUME, PUT_REFUSED, QUIESCED, RPT,
    RPT_PROBE_SURVIVED, RPT_QUIESCED, RPT_UP, START_ABSORB, START_FRESH, STATE_VA, SVC,
    SWAP_TRIGGER, V1, absorb, log_put, probe_device, state_blob, tag,
};

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `hot_swap_contract` with the split, and calef names it.
/// **Start one instance, whichever system it belongs to.** Both fixtures' `_start` is this call, so
/// which system an instance serves is the operator's choice and not a property of the binary.
///
/// `a0` is the device flag (`0` or `1`) for the three stateless systems, or [`START_FRESH`] /
/// [`START_ABSORB`] for the stateful one. `a2` is the wedge sequence number for the former and the
/// blob layout this instance understands for the latter.
pub fn start(version: u64, xform: fn(u64) -> u64, a0: u64, log_base: u64, a2: u64) -> ! {
    match a0 {
        START_FRESH | START_ABSORB => {
            serve_with_state(version, xform, log_base, a2, a0 == START_ABSORB)
        }
        _ => serve(version, xform, log_base, a0 != 0, a2),
    }
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `hot_swap_contract` with the split, and calef names it.
/// **Serve the stateful contract** ([`hot_swap_contract::STATEFUL_BACKEND`], DECISIONS §209
/// (state handoff is an opaque blob over a granted frame, and it is optional)).
///
/// [`serve`]'s loop with one word of state. An absorbing instance reads the handoff page **before
/// it serves anything** and announces the tally it took over ([`NOTE_ABSORBED`]) or refuses
/// ([`NOTE_NOT_ABSORBED`]) and exits, so the operator can decline to commit before any request is
/// answered wrongly. The reply's second word is the tally, equal to the echoed sequence number
/// exactly when nothing was lost. Quiescing writes the blob before the reply, so `QUIESCED` is the
/// receipt; a quiesced instance can be told to [`POKE_RESUME`]. See notes/state-handoff.md.
pub fn serve_with_state(
    version: u64,
    xform: fn(u64) -> u64,
    log_base: u64,
    layout: u64,
    absorbing: bool,
) -> ! {
    let mut tally = 0u64;
    if absorbing {
        match absorb(read_state(), layout) {
            Ok(t) => {
                tally = t;
                user_mode_runtime::send(NOTE, NOTE_ABSORBED, version, tally);
            }
            Err(found) => {
                // Refuse and leave. Nothing was received, nothing was answered, and the page is
                // untouched, so the incumbent's blob is still there if anyone wants it.
                user_mode_runtime::send(NOTE, NOTE_NOT_ABSORBED, version, found);
                user_mode_runtime::exit()
            }
        }
    }
    user_mode_runtime::send(RPT, RPT_UP, version, 0);

    loop {
        let mut since = 0u64;
        loop {
            let req = user_mode_runtime::receive_request(SVC);
            let (operation, arg) = (req.w0, req.w1);
            // Only a CALL is answered; a SEND_CAPABILITY's capability is deleted (milestone 706 (a `CALL`
            // server can tell a Reply from a delegation)).
            let Some(slot) = req.delivered.into_reply() else {
                continue;
            };
            match operation {
                OPERATION_PUT => {
                    if !log_put(log_base.saturating_add(arg), version) {
                        user_mode_runtime::reply(slot, PUT_REFUSED, PUT_REFUSED);
                        continue;
                    }
                    user_mode_runtime::reply(slot, xform(arg), tag(version, tally));
                    tally += 1;
                    since += 1;
                    // Counted from the last (re)start, so an incumbent resumed after a swap that did
                    // not commit asks for another one a further SWAP_TRIGGER requests in.
                    if since == SWAP_TRIGGER && version == V1 {
                        user_mode_runtime::send(NOTE, NOTE_SWAP_NOW, version, tally);
                    }
                }
                // BUGS: as in `serve`, the quiesce arrives on `SVC`, which every client holds
                // `WRITE` on, so any client can evict this instance. Closing it is a badge the
                // operator quiesces through, `line_editor::proto::SUPERVISOR_BADGE`'s shape.
                OPERATION_QUIESCE => {
                    write_state(state_blob(layout, tally));
                    user_mode_runtime::send(RPT, RPT_QUIESCED, version, tally);
                    user_mode_runtime::reply(slot, QUIESCED, tally);
                    break;
                }
                _ => {
                    user_mode_runtime::reply(slot, BAD_REQUEST, 0);
                }
            }
        }
        let (what, _, _) = user_mode_runtime::receive(POKE);
        if what != POKE_RESUME {
            user_mode_runtime::exit()
        }
    }
}

/// The handoff page's two words, read volatile for [`log_get`]'s reason: another address space wrote
/// them, and the read is ordered after the write by the IPC that started this instance.
fn read_state() -> [u64; 2] {
    // SAFETY: the handoff page is mapped read/write at STATE_VA in every instance of STATEFUL_BACKEND, by the
    // operator's plan, before the instance is started.
    unsafe {
        [
            core::ptr::read_volatile(STATE_VA as *const u64),
            core::ptr::read_volatile((STATE_VA + PAGE) as *const u64),
        ]
    }
}

fn write_state(blob: [u64; 2]) {
    // SAFETY: as `read_state`.
    unsafe {
        core::ptr::write_volatile(STATE_VA as *mut u64, blob[0]);
        core::ptr::write_volatile((STATE_VA + PAGE) as *mut u64, blob[1]);
    }
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `hot_swap_contract` with the split, and calef names it.
/// **Serve the stable endpoint until told to quiesce, then prove the revoke.**
///
/// `xform` is the only thing that differs between the two instances: v1 passes a Rust function, v2
/// passes one that calls into C. Everything else, including the capability layout and the wire
/// format, is this one function, which is what makes "the replacement is a different program" a
/// claim about the program and not about the harness.
///
/// `wedge` is [`hot_swap_contract::WEDGE_SEQ`] on the hung channel and **zero everywhere else**, which is the whole of
/// how that channel differs: the same program, the same contract, one request it never answers.
pub fn serve(version: u64, xform: fn(u64) -> u64, log_base: u64, device: bool, wedge: u64) -> ! {
    if device {
        // Before serving anything: can we reach the device we were endowed with? An instance that
        // could not would still answer every request correctly, and the swap would look perfect
        // while the hardware went unowned. A read that faults never returns, so reaching the next
        // line *is* the answer.
        let _ = probe_device();
    }
    user_mode_runtime::send(RPT, RPT_UP, version, device as u64);

    let mut served = 0u64;
    loop {
        let req = user_mode_runtime::receive_request(SVC);
        let (operation, arg) = (req.w0, req.w1);
        // A plain SEND or a SEND_CAPABILITY slipped in; the contract says CALL, and there is nobody to
        // answer. A delegated capability is deleted rather than answered into (milestone 706).
        let Some(slot) = req.delivered.into_reply() else {
            continue;
        };
        match operation {
            // **Stop answering, without dying**: keep the endpoint, the device and `slot`, and
            // park on a control endpoint nothing sends to, with no `log_put`. The ordinary swap
            // trigger is sent here, at the swallowed request, so the operator's quiesce arrives
            // after it was taken. See notes/hung-component/the-wedge-test.md.
            OPERATION_PUT if wedge != 0 && arg == wedge => {
                user_mode_runtime::send(NOTE, NOTE_SWAP_NOW, version, served);
                loop {
                    let _ = user_mode_runtime::receive(POKE);
                }
            }
            OPERATION_PUT => {
                if !log_put(log_base.saturating_add(arg), version) {
                    user_mode_runtime::reply(slot, PUT_REFUSED, PUT_REFUSED);
                    continue;
                }
                served += 1;
                user_mode_runtime::reply(slot, xform(arg), tag(version, arg));
                if served == SWAP_TRIGGER && version == V1 && wedge == 0 {
                    // Tell the operator the conversation is well under way. Sent *after* the reply,
                    // so the client is already waiting on its next call when the swap begins. A
                    // wedging instance sends the same trigger at the request it swallows instead.
                    user_mode_runtime::send(NOTE, NOTE_SWAP_NOW, version, served);
                }
            }
            // BUGS: the quiesce arrives on `SVC`, the endpoint every client holds `WRITE` on, and
            // nothing tells the operator's CALL from a client's. Any client can evict the server
            // from its endpoint: a denial of service, not a reach, found by milestone 633's third
            // outsider pass and read rather than booted. Closing it means the operator quiescing on
            // a channel only it holds, or a badge the server checks; the second is the shape of
            // §230 (badged endpoint capabilities), and either changes this protocol.
            OPERATION_QUIESCE => {
                user_mode_runtime::send(RPT, RPT_QUIESCED, version, served);
                user_mode_runtime::reply(slot, QUIESCED, served);
                break;
            }
            _ => {
                user_mode_runtime::reply(slot, BAD_REQUEST, 0);
            }
        }
    }

    // Quiesced. We no longer receive on the stable endpoint, so requests arriving from here on park
    // on its sender queue for whoever receives next. We wait for the operator to tell us what to do
    // with the corpse we are about to become.
    let (what, _, _) = user_mode_runtime::receive(POKE);
    if what == POKE_PROBE && device {
        // Touch the registers one last time. If the operator's revoke was real this faults, and the
        // kernel's fault message is the receipt. Reaching the line after it is the failure, and it
        // is reported positively: a silent success here would be indistinguishable from the healthy
        // run, in which this program simply stops existing at the read above.
        let _ = probe_device();
        user_mode_runtime::send(RPT, RPT_PROBE_SURVIVED, version, 0);
    }
    user_mode_runtime::exit()
}
