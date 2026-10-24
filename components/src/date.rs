//! `date`: print the wall-clock time (notes/date.md, notes/clock.md, notes/calendar.md).
//!
//! It reads the clock page, adds the ambient counter, formats with `calendar` and sends the bytes.
//! It holds the page read-only, so it cannot set the clock (§43). An unknown clock is an answer: it
//! reads the state word before the offset and names the cause on [`DIAG_SLOT`], so `date >
//! when.txt` on a clockless machine leaves the file empty. Every complaint is sent, and that stream
//! closed, before a byte of output: the reader drains it first, so interleaving would block a send
//! nobody hears. `a0` is the [`Format`] (`FMT_*`, `0` is `Human`), `a1` a signed UTC offset in
//! minutes (there is no tzdata), and `a2` non-zero adds the clock's provenance line.

//! # BUGS
//!
//! - One-second resolution; the counter is sampled just after the page, so an `a2` run's two lines
//!   can straddle a step and disagree about the generation.
//! - A fixed UTC offset is not a time zone, and there is no `strftime` (notes/calendar.md).
//!
//! Name: ratified 2026-09-14 (calef): the Unix command's name, a plain noun.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use calendar::{DateTime, Format, UtcOffset};
use clock_contract::{ClockPage, state};
use user_mode_runtime::{exit, is_granted, monotonic_nanos, send};

/// Slot 0: where the output goes. An endpoint with `WRITE`, and the same 16-bytes-per-message
/// framing the std PAL's stdout uses (`w0` = the byte count, `w1`|`w2` = the bytes, little-endian),
/// so a reader that can already reassemble a std program's stdout can read this too.
const REPORT: u64 = 0;

/// Slot 1: the clock page's `PageFrame` capability, with `READ` and nothing else. Its *presence* is
/// what [`clock_page`] probes for; its rights are what stop this program setting the clock.
const CLOCK_SLOT: u64 = 1;

/// **The declared second stream** (DECISIONS §67, notes/pipes.md): another endpoint with `WRITE`,
/// speaking the same sink contract as [`REPORT`], carrying this program's complaints rather than its
/// answer. "The time is unknown" is a statement about the run, not the time, so it stays out of a
/// redirected file. The slot is the one `grant_plan`'s manifest declares: a program that declares
/// none has no second stream, and `2>` aimed at it is refused at the prompt.
const DIAG_SLOT: u64 = grant_plan::DIAGNOSTICS_SLOT;

/// Where the wiring maps the clock page, read-only. Must match `kernel/src/user.rs`'s
/// `date_tests::CLOCK_VA`.
const CLOCK_VA: u64 = address_space_map::pair_page(0x00c0_0000);

// The `a0` format selector. Must match system_tests/src/user/date_tests.rs. Zero is `Human` so that a
// spawn that passes no arguments at all gets the default a person wants.
const FMT_HUMAN: u64 = 0;
const FMT_RFC3339: u64 = 1;
const FMT_DATE: u64 = 2;
const FMT_TIME: u64 = 3;
const FMT_UNIX: u64 = 4;

#[unsafe(no_mangle)]
pub extern "C" fn _start(fmt: u64, offset_minutes: u64, provenance: u64) -> ! {
    // **Is there a second stream to write to?** Probed once, before anything is said, because every
    // sentence below has to know which endpoint it is going to. A wiring that granted none (the
    // guest tests, which spawn this program directly) leaves the slot empty and every complaint goes
    // back in-band on the output, which is what `date` did before §67.
    HAS_DIAG.store(is_granted(DIAG_SLOT), Ordering::Relaxed);

    // The state first, before anything computes a time from the offset. This ordering is the whole
    // difference between this program and `SystemTime::now()`: it has an error channel and uses it.
    let Some(page) = clock_page() else {
        complain(b"date: the time is unknown: this process holds no clock capability");
        end();
    };
    let r = page.read();
    if !state::is_known(r.state) {
        complain(b"date: the time is unknown: the machine has no clock it believes");
        // Still worth saying which nothing it was, when asked: an unknown clock has a generation
        // too, and "nobody has ever published" reads differently from "somebody published unknown".
        if provenance != 0 {
            source_line(diag_slot(), r.state, r.generation);
        }
        end();
    }

    let Ok(offset) = UtcOffset::from_minutes(offset_minutes as i64 as i32) else {
        complain(b"date: that UTC offset is not one RFC 3339 can write");
        end();
    };

    // Nanoseconds to seconds, truncating towards zero, which is exact here: the wall clock is a
    // `u64` count of nanoseconds since 1970 and this calendar's second is the smallest unit it has.
    let unix_secs = (clock_contract::wall_nanos(r.offset_nanos, monotonic_nanos())
        / clock_contract::NANOS_PER_SEC) as i64;

    // Out of range is not reachable from a clock the service believed (its sanity window is 2026 to
    // 2100 and the calendar's is 0000 to 9999), and it is still an answer rather than a panic,
    // because "unreachable" is a claim about today's policy and this is a claim about arithmetic.
    let Ok(dt) = DateTime::from_unix(unix_secs, offset) else {
        complain(b"date: the clock reads a time outside the calendar's range");
        end();
    };

    // **Nothing to complain about, said out loud before the answer starts.** The reader of the
    // second stream drains it to end-of-stream *before* it reads the first (notes/pipes.md), so a
    // program that produced output while its complaints were still open would be writing to a reader
    // that is not listening yet, and both would wait forever. Saying "no complaints" costs one
    // message and is the whole of the rule a declaring program keeps.
    diag_end();
    line(dt.format(format_of(fmt)).as_bytes());
    if provenance != 0 {
        // Provenance is a fact about the clock rather than a complaint about the run, so it goes
        // with the answer. That is also why the second stream is already closed above.
        source_line(REPORT, r.state, r.generation);
    }
    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// The `a0` selector as a [`Format`]. An unrecognised value is `Human` rather than an error: this
/// is a register, not a parsed string, and the caller that got it wrong wants the default.
fn format_of(fmt: u64) -> Format {
    match fmt {
        FMT_HUMAN => Format::Human,
        FMT_RFC3339 => Format::Rfc3339,
        FMT_DATE => Format::Date,
        FMT_TIME => Format::Time,
        FMT_UNIX => Format::Unix,
        _ => Format::Human, // a caller that got the selector wrong wants the default
    }
}

/// **Where the time came from**, which is the readable form of `clock_contract`'s four states.
///
/// Worth printing because the provenance is a real distinction this system makes and no `date` on a
/// Unix can: "a human set it" and "an external source the service bounded accepted it" are
/// different claims, and a caller weighing a certificate expiry wants the difference. The
/// generation says how many times the page has been published to, so a reader can see whether the
/// clock has been stepped since boot.
fn source_line(slot: u64, st: u64, generation: u64) {
    fn push(buf: &mut [u8; 64], n: &mut usize, bytes: &[u8]) {
        for &b in bytes {
            if *n < buf.len() {
                buf[*n] = b;
                *n += 1;
            }
        }
    }

    let mut buf = [0u8; 64];
    let mut n = 0;
    push(&mut buf, &mut n, b"date: clock source: ");
    push(
        &mut buf,
        &mut n,
        match st {
            state::RTC => b"rtc" as &[u8],
            state::SET => b"set",
            state::SYNCED => b"synced",
            _ => b"unknown",
        },
    );
    push(&mut buf, &mut n, b", generation ");

    let mut digits = [0u8; 20];
    let mut d = digits.len();
    let mut v = generation;
    loop {
        d -= 1;
        digits[d] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    push(&mut buf, &mut n, &digits[d..]);
    line_on(slot, &buf[..n]);
}

/// **Announce the end of the stream, then exit** (milestone 50).
///
/// Every exit path from `_start` comes here, so a reader downstream of a `|` is told the stream
/// ended rather than inferring it from a death (notes/sink-protocol.md). The send blocks until
/// somebody takes it, so a reader that stopped at the newline leaves this process parked.
fn end() -> ! {
    diag_end();
    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// Whether this process was granted a second stream, decided once in [`_start`].
static HAS_DIAG: AtomicBool = AtomicBool::new(false);

/// Which endpoint a complaint goes to: the declared second stream when there is one, and the output
/// otherwise. The fallback is honest rather than a silent drop: a program nobody gave a diagnostic
/// sink still has something to say, and in-band is where it used to say it.
fn diag_slot() -> u64 {
    if HAS_DIAG.load(Ordering::Relaxed) {
        DIAG_SLOT
    } else {
        REPORT
    }
}

/// Say something about the run rather than about the time.
fn complain(bytes: &[u8]) {
    line_on(diag_slot(), bytes);
}

/// **Close the second stream**, which is not tidiness. Its reader drains it to end-of-stream before
/// it reads anything else, so a `date` that exited without this would leave the prompt waiting on a
/// message nobody is going to send.
fn diag_end() {
    if HAS_DIAG.load(Ordering::Relaxed) {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }
}

/// The clock page, or `None` when this process was granted no clock.
///
/// The probe has to answer **without touching the page**, because a process that was granted no
/// clock has nothing mapped at [`CLOCK_VA`] and a read there would fault instead of returning an
/// answer. So it invokes the capability in the slot with a method number no object type defines:
/// an empty slot answers `NoSuchSlot`, and a real `PageFrame` answers `BadMethod`, which is a refusal
/// from an object that exists and is therefore proof one is there. Same shape as the std PAL's
/// `is_granted()`; the two are the same problem.
fn clock_page() -> Option<ClockPage> {
    if !is_granted(CLOCK_SLOT) {
        return None;
    }
    // SAFETY: the wiring maps the clock page read-only at CLOCK_VA alongside the capability the
    // probe just found, and nothing unmaps it. Without the capability we never build the pointer.
    Some(unsafe { ClockPage::new(CLOCK_VA) })
}

/// Send `bytes` and a newline to the output endpoint, 16 bytes per message.
fn line(bytes: &[u8]) {
    line_on(REPORT, bytes);
}

/// [`line()`], to a named endpoint. The two streams speak the same contract, which is the point: a
/// diagnostic is bytes, and the only thing that makes one a diagnostic is which endpoint it is on.
fn line_on(slot: u64, bytes: &[u8]) {
    let mut out = [0u8; 96];
    let n = bytes.len().min(out.len() - 1);
    out[..n].copy_from_slice(&bytes[..n]);
    out[n] = b'\n';
    for chunk in out[..n + 1].chunks(16) {
        let mut w1 = 0u64;
        let mut w2 = 0u64;
        for (i, &b) in chunk.iter().enumerate() {
            if i < 8 {
                w1 |= (b as u64) << (8 * i);
            } else {
                w2 |= (b as u64) << (8 * (i - 8));
            }
        }
        send(slot, chunk.len() as u64, w1, w2);
    }
}

user_mode_runtime::panic_handler!();
