//! **`ps`: who else is running, and who is allowed to ask** (notes/process-view.md). Walk one
//! supervision domain with `abi::rendezvous::SURVEY`, say what it found on one stream and what went
//! wrong on the other. The listing is `crates/ps`, host-tested; this is the syscall and two sinks.
//!
//! **It cannot enumerate the machine.** A Unix `ps` reads an ambient `/proc`. This holds a
//! supervision endpoint and sees exactly the threads whose deaths arrive on it: no pid space to
//! scan, nothing in reach that takes a process id it was not handed. The wide grant is not
//! forbidden but nameable: a machine-wide monitor is a `ps` handed the machine's endpoint, and
//! `caps ps` prints which one this is.

//! # Capability contract
//!
//! | slot | what | why |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the table goes |
//! | 7 | the process domain, `ENUMERATE` | the supervision endpoint whose members it may **name** |
//! | 8 | the diagnostics sink, `WRITE` | where a refusal goes, so `>` cannot swallow it |
//!
//! None names a process, and `TIME` is a second walk of the same endpoint under the same right.
//! Why `ENUMERATE` and not `READ` is at [`DOMAIN_SLOT`]; it is also why there is no `pkill`.

//! # EXAMPLES
//!
//! ```text
//! $ ps
//!          TID  STATE     TIME(ms)
//!            5  running        310
//!            9  blocked         20
//! $ ps        ps: this process holds no process-domain capability
//! ```

//! # BUGS
//!
//! - No `CMD` column: a process here has no name (`crates/ps`).
//! - `pgrep` is this program filtered, with the same three capabilities.
//! - `ps` lists itself, `running`, as Unix's does.
//!
//! Name: ratified 2026-09-14 (calef): the name every reader knows, shared with `crates/ps` as one
//! thing split at the IO boundary.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use user_mode_runtime::{exit, is_granted, send, survey, survey_record};

/// The output sink: where the table goes. Slot 0 is where every spawned program's output lands.
const REPORT: u64 = 0;

/// **The process domain**: a supervision endpoint, `ENUMERATE`. Everything this program can see
/// comes through here and nothing else in the capability table names a process.
///
/// `ENUMERATE`, not `READ`: `READ` would also let it `RECEIVE` a death message or `REAP` a corpse
/// out from under the real supervisor, and a domain names its members without acting on them. A
/// named slot, not the next free one, so it does not move with whatever else a line granted (as §67
/// (a program's second stream is a declaration) did for diagnostics).
const DOMAIN_SLOT: u64 = grant_plan::DOMAIN_SLOT;

/// The declared second stream (DECISIONS §67): complaints about the run, never about the domain.
const DIAG_SLOT: u64 = grant_plan::DIAGNOSTICS_SLOT;

/// Whether this process was granted a second stream, decided once in [`_start`].
static HAS_DIAG: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    HAS_DIAG.store(is_granted(DIAG_SLOT), Ordering::Relaxed);

    // **Walk first, complain second, print third.** A survey cannot know its complaints up front,
    // and §67's reader drains diagnostics to end-of-stream *before* it reads a byte of output.
    // `MAX_ROWS` is the kernel's whole thread table, so truncation is unreachable: two kilobytes on
    // a twelve-page stack.
    let mut rows = [ps::Row::default(); ps::MAX_ROWS];
    let mut found = ps::collect(&mut rows, &mut |cursor| survey(DOMAIN_SLOT, cursor));
    // **The `TIME` column is a second walk** (milestone 282 (a thread's CPU time, and the `top` it
    // makes possible)): `SURVEY` answers one record per call, so the two facts are two walks joined
    // on the tid, over the same endpoint and right as the listing above.
    found.join_cpu_time(&mut |cursor| {
        survey_record(DOMAIN_SLOT, cursor, abi::survey::record::CPU_TIME)
    });

    found.write_diagnostics(&mut |bytes| write_on(diag_slot(), bytes));
    diag_end();

    found.write_report(&mut |bytes| write_on(REPORT, bytes));
    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// Which endpoint a complaint goes to: the declared second stream when there is one, and the output
/// otherwise. The fallback is honest rather than a silent drop, and it is what the kernel's own
/// tests spawn this program with, since they wire no second sink.
fn diag_slot() -> u64 {
    if HAS_DIAG.load(Ordering::Relaxed) {
        DIAG_SLOT
    } else {
        REPORT
    }
}

/// **Close the second stream**, which is not tidiness: its reader waits for the end before it reads
/// the first, so a `ps` that exited without this would leave the prompt blocked.
fn diag_end() {
    if HAS_DIAG.load(Ordering::Relaxed) {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }
}

/// Write bytes to an endpoint under the sink contract, sixteen at a time. No newline is added: the
/// listing already carries its own, because where a line ends is the table's business and not the
/// transport's.
fn write_on(slot: u64, bytes: &[u8]) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(slot, w0, w1, w2);
        rest = &rest[n..];
    }
}

user_mode_runtime::panic_handler!();
