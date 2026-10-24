//! **A timed window can exclude preemption, and this is the test that says so** (milestone 541 (a
//! timed window that excludes preemption)).
//!
//! `bench/spawn_map.rs`'s `map_new` masks interrupts across its timed window, and the benchmark
//! build runs no tests. What the suite can test is the primitive that window rests on:
//! `arch::interrupts::disable` suppresses the scheduler tick, and `restore` delivers the held one.
//! Each window spins on the wall clock and then waits until the tick is architecturally pending
//! ([`until_the_tick_is_raised`]), because the emulator raises it late. See
//! notes/load-sensitive-assertions.md and notes/benchmarks.md.

use crate::sched;

/// How long to spin, in counter ticks: three scheduler tick periods, so a tick is unambiguously
/// due rather than marginally due. `frequency()` is counter ticks per second and `TICK_HZ` is
/// scheduler ticks per second, so their ratio is counter ticks per scheduler tick.
fn three_tick_periods() -> u64 {
    3 * crate::arch::timer::frequency() / crate::arch::timer::TICK_HZ
}

/// How long a masked window waits for the host to raise a tick that is already due, in scheduler
/// tick periods: one second, against a worst case measured at under nine periods (86 ms).
///
/// **A leak trap, not a timing claim.** Nothing the kernel does is inside this bound: the deadline
/// has passed and the rest is the emulator getting round to it. A timer that is genuinely broken
/// never raises at all, and this is what turns that into a failure that says so rather than a hang.
///
/// Name: provisional, minted 2026-09-24 (`cda66d656`, waiting for the tick to be raised).
const RAISE_BOUND_PERIODS: u64 = 100;

/// With interrupts **already masked**, spin until this core's tick is architecturally pending, or
/// the bound runs out. Returns whether it was raised.
///
/// This is the difference between "a tick was held" and "enough wall clock passed that a tick
/// should have been held". Only the first is the property these tests are about.
///
/// Name: provisional, minted 2026-09-24 (`cda66d656`, waiting for the tick to be raised).
fn until_the_tick_is_raised() -> bool {
    let bound = RAISE_BOUND_PERIODS * crate::arch::timer::frequency() / crate::arch::timer::TICK_HZ;
    let start = crate::arch::timer::now();
    while !crate::arch::timer::is_tick_pending() {
        if crate::arch::timer::now().wrapping_sub(start) >= bound {
            return false;
        }
        core::hint::spin_loop();
    }
    true
}

/// The message both tests give when the host never raised the tick at all.
const NEVER_RAISED: &str = "the timer was never raised in a second of masked spinning, so this \
     window held nothing and proves nothing: the timer is not being armed, or the emulator has \
     stopped delivering it";

/// **Interrupts masked means no preemption is taken, however long the window is.**
///
/// The spin is three tick periods, so without the mask the timer would fire at least twice; with
/// it, this core's count must not move. The count is per core, because the global one counts
/// other cores ticking their own threads. It asserts on the count rather than elapsed time,
/// because a cheap preemption and a full quantum differ sevenfold and only the count is zero or
/// not.
#[test_case]
fn a_masked_window_takes_no_preemption() {
    let was_enabled = crate::arch::interrupts::disable();
    let before = sched::preemptions_here();
    crate::arch::timer::spin_for(three_tick_periods());
    // A tick must be waiting, or the window is vacuous. Still masked, so still on this core.
    let raised = until_the_tick_is_raised();
    let during = sched::preemptions_here();
    crate::arch::interrupts::restore(was_enabled);

    assert!(raised, "{}", NEVER_RAISED);
    assert_eq!(
        during,
        before,
        "{} preemption(s) were taken inside a window with interrupts masked; map_new's timed \
         window is built on this not happening, and if it can happen the row is timing the \
         scheduler again",
        during - before,
    );
}

/// **And the interrupt is deferred, not lost**: a mask that dropped the tick would starve the
/// scheduler for the window's length.
///
/// The window waits for the pending bit before the mask comes off, so delivery at `restore` is a
/// matter of instructions, not of when the host raises the timer. The spin after `restore` is
/// bounded at one tick period, far beyond the few cycles real hardware takes to deliver.
#[test_case]
fn unmasking_delivers_the_tick_that_was_held() {
    let was_enabled = crate::arch::interrupts::disable();
    // The core is fixed under the mask: the preemption may migrate this thread, and every later
    // read must be of the core that held the tick.
    let cpu = crate::cpu::id();
    let count = || sched::preemptions_on(cpu);
    let before = count();

    crate::arch::timer::spin_for(three_tick_periods());
    let raised = until_the_tick_is_raised();
    crate::arch::interrupts::restore(was_enabled);

    assert!(raised, "{}", NEVER_RAISED);

    let start = crate::arch::timer::now();
    let period = three_tick_periods() / 3;
    while count() == before && crate::arch::timer::now().wrapping_sub(start) < period {
        core::hint::spin_loop();
    }
    let after = count();

    assert!(
        after > before,
        "a tick was pending when the mask came off and no preemption followed within a tick \
         period, so the tick held across the mask was dropped rather than deferred; masking a \
         benchmark window would then be starving the scheduler rather than excluding it",
    );
}
