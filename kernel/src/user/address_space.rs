//! **A user address space**: its page table, the frames it owns, its address-space identifier, and
//! what its `Drop` gives back. See notes/user-address-spaces.md.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

use super::*;

/// A user address space: a root table for `TTBR0`, the region every page hanging off it comes
/// from, its TLB tag and its current-CPU pages.
///
/// An address space dies all at once: `Drop` destroys its region, tables and leaves alike, rather
/// than unmapping page by page. See notes/user-address-spaces.md.
pub struct AddressSpace {
    pub(super) root: PageFrame,
    /// **This address space's TLB tag, for life** (milestone 15; `crates/address_space_identifier`). Every user
    /// mapping is `nG`, so its TLB entries carry this number, and a context switch flushes
    /// nothing: the other spaces' entries just stop matching. Freed at drop, after
    /// `flush_asid` has made every entry so tagged vanish, which is what makes the number
    /// reusable.
    pub(super) asid: u16,
    /// **The untyped region every page of this address space comes from, and who frees it**
    /// (milestone 14 phase B.4): the root table, the intermediate tables, and every owned leaf
    /// are retyped out of one region. The region *is* the record of what this address space
    /// owns, which is why there is no frame list: teardown is `memory_region::destroy`, one call,
    /// made safe by §13 revocation.
    ///
    /// It carries the owner rather than just the name because **two different things build an
    /// address space and only one of them owns its memory**. See [`Backing`].
    pub(super) backing: Backing,

    /// **The frame this space's thread reads its own CPU out of, and its direct-map address**, or
    /// `None` if it has none: slot 0 of the space's thread pages (milestone 812), for a thread that
    /// is a process of its own or a process's first member.
    ///
    /// `crates/current_cpu_contract` holds the layout and calef's ruling of 2026-09-21 (a thread
    /// observing itself is a per-thread page). The frame comes from the global frame allocator, not
    /// `backing`'s region, so `Drop` frees it by hand. The address is worked out once at attach, to
    /// keep an overflow check off the context switch (notes/overflow-checks.md).
    pub(super) current_cpu_page: Option<(PageFrame, u64)>,

    /// **The other threads' current-CPU pages** (milestone 812 (`std::thread::spawn` runs real
    /// threads in one address space), §269 (how threads share a process) fork 6): slot `k`, for `k`
    /// from 1, is mapped read-only at `current_cpu_contract::thread_page_va(k)`. Slot 0 is
    /// [`Self::current_cpu_page`]. A slot's frame is kept once made and reused by the next thread to
    /// take the slot, and freed with the space. Physical addresses, valid where
    /// [`Self::thread_pages_made`] has the bit: an `Option<(PageFrame, u64)>` per slot was 384 bytes
    /// in every `AddressSpace`, which pushed `load`'s frame past the 4 KiB guard page on riscv64.
    pub(crate) thread_pages: [u64; current_cpu_contract::THREAD_PAGE_SLOTS],

    /// Which slots (bit `k`) have a frame in [`Self::thread_pages`].
    pub(crate) thread_pages_made: u16,

    /// Which slots (bit `k`, slot 0 included) a live thread is using now.
    pub(crate) thread_pages_in_use: u16,
}

/// **Who returns the region an [`AddressSpace`] spends.**
///
/// [`AddressSpace::new`] carves its own region, which nobody else can name, so its `Drop` frees it.
/// [`user_address_space_create`] is lent a region its caller holds a `MemoryRegion` capability to
/// and reclaims with `MemoryRegion::DESTROY` (milestone 19b (run a real workload)). Two names for
/// one run of memory, and only one may free it: a type, so no space can be built without saying
/// which. See notes/user-address-spaces.md.
#[derive(Clone, Copy)]
pub(super) enum Backing {
    /// The space carved this region itself and holds the only name for it. `Drop` frees it.
    Owned(u64),
    /// The region was handed in and belongs to whoever holds its `MemoryRegion` capability. `Drop`
    /// **must not** free it; the memory comes back at that owner's `sched::reclaim_region`.
    Lent(u64),
}

impl Backing {
    /// The region to retype from. Spending a lent region is correct and is the whole point of
    /// `RETYPE_OBJ(ADDRESS_SPACE)`: the space runs on the caller's budget. Only *freeing* is restricted.
    fn region(self) -> u64 {
        match self {
            Backing::Owned(region) | Backing::Lent(region) => region,
        }
    }
}

/// Page-table-and-slack overhead an address space needs beyond its content pages: the L0 root,
/// an L1 and L2, a handful of L3s (one per 2 MiB window touched, `Spawn` maps included), and
/// margin. Sixteen pages = 64 KiB, generous for every process this kernel builds.
const AS_OVERHEAD: u64 = 16;

/// **The window cost [`AS_OVERHEAD`]'s margin already carries**, which [`load`] subtracts rather
/// than charging a second time: one log page ([`crate::revoke::log_pages_for`], 170 recorded
/// mappings) and no table pages, since such a window touches at most one 2 MiB L3. Charging it
/// again costs a frame per long-lived process; see notes/user-address-spaces.md.
pub(super) const WINDOW_IN_OVERHEAD: u64 = 1;

impl AddressSpace {
    /// Carve this address space's budget: `content_pages` of expected leaves plus the
    /// page-table overhead. Everything the address space ever owns comes out of this region,
    /// and running out is a clean `OutOfPageFrames` at map time, spending nobody's memory but its
    /// own. The region's pages are retyped zeroed, so the root needs no separate scrub.
    pub fn new(content_pages: u64) -> Option<Self> {
        let region = crate::memory_region::create(content_pages + AS_OVERHEAD)?;
        let root = crate::memory_region::retype_page(region)?;

        // Share the kernel into this root. On RISC-V a process runs on a single `satp` that must map
        // both the process (low half) and the kernel (high half), so the root gets copies of the
        // kernel root's high-half entries. On aarch64 the kernel lives in a separate TTBR1 and this
        // is a no-op. See arch::mmu::share_kernel_half and DECISIONS §17.
        mmu::share_kernel_half(root);

        // A TLB tag of our own (milestone 15 (tagged address spaces)). Taken before the registry below, because the
        // registry records it: a revoke that cuts a table out of this space flushes by this tag.
        let Some(asid) = ASIDS.lock().alloc() else {
            crate::memory_region::destroy(region);
            return None;
        };

        // Into the revocation registry (phase C): this is how a later revoke finds our mapping
        // log, whose pages this same region will pay for. Full registry = no address space.
        if !crate::revoke::register_space(root, region, asid) {
            ASIDS.lock().free(asid);
            crate::memory_region::destroy(region);
            return None;
        }

        let mut space = AddressSpace {
            root: PageFrame::from_addr(root),
            asid,
            backing: Backing::Owned(region),
            current_cpu_page: None,
            thread_pages: [0; current_cpu_contract::THREAD_PAGE_SLOTS],
            thread_pages_made: 0,
            thread_pages_in_use: 0,
        };

        // Here rather than at each hand-built spawn: every space this returns is one a thread will
        // run in, so no seventh builder can forget it.
        space.attach_current_cpu_page();

        Some(space)
    }

    /// **Give this space the page its thread reads its own CPU from**, if it has not got one.
    ///
    /// Idempotent: `new` reaches it for the spaces the kernel builds, and
    /// `sched::configure_thread_control_block` for the ones userspace hands over. Every failure is
    /// silent and leaves the space without a page, so a missing frame cannot stop a program
    /// starting; the reader (`current_cpu_contract::CurrentCpuPage`) answers `None`.
    pub fn attach_current_cpu_page(&mut self) {
        if self.current_cpu_page.is_some() {
            return;
        }
        let Some(frame) = crate::memory::alloc_zeroed() else {
            return;
        };

        // Zeroed, because the whole frame is mapped into the process; stamped, so a prepared page
        // tells itself from a stray frame and "never run" is not CPU 0. The allowance is the online
        // core count until something grants fewer (§269 fork 7).
        let bytes = current_cpu_contract::build_page_allowing(
            u64::from(crate::smp::online_harts_mask().count_ones()),
            crate::arch::timer::TICK_HZ,
        );
        let kernel_va = mmu::phys_to_virt(frame.addr());
        // SAFETY: `frame` is freshly allocated and owned by nobody else yet, the direct map is
        // valid for it, and `PAGE_BYTES` (32) is far under `FRAME_SIZE`, so the copy stays inside
        // the frame.
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), kernel_va as *mut u8, bytes.len());
        };

        // Read-only to the process, which is what keeps this out of Tock's necessarily-unsafe
        // kernel category (notes/trusted-base.md): the kernel writes a frame it owns and lends the
        // process a view, rather than writing into memory the process supplied.
        if self
            .map_physical(
                current_cpu_contract::PAGE_VA,
                frame.addr(),
                Flags::user_rodata(),
                crate::revoke::PageMapSource::NoCapability,
            )
            .is_err()
        {
            crate::memory::free(frame);
            return;
        }
        self.current_cpu_page = Some((frame, kernel_va));
    }

    /// **Publish the core this space's thread is about to run on.** Called from the context
    /// switch, on the core doing the switching, which is the core the thread will execute on.
    ///
    /// One relaxed store; `current_cpu_contract` has the ordering argument. The context switch
    /// reads [`BoundSpace::publish_current_cpu`] instead (§249 (a running address space stays
    /// nameable)); this form is the tests' way to make the same write by hand.
    #[cfg(any(test, feature = "system_tests"))]
    #[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
    #[inline]
    pub fn publish_current_cpu(&self, cpu: u64) {
        if let Some((_, kernel_va)) = self.current_cpu_page {
            // SAFETY: the frame is this space's own, allocated by `attach_current_cpu_page` and
            // freed only by `Drop`, so the direct-map view is live and 16 bytes wide here. The
            // caller is the one core switching this thread in, and a thread is on one core, so
            // this is the only writer for as long as the store takes.
            unsafe { current_cpu_contract::publish(kernel_va, cpu) };
        }
    }

    /// **The kernel's own view of this space's current-CPU page**, for the tests that read it from
    /// the side the thread cannot: the unset state is unobservable from inside a thread, because a
    /// thread that can ask has already been switched in. `None` if the space has no page.
    #[cfg(any(test, feature = "system_tests"))]
    #[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
    pub fn current_cpu_page_kernel_va(&self) -> Option<u64> {
        self.current_cpu_page.map(|(_, kernel_va)| kernel_va)
    }

    /// Map one fresh, zeroed page at `va`, and hand back a **kernel** view of it.
    ///
    /// The returned slice is at `pa | KERNEL_VA_BASE` (the direct map), because the kernel
    /// cannot address `va` itself: `va` is a *low* address and means something entirely
    /// different from EL1's point of view. Two names for one frame, which is what the direct
    /// map is for.
    pub fn map_new(&mut self, va: u64, flags: Flags) -> Result<&'static mut [u8], MapError> {
        // Out of the address space's own region: the watermark is the ownership record, so
        // there is nothing to push anywhere. `retype_page` hands the page back zeroed, which is
        // what keeps `.bss` free for the loader.
        let frame = crate::memory_region::retype_page(self.backing.region())
            .ok_or(MapError::OutOfPageFrames)?;
        self.map_at(va, frame, flags)?;

        // SAFETY: the frame is ours (retyped from our region), and the direct map is valid for
        // it. 'static is a lie we tell for convenience and then keep: the frame outlives every
        // use of this slice, because the region is freed only at `Drop`.
        let page = unsafe {
            core::slice::from_raw_parts_mut(
                mmu::phys_to_virt(frame) as *mut u8,
                FRAME_SIZE as usize,
            )
        };
        Ok(page)
    }

    /// Map an **existing** physical page at `va` with `flags`, recording the mapping under `under`.
    ///
    /// The page is not this space's to free (device registers, or a page shared with another
    /// space), but the mapping is recorded, because one revocation cannot see is the DECISIONS §13
    /// (capability revocation and untyped reclamation) use-after-free; an unrecordable mapping is
    /// unmapped and refused as `OutOfPageFrames`. See notes/user-address-spaces.md.
    ///
    /// # BUGS
    /// [`Self::map_new`] does not record: its frames die with this space's region and no capability
    /// names them. If that changes, it is the second half of the same hole.
    pub fn map_physical(
        &mut self,
        va: u64,
        phys: u64,
        flags: Flags,
        under: crate::revoke::PageMapSource,
    ) -> Result<(), MapError> {
        self.map_physical_held(&mut crate::revoke::hold(), va, phys, flags, under)
    }

    /// [`Self::map_physical`] under a [`MappingHold`](crate::revoke::MappingHold) the caller already
    /// has, so a capability read under that hold and the mapping made from it are one critical
    /// section (`AddressSpace::MAP_INTO`; the hold's own docs have why).
    ///
    /// The plain form takes the hold itself, across `map_at` as well.
    pub fn map_physical_held(
        &mut self,
        hold: &mut crate::revoke::MappingHold,
        va: u64,
        phys: u64,
        flags: Flags,
        under: crate::revoke::PageMapSource,
    ) -> Result<(), MapError> {
        self.map_at(va, phys, flags)?;
        let root = self.root.addr();
        if !hold.record_mapping(phys, root, va, under) {
            mmu::unmap_user_at(root, va);
            return Err(MapError::OutOfPageFrames);
        }
        Ok(())
    }

    /// Map `phys` at `va`. Intermediate tables come from this address space's own region, so
    /// they are covered by the one teardown call; the target page is whoever's it was.
    fn map_at(&mut self, va: u64, phys: u64, flags: Flags) -> Result<(), MapError> {
        let root = self.root.addr();
        let region = self.backing.region();

        // SAFETY: `root` is a zeroed L0 table. Half::Low, so the mapper refuses a high address:
        // mapping the kernel's half into TTBR0 would build a translation the hardware never
        // consults, and we would chase the ghost for hours.
        let mut mapper = unsafe {
            Mapper::<_, _, crate::arch::mmu::Format>::new(
                root,
                Half::Low,
                || crate::memory_region::retype_page(region),
                phys_to_ptr,
            )
        };

        mapper.map(va, phys, flags)
    }

    /// The physical address of the L0 table: what page-table walks (translate, unmap,
    /// revocation) use. Not what goes in `TTBR0_EL1` any more; that is [`ttbr0`](Self::ttbr0),
    /// which carries the ASID too.
    pub fn root(&self) -> u64 {
        self.root.addr()
    }

    /// The composed `TTBR0_EL1` value: root plus this space's ASID, ready to install.
    pub fn ttbr0(&self) -> u64 {
        mmu::ttbr0_value(self.root.addr(), self.asid)
    }

    /// **Run `body` with this space installed as the current user half, then uninstall it**, so a
    /// kernel thread can ask the hardware's own tables about this space (`mmu::translate_user`).
    ///
    /// A closure rather than a guard, because a guard could be `mem::forget`ten with the space still
    /// installed. **Interrupts are masked throughout, so `body` must be short and must not block or
    /// yield**: the scheduler does not know about this install, and a switch away and back would
    /// uninstall it under the question. A body that blocks needs the space bound to its thread
    /// (`sched::adopt_address_space`), so the switch puts it back.
    #[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests are the callers
    pub fn while_installed<R>(&self, body: impl FnOnce() -> R) -> R {
        let interrupts_were_enabled = crate::arch::interrupts::disable();
        // SAFETY: `ttbr0` composes this space's own root (built by a `Mapper` over `Half::Low`,
        // with the kernel's high half shared into it by both constructors) with the ASID allocated
        // to it, which is every architecture's contract. The root lives as long as `self`, which
        // this call borrows until after the uninstall below, and nothing here runs at EL0 or
        // U-mode: `body` is kernel code on this kernel thread.
        unsafe { mmu::activate_user(self.ttbr0()) };
        let result = body();
        mmu::deactivate_user();
        crate::arch::interrupts::restore(interrupts_were_enabled);
        result
    }
}

/// The machine's ASID allocator (milestone 15; the crate carries the proofs). Taken alone, at
/// address-space creation and teardown, holding nothing else that matters; a leaf-adjacent rank.
pub(super) static ASIDS: crate::sync::IrqSafeMutex<address_space_identifier::Allocator> =
    crate::sync::IrqSafeMutex::new(
        crate::sync::rank::ASIDS,
        address_space_identifier::Allocator::new(),
    );

impl Drop for AddressSpace {
    fn drop(&mut self) {
        // Forget this space's revocation records before its tables are freed and reused: a stale
        // (root, va) would send a later revoke into someone else's tables.
        crate::revoke::forget_root(self.root.addr());

        // Stop being the live space before the frames go back, or the walker reads freed memory.
        // Cached translations are the `flush_asid` below, which reaches the other cores.
        if mmu::current_user_root() == self.root.addr() {
            mmu::deactivate_user();
        }

        // Only a region this space owns: a lent one is its capability holder's (`Backing`).
        if let Backing::Owned(region) = self.backing {
            crate::memory_region::destroy(region);
        }

        // The current-CPU and thread pages did not come from the region, so they are freed by
        // hand, now that the root is no longer live.
        if let Some((frame, _)) = self.current_cpu_page.take() {
            crate::memory::free(frame);
        }
        for (k, &addr) in self.thread_pages.iter().enumerate() {
            if self.thread_pages_made & (1 << k) != 0 {
                crate::memory::free(PageFrame::from_addr(addr));
            }
        }
        self.thread_pages_made = 0;

        // Invalidate every TLB entry wearing our tag, THEN free the tag: in the other order its next
        // owner could hit our stale translations.
        mmu::flush_asid(self.asid);
        ASIDS.lock().free(self.asid);
    }
}
