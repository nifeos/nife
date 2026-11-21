//! Tests of the two-tier interrupt escalation policy.

use crate::test_support::plan;
use crate::*;

#[test]
fn least_authority_demo_and_memory_grant_depleter_are_not_interruptible() {
    // Fast jobs finish in one step; the shell just waits for them, no ^C tier.
    assert!(!Program::LeastAuthorityDemo.manifest().interruptible);
    assert!(!Program::MemoryGrantDepleter.manifest().interruptible);
    let CommandLine::Run(r) = parse(b"least_authority_demo 9") else {
        panic!()
    };
    assert!(!plan(&r, ShellHoldings::default()).unwrap().interruptible);
}

#[test]
fn first_interrupt_is_cooperative_second_is_forcible() {
    let mut e = Escalation::new();
    assert_eq!(e.on_interrupt(), InterruptAction::Cooperative);
    assert!(!e.is_spent());
    assert_eq!(e.on_interrupt(), InterruptAction::Forcible);
    assert!(e.is_spent());
    // Spent: further events do nothing.
    assert_eq!(e.on_interrupt(), InterruptAction::None);
    assert_eq!(e.on_tick(), InterruptAction::None);
}

#[test]
fn a_cooperative_signal_times_out_into_a_forcible_teardown() {
    // The "shell-side timeout": the first ^C asks nicely; if the job never exits, the grace
    // window elapsing tears it down without a second keystroke.
    let mut e = Escalation::new();
    assert_eq!(e.on_interrupt(), InterruptAction::Cooperative);
    // Ticks short of the window do nothing.
    for _ in 0..COOPERATIVE_GRACE_POLLS - 1 {
        assert_eq!(e.on_tick(), InterruptAction::None);
    }
    // The last tick of the window escalates.
    assert_eq!(e.on_tick(), InterruptAction::Forcible);
    assert!(e.is_spent());
}

#[test]
fn ticks_before_any_interrupt_do_nothing() {
    // No ^C yet: the grace window is not armed, so watching a well-behaved job never escalates.
    let mut e = Escalation::new();
    for _ in 0..COOPERATIVE_GRACE_POLLS * 2 {
        assert_eq!(e.on_tick(), InterruptAction::None);
    }
    assert!(!e.is_spent());
    // And a first ^C after a long quiet run is still cooperative.
    assert_eq!(e.on_interrupt(), InterruptAction::Cooperative);
}
