//! A wait the system tests share, kept outside any one test module.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
/// Spin the scheduler until `done()`, or give up after about 2 s of wall clock. Returns whether it
/// happened.
///
/// Time-based, not a yield count (§28 (SMP placement)): on an idle core a yield returns at once, so a
/// count elapses in no real time. Two seconds lets other cores finish and stays far under the 60 s
/// hang watchdog. Here rather than in `user::tests`, which not every target compiles. See
/// notes/hvf-leg.md.
pub fn wait_for(mut done: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if done() {
            return true;
        }
        crate::sched::yield_now();
    }
    done()
}
