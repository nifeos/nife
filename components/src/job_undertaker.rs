//! The interactive boot's undertaker: it collects the progenitor's dead jobs, which the progenitor,
//! parked in `RECEIVE` on the spawn channel for life with no non-blocking receive, cannot. Without
//! it every job's region stayed spent and a long session ran out of memory. It never restarts a
//! job, because a command a person typed should not restart when it ends.
//!
//! Its authority is three endpoints and nothing else ([`DEATHS`], [`REPORT`], [`SPAWN`]): no
//! untyped, frame, TCB or address space. `Rendezvous::REAP` is authorized by the supervision
//! relationship (§32 (a supervisor may collect a corpse without being able to build one)), and the
//! pages go back to the progenitor, so it can free a job's memory and never spend it. See
//! notes/supervision.md.

//! # BUGS
//!
//! - It can say one word: a refused reap of a dead thread traps, shown as "could not spawn".
//! - A reaped message can park behind one request on the spawn endpoint, after its collect.
//! - A fault report parks this loop when nobody reads the result endpoint, as after a faulting
//!   non-tail stage of a screen-narrowed line (notes/supervision.md).
//! - A refused reclaim is retried, since a job behind a directory grant shares its region with its
//!   caretaker ([`MAX_ATTEMPTS`]).
//! - It cannot reap a hung job (§32 refuses a live thread); `^C` twice covers what a person sees.
//!
//! Name: ratified 2026-08-03 (calef), replacing `job_reaper`. Refused `job_reaper` (named for what
//! it does to jobs) and `job_killer` (an authority it is denied). An undertaker arrives strictly
//! after death and never causes it, beside supervisor and caretaker.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use grant_plan::spawnproto;
use user_mode_runtime::{reap, receive_fault, send, yield_now};

/// The supervision endpoint the progenitor endows every job with, held `READ`: the right to receive deaths
/// here, which is the same right §32 makes the right to collect them.
const DEATHS: u64 = 0;

/// **The progenitor's result endpoint, held `WRITE`** (milestone 235): the one the shell reads a job's answer
/// off, and therefore the one a job's *absence* of an answer has to arrive on. Nothing else in this
/// process may be said here, and nothing else is: [`report`] sends one constant.
const REPORT: u64 = 1;

/// **The progenitor's spawn endpoint, held `WRITE` through a copy badged
/// [`spawnproto::UNDERTAKER_BADGE`]** (milestone 685): where [`announce`] says a job is reaped. No
/// `GRANT`, so the badge cannot be passed on, and the progenitor is the only receiver.
const SPAWN: u64 = 2;

/// How many times one corpse may be refused before this program treats it as a broken kernel
/// (milestone 31 phase 3).
///
/// A job behind a directory grant shares its region with the `fs_subtree_caretaker` that carries
/// the grant (DECISIONS §92 (a caretaker is supervised by the client it serves)), so the first
/// reclaim of that region is refused by construction: the sweep wakes the caretaker, arms the kill
/// of §16 (object revocation), and the retry after its next preemption reclaims
/// (notes/grant-expression/per-file-grants.md). The number is far past the one preemption that
/// needs, and running out is still [`fail`]: a corpse that never becomes collectable is a leak,
/// and a leak here ends the prompt's memory a few commands later with nothing to point at.
const MAX_ATTEMPTS: usize = 1024;

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    loop {
        // The kernel is the only sender on this endpoint (§26 clears the child's fault slot at
        // `START`), so the tid is trustworthy without a badge.
        let (event, tid, _pc, _addr, _reserved, label) = receive_fault(DEATHS);
        collect(tid);
        // Before the fault report: the progenitor may be waiting on this to finish a spawn, and the
        // report may park (this module's `BUGS`).
        announce(label, tid);
        // **Only a fault is news** (milestone 235). §26.3 flows clean exits down this endpoint too,
        // and every command a person runs ends in one; a word for those would arrive on the result
        // endpoint behind the child's own answer and the next command's read would take it. So the
        // ordinary path stays exactly as silent as it was.
        if event == abi::fault::EVENT_FAULT {
            report();
        }
    }
}

/// **Tell the progenitor job `label` (thread `tid`) is reaped** (milestone 685): its region is back
/// in the pool, and its file-service window, if it held one, is free.
///
/// **After [`collect`], always**: the message means the memory is back, and a progenitor that read
/// it before the reap would hand out a window or wait for a pool that has not changed. The return
/// value is dropped for [`report`]'s reason.
fn announce(label: u64, tid: u64) {
    let (w0, w1, w2) = spawnproto::reaped(label, tid);
    let _ = send(SPAWN, w0, w1, w2);
}

/// Tell whoever is waiting on the progenitor's result endpoint that the job they are waiting for is dead.
///
/// **After [`collect`], deliberately.** This `SEND` is a rendezvous with no non-blocking form, so it
/// can park (this module's `BUGS` says exactly when); a parked report must not be able to cost the
/// reclamation that keeps the prompt's memory coming back, and putting the collect first is what
/// makes that true rather than hoped.
///
/// The return value is dropped on purpose. The endpoint is the progenitor's and outlives this process, so the
/// only failure the kernel could return here is one this program has no second channel to report on
/// and no authority to do anything about.
fn report() {
    let _ = send(REPORT, spawnproto::JOB_FAULTED, 0, 0);
}

/// Collect one corpse, retrying while the region still holds something that can run.
///
/// The tid arrived on a death message from this very endpoint, so two of §32's refusals are
/// unreachable: it cannot be `StillAlive` and it cannot be another supervisor's child. The third,
/// `NotPermitted`, means the *region* is not yet reclaimable, which is a fact about the region's
/// other residents rather than about this corpse; see [`MAX_ATTEMPTS`]. Yielding between attempts
/// is what lets the armed kill be spent: the thread standing in the way has to reach a preemption,
/// and spinning without giving up the core would be racing it on a single-core machine.
fn collect(tid: u64) {
    for _ in 0..MAX_ATTEMPTS {
        if reap(DEATHS, tid) == 0 {
            return;
        }
        yield_now();
    }
    fail()
}

fn fail() -> ! {
    user_mode_runtime::trap()
}

user_mode_runtime::panic_handler!();
