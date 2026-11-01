//! Tests of retrying a region's return.
//!
//! Name: provisional, minted 2026-10-31 (UTC) by lane/mem-budget-retry.

use core::cell::Cell;

use super::*;

/// A region that refuses `refusals` times, as one holding a running process does until its kill
/// lands, and then answers zero. The clock advances 10 ms per wait, one kernel timeslice.
fn run(refusals: u32, patience_ns: u64) -> (bool, u32) {
    let asks = Cell::new(0u32);
    let clock = Cell::new(0u64);
    let ok = is_reclaimed_within(
        patience_ns,
        || {
            asks.set(asks.get() + 1);
            // A retry that ignores its bound fails here instead of hanging the run.
            assert!(asks.get() < 1_000, "the retry never stopped asking");
            if asks.get() > refusals { 0 } else { -1 }
        },
        || clock.get(),
        || clock.set(clock.get() + 10_000_000),
    );
    (ok, asks.get())
}

#[test]
fn a_refusal_that_passes_is_retried_until_the_region_comes_back() {
    // The first ask arms the kill and is refused; the shell used to stop there.
    assert_eq!(run(1, 1_000_000_000), (true, 2));
    assert_eq!(run(5, 1_000_000_000), (true, 6));
}

#[test]
fn a_region_that_answers_at_once_is_asked_once() {
    assert_eq!(run(0, 1_000_000_000), (true, 1));
}

#[test]
fn a_refusal_that_never_passes_ends_at_the_patience_bound() {
    let (ok, asks) = run(u32::MAX, 1_000_000_000);
    assert!(!ok);
    // A hundred 10 ms waits, and the ask after the bound passes.
    assert_eq!(asks, 102);
}

#[test]
fn a_region_is_asked_once_even_with_no_patience() {
    assert_eq!(run(u32::MAX, 0).1, 2);
    assert_eq!(run(0, 0), (true, 1));
}

/// A budget whose `destroy` refuses `refusals` times, with `children` pages of child regions live
/// throughout. Returns the verdict, the asks of `destroy` and the waits.
fn budget_run(refusals: u32, children: i64, patience_ns: u64) -> (BudgetReturn, u32, u32) {
    let asks = Cell::new(0u32);
    let waits = Cell::new(0u32);
    let clock = Cell::new(0u64);
    let verdict = reclaim_budget_within(
        patience_ns,
        || {
            asks.set(asks.get() + 1);
            assert!(asks.get() < 1_000, "the retry never stopped asking");
            if asks.get() > refusals { 0 } else { -1 }
        },
        || children,
        || clock.get(),
        || {
            waits.set(waits.get() + 1);
            clock.set(clock.get() + 10_000_000);
        },
    );
    (verdict, asks.get(), waits.get())
}

#[test]
fn a_budget_with_a_child_stops_at_the_first_refusal() {
    // One ask, no wait: the full one-second bound would be a hundred waits.
    assert_eq!(budget_run(u32::MAX, 4, 1_000_000_000), (BudgetReturn::LeftChildren, 1, 0));
}

#[test]
fn a_budget_with_no_child_still_retries_a_resident_to_the_bound() {
    let (verdict, asks, _) = budget_run(u32::MAX, 0, 1_000_000_000);
    assert_eq!(verdict, BudgetReturn::Refused);
    assert_eq!(asks, 102);
    assert_eq!(budget_run(5, 0, 1_000_000_000), (BudgetReturn::Reclaimed, 6, 5));
}

#[test]
fn an_unreadable_children_record_retries_as_before() {
    assert_eq!(budget_run(3, -3, 1_000_000_000), (BudgetReturn::Reclaimed, 4, 3));
}

#[test]
fn a_clean_budget_is_asked_once() {
    assert_eq!(budget_run(0, 0, 1_000_000_000), (BudgetReturn::Reclaimed, 1, 0));
}

#[test]
fn the_leak_sentence_names_the_job_and_the_pages() {
    let mut text = std::string::String::new();
    write_budget_left(b"job_splitter", 4, &mut |b| text.push_str(core::str::from_utf8(b).unwrap()));
    assert_eq!(
        text,
        "  job_splitter left a child region in its --mem budget; its 4 pages stay spent until reboot.\n"
    );
}
