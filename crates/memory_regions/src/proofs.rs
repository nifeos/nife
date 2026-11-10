use super::*;

/// A successful carve stays within budget, does not overflow, and strictly progresses (so the
/// run it hands out, `[parent_watermark, new)`, is fresh and disjoint from every earlier carve).
/// A refused carve is exactly: empty, overflowing, or beyond budget.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.split_stays_within_budget_and_progresses.patch`
#[kani::proof]
fn split_stays_within_budget_and_progresses() {
    let parent_pages: u64 = kani::any();
    let parent_watermark: u64 = kani::any();
    let want: u64 = kani::any();
    // The table's own invariant: a region's watermark never exceeds its page count.
    kani::assume(parent_watermark <= parent_pages);

    match split_new_watermark(parent_pages, parent_watermark, want) {
        Some(new) => {
            assert!(want > 0, "a zero carve must be refused");
            assert!(
                new == parent_watermark + want,
                "watermark advances by exactly want"
            );
            assert!(
                new <= parent_pages,
                "the carve stays within the parent's budget"
            );
            assert!(
                new > parent_watermark,
                "the carve strictly progresses, so it is fresh"
            );
        }
        None => {
            // Refused: it must be one of the three legitimate reasons.
            assert!(
                want == 0
                    || parent_watermark.checked_add(want).is_none()
                    || parent_watermark + want > parent_pages
            );
        }
    }
}

/// **A retyped run is exactly the pages asked for, all fresh, all within budget, and `0` is
/// one.** The `RETYPE` count's whole contract, over every watermark and every requested word.
/// A refusal is exactly "does not fit": the table does not move, which its own host test shows.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_retyped_run_is_the_pages_asked_for_and_zero_is_one.patch`
#[kani::proof]
fn a_retyped_run_is_the_pages_asked_for_and_zero_is_one() {
    let pages: u64 = kani::any();
    let watermark: u64 = kani::any();
    let requested: u64 = kani::any();
    kani::assume(watermark <= pages);
    let want = if requested == 0 { 1 } else { requested };

    match retype_new_watermark(pages, watermark, requested) {
        Some(new) => {
            assert!(
                new == watermark + want,
                "the watermark advances by exactly the run"
            );
            assert!(new <= pages, "the run stays within the region's budget");
            assert!(
                new > watermark,
                "the run is fresh: no page handed out twice"
            );
        }
        None => {
            assert!(
                watermark.checked_add(want).is_none() || watermark + want > pages,
                "a run that fits must not be refused"
            );
        }
    }
}

/// **The no-double-free crux:** a region with a live object or a live child refuses; a root frees
/// to the allocator; and a child NEVER frees to the allocator (its pages return to the parent).
/// So a page reaches the allocator only through the root that owns it, exactly once.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.destroy_never_frees_a_child_to_the_allocator.patch`
#[kani::proof]
fn destroy_never_frees_a_child_to_the_allocator() {
    let pinned: bool = kani::any();
    let children: u32 = kani::any();
    let is_root: bool = kani::any();
    let is_lifo_top: bool = kani::any();
    let child_pages: u64 = kani::any();

    let outcome = destroy_outcome(pinned, children, is_root, is_lifo_top, child_pages);

    if pinned || children > 0 {
        assert!(
            outcome == DestroyOutcome::Refused,
            "a pinned or parent region must refuse"
        );
    } else if is_root {
        assert!(
            outcome == DestroyOutcome::FreeToGlobal,
            "a root frees to the allocator"
        );
    } else {
        // A child returns to its parent, never to the allocator: this is what prevents the
        // double-free when the parent's run is later freed.
        match outcome {
            DestroyOutcome::ReturnToParent { unbump } => {
                assert!(unbump == if is_lifo_top { child_pages } else { 0 });
            }
            _ => panic!("a child must not free to the allocator"),
        }
    }
}

/// **Reclaiming holes never lowers the watermark under anything still held, and never raises
/// it.** Under it stay the region's own retypes and every live child; an in-flight sibling
/// (not quiescent) leaves it where the LIFO rule put it. Together with
/// `split_stays_within_budget_and_progresses`, that is why a page carved after a reclaim is one
/// nothing else holds.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.coalescing_never_reclaims_a_held_page.patch`
#[kani::proof]
fn coalescing_never_reclaims_a_held_page() {
    let watermark: u64 = kani::any();
    let retyped_to: u64 = kani::any();
    let live_to: u64 = kani::any();
    let quiescent: bool = kani::any();
    // What the table guarantees: both floors are pages under the watermark.
    kani::assume(retyped_to <= watermark && live_to <= watermark);

    let after = coalesced_watermark(watermark, retyped_to, live_to, quiescent);

    assert!(after <= watermark, "reclaiming must never mint budget");
    assert!(after >= retyped_to, "the region's own retypes stay spent");
    assert!(after >= live_to, "a live child's pages stay spent");
    if !quiescent {
        assert!(
            after == watermark,
            "an in-flight sibling blocks the reclaim"
        );
    }
}

// --- Subtree ceilings ------------------------------------------------------------------------

use crate::table::RegionSnapshot;
//
// A root, a child and a grandchild, driven through a short symbolic run of carves, splits and
// releases over every kind. A ghost count of what each region retyped for itself is the model the
// table's live counts are checked against.

/// The regions the subtree-ceiling harnesses drive: a root, a child split from it, and a
/// grandchild split from the child.
const REGIONS: usize = 3;
/// How many symbolic steps a run takes.
const STEPS: usize = 3;
/// Every counted kind, in [`Ceilings`] order.
const KINDS: [Counted; 8] = [
    Counted::Object(ObjectKind::Rendezvous),
    Counted::Object(ObjectKind::AddressSpace),
    Counted::Object(ObjectKind::Thread),
    Counted::Object(ObjectKind::KernelPool),
    Counted::Object(ObjectKind::Notification),
    Counted::Object(ObjectKind::Timer),
    Counted::Object(ObjectKind::Process),
    Counted::Region,
];

fn any_object_kind() -> ObjectKind {
    match kani::any::<u8>() % 7 {
        0 => ObjectKind::Rendezvous,
        1 => ObjectKind::AddressSpace,
        2 => ObjectKind::Thread,
        3 => ObjectKind::KernelPool,
        4 => ObjectKind::Notification,
        5 => ObjectKind::Timer,
        _ => ObjectKind::Process,
    }
}

/// A root whose ceiling for each kind is anything up to 48, so a split child's sixteenth is
/// anything up to 3: small enough that a short run reaches it.
fn any_root_ceilings() -> Ceilings {
    let mut ceilings = Ceilings::UNLIMITED;
    for counted in KINDS {
        let most: u32 = kani::any();
        kani::assume(most <= 48);
        ceilings = ceilings.with(counted, most);
    }
    ceilings
}

/// The table, the three names (a refused split leaves `None`), and the ghost: how many objects of
/// each kind each region retyped for itself and has not released, and how many regions each split.
struct Run {
    table: RegionTable<4>,
    names: [Option<u64>; REGIONS],
    parent_of: [usize; REGIONS],
    own: [[u32; 8]; REGIONS],
}

impl Run {
    fn new() -> Self {
        let mut table = RegionTable::<4>::new();
        let root = table.insert_root_with_ceilings(0, 64, any_root_ceilings());
        let mut run = Run {
            table,
            names: [root, None, None],
            parent_of: [0, 0, 1],
            own: [[0; 8]; REGIONS],
        };
        for r in 1..REGIONS {
            let parent = run.parent_of[r];
            if let Some(p) = run.names[parent] {
                run.names[r] = run.table.split(p, 8);
                if run.names[r].is_some() {
                    run.own[parent][7] += 1;
                }
            }
        }
        run
    }

    /// `r`'s ghost subtree: its own counts and every live descendant's.
    fn ghost_tree_count(&self, r: usize, k: usize) -> u32 {
        let mut total = self.own[r][k];
        for d in (r + 1)..REGIONS {
            if self.names[d].is_some() && self.parent_of[d] == r {
                total += self.ghost_tree_count(d, k);
            }
        }
        total
    }

    fn snapshots(&self) -> [Option<RegionSnapshot>; REGIONS] {
        self.names.map(|n| n.and_then(|n| self.table.snapshot(n)))
    }

    /// One symbolic step on one region: a carve of any kind, a split of one page, or the release of
    /// an object it holds. Returns whether the table refused it.
    fn step(&mut self, only_below_root: bool) -> bool {
        let r = usize::from(kani::any::<u8>()) % REGIONS;
        kani::assume(!only_below_root || r > 0);
        let Some(name) = self.names[r] else {
            return false;
        };
        let kind = any_object_kind();
        match kani::any::<u8>() % 3 {
            0 => {
                let carved = self.table.retype_object_page(name, kind).is_some();
                if carved {
                    self.own[r][kind as usize] += 1;
                }
                !carved
            }
            1 => {
                // A split refused for a full table is the bump-only rule (`split`'s BUGS), not a
                // ceiling, so the table keeps a free slot.
                kani::assume(self.table.len() < 4);
                // Split, and forget the child: it stays live and counted, a region nobody drives.
                let split = self.table.split(name, 1).is_some();
                if split {
                    self.own[r][7] += 1;
                }
                !split
            }
            _ => {
                if self.own[r][kind as usize] > 0 {
                    self.table.release_object(name, kind);
                    self.own[r][kind as usize] -= 1;
                }
                false
            }
        }
    }
}

/// **A carve or a split the table refuses moves nothing**: no page, no pin, no child and no count,
/// in the refused region or any other, whichever limit refused it.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_refused_carve_or_split_moves_nothing.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_refused_carve_or_split_moves_nothing() {
    let mut run = Run::new();
    for _ in 0..STEPS {
        let before = run.snapshots();
        if run.step(false) {
            assert!(run.snapshots() == before, "a refusal moved something");
        }
    }
}

/// **Every region's live count is exactly what its subtree holds, and never above its ceiling**,
/// kind by kind, after every step of any run.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.live_counts_are_exact_and_never_pass_a_ceiling.patch`
#[kani::proof]
#[kani::unwind(9)]
fn live_counts_are_exact_and_never_pass_a_ceiling() {
    let mut run = Run::new();
    for _ in 0..STEPS {
        run.step(false);
        for r in 0..REGIONS {
            let Some(s) = run.names[r].and_then(|n| run.table.snapshot(n)) else {
                continue;
            };
            for (k, counted) in KINDS.into_iter().enumerate() {
                let live = s.live.get(counted);
                assert!(live <= s.ceilings.get(counted), "past a ceiling");
                // A child split by a step is not driven, so it holds no objects of its own.
                assert!(
                    live == run.ghost_tree_count(r, k),
                    "a count drifted from the subtree"
                );
            }
        }
    }
}

/// **Reclaiming a subtree gives every count it held back to the root**: after any run inside the
/// child's subtree, releasing what was reaped and reclaiming the grandchild and then the child
/// leaves the root's live counts at zero for every kind.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_reclaimed_region_tree_leaves_no_count_behind.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_reclaimed_region_tree_leaves_no_count_behind() {
    let mut run = Run::new();
    let (Some(root), Some(child), Some(grandchild)) = (run.names[0], run.names[1], run.names[2])
    else {
        return;
    };
    for _ in 0..STEPS {
        // Carves and releases only: a split here would leave a child the reclaim must refuse on.
        kani::assume(run.own[1][7] == 1 && run.own[2][7] == 0);
        run.step(true);
    }
    kani::assume(run.own[1][7] == 1 && run.own[2][7] == 0);
    // The reaper's half: a thread or a space it holds may already have been released, or not.
    for (r, region) in [(2, grandchild), (1, child)] {
        let kind = any_object_kind();
        if kani::any() && run.own[r][kind as usize] > 0 {
            run.table.release_object(region, kind);
        }
        run.table.unpin(region);
        let claim = run.table.claim_for_destroy(region);
        assert!(claim.is_some(), "an unpinned leaf is claimable");
        if let Some(claim) = claim {
            run.table.return_to_parent(&claim);
        }
    }
    let after = run.table.snapshot(root).expect("the root is live");
    for counted in KINDS {
        assert!(after.live.get(counted) == 0, "a count outlived its subtree");
    }
}
