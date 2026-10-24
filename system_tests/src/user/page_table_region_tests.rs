//! **A destroyed region takes no live address space's page tables with it** (the
//! page-tables-outlive-destroy lane, 2026-10-05 UTC, provisional; fatal risk 7's confinement claim,
//! DECISIONS §13 (frame revocation) and §16 (object revocation)).
//!
//! `PageFrame::MAP` and `MemoryRegion::MAP` build intermediate page tables from a region the caller
//! names, and a page table is the one page whose contents are authority. Each round maps with
//! tables from `R`, destroys `R` through the real `DESTROY`, then asks whether the first page still
//! translates, whether the log lists it, and whether a second map (tables from `R2`) builds fresh
//! tables or descends into a page `R` gave back, which names the use-after-free. A kernel test, as
//! a program cannot see its page tables. See notes/frames.md, "Page tables a region paid for".

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use abi::Error;

use crate::arch::exceptions::TrapFrame;
use crate::cap::{Rights, page_frame_run_cap, page_frame_run_len};
use crate::sched;
use crate::syscall::invoke;

/// Where each round maps: 128 GiB, which is in the low half on all three formats (Sv39's ends at
/// 256 GiB) and far from anything a freshly built space holds, so the walk to it needs tables that
/// do not exist yet and the region the caller names has to pay for them.
const VA: u64 = 0x20_0000_0000;

/// The two calls that build tables out of a region the caller names.
#[derive(Clone, Copy, Debug)]
enum Path {
    /// `PageFrame::MAP(va, writable, region)`: the leaf is the frame, the tables are the region's.
    PageFrame,
    /// `MemoryRegion::MAP(va)`: the leaf and the tables are both the region's.
    MemoryRegion,
}

impl Path {
    /// How many of the pages a map spends from its region are the leaf rather than tables.
    fn leaves(self) -> u64 {
        match self {
            Path::PageFrame => 0,
            Path::MemoryRegion => 1,
        }
    }
}

/// `invoke` through the real dispatcher, the way a program's `svc`/`ecall` reaches it.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// A syscall answer as `x0` carries it: the value, or the error's negative code.
fn encode(answer: Result<i64, Error>) -> u64 {
    answer.unwrap_or_else(|e| e as i64) as u64
}

fn spent(region: u64) -> u64 {
    crate::memory_region::usage(region).map_or(0, |(spent, _)| spent)
}

/// Does `root`'s mapping log list `va`?
fn listed(root: u64, va: u64) -> bool {
    let mut cursor = 0;
    loop {
        let crate::revoke::Listing::Entry(next, at) = crate::revoke::list_mapping(root, cursor)
        else {
            return false;
        };
        if at == va {
            return true;
        }
        cursor = next;
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

/// What the mapper saw, one round at a time.
struct Report {
    first: AtomicU64,
    tables_from_r: AtomicU64,
    destroyed: AtomicU64,
    mapped_after: AtomicBool,
    listed_after: AtomicBool,
    second: AtomicU64,
    tables_from_r2: AtomicU64,
    returned: AtomicBool,
}

impl Report {
    const fn new() -> Self {
        Self {
            first: AtomicU64::new(0),
            tables_from_r: AtomicU64::new(0),
            destroyed: AtomicU64::new(0),
            mapped_after: AtomicBool::new(false),
            listed_after: AtomicBool::new(false),
            second: AtomicU64::new(0),
            tables_from_r2: AtomicU64::new(0),
            returned: AtomicBool::new(false),
        }
    }
}

/// **`MemoryRegion::DESTROY` of a region that paid for a live space's page tables leaves no walk
/// into it.**
///
/// The mapper adopts a space of its own, maps at [`VA`] with tables from `R`, and destroys `R`
/// itself through the syscall, so the destroy is a real holder's `DESTROY` and nothing else is
/// running in between. The premise assertions check that `R` really paid for tables and really
/// went, so a green result cannot mean the round never put a table in `R`.
///
/// Falsification: replayable `system_tests/falsifications/user.page_table_region_tests.a_destroyed_region_takes_no_live_spaces_page_tables_with_it.patch`
#[test_case]
fn a_destroyed_region_takes_no_live_spaces_page_tables_with_it() {
    for path in [Path::MemoryRegion, Path::PageFrame] {
        static REPORT: Report = Report::new();
        REPORT.returned.store(false, Ordering::SeqCst);

        // Two frames for the PageFrame path, one per map, from a region nobody destroys mid-round.
        let frames = crate::memory_region::create(2).expect("no frame region");
        let first_frame = crate::memory_region::retype_page(frames).expect("no frame");
        let second_frame = crate::memory_region::retype_page(frames).expect("no frame");
        let r = crate::memory_region::create(8).expect("no region R");
        let r2 = crate::memory_region::create(8).expect("no region R2");

        let mapper = sched::spawn(move || {
            sched::adopt_address_space(crate::user::AddressSpace::new(2).expect("no space"));
            let root = crate::arch::mmu::current_user_root();
            let r_slot = sched::grant(crate::cap::memory_region_cap(r)).expect("grant R");
            let r2_slot = sched::grant(crate::cap::memory_region_cap(r2)).expect("grant R2");
            let rights = Rights::ALL;
            let one = page_frame_run_len(1);
            let f1 = sched::grant(page_frame_run_cap(first_frame, one, rights)).expect("grant f1");
            let f2 = sched::grant(page_frame_run_cap(second_frame, one, rights)).expect("grant f2");

            let map = |frame: u64, region: u64, va: u64| match path {
                Path::PageFrame => call(frame, abi::page_frame::MAP, va, 1, region),
                Path::MemoryRegion => call(region, abi::memory_region::MAP, va, 0, 0),
            };

            REPORT
                .first
                .store(encode(map(f1, r_slot, VA)), Ordering::SeqCst);
            REPORT
                .tables_from_r
                .store(spent(r) - path.leaves(), Ordering::SeqCst);

            let destroyed = call(r_slot, abi::memory_region::DESTROY, 0, 0, 0);
            REPORT.destroyed.store(encode(destroyed), Ordering::SeqCst);
            REPORT.mapped_after.store(
                crate::arch::mmu::translate_at(root, VA).is_some(),
                Ordering::SeqCst,
            );
            REPORT
                .listed_after
                .store(listed(root, VA), Ordering::SeqCst);

            let before = spent(r2);
            let second = map(f2, r2_slot, VA + page_frames::FRAME_SIZE);
            REPORT.second.store(encode(second), Ordering::SeqCst);
            REPORT
                .tables_from_r2
                .store(spent(r2) - before - path.leaves(), Ordering::SeqCst);
            REPORT.returned.store(true, Ordering::SeqCst);
            for slot in [r_slot, r2_slot, f1, f2] {
                let _ = sched::delete_current_cap(slot);
            }
        })
        .expect("no mapper thread");

        assert!(
            wait_for(|| REPORT.returned.load(Ordering::SeqCst)),
            "the {path:?} mapper never finished its round",
        );
        let load = |a: &AtomicU64| a.load(Ordering::SeqCst) as i64;

        assert_eq!(load(&REPORT.first), 0, "{path:?}: the first map failed");
        assert!(
            load(&REPORT.tables_from_r) > 0,
            "{path:?}: R paid for no page table, so this round tests nothing",
        );
        assert_eq!(
            load(&REPORT.destroyed),
            0,
            "{path:?}: DESTROY of R was refused"
        );
        assert!(
            crate::memory_region::region_bounds(r).is_none(),
            "{path:?}: R still resolves after its DESTROY, so this round's premise is false",
        );
        assert!(
            !REPORT.mapped_after.load(Ordering::SeqCst),
            "{path:?}: the page at VA still translated after the region holding its page tables \
             was destroyed: the space walks pages that region gave back",
        );
        assert!(
            !REPORT.listed_after.load(Ordering::SeqCst),
            "{path:?}: the space's log still lists VA after its tables were cut",
        );
        assert_eq!(
            load(&REPORT.second),
            0,
            "{path:?}: the map beside VA failed"
        );
        assert!(
            load(&REPORT.tables_from_r2) > 0,
            "{path:?}: a map beside VA after R's DESTROY built no tables from R2, so it wrote its \
             leaf into a table R had given back (a use-after-free of a page table)",
        );

        assert!(
            wait_for(|| !sched::is_thread_present(mapper)),
            "the {path:?} mapper was never reaped",
        );
        crate::memory_region::destroy(r2);
        crate::memory_region::destroy(frames);
    }
}
