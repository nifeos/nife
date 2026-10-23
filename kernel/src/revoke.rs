//! Capability revocation and untyped reclamation: the mapping log.
//!
//! Every mapping of an untyped-derived page is recorded with the capability it was made under, in
//! log pages retyped from the mapping space's own region. Reclamation (`memory_region::destroy`)
//! unmaps a page from every space, whoever mapped it; `PageFrame::REVOKE` unmaps only what the
//! revoked capability's derivation family mapped. Semantics are in
//! design/decisions/0013-frame-revocation.md and
//! design/decisions/0132-overlapping-page-frame-runs.md; the design is notes/mapping-log.md.

use crate::arch::mmu;
use crate::sync::{IrqSafeGuard, IrqSafeMutex, rank};

/// One recorded mapping: `va` in the owning space maps `phys`, under the capability whose run
/// starts at `object`. `phys == 0` is a tombstone (no real frame is 0). The run's base names a
/// capability and all its derivatives, because `Cap::derive` never changes the object; a mapping
/// made with no capability records the page itself (`object == phys`).
///
/// # BUGS
///
/// Two capabilities sharing a base but not a length are one object here, so a revoke of the
/// shorter unmaps the longer one's mappings of the pages they share. See notes/mapping-log.md.
#[repr(C)]
#[derive(Clone, Copy)]
struct LogEntry {
    phys: u64,
    va: u64,
    /// The base address of the run named by the capability this mapping was made under, or
    /// [`TABLE`] for a page table rather than a mapping.
    object: u64,
}

/// The `object` of a record that is a page table, not a mapping: `(table, va, TABLE)`, the
/// table's own page and the address whose map built it. [`revoke_region`] cuts such a table out
/// of the space's walk before the region that paid for it goes back.
///
/// A real object is a page address, page aligned and at least `0x4000_0000`, so 1 can never be
/// one. Every reader of a record must ask [`LogEntry::is_table`]; a reader that forgot would
/// treat a table as a mapped page. See notes/mapping-log.md.
///
/// Name: provisional.
const TABLE: u64 = 1;

impl LogEntry {
    /// A page-table record ([`TABLE`]) rather than a mapping. A tombstone is neither.
    fn is_table(&self) -> bool {
        self.phys != 0 && self.object == TABLE
    }

    /// A mapping record: live, and not a table.
    fn is_mapping(&self) -> bool {
        self.phys != 0 && self.object != TABLE
    }
}

/// How many entries fit a log page after its header: `16 + 24 * 170 == 4096`, which the `LogPage`
/// size assertion holds.
///
/// A benchmark input: [`MappingHold::record_mapping`] scans the head page for a tombstone before it
/// appends, so recording a mapping costs time linear in this, and `map_el0` and `spawn_el0` move
/// with it. notes/mapping-log.md has the measurements.
const LOG_ENTRIES: usize = 170;

/// How many log pages `n` recorded mappings cost, for a caller sizing an address space's backing
/// region. Every mapping is recorded and the space's own region pays, so a caller that maps a large
/// window (a whole initial ramdisk) must budget for its records as well as its page tables.
pub fn log_pages_for(n: u64) -> u64 {
    n.div_ceil(LOG_ENTRIES as u64)
}

/// What authority a mapping was made under, which [`MappingHold::record_mapping`] requires (see
/// design/decisions/0132-overlapping-page-frame-runs.md). An enum rather than a bare address so
/// every call site states the case: most mapping sites map a page no capability names.
///
/// Name: ratified 2026-08-27 (calef). Landed as `MappedUnder`; renamed to `PageMapSource` on
/// request, since "mapping" alone is overloaded across this tree (page mappings, capability
/// derivation, filesystem grants) and this tree already has an established, narrower term for
/// exactly this concept: `crates/pmap`, "page map." `Source` matches this tree's existing pattern
/// for a small enum naming where something came from (`grant_plan::Source`).
#[derive(Clone, Copy)]
pub enum PageMapSource {
    /// The capability whose object begins at this physical address, **and every capability derived
    /// from it**: `Cap::derive` narrows rights and never changes the object, so one address names
    /// the whole family. For a `PageFrame` run this is the run's base, not the page being mapped.
    Capability(u64),
    /// No capability names this page. `MemoryRegion::MAP` retypes and maps in one step, and the
    /// kernel's own loaders map pages straight into a space they are building; in both cases there
    /// is no derivation family for a revoke to be scoped to, so the page stands as its own object.
    NoCapability,
}

impl PageMapSource {
    /// The object address to file with the record: the run's base, or the mapped page itself when
    /// nothing names it.
    fn object(self, phys: u64) -> u64 {
        match self {
            PageMapSource::Capability(base) => base,
            PageMapSource::NoCapability => phys,
        }
    }
}

/// A live address space, as revocation sees it: where its tables root, which region pays for its
/// records, its TLB tag, and its chain of log pages.
struct SpaceLog {
    root: u64,
    region: u64,
    /// The space's TLB tag, which [`revoke_region`] flushes after cutting a table out of it: a cut
    /// takes a whole span, and only a flush by tag reaches every page and every cached walk in it.
    asid: u16,
    log: chain::LogChain,
}

impl SpaceLog {
    /// Tombstone every mapping record whose address lies in `[base, base + span)`: what hung
    /// beneath a table [`revoke_region`] has just cut, which no longer translates and must not go on
    /// being listed or revoked. Table records in the span are left alone: one that was beneath the
    /// cut is unreachable and its own region's destroy drops it, and one above it is still linked
    /// and still owed its own cut.
    fn forget_mappings_within(&mut self, base: u64, span: u64) {
        for (_, page) in self.log.pages_mut() {
            for e in page.used_mut() {
                if e.is_mapping() && e.va >= base && e.va - base < span {
                    e.phys = 0;
                }
            }
        }
    }
}

/// A space's chain of log pages, and the one place in the kernel that turns a physical address
/// into a log page. Names provisional.
///
/// - The lock is held by a borrow: every walk takes `&mut LogChain`, and a `LogChain` lives only
///   in a [`SpaceLog`] inside `SPACES`, from [`register_space`] until [`forget_root`], so a
///   mutable borrow of one is a borrow of the held registry.
/// - Membership is held by privacy: `head`, a page's `next` and [`chain::Pages`]' cursor are
///   private, and [`chain::LogChain::grow`], their only writer, links only a page `retype_page`
///   has just handed this log. A caller-chosen address (`LIST`'s cursor) must be found on the
///   chain by walking.
mod chain {
    use super::{LOG_ENTRIES, LogEntry, mmu};

    /// One page of mapping records, retyped from the owning space's region. Exactly one frame.
    #[repr(C)]
    pub(super) struct LogPage {
        /// Physical address of the next (older) log page in this space's chain; 0 ends it.
        /// Private: [`LogChain::grow`] is its only writer.
        next: u64,
        /// High-water mark of entries ever written here. Slots below it may be tombstones.
        pub(super) used: u64,
        pub(super) entries: [LogEntry; LOG_ENTRIES],
    }

    const _: () = assert!(size_of::<LogPage>() == page_frames::FRAME_SIZE as usize);

    impl LogPage {
        /// Every slot this page has ever written, tombstones included. A slice rather than an
        /// iterator chained across pages, because a `flat_map` over the chain is measurably slower on
        /// `map_el0` and `spawn_el0` (notes/mapping-log.md).
        pub(super) fn used_mut(&mut self) -> &mut [LogEntry] {
            let used = (self.used as usize).min(LOG_ENTRIES);
            &mut self.entries[..used]
        }
    }

    /// The newest log page's physical address; 0 until the first record needs one. Not `Clone`.
    pub(super) struct LogChain {
        head: u64,
    }

    impl LogChain {
        /// A chain with no pages, for a space that has recorded nothing yet.
        pub(super) const EMPTY: Self = Self { head: 0 };

        /// This chain's pages, newest first, each with its physical address.
        pub(super) fn pages_mut(&mut self) -> Pages<'_> {
            Pages {
                next: self.head,
                _chain: core::marker::PhantomData,
            }
        }

        /// Link a fresh page from `region` as the new head, holding `entry`. `false` when the region
        /// is spent: the caller unmaps, and the process pays for its own limit.
        ///
        /// The only writer of `head` and of any page's `next`: every address on the chain is a page
        /// `retype_page` handed this log, and the chain is acyclic because a page is prepended once.
        ///
        /// Context: under the `SPACES` borrow this chain lives in; takes `MEMORY_REGION` to retype.
        pub(super) fn grow(&mut self, region: u64, entry: LogEntry) -> bool {
            let Some(fresh) = crate::memory_region::retype_page(region) else {
                return false;
            };
            let older = self.head;
            self.head = fresh;
            // Retyped zeroed, so `used` and `next` start at 0 and the walk stops at the fresh page.
            let Some((_, page)) = self.pages_mut().next() else {
                unreachable!("the head was set to a nonzero frame one line up");
            };
            page.next = older;
            page.entries[0] = entry;
            page.used = 1;
            true
        }
    }

    /// One walk of a chain, from [`LogChain::pages_mut`].
    pub(super) struct Pages<'a> {
        next: u64,
        _chain: core::marker::PhantomData<&'a mut LogChain>,
    }

    impl<'a> Iterator for Pages<'a> {
        type Item = (u64, &'a mut LogPage);

        fn next(&mut self) -> Option<Self::Item> {
            let phys = self.next;
            if phys == 0 {
                return None;
            }
            // SAFETY: `phys` is `head` or a `next` read from a page on this chain, and only
            // `LogChain::grow` writes either, always with a page `retype_page` just handed this
            // log exclusively; the direct map names every RAM page. The `&mut LogChain` this walk
            // borrows is a borrow of the held `SPACES` registry, so no other walk of this chain
            // runs at once. Each page is yielded at most once (the chain is acyclic: pages are only
            // ever prepended fresh), so no two `&mut` this walk yields alias, and `next` is read
            // here before the page is handed out, so a caller cannot redirect the walk.
            let page = unsafe { &mut *(mmu::phys_to_virt(phys) as *mut LogPage) };
            self.next = page.next;
            Some((phys, page))
        }
    }
}

/// The most concurrently-live address spaces the registry can track: every user thread has one
/// (bounded by [`crate::sched::MAX_THREADS`]), plus 32 for the tests' bare `AddressSpace`s.
/// `user::MAX_USER_SPACES` is sized to it.
pub(crate) const MAX_SPACES: usize = crate::sched::MAX_THREADS + 32;

/// The registry of live address spaces: the fixed index that finds each space's log, whose
/// records live in the spaces' own regions.
static SPACES: IrqSafeMutex<Registry> = IrqSafeMutex::new(rank::MAPPINGS, Registry::new());

/// The registry array plus one past its highest live slot, so a walk costs the occupied prefix
/// rather than [`MAX_SPACES`], which grows with `sched::MAX_THREADS` (notes/mapping-log.md has
/// the measurement). `claim` is first-fit, so `top` stays near the live count, and every slot at
/// or above `top` is `None`, which makes a bounded walk identical to a full one.
struct Registry {
    spaces: [Option<SpaceLog>; MAX_SPACES],
    top: usize,
}

impl Registry {
    const fn new() -> Self {
        Self {
            spaces: [const { None }; MAX_SPACES],
            top: 0,
        }
    }

    /// Every live space. The `flatten` still skips holes; the bound is what stops the walk from
    /// visiting slots that have never been occupied at all. Mutable only, because every walk of a
    /// space's log goes through `&mut` ([`chain::LogChain::pages_mut`]), readers included.
    fn live_mut(&mut self) -> impl Iterator<Item = &mut SpaceLog> + '_ {
        self.spaces[..self.top].iter_mut().flatten()
    }

    /// Take the first free slot, growing the bound if it is past the end of the occupied prefix.
    /// `false` (and nothing stored) when the registry is full, which is `register_space`'s own
    /// "the caller should fail creation" answer.
    fn claim(&mut self, log: SpaceLog) -> bool {
        let Some(slot) = self.spaces.iter().position(|s| s.is_none()) else {
            return false;
        };
        self.spaces[slot] = Some(log);
        if slot >= self.top {
            self.top = slot + 1;
        }
        true
    }

    /// Drop every entry naming `root` and bring the bound back down over whatever emptiness that
    /// left at the top. The downward walk stops at the first live slot and can only step over a
    /// slot that was filled and freed, so it is amortised O(1).
    fn release(&mut self, root: u64) {
        for slot in self.spaces[..self.top].iter_mut() {
            if slot.as_ref().is_some_and(|s| s.root == root) {
                *slot = None;
            }
        }
        // Bounded by the array's length so a prover sees the bound; the `break` keeps it amortised O(1).
        let mut top = self.top;
        for _ in 0..MAX_SPACES {
            if top == 0 || self.spaces[top - 1].is_some() {
                break;
            }
            top -= 1;
        }
        self.top = top;
    }
}

/// Enter a newly created address space into the registry, with the TLB tag it runs under.
/// `false` (and the caller should fail creation) if the registry is full.
///
/// Context: takes `SPACES` (`rank::MAPPINGS`).
pub(crate) fn register_space(root: u64, region: u64, asid: u16) -> bool {
    let mut spaces = SPACES.lock();
    spaces.claim(SpaceLog {
        root,
        region,
        asid,
        log: chain::LogChain::EMPTY,
    })
}

/// Forget an address space. `AddressSpace::drop` calls this before its region is destroyed: a
/// stale entry would send a later revoke walking page tables and log pages that belong to someone
/// else. The log pages need no cleanup; they come back with the region.
///
/// Context: takes `SPACES` (`rank::MAPPINGS`).
pub(crate) fn forget_root(root: u64) {
    let mut spaces = SPACES.lock();
    spaces.release(root);
}

/// The mapping registry, held: the one critical section a mapping is read, made and recorded in
/// (name provisional). A capability's frame is mapped and recorded under the hold its capability
/// was read under ([`MappingHold::current_cap`]); a sweep deletes capabilities before it takes
/// this registry, so it either precedes the read (`NoSuchSlot`) or its unmap pass finds the
/// record. See notes/mapping-log.md.
///
/// # BUGS
///
/// A caller can still read with `sched::current_cap` outside a hold and map with it.
pub struct MappingHold {
    spaces: IrqSafeGuard<'static, Registry>,
}

/// Take the mapping registry. See [`MappingHold`]: hold it from the frame read through the record.
///
/// Context: takes `SPACES` (`rank::MAPPINGS`) until the hold drops.
pub(crate) fn hold() -> MappingHold {
    MappingHold {
        spaces: SPACES.lock(),
    }
}

impl MappingHold {
    /// Read the running thread's capability at `slot` under this hold: the read a mapping must be
    /// made from.
    ///
    /// Context: `SPACES` held; takes the running thread's `CAPABILITY_TABLE` lock beneath it.
    pub fn current_cap(&self, slot: u64) -> Result<crate::cap::Cap, crate::cap::Error> {
        crate::sched::current_cap(slot)
    }

    /// Record that the space rooted at `root` maps `phys` at `va` under `under`, paid for by that
    /// space's own region. `under` has no default, so a caller must say which capability made the
    /// mapping ([`PageMapSource`]).
    ///
    /// Returns `false` if the space is unknown or its budget is exhausted. The caller must then unmap
    /// what it just mapped: an unrecorded mapping is invisible to revocation.
    ///
    /// Context: `SPACES` held; may take `MEMORY_REGION` to retype a log page.
    #[must_use]
    pub(crate) fn record_mapping(
        &mut self,
        phys: u64,
        root: u64,
        va: u64,
        under: PageMapSource,
    ) -> bool {
        let object = under.object(phys);
        // The enum stops a caller swapping the two arguments; this catches a wrongly computed base,
        // which would scope the record to a family it does not belong to.
        debug_assert!(
            object <= phys && (phys - object).is_multiple_of(page_frames::FRAME_SIZE),
            "a mapping of {phys:#x} recorded under an object at {object:#x}: not a page of that run",
        );
        self.file(root, LogEntry { phys, va, object })
    }

    /// Retype a page table out of `region` for the space rooted at `root`, and record it as a
    /// [`TABLE`] before the mapper links it. Name provisional.
    ///
    /// Recording before linking, both under this hold, means [`revoke_region`]'s cut pass runs either
    /// before the retype (the region is claimed and the retype fails) or after the link (and the
    /// record finds it). `None` when the region is spent or the space cannot afford the record; in the
    /// second case the retyped page stays spent until the region is destroyed.
    ///
    /// Context: `SPACES` held; takes `MEMORY_REGION` to retype.
    pub(crate) fn retype_table(&mut self, region: u64, root: u64, va: u64) -> Option<u64> {
        let table = crate::memory_region::retype_page(region)?;
        self.file(
            root,
            LogEntry {
                phys: table,
                va,
                object: TABLE,
            },
        )
        .then_some(table)
    }

    /// File `entry` in `root`'s log, paid for by that space's own region: an existing free slot,
    /// or a fresh log page retyped from the region. `false` if the space is unknown or its budget
    /// is exhausted.
    ///
    /// Context: `SPACES` held; may take `MEMORY_REGION`.
    fn file(&mut self, root: u64, entry: LogEntry) -> bool {
        let Some(space) = self.spaces.live_mut().find(|s| s.root == root) else {
            return false;
        };

        for (_, page) in space.log.pages_mut() {
            if let Some(e) = page.used_mut().iter_mut().find(|e| e.phys == 0) {
                *e = entry;
                return true;
            }
            let used = page.used as usize;
            if used < LOG_ENTRIES {
                page.entries[used] = entry;
                page.used += 1;
                return true;
            }
        }

        space.log.grow(space.region, entry)
    }

    /// Undo one [`Self::record_mapping`]: tombstone the record that `root` maps `phys` at `va`, and
    /// no other. A rollback of a half-finished `PageFrame::MAP` uses this rather than
    /// [`unmap_everywhere`], which would pull a shared frame out of every peer's space. Silent when
    /// the space or the record is unknown: there is nothing left to undo.
    ///
    /// Context: `SPACES` held.
    pub(crate) fn forget_mapping(&mut self, phys: u64, root: u64, va: u64) {
        let Some(space) = self.spaces.live_mut().find(|s| s.root == root) else {
            return;
        };
        for (_, page) in space.log.pages_mut() {
            for e in page.used_mut() {
                if e.is_mapping() && e.phys == phys && e.va == va {
                    e.phys = 0; // tombstone: reusable by the next record, exactly as a revoke leaves it
                    return;
                }
            }
        }
    }

    /// Tombstone the record that `root` maps a page at `va`, for `AddressSpace::UNMAP`, and return
    /// the page it named; `None` if no record does. Keyed by address, because the caller may hold no
    /// capability for the page (design/decisions/0162-giving-up-a-mapping.md). Required: a stale
    /// record would let a revoke of the old frame unmap a new frame at `va`
    /// (`kernel::user::unmap_tests::an_unmapped_va_remapped_to_another_frame_survives_the_old_frames_revoke`).
    /// A table record at `va` stays (`notes/unmap.md`'s `BUGS`).
    ///
    /// Context: `SPACES` held.
    pub(crate) fn forget_mapping_at(&mut self, root: u64, va: u64) -> Option<u64> {
        let space = self.spaces.live_mut().find(|s| s.root == root)?;
        for (_, page) in space.log.pages_mut() {
            for e in page.used_mut() {
                if e.is_mapping() && e.va == va {
                    let phys = e.phys;
                    e.phys = 0; // tombstone, exactly as a revoke leaves it
                    return Some(phys);
                }
            }
        }
        None
    }
}

/// What one [`list_mapping`] call found: one entry to show, the end of the listing, or a cursor
/// this space's own log never minted, which the syscall layer refuses rather than walks.
///
/// Name: provisional (the lane for milestone 779 (fuzz the surface a confined process can
/// reach), 2026-10-06 UTC); calef names public types.
pub enum Listing {
    /// One mapping: the cursor to resume with, and its `va`.
    Entry(u64, u64),
    /// The listing is exhausted, or the space is gone (a race with teardown, or a stale cursor
    /// from a caller that kept one past the space's life): nothing to report. Not a refusal;
    /// the syscall layer already checked the capability before calling here.
    Done,
    /// The cursor names a log page this space's chain never held. The syscall layer refuses it
    /// rather than walking it.
    ForeignCursor,
}

/// One entry of what `root` has mapped, resuming from `cursor`, for `abi::address_space::LIST`
/// (design/decisions/0114-aspace-enumerate.md). Start at 0 and feed each returned cursor back. A
/// cursor is a log page's address OR'd with an index, checked for membership in this space's
/// chain before anything follows it. A slot reused between calls can show the wrong mapping
/// (`crates/pmap`'s `BUGS`). See notes/mapping-log.md.
///
/// Context: takes `SPACES` across the lookup, the membership check and the walk, so the space
/// cannot die between the check and the follow.
pub fn list_mapping(root: u64, cursor: u64) -> Listing {
    let mut spaces = SPACES.lock();
    let Some(space) = spaces.live_mut().find(|s| s.root == root) else {
        return Listing::Done;
    };

    const PAGE_MASK: u64 = !(page_frames::FRAME_SIZE - 1);
    let mut chain = space.log.pages_mut().peekable();
    let mut index = 0usize;
    if cursor != 0 {
        // The caller's word: find its page on this space's chain, walking from the head. The listing
        // resumes from the page the check stopped on, so every page it follows is this chain's own.
        let want = cursor & PAGE_MASK;
        while chain.next_if(|(phys, _)| *phys != want).is_some() {}
        if chain.peek().is_none() {
            return Listing::ForeignCursor;
        }
        index = (cursor & !PAGE_MASK) as usize;
    }

    for (page_phys, page) in chain {
        while index < page.used as usize {
            let entry = page.entries[index];
            index += 1;
            // A table record is not something the space maps, so `LIST` never shows one.
            if entry.is_mapping() {
                // `page_phys` is never 0, so `page_phys | index` resumes correctly even when `index` has
                // walked off the end of this page: the next call falls through to `Listing::Done`.
                return Listing::Entry(page_phys | index as u64, entry.va);
            }
        }
        index = 0;
    }
    Listing::Done
}

/// Unmap `phys` from every address space whose log records it, except `spare`'s (0 spares none),
/// tombstoning the records. Object-blind on purpose: its callers are reclamation
/// ([`revoke_region`]), where any surviving mapping is a use-after-free, and the device take-back
/// ([`revoke_device_from_others`]), which scopes by holder. Capability scope is
/// [`unmap_under_object`].
///
/// Context: takes `SPACES`; the unmaps and their TLB invalidation run under it.
fn unmap_everywhere(phys: u64, spare: u64) {
    unmap_matching(phys, spare, None);
}

/// Unmap `phys` only from the mappings made under the capability whose run begins at `object`
/// and its derivatives, leaving every other holder's mapping of the page alone: `PageFrame::REVOKE`'s
/// unmap half (design/decisions/0132-overlapping-page-frame-runs.md).
///
/// Context: takes `SPACES`; the unmaps and their TLB invalidation run under it.
fn unmap_under_object(phys: u64, object: u64) {
    unmap_matching(phys, 0, Some(object));
}

/// The body both sweeps share: unmap `phys` from every recorded mapping except those in `spare`'s
/// space and, when `object` is `Some`, those made under a different capability. One body keeps
/// the locking, the tombstoning and the TLB invalidation the same for both.
///
/// Context: takes `SPACES`; the unmaps and their TLB invalidation run under it.
fn unmap_matching(phys: u64, spare: u64, object: Option<u64>) {
    let mut spaces = SPACES.lock();
    for space in spaces.live_mut() {
        let root = space.root;
        if root == spare {
            continue;
        }
        for (_, page) in space.log.pages_mut() {
            for e in page.used_mut() {
                if e.is_mapping() && e.phys == phys && object.is_none_or(|o| e.object == o) {
                    mmu::unmap_user_at(root, e.va);
                    e.phys = 0; // tombstone: reusable by the next record
                }
            }
        }
    }
}

/// Revoke a single-page frame: [`revoke_page_frame_run`] with a count of 1. Only an exact
/// `PageFrame(phys, 1)` capability is deleted, so a longer run containing `phys` survives, and
/// only mappings made under that capability are unmapped. Reclamation uses [`revoke_region`]
/// instead. Test-only outside an `initial_ramdisk` build; kept as the one-page primitive the
/// revocation tests drive.
///
/// Context: as [`revoke_page_frame_run`].
#[cfg_attr(
    not(all(any(test, feature = "system_tests"), initial_ramdisk)),
    allow(dead_code)
)]
pub fn revoke_page_frame(phys: u64) {
    revoke_page_frame_run(phys, 1);
}

/// Revoke a run of `count` frames, `PageFrame::REVOKE`'s body: delete every capability naming
/// exactly `(phys, count)`, then unmap each page from the mappings made under that run
/// (design/decisions/0132-overlapping-page-frame-runs.md).
///
/// # BUGS
///
/// An overlapping capability keeps its authority and mappings, and a device keeps its DMA window
/// (§132's question 3). See notes/mapping-log.md.
///
/// Context: takes `IPC_TABLES` (each `CAPABILITY_TABLE` beneath it) for the capability pass and
/// releases it, then `SPACES` once per page for the unmap pass.
pub(crate) fn revoke_page_frame_run(phys: u64, count: u64) {
    crate::sched::delete_page_frame_caps(phys, count);
    for k in 0..count {
        unmap_under_object(phys + k * page_frames::FRAME_SIZE, phys);
    }
}

/// Take a device's registers back from everyone else: delete every `DeviceFrame` capability
/// naming `phys` except the invoker's, then unmap `phys` from every space except the invoker's.
/// Afterwards exactly one process can reach the device. Scoped by holder rather than by
/// capability, so a second capability to the same MMIO page does not survive (milestone 23 (a
/// capability-routed component OS with live replacement); notes/mapping-log.md).
///
/// Context: takes `IPC_TABLES` for the capability pass and releases it, then `SPACES`.
pub fn revoke_device_from_others(phys: u64) {
    crate::sched::delete_device_frame_caps_from_others(phys);
    // The invoker is the current thread, so its space is the one installed in TTBR0 (satp, CR3).
    unmap_everywhere(phys, mmu::current_user_root());
}

/// Take an x86 port range back from every other thread: `PortRange::REVOKE`'s body, and the port
/// analogue of [`revoke_device_from_others`]. A port has no page to unmap, so this is capability
/// deletion and forgetting the cached grant; the TSS reset that follows makes the revoked holder
/// fault on its next `in`/`out`. `x86_64` only.
#[cfg(target_arch = "x86_64")]
pub fn revoke_port_range_from_others(base: u16, count: u16) {
    crate::sched::delete_port_range_caps_from_others(base, count);
}

/// Take an x86 port range back from everyone, the invoker included, for the kernel's port tests.
/// Test-only, like [`revoke_page_frame`]. `x86_64` only.
#[cfg(target_arch = "x86_64")]
#[cfg_attr(
    not(all(any(test, feature = "system_tests"), initial_ramdisk)),
    allow(dead_code)
)]
pub fn revoke_port_range(base: u16, count: u16) {
    crate::sched::delete_port_range_caps(base, count);
}

/// Revoke every page in `[base, base + size)` before `memory_region::destroy` returns them: delete
/// every overlapping `PageFrame` capability, unmap every recorded page in range, and cut out each
/// page table the region paid for ([`TABLE`]). See notes/mapping-log.md.
///
/// # BUGS
///
/// A space's root is a page table too, and nothing here cuts it (milestone 765 (a destroyed region
/// cannot free the root a running thread walks)).
///
/// Context: takes `IPC_TABLES` for the capability pass and releases it, then `SPACES` for each
/// scan and cut. The caller holds neither, nor `MEMORY_REGION`.
pub fn revoke_region(base: u64, size: u64) {
    crate::sched::delete_page_frame_caps_overlapping(base, size);
    // One scan finds either kind of record, so the common case costs one pass over the logs. Order
    // between kinds does not matter: a leaf under a table cut first is tombstoned with the cut.
    loop {
        let spaces = &mut *SPACES.lock();
        let mut table = None;
        for space in spaces.live_mut() {
            let (root, asid) = (space.root, space.asid);
            let log = &mut space.log;
            mmu::unmap_user_pages(root, asid, |unmap| {
                for (_, page) in log.pages_mut() {
                    for e in page.used_mut() {
                        if e.phys < base || e.phys >= base + size {
                            continue;
                        }
                        if !e.is_table() {
                            unmap(e.va);
                            e.phys = 0;
                        } else if table.is_none() {
                            table = Some((root, asid, e.va, e.phys));
                            e.phys = 0;
                        }
                    }
                }
            });
            if table.is_some() {
                break;
            }
        }
        let Some((root, asid, va, table)) = table else {
            break;
        };
        // Cut under the registry, so no `MAP` is halfway down this walk.
        if let Some((cut, span)) = mmu::cut_user_table(root, va, table, asid)
            && let Some(space) = spaces.live_mut().find(|s| s.root == root)
        {
            space.forget_mappings_within(cut, span);
        }
    }
}

#[cfg(test)]
mod tests {
    use paging::Flags;

    use super::*;
    use crate::cap::{Rights, page_frame_run_cap, page_frame_run_len};
    use crate::user::AddressSpace;

    /// `N` consecutive pages out of one region. A region is a contiguous span and `retype_page`
    /// bumps through it, so this holds; it is asserted rather than assumed because the run tests
    /// below are meaningless if it ever stops holding.
    fn consecutive<const N: usize>(region: u64) -> [u64; N] {
        let mut pages = [0u64; N];
        for (k, page) in pages.iter_mut().enumerate() {
            *page = crate::memory_region::retype_page(region)
                .unwrap_or_else(|| panic!("region ran out at page {k} of {N}"));
        }
        for k in 1..N {
            assert_eq!(
                pages[k],
                pages[k - 1] + page_frames::FRAME_SIZE,
                "a region stopped retyping contiguously at page {k}; a PageFrame run has no \
                 meaning without that",
            );
        }
        pages
    }

    /// A multi-page `REVOKE` unmaps every page of the run in every space and deletes the capability
    /// that named it (design/decisions/0102-frame-names-a-run.md). Two spaces hold different pages
    /// of one three-page run, so a wrong loop bound in either direction fails.
    #[test_case]
    fn revoking_a_run_unmaps_every_page_of_it_everywhere() {
        let mut a = AddressSpace::new(2).expect("no space A");
        let mut b = AddressSpace::new(2).expect("no space B");
        let region = crate::memory_region::create(4).expect("no region");
        let run = consecutive::<3>(region);

        let (va_a, va_b) = (0x40_0000u64, 0x80_0000u64);
        for (k, phys) in run[..2].iter().enumerate() {
            let va = va_a + k as u64 * page_frames::FRAME_SIZE;
            a.map_physical(
                va,
                *phys,
                Flags::user_data(),
                PageMapSource::Capability(run[0]),
            )
            .expect("map A");
        }
        b.map_physical(
            va_b,
            run[2],
            Flags::user_rodata(),
            PageMapSource::Capability(run[0]),
        )
        .expect("map B");

        let slot = crate::sched::grant(page_frame_run_cap(
            run[0],
            page_frame_run_len(3),
            Rights::ALL,
        ))
        .expect("grant the run");

        revoke_page_frame_run(run[0], 3);

        for k in 0..2u64 {
            assert!(
                mmu::translate_at(a.root(), va_a + k * page_frames::FRAME_SIZE).is_none(),
                "page {k} of the run survived the revoke in A",
            );
        }
        assert!(
            mmu::translate_at(b.root(), va_b).is_none(),
            "the run's last page survived the revoke in B",
        );
        assert!(
            crate::sched::current_cap(slot).is_err(),
            "REVOKE left the capability that named the run in the revoker's own table",
        );

        crate::memory_region::destroy(region);
    }

    /// `REVOKE` takes back what was mapped under the capability invoked, and nothing else
    /// (design/decisions/0132-overlapping-page-frame-runs.md, option C). A driver holds
    /// `PageFrame(run[0], 4)` and a client `PageFrame(run[1], 3)`, the display wiring shrunk to four
    /// pages; both map the same physical page. The client's revoke must leave the driver's mapping
    /// and capability alone.
    #[test_case]
    fn revoking_one_run_leaves_an_overlapping_capability_and_its_mappings_alone() {
        let mut driver = AddressSpace::new(2).expect("no driver space");
        let mut client = AddressSpace::new(2).expect("no client space");
        let region = crate::memory_region::create(8).expect("no region");
        let run = consecutive::<4>(region);

        // `Rights::ALL` carries the `GRANT` `PageFrame::REVOKE` requires, which no run capability in the
        // shipping tree has; minting it here is what makes this path reachable.
        let window = crate::sched::grant(page_frame_run_cap(
            run[0],
            page_frame_run_len(4),
            Rights::ALL,
        ))
        .expect("grant the whole window");
        let surface = crate::sched::grant(page_frame_run_cap(
            run[1],
            page_frame_run_len(3),
            Rights::ALL,
        ))
        .expect("grant the inner surface");

        let (va_driver, va_client) = (0x40_0000u64, 0x80_0000u64);
        driver
            .map_physical(
                va_driver,
                run[1],
                Flags::user_data(),
                PageMapSource::Capability(run[0]),
            )
            .expect("map driver");
        client
            .map_physical(
                va_client,
                run[1],
                Flags::user_data(),
                PageMapSource::Capability(run[1]),
            )
            .expect("map client");

        // The client hands its surface back.
        revoke_page_frame_run(run[1], 3);

        assert!(
            mmu::translate_at(client.root(), va_client).is_none(),
            "the revoked capability's own mapping survived the revoke",
        );
        assert!(
            mmu::translate_at(driver.root(), va_driver).is_some(),
            "revoking one capability unmapped a page another capability had mapped: the \
             space-blind unmap §132 replaced",
        );
        assert!(
            crate::sched::current_cap(surface).is_err(),
            "REVOKE left the capability that named the revoked run in the revoker's own table",
        );
        assert!(
            crate::sched::current_cap(window).is_ok(),
            "revoking a run deleted an overlapping capability nobody revoked: §132 option B, \
             refused because a three-page holder must not be able to delete a four-page one",
        );

        crate::memory_region::destroy(region);
    }

    /// A device take-back is scoped by holder, not by capability: two holders map one page under
    /// different objects, and the take-back must still unmap the one it does not spare. Drives
    /// `unmap_everywhere`, because `revoke_device_from_others` reads its spare from the running space;
    /// the swap suite's `LOG_REVOKE_ENFORCED` covers the whole function end to end.
    #[test_case]
    fn a_device_take_back_ignores_the_object_and_spares_one_holder() {
        let mut keeper = AddressSpace::new(2).expect("no keeper space");
        let mut loser = AddressSpace::new(2).expect("no loser space");
        let region = crate::memory_region::create(4).expect("no region");
        let run = consecutive::<2>(region);
        let shared = run[1];

        let (va_keeper, va_loser) = (0x40_0000u64, 0x80_0000u64);
        keeper
            .map_physical(
                va_keeper,
                shared,
                Flags::user_data(),
                PageMapSource::Capability(run[0]),
            )
            .expect("map keeper");
        loser
            .map_physical(
                va_loser,
                shared,
                Flags::user_data(),
                PageMapSource::Capability(shared),
            )
            .expect("map loser");

        unmap_everywhere(shared, keeper.root());

        assert!(
            mmu::translate_at(keeper.root(), va_keeper).is_some(),
            "the take-back unmapped the invoker's own mapping: §41's asymmetry is the point",
        );
        assert!(
            mmu::translate_at(loser.root(), va_loser).is_none(),
            "a holder that recorded its mapping under a different object survived the take-back: \
             the device now has two owners",
        );

        crate::memory_region::destroy(region);
    }

    /// Destroying a region deletes every capability naming its pages: a run capability, which no
    /// exact-object sweep matches, and a single page that was never mapped, which no log-driven sweep
    /// finds. Either surviving would name memory the allocator hands out again.
    #[test_case]
    fn destroying_a_region_deletes_every_capability_naming_it() {
        let mut space = AddressSpace::new(2).expect("no space");
        let region = crate::memory_region::create(4).expect("no region");
        let run = consecutive::<3>(region);
        let unmapped = crate::memory_region::retype_page(region).expect("retype 3");

        let va = 0x40_0000u64;
        space
            .map_physical(
                va,
                run[0],
                Flags::user_data(),
                PageMapSource::Capability(run[0]),
            )
            .expect("map");

        let run_slot = crate::sched::grant(page_frame_run_cap(
            run[0],
            page_frame_run_len(3),
            Rights::ALL,
        ))
        .expect("grant the run");
        let lone_slot = crate::sched::grant(crate::cap::page_frame_cap(unmapped, Rights::ALL))
            .expect("grant the unmapped page");

        crate::memory_region::destroy(region);

        assert!(
            crate::sched::current_cap(run_slot).is_err(),
            "a run capability outlived the reclamation of the pages it names: §13's use-after-free",
        );
        assert!(
            crate::sched::current_cap(lone_slot).is_err(),
            "a capability to a never-mapped page outlived its region: the mapping log cannot see it",
        );
        assert!(
            mmu::translate_at(space.root(), va).is_none(),
            "destroy reclaimed a page a live address space still maps",
        );
    }

    /// Revocation unmaps a shared page from every address space that held it: two spaces map one
    /// page, and after `revoke_page_frame` neither does.
    #[test_case]
    fn revoke_unmaps_a_shared_page_from_every_address_space() {
        let mut a = AddressSpace::new(2).expect("no space A");
        let mut b = AddressSpace::new(2).expect("no space B");
        let shared = crate::memory::alloc().expect("no frame").addr();
        let (va_a, va_b) = (0x40_0000u64, 0x80_0000u64);

        a.map_physical(
            va_a,
            shared,
            Flags::user_data(),
            PageMapSource::NoCapability,
        )
        .expect("map A");
        b.map_physical(
            va_b,
            shared,
            Flags::user_rodata(),
            PageMapSource::NoCapability,
        )
        .expect("map B");

        assert!(
            mmu::translate_at(a.root(), va_a).is_some(),
            "A does not map the page"
        );
        assert!(
            mmu::translate_at(b.root(), va_b).is_some(),
            "B does not map the page"
        );

        revoke_page_frame(shared);

        assert!(
            mmu::translate_at(a.root(), va_a).is_none(),
            "A still maps the revoked page"
        );
        assert!(
            mmu::translate_at(b.root(), va_b).is_none(),
            "B still maps the revoked page"
        );

        crate::memory::free(page_frames::PageFrame::from_addr(shared));
    }

    /// Destroying a region unmaps its pages, then reclaims them: the mapping is gone and the
    /// region's frames come back to the allocator.
    #[test_case]
    fn destroy_unmaps_a_region_before_reclaiming_it() {
        let mut space = AddressSpace::new(2).expect("no space");
        let region = crate::memory_region::create(4).expect("no region");
        let phys = crate::memory_region::retype_page(region).expect("retype");
        let va = 0x40_0000u64;
        space
            .map_physical(va, phys, Flags::user_data(), PageMapSource::NoCapability)
            .expect("map");
        assert!(
            mmu::translate_at(space.root(), va).is_some(),
            "the page was not mapped"
        );

        let free_before = crate::memory::stats().unwrap().free();
        crate::memory_region::destroy(region);
        let free_after = crate::memory::stats().unwrap().free();

        assert!(
            mmu::translate_at(space.root(), va).is_none(),
            "destroy reclaimed a page a live address space still maps: the tripwire's use-after-free",
        );
        assert_eq!(
            free_after,
            free_before + 4,
            "destroy did not return the region's 4 frames to the allocator",
        );
    }

    /// A mapping that cannot be recorded cannot exist: a space with a tiny region maps until its
    /// budget is gone, and the failing map returns `Err` rather than leaving a mapping revocation
    /// would miss.
    #[test_case]
    fn an_exhausted_budget_refuses_the_record_not_the_safety() {
        let mut space = AddressSpace::new(0).expect("no space");
        let shared = crate::memory::alloc().expect("no frame").addr();

        // A 0-content space has about 15 spendable pages, and 4096 mappings need about 16 log pages and
        // 8 table pages, so the budget must run out mid-loop (2048 fits exactly). An exhausted region
        // refuses a table page or a log page, and the `Err` does not say which.
        let mut refused = false;
        for i in 0..4096u64 {
            let va = 0x40_0000 + i * page_frames::FRAME_SIZE;
            if space
                .map_physical(
                    va,
                    shared,
                    Flags::user_rodata(),
                    PageMapSource::NoCapability,
                )
                .is_err()
            {
                refused = true;
                break;
            }
        }
        assert!(
            refused,
            "2048 mappings recorded out of a {}-page region: records are not being paid for",
            crate::memory_region::usage(0).map(|(_, p)| p).unwrap_or(0),
        );

        crate::memory::free(page_frames::PageFrame::from_addr(shared));
    }
}
