//! **A page-table allocator that fails on its Nth call**, swept over N until the mapping succeeds.
//!
//! It fails on exactly one call, counted from 0, so a mapper that swallowed the error would leave a
//! hole. After every failure the sweep asserts that no table frame leaked (every frame handed out
//! is reachable from the root), that what is left mapped is a prefix of what the successful run
//! maps (empty for `map_block`), and that the error is `OutOfPageFrames`. See notes/paging.md,
//! "The Nth-allocation sweep".
//!
//! Names in this file (the file, the helpers, the tests) are provisional.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

use paging::domain::build_identity_domain;
use paging::{
    Aarch64, DmaRegion, Flags, Half, Ia32e, MapError, Mapper, PAGE_SIZE, PageFormat, PageSize,
    PageTable, Sv39, Vtd,
};

const MIB2: u64 = 2 << 20;
const GIB1: u64 = 1 << 30;

/// The pretend frame allocator, failing on call `fail_at` (counted from 0) and on no other.
///
/// The root is taken outside the count: a mapper cannot exist without one, so it is never the
/// subject of the sweep. **The mapper borrows the pool**, so the tables outlive it and are freed on
/// drop, whatever the test did (the shape `blocks.rs` uses).
struct FailingPool {
    /// Every frame this pool owns, root included, for freeing.
    owned: RefCell<Vec<*mut PageTable>>,
    /// The frames the mapper was handed, in order. The root is not among them.
    handed: RefCell<Vec<u64>>,
    calls: Cell<usize>,
    fail_at: Option<usize>,
}

impl FailingPool {
    fn new(fail_at: Option<usize>) -> Self {
        FailingPool {
            owned: RefCell::new(Vec::new()),
            handed: RefCell::new(Vec::new()),
            calls: Cell::new(0),
            fail_at,
        }
    }

    fn frame(&self) -> u64 {
        let p = Box::into_raw(Box::new(PageTable::new()));
        self.owned.borrow_mut().push(p);
        p as u64
    }

    fn alloc(&self) -> Option<u64> {
        let n = self.calls.get();
        self.calls.set(n + 1);
        if Some(n) == self.fail_at {
            return None;
        }
        let pa = self.frame();
        self.handed.borrow_mut().push(pa);
        Some(pa)
    }

    #[allow(clippy::type_complexity)]
    fn mapper<F: PageFormat>(
        &self,
        root: u64,
    ) -> Mapper<impl FnMut() -> Option<u64> + '_, fn(u64) -> *mut PageTable, F> {
        // SAFETY: `root` is fresh, zeroed and 4 KiB-aligned; `phys_to_ptr` is the identity, right
        // because these "physical" addresses are host addresses.
        unsafe { Mapper::new(root, Half::Low, move || self.alloc(), phys_to_ptr) }
    }
}

impl Drop for FailingPool {
    fn drop(&mut self) {
        for p in self.owned.borrow_mut().drain(..) {
            // SAFETY: each `p` came from `Box::into_raw` in `frame`, registered exactly once.
            unsafe { drop(Box::from_raw(p)) };
        }
    }
}

fn phys_to_ptr(pa: u64) -> *mut PageTable {
    pa as *mut PageTable
}

/// One leaf as the tables hold it: where it starts, how much it covers, what it maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Leaf {
    va: u64,
    bytes: u64,
    pa: u64,
}

/// What the tables under `root` actually hold: every table frame reachable, every leaf, and how
/// many reachable tables hold nothing at all (no leaf beneath them).
struct Walk {
    tables: BTreeSet<u64>,
    leaves: Vec<Leaf>,
    empty_tables: usize,
}

/// **Read the tables back the way the hardware would**, independently of `Mapper`, which is the
/// thing under test. Low half only, which is all this file maps. Returns whether `table` reaches a
/// leaf, so empty tables can be counted.
fn walk_table<F: PageFormat>(table: u64, level: usize, base: u64, out: &mut Walk) -> bool {
    out.tables.insert(table);
    let span = PAGE_SIZE << (9 * (F::LEVELS - 1 - level));
    let mut any = false;
    for i in 0..512u64 {
        // SAFETY: `table` is a frame this file's pool owns, reached through a present table entry.
        let entry = unsafe { (*phys_to_ptr(table)).entries[i as usize] };
        if !F::is_present(entry) {
            continue;
        }
        let va = base + i * span;
        if level == F::LEVELS - 1 || F::is_block(entry) {
            out.leaves.push(Leaf {
                va,
                bytes: span,
                pa: F::entry_pa(entry) & !(span - 1),
            });
            any = true;
        } else {
            any |= walk_table::<F>(F::entry_pa(entry), level + 1, va, out);
        }
    }
    if !any && level > 0 {
        out.empty_tables += 1;
    }
    any
}

fn walk<F: PageFormat>(root: u64) -> Walk {
    let mut w = Walk {
        tables: BTreeSet::new(),
        leaves: Vec::new(),
        empty_tables: 0,
    };
    walk_table::<F>(root, 0, 0, &mut w);
    w
}

/// The three assertions, after one run of the sweep. `full` is the leaf list a successful run
/// leaves, in the order the request maps them, which for every request here is ascending VA.
///
/// Returns `true` when the run succeeded, which ends the sweep.
fn check<F: PageFormat>(
    what: &str,
    n: usize,
    root: u64,
    pool: &FailingPool,
    result: Result<(), MapError>,
    full: Option<&[Leaf]>,
) -> (bool, Walk) {
    let w = walk::<F>(root);
    let handed: BTreeSet<u64> = pool.handed.borrow().iter().copied().collect();
    let mut expected = handed.clone();
    expected.insert(root);

    // (1) No leak, and nothing foreign: the reachable tables are exactly the root and the frames
    // handed out.
    let leaked: Vec<_> = expected.difference(&w.tables).collect();
    assert!(
        leaked.is_empty(),
        "{what}, allocation {n} failed: {} table frame(s) handed out and unreachable from the \
         root, so nothing can free them: {leaked:#x?}",
        leaked.len(),
    );
    assert_eq!(
        w.tables, expected,
        "{what}, allocation {n}: a reachable table nobody allocated"
    );

    match (result, full) {
        (Ok(()), _) => (true, w),
        (Err(e), Some(full)) => {
            // (3)
            assert_eq!(
                e,
                MapError::OutOfPageFrames,
                "{what}, allocation {n}: wrong error"
            );
            // (2) A prefix of the successful run, and a strict one: the failing request's own
            // leaf was not written.
            assert!(
                w.leaves.len() < full.len() && w.leaves[..] == full[..w.leaves.len()],
                "{what}, allocation {n} failed: what is left mapped is not a prefix of the \
                 request, so the caller cannot unwind it by count.\n left: {:#x?}\n full: {:#x?}",
                w.leaves,
                full,
            );
            (false, w)
        }
        (Err(_), None) => unreachable!("the successful run is taken first"),
    }
}

/// The sweep itself. `run` builds a fresh mapping on a fresh root with the pool given, and is
/// called once with no failure (the reference), then with a failure at 0, 1, 2, ... until it
/// succeeds. Returns how many runs failed and how many empty tables the failures left in total.
fn sweep<F: PageFormat>(
    what: &str,
    run: impl Fn(u64, &FailingPool) -> Result<(), MapError>,
) -> (usize, usize) {
    let full = {
        let pool = FailingPool::new(None);
        let root = pool.frame();
        run(root, &pool).unwrap_or_else(|e| panic!("{what}: the reference run failed: {e:?}"));
        let (_, w) = check::<F>(what, usize::MAX, root, &pool, Ok(()), None);
        w.leaves
    };
    assert!(!full.is_empty(), "{what}: the reference run mapped nothing");

    let mut empty = 0;
    for n in 0.. {
        assert!(
            n < 64,
            "{what}: still failing at allocation {n}; the sweep never succeeded"
        );
        let pool = FailingPool::new(Some(n));
        let root = pool.frame();
        let result = run(root, &pool);
        let (done, w) = check::<F>(what, n, root, &pool, result, Some(&full));
        if done {
            assert_eq!(
                w.leaves, full,
                "{what}: success after a skipped failure maps differently"
            );
            return (n, empty);
        }
        empty += w.empty_tables;
    }
    unreachable!()
}

/// **`map_block`, one leaf of each size, fails clean at every depth.** On a fresh root every level
/// above the leaf needs a table, so the sweep fails each of those allocations in turn: exactly one
/// failing run per level above the leaf, none for an Sv39 gigapage, which sits in the root.
fn a_failed_map_block_leaves_nothing<F: PageFormat>(sizes: &[PageSize]) {
    for &size in sizes {
        let va = 4 * GIB1 - size.bytes();
        let (failures, _) = sweep::<F>(&format!("map_block({size:?})"), |root, pool| {
            pool.mapper::<F>(root)
                .map_block(va, 7 * GIB1, size, Flags::user_data())
        });
        let above = F::LEVELS - 1 - size.levels_above_bottom();
        assert_eq!(
            failures, above,
            "map_block({size:?}) took a table per level above the leaf"
        );
    }
}

/// Falsification: replayable `crates/paging/falsifications/nth_allocation.a_failed_map_block_leaks_no_table.patch`
/// (unlinks the tables a failed `map_block` installed, orphaning them).
#[test]
fn a_failed_map_block_leaks_no_table() {
    a_failed_map_block_leaves_nothing::<Aarch64>(&PageSize::ALL);
    a_failed_map_block_leaves_nothing::<Sv39>(&PageSize::ALL);
    a_failed_map_block_leaves_nothing::<Ia32e>(&PageSize::ALL);
}

/// **`map_range` straddling a 1 GiB boundary**, so the second page needs two new tables after the
/// first page is mapped: the case where something is already taken when the allocator fails.
fn a_failed_map_range_leaves_a_prefix<F: PageFormat>() {
    let va = GIB1 - 2 * PAGE_SIZE;
    let (failures, _) = sweep::<F>("map_range", |root, pool| {
        pool.mapper::<F>(root)
            .map_range(va, 9 * GIB1, 4, Flags::user_data())
    });
    assert!(
        failures > F::LEVELS - 1,
        "map_range swept only the first page's tables"
    );
}

/// Falsification: replayable `crates/paging/falsifications/nth_allocation.a_failed_map_range_leaks_no_table.patch`
/// (unlinks the tables a failed `map` installed, orphaning them).
#[test]
fn a_failed_map_range_leaks_no_table() {
    a_failed_map_range_leaves_a_prefix::<Aarch64>();
    a_failed_map_range_leaves_a_prefix::<Sv39>();
    a_failed_map_range_leaves_a_prefix::<Ia32e>();
}

/// **`map_span` with every leaf size in play**: a 4 KiB head, a 2 MiB block, a 1 GiB block, a 2 MiB
/// block and a 4 KiB tail, in that order, so a failure can land after any number of them.
fn a_failed_map_span_leaves_a_prefix<F: PageFormat>() {
    let va = 2 * GIB1 - MIB2 - PAGE_SIZE;
    let len = PAGE_SIZE + MIB2 + GIB1 + MIB2 + PAGE_SIZE;
    let (failures, _) = sweep::<F>("map_span", |root, pool| {
        pool.mapper::<F>(root).map_span(
            va,
            va + 4 * GIB1,
            len,
            Flags::user_data(),
            PageSize::Size1GiB,
        )
    });
    assert!(
        failures > F::LEVELS - 1,
        "map_span swept only the head's tables"
    );
}

/// Falsification: replayable `crates/paging/falsifications/nth_allocation.a_failed_map_span_maps_a_prefix.patch`
/// (skips a leaf whose tables could not be allocated and maps the rest).
#[test]
fn a_failed_map_span_maps_a_prefix() {
    a_failed_map_span_leaves_a_prefix::<Aarch64>();
    a_failed_map_span_leaves_a_prefix::<Sv39>();
    a_failed_map_span_leaves_a_prefix::<Ia32e>();
}

/// **`build_identity_domain` over two regions** far enough apart that the second needs its own
/// tables, several pages each, so a failure can land after the first region is wholly mapped.
fn a_failed_domain_build_leaves_a_prefix<F: PageFormat>() {
    let regions = [
        DmaRegion {
            base: GIB1 - 2 * PAGE_SIZE,
            size: 3 * PAGE_SIZE,
            writable: true,
        },
        DmaRegion {
            base: 3 * GIB1,
            size: 2 * PAGE_SIZE,
            writable: true,
        },
    ];
    let (failures, _) = sweep::<F>("build_identity_domain", |root, pool| {
        // SAFETY: `root` and every frame `alloc` returns are fresh, zeroed and page-aligned, and
        // the identity is the right `phys_to_ptr` for host frames.
        unsafe { build_identity_domain::<_, _, F>(root, || pool.alloc(), phys_to_ptr, &regions) }
    });
    assert!(
        failures > F::LEVELS - 1,
        "the domain build swept only the first page's tables"
    );
}

/// Falsification: replayable `crates/paging/falsifications/nth_allocation.a_failed_domain_build_maps_a_prefix.patch`
/// (skips a page whose tables could not be allocated and maps the rest of the grant).
#[test]
fn a_failed_domain_build_maps_a_prefix() {
    a_failed_domain_build_leaves_a_prefix::<Aarch64>();
    a_failed_domain_build_leaves_a_prefix::<Sv39>();
    a_failed_domain_build_leaves_a_prefix::<Vtd>();
}
