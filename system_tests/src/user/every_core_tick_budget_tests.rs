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
use crate::testing::{
    BUDGET_EXPIRED_MARKER, EveryCoreTickBudget, capture_console, dump_threads_for_budget_expired,
    wait_on_every_core,
};

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
///
/// Falsification: replayable `system_tests/falsifications/user.every_core_tick_budget_tests.a_wait_on_every_core_gives_up_on_work_that_never_finishes.patch`
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
///
/// Falsification: replayable `system_tests/falsifications/user.every_core_tick_budget_tests.the_backstop_bounds_a_wait_on_a_core_that_never_ticks.patch`
#[test_case]
fn the_backstop_bounds_a_wait_on_a_core_that_never_ticks() {
    if stop_another_core_for(2).is_none() {
        crate::testing::skip!("one core online: no other core to stop");
    }
    let mut budget = EveryCoreTickBudget::new((2 * crate::arch::timer::TICK_HERTZ) as u32, 1);
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

/// **A budget expiry's message is followed by a thread dump, and the message is unchanged.** The
/// panic handler calls `dump_threads_for_budget_expired` with the panic's message after printing
/// it. This prints a message in the drivers' form and then makes that call, and checks that the
/// message line opens with the marker and keeps its `{ms} ms of the counter[ (the backstop)];
/// ticks taken of {n} on each core:` form, that the dump follows it with thread states and each
/// core's `inbox_len`, and that the dump repeats no marker and carries no replay line, so the
/// supervisor and the filer read the same thing with and without it. Another message gets no dump.
///
/// Name: provisional (lane/fuzz-budget-expired-thread-dump).
///
/// Falsification: replayable `system_tests/falsifications/user.every_core_tick_budget_tests.a_budget_expiry_message_is_followed_by_a_thread_dump.patch`
#[test_case]
fn a_budget_expiry_message_is_followed_by_a_thread_dump() {
    let Err(budget) = wait_on_every_core(|| false) else {
        panic!("a wait on a property that is never true succeeded");
    };
    let mut message = [0u8; 200];
    let message = {
        use core::fmt::Write;
        struct Cursor<'a>(&'a mut [u8], usize);
        impl Write for Cursor<'_> {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                let end = self.1 + s.len();
                self.0
                    .get_mut(self.1..end)
                    .ok_or(core::fmt::Error)?
                    .copy_from_slice(s.as_bytes());
                self.1 = end;
                Ok(())
            }
        }
        let mut cursor = Cursor(&mut message, 0);
        write!(
            cursor,
            "{BUDGET_EXPIRED_MARKER} actor 0 neither finished nor parked: {budget}"
        )
        .expect("the message fits");
        let end = cursor.1;
        core::str::from_utf8(&message[..end]).expect("ASCII")
    };
    capture_console(
        || {
            crate::println!("{message}");
            dump_threads_for_budget_expired(message);
        },
        |text| {
            // Another core may print around the lines under test, so find them by content.
            let at = text.find(message).expect("the message was printed");
            let first = &text[at..];
            let first = &first[..first.find('\n').expect("the message line ends")];
            assert_eq!(first, message, "the message line changed");
            assert!(
                first.starts_with("budget-expired: actor 0 neither finished nor parked: ")
                    && first.contains(" ms of the counter")
                    && first.contains("; ticks taken of 200 on each core:"),
                "the prefix and the budget's form are the parsed contract: {first}"
            );
            let dump = &text[at + first.len()..];
            assert!(
                dump.contains("--- thread dump"),
                "the dump follows the message: {dump}"
            );
            assert!(
                dump.contains(" state="),
                "the dump lists thread states: {dump}"
            );
            assert!(
                dump.matches(" inbox_len=").count() >= crate::smp::online_cpus().count(),
                "the dump lists every online core's inbox_len: {dump}"
            );
            assert!(
                !dump.contains(BUDGET_EXPIRED_MARKER) && !dump.contains("replay:"),
                "the dump must not carry what the supervisor greps for"
            );
        },
    );
    capture_console(
        || dump_threads_for_budget_expired("syscall fuzzer: seed 0x1 step 2: a mismatch"),
        |text| {
            assert!(
                text.is_empty(),
                "a message that is not a budget expiry got a dump: {text}"
            )
        },
    );
}
