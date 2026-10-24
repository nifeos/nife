//! A revocation sweep that lands inside a `MAP` leaves no mapping behind (the map-revocation-window
//! lane, 2026-10-04 UTC, provisional; fatal risk 7's confinement claim and DECISIONS §13
//! (capability revocation and untyped reclamation), whose "one honest race" this is).
//!
//! `delegation_pause` holds a mapper inside `PageFrame::MAP`, `AddressSpace::MAP_INTO` or
//! `MemoryRegion::MAP`, after it reads its source and before it records the mapping; the test runs
//! a whole sweep, lets it go, and asserts no mapping survived. The first two run under both
//! `REVOKE` and `DESTROY`, whose predicates and scans differ. `revoke::MappingHold` is the fix. See
//! notes/capability-lifecycle.md, "A sweep inside a `MAP`".

//! # BUGS
//!
//! - The `DeviceFrame` arm of `MAP_INTO` is reasoned, not driven: it reads its capability through
//!   the same `MappingHold`, so `DeviceFrame::REVOKE` is covered by construction, but no test here
//!   drives it.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::cap::{Object, Rights, page_frame_run_cap, page_frame_run_len};
use crate::syscall::invoke;
use crate::{delegation_pause, sched};

/// Where every test here maps its frame: an aligned low-half page well away from anything a kernel
/// thread's freshly built space already holds.
const VA: u64 = 0x40_0000;

/// The two sweeps a frame can be taken back by.
#[derive(Clone, Copy, Debug)]
enum Sweep {
    /// `PageFrame::REVOKE`: delete the run's capability family, unmap what it mapped.
    Revoke,
    /// `MemoryRegion::DESTROY`: delete every capability overlapping the region, unmap everything
    /// in it, and hand the pages back to the allocator.
    Destroy,
}

impl Sweep {
    fn run(self, frames: u64, phys: u64) {
        match self {
            Sweep::Revoke => crate::revoke::revoke_page_frame(phys),
            Sweep::Destroy => crate::memory_region::destroy(frames),
        }
    }

    /// The frame's region after the round, if this sweep did not already destroy it.
    fn clean_up(self, frames: u64) {
        if let Sweep::Revoke = self {
            crate::memory_region::destroy(frames);
        }
    }
}

fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if cond() {
            return true;
        }
        sched::yield_now();
    }
    cond()
}

/// `invoke` through the real dispatcher, the way a program's `svc`/`ecall` reaches it.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// Does any slot of `tid`'s table name a `PageFrame` containing `phys`?
fn holds_frame(tid: crate::thread::ThreadId, phys: u64) -> bool {
    // Lent rather than copied out, as `revocation_window_tests::holds_frame_in` does: a 64-slot copy
    // in this frame is what put the nth-retype tests over the guard page (milestone 754 (the
    // capability table grows to 64 slots)).
    sched::with_capability_table(tid, |table| {
        (0..crate::cap::CAPABILITY_TABLE_SLOTS as u64).any(|slot| {
            table.get(slot).is_ok_and(|c| match c.object {
                Object::PageFrame(p, n) => {
                    p <= phys && phys < p + n.get() * page_frames::FRAME_SIZE
                }
                _ => false,
            })
        })
    })
    .unwrap_or(false)
}

/// A syscall answer as `x0` carries it: the value, or the error's negative code.
fn encode(answer: Result<i64, Error>) -> u64 {
    answer.unwrap_or_else(|e| e as i64) as u64
}

/// What a mapper thread reports: who it is, what its held `MAP` answered, whether the mapping was
/// there when that returned, what the same `MAP` answered when made again afterwards, and that it
/// has returned. One per test, reset per round.
struct Report {
    tid: AtomicU64,
    answer: AtomicU64,
    mapped: AtomicBool,
    after: AtomicU64,
    returned: AtomicBool,
}

impl Report {
    const fn new() -> Self {
        Self {
            tid: AtomicU64::new(0),
            answer: AtomicU64::new(0),
            mapped: AtomicBool::new(false),
            after: AtomicU64::new(0),
            returned: AtomicBool::new(false),
        }
    }

    fn reset(&self) {
        self.mapped.store(false, Ordering::SeqCst);
        self.returned.store(false, Ordering::SeqCst);
    }

    fn answer(&self) -> i64 {
        self.answer.load(Ordering::SeqCst) as i64
    }

    fn after(&self) -> i64 {
        self.after.load(Ordering::SeqCst) as i64
    }
}

/// The shared middle of a round: wait for the mapper at the seam, sweep, record the premise (the
/// sweep reached the mapper's own table), let it go, and wait for it to report.
fn round(sweep: Sweep, frames: u64, phys: u64, report: &Report) -> bool {
    assert!(
        wait_for(delegation_pause::is_parked),
        "the mapper never reached the seam, so no {sweep:?} landed inside its MAP",
    );
    sweep.run(frames, phys);
    let premise = !holds_frame(report.tid.load(Ordering::SeqCst), phys);
    delegation_pause::release();
    assert!(
        wait_for(|| report.returned.load(Ordering::SeqCst)),
        "the mapper never returned after the seam released it",
    );
    premise
}

/// A sweep that lands inside a `PageFrame::MAP` must not leave the mapping in place.
///
/// The mapper adopts its own address space, holds a frame with every right and a region for page
/// tables, and maps the frame read/write. Held after the read, before the map, while the sweep
/// runs; then it asks its own tables whether `VA` translates. The premise asserts the sweep reached
/// the mapper's table, so green cannot mean the sweep missed it.
///
/// Falsification: replayable `system_tests/falsifications/user.map_revocation_window_tests.a_sweep_inside_a_page_frame_map_leaves_no_mapping.patch`
#[test_case]
fn a_sweep_inside_a_page_frame_map_leaves_no_mapping() {
    for sweep in [Sweep::Revoke, Sweep::Destroy] {
        static REPORT: Report = Report::new();
        REPORT.reset();

        let frames = crate::memory_region::create(2).expect("no frame region");
        let phys = crate::memory_region::retype_page(frames).expect("no page");
        let tables = crate::memory_region::create(8).expect("no page-table region");

        delegation_pause::disarm();
        let mapper = sched::spawn(move || {
            sched::adopt_address_space(crate::user::AddressSpace::new(2).expect("no space"));
            let frame_slot =
                sched::grant(page_frame_run_cap(phys, page_frame_run_len(1), Rights::ALL))
                    .expect("grant the frame");
            let tables_slot =
                sched::grant(crate::cap::memory_region_cap(tables)).expect("grant the tables");
            REPORT.tid.store(sched::current(), Ordering::SeqCst);
            delegation_pause::arm(sched::current());
            let answer = call(frame_slot, abi::page_frame::MAP, VA, 1, tables_slot);
            REPORT.answer.store(encode(answer), Ordering::SeqCst);
            let root = crate::arch::mmu::current_user_root();
            REPORT.mapped.store(
                crate::arch::mmu::translate_at(root, VA).is_some(),
                Ordering::SeqCst,
            );
            let after = call(frame_slot, abi::page_frame::MAP, VA, 1, tables_slot);
            REPORT.after.store(encode(after), Ordering::SeqCst);
            REPORT.returned.store(true, Ordering::SeqCst);
            let _ = sched::delete_current_cap(frame_slot);
            let _ = sched::delete_current_cap(tables_slot);
        })
        .expect("no mapper thread");

        let premise = round(sweep, frames, phys, &REPORT);

        assert!(
            premise,
            "the {sweep:?} did not reach the mapper's own table, so this test's premise is false",
        );
        assert!(
            !REPORT.mapped.load(Ordering::SeqCst),
            "a PageFrame::MAP held while a {sweep:?} ran kept its mapping: it read the frame before \
             the sweep and recorded the mapping after the sweep's unmap pass had scanned (answer {})",
            REPORT.answer(),
        );
        assert_eq!(
            REPORT.answer(),
            REPORT.after(),
            "a PageFrame::MAP that lost its frame to a {sweep:?} did not answer as if it had \
             started after it",
        );

        // The mapper's page tables are in `tables`, so the region goes back only once the mapper's
        // space has been torn down with it.
        assert!(
            wait_for(|| !sched::is_thread_present(mapper)),
            "the mapper was never reaped"
        );
        crate::memory_region::destroy(tables);
        sweep.clean_up(frames);
    }
}

/// A sweep that lands inside an `AddressSpace::MAP_INTO` must not leave the mapping in place.
///
/// The loader-side twin of the test above: a builder maps a frame it holds into a space under
/// construction, the way `supervision_protocol::build_child_space` does. It is held after the
/// frame is read; the sweep runs whole; then the space's own tables are read at `VA`.
///
/// Falsification: replayable `system_tests/falsifications/user.map_revocation_window_tests.a_sweep_inside_a_map_into_leaves_no_mapping.patch`
#[test_case]
fn a_sweep_inside_a_map_into_leaves_no_mapping() {
    for sweep in [Sweep::Revoke, Sweep::Destroy] {
        static REPORT: Report = Report::new();
        REPORT.reset();

        let frames = crate::memory_region::create(2).expect("no frame region");
        let phys = crate::memory_region::retype_page(frames).expect("no page");
        let space_region = crate::memory_region::create(32).expect("no space region");
        let space = crate::user::user_address_space_create(space_region).expect("no address space");

        delegation_pause::disarm();
        sched::spawn(move || {
            let space_slot = sched::grant(crate::cap::address_space_cap(space, Rights::WRITE))
                .expect("grant the space");
            let frame_slot =
                sched::grant(page_frame_run_cap(phys, page_frame_run_len(1), Rights::ALL))
                    .expect("grant the frame");
            REPORT.tid.store(sched::current(), Ordering::SeqCst);
            delegation_pause::arm(sched::current());
            let answer = call(
                space_slot,
                abi::address_space::MAP_INTO,
                VA,
                frame_slot,
                abi::address_space::MAP_RW,
            );
            REPORT.answer.store(encode(answer), Ordering::SeqCst);
            REPORT.mapped.store(
                crate::user::user_address_space_root(space)
                    .is_some_and(|root| crate::arch::mmu::translate_at(root, VA).is_some()),
                Ordering::SeqCst,
            );
            let after = call(
                space_slot,
                abi::address_space::MAP_INTO,
                VA,
                frame_slot,
                abi::address_space::MAP_RW,
            );
            REPORT.after.store(encode(after), Ordering::SeqCst);
            REPORT.returned.store(true, Ordering::SeqCst);
            let _ = sched::delete_current_cap(frame_slot);
            let _ = sched::delete_current_cap(space_slot);
        })
        .expect("no builder thread");

        let premise = round(sweep, frames, phys, &REPORT);

        assert!(
            premise,
            "the {sweep:?} did not reach the builder's own table, so this test's premise is false",
        );
        assert!(
            !REPORT.mapped.load(Ordering::SeqCst),
            "an AddressSpace::MAP_INTO held while a {sweep:?} ran kept its mapping: it read the frame \
             before the sweep and recorded the mapping after the sweep's unmap pass had scanned \
             (answer {})",
            REPORT.answer(),
        );
        assert_eq!(
            REPORT.answer(),
            REPORT.after(),
            "an AddressSpace::MAP_INTO that lost its frame to a {sweep:?} did not answer as if it \
             had started after it",
        );

        let deadline = crate::arch::timer::now() + crate::arch::timer::frequency();
        while sched::reclaim_region(space_region).is_err() && crate::arch::timer::now() < deadline {
            sched::yield_now();
        }
        sweep.clean_up(frames);
    }
}

/// A `MemoryRegion::DESTROY` inside a `MemoryRegion::MAP` of that region leaves no page mapped.
///
/// No capability names the retyped page, so the race is with the region itself. `VA`'s page tables
/// are built first from a second region, or destroying this one would also cut the mapper's tables
/// (`page_table_region_tests` drives that). Premise: the region's name no longer resolves.
///
/// Falsification: replayable `system_tests/falsifications/user.map_revocation_window_tests.a_destroy_inside_a_memory_region_map_leaves_no_mapping.patch`
#[test_case]
fn a_destroy_inside_a_memory_region_map_leaves_no_mapping() {
    static REPORT: Report = Report::new();
    static NEIGHBOUR: AtomicU64 = AtomicU64::new(0);

    let region = crate::memory_region::create(4).expect("no region");
    let tables = crate::memory_region::create(8).expect("no page-table region");

    delegation_pause::disarm();
    let mapper = sched::spawn(move || {
        sched::adopt_address_space(crate::user::AddressSpace::new(2).expect("no space"));
        let tables_slot =
            sched::grant(crate::cap::memory_region_cap(tables)).expect("grant the tables");
        let region_slot =
            sched::grant(crate::cap::memory_region_cap(region)).expect("grant the region");
        NEIGHBOUR.store(
            encode(call(
                tables_slot,
                abi::memory_region::MAP,
                VA + page_frames::FRAME_SIZE,
                0,
                0,
            )),
            Ordering::SeqCst,
        );
        delegation_pause::arm(sched::current());
        let answer = call(region_slot, abi::memory_region::MAP, VA, 0, 0);
        REPORT.answer.store(encode(answer), Ordering::SeqCst);
        let root = crate::arch::mmu::current_user_root();
        REPORT.mapped.store(
            crate::arch::mmu::translate_at(root, VA).is_some(),
            Ordering::SeqCst,
        );
        let after = call(region_slot, abi::memory_region::MAP, VA, 0, 0);
        REPORT.after.store(encode(after), Ordering::SeqCst);
        REPORT.returned.store(true, Ordering::SeqCst);
        let _ = sched::delete_current_cap(region_slot);
        let _ = sched::delete_current_cap(tables_slot);
    })
    .expect("no mapper thread");

    assert!(
        wait_for(delegation_pause::is_parked),
        "the mapper never reached the seam, so no DESTROY landed inside its MAP",
    );
    assert_eq!(
        NEIGHBOUR.load(Ordering::SeqCst),
        0,
        "the page beside VA did not map, so the region under test would pay for page tables too",
    );
    crate::memory_region::destroy(region);
    let premise = crate::memory_region::region_bounds(region).is_none();
    delegation_pause::release();
    assert!(
        wait_for(|| REPORT.returned.load(Ordering::SeqCst)),
        "the mapper never returned after the seam released it",
    );

    assert!(
        premise,
        "the region still resolved after DESTROY, so this test's premise is false"
    );
    assert!(
        !REPORT.mapped.load(Ordering::SeqCst),
        "a MemoryRegion::MAP held while its region was destroyed kept its mapping: it retyped \
         before the claim and recorded after the unmap pass had scanned (answer {})",
        REPORT.answer(),
    );
    assert_eq!(
        REPORT.answer(),
        REPORT.after(),
        "a MemoryRegion::MAP that lost its region to DESTROY did not answer as if it had started \
         after it",
    );

    assert!(
        wait_for(|| !sched::is_thread_present(mapper)),
        "the mapper was never reaped"
    );
    crate::memory_region::destroy(tables);
}
