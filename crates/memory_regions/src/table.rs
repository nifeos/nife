//! The region table, and the claim that makes reclamation single-winner. The crate root's
//! arithmetic is what one caller decides; this module is what happens when two callers hold a name
//! for one region, which the loom harnesses (milestone 135) model and Kani does not.
//! [`RegionTable::claim_for_destroy`] decides and removes in one `&mut self` call, so two callers
//! cannot both pass the check, and the [`DestroyClaim`] it mints is the only right to reclaim the
//! run. Everything is in page units, `base_page` included; the kernel multiplies. See
//! notes/memory-regions.md, "The `memory_regions` crate".
//!
//! Name: provisional, for `RegionTable`, `DestroyClaim`, `insert_root`, `claim_for_destroy` and
//! `return_to_parent`, minted 2026-08-18 by milestone 135 (the region claim, under loom)'s lane.
//! Nouns per the naming tenet; `RegionTable` takes its shape from the crate's name, `memory_regions`.

use crate::{DestroyOutcome, coalesced_watermark, destroy_outcome, split_new_watermark};

/// A region's parent field naming no parent: this region came from the frame allocator rather than from
/// a [`split`](RegionTable::split), so it is a **root** and its pages go back to the allocator.
///
/// `u64::MAX` never resolves as a `generational_table` name (its slot bits exceed any table's capacity), so it is
/// a safe sentinel rather than a value that could collide with a real region.
pub const NO_PARENT: u64 = u64::MAX;

/// One untyped region: a run of physical pages, and how far into it we have retyped.
///
/// Private on purpose. Every field is a decision input to something in this module, and a caller
/// that could read them could reconstruct the pre-#316 check-then-release-then-remove shape by
/// hand; the whole value of the claim is that it cannot.
#[derive(Clone, Copy)]
struct Region {
    /// Physical page number of the first page. Page units, not bytes: see the module docs.
    base_page: u64,
    pages: u64,
    /// Pages handed out so far. A bump pointer, and the whole of the allocator.
    watermark: u64,
    /// **How far this region's own retypes reached**: the watermark just after the highest page
    /// retyped straight out of this region rather than carved into a child. Bump-only. It is the
    /// floor [`return_to_parent`](RegionTable::return_to_parent) may lower the watermark to, since
    /// a page under it may hold an object or a mapping and is never handed out twice. Name:
    /// provisional.
    retyped_to: u64,
    /// **A kernel object lives in this region**: a page here was retyped into an endpoint, an
    /// address space or a TCB, so a claim is refused until `sched::reclaim_region` has torn the
    /// objects down and called [`unpin`](RegionTable::unpin).
    pinned: bool,
    /// The region this one was split out of, or [`NO_PARENT`].
    parent: u64,
    /// **How many live children were split off this region.** A count rather than a bool, so a
    /// parent becomes reclaimable again once its last child returns, which is what stops a split
    /// parent being committed for its whole lifetime.
    children: u32,
    /// **What the watermark was spent on**, one count per [`ObjectKind`] plus plain frames, and
    /// the pages currently carved into live children (milestone 126 (the `procps` package), §225 (`free` sees the machine and your share) part 1: `free`'s "yours"
    /// line and `slabtop` per object type). Pure bookkeeping beside the watermark: no decision in
    /// this module reads it, so it cannot become the read half of a claim.
    spent: Spent,
}

/// The per-use counts behind [`RegionTable::spent`]. Bump-only like the watermark, except
/// `children`, which falls when a child's pages come back.
#[derive(Clone, Copy, Default)]
struct Spent {
    frames: u64,
    objects: [u64; ObjectKind::COUNT],
    children: u64,
}

/// **Which kind of kernel object a page was retyped into**, so the region can say afterwards what
/// its pages were spent on (milestone 126, DECISIONS §225).
///
/// Name: provisional, minted 2026-09-26 by milestone 126's `free` lane.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ObjectKind {
    /// An IPC rendezvous (`abi::objtype::RENDEZVOUS`).
    Rendezvous,
    /// An address space's root table (`abi::objtype::ADDRESS_SPACE`).
    AddressSpace,
    /// A thread control block (`abi::objtype::THREAD_CONTROL_BLOCK`).
    Thread,
    /// A page of the kernel's own object pool (`kernel/src/kmem.rs`), whose contents change kind as
    /// pages are recycled, so no finer count would stay true.
    KernelPool,
    /// A notification object (`abi::objtype::NOTIFICATION`). Counted, and not yet reported by any
    /// `abi::usage` record: the ratified records predate this object kind (see that module's BUGS).
    Notification,
    /// A timer object (`abi::objtype::TIMER`), counted and unreported for the same reason.
    Timer,
    /// A process (`abi::objtype::PROCESS`, milestone 812 (`std::thread::spawn` runs real threads
    /// in one address space)), counted and unreported for the same reason.
    Process,
}

impl ObjectKind {
    const COUNT: usize = 7;
}

/// **One line of a region's spending, as [`RegionTable::spent`] reports it.**
///
/// Name: provisional, minted 2026-09-26 by milestone 126's `free` lane.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageUse {
    /// Plain pages: mapped memory, page tables, image pages and revocation records.
    Frames,
    /// Pages retyped into one kind of kernel object.
    Object(ObjectKind),
    /// Pages carved off into children that are still live.
    Children,
}

/// The exclusive right to reclaim one region. Minted only by
/// [`claim_for_destroy`](RegionTable::claim_for_destroy), which removes the region's table slot in
/// the same borrow, so at most one can exist for a region. Not `Copy` and not `Clone`: duplicating
/// a claim would duplicate the right to free the run.
///
/// # The two doctests below are a gate
///
/// `#[derive(Clone)]` or a `pub` field would hand the kernel back a right to free pages the table
/// no longer knows it gave out. `script/lint` pins this module's public items; these pin field
/// visibility and the missing traits, each by its error code, since a bare `compile_fail` passes
/// on any failure, a typo included. Private fields: a caller cannot build its own claim.
///
/// ```compile_fail,E0451
/// let claim = memory_regions::DestroyClaim { base_page: 0, pages: 4, parent: u64::MAX, is_root: true };
/// let _ = claim.pages();
/// ```
///
/// A claim cannot be duplicated: two claims for one region is two callers in the free loop.
///
/// ```compile_fail,E0599
/// let mut table = memory_regions::RegionTable::<2>::new();
/// let name = table.insert_root(0x1000, 4).unwrap();
/// let claim = table.claim_for_destroy(name).unwrap();
/// let second = claim.clone();
/// let _ = (claim.pages(), second.pages());
/// ```
///
/// The same sequence without the offending line compiles and runs, so the two above fail on it.
///
/// ```
/// let mut table = memory_regions::RegionTable::<2>::new();
/// let name = table.insert_root(0x1000, 4).unwrap();
/// let claim = table.claim_for_destroy(name).unwrap();
/// assert_eq!(claim.pages(), 4);
/// assert!(claim.is_root());
/// ```
#[derive(Debug, PartialEq, Eq)]
#[must_use = "a claim that is dropped without being spent leaks the region's pages"]
pub struct DestroyClaim {
    base_page: u64,
    pages: u64,
    parent: u64,
    is_root: bool,
}

impl DestroyClaim {
    /// Physical page number of the first page in the claimed run.
    #[must_use]
    pub fn base_page(&self) -> u64 {
        self.base_page
    }

    /// How many pages the claimed run holds.
    #[must_use]
    pub fn pages(&self) -> u64 {
        self.pages
    }

    /// **Whether these pages belong to the frame allocator.** `true` for a region created from it,
    /// so the caller frees the run; `false` for a split child, whose pages return to its parent via
    /// [`return_to_parent`](RegionTable::return_to_parent) and **never** to the allocator. That
    /// asymmetry is the no-double-free crux the crate root's Kani proof states.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.is_root
    }
}

/// **The untyped regions**, a generational table (`crates/generational_table`) plus every decision
/// taken over it. Removing a region bumps its slot's generation, so every capability minted for it
/// stops resolving: that is what makes [`claim_for_destroy`](Self::claim_for_destroy) single-winner.
/// `N` bounds the regions live at once (the kernel picks 256, the loom harnesses 2 or 4).
///
/// # Examples
///
/// ```
/// use memory_regions::RegionTable;
///
/// let mut table = RegionTable::<8>::new();
///
/// let root = table.insert_root(0x1000, 16).expect("a free slot");
///
/// let child = table.split(root, 4).expect("budget and a slot");
/// assert!(table.claim_for_destroy(root).is_none(), "a parent with a live child refuses");
///
/// // Reclaim the child: it returns its pages to the parent, never to the allocator.
/// let claim = table.claim_for_destroy(child).expect("a childless, unpinned region");
/// assert!(!claim.is_root(), "a split child is not a root");
/// // ... the kernel revokes claim.base_page()..+claim.pages() here ...
/// table.return_to_parent(&claim);
///
/// assert_eq!(table.usage(root), Some((0, 16)));
/// let claim = table.claim_for_destroy(root).expect("no children left");
/// assert!(claim.is_root(), "a created region frees to the allocator");
/// ```
pub struct RegionTable<const N: usize> {
    table: generational_table::Table<Region, N>,
}

impl<const N: usize> Default for RegionTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> RegionTable<N> {
    /// An empty table.
    ///
    /// `const` so a kernel static can hold one behind its lock.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            table: generational_table::Table::new(),
        }
    }

    /// **How many regions are live right now**, roots and split children together.
    ///
    /// The kernel reads this under its lock after every insert, to keep the high-water mark the
    /// test suite's closing `regions:` line prints (`memory_region::MAX_REGIONS` has the ledger).
    /// An observer about the table rather than about any region, so it names nothing a caller
    /// could act on after the answer stopped being true, which is the argument `script/lint`'s
    /// surface pin asks of a new `&self` method.
    ///
    /// Name: provisional, from milestone 601 (the region table prints its peak). It is
    /// `generational_table`'s own spelling of the same question.
    #[must_use]
    pub fn len(&self) -> usize {
        self.table.len()
    }

    /// Whether no region is live. The pair clippy asks of a public `len`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    /// **Record a region carved straight out of the frame allocator**, returning its generational
    /// name. `None` when every slot is live, in which case the caller still owns the pages and must
    /// give them back rather than leak them.
    ///
    /// This is the table half of the kernel's `create`; the allocation itself is the kernel's,
    /// because a crate that can be model-checked cannot also own a physical allocator.
    pub fn insert_root(&mut self, base_page: u64, pages: u64) -> Option<u64> {
        self.table.insert_with(|_| Region {
            base_page,
            pages,
            watermark: 0,
            retyped_to: 0,
            pinned: false,
            parent: NO_PARENT,
            children: 0,
            spent: Spent::default(),
        })
    }

    /// Carve `pages` off `parent`'s unspent budget into a new child region and return the child's
    /// name: seL4's untyped-retype-into-untyped, so a spawner can give each child its own
    /// independently reclaimable region.
    ///
    /// `None` if the parent's name is dead, its budget cannot cover the carve (the proved
    /// [`split_new_watermark`] decides), or the table is full. On a full table the run stays spent
    /// on the parent and its child count stays raised: the bump-only rule.
    ///
    /// # BUGS
    ///
    /// - A split refused for a full table leaves its parent undestroyable until reboot, and each
    ///   refusal makes the table fuller. The doctest below reproduces it, and
    ///   `notes/region-split-on-a-full-table.md` has the account, the kernel's `SPLIT` sibling and
    ///   the options:
    ///
    /// ```
    /// let mut table = memory_regions::RegionTable::<2>::new();
    /// let parent = table.insert_root(0x1000, 8).unwrap();
    /// let child = table.split(parent, 2).unwrap(); // the second and last slot
    ///
    /// // The table is full, so this split is refused, but the parent's count went up anyway.
    /// assert_eq!(table.split(parent, 2), None);
    ///
    /// // Reclaim the one real child; the parent should now be reclaimable, and is not.
    /// let claim = table.claim_for_destroy(child).unwrap();
    /// table.return_to_parent(&claim);
    /// assert!(table.has_children(parent), "a phantom child nobody can name");
    /// assert!(table.claim_for_destroy(parent).is_none(), "refused, and it always will be");
    /// assert_eq!(table.usage(parent), Some((4, 8)), "the refused carve stays spent");
    /// ```
    pub fn split(&mut self, parent: u64, pages: u64) -> Option<u64> {
        let base_page = {
            let r = self.table.get_mut(parent)?;
            let new_watermark = split_new_watermark(r.pages, r.watermark, pages)?;
            let base_page = r.base_page + r.watermark;
            r.watermark = new_watermark;
            r.children += 1;
            r.spent.children += pages;
            base_page
        };
        self.table.insert_with(|_| Region {
            base_page,
            pages,
            watermark: 0,
            retyped_to: 0,
            pinned: false,
            parent,
            children: 0,
            spent: Spent::default(),
        })
    }

    /// Whether this region has live children, so a claim on it will be refused. `false` for a dead
    /// name.
    #[must_use]
    pub fn has_children(&self, name: u64) -> bool {
        self.table.get(name).is_some_and(|r| r.children > 0)
    }

    /// **Retype one page out of the region**, returning its physical page number. `None` when the
    /// region is exhausted or its name is dead: the *process* is out of budget, not the kernel.
    ///
    /// The caller zeroes the page. This cannot, and the split is the reason the kernel keeps the
    /// I/O: a crate loom can run has no direct map to write through.
    pub fn retype_page(&mut self, name: u64) -> Option<u64> {
        self.retype_run(name, 1)
    }

    /// **Retype a run of pages out of the region**, returning the first page's number; the run is
    /// `requested` pages long, or one page when `requested` is `0` ([`crate::retype_pages`]). The
    /// pages are contiguous because the watermark is. `None`, with **nothing moved**, when the run
    /// does not fit or the name is dead.
    ///
    /// The caller zeroes the run, for [`retype_page`](RegionTable::retype_page)'s reason.
    pub fn retype_run(&mut self, name: u64, requested: u64) -> Option<u64> {
        let r = self.table.get_mut(name)?;
        let new = crate::retype_new_watermark(r.pages, r.watermark, requested)?;
        let page = r.base_page + r.watermark;
        let spent = new - r.watermark;
        r.watermark = new;
        r.retyped_to = new;
        r.spent.frames += spent;
        Some(page)
    }

    /// **Retype one page for a kernel object, pinning the region in the same breath.** The pin and
    /// the carve happen in one borrow, so no claim can slip between them and reclaim a page that is
    /// about to hold an endpoint.
    ///
    /// `None` on an exhausted or dead region, and **nothing is pinned** in that case: a caller that
    /// got no page owes no unpin.
    ///
    /// `kind` is recorded for [`spent`](Self::spent) and decides nothing here.
    pub fn retype_object_page(&mut self, name: u64, kind: ObjectKind) -> Option<u64> {
        let r = self.table.get_mut(name)?;
        if r.watermark >= r.pages {
            return None;
        }
        r.pinned = true;
        let page = r.base_page + r.watermark;
        r.watermark += 1;
        r.retyped_to = r.watermark;
        r.spent.objects[kind as usize] += 1;
        Some(page)
    }

    /// **Clear a region's object pin**, after its objects have been torn down. A no-op on a dead
    /// name.
    ///
    /// Deliberately separate from the claim, and the split is not cosmetic: tearing down a TCB needs
    /// the scheduler lock, and the claim must never take it, because reclamation is reachable from
    /// an address space's `Drop`.
    pub fn unpin(&mut self, name: u64) {
        if let Some(r) = self.table.get_mut(name) {
            r.pinned = false;
        }
    }

    /// How many pages the region has retyped, and how many it holds. `None` for a dead name.
    #[must_use]
    pub fn usage(&self, name: u64) -> Option<(u64, u64)> {
        self.table.get(name).map(|r| (r.watermark, r.pages))
    }

    /// How many of the region's pages went to `on` (milestone 126, DECISIONS §225 part 1), or
    /// `None` for a dead name. An observer: no decision reads these counts, so a stale answer can
    /// print a wrong number and free nothing. `Frames` and each `Object` kind count the whole
    /// subtree (this region and every live region split from it, however deep); `Children` is this
    /// region's alone. After an out-of-order return the counts do not sum to the watermark: the
    /// child's pages leave `Children` and stay spent, the hole
    /// [`return_to_parent`](Self::return_to_parent) describes.
    #[must_use]
    pub fn spent(&self, name: u64, on: PageUse) -> Option<u64> {
        let own = self.table.get(name)?;
        let count = |r: &Region| match on {
            PageUse::Frames => r.spent.frames,
            PageUse::Object(kind) => r.spent.objects[kind as usize],
            PageUse::Children => r.spent.children,
        };
        if on == PageUse::Children {
            return Some(count(own));
        }
        Some(
            self.table
                .iter()
                .filter(|&(n, _)| n == name || self.descends_from(n, name))
                .map(|(_, r)| count(r))
                .sum(),
        )
    }

    /// Whether live region `n` was split, directly or through its ancestors, from `ancestor`.
    /// Bounded by the capacity, because a parent chain longer than the table would be a cycle,
    /// which the split protocol cannot build.
    fn descends_from(&self, n: u64, ancestor: u64) -> bool {
        let mut at = self.table.get(n).map_or(NO_PARENT, |r| r.parent);
        for _ in 0..N {
            if at == NO_PARENT {
                return false;
            }
            if at == ancestor {
                return true;
            }
            at = self.table.get(at).map_or(NO_PARENT, |r| r.parent);
        }
        false
    }

    /// This region's physical span as `(base_page, pages)`, or `None` for a dead name. Object
    /// revocation needs it to find which kernel objects live inside the region.
    #[must_use]
    pub fn bounds(&self, name: u64) -> Option<(u64, u64)> {
        self.table.get(name).map(|r| (r.base_page, r.pages))
    }

    /// Decide whether this region may be reclaimed, and claim it in the same borrow.
    ///
    /// `Some(claim)` transfers the exclusive right to reclaim the run, and the name is dead the
    /// instant this returns, so a second caller gets `None` whatever the interleaving. `None` means
    /// the name was already dead or the region refuses: a live object pins it, or a child still owns
    /// part of its run. Removing the slot before the caller's I/O also makes a retype on a region on
    /// its way back to the allocator fail. See notes/memory-regions.md.
    ///
    /// # Examples
    ///
    /// ```
    /// use memory_regions::RegionTable;
    ///
    /// let mut table = RegionTable::<4>::new();
    /// let r = table.insert_root(0, 2).unwrap();
    /// assert!(table.claim_for_destroy(r).is_some());
    /// assert!(table.claim_for_destroy(r).is_none(), "the name stopped resolving at the first claim");
    /// ```
    pub fn claim_for_destroy(&mut self, name: u64) -> Option<DestroyClaim> {
        let r = self.table.get(name)?;
        let is_root = r.parent == NO_PARENT;
        // The refuse/root/child decision is the Kani-proved arithmetic in the crate root. The LIFO
        // input is irrelevant to a refusal and to the root case, so a placeholder is honest here;
        // `return_to_parent` computes the real one against the parent's current watermark.
        if destroy_outcome(r.pinned, r.children, is_root, false, r.pages) == DestroyOutcome::Refused
        {
            return None;
        }
        let claim = DestroyClaim {
            base_page: r.base_page,
            pages: r.pages,
            parent: r.parent,
            is_root,
        };
        // The generation bump. Every `MemoryRegion` capability for this region stops resolving here, and
        // that is what makes the claim single-winner rather than merely first.
        self.table.remove(name);
        Some(claim)
    }

    /// Give a claimed child's pages back to its parent's budget, after the caller has revoked every
    /// mapping in the run; a no-op for a root claim or a parent whose name has died. The LIFO test
    /// and the un-bump share one borrow. The watermark then falls to the highest page still held
    /// ([`crate::coalesced_watermark`]) unless a claimed sibling is in flight, and the child count
    /// drops here rather than at the claim, so the parent cannot be reclaimed under this child's
    /// pages. See notes/memory-regions.md, "The `memory_regions` crate".
    ///
    /// # BUGS
    /// - A hole under a live child is not reused while that child lives.
    /// - A phantom child ([`split`](Self::split)'s `BUGS`) stops the coalescing for good.
    pub fn return_to_parent(&mut self, claim: &DestroyClaim) {
        if claim.is_root {
            return;
        }
        let Some(parent_base) = self.table.get(claim.parent).map(|p| p.base_page) else {
            return;
        };
        // What the parent still has out: how many of its children are live, and where the highest
        // one ends, in the parent's own page offsets. The returning child left the table at its
        // claim, so it is in neither figure.
        let (live, live_to) = self
            .table
            .iter()
            .filter(|(_, r)| r.parent == claim.parent)
            .fold((0u32, 0u64), |(n, to), (_, r)| {
                (n + 1, to.max(r.base_page + r.pages - parent_base))
            });
        let Some(p) = self.table.get_mut(claim.parent) else {
            return;
        };
        let is_lifo_top = claim.base_page + claim.pages == p.base_page + p.watermark;
        if let DestroyOutcome::ReturnToParent { unbump } =
            destroy_outcome(false, 0, false, is_lifo_top, claim.pages)
        {
            p.watermark -= unbump;
        }
        p.children = p.children.saturating_sub(1);
        p.spent.children = p.spent.children.saturating_sub(claim.pages);
        p.watermark = coalesced_watermark(p.watermark, p.retyped_to, live_to, p.children == live);
    }
}

// --- The ordinary host tests -------------------------------------------------------------------
//
// Single-threaded: what each operation means, and that the refusals are refusals. The
// *interleavings* are the loom module below, which is a different question and a different tool.
#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;

    #[test]
    fn a_claim_kills_the_name_so_a_second_caller_gets_nothing() {
        let mut t = RegionTable::<4>::new();
        let r = t.insert_root(0x100, 8).unwrap();
        assert!(t.claim_for_destroy(r).is_some());
        assert!(t.claim_for_destroy(r).is_none());
        assert_eq!(t.bounds(r), None, "the name is dead, not merely claimed");
    }

    /// `len` and `is_empty` are what the region table prints as its peak (milestone 601): they must
    /// follow the live count up through inserts and splits and back down through claims.
    #[test]
    fn len_and_is_empty_follow_the_live_regions() {
        let mut t = RegionTable::<4>::new();
        assert_eq!((t.len(), t.is_empty()), (0, true));
        let root = t.insert_root(0, 16).unwrap();
        assert_eq!((t.len(), t.is_empty()), (1, false));
        let child = t.split(root, 4).unwrap();
        assert_eq!((t.len(), t.is_empty()), (2, false));
        let claim = t.claim_for_destroy(child).unwrap();
        t.return_to_parent(&claim);
        assert_eq!((t.len(), t.is_empty()), (1, false));
        assert!(t.claim_for_destroy(root).is_some());
        assert_eq!((t.len(), t.is_empty()), (0, true));
    }

    /// Returning the top child after a hole below it lets the parent's watermark fall to the highest
    /// child still live, measured in the parent's own page offsets: the parent sits at a non-zero
    /// base so a child's end taken from the wrong origin is a different number.
    #[test]
    fn the_watermark_falls_to_the_highest_live_child_after_a_hole() {
        let mut t = RegionTable::<8>::new();
        let root = t.insert_root(0x100, 16).unwrap();
        let a = t.split(root, 4).unwrap();
        let b = t.split(root, 4).unwrap();
        let c = t.split(root, 4).unwrap();
        assert_eq!(t.usage(root), Some((12, 16)));

        let cb = t.claim_for_destroy(b).unwrap();
        t.return_to_parent(&cb);
        assert_eq!(
            t.usage(root),
            Some((12, 16)),
            "a hole under `c` keeps the mark"
        );

        let cc = t.claim_for_destroy(c).unwrap();
        t.return_to_parent(&cc);
        assert_eq!(
            t.usage(root),
            Some((4, 16)),
            "down to the end of `a`, not to 8"
        );
        assert!(t.bounds(a).is_some());
    }

    #[test]
    fn a_pinned_region_refuses_and_stays_alive() {
        let mut t = RegionTable::<4>::new();
        let r = t.insert_root(0x100, 8).unwrap();
        assert_eq!(t.retype_object_page(r, ObjectKind::Rendezvous), Some(0x100));
        assert!(t.claim_for_destroy(r).is_none(), "an object pins it");
        assert_eq!(t.bounds(r), Some((0x100, 8)), "a refusal must not remove");
        t.unpin(r);
        assert!(t.claim_for_destroy(r).is_some());
    }

    #[test]
    fn a_parent_refuses_until_its_last_child_returns() {
        let mut t = RegionTable::<4>::new();
        let root = t.insert_root(0, 16).unwrap();
        let a = t.split(root, 4).unwrap();
        let b = t.split(root, 4).unwrap();
        assert!(t.claim_for_destroy(root).is_none());

        // `b` sits at the top, so it un-bumps; `a` does not, so it leaves a hole.
        let cb = t.claim_for_destroy(b).unwrap();
        t.return_to_parent(&cb);
        assert_eq!(t.usage(root), Some((4, 16)));
        assert!(t.claim_for_destroy(root).is_none(), "`a` is still live");

        let ca = t.claim_for_destroy(a).unwrap();
        t.return_to_parent(&ca);
        assert_eq!(
            t.usage(root),
            Some((0, 16)),
            "the last return closes the hole"
        );
        assert!(t.claim_for_destroy(root).is_some());
    }

    #[test]
    fn spending_is_counted_by_use_and_a_hole_shows_as_the_difference() {
        let mut t = RegionTable::<4>::new();
        let root = t.insert_root(0, 16).unwrap();
        t.retype_page(root).unwrap();
        t.retype_object_page(root, ObjectKind::Thread).unwrap();
        t.retype_object_page(root, ObjectKind::Thread).unwrap();
        t.retype_object_page(root, ObjectKind::AddressSpace)
            .unwrap();
        t.unpin(root);
        let a = t.split(root, 4).unwrap();
        let _b = t.split(root, 2).unwrap();
        let used = |on| t.spent(root, on).unwrap();
        assert_eq!(used(PageUse::Frames), 1);
        assert_eq!(used(PageUse::Object(ObjectKind::Thread)), 2);
        assert_eq!(used(PageUse::Object(ObjectKind::AddressSpace)), 1);
        assert_eq!(used(PageUse::Object(ObjectKind::Rendezvous)), 0);
        assert_eq!(used(PageUse::Children), 6);
        assert_eq!(
            t.usage(root),
            Some((10, 16)),
            "every page is one of the above"
        );

        // `a` is below `_b`, so its return leaves a hole: out of `Children`, still spent.
        let ca = t.claim_for_destroy(a).unwrap();
        t.return_to_parent(&ca);
        assert_eq!(t.spent(root, PageUse::Children), Some(2));
        assert_eq!(t.usage(root), Some((10, 16)));
        assert_eq!(
            t.spent(a, PageUse::Frames),
            None,
            "a dead name answers nothing"
        );
    }

    #[test]
    fn object_counts_reach_through_every_live_descendant_and_children_do_not() {
        let mut t = RegionTable::<8>::new();
        let budget = t.insert_root(0, 64).unwrap();
        let job = t.split(budget, 16).unwrap();
        let grandchild = t.split(job, 4).unwrap();
        t.retype_object_page(job, ObjectKind::Thread).unwrap();
        t.retype_object_page(grandchild, ObjectKind::Thread)
            .unwrap();
        t.retype_page(grandchild).unwrap();
        let unrelated = t.insert_root(0x1000, 8).unwrap();
        t.retype_object_page(unrelated, ObjectKind::Thread).unwrap();

        let thread = PageUse::Object(ObjectKind::Thread);
        assert_eq!(
            t.spent(budget, thread),
            Some(2),
            "job and grandchild, not the stranger"
        );
        assert_eq!(t.spent(budget, PageUse::Frames), Some(1));
        assert_eq!(
            t.spent(budget, PageUse::Children),
            Some(16),
            "direct children only"
        );
        assert_eq!(t.spent(job, thread), Some(2));
        assert_eq!(t.spent(grandchild, thread), Some(1));
    }

    #[test]
    fn a_child_freed_out_of_order_leaves_a_hole() {
        let mut t = RegionTable::<4>::new();
        let root = t.insert_root(0, 16).unwrap();
        let a = t.split(root, 4).unwrap();
        let _b = t.split(root, 4).unwrap();
        let ca = t.claim_for_destroy(a).unwrap();
        assert!(!ca.is_root());
        t.return_to_parent(&ca);
        assert_eq!(
            t.usage(root),
            Some((8, 16)),
            "an out-of-order return un-bumps nothing"
        );
    }

    /// The job pool's leak, in miniature: a pipeline's writer ends before its reader, so its region
    /// comes back out of order. Before 2026-10-03 the watermark stopped at 4 here for good, and the
    /// progenitor's pool lost a job's pages to every pipeline a session ran.
    #[test]
    fn a_hole_closes_when_the_child_above_it_returns() {
        let mut t = RegionTable::<4>::new();
        let root = t.insert_root(0, 16).unwrap();
        let a = t.split(root, 4).unwrap();
        let b = t.split(root, 4).unwrap();
        let ca = t.claim_for_destroy(a).unwrap();
        t.return_to_parent(&ca);
        assert_eq!(t.usage(root), Some((8, 16)), "`b` is live above the hole");
        let cb = t.claim_for_destroy(b).unwrap();
        t.return_to_parent(&cb);
        assert_eq!(
            t.usage(root),
            Some((0, 16)),
            "nothing above the hole is held"
        );
        assert_eq!(
            t.split(root, 16).map(|_| ()),
            Some(()),
            "the whole budget again"
        );
    }

    /// A sibling claimed and not yet returned is invisible to the scan, so it must hold the
    /// watermark where the LIFO rule leaves it: its pages may still be mapped until the caller's
    /// revoke finishes, and a split that handed them out now would be the use-after-free.
    #[test]
    fn an_in_flight_sibling_holds_the_watermark_until_it_returns() {
        let mut t = RegionTable::<4>::new();
        let root = t.insert_root(0, 16).unwrap();
        let a = t.split(root, 4).unwrap();
        let b = t.split(root, 4).unwrap();
        let c = t.split(root, 4).unwrap();
        let cb = t.claim_for_destroy(b).unwrap(); // in flight: out of the table, not yet returned
        let ca = t.claim_for_destroy(a).unwrap();
        t.return_to_parent(&ca);
        let cc = t.claim_for_destroy(c).unwrap();
        t.return_to_parent(&cc);
        assert_eq!(
            t.usage(root),
            Some((8, 16)),
            "`b`'s run, [4, 8), stays spent"
        );
        t.return_to_parent(&cb);
        assert_eq!(
            t.usage(root),
            Some((0, 16)),
            "and the last return reclaims `a`'s hole too"
        );
    }

    /// A page the parent retyped for itself is never reclaimed with the children's holes.
    #[test]
    fn a_hole_closes_down_to_the_parents_own_retypes_and_no_further() {
        let mut t = RegionTable::<4>::new();
        let root = t.insert_root(0, 16).unwrap();
        let a = t.split(root, 4).unwrap();
        assert_eq!(
            t.retype_page(root),
            Some(4),
            "the parent's own page, above `a`"
        );
        let b = t.split(root, 4).unwrap();
        let ca = t.claim_for_destroy(a).unwrap();
        t.return_to_parent(&ca);
        let cb = t.claim_for_destroy(b).unwrap();
        t.return_to_parent(&cb);
        assert_eq!(
            t.usage(root),
            Some((5, 16)),
            "the retyped page stays under the watermark"
        );
    }

    #[test]
    fn a_root_claim_frees_to_the_allocator_and_a_child_never_does() {
        let mut t = RegionTable::<4>::new();
        let root = t.insert_root(0x200, 8).unwrap();
        let child = t.split(root, 2).unwrap();
        let cc = t.claim_for_destroy(child).unwrap();
        assert!(!cc.is_root(), "a split child is never a root");
        assert_eq!((cc.base_page(), cc.pages()), (0x200, 2));
        t.return_to_parent(&cc);
        let cr = t.claim_for_destroy(root).unwrap();
        assert!(cr.is_root());
        assert_eq!((cr.base_page(), cr.pages()), (0x200, 8));
    }

    #[test]
    fn retype_spends_the_budget_and_then_refuses() {
        let mut t = RegionTable::<4>::new();
        let r = t.insert_root(7, 2).unwrap();
        assert_eq!(t.retype_page(r), Some(7));
        assert_eq!(t.retype_page(r), Some(8));
        assert_eq!(t.retype_page(r), None, "exhausted, not an error");
        assert_eq!(t.usage(r), Some((2, 2)));
    }

    /// **A run is contiguous, `0` is one page, and a run that does not fit moves nothing.** The
    /// last is the one a caller relies on: a refused `RETYPE` of three pages must leave the two
    /// that remain retypable, one page at a time or as a run.
    #[test]
    fn a_refused_run_moves_nothing_and_zero_means_one_page() {
        let mut t = RegionTable::<4>::new();
        let r = t.insert_root(100, 6).unwrap();
        assert_eq!(t.retype_run(r, 0), Some(100), "0 is one page");
        assert_eq!(t.usage(r), Some((1, 6)));
        assert_eq!(
            t.retype_run(r, 3),
            Some(101),
            "a run starts at the watermark"
        );
        assert_eq!(t.usage(r), Some((4, 6)), "and advances it by the whole run");
        assert_eq!(t.retype_run(r, 3), None, "three pages do not fit in two");
        assert_eq!(
            t.usage(r),
            Some((4, 6)),
            "a refused run moved the watermark"
        );
        assert_eq!(
            t.retype_run(r, u64::MAX),
            None,
            "a wrapping count is refused"
        );
        assert_eq!(t.usage(r), Some((4, 6)));
        assert_eq!(
            t.retype_run(r, 2),
            Some(104),
            "the two that remain are still a run"
        );
        assert_eq!(t.retype_page(r), None);
    }

    #[test]
    fn an_exhausted_region_pins_nothing() {
        let mut t = RegionTable::<4>::new();
        let r = t.insert_root(0, 1).unwrap();
        assert_eq!(t.retype_page(r), Some(0));
        assert_eq!(t.retype_object_page(r, ObjectKind::Rendezvous), None);
        assert!(
            t.claim_for_destroy(r).is_some(),
            "a failed object retype must not leave the region pinned"
        );
    }

    /// **The claim gate's question, asked directly.** `claim_for_destroy` consults the `children`
    /// count through `destroy_outcome` rather than through this accessor, so every test above
    /// exercises the *refusal* without ever calling `has_children`; the only call one made was on a
    /// dead name, where `get` returns `None` and the predicate inside is never evaluated. A mutation
    /// run of 2026-09-19 (milestone 326) found all four of its mutants alive on that, including the
    /// whole function replaced by `false`. The kernel asks this before it offers to destroy, so a
    /// constant answer is a region reported childless while a child still holds pages inside it.
    #[test]
    fn has_children_answers_for_the_living_the_childless_and_the_dead() {
        let mut t = RegionTable::<4>::new();
        let root = t.insert_root(0, 16).unwrap();
        assert!(!t.has_children(root), "a fresh root has split nothing off");

        let child = t.split(root, 4).unwrap();
        assert!(t.has_children(root));
        assert!(!t.has_children(child), "a leaf is not its own parent");

        let c = t.claim_for_destroy(child).unwrap();
        t.return_to_parent(&c);
        assert!(!t.has_children(root), "the last return empties the count");
        assert!(!t.has_children(child), "and a dead name has no children");
    }

    /// **The object retype carves the same run `retype_page` does**, and until 2026-09-19 nothing
    /// said so: the tests above call it once, at watermark zero, where `base_page + watermark` and
    /// `base_page - watermark` agree and a watermark that never advances is invisible. Milestone
    /// 326's run found both. A page handed out twice is two kernel objects on one frame.
    #[test]
    fn the_object_retype_walks_the_region_one_page_at_a_time() {
        let mut t = RegionTable::<4>::new();
        let r = t.insert_root(0x40, 3).unwrap();
        assert_eq!(t.retype_object_page(r, ObjectKind::Rendezvous), Some(0x40));
        assert_eq!(t.retype_object_page(r, ObjectKind::Rendezvous), Some(0x41));
        assert_eq!(t.usage(r), Some((2, 3)));
        // And it shares one budget with the plain retype rather than keeping its own.
        assert_eq!(t.retype_page(r), Some(0x42));
        assert_eq!(
            t.retype_object_page(r, ObjectKind::Rendezvous),
            None,
            "exhausted, not an error"
        );
    }

    #[test]
    fn a_full_table_refuses_rather_than_overwriting() {
        let mut t = RegionTable::<2>::new();
        assert!(t.insert_root(0, 1).is_some());
        assert!(t.insert_root(1, 1).is_some());
        assert!(t.insert_root(2, 1).is_none(), "no slot left");
    }

    #[test]
    fn a_dead_name_is_inert_everywhere() {
        let mut t = RegionTable::<2>::new();
        let r = t.insert_root(0, 4).unwrap();
        let claim = t.claim_for_destroy(r).unwrap();
        assert!(claim.is_root());
        assert_eq!(t.retype_page(r), None);
        assert_eq!(t.retype_object_page(r, ObjectKind::Rendezvous), None);
        assert_eq!(t.split(r, 1), None);
        assert!(!t.has_children(r));
        assert_eq!(t.usage(r), None);
        t.unpin(r); // must not panic
    }
}

// --- The loom model ----------------------------------------------------------------------------
#[cfg(all(test, loom))]
mod interleavings {
    //! Every interleaving of the region claim, and every reordering C11 permits within it. Run with
    //! `script/interleaving-check`; notes/interleaving.md has the method and
    //! notes/object-revocation.md the bug behind it.
    //!
    //! The protocol is lock-based, so loom searches the interleaving of critical sections, not
    //! memory orderings: the model assumes mutual exclusion and says nothing about whether
    //! `IrqSafeMutex` delivers it. The harnesses use `RegionTable<2>` or `<4>`, not the kernel's 256,
    //! because the search is exponential in the interleavings and nothing in the protocol depends
    //! on capacity. Every other input is the kernel's own source.

    use loom::sync::{Arc, Mutex};
    use loom::thread;

    use super::*;

    /// **Non-vacuity, which a model checker needs for exactly the reason a solver does.**
    ///
    /// A harness whose interesting branch is never reached passes without checking anything, and
    /// reports success while doing it. Kani's answer is `kani::cover!`; loom has no equivalent, so
    /// these count reachability across the model's executions and the harness asserts on the count
    /// afterwards. The flag is a *real* atomic, deliberately: it lives outside the model and
    /// accumulates across every execution loom runs, where a loom atomic would be reset by each.
    ///
    /// Copied from `crates/memory_corruption_canary_gate`, which is where this shape was first written. See
    /// notes/verification.md's non-vacuity section for why it is not optional.
    struct Reached(core::sync::atomic::AtomicBool);

    impl Reached {
        const fn new() -> Self {
            Self(core::sync::atomic::AtomicBool::new(false))
        }
        fn mark(&self) {
            self.0.store(true, core::sync::atomic::Ordering::Relaxed);
        }
        fn assert(&self, what: &str) {
            assert!(
                self.0.load(core::sync::atomic::Ordering::Relaxed),
                "vacuous harness: no execution ever reached {what}, so nothing was checked"
            );
        }
    }

    /// **The spending observer never sees a child's pages half returned** (milestone 126 (the
    /// `procps` package), `MemoryRegion::USAGE`). One thread reclaims a child and returns its pages;
    /// the other asks the parent what is carved into children. Every execution must read the whole
    /// child or none of it, and loom must be seen reaching both answers.
    #[test]
    fn a_spending_read_racing_a_return_sees_all_of_a_child_or_none() {
        static SAW_BEFORE: Reached = Reached::new();
        static SAW_AFTER: Reached = Reached::new();

        loom::model(|| {
            let table = Arc::new(Mutex::new(RegionTable::<4>::new()));
            let (parent, child) = {
                let mut t = table.lock().unwrap();
                let parent = t.insert_root(0, 8).unwrap();
                (parent, t.split(parent, 3).unwrap())
            };
            let reclaimer = {
                let table = Arc::clone(&table);
                thread::spawn(move || {
                    let claim = table.lock().unwrap().claim_for_destroy(child).unwrap();
                    // The kernel's revoke runs here, with the lock released.
                    table.lock().unwrap().return_to_parent(&claim);
                })
            };
            let seen = table.lock().unwrap().spent(parent, PageUse::Children);
            reclaimer.join().unwrap();
            match seen {
                Some(3) => SAW_BEFORE.mark(),
                Some(0) => SAW_AFTER.mark(),
                other => panic!("a read saw part of a child: {other:?}"),
            }
        });
        SAW_BEFORE.assert("a read before the return");
        SAW_AFTER.assert("a read after the return");
    }

    /// **The property the whole reclamation path rests on: two callers, one winner.**
    ///
    /// The kernel reaches `destroy` for one region by two routes that can run concurrently on two
    /// cores, an owner's `MemoryRegion::DESTROY` and a supervisor's `rendezvous::REAP`. Each thread here
    /// takes the lock, claims, and (if it won) runs the free loop, counting the run. Loom fails the
    /// model on any execution where the count reaches two.
    ///
    /// The two `Reached` flags are what make it non-vacuous: they assert that loom really explored
    /// both winners, rather than running one order twice.
    #[test]
    fn two_destroyers_race_and_exactly_one_reclaims() {
        static FIRST_WON: Reached = Reached::new();
        static SECOND_WON: Reached = Reached::new();

        loom::model(|| {
            let table = Arc::new(Mutex::new(RegionTable::<2>::new()));
            let name = table.lock().unwrap().insert_root(0x1000, 4).unwrap();
            // Stands in for the frame allocator: how many times the free loop ran over this run,
            // and which thread ran it. Two is the double free, in the units the bug report used.
            let freed = Arc::new(Mutex::new(Vec::<usize>::new()));

            let handles: Vec<_> = (0..2usize)
                .map(|who| {
                    let table = Arc::clone(&table);
                    let freed = Arc::clone(&freed);
                    thread::spawn(move || {
                        let claim = table.lock().unwrap().claim_for_destroy(name);
                        if let Some(claim) = claim {
                            assert!(claim.is_root(), "a created region frees to the allocator");
                            assert_eq!((claim.base_page(), claim.pages()), (0x1000, 4));
                            // The kernel revokes and frees HERE, with the lock released. That
                            // release is what the pre-#316 protocol could not survive.
                            freed.lock().unwrap().push(who);
                        }
                    })
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }

            let freed = freed.lock().unwrap();
            assert_eq!(
                freed.len(),
                1,
                "exactly one caller may free a region's pages, and this execution had {}",
                freed.len()
            );
            match freed[0] {
                0 => FIRST_WON.mark(),
                _ => SECOND_WON.mark(),
            }
        });

        FIRST_WON.assert("an execution in which the first destroyer won");
        SECOND_WON.assert("an execution in which the second destroyer won");
    }

    /// The falsification witness: the pre-#316 protocol, and loom finding its double free.
    ///
    /// A model that cannot fail proves nothing (milestone 62), so the broken protocol lives here
    /// permanently, written against this module's private internals in the shape
    /// `memory_region::destroy` had before the fix: check under the lock, release it, revoke, free
    /// every page, and remove the slot last. The assertion is inverted on purpose: this harness
    /// passes only when loom finds an execution in which both callers free the same run.
    #[test]
    fn the_pre_fix_protocol_double_frees_and_this_model_finds_it() {
        static DOUBLE_FREE: Reached = Reached::new();

        /// `memory_region::destroy` as it stood before pull request #316. Kept in the test module rather
        /// than in the crate's API so that nothing outside a loom run can call it.
        fn destroy_with_the_gap(table: &Mutex<RegionTable<2>>, name: u64, freed: &Mutex<usize>) {
            // 1. Decide under the lock.
            let captured = {
                let guard = table.lock().unwrap();
                let Some(r) = guard.table.get(name) else {
                    return;
                };
                let is_root = r.parent == NO_PARENT;
                if destroy_outcome(r.pinned, r.children, is_root, false, r.pages)
                    == DestroyOutcome::Refused
                {
                    return;
                }
                (r.base_page, r.pages)
            };
            // 2. Release the lock and revoke every mapping in the run. This is the gap.
            thread::yield_now();
            // 3. Free every page, whether or not anyone else already did.
            let _ = captured;
            *freed.lock().unwrap() += 1;
            // 4. And only now remove the slot, far too late to have decided anything.
            table.lock().unwrap().table.remove(name);
        }

        loom::model(|| {
            let table = Arc::new(Mutex::new(RegionTable::<2>::new()));
            let name = table.lock().unwrap().insert_root(0x1000, 4).unwrap();
            let freed = Arc::new(Mutex::new(0usize));

            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let table = Arc::clone(&table);
                    let freed = Arc::clone(&freed);
                    thread::spawn(move || destroy_with_the_gap(&table, name, &freed))
                })
                .collect();
            for h in handles {
                h.join().unwrap();
            }

            let freed = *freed.lock().unwrap();
            assert!(freed >= 1, "somebody must reclaim the region");
            if freed == 2 {
                DOUBLE_FREE.mark();
            }
        });

        DOUBLE_FREE.assert(
            "an execution in which both callers freed the same run, \
             which is the bug pull request #316 fixed and this model must be able to see",
        );
    }

    /// **A retype never hands out a page the reclaim is about to free.**
    ///
    /// The other half of why the slot comes out first. Before the fix, the removal happened after
    /// the free loop, so a `retype_page` landing in the gap resolved the name and returned a page
    /// that was already on its way back to the allocator. Claiming and removing in one borrow makes
    /// the two outcomes exclusive: whichever wins, the loser gets `None`.
    #[test]
    fn a_retype_never_hands_out_a_page_the_reclaim_is_about_to_free() {
        static RETYPE_WON: Reached = Reached::new();
        static CLAIM_WON: Reached = Reached::new();

        loom::model(|| {
            let table = Arc::new(Mutex::new(RegionTable::<2>::new()));
            let name = table.lock().unwrap().insert_root(0x2000, 4).unwrap();
            let outcome = Arc::new(Mutex::new((None::<u64>, false)));

            let retyper = {
                let table = Arc::clone(&table);
                let outcome = Arc::clone(&outcome);
                thread::spawn(move || {
                    let page = table.lock().unwrap().retype_page(name);
                    outcome.lock().unwrap().0 = page;
                })
            };
            let destroyer = {
                let table = Arc::clone(&table);
                let outcome = Arc::clone(&outcome);
                thread::spawn(move || {
                    let claim = table.lock().unwrap().claim_for_destroy(name);
                    if let Some(claim) = claim {
                        assert_eq!(claim.pages(), 4);
                        outcome.lock().unwrap().1 = true;
                    }
                })
            };
            retyper.join().unwrap();
            destroyer.join().unwrap();

            let (page, claimed) = *outcome.lock().unwrap();
            // The claim is unconditional here (nothing pins the region and it has no children), so
            // the only question is which side of the removal the retype landed on.
            assert!(claimed, "an unpinned childless region must be reclaimable");
            match page {
                Some(p) => {
                    assert_eq!(p, 0x2000, "the retype ran first, on a live region");
                    RETYPE_WON.mark();
                }
                None => CLAIM_WON.mark(),
            }
        });

        RETYPE_WON.assert("an execution in which the retype ran before the claim");
        CLAIM_WON.assert("an execution in which the claim killed the name first");
    }

    /// **A split never carves from a region another caller has claimed.**
    ///
    /// The same exclusion one level along, and it matters more than the retype case because a child
    /// that outlived its parent's reclamation would hold a name for pages already back in the
    /// allocator. Exactly one of the two can succeed.
    #[test]
    fn a_split_and_a_claim_on_one_parent_cannot_both_succeed() {
        static SPLIT_WON: Reached = Reached::new();
        static CLAIM_WON: Reached = Reached::new();

        loom::model(|| {
            let table = Arc::new(Mutex::new(RegionTable::<4>::new()));
            let parent = table.lock().unwrap().insert_root(0x3000, 8).unwrap();
            let outcome = Arc::new(Mutex::new((None::<u64>, false)));

            let splitter = {
                let table = Arc::clone(&table);
                let outcome = Arc::clone(&outcome);
                thread::spawn(move || {
                    let child = table.lock().unwrap().split(parent, 2);
                    outcome.lock().unwrap().0 = child;
                })
            };
            let destroyer = {
                let table = Arc::clone(&table);
                let outcome = Arc::clone(&outcome);
                thread::spawn(move || {
                    let claim = table.lock().unwrap().claim_for_destroy(parent);
                    if let Some(claim) = claim {
                        assert!(claim.is_root());
                        outcome.lock().unwrap().1 = true;
                    }
                })
            };
            splitter.join().unwrap();
            destroyer.join().unwrap();

            let (child, claimed) = *outcome.lock().unwrap();
            assert!(
                !(child.is_some() && claimed),
                "a claimed parent must not also have been carved: the child would name freed pages"
            );
            if child.is_some() {
                SPLIT_WON.mark();
            }
            if claimed {
                CLAIM_WON.mark();
            }
        });

        SPLIT_WON.assert("an execution in which the split ran first and the claim was refused");
        CLAIM_WON.assert("an execution in which the claim ran first and the split found no parent");
    }

    /// **A parent is never reclaimed while a child's pages are still in flight.**
    ///
    /// The child's slot comes out at its claim, but the parent's child count drops only in
    /// `return_to_parent`, after the caller has revoked the run. Those two facts together are what
    /// keep the parent refusing across the whole window in which the child's pages are neither the
    /// child's nor yet the parent's.
    ///
    /// The flag lives under the same lock as the table on purpose. A separate lock would let the
    /// parent's claim land between the return and the flag's store, and the harness would be
    /// checking its own bookkeeping rather than the protocol.
    #[test]
    fn a_parent_is_never_reclaimed_while_its_child_is_returning() {
        static PARENT_LOST: Reached = Reached::new();
        static PARENT_WON: Reached = Reached::new();

        loom::model(|| {
            // `.1` is "the child has finished returning its pages", written under the table's lock.
            let shared = Arc::new(Mutex::new((RegionTable::<4>::new(), false)));
            let (parent, child) = {
                let mut guard = shared.lock().unwrap();
                let p = guard.0.insert_root(0x4000, 8).unwrap();
                let c = guard.0.split(p, 4).unwrap();
                (p, c)
            };

            let reaper = {
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    let claim = shared.lock().unwrap().0.claim_for_destroy(child).unwrap();
                    // The kernel revokes the child's run here, lock released.
                    thread::yield_now();
                    let mut guard = shared.lock().unwrap();
                    guard.0.return_to_parent(&claim);
                    guard.1 = true;
                })
            };
            let owner = {
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    let mut guard = shared.lock().unwrap();
                    if guard.0.claim_for_destroy(parent).is_some() {
                        assert!(
                            guard.1,
                            "the parent was reclaimed while its child's pages were in flight"
                        );
                        true
                    } else {
                        false
                    }
                })
            };
            reaper.join().unwrap();
            if owner.join().unwrap() {
                PARENT_WON.mark();
            } else {
                PARENT_LOST.mark();
            }
        });

        PARENT_LOST.assert("an execution in which the parent's claim was refused");
        PARENT_WON.assert("an execution in which the parent's claim ran after the child returned");
    }
}
