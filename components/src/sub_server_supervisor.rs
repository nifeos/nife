//! **The sub-server's supervisor: restart policy, in userspace, holding nothing** (milestone 22
//! phase B.2).
//!
//! The kernel turns a death into a message (DECISIONS §26); this decides what to do about it:
//! bounded retries, a clean exit read as finished, and a give-up. It holds a request channel to the
//! spawner, its children's death channel, a WRITE view of a report endpoint and no untyped, so a
//! compromised supervisor is a restart loop, not a foothold. It reaps its own children (`reap`)
//! and tells its two children apart by the label the kernel returns with each death (§148 (resolves
//! by asking the kernel) as amended 2026-10-04). See notes/trusted-init/supervision-tree.md.
//!
//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `subsup`. Refused `subsup` and
//! `sub_supervisor`, which is ambiguous in the way that matters: this supervises **a sub-server**,
//! rather than being a supervisor beneath another one. "Sub-server" was already established
//! vocabulary here, 44 occurrences across the decisions, `supervision_protocol`, the kernel and the
//! notes, so the name is built from a word the reader has met.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

// Each binary in the tree compiles the shared module but uses a different slice of it (the sub-server
// builds nothing, the supervisor holds no memory), so the unused halves are expected, not dead.
use supervision_protocol::{
    REP_BUILT, REPORT_FAILED, REPORT_SUP_GAVE_UP, REPORT_SUP_SAW_DEATH, REQ_BUILD,
    SUB_SERVER_LABELS,
};
use user_mode_runtime::{receive, receive_fault, send};

/// What `root_supervisor` endowed us with, in order. Notice what is missing: memory.
const REQ: u64 = 0; // WRITE: ask the spawner to build or reap
const REP: u64 = 1; // READ: its answers
const FAULT: u64 = 2; // READ: our children's deaths, kernel-stamped and labelled
const REPORT: u64 = 3; // WRITE: what we saw and what we decided

/// **The policy.** How many times we will rebuild one crashing sub-server before we stop. Bounded,
/// because a deterministic fault restarts into the same fault forever; this is the crash-loop guard
/// that §26 refused to put in the kernel. Two is enough to prove the shape and small enough that a
/// broken child does not spin the machine.
const MAX_RESTARTS: u64 = 2;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    // One entry per child: its label, the attempt it is on, and how often it has been restarted.
    // The label is the only thing that says which entry a death belongs to.
    let mut children = SUB_SERVER_LABELS.map(|(label, first)| Child {
        label,
        attempt: first,
        restarts: 0,
        running: false,
    });
    for child in &mut children {
        if !build(child.attempt, child.label) {
            send(REPORT, REPORT_FAILED, 20, 0);
            supervision_protocol::fail()
        }
        child.running = true;
    }

    while children.iter().any(|c| c.running) {
        // The kernel is the only sender here, so the tid and the label are trustworthy without
        // asking anyone (§26 (the fault endpoint), item 5; and §148 as amended, for the label).
        let (event, tid, _pc, _addr, _reserved, label) = receive_fault(FAULT);
        send(REPORT, REPORT_SUP_SAW_DEATH, label, event);

        // Reap first, either way: the corpse is dead-until-reaped, so its region stays pinned until we
        // say so. Reaping a finished child matters as much as reaping a crashed one.
        if !reap(tid) {
            send(REPORT, REPORT_FAILED, 22, 0);
            supervision_protocol::fail()
        }

        // **Which child died is the label, and nothing else** (milestone 105). A label we did not
        // give out, including the 0 an unlabelled capability would bring, means we cannot say whose
        // restart policy applies, so we stop rather than guess.
        let Some(child) = children.iter_mut().find(|c| c.running && c.label == label) else {
            send(REPORT, REPORT_FAILED, 23, label);
            supervision_protocol::fail()
        };
        child.running = false;

        if event != abi::fault::EVENT_FAULT {
            // A clean exit is not a reason to restart, which is why §26 delivers both events.
            continue;
        }

        child.restarts += 1;
        if child.restarts > MAX_RESTARTS {
            send(REPORT, REPORT_SUP_GAVE_UP, child.restarts, 0);
            continue;
        }
        child.attempt += 1;
        if !build(child.attempt, child.label) {
            send(REPORT, REPORT_FAILED, 21, 0);
            continue;
        }
        child.running = true;
    }

    // Park rather than exit. Exiting would make *us* a death our own supervisor has to handle, and a
    // supervisor whose last act is to create work for its parent is a poor one.
    loop {
        let (event, _tid, _pc, _addr, _reserved, label) = receive_fault(FAULT);
        send(REPORT, REPORT_SUP_SAW_DEATH, label, event);
    }
}

/// One supervised child, as far as this supervisor knows it.
struct Child {
    /// What the spawner stamped on its supervision capability, and the kernel hands back when it dies.
    label: u64,
    /// The attempt the current or last instance was started as.
    attempt: u64,
    /// How many times it has been rebuilt after a crash.
    restarts: u64,
    /// An instance is alive, so a death with this label is expected.
    running: bool,
}

/// Ask the spawner to build the next instance of one child. We name the attempt and the label; it
/// builds, stamps the label where the kernel will find it, and tells us nothing but whether it
/// worked. The kernel names the child, by tid and by label, on the death message, when the time comes.
fn build(attempt: u64, label: u64) -> bool {
    send(REQ, REQ_BUILD, attempt, label);
    let (verdict, _, _) = receive(REP);
    verdict == REP_BUILT
}

/// **Collect the corpse** (§32): a method on the supervision endpoint we already hold, naming the
/// tid the kernel stamped on the death message we just received. We hold no memory and cannot build
/// anything, and this does not change that: the pages go back to the spawner's budget, because the
/// spawner owns the region (§13). We can free our child's memory; we cannot spend it.
///
/// A `StillAlive` refusal cannot happen here, because we only ever call this on a tid that arrived
/// on a death message. It is reported rather than ignored anyway: if it ever did happen it would mean
/// the kernel had told us a thread was dead and then said otherwise, which is worth failing loudly
/// over rather than silently leaking a corpse.
fn reap(tid: u64) -> bool {
    user_mode_runtime::reap(FAULT, tid) == 0
}

user_mode_runtime::panic_handler!();
