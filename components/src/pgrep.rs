//! **`pgrep`: name the members of a domain that match, and do nothing to them** (milestone 126,
//! notes/process-view.md).
//!
//! Walk one supervision domain with `abi::rendezvous::SURVEY`, filter it, then say what matched on
//! one stream and what went wrong on the other. The walk is `crates/ps` and the filter is
//! `crates/pgrep`, both host-tested; what lives here is the syscall and the two sinks. There is no
//! `pkill`: a domain names its members and does not act on them (calef, 2026-08-17), and no method
//! turns a tid into authority over its thread (notes/process-view/pgrep.md has the argument and
//! the examples at the prompt).

//! # Capability contract
//!
//! | slot | what | why |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the tids go |
//! | 7 | the process domain, `ENUMERATE` | the supervision endpoint whose members it may **name** |
//! | 8 | the diagnostics sink, `WRITE` | where a refusal goes, so `>` cannot swallow it |
//!
//! **Exactly `ps`'s three.** No memory grant, file, directory or clock, and no argument a person
//! could type widens any of it.

//! # BUGS
//!
//! - **At this prompt it always prints every member**: the selector arrives in a register the
//!   shell cannot set (`crates/pgrep`'s `BUGS`), so `pgrep` here is `pgrep '*'`.
//! - **`pgrep` names itself**, as `running`, the same as Unix's `pgrep` without `-x`.
//! - **A tid is a name and not a handle, so the answer goes stale**, and there is no operation to
//!   aim at a stale tid.
//!
//! Name: ratified 2026-09-14 (calef, working the unratified worklist), on milestone 126 (the `procps` package). Upstream's
//! name, shared with `crates/pgrep` as `ps` and `crates/ps` share theirs.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use user_mode_runtime::{exit, is_granted, send, survey};

/// The output sink: where the tids go. Slot 0 is where every spawned program's output lands.
const REPORT: u64 = 0;

/// **The process domain**: a supervision endpoint carrying `ENUMERATE`. Everything this program can
/// see comes through here, and nothing else in the capability table names a process.
const DOMAIN_SLOT: u64 = grant_plan::DOMAIN_SLOT;

/// The declared second stream (DECISIONS §67): complaints about the run, never about the domain.
const DIAG_SLOT: u64 = grant_plan::DIAGNOSTICS_SLOT;

/// Whether this process was granted a second stream, decided once in [`_start`].
static HAS_DIAG: AtomicBool = AtomicBool::new(false);

user_mode_runtime::program_entry!(start);

/// `a1` is the state mask, in `grant_plan::spawnproto`'s integer-argument position: the progenitor starts every
/// child `start_child(child, 0, arg, 0)`, so the endowment's `arg` is the second register and the first
/// is always zero. Zero means every state (`pgrep::Selector::from_wire`), which is what a shell that
/// cannot spell a selector sends and is why an unfiltered `pgrep` prints the whole domain rather than
/// nothing.
fn start(_a0: u64, a1: u64, _a2: u64) -> ! {
    HAS_DIAG.store(is_granted(DIAG_SLOT), Ordering::Relaxed);

    // **Walk first, filter second, complain third, print fourth.** DECISIONS §67's reader drains
    // diagnostics to end-of-stream before it reads a byte of output, and neither the complaints nor
    // "nothing matched" (which needs the count) is known until the walk ends. `MAX_ROWS` is the
    // kernel's whole thread table, so truncation is unreachable: 2 KiB on a twelve-page stack.
    let mut rows = [ps::Row::default(); ps::MAX_ROWS];
    let walked = ps::collect(&mut rows, &mut |cursor| survey(DOMAIN_SLOT, cursor));
    let found = pgrep::select(&walked, pgrep::Selector::from_wire(a1));

    found.write_diagnostics(&mut |bytes| write_on(diag_slot(), bytes));
    diag_end();

    found.write_report(&mut |bytes| write_on(REPORT, bytes));
    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// Which endpoint a complaint goes to: the declared second stream when there is one, and the output
/// otherwise. The fallback is honest rather than a silent drop, and it is what the kernel's own tests
/// spawn a program with, since they wire no second sink.
fn diag_slot() -> u64 {
    if HAS_DIAG.load(Ordering::Relaxed) {
        DIAG_SLOT
    } else {
        REPORT
    }
}

/// **Close the second stream**, which is not tidiness: its reader waits for the end before it reads
/// the first, so a `pgrep` that exited without this would leave the prompt blocked.
fn diag_end() {
    if HAS_DIAG.load(Ordering::Relaxed) {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }
}

/// Write bytes to an endpoint under the sink contract, sixteen at a time. No newline is added: the
/// listing already carries its own, because where a line ends is the answer's business and not the
/// transport's.
fn write_on(slot: u64, bytes: &[u8]) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(slot, w0, w1, w2);
        rest = &rest[n..];
    }
}

// Nothing here should panic: `ps::collect` is total for every reader and `pgrep::select` only reads
// the rows it returned. A fault the kernel turns into a kill is the honest signal if one ever does,
// and the macro's handler over `user_mode_runtime::trap` gives that signal on all three ISAs.
user_mode_runtime::panic_handler!();
