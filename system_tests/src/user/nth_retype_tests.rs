//! **`login` survives a failed retype at every point of a login** (milestone 757 (a test kernel
//! fails a process on its Nth retype), provisional; raised by milestone 745 (count the error paths
//! no test reaches) and approved by calef on 2026-10-04 UTC).
//!
//! `crate::retype_fault` makes the kernel refuse one thread's Nth retype as though the region were
//! empty. This sweeps N over one whole login against `login` and after each run compares its
//! capability table and region usage with their values before the sweep: a forgotten `cap_delete`
//! or split child crashes nothing, and those two counters keep the evidence. It shares
//! `login_tests`' one `login` and leaves it as it found it; one portable body (§19 (architectural
//! parity is a tenet)). See notes/untested-error-paths.md, "The retype-fault sweep of `login`".

//! # BUGS
//!
//! - **One program, one exchange.** `login`'s start-up, a login that opens a schedule, and the
//!   other services' cleanup paths are not swept: each needs a fresh `login` per N, a teardown
//!   `holding.rs` cannot do. All of it is
//!   `design/roadmap/0786-sweep-the-other-long-lived-services-under-the-retype-fault.md`.
//! - **x86_64 skips the `login` sweep** (its test boot attaches no RedoxFS disk);
//!   [`the_nth_retype_fails_as_an_empty_region_would_and_no_other`] runs there too.
//! - **It sees leaks, not wrong answers past the first**: beyond `DENIED`, a working next exchange
//!   and the two counters, nothing is checked.

use credential_contract::fixture::BOB;
use login_service as ls;

use super::login_tests::{free_terminal, wired};
use super::*;
use crate::cap::Object;
use crate::{retype_fault, sched};

/// A sweep that has not reached the end of a login by this N has found a program that retypes
/// without bound, which is a finding of its own. A clean login was measured at well under this (the
/// roadmap block has the count).
const MAX_N: u64 = 256;

/// What a failed retype must leave unchanged: the occupied slots of `login`'s table, and for each
/// region it holds, how many pages that region has spent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Census {
    occupied: usize,
    /// `(slot, region, pages spent)` for each `MemoryRegion` capability, by slot, in the first
    /// `held` entries. Bounded small and not per-slot: a `[Option<_>; CAPABILITY_TABLE_SLOTS]` here
    /// is a 1.5 KiB value at 64 slots, copied through every frame (milestone 754 (the capability
    /// table grows to 64 slots)).
    regions: [(usize, u64, u64); MAX_REGIONS_HELD],
    held: usize,
}

/// More memory regions than `login` has ever held at once, with room; `census` panics past it.
const MAX_REGIONS_HELD: usize = 8;

fn census(tid: crate::thread::ThreadId) -> Census {
    let mut regions = [(0, 0, 0); MAX_REGIONS_HELD];
    let mut held = 0;
    for slot in 0..crate::cap::CAPABILITY_TABLE_SLOTS {
        // One slot per lock acquisition: `memory_region::usage` takes a lock of higher rank than the
        // capability table's, so it must run after the table is released (the lock-order check
        // panicked on exactly this at the first push).
        let cap = slot_of(tid, slot);
        if let Some(cap) = cap
            && let Object::MemoryRegion(region) = cap.object
        {
            let spent = crate::memory_region::usage(region).map_or(u64::MAX, |(spent, _)| spent);
            assert!(
                held < MAX_REGIONS_HELD,
                "login holds more than {MAX_REGIONS_HELD} regions"
            );
            regions[held] = (slot, region, spent);
            held += 1;
        }
    }
    Census {
        occupied: sched::with_capability_table(tid, |table| table.used())
            .expect("login's thread is gone"),
        regions,
        held,
    }
}

/// What `tid` holds in `slot`, read under the capability table's lock and nothing else.
fn slot_of(tid: crate::thread::ThreadId, slot: usize) -> Option<crate::cap::Cap> {
    sched::with_capability_table(tid, |table| table.get(slot as u64).ok())
        .expect("login's thread is gone")
}

/// One `LOGOUT` login as `bob`, then the front-door round trip that proves `login` is back at the
/// top of its loop, so whatever it does after answering the client has finished before anything is
/// counted. Returns the client's verdict.
///
/// `bob`, not `alice`: `login_tests`' schedule tests give `alice` a durable session on this
/// same `login`, and a `LOGOUT` login that reattached one would destroy that session's budget.
/// Nothing on this instance gives `bob` one.
fn one_login(cli: &'static [u8], w: &ls::Wiring) -> u64 {
    let r = ls::client(cli, w, ls::LOGOUT, BOB, BOB);
    if r[0] == ls::RPT_OK {
        // `login` sends the audit record after the capabilities; nothing it does after `OK`
        // retypes, so a run that reached `OK` always gets this far.
        let a = sched::ipc_receive(w.audit);
        assert_eq!(
            a[0],
            login_contract::ATTRIBUTED,
            "no attribution record followed a login"
        );
    }
    free_terminal(w);
    r[0]
}

/// The sweep. The headline of milestone 757.
///
/// Falsification: replayable `system_tests/falsifications/user.nth_retype_tests.login_gives_back_everything_when_any_retype_of_a_login_fails.patch`
#[test_case]
fn login_gives_back_everything_when_any_retype_of_a_login_fails() {
    if fs_service::fs_server_image().is_none() {
        crate::testing::skip!(fs_service::NO_FS_SERVER);
    }
    let Some(w) = wired() else {
        crate::testing::skip!("no virtio-rng device or no RedoxFS disk attached");
    };
    free_terminal(&w);
    let cli =
        program("login_test_client").expect("no login_test_client program in the initial ramdisk");

    // A clean login first, counted and not failed, so the census below is taken after `login` has
    // built whatever it builds once (a page table for the channel window, say) and every run after
    // it is compared against a steady state.
    retype_fault::arm(w.tid, u64::MAX);
    assert_eq!(
        one_login(cli, &w),
        ls::RPT_OK,
        "the uninjected login failed"
    );
    let clean = retype_fault::disarm();
    assert!(
        clean.seen > 0,
        "a whole login made no retype in login's thread"
    );
    let before = census(w.tid);

    let mut refused = 0u64;
    let mut n = 1;
    let steady = loop {
        assert!(
            n <= MAX_N,
            "login was still retyping at N = {MAX_N}; a clean login made {}",
            clean.seen
        );
        retype_fault::arm(w.tid, n);
        let verdict = one_login(cli, &w);
        let tally = retype_fault::disarm();
        let after = census(w.tid);
        if after != before {
            crate::println!("    login's table after failing retype {n}:");
            for slot in 0..crate::cap::CAPABILITY_TABLE_SLOTS {
                if let Some(cap) = slot_of(w.tid, slot) {
                    crate::println!("      slot {slot}: {:?}", cap.object);
                }
            }
            panic!(
                "failing login's retype {n} of {} (verdict {verdict:#x}) changed what login holds: \
                 a cleanup path kept a capability slot or a region's pages.\n  before: \
                 {before:?}\n  after:  {after:?}",
                tally.seen,
            );
        }
        if !tally.refused {
            // The run made fewer than N retypes: it was a whole login, and it must have worked.
            assert_eq!(
                verdict,
                ls::RPT_OK,
                "an uninjected login failed after the sweep"
            );
            break tally.seen;
        }
        refused += 1;
        assert!(
            verdict == ls::RPT_OK || verdict == ls::RPT_DENIED,
            "failing login's retype {n} was answered {verdict:#x}, neither OK nor DENIED",
        );
        n += 1;
    };
    // The first login builds what later ones reuse (a page table for its scratch window, say), so
    // it counts more retypes than the steady state the sweep runs in.
    crate::println!(
        "[nth retype] login: {} retypes in the first login, {steady} in a later one; failed each of \
         {refused} in turn and login kept nothing",
        clean.seen
    );
}

/// One of each method that takes pages out of a region, in order, and how many pages each takes:
/// `RETYPE` one page, `RETYPE_OBJ` one page for a rendezvous, `SPLIT` a two-page child.
/// `(method, argument, pages)`; `RETYPE`'s argument `0` is one page, the way every caller asks.
const SCRIPT: [(u64, u64, u64); 3] = {
    use abi::memory_region::{RETYPE, RETYPE_OBJ, SPLIT};
    [
        (RETYPE, 0, 1),
        (RETYPE_OBJ, abi::objtype::RENDEZVOUS, 1),
        (SPLIT, 2, 2),
    ]
};

/// Run [`SCRIPT`] from a fresh region through the real handler, and give everything back. Returns
/// each call's answer and the pages the region had spent at the end.
fn run_script() -> ([Result<i64, abi::Error>; 3], u64) {
    let mut trap = crate::arch::exceptions::TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    let region = crate::memory_region::create(8).expect("a region to retype from");
    let slot = sched::grant(crate::cap::memory_region_root_cap(region)).expect("grant it");
    let answers =
        SCRIPT.map(|(method, a0, _)| crate::syscall::invoke(&mut trap, slot, method, a0, 0, 0));
    let spent = crate::memory_region::usage(region)
        .expect("the region is live")
        .0;
    for &answer in answers.iter().flatten() {
        // The split child goes first: a parent with a live child is not reclaimable.
        if let Ok(cap) = sched::current_cap(answer as u64)
            && let Object::MemoryRegion(child) = cap.object
        {
            crate::memory_region::destroy(child);
        }
        let _ = sched::delete_current_cap(answer as u64);
    }
    let _ = sched::delete_current_cap(slot);
    sched::reclaim_region(region).expect("the script's region did not reclaim");
    (answers, spent)
}

/// **The mechanism, on every architecture, with no service in the way.** For each N, the Nth of
/// three retypes this thread makes is answered `OutOfMemory` and moves the region not at all, and
/// the other two succeed. Armed on some other thread, nothing fails and nothing is counted. This is
/// the half of milestone 757 that runs where `login`'s sweep skips (x86_64 attaches no RedoxFS disk
/// to the test boot), and it is what says the hook sits in all three spending functions.
///
/// Falsification: replayable `system_tests/falsifications/user.nth_retype_tests.the_nth_retype_fails_as_an_empty_region_would_and_no_other.patch`
#[test_case]
fn the_nth_retype_fails_as_an_empty_region_would_and_no_other() {
    let me = sched::current();
    let full: u64 = SCRIPT.iter().map(|&(_, _, pages)| pages).sum();
    for n in 1..=SCRIPT.len() as u64 {
        retype_fault::arm(me, n);
        let (answers, spent) = run_script();
        let tally = retype_fault::disarm();
        assert_eq!(
            tally,
            retype_fault::Tally {
                seen: SCRIPT.len() as u64,
                refused: true,
            },
            "arming retype {n} of {}",
            SCRIPT.len()
        );
        for (k, answer) in answers.iter().enumerate() {
            if k as u64 + 1 == n {
                assert_eq!(
                    *answer,
                    Err(abi::Error::OutOfMemory),
                    "retype {n} was armed and was not refused"
                );
            } else {
                assert!(
                    answer.is_ok(),
                    "retype {} failed with {answer:?} when only retype {n} was armed",
                    k + 1
                );
            }
        }
        let refused_pages = SCRIPT[n as usize - 1].2;
        assert_eq!(
            spent,
            full - refused_pages,
            "failing retype {n} moved the region's watermark",
        );
    }

    // Some other thread armed: this one is neither failed nor counted. `NO_TID - 1` is not a live
    // thread; what matters is that it is not this one.
    retype_fault::arm(crate::cpu::NO_TID - 1, 1);
    let (answers, spent) = run_script();
    let tally = retype_fault::disarm();
    assert!(
        answers.iter().all(Result::is_ok),
        "a fault armed on another thread failed this one: {answers:?}"
    );
    assert_eq!(spent, full);
    assert_eq!(
        tally,
        retype_fault::Tally {
            seen: 0,
            refused: false,
        },
        "a fault armed on another thread counted this one's retypes",
    );
}
