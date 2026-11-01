//! **Whether the shell gets a `--mem` grant back after the job that held it built from it.**
//!
//! A program the shell starts with `--mem N` holds the grant as `SPEND_ONLY`, and every spending
//! method asks for `WRITE` alone, so the program may `SPLIT` it and build a process in it. When the
//! job ends, the shell gives the grant back with `release_budget` in `components/src/swish/jobs.rs`,
//! which retries the `DESTROY` for up to a second (`swish::reclaim_budget_within`), stops at once on
//! a live sub-region, and then deletes its slot. These tests take the kernel's side of that sequence: what the `DESTROY` answers when the
//! job left a sub-region behind, and when it left a process running.
//!
//! Each test stands in for the job with kernel calls on a region split from a stand-in for the
//! shell's budget, and makes the shell's `DESTROY` through the real dispatcher.
//!
//! Name: provisional, minted 2026-10-31 (UTC) by lane/mem-budget-reclaim, for the module and the file.

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::capability_object::{memory_region_capability, memory_region_lending_capability};
use crate::sched;
use crate::syscall::invoke;

/// A one-instruction runaway, as `force_kill_tests` has it: the process a job built and left
/// running when it exited.
#[cfg(target_arch = "aarch64")]
const SPIN_STUB: &[u32] = &[0x1400_0000]; // b .
#[cfg(target_arch = "riscv64")]
const SPIN_STUB: &[u32] = &[0x0000_006F]; // j .  (jal x0, 0)
#[cfg(target_arch = "x86_64")]
const SPIN_STUB: &[u32] = super::x86_programs::SPIN;

/// The stand-in for the shell's budget: two grants' worth.
const SHELL_PAGES: u64 = 32;

/// The `--mem` grant: what `build_child` carves for one process.
const GRANT_PAGES: u64 = 16;

/// The budget a job splits off its grant for a child of its own.
const SUB_PAGES: u64 = 4;

/// `invoke(slot, method, a0)` through the real dispatcher.
fn call(slot: u64, method: u64, a0: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, 0, 0)
}

/// The pages of `region` spent and not given back.
fn spent(region: u64) -> u64 {
    crate::memory_region::usage(region)
        .expect("the region exists")
        .0
}

/// **A grant the job split from is refused for good**, so the shell's one `DESTROY` loses it.
///
/// The job splits a budget off its `SPEND_ONLY` grant, which `WRITE` alone permits, and exits. Its
/// table goes with it, so nothing names the sub-region. The shell's `DESTROY` refuses with
/// `HasChildren`, and keeps refusing however long it waits. The grant's pages stay spent in the
/// shell's budget.
///
/// Falsification: unfalsified. Dropping the `has_children` refusal in `sched::reclaim_region` should
/// fail the first assertion; no patch replays it yet.
#[test_case]
fn a_memory_grant_the_job_split_is_refused_to_the_shell_for_good() {
    let shell = crate::memory_region::create(SHELL_PAGES).expect("no shell budget");
    let mem = crate::memory_region::split(shell, GRANT_PAGES).expect("no --mem grant");

    // The job's view of its grant, as the progenitor endows it.
    let job = sched::grant(memory_region_capability(mem)).expect("grant the job its --mem");
    let sub_slot = call(job, abi::memory_region::SPLIT, SUB_PAGES)
        .expect("a SPEND_ONLY grant refused SPLIT, which asks for WRITE alone")
        as u64;
    let crate::capability_object::Object::MemoryRegion(sub) = sched::current_capability(sub_slot)
        .expect("the split's slot")
        .object
    else {
        panic!("SPLIT filed something other than a region");
    };
    // The job exits: its slots go with its region.
    let _ = sched::delete_current_capability(sub_slot);
    let _ = sched::delete_current_capability(job);

    // The shell's copy carries every right (`delegate(slot, abi::rights::ALL)` keeps one).
    let shell_slot = sched::grant(memory_region_lending_capability(mem)).expect("the shell's copy");
    // What the shell can see: the refusal is one `NotPermitted`, and `USAGE`'s `CHILDREN` record is
    // how it tells a sub-region from a resident (`swish::reclaim_budget_within`).
    assert_eq!(
        call(shell_slot, abi::memory_region::USAGE, abi::usage::CHILDREN),
        Ok(SUB_PAGES as i64),
        "the shell's copy cannot see the sub-region the job left",
    );
    for attempt in 0..64 {
        assert_eq!(
            call(shell_slot, abi::memory_region::DESTROY, 0),
            Err(Error::NotPermitted),
            "attempt {attempt}: a grant with a live sub-region was destroyed",
        );
        assert!(
            matches!(
                sched::last_reclaim_refusal(),
                Some(sched::ReclaimRefusal::HasChildren)
            ),
            "attempt {attempt}: refused for {:?}, not for the sub-region",
            sched::last_reclaim_refusal(),
        );
        sched::yield_now();
    }
    assert!(
        spent(shell) >= GRANT_PAGES,
        "the shell's budget got the grant back although DESTROY refused it",
    );

    // Only a holder of the sub-region can undo it, and after the job exits that is nobody. The
    // kernel can, which is how this test tidies.
    let _ = sched::delete_current_capability(shell_slot);
    sched::reclaim_region(sub).expect("the orphaned sub-region");
    sched::reclaim_region(mem).expect("the grant, once childless");
    assert_eq!(
        spent(shell),
        0,
        "the shell's budget did not come back whole"
    );
    sched::reclaim_region(shell).expect("the shell's budget");
}

/// **A grant holding a process the job left running is refused once, then given back.**
///
/// The job built a process in its grant and exited with it still running. The shell's first
/// `DESTROY` arms the process's kill and refuses. A retry once the kill lands succeeds, which is
/// why `release_budget` retries as `jobs::reclaim` does, and the shell's budget comes back whole.
///
/// Falsification: unfalsified. Treating a `Running` resident as reapable in `region_reap_verdict`
/// should fail the first assertion; no patch replays it yet.
#[test_case]
fn a_memory_grant_holding_a_running_process_is_refused_once_then_reclaimed() {
    let shell = crate::memory_region::create(SHELL_PAGES).expect("no shell budget");
    let mem = crate::memory_region::split(shell, GRANT_PAGES).expect("no --mem grant");
    let _tid = super::supervision_tests::build_child_with(mem, SPIN_STUB, &[], None);
    // Let it reach EL0, so the teardown meets a running thread rather than an embryo.
    for _ in 0..8 {
        sched::yield_now();
    }

    let shell_slot = sched::grant(memory_region_lending_capability(mem)).expect("the shell's copy");
    assert_eq!(
        call(shell_slot, abi::memory_region::DESTROY, 0),
        Err(Error::NotPermitted),
        "the first DESTROY reclaimed a region a running thread occupies",
    );
    assert!(
        matches!(
            sched::last_reclaim_refusal(),
            Some(sched::ReclaimRefusal::Resident { .. })
        ),
        "refused for {:?}, not for the running process",
        sched::last_reclaim_refusal(),
    );
    assert!(
        spent(shell) >= GRANT_PAGES,
        "the shell's budget got the grant back although DESTROY refused it",
    );

    // A second for the kill to land on another core's tick, as `force_kill_tests` allows.
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency();
    let mut reclaimed = false;
    while crate::arch::timer::now() < deadline {
        if call(shell_slot, abi::memory_region::DESTROY, 0) == Ok(0) {
            reclaimed = true;
            break;
        }
        sched::yield_now();
    }
    assert!(
        reclaimed,
        "a retried DESTROY never reclaimed the grant once its process was killed",
    );
    assert_eq!(
        spent(shell),
        0,
        "the shell's budget did not come back whole"
    );
    let _ = sched::delete_current_capability(shell_slot);
    sched::reclaim_region(shell).expect("the shell's budget");
}
