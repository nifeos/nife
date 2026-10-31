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
