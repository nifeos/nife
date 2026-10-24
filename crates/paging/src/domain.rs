//! **The portable DMA-domain seam**, §20 (IOMMU-backed DMA isolation). An IOMMU translates every
//! address a device emits through tables in the CPU's own format (SMMUv3 walks
//! [`Aarch64`](crate::Aarch64), the RISC-V IOMMU walks [`Sv39`](crate::Sv39)), so a DMA domain is a
//! [`Mapper`] holding an identity map, in the low half, over exactly the frames the device may
//! reach. Its leaves are user-accessible and never executable, because a single-stage RISC-V IOMMU
//! faults on a leaf whose U bit is clear. See notes/paging.md, "DMA domains".

use crate::{Flags, Half, MapError, Mapper, PAGE_SIZE, PageFormat, PageTable};

/// A physical region a device is allowed to touch: `[base, base + size)`, 4 KiB aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaRegion {
    /// Physical address of the first byte.
    pub base: u64,
    /// Length in bytes.
    pub size: u64,
    /// **Whether the device may write the region, as well as read it.** Read is always granted:
    /// no format this seam builds can say "write but not read", and a region nobody may read is
    /// not a grant. A required field with no default, so every caller says which it means. Name
    /// provisional.
    pub writable: bool,
}

impl DmaRegion {
    /// The leaf flags a page of this region is mapped with: [`Flags::user_data`] when the device
    /// may write, [`Flags::user_rodata`] when it may only read. Both set the U bit, which the
    /// RISC-V IOMMU needs (this module's header says why); neither is executable.
    pub const fn flags(&self) -> Flags {
        if self.writable {
            Flags::user_data()
        } else {
            Flags::user_rodata()
        }
    }
}

/// **How many whole 4 KiB pages a grant contains, or why it is not a grant a page-granular domain
/// can express.** The decision half of [`build_identity_domain`], loopless so Kani can quantify it
/// over every region.
///
/// Refuses an unaligned `base` with [`MapError::Misaligned`] (rounding down would map bytes below
/// the grant, rounding up would shift it) and a `base + size` that wraps `u64` with
/// [`MapError::BadRegion`] (such a region has no end). A trailing partial page is not refused and
/// not mapped: the device reaches less than it was granted, never more. See notes/paging.md, "DMA
/// domains".
pub fn grant_pages(r: DmaRegion) -> Result<u64, MapError> {
    if !r.base.is_multiple_of(PAGE_SIZE) {
        return Err(MapError::Misaligned);
    }
    if r.base.checked_add(r.size).is_none() {
        return Err(MapError::BadRegion);
    }
    Ok(r.size / PAGE_SIZE)
}

/// **The IOVA of the `i`th whole page of a grant, which is also its PA (the domain is identity).**
/// `None` if `i` does not name a whole page of this region; every add and multiply is checked, so
/// the enumeration is total for any region and index. Soundness rests on [`grant_pages`] flooring
/// the count, and the whole-page test below is defense in depth (notes/paging.md).
pub fn grant_page(r: DmaRegion, i: u64) -> Option<u64> {
    let off = i.checked_mul(PAGE_SIZE)?;
    // The whole page must fit: a trailing partial page is not one of the grant's pages.
    if off.checked_add(PAGE_SIZE)? > r.size {
        return None;
    }
    r.base.checked_add(off)
}

/// **Build a device's DMA domain: an identity map over `regions` and nothing else.** `root` becomes
/// the top-level table (what the IOMMU's STE or context descriptor points at), `alloc_page_frame`
/// supplies intermediate tables, `phys_to_ptr` is [`Mapper`]'s transform, and `F` is the format.
/// Every page is mapped IOVA == PA with its region's [`DmaRegion::flags`]; overlapping regions fail
/// with [`MapError::AlreadyMapped`].
///
/// # Safety
/// `root` and every frame `alloc_page_frame` returns must be zeroed and page-aligned, and `phys_to_ptr`
/// must satisfy [`Mapper`]'s contract (the tables are reachable through it). The caller owns `root`
/// and must not install it anywhere until this returns `Ok`.
pub unsafe fn build_identity_domain<A, P, F>(
    root: u64,
    alloc_page_frame: A,
    phys_to_ptr: P,
    regions: &[DmaRegion],
) -> Result<(), MapError>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
    F: PageFormat,
{
    // IOVA == PA in the low half, so the WrongHalf guard refuses a region in the high half, which
    // the IOMMU would never consult.
    // SAFETY: forwarded from this function's contract.
    let mut m = unsafe { Mapper::<A, P, F>::new(root, Half::Low, alloc_page_frame, phys_to_ptr) };
    for r in regions {
        // The page set comes from the proved arithmetic, not `map_range`'s unchecked
        // `va + i * PAGE_SIZE`.
        let pages = grant_pages(*r)?;
        for i in 0..pages {
            // Proved unreachable (`a_page_index_below_the_count_always_resolves`); an error rather
            // than a silent `continue`, because the compiler cannot see that.
            let page = grant_page(*r, i).ok_or(MapError::BadRegion)?;
            // The tables are not installed yet, so no leaf needs a flush. `AlreadyMapped` means two
            // grants overlap, and fails the build closed.
            m.map(page, page, r.flags())?;
        }
    }
    Ok(())
}

// Machine-checked proofs (Kani) that the domain maps exactly the grant: the page enumeration is
// sound, complete, injective and total, for every region and index. See notes/paging.md, "DMA
// domains", for the decomposition and what it leaves to tests.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **The enumeration is total: no region and no index makes it panic or wrap.** Both entry points
    /// over fully symbolic inputs. Kani checks arithmetic overflow and panics on every operation, so a
    /// run that trips nothing is the totality proof; it is worth its own harness because the builder
    /// calls these on a region a caller supplied.
    /// Falsification: replayable `crates/paging/falsifications/domain.verification.the_grant_enumeration_is_total.patch`
    #[kani::proof]
    fn the_grant_enumeration_is_total() {
        let r = DmaRegion {
            base: kani::any(),
            size: kani::any(),
            writable: kani::any(),
        };
        let i: u64 = kani::any();
        let _ = grant_pages(r);
        let _ = grant_page(r, i);
    }

    /// **Soundness, the security direction: every page the domain maps lies wholly inside the
    /// grant.** For every region and every index the builder will visit, the enumerated IOVA is
    /// page-aligned and `[page, page + PAGE_SIZE)` is contained in `[base, base + size)`. So the
    /// domain cannot map a byte the device was not granted: not the bytes below an unaligned base
    /// (refused), not a trailing partial page (never enumerated), and not a low address reached by
    /// wrapping (refused). Since IOVA == PA, this is equally the statement that the device translates
    /// only to granted physical memory.
    /// Falsification: replayable `crates/paging/falsifications/domain.verification.an_enumerated_page_lies_inside_the_grant.patch`
    #[kani::proof]
    fn an_enumerated_page_lies_inside_the_grant() {
        let r = DmaRegion {
            base: kani::any(),
            size: kani::any(),
            writable: kani::any(),
        };
        let i: u64 = kani::any();
        if let Ok(pages) = grant_pages(r) {
            kani::assume(i < pages);
            let page = grant_page(r, i).expect("a page index below the count always resolves");
            assert!(
                page.is_multiple_of(PAGE_SIZE),
                "an unaligned page was mapped"
            );
            assert!(page >= r.base, "a page below the grant was mapped");
            let end = page
                .checked_add(PAGE_SIZE)
                .expect("an enumerated page cannot end past u64");
            let limit = r
                .base
                .checked_add(r.size)
                .expect("an accepted grant has an end");
            assert!(end <= limit, "a page past the end of the grant was mapped");
            // The harness reaches grants that actually have pages to map (see the non-vacuity note on
            // `every_whole_page_of_the_grant_is_enumerated`).
            kani::cover!(pages > 1, "a multi-page grant satisfies the assumptions");
        }
    }

    /// **The `ok_or` in the builder is dead code, provably.** For every accepted region, every index
    /// below the page count resolves, so the builder's defensive branch cannot fire and the loop maps
    /// exactly `pages` pages. Split from the soundness harness because it is the fact the builder's
    /// control flow depends on, and it should be named where a reader of that branch will look.
    /// Falsification: replayable `crates/paging/falsifications/domain.verification.a_page_index_below_the_count_always_resolves.patch`
    #[kani::proof]
    fn a_page_index_below_the_count_always_resolves() {
        let r = DmaRegion {
            base: kani::any(),
            size: kani::any(),
            writable: kani::any(),
        };
        let i: u64 = kani::any();
        if let Ok(pages) = grant_pages(r) {
            kani::assume(i < pages);
            assert!(
                grant_page(r, i).is_some(),
                "an index below the page count did not resolve",
            );
        }
    }

    /// **Completeness, the functional direction: no whole page of the grant is left unmapped.** For
    /// every accepted region and every page-aligned address whose whole page lies inside it, there is
    /// an index below the page count that enumerates exactly that address. Proved constructively (the
    /// witness is `(iova - base) / PAGE_SIZE`), which is stronger than an existence claim: it says
    /// *which* iteration maps it. Without it, a domain that mapped nothing would satisfy soundness
    /// and confine the device by starving it.
    /// Falsification: replayable `crates/paging/falsifications/domain.verification.every_whole_page_of_the_grant_is_enumerated.patch`
    #[kani::proof]
    fn every_whole_page_of_the_grant_is_enumerated() {
        let r = DmaRegion {
            base: kani::any(),
            size: kani::any(),
            writable: kani::any(),
        };
        let iova: u64 = kani::any();
        if let Ok(pages) = grant_pages(r) {
            kani::assume(iova.is_multiple_of(PAGE_SIZE));
            kani::assume(iova >= r.base);
            let Some(end) = iova.checked_add(PAGE_SIZE) else {
                return;
            };
            let limit = r
                .base
                .checked_add(r.size)
                .expect("an accepted grant has an end");
            kani::assume(end <= limit);

            let i = (iova - r.base) / PAGE_SIZE;
            assert!(i < pages, "a granted page has no index in the enumeration");
            assert_eq!(
                grant_page(r, i),
                Some(iova),
                "a granted page is not the page its own index enumerates",
            );

            // **Non-vacuity, machine-checked.** Four stacked assumptions could in principle be jointly
            // unsatisfiable, in which case this harness would verify while proving nothing at all;
            // notes/verification.md calls that the main failure mode ("it proves what you asserted, not
            // what you meant"). `kani::cover!` fails when a state is *un*reachable, so this asserts the
            // harness really does reach a grant with pages in it.
            kani::cover!(pages > 0, "a non-empty grant satisfies the assumptions");
            kani::cover!(
                pages > 1,
                "so does a multi-page grant, so `i` is not pinned to 0"
            );
        }
    }

    /// **The enumeration is injective: two distinct indices are two distinct pages.** So one region's
    /// build never maps the same IOVA twice, which matters because [`Mapper::map`] refuses a second
    /// write to a live leaf ([`MapError::AlreadyMapped`]): without injectivity a perfectly legal grant
    /// could fail its own build. Together with soundness and completeness this pins the mapped set to
    /// exactly the grant's whole pages, counted once each.
    /// Falsification: replayable `crates/paging/falsifications/domain.verification.the_enumeration_is_injective.patch`
    #[kani::proof]
    fn the_enumeration_is_injective() {
        let r = DmaRegion {
            base: kani::any(),
            size: kani::any(),
            writable: kani::any(),
        };
        let i: u64 = kani::any();
        let j: u64 = kani::any();
        if let Ok(pages) = grant_pages(r) {
            kani::assume(i < pages);
            kani::assume(j < pages);
            kani::assume(i != j);
            assert_ne!(
                grant_page(r, i),
                grant_page(r, j),
                "two distinct indices enumerate the same page",
            );
            // Two distinct in-range indices must actually exist, or this proves nothing: a grant of one
            // page satisfies `i < pages && j < pages && i != j` for no `(i, j)` at all.
            kani::cover!(
                pages > 1,
                "a grant with two distinct page indices is reachable"
            );
        }
    }

    /// **A grant the domain cannot express exactly is refused, not approximated.** For every region
    /// with an unaligned base or an end that wraps `u64`, [`grant_pages`] errors, so the builder maps
    /// nothing at all rather than mapping a rounded or wrapped page set. This is the fail-closed half
    /// of the property: the two inputs that could produce an *over*-map are the two that are refused.
    /// Falsification: replayable `crates/paging/falsifications/domain.verification.a_grant_the_domain_cannot_express_is_refused.patch`
    #[kani::proof]
    fn a_grant_the_domain_cannot_express_is_refused() {
        let r = DmaRegion {
            base: kani::any(),
            size: kani::any(),
            writable: kani::any(),
        };
        if !r.base.is_multiple_of(PAGE_SIZE) || r.base.checked_add(r.size).is_none() {
            assert!(
                grant_pages(r).is_err(),
                "a grant no page-granular domain can express exactly was accepted",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::{Layout, alloc_zeroed};
    use std::cell::RefCell;
    use std::vec::Vec;

    use super::*;
    use crate::{Aarch64, AmdVi, Sv39};

    // A synthetic physical address space: the tests choose "physical" addresses in the low half
    // every format can express, and the host frames sit behind `phys_to_ptr`. A host allocator's
    // addresses need not lie in the low half (on aarch64 Linux they do not), and
    // `build_identity_domain` rightly refuses those with `MapError::WrongHalf`.

    /// Where the synthetic pool starts. Low half, page-aligned, and far below Sv39's 2^38 ceiling.
    const POOL_BASE: u64 = 0x0100_0000;

    thread_local! {
        /// Synthetic PA to host pointer. Thread-local so parallel tests cannot see each other's
        /// frames, which also means each test's pool starts empty.
        static PHYS: RefCell<Vec<(u64, u64)>> = const { RefCell::new(Vec::new()) };
    }

    /// The layout every host frame is allocated with, and the one every `dealloc` must repeat.
    ///
    /// Named once so the two sites cannot drift apart. A `dealloc` whose layout disagrees with its
    /// `alloc` is itself undefined behaviour, which would be an unusually embarrassing way to fail
    /// the check this fixture exists to pass.
    fn frame_layout() -> Layout {
        Layout::from_size_align(PAGE_SIZE as usize, PAGE_SIZE as usize).unwrap()
    }

    /// The synthetic pool. **Holding one is the only way to allocate a frame**, and dropping it frees
    /// every frame allocated through it, which keeps Miri's leak check meaningful
    /// (`notes/undefined-behavior.md`). `frame` and `frame_at` are methods so that a test cannot
    /// allocate without a pool; that is the whole reason for the otherwise unused receiver.
    ///
    /// # BUGS
    ///
    /// Nesting two live pools on one thread would make the inner drop free the outer's frames, so
    /// [`FramePool::new`] refuses it loudly rather than leave a use-after-free.
    struct FramePool;

    /// A [`Mapper`] that allocates nothing, over this module's synthetic physical memory.
    type Walker<F> = Mapper<fn() -> Option<u64>, fn(u64) -> *mut PageTable, F>;

    impl FramePool {
        /// Open the thread's pool. Panics if one is already open on this thread; see the type's
        /// `BUGS`.
        fn new() -> Self {
            assert!(
                PHYS.with(|m| m.borrow().is_empty()),
                "a FramePool is already open on this thread: nesting them would free its frames early"
            );
            FramePool
        }

        /// Back the synthetic physical address `pa` with a real zeroed host frame.
        fn frame_at(&self, pa: u64) -> u64 {
            assert!(
                pa.is_multiple_of(PAGE_SIZE),
                "a synthetic PA must be page-aligned"
            );
            // SAFETY: a valid non-zero layout; alloc_zeroed returns zeroed memory or null (asserted).
            let p = unsafe { alloc_zeroed(frame_layout()) };
            assert!(!p.is_null());
            PHYS.with(|m| m.borrow_mut().push((pa, p as u64)));
            pa
        }

        /// The next frame from the sequential pool: for roots and page tables, where the address does
        /// not matter as long as it is legal and distinct.
        fn frame(&self) -> u64 {
            let n = PHYS.with(|m| m.borrow().len()) as u64;
            self.frame_at(POOL_BASE + n * PAGE_SIZE)
        }

        /// [`build_identity_domain`] over `regions` into a fresh root, every table from this pool,
        /// and the root back. The one place these tests argue the build's contract.
        fn build_domain<F: PageFormat>(&self, regions: &[DmaRegion]) -> u64 {
            let root = self.frame();
            // SAFETY: `root` and every table the build asks for are zeroed, page-aligned host
            // frames this pool just allocated and still holds; `phys_to_ptr` is the identity over
            // exactly those frames; and nothing installs `root` anywhere.
            unsafe {
                build_identity_domain::<_, _, F>(root, || Some(self.frame()), phys_to_ptr, regions)
            }
            .expect("domain build failed");
            root
        }

        /// A walker over the domain at `root`, which allocates nothing, for `translate`. The one
        /// place these tests argue `Mapper::new`'s contract.
        fn walker<F: PageFormat>(&self, root: u64) -> Walker<F> {
            // SAFETY: `root` is a page-aligned frame of this pool, zeroed when the pool made it and
            // written since only by a build, and `phys_to_ptr` reaches every frame the pool holds.
            // It panics on any other address, so a walker kept past the pool fails loudly.
            unsafe { Mapper::new(root, Half::Low, || None, phys_to_ptr) }
        }
    }

    impl Drop for FramePool {
        fn drop(&mut self) {
            PHYS.with(|m| {
                for (_, host) in m.borrow_mut().drain(..) {
                    // SAFETY: `host` came from `frame_at`'s alloc_zeroed with exactly this layout,
                    // registered exactly once, and the drain removes it so nothing frees it twice.
                    unsafe { std::alloc::dealloc(host as *mut u8, frame_layout()) };
                }
            });
        }
    }

    /// Resolve a synthetic PA to the host frame behind it. Panics on an unregistered address, which
    /// would mean the walker followed a pointer nothing ever allocated: a loud test bug beats a
    /// segfault.
    fn phys_to_ptr(pa: u64) -> *mut PageTable {
        PHYS.with(|m| {
            m.borrow()
                .iter()
                .find(|(p, _)| *p == pa)
                .map(|(_, host)| *host as *mut PageTable)
                .unwrap_or_else(|| panic!("no host frame backs synthetic PA {pa:#x}"))
        })
    }

    /// Build a domain over one region and check every in-region page translates to itself while a
    /// page just past the end does not. This is the confinement stated as a property of the tables.
    fn one_region_confines<F: PageFormat>() {
        let pool = FramePool::new();
        // The device's data frame, at an address the TEST chooses so the assertions below mean what
        // they say: `base + PAGE_SIZE` must be a page the domain was not given, and with a sequential
        // pool it would have been the next frame allocated instead.
        let base = pool.frame_at(0x2000_0000);
        let regions = [DmaRegion {
            base,
            size: PAGE_SIZE,
            writable: true,
        }];
        let root = pool.build_domain::<F>(&regions);

        let m = pool.walker::<F>(root);
        assert_eq!(
            m.translate(base),
            Some((base, Flags::user_data())),
            "an in-region IOVA did not translate to its own PA",
        );
        assert_eq!(
            m.translate(base + PAGE_SIZE),
            None,
            "a page past the region translated: the device is not confined",
        );
    }

    #[test]
    fn aarch64_domain_confines_a_region() {
        one_region_confines::<Aarch64>();
    }

    #[test]
    fn sv39_domain_confines_a_region() {
        one_region_confines::<Sv39>();
    }

    /// **An AMD-Vi domain confines through the shared walk**, which is the one claim about the
    /// format that only a whole build can make: [`build_identity_domain`] writes a directory entry
    /// at every level with the level it is at, and the walk then stops only at the leaf. A wrong
    /// `NextLevel` would read as a page one level up (`is_block`), and the in-region page would
    /// translate to the wrong place or not at all. `leaf_flags` carries only `IW` on this format,
    /// so the flags are checked for writability rather than compared whole.
    #[test]
    fn amd_vi_domain_confines_a_region() {
        let pool = FramePool::new();
        let base = pool.frame_at(0x2000_0000);
        let regions = [DmaRegion {
            base,
            size: PAGE_SIZE,
            writable: true,
        }];
        let root = pool.build_domain::<AmdVi>(&regions);
        let m = pool.walker::<AmdVi>(root);
        let (pa, flags) = m
            .translate(base)
            .expect("an in-region IOVA did not translate");
        assert_eq!(pa, base);
        assert!(flags.is_writable());
        assert_eq!(
            m.translate(base + PAGE_SIZE),
            None,
            "a page past the region translated"
        );
    }

    /// **A region the device may only read is mapped without write, on every DMA format**, and a
    /// writable one beside it keeps write: the leaf carries the region's rights rather than one
    /// answer for the whole domain. Asked through `leaf_flags`, which on
    /// `Vtd` and `AmdVi` reports only write, so writability is what is compared.
    fn a_read_only_region_maps_read_only<F: PageFormat>() {
        let pool = FramePool::new();
        let ro = pool.frame_at(0x2000_0000);
        let rw = pool.frame_at(0x3000_0000);
        let regions = [
            DmaRegion {
                base: ro,
                size: PAGE_SIZE,
                writable: false,
            },
            DmaRegion {
                base: rw,
                size: PAGE_SIZE,
                writable: true,
            },
        ];
        let root = pool.build_domain::<F>(&regions);
        let m = pool.walker::<F>(root);
        let (pa, flags) = m.translate(ro).expect("the read-only region did not map");
        assert_eq!(pa, ro);
        assert!(
            !flags.is_writable(),
            "a region granted read-only is writable to the device"
        );
        let (_, flags) = m.translate(rw).expect("the writable region did not map");
        assert!(flags.is_writable(), "a writable region lost write");
    }

    #[test]
    fn a_read_only_region_maps_read_only_on_every_format() {
        a_read_only_region_maps_read_only::<Aarch64>();
        a_read_only_region_maps_read_only::<Sv39>();
        a_read_only_region_maps_read_only::<crate::Vtd>();
        a_read_only_region_maps_read_only::<AmdVi>();
    }

    /// **A grant of more than one page maps every whole page in it, and stops at the partial tail.**
    /// Every other test in this module grants exactly one page, and a one-page region is the shape
    /// that can tell nothing apart: a page count and the constant 1 agree on it, and so do
    /// [`grant_page`]'s whole-page test and its opposite. Two pages and a half separates all three.
    #[test]
    fn a_multi_page_grant_maps_its_whole_pages_and_not_the_partial_tail() {
        let pool = FramePool::new();
        let base = pool.frame_at(0x2000_0000);
        let regions = [DmaRegion {
            base,
            size: 2 * PAGE_SIZE + PAGE_SIZE / 2,
            writable: true,
        }];
        let root = pool.build_domain::<Aarch64>(&regions);
        let m = pool.walker::<Aarch64>(root);
        assert_eq!(m.translate(base), Some((base, Flags::user_data())));
        assert_eq!(
            m.translate(base + PAGE_SIZE),
            Some((base + PAGE_SIZE, Flags::user_data())),
            "the grant's second whole page is missing: an honest driver is starved",
        );
        assert_eq!(
            m.translate(base + 2 * PAGE_SIZE),
            None,
            "the trailing half page was mapped whole: the device reaches past its grant",
        );
    }

    /// Two disjoint regions (the driver's DMA region and the kernel's shadow page are not adjacent)
    /// both map, and the gap between them does not: the domain is exactly the allow-list, no more.
    #[test]
    fn two_disjoint_regions_map_and_the_gap_does_not() {
        let pool = FramePool::new();
        // Far apart on purpose: the point of this test is the GAP between them, and adjacent regions
        // would prove nothing about it. Sequentially-pooled frames would be adjacent, which is how
        // this assertion could have quietly stopped testing anything.
        let a = pool.frame_at(0x2000_0000);
        let b = pool.frame_at(0x3000_0000);
        let regions = [
            DmaRegion {
                base: a,
                size: PAGE_SIZE,
                writable: true,
            },
            DmaRegion {
                base: b,
                size: PAGE_SIZE,
                writable: true,
            },
        ];
        let root = pool.build_domain::<Sv39>(&regions);
        let m = pool.walker::<Sv39>(root);
        assert!(m.translate(a).is_some(), "region A did not map");
        assert!(m.translate(b).is_some(), "region B did not map");
        // A frame the domain was never given: unmapped, so the device faults on it. In the gap
        // between A and B, which is the strongest place to ask.
        let c = pool.frame_at(0x2800_0000);
        assert_eq!(
            m.translate(c),
            None,
            "an ungranted frame translated: the domain leaks",
        );
    }
}
