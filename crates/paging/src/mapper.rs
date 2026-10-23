//! **The shared walk**: descending a format's levels, allocating intermediate tables and writing
//! a leaf, and why a mapping was refused.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Why [`Mapper::map`] or [`Mapper::unmap`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    /// Ran out of frames while building intermediate tables.
    OutOfPageFrames,
    /// The virtual or physical address is not 4 KiB aligned.
    Misaligned,
    /// Something is already mapped here.
    ///
    /// **Load-bearing, not defensive.** Refusing to overwrite is what forces break-before-make: to
    /// change a mapping you must `unmap` (which hands you a [`TlbFlush`]) and then `map`. Silently
    /// overwriting would go valid → valid (which the hardware may reject) *and* leak the old frame.
    AlreadyMapped,
    /// This address belongs to the *other* half, or is non-canonical. Mapping a kernel address into
    /// the user tables would silently do nothing: the hardware would never consult this table for
    /// that address. Catching it here turns a mystery into an error.
    WrongHalf,
    /// Nothing is mapped at this address, so there is nothing to unmap.
    NotMapped,
    /// **This page is part of a block**, so it cannot be unmapped on its own: `unmap` takes 4 KiB
    /// and a block is 2 MiB or 1 GiB of one entry. Splitting the block would be the way to honour
    /// the request, and nothing here does that (see the crate header). Name provisional.
    InsideBlock,
    /// The format has no leaf of the size asked for ([`PageFormat::block_entry`] declined). Name
    /// provisional.
    UnsupportedSize,
    /// **A region that is not a region**: its `base + size` wraps `u64`, so it has no end. Only
    /// [`domain::grant_pages`] raises this, and only for an input no caller can construct today; it
    /// exists because refusing such a region is what stops the page enumeration from wrapping into
    /// addresses *outside* the grant. See notes/paging.md, "DMA domains".
    BadRegion,
}

/// Builds page tables, for any [`PageFormat`].
///
/// # Safety contract
///
/// The mapper dereferences physical addresses through `phys_to_ptr`. That is sound while the MMU is
/// off, so an address is a physical one (how the first table is built), or while physical memory
/// is identity- or direct-mapped by a known transform (how tables are edited afterwards). The host
/// tests pass the identity, since a host allocation's address serves as a physical one.
pub struct Mapper<A, P, F>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
    F: PageFormat,
{
    root: u64,
    half: Half,
    alloc_page_frame: A,
    phys_to_ptr: P,
    _fmt: PhantomData<F>,
}

impl<A, P, F> Mapper<A, P, F>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
    F: PageFormat,
{
    /// # Safety
    /// `root` must be a zeroed, page-aligned frame, and `phys_to_ptr` must satisfy the contract in
    /// the type's docs.
    pub unsafe fn new(root: u64, half: Half, alloc_page_frame: A, phys_to_ptr: P) -> Self {
        Self {
            root,
            half,
            alloc_page_frame,
            phys_to_ptr,
            _fmt: PhantomData,
        }
    }

    /// The physical address of the top-level table.
    pub fn root(&self) -> u64 {
        self.root
    }

    /// Which half of the address space this mapper builds into.
    pub fn half(&self) -> Half {
        self.half
    }

    /// Map one 4 KiB page. Walks the levels, creating tables as needed, and writes a leaf at the
    /// bottom.
    ///
    /// # Examples
    ///
    /// Real aarch64 tables in host memory; the pointer arithmetic is the kernel's:
    ///
    /// ```
    /// # use std::cell::RefCell;
    /// # use paging::{Aarch64, Flags, Half, MapError, Mapper, PageTable};
    /// # let tables: RefCell<Vec<*mut PageTable>> = RefCell::new(Vec::new());
    /// # let mut fresh = || {
    /// #     let p = Box::into_raw(Box::new(PageTable::new()));
    /// #     tables.borrow_mut().push(p);
    /// #     p as u64
    /// # };
    /// # let root = fresh();
    /// # let to_ptr = (|pa: u64| pa as *mut PageTable) as fn(u64) -> *mut PageTable;
    /// # // SAFETY: `root` is a fresh, zeroed, 4 KiB-aligned table, and these "physical" addresses ARE
    /// # // host addresses, so the identity cast is the transform.
    /// # let mut m: Mapper<_, fn(u64) -> *mut PageTable, Aarch64> =
    /// #     unsafe { Mapper::new(root, Half::Low, || Some(fresh()), to_ptr) };
    /// // A user text page. There is no writable-and-executable constructor to pass here at all.
    /// m.map(0x40_0000, 0x4000_0000, Flags::user_code()).unwrap();
    /// let (pa, flags) = m.translate(0x40_0000).unwrap();
    /// assert!(pa == 0x4000_0000 && flags.is_user_executable() && !flags.is_writable());
    ///
    /// // Overwriting a live mapping would go valid to valid and leak the old frame: refused.
    /// assert_eq!(m.map(0x40_0000, 0x5000_0000, Flags::user_data()), Err(MapError::AlreadyMapped));
    /// // A kernel address in user tables is a mapping the CPU would never consult.
    /// let kernel = m.map(0xffff_0000_0000_0000, 0x4000_0000, Flags::kernel_data());
    /// assert_eq!(kernel, Err(MapError::WrongHalf));
    /// # drop(m); // the mapper borrows the tables; it goes first
    /// # for p in tables.borrow_mut().drain(..) {
    /// #     // SAFETY: each `p` came from `Box::into_raw` above and is registered exactly once.
    /// #     unsafe { drop(Box::from_raw(p)) };
    /// # }
    /// ```
    pub fn map(&mut self, va: u64, pa: u64, flags: Flags) -> Result<(), MapError> {
        // The hardware selects the table set from the top bits before it touches an index, so
        // mapping a high address into the low tables would build a mapping the CPU never consults.
        if !F::is_in_half(self.half, va) {
            return Err(MapError::WrongHalf);
        }

        if !va.is_multiple_of(PAGE_SIZE) || !pa.is_multiple_of(PAGE_SIZE) {
            return Err(MapError::Misaligned);
        }

        let mut table_pa = self.root;

        // Descend through every level but the last, creating intermediate tables as we go.
        for level in 0..F::LEVELS - 1 {
            let i = F::index(va, level);

            // SAFETY: `table_pa` is a page-aligned table, per the type's contract.
            let entry = unsafe { &mut (*(self.phys_to_ptr)(table_pa)).entries[i] };

            if !F::is_present(*entry) {
                let new = (self.alloc_page_frame)().ok_or(MapError::OutOfPageFrames)?;

                // SAFETY: a fresh frame from the allocator. Zero it before it becomes reachable by
                // the hardware, or the walk reads whatever garbage was in RAM and follows it.
                unsafe {
                    (*(self.phys_to_ptr)(new)).entries = [0; ENTRIES];
                }

                *entry = F::table_entry(new, level);
            } else if F::is_block(*entry) {
                // A block already maps this address. Descending would take the block's frame for
                // a page table and write a leaf into somebody's data.
                return Err(MapError::AlreadyMapped);
            }

            table_pa = F::entry_pa(*entry);
        }

        // The leaf.
        let i = F::index(va, F::LEVELS - 1);
        // SAFETY: as above.
        let entry = unsafe { &mut (*(self.phys_to_ptr)(table_pa)).entries[i] };

        if F::is_present(*entry) {
            return Err(MapError::AlreadyMapped);
        }

        *entry = F::leaf_entry(pa, flags);

        Ok(())
    }

    /// **Map one leaf of `size`**: a 4 KiB page (exactly [`map`](Self::map)), or a 2 MiB or 1 GiB
    /// block written one or two levels up, with the tables above it created as needed.
    ///
    /// Refuses a misaligned `va` or `pa` (both must be multiples of `size`), a size the format
    /// cannot encode ([`MapError::UnsupportedSize`]), and anything already present at or above the
    /// block's slot, **including an empty table**: taking over a table slot would orphan the table
    /// frame, and the mapper does not own frames to free them.
    pub fn map_block(
        &mut self,
        va: u64,
        pa: u64,
        size: PageSize,
        flags: Flags,
    ) -> Result<(), MapError> {
        if size == PageSize::Size4KiB {
            return self.map(va, pa, flags);
        }
        if !F::is_in_half(self.half, va) {
            return Err(MapError::WrongHalf);
        }
        let bytes = size.bytes();
        if !va.is_multiple_of(bytes) || !pa.is_multiple_of(bytes) {
            return Err(MapError::Misaligned);
        }
        let leaf = F::block_entry(pa, flags, size).ok_or(MapError::UnsupportedSize)?;
        // Every CPU format here has at least three levels, so a 1 GiB block (two up) always has a
        // level to sit at; a format with fewer would have declined above.
        let leaf_level = F::LEVELS - 1 - size.levels_above_bottom();

        let mut table_pa = self.root;
        for level in 0..leaf_level {
            let i = F::index(va, level);
            // SAFETY: `table_pa` is a page-aligned table, per the type's contract.
            let entry = unsafe { &mut (*(self.phys_to_ptr)(table_pa)).entries[i] };
            if !F::is_present(*entry) {
                let new = (self.alloc_page_frame)().ok_or(MapError::OutOfPageFrames)?;
                // SAFETY: a fresh frame, zeroed before it becomes reachable (see `map`).
                unsafe {
                    (*(self.phys_to_ptr)(new)).entries = [0; ENTRIES];
                }
                *entry = F::table_entry(new, level);
            } else if F::is_block(*entry) {
                return Err(MapError::AlreadyMapped);
            }
            table_pa = F::entry_pa(*entry);
        }

        let i = F::index(va, leaf_level);
        // SAFETY: as above.
        let entry = unsafe { &mut (*(self.phys_to_ptr)(table_pa)).entries[i] };
        if F::is_present(*entry) {
            return Err(MapError::AlreadyMapped);
        }
        *entry = leaf;
        Ok(())
    }

    /// **Map `len` bytes from `va` to `pa` in the largest leaves that fit**, none larger than
    /// `largest`: the direct map's shape, a 4 KiB head, blocks through the middle, a 4 KiB tail.
    ///
    /// A block is used only where it lies inside the span and both addresses are aligned to it
    /// ([`PageSize::largest_fitting`], where that is proved), so this maps exactly the pages
    /// [`map_range`](Self::map_range) would. `largest` is the caller's, because only it knows what
    /// the machine supports (x86 has 1 GiB leaves only where `CPUID` says so). `va`, `pa` and `len`
    /// must be page-aligned. Not atomic: on an error, what was mapped before it stays mapped.
    pub fn map_span(
        &mut self,
        va: u64,
        pa: u64,
        len: u64,
        flags: Flags,
        largest: PageSize,
    ) -> Result<(), MapError> {
        if !len.is_multiple_of(PAGE_SIZE) {
            return Err(MapError::Misaligned);
        }
        let mut done = 0;
        while done < len {
            let size = PageSize::largest_fitting(va + done, pa + done, len - done, largest);
            self.map_block(va + done, pa + done, size, flags)?;
            done += size.bytes();
        }
        Ok(())
    }

    /// Map `count` consecutive pages.
    pub fn map_range(
        &mut self,
        va: u64,
        pa: u64,
        count: u64,
        flags: Flags,
    ) -> Result<(), MapError> {
        for i in 0..count {
            self.map(va + i * PAGE_SIZE, pa + i * PAGE_SIZE, flags)?;
        }
        Ok(())
    }

    /// Remove a mapping, and return the physical frame it pointed at, which the caller took from the
    /// frame allocator and must give back. Only the leaf is cleared: break-before-make remaps the
    /// same address at once, so the tables stay. Teardown does not use this (notes/teardown.md).
    ///
    /// Returns a [`TlbFlush`] you cannot ignore.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::cell::RefCell;
    /// # use paging::{Aarch64, Flags, Half, MapError, Mapper, PageTable};
    /// # let tables: RefCell<Vec<*mut PageTable>> = RefCell::new(Vec::new());
    /// # let mut fresh = || {
    /// #     let p = Box::into_raw(Box::new(PageTable::new()));
    /// #     tables.borrow_mut().push(p);
    /// #     p as u64
    /// # };
    /// # let root = fresh();
    /// # let to_ptr = (|pa: u64| pa as *mut PageTable) as fn(u64) -> *mut PageTable;
    /// # // SAFETY: `root` is a fresh, zeroed, 4 KiB-aligned table, and these "physical" addresses ARE
    /// # // host addresses, so the identity cast is the transform.
    /// # let mut m: Mapper<_, fn(u64) -> *mut PageTable, Aarch64> =
    /// #     unsafe { Mapper::new(root, Half::Low, || Some(fresh()), to_ptr) };
    /// # m.map(0x40_0000, 0x4000_0000, Flags::user_code()).unwrap();
    /// let (freed, flush) = m.unmap(0x40_0000).unwrap();
    /// assert_eq!(freed, 0x4000_0000);
    /// flush.flush(|va| assert_eq!(va, 0x40_0000)); // the kernel's `tlbi vaae1is` goes here
    /// assert!(m.translate(0x40_0000).is_none());
    /// m.map(0x40_0000, 0x5000_0000, Flags::user_data()).unwrap(); // free again
    /// # let _ = MapError::AlreadyMapped;
    /// # drop(m); // the mapper borrows the tables; it goes first
    /// # for p in tables.borrow_mut().drain(..) {
    /// #     // SAFETY: each `p` came from `Box::into_raw` above and is registered exactly once.
    /// #     unsafe { drop(Box::from_raw(p)) };
    /// # }
    /// ```
    pub fn unmap(&mut self, va: u64) -> Result<(u64, TlbFlush), MapError> {
        if !F::is_in_half(self.half, va) {
            return Err(MapError::WrongHalf);
        }
        if !va.is_multiple_of(PAGE_SIZE) {
            return Err(MapError::Misaligned);
        }

        let mut table_pa = self.root;

        for level in 0..F::LEVELS - 1 {
            let i = F::index(va, level);
            // SAFETY: per the type's contract.
            let entry = unsafe { (*(self.phys_to_ptr)(table_pa)).entries[i] };
            if !F::is_present(entry) {
                return Err(MapError::NotMapped);
            }
            if F::is_block(entry) {
                return Err(MapError::InsideBlock);
            }
            table_pa = F::entry_pa(entry);
        }

        let i = F::index(va, F::LEVELS - 1);
        // SAFETY: per the type's contract.
        let entry = unsafe { &mut (*(self.phys_to_ptr)(table_pa)).entries[i] };

        if !F::is_present(*entry) {
            return Err(MapError::NotMapped);
        }

        let pa = F::entry_pa(*entry);

        // Break: the entry becomes invalid *before* anything else happens.
        *entry = 0;

        Ok((pa, TlbFlush { va }))
    }

    /// **Cut the intermediate table at physical `table` out of the walk that reaches `va`**, and
    /// return `(base, span)`, the aligned range `table` translated, which now translates to nothing.
    /// `None` when the walk to `va` never passes through `table`, which a caller cutting tables
    /// top-down meets for every table below the first. Nothing is freed. Name provisional.
    ///
    /// The kernel cuts a table whose page is about to be handed back (`revoke::revoke_region`): one
    /// store makes the page unreachable, where clearing its leaves would leave it linked. The
    /// [`TlbFlush`]'s [`address`](TlbFlush::address) is `base`, and it must be discharged by an
    /// invalidation that reaches non-leaf entries for the whole span (a by-ASID flush does).
    pub fn unlink_table(&mut self, va: u64, table: u64) -> Option<(u64, u64, TlbFlush)> {
        if !F::is_in_half(self.half, va) {
            return None;
        }
        let mut table_pa = self.root;
        for level in 0..F::LEVELS - 1 {
            let i = F::index(va, level);
            // SAFETY: `table_pa` is a page-aligned table, per the type's contract.
            let entry = unsafe { &mut (*(self.phys_to_ptr)(table_pa)).entries[i] };
            if !F::is_present(*entry) || F::is_block(*entry) {
                return None;
            }
            if F::entry_pa(*entry) == table {
                // The table one level down translates the span one entry at `level` covers.
                let span = PAGE_SIZE << (9 * (F::LEVELS - 1 - level));
                let base = va & !(span - 1);
                // Break: no walk can reach `table` from here after this store.
                *entry = 0;
                return Some((base, span, TlbFlush { va: base }));
            }
            table_pa = F::entry_pa(*entry);
        }
        None
    }

    /// Walk the tables and report what a virtual address actually maps to. This is what the hardware
    /// does on every access, in silicon, and it is worth having in software: it is the only way to
    /// *check* that the tables say what you think they say.
    pub fn translate(&self, va: u64) -> Option<(u64, Flags)> {
        if !F::is_in_half(self.half, va) {
            return None;
        }

        let mut table_pa = self.root;

        for level in 0..F::LEVELS - 1 {
            let i = F::index(va, level);
            // SAFETY: per the type's contract.
            let entry = unsafe { (*(self.phys_to_ptr)(table_pa)).entries[i] };

            if !F::is_present(entry) {
                return None;
            }
            if F::is_block(entry) {
                // The walk ends early: this entry maps the whole span it covers. The span is
                // what the remaining index bits and the page offset address, together.
                let span = PAGE_SIZE << (9 * (F::LEVELS - 1 - level));
                let base = F::entry_pa(entry) & !(span - 1);
                return Some((base + (va & (span - 1)), F::leaf_flags(entry)));
            }
            table_pa = F::entry_pa(entry);
        }

        let i = F::index(va, F::LEVELS - 1);
        // SAFETY: per the type's contract.
        let entry = unsafe { (*(self.phys_to_ptr)(table_pa)).entries[i] };

        if !F::is_present(entry) {
            return None;
        }

        let offset = va % PAGE_SIZE;
        Some((F::entry_pa(entry) + offset, F::leaf_flags(entry)))
    }

    /// **[`translate`](Self::translate), keeping every entry it read**: for a report about a walk
    /// that failed, where "not mapped" is the question and not the answer. Which level stopped, and
    /// what was written there, is what tells a cleared entry from a table that was never built or a
    /// frame somebody else wrote into.
    ///
    /// `readable` is asked about every table before it is read, the root included, and a table it
    /// refuses ends the walk as [`WalkEnd::Unreadable`]. A failed walk is by definition one whose
    /// tables may hold garbage, and following a garbage address through `phys_to_ptr` from a fault
    /// handler would fault the kernel; the caller says which frames are safe to read.
    ///
    /// Name: provisional (lane `maintainer/pipeline-fetch-fault`, 2026-10-14 UTC).
    pub fn walk(&self, va: u64, readable: impl Fn(u64) -> bool) -> Walk {
        let mut walk = Walk {
            steps: [WalkStep::default(); MAX_WALK],
            len: 0,
            end: WalkEnd::OtherHalf,
        };
        if !F::is_in_half(self.half, va) {
            return walk;
        }
        let mut table = self.root;
        for level in 0..F::LEVELS.min(MAX_WALK) {
            if !readable(table) {
                walk.end = WalkEnd::Unreadable(table);
                return walk;
            }
            let index = F::index(va, level);
            // SAFETY: per the type's contract, for a table `readable` admitted.
            let entry = unsafe { (*(self.phys_to_ptr)(table)).entries[index] };
            walk.steps[walk.len] = WalkStep {
                table,
                index,
                entry,
            };
            walk.len += 1;
            walk.end = if !F::is_present(entry) {
                WalkEnd::NotPresent
            } else if level == F::LEVELS - 1 {
                WalkEnd::Mapped
            } else if F::is_block(entry) {
                WalkEnd::Block
            } else {
                table = F::entry_pa(entry);
                continue;
            };
            return walk;
        }
        walk
    }
}

/// The most levels any format here has: four (aarch64's 4 KiB granule, `Ia32e`). One spare, so a
/// fifth-level format would be caught by its own walk's length rather than by an index panic.
const MAX_WALK: usize = 5;

/// One level of a [`Walk`]: the table read, the index read at, and the raw entry found there.
/// Name: provisional (lane `maintainer/pipeline-fetch-fault`, 2026-10-14 UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WalkStep {
    /// The table's physical address.
    pub table: u64,
    /// The index the address selects in it.
    pub index: usize,
    /// The entry, raw, in the format's encoding.
    pub entry: u64,
}

/// How a [`Walk`] stopped. Name: provisional (lane `maintainer/pipeline-fetch-fault`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkEnd {
    /// A present leaf at the last level.
    Mapped,
    /// A present block entry above the last level.
    Block,
    /// The last step's entry is not present.
    NotPresent,
    /// The next table, at this physical address, was refused by the caller's `readable`.
    Unreadable(u64),
    /// The address is in the other half, so this mapper's tables never see it. No steps.
    OtherHalf,
}

/// **The entries a walk read, root first**, from [`Mapper::walk`]. Name: provisional (lane
/// `maintainer/pipeline-fetch-fault`, 2026-10-14 UTC).
#[derive(Debug, Clone, Copy)]
pub struct Walk {
    steps: [WalkStep; MAX_WALK],
    len: usize,
    end: WalkEnd,
}

impl Walk {
    /// Every level read, root first.
    pub fn steps(&self) -> &[WalkStep] {
        &self.steps[..self.len]
    }

    /// Why the walk stopped where it did.
    pub fn end(&self) -> WalkEnd {
        self.end
    }
}
