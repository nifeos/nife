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

/// **A retyped run is exactly the pages asked for, all fresh, all within budget, and a run of no
/// pages is refused.** The retype count's whole contract, over every watermark and every count.
/// A refusal is exactly "empty or does not fit": the table does not move, which its own host test
/// shows.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_retyped_run_is_the_pages_asked_for_and_zero_is_refused.patch`
#[kani::proof]
fn a_retyped_run_is_the_pages_asked_for_and_zero_is_refused() {
    let pages: u64 = kani::any();
    let watermark: u64 = kani::any();
    let count: u64 = kani::any();
    kani::assume(watermark <= pages);

    match retype_new_watermark(pages, watermark, count) {
        Some(new) => {
            assert!(count > 0, "a run of no pages must be refused");
            assert!(
                new == watermark + count,
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
                count == 0 || watermark.checked_add(count).is_none() || watermark + count > pages,
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
/// (not quiescent) leaves it where the last-in-first-out rule put it. Together with
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
// A root, a child split from it and a grandchild split from the child. The invariant: every live
// count is the ghost count of what the region's subtree holds and is at or under its ceiling, and
// every split region lives under a sixteenth of its root's ceilings. A new root holds it, and the
// three `a_..._keeps_...` harnesses show that one carve, split or release from any state that
// holds it keeps it and that a split child starts in it, so by induction every run of any length
// does. The other harnesses start from any such state, so they hold after every run too. One
// operation from any state is a far smaller model than a run of operations from a fresh tree, and
// one harness per operation keeps each model under a hosted runner's memory.

/// The regions the subtree-ceiling harnesses drive: a root, a child and a grandchild.
const REGIONS: usize = 3;
/// Every counted kind, in [`Ceilings`] order.
const KINDS: [Counted; 8] = {
    let mut kinds = [Counted::Region; 8];
    let mut i = 0;
    while i < ObjectKind::ALL.len() {
        kinds[i] = Counted::Object(ObjectKind::ALL[i]);
        i += 1;
    }
    kinds
};
/// Where [`Counted::Region`] sits in [`KINDS`].
const REGION_COUNT: usize = 7;

fn any_object_kind() -> ObjectKind {
    ObjectKind::ALL[(kani::any::<u8>() % ObjectKind::ALL.len() as u8) as usize]
}

/// The ceilings region `r` of a tree lives under, when its root's are `root`: the root's own, or a
/// sixteenth of them, kind by kind, for every region split below it at any depth.
fn ceilings_of(r: usize, root: Ceilings) -> Ceilings {
    if r == 0 {
        return root;
    }
    let mut counts = [0; 8];
    for (k, counted) in KINDS.into_iter().enumerate() {
        counts[k] = root.get(counted) / 16;
    }
    Ceilings::from_counts(counts)
}

/// Kind by kind, since a derived comparison of the arrays is a byte loop the model checker cannot
/// bound.
fn same_ceilings(a: Ceilings, b: Ceilings) -> bool {
    KINDS.into_iter().all(|c| a.get(c) == b.get(c))
}

fn is_same_snapshot(a: Option<RegionSnapshot>, b: Option<RegionSnapshot>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.watermark == b.watermark
                && a.pinned == b.pinned
                && a.children == b.children
                && same_ceilings(a.live, b.live)
                && same_ceilings(a.ceilings, b.ceilings)
                && a.held.iter().zip(b.held).all(|(a, b)| *a == b)
        }
        _ => false,
    }
}

/// The invariant, part by part, over every live region of the tree.
struct Counts {
    /// Its ceilings are its root's, or a sixteenth of them below the root.
    shares: bool,
    /// Each live count is at or under its ceiling.
    within_ceilings: bool,
    /// Each live count is the ghost count of what its subtree holds.
    exact: bool,
}

/// The table, the three names (a region the tree does not reach is `None`), the root's ceilings,
/// and the ghost: how many objects of each kind each region retyped for itself and has not
/// released, and how many regions it split that are still live.
struct Run {
    table: RegionTable<4>,
    names: [Option<RegionName>; REGIONS],
    root_ceilings: Ceilings,
    own: [[u32; 8]; REGIONS],
}

impl Run {
    /// Any state of a tree `depth` regions deep that the invariant allows: any root ceilings, any
    /// live counts at or under each region's ceiling and at or over its child's, any pages and
    /// watermark, each child's run inside the part of its parent's that is spent. What a region
    /// counts beyond its child is its own. With `others`, its region count is any number of
    /// children, the one in the tree among them, and the rest are regions nobody drives; without,
    /// the tree is all there is.
    ///
    /// Every region is pinned as `pinned` says. The pin is a constant because no carve, split or
    /// release reads it, and the table keeps a region's `None` in the pin's spare values, so a
    /// symbolic pin makes every lookup a branch and the model twenty times larger.
    fn any(depth: usize, pinned: bool, others: bool) -> Self {
        let root_ceilings = Ceilings::from_counts(kani::any());
        let mut live = [[0u32; 8]; REGIONS];
        let mut own = [[0u32; 8]; REGIONS];
        for r in (0..depth).rev() {
            let ceilings = ceilings_of(r, root_ceilings);
            for (k, counted) in KINDS.into_iter().enumerate() {
                let below = if r + 1 < depth { live[r + 1][k] } else { 0 };
                let count: u32 = if k == REGION_COUNT && !others {
                    // A constant, since a claim branches on the child count it gives.
                    (depth - 1 - r) as u32
                } else {
                    kani::any()
                };
                kani::assume(below <= count && count <= ceilings.get(counted));
                live[r][k] = count;
                own[r][k] = count - below;
            }
            if r + 1 < depth {
                kani::assume(own[r][REGION_COUNT] >= 1);
            }
        }
        let mut table = RegionTable::<4>::new();
        let mut names = [None; REGIONS];
        let mut parent = NO_PARENT;
        let (mut spent_from, mut spent_to) = (0u64, u64::MAX);
        for r in 0..depth {
            let base_page: u64 = kani::any();
            let pages: u64 = kani::any();
            let watermark: u64 = kani::any();
            let retyped_to: u64 = kani::any();
            kani::assume(retyped_to <= watermark && watermark <= pages);
            kani::assume(spent_from <= base_page && base_page <= spent_to);
            kani::assume(pages <= spent_to - base_page);
            names[r] = table.insert_in_any_state(
                parent,
                (base_page, pages, watermark, retyped_to),
                pinned,
                own[r][REGION_COUNT],
                (
                    ceilings_of(r, root_ceilings),
                    Ceilings::from_counts(live[r]),
                    core::array::from_fn(|k| own[r][k]),
                ),
            );
            parent = names[r].expect("the table has room for the tree");
            (spent_from, spent_to) = (base_page, base_page + watermark);
        }
        Run {
            table,
            names,
            root_ceilings,
            own,
        }
    }

    /// `body` on the deepest region of [`any`](Self::any) unpinned state of a tree one, two or
    /// three regions deep, so a refusal that pins shows: the root, a child or a grandchild, with
    /// any number of regions of its own below it. A carve, a split and a release read and write
    /// only the region and its ancestors (the table keeps parent links and no child links), and a
    /// split also fills a free slot, so a tree below the region is never touched and adds nothing
    /// a deeper tree would show. Each depth is its own branch, so every name in it is
    /// a constant: a symbolic index makes each lookup a choice among every slot.
    fn for_every_depth(body: impl Fn(Run, usize)) {
        match kani::any::<u8>() % 3 {
            0 => {
                let run = Run::any(1, false, true);
                kani::cover!(true, "a root alone");
                body(run, 0);
            }
            1 => {
                let run = Run::any(2, false, true);
                kani::cover!(true, "a child below a root");
                body(run, 1);
            }
            _ => {
                let run = Run::any(3, false, true);
                kani::cover!(true, "a grandchild below a child");
                body(run, 2);
            }
        }
    }

    /// `r`'s ghost subtree: its own counts and every live descendant's.
    fn ghost_tree_count(&self, r: usize, k: usize) -> u32 {
        (r..REGIONS)
            .filter(|&d| self.names[d].is_some())
            .map(|d| self.own[d][k])
            .sum()
    }

    fn snapshots(&self) -> [Option<RegionSnapshot>; REGIONS] {
        self.names.map(|n| n.and_then(|n| self.table.snapshot(n)))
    }

    /// Whether every region's snapshot is the one in `before`.
    fn is_unmoved_since(&self, before: [Option<RegionSnapshot>; REGIONS]) -> bool {
        let now = self.snapshots();
        (0..REGIONS).all(|r| is_same_snapshot(now[r], before[r]))
    }

    /// Which parts of the invariant every live region of the tree holds.
    fn counts(&self) -> Counts {
        let mut counts = Counts {
            shares: true,
            within_ceilings: true,
            exact: true,
        };
        for r in 0..REGIONS {
            let Some(s) = self.names[r].and_then(|n| self.table.snapshot(n)) else {
                continue;
            };
            counts.shares &= same_ceilings(s.ceilings, ceilings_of(r, self.root_ceilings));
            for (k, counted) in KINDS.into_iter().enumerate() {
                let live = s.live.get(counted);
                counts.within_ceilings &= live <= s.ceilings.get(counted);
                counts.exact &= live == self.ghost_tree_count(r, k);
            }
        }
        counts
    }

    fn name(&self, r: usize) -> RegionName {
        self.names[r].expect("a region of the tree")
    }

    /// Carve one page for a `kind` object out of `r`. Returns whether the table refused.
    fn carve(&mut self, r: usize, kind: ObjectKind) -> bool {
        let carved = self.table.retype_object_page(self.name(r), kind).is_some();
        if carved {
            self.own[r][kind as usize] += 1;
        }
        !carved
    }

    /// Split one page off `r`. The tree holds at most three of the table's four slots, so only
    /// [`fill_table`](Self::fill_table) makes a split here refused for a full table. The child is
    /// live and counted, a region nobody drives. Returns its name, or `None` when the table
    /// refused.
    fn split(&mut self, r: usize) -> Option<RegionName> {
        let child = self.table.split(self.name(r), 1);
        if child.is_some() {
            self.own[r][REGION_COUNT] += 1;
        }
        child
    }

    /// Take every free slot of the table with a one-page root nobody drives, so the next split
    /// finds no slot for its child.
    fn fill_table(&mut self) {
        for _ in 0..4 {
            if self.table.insert_root(0, 1).is_none() {
                break;
            }
        }
    }

    /// Release a `kind` object from `r`, whether or not it holds one: the table must refuse a
    /// release of nothing held, a second release of one object, and take it only when `r` holds
    /// one. Returns whether it held one.
    fn release(&mut self, r: usize, kind: ObjectKind) -> bool {
        let held = self.own[r][kind as usize] > 0;
        let released = self.table.release_object(self.name(r), kind);
        assert!(
            released == held,
            "a release taken without a held object, or refused with one"
        );
        if held {
            self.own[r][kind as usize] -= 1;
        }
        held
    }
}

/// **A carve the table refuses moves nothing**: no page, no pin, no child and no count, in the
/// refused region or any other, whichever limit refused it, from any state.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_refused_carve_moves_nothing.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_refused_carve_moves_nothing() {
    Run::for_every_depth(|mut run, r| {
        let before = run.snapshots();
        let regions = run.table.len();
        let (watermark, pages) = run.table.usage(run.name(r)).expect("a region of the tree");
        let kind = any_object_kind();
        let refused = run.carve(r, kind);
        if refused {
            assert!(run.is_unmoved_since(before), "a refusal moved something");
            assert!(run.table.len() == regions, "a refusal left a region");
        }
        let own = before[r].expect("a region of the tree");
        let counted = Counted::Object(kind);
        kani::cover!(
            refused && watermark < pages && own.live.get(counted) < own.ceilings.get(counted),
            "a carve refused at an ancestor's ceiling"
        );
        kani::cover!(
            refused && watermark == pages,
            "a carve refused for its pages"
        );
    });
}

/// **A split the table refuses moves nothing**: no page, no pin, no child and no count, in the
/// refused region or any other, whichever limit refused it, from any state.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_refused_split_moves_nothing.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_refused_split_moves_nothing() {
    Run::for_every_depth(|mut run, r| {
        let before = run.snapshots();
        let regions = run.table.len();
        let (watermark, pages) = run.table.usage(run.name(r)).expect("a region of the tree");
        let refused = run.split(r).is_none();
        if refused {
            assert!(run.is_unmoved_since(before), "a refusal moved something");
            assert!(run.table.len() == regions, "a refusal left a region");
        }
        let own = before[r].expect("a region of the tree");
        let counted = Counted::Region;
        kani::cover!(
            refused && watermark < pages && own.live.get(counted) < own.ceilings.get(counted),
            "a split refused at an ancestor's ceiling"
        );
        kani::cover!(
            refused && watermark == pages,
            "a split refused for its pages"
        );
    });
}

/// **A split refused for a full table moves nothing**: from any state of the tree with every
/// other slot of the table taken, the split is refused and no page, pin, child or count moves.
/// Its own harness rather than a branch of
/// [`a_refused_split_moves_nothing`], whose model a symbolic full table makes too large to check
/// in 12 GiB.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_split_refused_for_a_full_table_moves_nothing.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_split_refused_for_a_full_table_moves_nothing() {
    Run::for_every_depth(|mut run, r| {
        run.fill_table();
        let before = run.snapshots();
        let regions = run.table.len();
        let (watermark, pages) = run.table.usage(run.name(r)).expect("a region of the tree");
        assert!(run.split(r).is_none(), "a full table took a child");
        assert!(run.is_unmoved_since(before), "a refusal moved something");
        assert!(run.table.len() == regions, "a refusal left a region");
        let own = before[r].expect("a region of the tree");
        let counted = Counted::Region;
        kani::cover!(
            watermark < pages && own.live.get(counted) < own.ceilings.get(counted),
            "a split that only the full table refuses"
        );
    });
}

/// **A carve keeps every region's live count exactly what its subtree holds, and never above its
/// ceiling**, kind by kind, from any state that held it.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_carve_keeps_every_count_exact_and_under_its_ceiling.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_carve_keeps_every_count_exact_and_under_its_ceiling() {
    Run::for_every_depth(|mut run, r| {
        let refused = run.carve(r, any_object_kind());
        let counts = run.counts();
        assert!(
            counts.shares,
            "a region's ceilings are not its root's share"
        );
        assert!(counts.within_ceilings, "past a ceiling");
        assert!(counts.exact, "a count drifted from the subtree");
        kani::cover!(
            !refused
                && r == 2
                && run.snapshots()[0].is_some_and(|s| {
                    KINDS
                        .into_iter()
                        .any(|c| c != Counted::Region && s.live.get(c) == s.ceilings.get(c))
                }),
            "a carve from the grandchild fills the root to a ceiling"
        );
    });
}

/// **A split keeps every region's live count exactly what its subtree holds, and never above its
/// ceiling**, kind by kind, from any state that held it, and the child it makes starts in such a
/// state: nothing counted, and a sixteenth of its root's ceilings. A root starts there too, so
/// every region of every tree does.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_split_keeps_every_count_exact_and_under_its_ceiling.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_split_keeps_every_count_exact_and_under_its_ceiling() {
    let mut table = RegionTable::<1>::new();
    let ceilings = Ceilings::from_counts(kani::any());
    let (base_page, pages): (u64, u64) = (kani::any(), kani::any());
    kani::assume(pages > 0 && base_page.checked_add(pages).is_some());
    let root = table
        .insert_root_with_ceilings(base_page, pages, ceilings)
        .expect("an empty table has room for a root that fits");
    let s = table.snapshot(root).expect("the root is live");
    assert!(
        same_ceilings(s.ceilings, ceilings)
            && same_ceilings(s.live, Ceilings::from_counts([0; 8]))
            && s.children == 0,
        "a new root starts with something counted"
    );
    Run::for_every_depth(|mut run, r| {
        let child = run.split(r);
        let counts = run.counts();
        assert!(
            counts.shares,
            "a region's ceilings are not its root's share"
        );
        assert!(counts.within_ceilings, "past a ceiling");
        assert!(counts.exact, "a count drifted from the subtree");
        if let Some(child) = child {
            let s = run.table.snapshot(child).expect("the child is live");
            assert!(
                same_ceilings(s.ceilings, ceilings_of(1, run.root_ceilings)),
                "a split child's ceilings are not its root's share"
            );
            assert!(
                same_ceilings(s.live, Ceilings::from_counts([0; 8])) && s.children == 0,
                "a split child starts with something counted"
            );
        }
        kani::cover!(child.is_some() && r == 2, "the grandchild splits");
    });
}

/// **A release keeps every region's live count exactly what its subtree holds**, kind by kind,
/// from any state that held it.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_release_keeps_every_count_exact.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_release_keeps_every_count_exact() {
    Run::for_every_depth(|mut run, r| {
        let held = run.release(r, any_object_kind());
        let counts = run.counts();
        assert!(
            counts.shares,
            "a region's ceilings are not its root's share"
        );
        assert!(counts.within_ceilings, "past a ceiling");
        assert!(counts.exact, "a count drifted from the subtree");
        kani::cover!(held && r == 2, "the grandchild releases an object");
    });
}

/// **A release of an object its region does not hold moves nothing**: a second release of one
/// object, or a release with no retype before it, is refused and leaves every count of both
/// regions where it was, from any state of a root and a child, even when the child holds objects
/// of that kind that a release from the root could otherwise take. The release from a leaf, which
/// has no such count to take, is [`a_release_keeps_every_count_exact`]'s.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_release_of_nothing_held_moves_nothing.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_release_of_nothing_held_moves_nothing() {
    let mut run = Run::any(2, false, true);
    let kind = any_object_kind();
    kani::assume(run.own[0][kind as usize] == 0);
    let before = run.snapshots();
    let root = before[0].expect("the root is live");
    let released = run.table.release_object(run.name(0), kind);
    assert!(!released, "a release of nothing held was taken");
    assert!(
        run.is_unmoved_since(before),
        "a refused release moved something"
    );
    kani::cover!(
        root.live.get(Counted::Object(kind)) > 0,
        "the child holds an object of the kind"
    );
}

/// A root over any page range, as the four "never overflows" harnesses below build it. A root of
/// no pages or one whose end does not fit is refused, and for exactly that reason; otherwise the
/// root is returned.
fn any_root_or_refused_for_its_range(table: &mut RegionTable<3>) -> Option<RegionName> {
    let (base_page, pages): (u64, u64) = (kani::any(), kani::any());
    let Some(root) = table.insert_root(base_page, pages) else {
        assert!(
            pages == 0 || base_page.checked_add(pages).is_none(),
            "a root that fits was refused"
        );
        return None;
    };
    kani::cover!(
        base_page > u64::MAX - 4,
        "a root at the top of the page numbers"
    );
    Some(root)
}

/// The reports a caller reads off a root, which compute ends and spent figures from its range.
fn read_every_report_of(table: &RegionTable<3>, root: RegionName) {
    let _ = table.usage(root);
    let _ = table.bounds(root);
    let _ = table.spent(root, PageUse::Frames);
    let _ = table.spent(root, PageUse::Children);
}

/// **No call on a root the table accepts panics, whatever its page range (retype a run)**: a root
/// at the top of the page-number space and any retype count run with the model checker's overflow
/// checks as the assertion. A root of no pages or one whose end does not fit is refused, and for
/// exactly that reason. Every region below a root lies inside it, so with those two refused no page
/// number or end the table computes overflows. The four `a_root_the_table_accepts_never_overflows_*`
/// harnesses split one proof by the operation driven, because one harness with all four runs past
/// the shard's memory.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_root_the_table_accepts_never_overflows_retyping_a_run.patch`
#[kani::proof]
#[kani::unwind(5)]
fn a_root_the_table_accepts_never_overflows_retyping_a_run() {
    let mut table = RegionTable::<3>::new();
    let Some(root) = any_root_or_refused_for_its_range(&mut table) else {
        return;
    };
    let _ = table.retype_run(root, kani::any());
    read_every_report_of(&table, root);
}

/// **No call on a root the table accepts panics, whatever its page range (retype an object
/// page)**: see [`a_root_the_table_accepts_never_overflows_retyping_a_run`].
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_root_the_table_accepts_never_overflows_retyping_an_object_page.patch`
#[kani::proof]
#[kani::unwind(5)]
fn a_root_the_table_accepts_never_overflows_retyping_an_object_page() {
    let mut table = RegionTable::<3>::new();
    let Some(root) = any_root_or_refused_for_its_range(&mut table) else {
        return;
    };
    let _ = table.retype_object_page(root, any_object_kind());
    read_every_report_of(&table, root);
}

/// **No call on a root the table accepts panics, whatever its page range (split a child and
/// return it)**: any split size, a retype in the child, and the return of the child. See
/// [`a_root_the_table_accepts_never_overflows_retyping_a_run`].
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_root_the_table_accepts_never_overflows_returning_a_child.patch`
#[kani::proof]
#[kani::unwind(5)]
fn a_root_the_table_accepts_never_overflows_returning_a_child() {
    let mut table = RegionTable::<3>::new();
    let Some(root) = any_root_or_refused_for_its_range(&mut table) else {
        return;
    };
    if let Some(child) = table.split(root, kani::any()) {
        let _ = table.retype_run(child, kani::any());
        table.unpin(child);
        if let Some(claim) = table.claim_for_destroy(child) {
            table.return_to_parent(&claim);
        }
    }
    read_every_report_of(&table, root);
}

/// **No call on a root the table accepts panics, whatever its page range (return a
/// grandchild)**: any two split sizes and the return of the grandchild. See
/// [`a_root_the_table_accepts_never_overflows_retyping_a_run`].
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_root_the_table_accepts_never_overflows_returning_a_grandchild.patch`
#[kani::proof]
#[kani::unwind(5)]
fn a_root_the_table_accepts_never_overflows_returning_a_grandchild() {
    let mut table = RegionTable::<3>::new();
    let Some(root) = any_root_or_refused_for_its_range(&mut table) else {
        return;
    };
    let range = table.bounds(root);
    if let Some(child) = table.split(root, kani::any())
        && let Some(grandchild) = table.split(child, kani::any())
        && let Some(claim) = table.claim_for_destroy(grandchild)
    {
        table.return_to_parent(&claim);
        kani::cover!(table.has_children(root), "the child stays below the root");
    }
    assert!(
        table.bounds(root) == range,
        "returning a grandchild moved the root's range"
    );
    read_every_report_of(&table, root);
}

/// **A return keeps every region's run where the table's discipline puts it**: from any state of
/// a root, a child and a grandchild, claiming the grandchild and giving its pages back leaves every
/// live region with `retyped_to <= watermark <= pages`, and the child's run inside the part of the
/// root's pages already spent. Every other harness assumes these as its start state; this is the one
/// that checks the table keeps them across a claim and a return.
///
/// **One assumption, stated.** `Run::any` leaves a parent's retyped pages free to overlap a live
/// child's run, which no sequence of the table's calls produces (a retype lands above every child
/// that exists, and a split above every retype before it). The harness assumes the states without
/// that overlap. Building the start state from real calls instead runs the C Bounded Model Checker
/// (CBMC) out of memory, so the reachability evidence is a host test, `no_short_call_sequence_overlaps_a_childs_run_with_its_parents_retyped_pages`,
/// which replays every call sequence of up to five calls and checks the same rule after each call.
/// The covers below keep the assumption from emptying the proof: a state with retyped pages below
/// a live grandchild and one with them above it both reach the claim.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_return_keeps_every_run_inside_its_parents_spent_part.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_return_keeps_every_run_inside_its_parents_spent_part() {
    // A root, a child and a grandchild in any state, built without `Run::any`'s counts and
    // ceilings, which this proof never reads and which make it far slower than the shard allows.
    // Each region sits inside its parent's spent part, retyped pages end by its watermark, and
    // the two parents hold at least the one child they have.
    let mut table = RegionTable::<4>::new();
    let mut names = [NO_PARENT; 3];
    let mut parent = NO_PARENT;
    let (mut spent_from, mut spent_to) = (0u64, u64::MAX);
    for (r, name) in names.iter_mut().enumerate() {
        let base_page: u64 = kani::any();
        let pages: u64 = kani::any();
        let watermark: u64 = kani::any();
        let retyped_to: u64 = kani::any();
        kani::assume(retyped_to <= watermark && watermark <= pages);
        kani::assume(spent_from <= base_page && base_page <= spent_to);
        kani::assume(pages <= spent_to - base_page);
        let children: u32 = if r < 2 { kani::any() } else { 0 };
        kani::assume(r == 2 || (1..=2).contains(&children));
        *name = table
            .insert_in_any_state(
                parent,
                (base_page, pages, watermark, retyped_to),
                false,
                children,
                (Ceilings::UNLIMITED, Ceilings::from_counts([0; 8]), [0; 7]),
            )
            .expect("the table has room for the tree");
        parent = *name;
        (spent_from, spent_to) = (base_page, base_page + watermark);
    }
    let before = names.map(|n| table.snapshot(n));
    for (parent, child) in [(0, 1), (1, 2)] {
        if let (Some(p), Some(c)) = (before[parent], before[child]) {
            let start = c.base_page - p.base_page;
            kani::assume(p.retyped_to <= start || p.retyped_to > start + c.pages);
        }
    }
    let claim = table.claim_for_destroy(names[2]);
    kani::cover!(
        claim.is_some() && before[1].is_some_and(|c| c.retyped_to > 0),
        "the grandchild is claimed under a child with retyped pages"
    );
    kani::cover!(
        claim.is_some()
            && before[1]
                .zip(before[2])
                .is_some_and(|(c, g)| c.retyped_to > g.base_page - c.base_page + g.pages),
        "the child retyped pages after its grandchild was split"
    );
    if let Some(claim) = claim {
        table.return_to_parent(&claim);
    }
    // Each region by name, never through `Iterator::flatten` over the three: the flatten's inner
    // loop is one the model checker cannot bound, and unrolling it ran CBMC past a hosted
    // runner's memory before the solver started.
    let [root, child, grandchild] = names.map(|n| table.snapshot(n));
    assert!(
        grandchild.is_none(),
        "the claimed grandchild still resolves"
    );
    let (root, child) = (root.expect("root"), child.expect("child"));
    for s in [root, child] {
        assert!(s.retyped_to <= s.watermark, "retyped past the watermark");
        assert!(s.watermark <= s.pages, "watermark past the region");
    }
    assert!(
        child.base_page >= root.base_page
            && child.base_page + child.pages <= root.base_page + root.watermark,
        "a child's run left its parent's spent part"
    );
}

/// **Reclaiming a subtree gives every count it held back to the root**: from any state of the
/// tree in which the root has carved nothing itself and nothing else was split below it,
/// releasing what was reaped and reclaiming the grandchild and then the child leaves the root's
/// live counts at zero for every kind.
/// Falsification: replayable `crates/memory_regions/falsifications/proofs.a_reclaimed_region_tree_leaves_no_count_behind.patch`
#[kani::proof]
#[kani::unwind(9)]
fn a_reclaimed_region_tree_leaves_no_count_behind() {
    let mut run = Run::any(3, true, false);
    for held in &run.own[0][..REGION_COUNT] {
        kani::assume(*held == 0);
    }
    kani::cover!(
        run.own[2][ObjectKind::Rendezvous as usize] > 0
            && run.own[1][ObjectKind::Process as usize] > 0,
        "the reclaimed tree holds objects the reaper does not release"
    );
    // The reaper's half: a thread or a space it holds may already have been released, or not.
    for r in [2, 1] {
        let name = run.name(r);
        if kani::any() {
            run.release(r, any_object_kind());
        }
        run.table.unpin(name);
        let claim = run.table.claim_for_destroy(name);
        assert!(claim.is_some(), "an unpinned leaf is claimable");
        if let Some(claim) = claim {
            run.table.return_to_parent(&claim);
        }
    }
    let after = run.table.snapshot(run.name(0)).expect("the root is live");
    for counted in KINDS {
        assert!(after.live.get(counted) == 0, "a count outlived its subtree");
    }
}
