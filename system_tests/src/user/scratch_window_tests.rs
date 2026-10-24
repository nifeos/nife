//! A userspace builder keeps building past its scratch window (milestone 604 (provisional), the
//! builder's scratch cursor is bounded).
//!
//! `supervision_protocol`'s loader maps every page it fills for a child into the builder's own
//! space, inside a fixed window it wraps around, probing for free pages. This proves the kernel's
//! half: destroying a child's region takes the builder's scratch mappings of its frames back
//! (DECISIONS §13 (frame revocation)), and `PageFrame::MAP` refuses a taken page whole, so a probe
//! is safe. The builder is `fixtures/src/scratch_window_exerciser.rs`, whose header has the numbers
//! and the negative control. One body serves every ISA (DECISIONS §19 (architectural parity is a
//! tenet)); `x86_64` and `riscv64` build one more scratch page per child, the timebase page.

//! # BUGS
//!
//! - CI does not run it: the whole suite skips it, and it runs only when named
//!   (`script/test --test scratch_window`).
//! - It is slow in the suite (104 s on aarch64, past `x86_64`'s leg timeout): milestone 658 (a
//!   region reap scans every mapping on the machine per page).
//! - The test keeps 32 frames, unexplained.
//! - It proves the loader, not the progenitor.
//!
//! notes/unmap.md, "The scratch window test", has each in full.

use super::*;
use crate::cap::{Rights, memory_region_cap, memory_region_lending_capability, rendezvous_cap};
use crate::sched;

/// The exerciser's building budget: 7 extra stack pages and 720 for one child at a time, with 33
/// over. Must cover `EXTRA_STACK_PAGES + CHILD_PAGES` in the fixture.
const BUDGET_PAGES: u64 = 760;

/// **The exerciser's own table budget, and the negative control.** 48 pages covers the scratch
/// window's 32 last-level tables and the one or two above them; the old cursor would have needed
/// 53 for this run.
const OWN_TABLE_PAGES: u64 = 48;

/// The builds the exerciser is written to make.
const BUILDS: u64 = 40;

/// How long to wait for the report, run by name: 5 s measured on aarch64 alone, so twelve times
/// that. In the whole suite it took 104 s (see this module's BUGS), which is why it does not run
/// there.
const WAIT_SECS: u64 = 60;

/// Why the whole suite skips this test.
const OPT_IN: &str = "opt-in: 104 s in CI's whole aarch64 suite, and past x86_64's leg timeout, \
                      because every region reap scans every live mapping log per page; run it \
                      with `script/test --test scratch_window` (milestone 604)";

/// **A builder with a 48-page table budget builds 40 `ripgrep`-sized children in a row**, three
/// more than a climbing cursor manages on that budget, and 1.6 laps of its scratch window.
#[test_case]
fn a_builder_reuses_scratch_its_reaped_children_gave_back() {
    if !crate::testing::run_was_filtered() {
        crate::testing::skip!(OPT_IN);
    }
    use core::sync::atomic::Ordering;

    use crate::arch::exceptions::USER_FAULTS;

    let image = program("scratch_window_exerciser")
        .expect("no scratch_window_exerciser program in the archive");
    let region = crate::memory_region::create(BUDGET_PAGES).expect("no region for the exerciser");
    let tables =
        crate::memory_region::create(OWN_TABLE_PAGES).expect("no table region for the exerciser");
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let faults = USER_FAULTS.load(Ordering::Relaxed);

    let tid = sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    memory_region_lending_capability(region), // slot 0: the budget
                    rendezvous_cap(report, Rights::WRITE),    // slot 1: the report
                    memory_region_cap(tables),                // slot 2: its own page tables
                ],
                maps: &[],
            },
        )
    })
    .expect("could not spawn the scratch window exerciser");

    // Bounded, so a builder that faults says so here rather than as the suite's watchdog.
    let start = crate::arch::timer::now();
    let deadline = start + WAIT_SECS * crate::arch::timer::frequency();
    while sched::rendezvous_waiting_senders(report) == 0 && crate::arch::timer::now() < deadline {
        sched::yield_now();
    }
    assert!(
        sched::rendezvous_waiting_senders(report) > 0,
        "the exerciser never reported: it faulted ({} faults) or is still building after \
         {WAIT_SECS} s",
        USER_FAULTS.load(Ordering::Relaxed) - faults,
    );
    let [built, pages, window, ..] = sched::ipc_receive(report);
    crate::println!(
        "    {built} builds, {pages} scratch pages, in {} s",
        (crate::arch::timer::now() - start) / crate::arch::timer::frequency(),
    );

    assert_eq!(
        built,
        BUILDS,
        "build {} of {BUILDS} failed after {pages} scratch pages, against a window of {window}: \
         the loader did not reuse the pages a destroyed region gave back",
        built + 1,
    );
    assert!(
        pages > window * 3 / 2,
        "{pages} scratch pages is not one and a half laps of the {window}-page window, so \
         this run proves less than it claims",
    );

    assert!(
        wait_for(|| !sched::is_thread_present(tid)),
        "the exerciser reported and did not exit",
    );
    assert_eq!(
        USER_FAULTS.load(Ordering::Relaxed),
        faults,
        "the exerciser faulted",
    );
    sched::reclaim_region(region).expect("the exerciser's budget did not come back");
    sched::reclaim_region(tables).expect("the exerciser's table budget did not come back");
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}
