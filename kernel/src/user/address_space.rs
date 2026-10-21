//! **A user address space**: its page table, the frames it owns, its address-space identifier, and
//! what its `Drop` gives back.
//!
//! Cut out of `user.rs` unchanged by milestone 872 (the kernel's user-mode file is split along its
//! seams), which left the parent an index.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

use super::*;

/// A user address space: an L0 table for `TTBR0`, and every frame that hangs off it.
///
/// The `frames` vec holds **both** the pages we mapped and the intermediate page tables the
/// mapper allocated to reach them, because the allocator we hand the `Mapper` records
/// everything it hands out. That is the fix for the leak milestone 6 found the hard way
/// (`unmap_page` frees a leaf and leaves its L1/L2/L3 standing), applied *before* it bites:
/// an address space dies all at once, so we do not need `unmap` at all. We free the frames and
/// throw the whole table away.
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

    /// **The frame this space's thread reads its own CPU out of**, or `None` if it has none.
    ///
    /// calef ruled on 2026-09-21 that a thread observing *itself* is a per-thread page rather than
    /// a crossing, because the consumer is a memory allocator asking on every allocation. (That
    /// ruling's `design/decisions/` section is on another branch and not on `main` yet, so it is
    /// named here rather than cited.) `crates/current_cpu_contract` holds the layout and the
    /// argument; this field is the kernel's end of it.
    ///
    /// **This is slot 0 of the space's thread pages** (milestone 812 (`std::thread::spawn` runs real
    /// threads in one address space)): the page of a thread that is a process of its own, or of a
    /// process's first member. Every other member of a process has its own, in [`Self::thread_pages`].
    ///
    /// Allocated from the global frame allocator rather than retyped from `backing`'s region, which
    /// is the one place this differs from every other page a space owns. A lent region is sized by
    /// whoever lent it, and spending one more page of it unconditionally is exactly what cost two
    /// regressions when the timebase page tried it in `user_address_space_create`; the comment
    /// recording that is still beside that function. So `Drop` frees this frame by hand, the one
    /// thing in this struct that `memory_region::destroy` does not cover.
    ///
    /// **The frame and its direct-map address**, the second worked out once by
    /// `attach_current_cpu_page` rather than on every switch: release builds check overflow
    /// (notes/overflow-checks.md), so recomputing `phys_to_virt` in `publish_current_cpu` put a
    /// check on the hottest path that the attach had already passed for the same frame.
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

/// **Who returns the region an [`AddressSpace`] spends, and the reason this is a type rather
/// than a comment.**
///
/// A space is built two ways, and they differ in exactly this. [`AddressSpace::new`] carves its
/// own region out of the frame allocator, so nobody else has a name for it and its `Drop` is the
/// only thing that can ever free it. [`user_address_space_create`] is handed a region that the caller
/// already holds a `MemoryRegion` capability to (the `RETYPE_OBJ(ADDRESS_SPACE)` engine, milestone 19b), and
/// that caller reclaims it with `MemoryRegion::DESTROY`. **A lent region has two names for one run of
/// memory, and only one of them may free it.**
///
/// Until 2026-08-18 both cases stored a bare `u64` and `Drop` called `memory_region::destroy`
/// unconditionally, on the theory that a lent region is still pinned (`retype_object_page` pins,
/// `sched::reclaim_region` unpins) so the borrower's `destroy` is refused. That reasoning holds
/// only while the pin is still set, and `reclaim_region` clears it **before** the reaper's
/// deferred drop can land: `sched::finish_switch` hoists a dead thread's space out from under
/// `IPC_TABLES`, releases the lock, and only then drops it. Two `memory_region::destroy` calls for one
/// region then overlap, both pass the refusal check, and both free every page of the run. That is
/// the intermittent `double free of frame 0x82a3e000` in
/// `force_kill_tests::destroy_reclaims_a_region_whose_resident_is_blocked_in_receive`
/// (notes/object-revocation.md BUGS, one sighting in 45 runs on riscv64).
///
/// Making it a two-variant enum with the name inside is rung one of AGENTS.md's ladder: a space
/// cannot be constructed without saying who frees its region, so the borrower's `Drop` cannot
/// free memory it does not own even if the pin is gone.
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
/// than charging a second time.
///
/// One log page ([`crate::revoke::log_pages_for`]), which is 170 recorded mappings, and no table
/// pages, because a window of that size touches at most one 2 MiB L3 that the "handful of L3s"
/// above is already for. Every caller in this tree but one maps a handful of pages and lands
/// inside it.
///
/// **It is subtracted because charging it again is measurable and was measured.** The first
/// version of this accounting charged the full window and the aarch64 suite's frame ledger went
/// from 22249 kept frames to 22317: one frame per long-lived process, permanently, reserved into a
/// region that never used it, to fix a window that one caller has. A budget that is right for the
/// exceptional caller and wasteful for all sixty-eight ordinary ones is the wrong shape; what a
/// caller owes is the cost **above** what the overhead was always providing.
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

        // Unconditional, here rather than at the six places that build a process by hand, for the
        // reason `map_timebase_page` learned empirically: `load`'s own coverage misses every
        // kernel-built spawn, and each one found that out as a page fault. Every space this
        // function returns is a space a thread will run in, so this is the one place that covers
        // all of them and cannot be forgotten by a seventh.
        space.attach_current_cpu_page();

        Some(space)
    }

    /// **Give this space the page its thread reads its own CPU from**, if it has not got one.
    ///
    /// Idempotent, because two paths reach it: `new` for the spaces the kernel builds, and
    /// `sched::configure_thread_control_block` for the ones userspace builds and hands over. A
    /// space that goes through both gets one page.
    ///
    /// **Every failure is silent and leaves the space without a page**, which is deliberate and is
    /// the honest shape rather than the convenient one: a thread with no page reads an unmapped
    /// address, and a thread with a page reads the truth, but a `load` that failed outright because
    /// one frame was unavailable would turn a diagnostic convenience into a reason a program will
    /// not start. The states are told apart at the reader (`current_cpu_contract::CurrentCpuPage`
    /// answers `None`), which is where somebody can act on it.
    pub fn attach_current_cpu_page(&mut self) {
        if self.current_cpu_page.is_some() {
            return;
        }
        let Some(frame) = crate::memory::alloc_zeroed() else {
            return;
        };

        // Zeroed above, then stamped: the magic makes a prepared page tell itself from a frame
        // nobody wrote, and the sentinel makes "this thread has never run" a state rather than
        // CPU 0. `alloc_zeroed` rather than `alloc` because the *rest* of this frame is mapped
        // into the process too, and whatever the last owner left in it would go with it.
        // The allowance (milestone 812, §269 fork 7): the cores this process may use, which is the
        // online count until something grants fewer.
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
    /// One branch and one relaxed store when the space has a page, one branch when it has not. The
    /// ordering argument is `current_cpu_contract`'s and is not repeated here; its short form is
    /// that writer and reader are the same hardware thread, and that two successive writers are
    /// ordered by the scheduler's own release/acquire handoff rather than by anything this adds.
    ///
    /// The context switch reads [`BoundSpace::publish_current_cpu`] instead since §249, because the
    /// space lives in the registry and the thread keeps a copy; this form is the tests' way to make
    /// the same write by hand.
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

    /// Map an **existing** physical page into this address space, at `va`, with `flags`.
    ///
    /// The frame is **not** recorded for freeing, because we do not own it: it is either a
    /// device's MMIO (the PL011, for a console server) or a page **shared** with another address
    /// space (a message buffer). Freeing MMIO is meaningless, and freeing a shared page when one
    /// of its two holders dies would hand live memory to the allocator. So `Drop` leaves it
    /// alone. The intermediate page tables reaching it *are* recorded, exactly as in `map_new`,
    /// because those genuinely belong to this address space.
    ///
    /// This one function is what lets a driver leave the kernel: it is how the UART's registers
    /// get into a userspace server's address space, and how a shared buffer gets into both a
    /// client's and a server's.
    ///
    /// **The mapping is recorded** (`under`), because a mapping revocation cannot see is the
    /// DECISIONS §13 (capability revocation and untyped reclamation) use-after-free. Not freeing a
    /// frame (the paragraph above) and not being able to unmap it are unrelated. An unrecordable
    /// mapping is unmapped and refused as `OutOfPageFrames`, as at the `PageFrame::MAP` syscall.
    ///
    /// `under` says which capability's authority made this mapping, and it is a required argument
    /// with no default for [`crate::revoke::PageMapSource`]'s own reason (AGENTS.md's ladder, rung
    /// one). Every kernel-wiring caller passes `NoCapability`, truthfully: a [`Spawn`]`::maps`
    /// entry, a [`DeviceRun`], the initial ramdisk and the `x86_64` timebase page are all
    /// endowments the process holds no capability for, so the page stands as its own object and a
    /// single-page revoke of it finds the record. [`user_address_space_map`] is the one caller that
    /// passes a capability through, because the `MAP_INTO` syscall has one to pass.
    ///
    /// Every unmap sweep in `crate::revoke` is driven by the mapping log, so an unrecorded page
    /// here would survive `PageFrame::REVOKE` of a frame shared through `Spawn::maps` (the FS
    /// server's file channel, which [`boot_progenitor`] also grants the progenitor).
    /// `user::spawn_mapping_revocation_tests` is the falsification.
    ///
    /// # BUGS
    ///
    /// **[`Self::map_new`] still does not record**, and is deliberately left alone: its frames are
    /// retyped from this space's own backing region and freed with it, so no capability names them
    /// and no sweep can be asked about them. If that ever stops being true, this is the second half
    /// of the same hole.
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
    /// The plain form above takes the hold for itself and therefore holds it across `map_at` too,
    /// which it did not before 2026-10-04: one shape for both rather than a second body that maps
    /// outside the registry and records inside it.
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
    /// kernel thread can ask the hardware's own tables a question about this space
    /// (`mmu::translate_user`, `mmu::map_current_user_page_frame`) rather than reading back the
    /// kernel's record of them.
    ///
    /// The safe form of `mmu::activate_user` for every caller that wants exactly this (milestone 139
    /// (drive the unsafe count down), 2026-10-07 UTC; name provisional). Five call sites, all system
    /// tests, each restated that function's contract by hand; it is a fact
    /// about this type, so it is asserted once, here. A closure rather than a guard whose `Drop`
    /// uninstalls, because `mem::forget` is safe: a forgotten guard would end the borrow with the
    /// space still installed, and the space could then be dropped under a live root register.
    ///
    /// Not for a test that reads through the user translation itself or installs a space on
    /// another core: those carry their own arguments (`system_tests::user::tests` keeps both).
    ///
    /// **Interrupts are masked from the install to the uninstall, so `body` must be short and must
    /// not block or yield.** The scheduler does not know about this install: the calling kernel
    /// thread has no space of its own, so a switch back to it puts the reserved root in place of
    /// this one. A timer tick inside `body` that switched away and back therefore uninstalled the
    /// space under the question being asked, and `translate_user` answered for the kernel's empty
    /// low half instead. That was the x86_64 OVMF flake of 2026-10-13 (UTC), merge-group run
    /// 38345573542 ("shared page not mapped"), and it was the same race on all three
    /// architectures. Masking also keeps the thread on this core, which a migration would break the
    /// same way. `address_space_tests::an_installed_space_stays_installed_across_a_timer_tick` is
    /// the witness. A body that has to block would need the space bound to the thread
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
        // Drop this address space's entries from the revocation database (§13) before its page
        // tables are freed and reused: a stale (root, va) would send a later revoke to walk tables
        // that now belong to someone else.
        crate::revoke::forget_root(self.root.addr());

        // If we are the live address space, stop being it BEFORE the frames go back on the free
        // list. Otherwise the TTBR0 the CPU is walking points at memory the allocator has
        // already handed to somebody else, and the next low-half access reads whatever they put
        // there. This is about the *walker*, not the TLB: since milestone 58 neither ISA flushes
        // anything on a root switch, and the cached translations are dealt with by the `flush_asid`
        // at the bottom of this function, which is the half that has to reach the other cores.
        if mmu::current_user_root() == self.root.addr() {
            mmu::deactivate_user();
        }

        // One call: revoke anything delegated out of this region (nothing can be: the region
        // has no capability, so userspace could never retype from it), then return the whole
        // run, root and tables and leaves alike, to the allocator. This is the
        // "reclaim-on-process-death" wiring §13 deferred; the frame list it replaced is gone.
        //
        // **Only for a region we own.** A lent one (`user_address_space_create`) belongs to whoever
        // holds its `MemoryRegion` capability, and freeing it here is a double free of the whole run
        // the moment `sched::reclaim_region` has already unpinned it. `Backing` carries the
        // whole argument.
        if let Backing::Owned(region) = self.backing {
            crate::memory_region::destroy(region);
        }

        // The current-CPU page is the one frame this space owns that did NOT come out of its
        // region, so `destroy` above does not cover it and ownership has to do the work by hand.
        // Safe to do here, after the root is no longer live: nothing can read the mapping any
        // more, and the only writer was the context switch of a thread that is gone.
        if let Some((frame, _)) = self.current_cpu_page.take() {
            crate::memory::free(frame);
        }
        for (k, &addr) in self.thread_pages.iter().enumerate() {
            if self.thread_pages_made & (1 << k) != 0 {
                crate::memory::free(PageFrame::from_addr(addr));
            }
        }
        self.thread_pages_made = 0;

        // The ASID contract (crates/address_space_identifier): invalidate every TLB entry wearing our tag, THEN
        // hand the number back. In the other order, the next owner of this ASID could hit our
        // stale translations, which is exactly the bug tagging exists to prevent.
        mmu::flush_asid(self.asid);
        ASIDS.lock().free(self.asid);
    }
}
