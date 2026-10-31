//! `testing::EveryCoreTickBudget` and `testing::wait_on_every_core`, the seeded fuzz drivers' wait:
//! a core that runs nothing holds the wait open, a machine that runs and still does not finish ends
//! it, and the backstop bounds it. See notes/kernel-test-harness.md.
//!
//! A host that stops running one vCPU looks, from inside the guest, like a core that takes no ticks
//! while the counter runs on. A core spinning with its interrupts masked gives the guest the same
//! observation, so these tests stand one in for the other.
//!
//! Name: provisional, minted 2026-10-31 (UTC) by lane/fuzz-deadlines-count-guest-progress.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::sched;
use crate::testing::{EveryCoreTickBudget, wait_on_every_core};

/// What the stand-in core has done: 1 once its interrupts are masked, 2 once it has unmasked them.
static STAGE: AtomicU32 = AtomicU32::new(0);

/// Mask interrupts on a core other than this one for `seconds` of the counter, then unmask them
/// and set [`STAGE`] to 2. Returns once they are masked. `None` on a one-core machine.
fn stop_another_core_for(seconds: u64) -> Option<()> {
    let here = crate::cpu::id();
    let target = crate::smp::online_cpus().find(|&c| c != here)?;
    STAGE.store(0, Ordering::SeqCst);
    sched::spawn_on(target, move || {
        let was_enabled = crate::arch::interrupts::disable();
        STAGE.store(1, Ordering::SeqCst);
        crate::arch::timer::spin_for(seconds * crate::arch::timer::frequency());
        crate::arch::interrupts::restore(was_enabled);
        STAGE.store(2, Ordering::SeqCst);
    })
    .expect("spawn_on failed");
    assert!(
        wait_on_every_core(|| STAGE.load(Ordering::SeqCst) >= 1).is_ok(),
        "the stand-in never masked its interrupts on core {target}"
    );
    Some(())
}

/// **A wait on every core outlasts a core that takes no ticks.** Another core masks its interrupts
/// for three seconds of the counter, a second longer than the wait's budget, and then finishes the
/// awaited work. A wall-clock deadline of two seconds gives up first; a budget of every core's
/// ticks sees that one core has not run and waits for it.
///
/// Name: provisional (lane/fuzz-deadlines-count-guest-progress, 2026-10-31 UTC).
///
/// Falsification: replayable `system_tests/falsifications/user.every_core_tick_budget_tests.a_wait_on_every_core_outlasts_a_core_that_takes_no_ticks.patch`
#[test_case]
fn a_wait_on_every_core_outlasts_a_core_that_takes_no_ticks() {
    if stop_another_core_for(3).is_none() {
        crate::testing::skip!("one core online: no other core to stop");
    }
    if let Err(budget) = wait_on_every_core(|| STAGE.load(Ordering::SeqCst) == 2) {
        panic!("the wait gave up while a core took no ticks: {budget}");
    }
}

/// **A wait on every core still gives up on work that never finishes.** Every core runs and takes
/// its ticks, so the budget runs out on ticks, not on the backstop: a real hang is still reported,
/// after two seconds of running on every core.
///
/// Name: provisional (lane/fuzz-deadlines-count-guest-progress, 2026-10-31 UTC).
#[test_case]
fn a_wait_on_every_core_gives_up_on_work_that_never_finishes() {
    let Err(budget) = wait_on_every_core(|| false) else {
        panic!("a wait on a property that is never true succeeded");
    };
    assert!(
        !budget.is_past_backstop(),
        "every core was running, so the ticks should have ended the wait: {budget}"
    );
}

/// **The backstop bounds a wait on a core that never ticks.** Another core masks its interrupts for
/// two seconds of the counter against a budget with a one-second backstop, so the budget ends on
/// the backstop while that core is still silent.
///
/// Name: provisional (lane/fuzz-deadlines-count-guest-progress, 2026-10-31 UTC).
#[test_case]
fn the_backstop_bounds_a_wait_on_a_core_that_never_ticks() {
    if stop_another_core_for(2).is_none() {
        crate::testing::skip!("one core online: no other core to stop");
    }
    let mut budget = EveryCoreTickBudget::new(2 * crate::arch::timer::TICK_HZ, 1);
    while !budget.is_expired() {
        sched::yield_now();
    }
    assert!(
        budget.is_past_backstop() && STAGE.load(Ordering::SeqCst) == 1,
        "the budget should have ended on its backstop with the other core still masked: {budget}"
    );
    assert!(
        wait_on_every_core(|| STAGE.load(Ordering::SeqCst) == 2).is_ok(),
        "the stand-in core never unmasked its interrupts"
    );
}
