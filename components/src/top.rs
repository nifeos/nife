//! **`top`: the domain `ps` lists, ordered by what it is costing** (§150 (how does a thread's CPU
//! time reach userspace?)). It walks one supervision domain twice, for run states and for CPU time,
//! ranks by the second, reports on [`REPORT`] and complains on [`DIAG_SLOT`]. The walk and table
//! are `crates/ps`, the summary `crates/top`; this is the syscalls and the two sinks. It holds
//! `ps`'s authority plus one read-only page (slot 11, the machine statistics page, for the machine
//! line that is what became of `tload`, §225 (`free` sees the machine and your share)), so it can
//! name no process outside its domain. No clock: uptime is the ambient monotonic counter. CPU time
//! is continuous, so two runs show another thread's work between them; §150 accepted that, because
//! `ENUMERATE` is already the right to learn what exists. See notes/process-view/top.md.

//! # BUGS
//!
//! - It does not refresh: an interval spin would be charged to this thread by the counter the table
//!   ranks on (`crates/top`'s `BUGS` has the rest).
//! - `top` lists itself, `running` and still accumulating, as Unix's does.
//! - Everything `ps` records applies: no `CMD` column, whole generational tids, snapshot rows.
//!
//! Name: ratified 2026-10-06 (calef): the standard term for a ranked view. It stays its own program
//! rather than a flag on `ps`, by calef's one-program-one-thing preference.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates: one lint policy)'s ratchet
// tracks (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`)): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use user_mode_runtime::{exit, is_granted, monotonic_nanos, send, survey, survey_record};

/// The output sink: where the summary and the table go. Slot 0 is where every spawned program's
/// output lands.
const REPORT: u64 = 0;

/// **The process domain** (slot 7): a supervision endpoint with `ENUMERATE`, whose members this
/// program may name. Everything it can see comes through here, and both walks use it.
const DOMAIN_SLOT: u64 = grant_plan::DOMAIN_SLOT;

/// The declared second stream (slot 8, DECISIONS §67 (a program's second stream is a declaration,
/// not a number)): complaints about the run, never about the domain, so `>` cannot swallow them.
const DIAG_SLOT: u64 = grant_plan::DIAGNOSTICS_SLOT;

/// Whether this process was granted a second stream, decided once in [`_start`].
static HAS_DIAG: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    HAS_DIAG.store(is_granted(DIAG_SLOT), Ordering::Relaxed);

    // Walk first, complain second, print third (§67): a survey cannot know its complaints up front,
    // and the second stream's reader drains it to end-of-stream before it reads the first.
    let mut rows = [ps::Row::default(); ps::MAX_ROWS];
    let mut found = ps::collect(&mut rows, &mut |cursor| survey(DOMAIN_SLOT, cursor));
    found.join_cpu_time(&mut |cursor| {
        survey_record(DOMAIN_SLOT, cursor, abi::survey::record::CPU_TIME)
    });
    // The ranking is the program. Everything above it is `ps`.
    found.rank_by_cpu_time();

    found.write_diagnostics(&mut |bytes| write_on(diag_slot(), bytes));
    diag_end();

    // The summary goes out only when the table does: above a refusal it would count rows about to
    // be declined, the "plausible listing of nothing" `ps::Survey::write_report` refuses.
    if found.is_complete() && !found.rows().is_empty() {
        top::write_summary(found.rows(), monotonic_nanos(), &mut |bytes| {
            write_on(REPORT, bytes);
        });
        // What became of `tload` (milestone 126, DECISIONS §225): the machine's run queue and busy
        // share, from the machine statistics page when the owner granted it.
        let machine = if is_granted(grant_plan::MACHINE_SLOT) {
            // SAFETY: granted only alongside a read-only mapping of the same frame at `PAGE_VA`,
            // which lives as long as this process (`system_initializer`'s spawn service).
            unsafe {
                machine_statistics_contract::Snapshot::read(machine_statistics_contract::PAGE_VA)
            }
        } else {
            None
        };
        top::write_machine_line(machine.as_ref(), &mut |bytes| write_on(REPORT, bytes));
    }
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
/// the first, so a `top` that exited without this would leave the prompt blocked.
fn diag_end() {
    if HAS_DIAG.load(Ordering::Relaxed) {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }
}

/// Write bytes to an endpoint under the sink contract, sixteen at a time. No newline is added:
/// where a line ends is the table's business and not the transport's.
fn write_on(slot: u64, bytes: &[u8]) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(slot, w0, w1, w2);
        rest = &rest[n..];
    }
}

user_mode_runtime::panic_handler!();
