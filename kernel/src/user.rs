//! Userspace. EL0. The actual operating system boundary.
//!
//! Everything before this was a Rust program that boots. From here on, the machine runs code
//! that **we did not compile and do not trust**, and the kernel's job stops being "do things"
//! and starts being "decide what is allowed."
//!
//! # Entering EL0 is returning from an exception that never happened
//!
//! There is no "drop to EL0" instruction. There is only `eret`, which restores whatever
//! `SPSR_EL1` says and jumps to `ELR_EL1`, and the exception level to return to is *in*
//! `SPSR_EL1`. So we do not need a new way down. We need a **fake way back**: fabricate a
//! [`TrapFrame`] with `SPSR = EL0t`, point `sp` at it, and fall into the `exception_restore`
//! that milestone 2 already wrote.
//!
//! This is the second time the project has pulled exactly this trick. `Thread::spawn_into` fakes a
//! `switch_to` frame so that the `ret` which *resumes* a thread also *starts* one
//! (notes/threads.md). Both times the "start" path turned out to be the "resume" path with a
//! forged frame, and no new code at all.
//!
//! # What milestone 4 already paid for
//!
//! The kernel lives entirely in `TTBR1`, at `0xffff_...`. Userspace lives in `TTBR0`, at
//! `0x0000_...`. **The hardware picks the table register from bits 63:48 of the address**, so:
//!
//! - The kernel is mapped in every address space, for free. Nobody had to copy anything.
//! - A syscall **does not switch page tables**. There is nothing to sync and nothing to remap.
//! - Installing a process is one `msr ttbr0_el1`.
//!
//! None of that was written for milestone 7. It fell out of a higher-half decision made three
//! milestones ago, and `Flags::user_code()` / `Flags::user_data()` have been sitting in the
//! `paging` crate, unused, waiting for today.
//!
//! # What is deliberately NOT here
//!
//! **A syscall ABI.** The user program below executes `svc #0` and asks for nothing. There is
//! no syscall number, no argument convention, no return value. DECISIONS §10 chose
//! capabilities, and the syscall surface gets designed against a capability table at 7d, in one
//! piece, on purpose. Not accreted here because it was convenient.

use elf::Elf;
use page_frames::{FRAME_SIZE, PageFrame};
use paging::{Flags, Half, MapError, Mapper};

use crate::arch::exceptions::{TrapFrame, enter_user};
use crate::arch::mmu::{self, phys_to_ptr};
use crate::arch::sync_icache;
use crate::memory;

/// Where a user program's stack goes. One page, and `sp` starts at the top of it: stacks grow down.
/// A program given a deeper stack gets the rest mapped below this, one page at a time.
///
/// **Derived from `address_space_map`**, the band the map gives the stack, rather than chosen here.
/// It was `0x50_0000` until 2026-09-26 (milestone 206 (a program image has under 896 KiB)), directly above an image linked at
/// `0x40_0000`, which left a program image under 896 KiB; the map moved the image and the stack
/// together to the top of the second gigabyte.
///
/// There is no matching `USER_CODE_VA` any more: it existed for `exec`, the one-page raw
/// machine-code loader the hand-assembled programs needed, and every program the kernel runs now
/// names its own load address in its ELF header, which [`load`] checks against the map's image band.
pub const USER_STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
pub const USER_STACK_TOP: u64 = address_space_map::STACK_TOP;
const _: () = assert!(USER_STACK_TOP == USER_STACK_VA + FRAME_SIZE);

/// A user address space: an L0 table for `TTBR0`, and every frame that hangs off it.
///
/// The `frames` vec holds **both** the pages we mapped and the intermediate page tables the
/// mapper allocated to reach them, because the allocator we hand the `Mapper` records
/// everything it hands out. That is the fix for the leak milestone 6 found the hard way
/// (`unmap_page` frees a leaf and leaves its L1/L2/L3 standing), applied *before* it bites:
/// an address space dies all at once, so we do not need `unmap` at all. We free the frames and
/// throw the whole table away.
pub struct AddressSpace {
    root: PageFrame,
    /// **This address space's TLB tag, for life** (milestone 15; `crates/address_space_identifier`). Every user
    /// mapping is `nG`, so its TLB entries carry this number, and a context switch flushes
    /// nothing: the other spaces' entries just stop matching. Freed at drop, after
    /// `flush_asid` has made every entry so tagged vanish, which is what makes the number
    /// reusable.
    asid: u16,
    /// **The untyped region every page of this address space comes from, and who frees it**
    /// (milestone 14 phase B.4): the root table, the intermediate tables, and every owned leaf
    /// are retyped out of one region. The region *is* the record of what this address space
    /// owns, which is why there is no frame list: teardown is `memory_region::destroy`, one call,
    /// made safe by §13 revocation.
    ///
    /// It carries the owner rather than just the name because **two different things build an
    /// address space and only one of them owns its memory**. See [`Backing`].
    backing: Backing,

    /// **The frame this space's thread reads its own CPU out of**, or `None` if it has none.
    ///
    /// calef ruled on 2026-09-21 that a thread observing *itself* is a per-thread page rather than
    /// a crossing, because the consumer is a memory allocator asking on every allocation. (That
    /// ruling's `design/decisions/` section is on another branch and not on `main` yet, so it is
    /// named here rather than cited.) `crates/current_cpu_protocol` holds the layout and the
    /// argument; this field is the kernel's end of it.
    ///
    /// **Per address space is per thread only because of §105 (`std::thread::spawn` stays
    /// declined)**: `Tcb::CONFIGURE` refuses a space already bound to a thread (§249's amendment
    /// (b)), so no two TCBs share one space. The crate's `BUGS` section carries what has to change if that ever stops being true.
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
    current_cpu_page: Option<(PageFrame, u64)>,
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
enum Backing {
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
const WINDOW_IN_OVERHEAD: u64 = 1;

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
    /// not start. The states are told apart at the reader (`current_cpu_protocol::CurrentCpuPage`
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
        let bytes = current_cpu_protocol::build_page();
        let kernel_va = mmu::phys_to_virt(frame.addr());
        // SAFETY: `frame` is freshly allocated and owned by nobody else yet, the direct map is
        // valid for it, and `PAGE_BYTES` (16) is far under `FRAME_SIZE`, so the copy stays inside
        // the frame.
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), kernel_va as *mut u8, bytes.len());
        };

        // Read-only to the process, which is what keeps this out of Tock's necessarily-unsafe
        // kernel category (notes/trusted-base.md): the kernel writes a frame it owns and lends the
        // process a view, rather than writing into memory the process supplied.
        if self
            .map_physical(
                current_cpu_protocol::PAGE_VA,
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
    /// ordering argument is `current_cpu_protocol`'s and is not repeated here; its short form is
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
            unsafe { current_cpu_protocol::publish(kernel_va, cpu) };
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
    /// DECISIONS §13 (capability revocation and untyped reclamation) use-after-free, and until
    /// 2026-09-21 this function was the one mapping site in the kernel that recorded nothing. The
    /// paragraph above is about `Drop` and frame *ownership*, which is a different question that a
    /// reader can easily take this for: not freeing a frame and not being able to unmap it are
    /// unrelated, and the tree read the first as covering the second for as long as this function
    /// existed. An unrecordable mapping is unmapped and refused as `OutOfPageFrames`, exactly as at
    /// the `PageFrame::MAP` syscall, because the alternative is a mapping no sweep can reach.
    ///
    /// `under` says which capability's authority made this mapping, and it is a required argument
    /// with no default for [`crate::revoke::PageMapSource`]'s own reason (AGENTS.md's ladder, rung
    /// one). Every kernel-wiring caller passes `NoCapability`, truthfully: a [`Spawn`]`::maps`
    /// entry, a [`DeviceRun`], the initrd and the `x86_64` timebase page are all endowments the
    /// process holds no capability for, so the page stands as its own object and a single-page
    /// revoke of it finds the record. [`user_address_space_map`] is the one caller that passes a
    /// capability through, because the `MAP_INTO` syscall has one to pass.
    ///
    /// **What the defect was, since a reader will meet the fix without the failure.** Every unmap
    /// sweep in `crate::revoke` is driven by the mapping log, so a page wired here was invisible to
    /// `PageFrame::REVOKE`, `DeviceFrame::REVOKE` and `MemoryRegion::DESTROY` alike: the capability
    /// went and the mapping stayed. Recorded by risk 7's adversarial pass as latent; it was not.
    /// [`fs_service::spawn_fs_server`](crate::user::fs_service) wires the file channel's shared
    /// pages into the FS server through `Spawn::maps`, and [`boot_progenitor`] hands the progenitor
    /// `PageFrame(file_shared, 1)` with `GRANT` over the first of them, so `PageFrame::REVOKE` on
    /// that slot left the FS server writing to a page the progenitor had just un-shared.
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
    /// (drive the unsafe count down), 2026-10-07 UTC; name provisional). Six call sites, the boot
    /// tour and five system tests, each restated that function's contract by hand; it is a fact
    /// about this type, so it is asserted once, here. A closure rather than a guard whose `Drop`
    /// uninstalls, because `mem::forget` is safe: a forgotten guard would end the borrow with the
    /// space still installed, and the space could then be dropped under a live root register.
    ///
    /// Not for a test that reads through the user translation itself or installs a space on
    /// another core: those carry their own arguments (`system_tests::user::tests` keeps both).
    #[cfg_attr(
        not(any(feature = "system_tests", target_arch = "riscv64")),
        allow(dead_code)
    )]
    pub fn while_installed<R>(&self, body: impl FnOnce() -> R) -> R {
        // SAFETY: `ttbr0` composes this space's own root (built by a `Mapper` over `Half::Low`,
        // with the kernel's high half shared into it by both constructors) with the ASID allocated
        // to it, which is every architecture's contract. The root lives as long as `self`, which
        // this call borrows until after the uninstall below, and nothing here runs at EL0 or
        // U-mode: `body` is kernel code on this kernel thread.
        unsafe { mmu::activate_user(self.ttbr0()) };
        let result = body();
        mmu::deactivate_user();
        result
    }
}

/// The machine's ASID allocator (milestone 15; the crate carries the proofs). Taken alone, at
/// address-space creation and teardown, holding nothing else that matters; a leaf-adjacent rank.
static ASIDS: crate::sync::IrqSafeMutex<address_space_identifier::Allocator> =
    crate::sync::IrqSafeMutex::new(
        crate::sync::rank::ASIDS,
        address_space_identifier::Allocator::new(),
    );

/// **How many address spaces the registry can name at once: every one the machine can hold.**
///
/// §249 (a running address space stays nameable) made the registry the owner of every space, bound
/// to a thread or not, so a running process now holds an entry for as long as it lives. Until then
/// this was 32 and named only spaces under construction, which was right while `CONFIGURE` moved a
/// space out into its thread.
///
/// **The number is [`crate::revoke::MAX_SPACES`], and that is an argument rather than a
/// coincidence.** Every [`AddressSpace`] registers with the revocation registry when it is built
/// and leaves it only when it drops, so no more than that many spaces can exist at once, and an
/// entry here holds exactly one. Sized the same, this table cannot fill while a space that wants an
/// entry exists, which is what lets [`register_bound_address_space`] treat a full table as a broken
/// invariant rather than an error every kernel spawn path would have to carry. The `const` assert
/// below keeps the two from drifting apart; the revocation registry's own constant is the thread
/// ceiling plus headroom, so the argument follows the thread ceiling when it moves.
///
/// What it costs, measured from the type rather than estimated: `registry_footprint`, which
/// `system_tests`' `running_space_tests` prints, is 21,904 bytes of `.bss` on all three
/// architectures at 288 slots (at 32 it was about a ninth of that; not measured then): 72 bytes an entry and a 4-byte
/// generation each. That is the whole of what every process holding an entry needs, because the
/// space's pages were always paid from its own region. Nothing walks the empty slots:
/// `generational_table::Table` bounds every sweep by its highest live slot, and the context switch
/// never reads this table at all ([`BoundSpace`]).
const MAX_USER_SPACES: usize = crate::revoke::MAX_SPACES;
const _: () = assert!(MAX_USER_SPACES >= crate::revoke::MAX_SPACES);

/// **One registry entry: a space, and the thread bound to it if there is one** (§249; name
/// provisional).
///
/// `bound` is ruling (b) of §249 made a field: `CONFIGURE` used to be stopped from binding a space
/// twice only because it consumed the one name the space had, and §105 (`std::thread::spawn` stays
/// declined) rested on that. The name survives a bind now, so the refusal is stated here instead,
/// and [`bind_user_address_space`] reads it.
struct Registered {
    space: AddressSpace,
    bound: Option<crate::thread::ThreadId>,
}

/// **What a thread keeps of the space it is bound to** (§249; name provisional): the registry's
/// name for it, and the three values the context switch reads, copied out once at bind time.
///
/// The space itself lives in the registry, which owns it. The switch reads this instead, because
/// it runs under `IPC_TABLES` and the registry's lock ranks above that one, so the switch could not
/// take it, and should not want to on the hottest path in the kernel. All three are immutable for
/// the life of the space: a space's root and its TLB tag are fixed at creation, and its current-CPU
/// page is attached before the bind and freed only by its `Drop`.
///
/// **What makes the copy safe is who may drop the space**, and that is [`reap_address_spaces_in_region`]'s
/// and the reaper's rule, not this type's: a bound space is dropped only once its thread can never
/// be switched in again (it is gone from the thread table, or it is a corpse no core is standing
/// on). So no switch ever reads a `BoundSpace` whose space has been freed.
#[derive(Clone, Copy)]
pub struct BoundSpace {
    name: u64,
    root: u64,
    ttbr0: u64,
    current_cpu_page: Option<u64>,
}

impl BoundSpace {
    fn of(name: u64, space: &AddressSpace) -> Self {
        BoundSpace {
            name,
            root: space.root(),
            ttbr0: space.ttbr0(),
            current_cpu_page: space.current_cpu_page.map(|(_, kernel_va)| kernel_va),
        }
    }

    /// The registry's name for the space, which is what a thread's reaper takes it out by.
    pub fn name(&self) -> u64 {
        self.name
    }

    /// The physical address of the space's root table.
    pub fn root(&self) -> u64 {
        self.root
    }

    /// The composed `TTBR0_EL1` (or `satp`, or `CR3`) value the context switch installs.
    pub fn ttbr0(&self) -> u64 {
        self.ttbr0
    }

    /// `AddressSpace::publish_current_cpu`, from the copy: one branch and one relaxed store.
    #[inline]
    pub fn publish_current_cpu(&self, cpu: u64) {
        if let Some(kernel_va) = self.current_cpu_page {
            // SAFETY: the frame is the bound space's own, freed only by that space's `Drop`, and
            // the space is dropped only after its thread can never be switched in again (this
            // type's own doc). The caller is the one core switching this thread in, so it is the
            // only writer for as long as the store takes.
            unsafe { current_cpu_protocol::publish(kernel_va, cpu) };
        }
    }
}

/// **The address-space registry** (milestone 19b (run a real workload); since §249, the owner of every space a capability
/// or a thread can name): the kernel-side records behind `Object::AddressSpace` capabilities, named
/// generationally like everything since milestone 14 (kernel objects from untyped). The `AddressSpace` in the slot is the same
/// type exec builds, so every mechanism that works on a process's space (region-paid tables,
/// revocation logs, ASID tagging) works on a user-built one identically.
///
/// **Two removers, and a removal is take-once.** A space leaves when its thread is reaped
/// (`sched::reap_switched_out`, by the name in the thread's [`BoundSpace`]) or when the region
/// sweep takes it ([`reap_address_spaces_in_region`]). Both go through `Table::remove`, which hands
/// the space out once and makes the name dead in the same step, so whichever comes second finds
/// nothing and a double free is not representable. Deleting a capability removes nothing (§249's
/// amendment (a)): a capability is a name, and it simply stops resolving once the space is gone.
static USER_SPACES: crate::sync::IrqSafeMutex<
    generational_table::Table<Registered, MAX_USER_SPACES>,
> = crate::sync::IrqSafeMutex::new(
    crate::sync::rank::ADDRESS_SPACES,
    generational_table::Table::new(),
);

/// **The registry's static footprint in bytes**, for the boot that reports it (§249's
/// `MAX_USER_SPACES` raise; name provisional).
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const fn registry_footprint() -> usize {
    size_of::<generational_table::Table<Registered, MAX_USER_SPACES>>()
}

/// Create an address space **in and backed by** `region` (the `RETYPE_OBJ(ADDRESS_SPACE)` engine): the
/// root page is retyped from it (pinning it, atomically with the carve), and the region becomes
/// the space's table-and-record budget, exactly as for an exec-built space. `None` on an
/// exhausted region, a full registry, or ASID exhaustion (unreachable; the type is honest).
pub fn user_address_space_create(region: u64) -> Option<u64> {
    let root = crate::memory_region::retype_object_page(
        region,
        crate::memory_region::ObjectKind::AddressSpace,
    )?;
    mmu::share_kernel_half(root); // RISC-V single-satp: the process root carries the kernel high half

    // The tag first, because the registry records it (see `AddressSpace::new`).
    let asid = ASIDS.lock().alloc()?;
    if !crate::revoke::register_space(root, region, asid) {
        ASIDS.lock().free(asid);
        return None; // registry full; the carved page is spent, the caller's own loss (B.4 rule)
    }

    let space = AddressSpace {
        root: PageFrame::from_addr(root),
        asid,
        // Lent, not owned: the caller holds the `MemoryRegion` capability to this region and reclaims
        // it with `DESTROY`. See `Backing` for the double free that taught us to say so.
        backing: Backing::Lent(region),
        // **Not attached here**, for the same reason the timebase page is not mapped here (the
        // comment below): this syscall serves every purpose that wants a bare address-space
        // object, most of which never run a thread and some of which are sized to the page. The
        // space gets its page when a TCB binds it, in `bind_user_address_space`,
        // which is the moment it becomes a thread's space and therefore the moment the question
        // "which CPU am I on" starts having an answer.
        current_cpu_page: None,
    };

    // The timebase page is **not** mapped unconditionally here (an earlier version of this
    // lane's work did, and a full-suite run under `script/test --arch x86_64` caught two
    // regressions: the hand-sized demo region of the test now named
    // `a_process_composed_from_two_capabilities_runs_in_the_space_it_built` ran out of table
    // budget, and it makes no sense for the many callers of this syscall that build
    // nothing resembling a real ELF process at all). This syscall is shared by every purpose that
    // needs a bare address space object, not only the userspace ELF loader
    // (`supervision_protocol::build_child_space`), and the loader is where this page actually
    // belongs: see that crate's own code for the targeted fix, which writes the page from the rate
    // the *parent* already holds, so a child reads its parent's measured number and a parent that
    // knows nothing hands down nothing rather than a plausible constant.
    let name = USER_SPACES
        .lock()
        .insert_with(|_| Registered { space, bound: None });
    if name.is_none() {
        // Undo the bookkeeping; the page stays spent on the caller's budget. (Unreachable while
        // `MAX_USER_SPACES` covers the revocation registry, which `register_space` above already
        // admitted this space to; kept because the value would otherwise be dropped unregistered.)
        crate::revoke::forget_root(root);
        ASIDS.lock().free(asid);
    }
    name
}

/// Map `phys` into the user-built space `name` at `va`, one page under one hold. Tables and the
/// §13 record come from the space's own backing region; an unrecordable mapping is unmapped and
/// refused, exactly as at the `page_frame::MAP` syscall, because a mapping revocation cannot see is
/// the §13 use-after-free.
///
/// `under` says which capability's authority this mapping was made with, which is what scopes a
/// later `PageFrame::REVOKE` to that capability's derivation family rather than to the physical
/// page (DECISIONS §132). The `MAP_INTO` syscall passes the invoked frame capability's object; the
/// kernel's own callers, which build a space directly out of a region, pass
/// `PageMapSource::NoCapability`.
// Not `MAP_INTO`'s engine since 2026-10-04 (it maps under one `MappingHold` through
// `with_user_address_space`); the kernel's own wiring calls it only in test builds, and the system
// tests build spaces with it.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn user_address_space_map(
    name: u64,
    va: u64,
    phys: u64,
    flags: Flags,
    under: crate::revoke::PageMapSource,
) -> Result<(), MapError> {
    let mut spaces = USER_SPACES.lock();
    let space = &mut spaces.get_mut(name).ok_or(MapError::NotMapped)?.space;

    // `map_physical` maps and records in one step since 2026-09-21, including the unmap-and-refuse
    // on an unrecordable mapping that used to live here: this function was the one caller that
    // remembered to record, which is exactly why it is now the one caller with nothing extra to
    // remember. See that function's own docs for the defect the other callers carried.
    space.map_physical(va, phys, flags, under)?;
    // A code page a loader just filled via data writes (milestone 19d): the instruction fetcher
    // has its own cache and has never heard of those bytes. On aarch64 the I-cache is not
    // coherent with the D-cache, so make it so now, via the frame's direct-map VA (any VA that
    // maps the physical page works; caches are PIPT to the point of unification). Without this,
    // the child fetches whatever was in the frame before the loader wrote its program.
    if flags.is_user_executable() {
        sync_icache(mmu::phys_to_virt(phys), FRAME_SIZE as usize);
    }
    Ok(())
}

/// The root table of a space the registry names, bound or not.
///
/// Built for tests (so a walker can ask what a space really maps) and now also
/// `abi::address_space::LIST`'s way in (milestone 126's `pmap`, DECISIONS §114): the syscall handler
/// resolves the capability's `name` to a root here before consulting `revoke::list_mapping` and
/// `arch::mmu::translate_at`. `None` once the space is gone: its thread was reaped or its region
/// destroyed (§249). A `LIST` against a capability that outlived its space reads as "nothing to
/// report," the same as an empty space, because the capability itself was never refused and the
/// kernel has nothing left to say about where it used to point.
pub fn user_address_space_root(name: u64) -> Option<u64> {
    USER_SPACES.lock().get(name).map(|e| e.space.root())
}

/// **Run `f` with the space `name` under the registry lock**, `None` if the name does not
/// resolve. `AddressSpace::MAP_INTO`'s and `UNMAP`'s way in (the map-revocation-window lane,
/// 2026-10-04 UTC; name provisional): it must hold this registry (`ADDRESS_SPACES`, 61) *above* the
/// mapping registry (`MAPPINGS`, 59) for its whole run, so it cannot go through
/// [`user_address_space_map`], which takes and drops this lock once per page. The `Option` is
/// handed in rather than checked here so the caller keeps its own refusal order: a frame that is
/// not there answers before a space that is not there, as it always has.
///
/// Since §249 a running space resolves here too, so a mapping changed through it may be one a core
/// is translating through right now. That is why `UNMAP` unmaps with the function every revoke uses,
/// whose TLB obligation reaches every core.
pub fn with_user_address_space<R>(name: u64, f: impl FnOnce(Option<&mut AddressSpace>) -> R) -> R {
    let mut spaces = USER_SPACES.lock();
    f(spaces.get_mut(name).map(|e| &mut e.space))
}

/// **Bind the space `name` to the embryo `tid`** (`ThreadControlBlock::CONFIGURE`'s engine since
/// §249; name provisional). The space stays in the registry and keeps its name, which is the whole
/// of §249's option A: a copy of the capability made before `CONFIGURE` still names the space while
/// its thread runs.
///
/// `NoSuchSlot` if the name does not resolve. **`WrongObject` if the space is already bound**, which
/// is §249's amendment (b), the answer a second `CONFIGURE` of a started thread already gives:
/// §105 (`std::thread::spawn` stays declined) stands because this refusal is stated rather than
/// inherited from a consumed name.
///
/// `bind` is called with the copy the thread will keep, under this registry's lock, and takes
/// `IPC_TABLES` itself (60, below this one's 61, so the nesting is the rank order's own direction).
/// That makes the bind one critical section across both tables: no region sweep can see a space
/// marked bound to a thread that does not have it, or the reverse. If `bind` refuses, the entry is
/// left unbound. The current-CPU page is attached first, because the copy carries its address; a
/// space that goes on to be refused keeps it, which is harmless (the attach is idempotent, and the
/// page dies with the space).
pub fn bind_user_address_space(
    name: u64,
    tid: crate::thread::ThreadId,
    bind: impl FnOnce(BoundSpace) -> Result<(), abi::Error>,
) -> Result<(), abi::Error> {
    let mut spaces = USER_SPACES.lock();
    let entry = spaces.get_mut(name).ok_or(abi::Error::NoSuchSlot)?;
    if entry.bound.is_some() {
        return Err(abi::Error::WrongObject);
    }
    entry.space.attach_current_cpu_page();
    bind(BoundSpace::of(name, &entry.space))?;
    entry.bound = Some(tid);
    Ok(())
}

/// **Put a space the kernel built into the registry, already bound to `tid`** (`sched::adopt_address_space`'s
/// half, §249; name provisional), and return the copy the thread keeps.
///
/// Infallible because it cannot fail: [`MAX_USER_SPACES`] covers every space the revocation
/// registry can hold, and this one is registered there. A full table here is a broken invariant,
/// not a resource limit, so it panics with that sentence rather than handing every kernel spawn
/// path an error it could never act on.
pub fn register_bound_address_space(
    space: AddressSpace,
    tid: crate::thread::ThreadId,
) -> BoundSpace {
    let mut spaces = USER_SPACES.lock();
    let mut copy = None;
    spaces
        .insert_with(|name| {
            copy = Some(BoundSpace::of(name, &space));
            Registered {
                space,
                bound: Some(tid),
            }
        })
        .expect("the address-space registry is full, which MAX_USER_SPACES says cannot happen");
    copy.expect("insert_with names the entry before it stores it")
}

/// **Take a space out of the registry**: the reaper's half of the take-once removal (§249), and the
/// system tests' way of ending a space without a thread. `None` if the name does not resolve, which
/// for the reaper means the region sweep took it first. The space drops in the caller, outside this
/// registry's lock, because its `Drop` takes the revocation, region and ASID locks.
pub fn take_user_address_space(name: u64) -> Option<AddressSpace> {
    USER_SPACES.lock().remove(name).map(|e| e.space)
}

/// **Tear down every space a region's destruction ends** (object revocation, the address-space
/// case; widened by §249). Each removed `AddressSpace` drops here, and its `Drop` forgets its
/// revocation records and frees its ASID (its region's memory comes back at the enclosing
/// `reclaim_region`, which unpins after this).
///
/// Three kinds of entry go, and the registry owning bound spaces (§249) is what made the second and
/// third reachable here at all:
///
/// 1. **An unbound space whose root is in `[base, end)`**: built from the region, never bound.
/// 2. **A bound space whose thread is gone from the thread table**, wherever its root lives. This is
///    a thread `reap_region_objects` just removed because its TCB was in the region: removing a
///    `Thread` drops nothing of its space now, so the space is collected here, in the same
///    `reclaim_region`, before the region is unpinned.
/// 3. **A bound space whose root is in the span and whose thread is a corpse no core stands on**
///    (`Dead` or `Finished`, off its stack). The corpse can never be switched in again, so taking its
///    space is safe, and it is what closes the gap `notes/naming-a-running-address-space.md` found
///    by reading: a corpse whose TCB is outside the region used to keep its space, so the region came
///    back while the corpse still owned the root, and the corpse's later drop forgot the revocation
///    records of whoever was given that page next. Its reaper now finds the name dead and drops
///    nothing.
///
/// **A bound space whose thread can still run is left alone**, root in the span or not. That is the
/// hole milestone 765 (a destroyed region cannot free the root a running thread walks) closes, by
/// making `reap_region_objects` refuse and kill such a thread first; until then it stands exactly as
/// it stood before §249, recorded at `revoke::revoke_region`. A corpse still standing on its stack
/// (`on_cpu`) is counted with the runnable ones for the same reason: a core still has its root
/// installed.
///
/// Takes the registry lock and, under it, `IPC_TABLES` once per scan to ask about the threads (61
/// then 60, the rank order's direction). Never drops an `AddressSpace` under either.
pub fn reap_address_spaces_in_region(base: u64, end: u64) {
    loop {
        let victim = {
            let spaces = USER_SPACES.lock();
            crate::sched::with_binders(|binder| {
                spaces.iter().find_map(|(name, entry)| {
                    let root = entry.space.root.addr();
                    let in_span = base <= root && root < end;
                    let goes = match entry.bound {
                        None => in_span,
                        Some(tid) => match binder(tid) {
                            crate::sched::Binder::Gone => true,
                            crate::sched::Binder::Corpse => in_span,
                            crate::sched::Binder::CanRun => false,
                        },
                    };
                    goes.then_some(name)
                })
            })
        };
        let Some(name) = victim else { break };
        // `remove` returns the entry; the registry lock is released at the `;`, then the space drops.
        let space = USER_SPACES.lock().remove(name);
        drop(space);
    }
}

/// Put a space the kernel built into the registry, unbound, and return its name: how a kernel
/// spawn path that builds a process the way userspace does (`CONFIGURE` by name) gets one. Named for
/// milestone 19c.3's unwind path, which it no longer serves: since §249 a refused bind leaves the
/// space where it was.
pub fn readopt_user_address_space(space: AddressSpace) -> Option<u64> {
    USER_SPACES
        .lock()
        .insert_with(|_| Registered { space, bound: None })
}

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

        // The ASID contract (crates/address_space_identifier): invalidate every TLB entry wearing our tag, THEN
        // hand the number back. In the other order, the next owner of this ASID could hit our
        // stale translations, which is exactly the bug tagging exists to prevent.
        mmu::flush_asid(self.asid);
        ASIDS.lock().free(self.asid);
    }
}

/// Why a binary was refused.
///
/// **A bad user program must not be a kernel panic.** Every one of these is a thing a file can
/// simply *say*, and the answer is to decline and kill the thread, not to take the machine down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    /// The file is not an aarch64 static ELF we are willing to run. See `elf::Error`.
    NotLoadable(elf::Error),

    /// It asked to be loaded somewhere it may not go.
    ///
    /// **Including a KERNEL address.** An ELF gets to name its own load address, so this is
    /// exactly the thing a hostile binary tries: ask to be mapped over the kernel. It is
    /// refused by construction rather than by a check, because the `Mapper` is built with
    /// `Half::Low` and a high address is not a thing it can express (`MapError::WrongHalf`).
    Unmappable(MapError),

    /// **It does not fit the address-space map's image band**: too large, which names the image's
    /// end and the stack's base, or linked somewhere else entirely (milestone 206, DECISIONS §171 (where a program image starts)
    /// option A). Refused before a page is mapped. Until 2026-09-26 an image too large for its band
    /// was reported as `Unmappable(AlreadyMapped)` from the first stack page it collided with, which
    /// named an overlap and not a size, and nobody hitting it learned what was wrong.
    ///
    /// Its `Display` is the sentence; the boot prints that rather than the `Debug` form.
    Misplaced(address_space_map::ImagePlacement),
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::Misplaced(p) => write!(f, "{p}"),
            other => write!(f, "{other:?}"),
        }
    }
}

/// **Refuse an image the address-space map has no room for**, before anything is mapped.
///
/// An image that starts in the kernel's half is left to the `Mapper`, which refuses a kernel-half
/// address by construction (`MapError::WrongHalf`) and must keep being the thing that does:
/// `an_elf_that_asks_to_be_loaded_over_the_kernel_is_refused` proves the construction, and a band
/// check in front of it would prove only the check. Every other image the map has no room for,
/// including one linked for the old layout at `0x40_0000`, is refused here.
fn check_image_band(elf: &Elf) -> Result<(), LoadError> {
    let mut lo = u64::MAX;
    let mut hi = 0;
    for seg in elf.segments() {
        let (start, end) = seg.page_range(FRAME_SIZE);
        lo = lo.min(start);
        hi = hi.max(end);
    }
    if lo == u64::MAX || mmu::KERNEL_VA_BASE <= lo {
        return Ok(());
    }
    address_space_map::check_image(lo, hi).map_err(LoadError::Misplaced)
}

/// Parse an ELF, build an address space, and put it in memory. Do **not** run it.
///
/// Split out from [`run`] on purpose: this is the part that can fail, so it is the part a test can
/// call without dying (`run` diverges into the new process, or into `exit`).
///
/// **`windowed` is how many pages the caller is about to map into the new space itself**, and a
/// caller that maps nothing passes zero.
///
/// **A `Spawn::maps` page is not free**, and since 2026-09-21 it is less free than it was: it costs
/// a share of an intermediate table and, now that `map_physical` records, a share of a log page,
/// both out of this space's own region. `AS_OVERHEAD`'s sixteen pages absorb that for the handful
/// of pages most callers map and do not absorb it for the two that map thousands: the progenitor's
/// archive window, which pays for itself at its own call site, and the installer's copy of the boot
/// file, which did not and could not, because it goes through [`run`] and [`run`] had nowhere to
/// put the number.
///
/// So the accounting is done **here**, from `spawn.maps` itself, rather than asked of every caller.
/// That is AGENTS.md's ladder read downward: a budget a caller must remember to widen is a budget
/// that is wrong the first time somebody maps a bigger window, and the failure it produces is an
/// `OutOfPageFrames` panic in a spawn three frames away from anything that mentions memory.
///
/// What is charged is the cost **above** [`WINDOW_IN_OVERHEAD`], for the reason recorded there: a
/// handful of mapped pages has always been paid for out of `AS_OVERHEAD`'s margin, and charging it
/// twice costs a frame per process forever.
pub fn load(image: &[u8], windowed: u64) -> Result<(AddressSpace, u64), LoadError> {
    let elf = Elf::parse(image).map_err(LoadError::NotLoadable)?;
    check_image_band(&elf)?;

    // The budget, counted from the file before anything is carved: every segment's pages, plus
    // one for the stack, plus what the caller's own windows will cost. (AS_OVERHEAD covers the
    // tables for everything else.) A binary that lies about its size simply exhausts its own
    // region and fails to map, spending nobody else's memory.
    let content: u64 = elf
        .segments()
        .map(|seg| {
            let (start, end) = seg.page_range(FRAME_SIZE);
            (end - start) / FRAME_SIZE
        })
        .sum::<u64>()
        + 1
        + (windowed / 512 + crate::revoke::log_pages_for(windowed))
            .saturating_sub(WINDOW_IN_OVERHEAD);

    let mut space =
        AddressSpace::new(content).ok_or(LoadError::Unmappable(MapError::OutOfPageFrames))?;

    map_segments(&mut space, &elf)?;

    space
        .map_new(USER_STACK_VA, Flags::user_data())
        .map_err(LoadError::Unmappable)?;

    // The timebase page, from milestone 161 (the kernel port) and its `cntfrq` follow-up,
    // widened to riscv64 on 2026-09-21: the
    // one number `user_mode_runtime::now()` needs on an architecture with no `CNTFRQ_EL0` to read it
    // from. `x86_64` measures it; `riscv64` reads it out of the device tree, which is privileged
    // knowledge a process has no way to reach. `map_physical` does not
    // spend `content`'s budget (only the intermediate table pages it walks come from the space's
    // own region, the same as every `Spawn::maps` entry `run()` applies below), so this needs no
    // extra accounting here. Unconditional, the same "grant is unconditional, a zeroed page reads
    // as unknown" shape `boot_clock_page` already uses: see `timebase_page_phys`'s own docs.
    #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
    if let Some(phys) = timebase_page_phys() {
        space
            .map_physical(
                counter_frequency_protocol::PAGE_VA,
                phys,
                Flags::user_rodata(),
                crate::revoke::PageMapSource::NoCapability,
            )
            .map_err(LoadError::Unmappable)?;
    }

    Ok((space, elf.entry()))
}

/// **The timebase page's one physical frame**, computed and written once, then reused for
/// every process `load` maps it into. The frequency the kernel measured (`x86_64`) or read from the
/// device tree (`riscv64`)
/// does not change while the machine runs, so one frame mapped read-only into every address space
/// is correct rather than merely convenient: there is only ever one true answer to publish.
///
/// **aarch64 has no such frame and needs none**: `CNTFRQ_EL0` is architected, the kernel opens it to
/// EL0, and a register the machine itself states cannot go stale between the kernel reading it and
/// a process reading it.
///
/// `None` only if the frame allocator is out of memory (propagated by `load` as the same
/// `OutOfFrames` a segment that would not fit reports; this is not a bad-program condition, so it
/// is not a panic). If [`crate::arch::timer::frequency_checked`] has not resolved yet (never
/// observed: `init_frequency` runs early in both boot tours, well before the first call to
/// `load`), the frame is allocated anyway and left zeroed, which [`counter_frequency_protocol::TimebasePage::hz`]
/// reads as "unknown" rather than a fabricated rate; that keeps every process's layout
/// identical regardless of boot order, the same reason `boot_clock_page` hands out a zeroed page
/// when there is no `clock` program to ask.
#[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
fn timebase_page_phys() -> Option<u64> {
    use core::sync::atomic::{AtomicU64, Ordering};
    static PAGE_PHYS: AtomicU64 = AtomicU64::new(0);

    let cached = PAGE_PHYS.load(Ordering::Acquire);
    if cached != 0 {
        return Some(cached);
    }

    // `alloc_zeroed`, not `alloc`: only the first 16 bytes of this frame are written below, and
    // the WHOLE frame is then mapped read-only into every process on this architecture. An
    // unzeroed frame would carry whatever its last owner left in it across that boundary. Found
    // 2026-09-21 by the lane that built the current-CPU page on this function's shape.
    let phys = crate::memory::alloc_zeroed()?.addr();
    // `phys_to_virt` is a plain address computation (a `const fn`, no memory access), so nothing
    // below this line needs a safety comment for naming `dst`; the comments that follow cover the
    // two places `dst` is actually written through.
    let dst = mmu::phys_to_virt(phys) as *mut u8;
    match crate::arch::timer::frequency_checked() {
        Some(hz) => {
            let bytes = counter_frequency_protocol::build_page(hz);
            // SAFETY: `dst` names a freshly allocated frame, reachable through the direct map and
            // owned by nobody else yet; `bytes` is `PAGE_BYTES` (16) bytes, far under the frame's
            // `FRAME_SIZE`, so the copy does not run past it.
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len()) };
        }
        // Not yet measured: leave the frame zeroed (retype_page-equivalent frames from
        // `memory::alloc` are not guaranteed pre-zeroed the way `Untyped::retype_page` promises,
        // so this writes zero explicitly rather than assuming it).
        //
        // SAFETY: as the `Some` arm above: `dst` names a freshly allocated, exclusively owned
        // frame, and `PAGE_BYTES` is far under `FRAME_SIZE`.
        None => unsafe { core::ptr::write_bytes(dst, 0, counter_frequency_protocol::PAGE_BYTES) },
    }

    PAGE_PHYS.store(phys, Ordering::Release);
    Some(phys)
}

/// Map the timebase page into `space`, if this process needs one built directly rather
/// than through [`load`]. Several kernel-side functions build a top-level process's own
/// `AddressSpace` by hand instead of calling `load` (`spawn_hello`, and every
/// `spawn_<program>`-shaped test harness that hands a narrowed archive to a named program:
/// `timetable_tests::spawn_timetable`, `authority_tests`' `root_supervisor` spawn,
/// `c_seam_tests::spawn_confiner`, `login_service`, `live_swap_tests`' `swapper` spawn), because
/// each wants a narrower or differently-shaped world than a generic `load` call builds. `load`'s
/// own unconditional mapping never reaches any of them, and each one found this the same way:
/// **empirically**, as an unmapped-read page fault the first time something in that process
/// called `user_mode_runtime::cntfrq` (`timetable`'s own scheduling logic was the one that actually found
/// this; `load`'s coverage alone left every one of these kernel-built processes unmapped and it
/// took a real `script/test --arch x86_64` run, not a reading of the call graph, to find them
/// all). Factored out once here rather than copied into each, per CLAUDE.md rule 7's reasoning
/// one level down: this is kernel-internal, not shared with a second *binary*, but the six call
/// sites are exactly the "same lines three times" shape that rule exists to prevent.
///
/// **`riscv64` joined this path on 2026-09-21** and inherited every one of those call sites at
/// once, rather than rediscovering them one page fault at a time, which is what the factoring above
/// bought: there was one function to widen and one `cfg` per site to change.
#[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
pub fn map_timebase_page(space: &mut AddressSpace) -> Result<(), MapError> {
    if let Some(phys) = timebase_page_phys() {
        space.map_physical(
            counter_frequency_protocol::PAGE_VA,
            phys,
            Flags::user_rodata(),
            crate::revoke::PageMapSource::NoCapability,
        )?;
    }
    Ok(())
}

/// Lay an ELF's loadable segments into `space`, honouring their permissions exactly (milestone
/// 19d factored this out of `load` so `spawn_hello` shares it; the progenitor's userspace
/// loader mirrors it).
/// A read-only segment gets `user_rodata`, not `user_data`: a loader that widens permissions is
/// a loader you cannot reason about. `.bss` is free because `map_new` zeroes every page.
pub fn map_segments(space: &mut AddressSpace, elf: &Elf) -> Result<(), LoadError> {
    // Every kernel path that lays out an image comes through here, so the map is enforced here as
    // well as early in [`load`] (which checks before it sizes a region, so a huge image is told it
    // is too large rather than that memory ran out).
    check_image_band(elf)?;
    for seg in elf.segments() {
        let flags = if seg.is_executable() {
            Flags::user_code()
        } else if seg.is_writable() {
            Flags::user_data()
        } else {
            Flags::user_rodata()
        };

        let (start, end) = seg.page_range(FRAME_SIZE);
        let mut va = start;
        while va < end {
            let page = space.map_new(va, flags).map_err(LoadError::Unmappable)?;

            // Which of the file's bytes land in this page? An intersection, because `p_vaddr`
            // need not be page-aligned.
            let file_lo = seg.vaddr;
            let file_hi = seg.vaddr + seg.data.len() as u64;
            let lo = va.max(file_lo);
            let hi = (va + FRAME_SIZE).min(file_hi);
            if lo < hi {
                let dst = (lo - va) as usize;
                let src = (lo - file_lo) as usize;
                let n = (hi - lo) as usize;
                page[dst..dst + n].copy_from_slice(&seg.data[src..src + n]);
            }

            if seg.is_executable() {
                sync_icache(page.as_ptr() as u64, FRAME_SIZE as usize);
            }
            va += FRAME_SIZE;
        }
    }
    Ok(())
}

/// The program QEMU loaded into RAM for us, found via the device tree.
///
/// **The same road Linux's initramfs travels.** Nothing about this binary is known to the kernel
/// at build time: QEMU put a file somewhere in RAM and wrote the address into
/// `/chosen/linux,initrd-start`, and `memory::init` read it there and told the frame allocator
/// to keep its hands off. That reservation was written at milestone 3, for this.
#[cfg_attr(feature = "bench", allow(dead_code))] // the bench boot runs no user programs
pub fn initrd() -> Option<&'static [u8]> {
    let (start, size) = memory::initrd_region()?;

    // SAFETY: the region came from the device tree, it is inside RAM, the frame allocator has
    // been told it is forbidden, and the direct map names it. Nothing else will ever write here.
    Some(unsafe {
        core::slice::from_raw_parts(mmu::phys_to_virt(start) as *const u8, size as usize)
    })
}

/// The bytes of the program named `name` inside the initrd archive (milestone 19f). The initrd is a
/// nifefs image carrying the progenitor plus the programs it loads. The milestone tour and the
/// kernel-side service demos ask for whichever program they wire, by name; since milestone 291
/// that is one program per demo rather than one role of [`HELLO_ENTRY`].
/// `spawn_hello` and `boot_progenitor` instead take the whole archive, because the
/// progenitor parses the rest itself. Returns `None` if there is no initrd, it will not parse, or
/// it holds no such program.
// Used by the milestone tour, the kernel-wired virtio/console/shell demos, and the tests that load
// a user program; dead only in the bench boot, which runs no user programs.
#[cfg_attr(feature = "bench", allow(dead_code))]
pub fn program(name: &str) -> Option<&'static [u8]> {
    nifefs::Fs::parse(initrd()?).ok()?.read(name)
}

/// A physical page to map into a new process's address space, at a chosen VA.
///
/// The frame is **not** owned by the process (it is shared, or it is device MMIO), so it is not
/// freed when the process dies. See [`AddressSpace::map_physical`].
#[derive(Clone, Copy)]
pub struct Mapping {
    pub va: u64,
    pub phys: u64,
    pub flags: Flags,
}

/// **Everything a new process is handed at birth.** Its world, made explicit.
///
/// A capability system has no ambient environment: no inherited file descriptors, no `PATH`, no
/// uid. So a process gets *exactly* what is in this struct and nothing else. The whole of what it
/// can do is a function of `arg0`, `grants`, and `maps`, and reading a `Spawn` literal tells you
/// the complete authority of the thing you are about to start.
pub struct Spawn<'a> {
    /// Lands in `x0` at `_start`. A tiny channel for "which role are you" that needs no
    /// capability, the way a real kernel hands a new process its argc.
    pub arg0: u64,
    /// Lands in `x1`. A second scalar the process needs before it can name anything: the virtio
    /// driver's DMA region physical address, which it must write into device descriptors and
    /// cannot discover, because a process only knows virtual addresses.
    pub arg1: u64,
    /// Lands in `x2`. The virtio driver's device registers sit at a sub-page offset (slots are
    /// 0x200 apart, pages are 0x1000), so we map the containing page and tell the driver where in
    /// it the slot begins.
    pub arg2: u64,
    /// Capabilities, granted into slots 0, 1, 2, ... in order.
    pub grants: &'a [crate::cap::Cap],
    /// Extra pages: a shared buffer, a device's registers. Mapped after the ELF's own segments.
    pub maps: &'a [Mapping],
}

/// Where the kernel maps the initrd read-only into the progenitor's address space (milestone 19d): the progenitor
/// reads the ELF to parse it here. A runtime window on the address-space map (milestone 206): the
/// kernel places it for a program that did not choose the address.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // becomes the boot path at 19d.2; test-driven until then
pub const INITRD_VA: u64 = address_space_map::runtime_window(0x2000_0000);

/// **Spawn the progenitor task** (milestone 19d): load `image` as an ordinary user process, but also
/// map the whole initrd read-only at [`INITRD_VA`] so the progenitor can parse it, and hand the progenitor a building
/// budget (an untyped, slot 0) plus `report` (slot 1, `WRITE|GRANT` so the progenitor can endow a child).
/// The progenitor enters with `x0` = `role` and `x1` = the initrd length. This is the one program the kernel
/// still loads; the progenitor loads the rest (design/init-and-granular-spawn.md).
/// The interrupt the kernel routes to the progenitor for the IRQ-delegation test (19d.2b).
///
/// aarch64: SGI 3, distinct from the scheduler's RESCHED (0) and the older endpoint SGIs (1, 2).
/// RISC-V has no software-generated interrupt a test can raise on itself at all (the SBI IPI
/// arrives down the *software*-interrupt arm, never touching `irq_route`), so it names the console
/// UART's own line, which is the one interrupt this ISA can assert by hand. That makes it the same
/// number as [`UART_RX_INTID`] there, deliberately; [`spawn_hello`] binds the route once and grants
/// two capabilities naming it. See `sched::tests`' `DELIVERY_IRQ`, which reached the same conclusion.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
#[cfg(target_arch = "aarch64")]
pub const INIT_TEST_SGI: u32 = 3;
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
#[cfg(target_arch = "riscv64")]
pub const INIT_TEST_SGI: u32 = 10;
/// `x86_64` (milestone 161, updated by roadmap item 4): **the local APIC's self-IPI test vector**,
/// which puts this ISA on aarch64's side of the split rather than RISC-V's. The local APIC will
/// deliver a vector to its own CPU on demand through the ICR, so x86 needs no device to raise an
/// interrupt by hand, and `arch::x86_64::irq::raise_self_interrupt` is the mechanism. The intid for
/// such a source **is its vector**, which is why this is 0x22 and not a small number like the other
/// two: see `arch::x86_64::exceptions::x86_trap_body`'s self-IPI arm for the naming rule.
///
/// It was 4 (COM1's legacy IRQ) while the APIC was unbuilt and that arm's own comment said to
/// revisit this when it landed.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(target_arch = "x86_64")]
pub const INIT_TEST_SGI: u32 = crate::arch::irq::SELF_TEST_VECTOR as u32;

/// The console UART's receive interrupt on QEMU `virt`. The progenitor routes and delegates it so the input
/// driver it builds (19d.2c) can wait on keystrokes. aarch64's PL011 is SPI 1 = INTID 33; RISC-V's
/// NS16550 is PLIC source 10.
///
/// **The documented fallback, not the answer.** The boot paths ask the machine first
/// ([`uart_irq_and_source`]): on the JH7110, UART0 interrupts on PLIC line 32, and a kernel that
/// armed this constant there enabled an unrelated source, proven on silicon when a key press at
/// boot 13's completed tour reached nothing (notes/visionfive2.md, BUGS). This number is what a
/// tree that does not say falls back to, which on QEMU is also the right answer.
///
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/boolean-predicates-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
/// for in his #1255 review; recommended `UART_RECEIVE_INTID`; `INTID` is the GIC's own term and
/// stays.
#[cfg(target_arch = "aarch64")]
pub const UART_RX_INTID: u32 = 33;
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/boolean-predicates-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
/// for in his #1255 review; recommended `UART_RECEIVE_INTID`; `INTID` is the GIC's own term and
/// stays.
#[cfg(target_arch = "riscv64")]
pub const UART_RX_INTID: u32 = 10;
/// `x86_64`: COM1 is ISA IRQ 4, which has been true since the PC/AT and is what QEMU's `q35`
/// presents. **What that number means depends on the interrupt controller**, and on x86 that is
/// two questions rather than one: which IO APIC input the legacy IRQ was remapped to (the ACPI
/// MADT's interrupt source overrides say, and this port does not read them), and which IDT vector
/// that input is programmed to raise. 4 is the legacy line, not either of those.
///
/// Name: provisional, flagged 2026-09-25 by the lane that re-derived the x86 port falsifications
/// (design/naming/boolean-predicates-worklist.md, "`rx` and `tx`"). calef asked what `rx` stands
/// for in his #1255 review; recommended `UART_RECEIVE_INTID`; `INTID` is the GIC's own term and
/// stays.
#[cfg(target_arch = "x86_64")]
pub const UART_RX_INTID: u32 = 4;

/// The console UART's interrupt line and which source decided it: the machine's own answer when
/// it gave one (`memory::uart_irq`, a device tree on aarch64/riscv64 or ACPI on `x86_64`), else
/// [`UART_RX_INTID`], QEMU `virt`'s constant. The source string exists to be printed: a bench
/// transcript that names the number's origin is diagnosable, and the one that did not already
/// cost a boot (notes/visionfive2.md).
pub fn uart_irq_and_source() -> (u32, &'static str) {
    match crate::memory::uart_irq() {
        Some(n) => (n, "machine description"),
        None => (UART_RX_INTID, "QEMU-virt fallback; the machine did not say"),
    }
}

/// The console UART's registers, physically. aarch64 `virt` puts a PL011 at `0x0900_0000`; RISC-V
/// `virt` puts an NS16550 at `0x1000_0000`. The progenitor holds a device capability for it and delegates it
/// to the console and input drivers it builds. Matches `console::UART_PHYS`.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(target_arch = "aarch64")]
pub const UART_PHYS: u64 = 0x0900_0000;
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(target_arch = "riscv64")]
pub const UART_PHYS: u64 = crate::arch::machine::CONSOLE_UART_PHYS;
/// `x86_64` has **no physical address for its console at all**: COM1 lives in the I/O port space,
/// which has no page tables in front of it, so there is nothing here for a device capability to be
/// a mapping *of*. The console is reached through [`X86_COM1_PORT_BASE`] as a `PortRange` capability
/// instead (milestone 299, DECISIONS §121 reversed 2026-09-15); this constant stays zero because a
/// port has no physical page, and the predicate below still reads it to keep a fixture from ever
/// granting a device page where there is none. See `arch/x86_64/port.rs` and `segments.rs`.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(target_arch = "x86_64")]
pub const UART_PHYS: u64 = 0;

/// **COM1's I/O ports** (milestone 299): the eight consecutive ports `0x3F8..=0x3FF` a 16550 UART
/// occupies, which QEMU's `q35` and every PC since the PC/AT put COM1 at. The progenitor is minted a
/// `PortRange(0x3F8, 8)` capability over exactly this range and delegates it to the console and input
/// drivers, so their `in`/`out` reach these ports and no others (enforced by the TSS I/O bitmap). The
/// port analogue of [`UART_PHYS`] on the other two architectures.
#[cfg(target_arch = "x86_64")]
pub const X86_COM1_PORT_BASE: u16 = 0x3F8;
/// The eight ports a 16550 occupies (data/interrupt-enable/FIFO-control/line-control/modem-control/
/// line-status/modem-status/scratch). A range because a device capability names what the hardware
/// names, the port-space twin of a `DeviceFrame` naming a page.
#[cfg(target_arch = "x86_64")]
pub const X86_COM1_PORT_COUNT: u16 = 8;

/// **Is there a UART a device capability can be a mapping of on this machine?** (milestone 161.)
///
/// [`UART_PHYS`] is zero on `x86_64` and that zero is the marker for
/// [DECISIONS §121](../../design/decisions/0121-port-io-capability.md) (PROPOSED). Several fixtures
/// need to ask, so they ask here rather than each testing a constant against zero and each writing
/// its own sentence about why.
///
/// **The trap this exists to prevent is a green test rather than a red one.** A fixture that went
/// ahead anyway would grant a device capability over *physical page zero*, map it, and read
/// real-mode interrupt-vector bytes; the read would succeed, a revoke would still fault, and a
/// suite would report that x86 has a userspace device story. It does not.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn machine_has_no_device_page_for_the_console() -> bool {
    UART_PHYS == 0
}

/// The reason a fixture gives when [`machine_has_no_device_page_for_the_console`] is true.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const NO_UART_PAGE: &str = "this machine's console UART is in the I/O port space, so there is \
                                no page for a device capability to be a mapping of and no \
                                capability shape for a port yet (DECISIONS \u{a7}121)";

/// **The archive entry the kernel enters as the first process**, on every architecture.
///
/// One name, one binary, one program (milestone 266). This used to be `init`, and it meant
/// `fixtures/src/hello.rs`'s `init_boot` role on aarch64 and `system_initializer` on riscv64: an alias
/// standing over two implementations of one job, which is DECISIONS §19's own failure mode and had
/// already been paid for once as a boot that reached userspace and printed nothing at all.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const PROGENITOR_ENTRY: &str = "progenitor";

/// The archive entry holding milestone 19d's and 19e's **init roles**: the one binary the kernel
/// still re-enters at a chosen role, to play a userspace parent that builds a child out of an ELF
/// it parsed.
///
/// One name on all three architectures since milestone 266. aarch64 used to pack it as `init`,
/// because there it carried the boot role as well; that role is [`PROGENITOR_ENTRY`]'s own program
/// now, and `hello` is packed as `hello` everywhere. The kernel still enters it directly for those
/// init roles, which is why it is in `boot_programs` and measured.
///
/// **It was the whole milestone 7-19 role catalogue until milestone 291**, thirty-one roles in one
/// binary. Twenty-two of them are their own programs or `block_driver`'s roles now; nine are left,
/// and milestone 405, `design/roadmap/0405-nine-init-roles-and-the-entry-the-kernel-picks.md`, is what would
/// take them, since splitting them is a change to [`spawn_hello`]'s choice of entry rather
/// than to `fixtures/`.
///
/// Name: provisional (milestone 266 (one progenitor, on all three architectures)): a constant
/// rather than a program, but it is the name a reader meets at eight call sites, and
/// `kernel::user::tests` already spelled it this way. The program's own name is overdue and is
/// an architect's; see that file's `BUGS`.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const HELLO_ENTRY: &str = "hello";

/// The progenitor's stack, in pages (19d.2c): it loads whole ELFs with deep call chains, so its stack is
/// larger than an ordinary process's one page. Also the stack of `hello`'s init roles and the
/// riscv64 serial driver, which share this constant and are far shallower.
///
/// **Twelve pages (48 KiB) since the progenitor's stack was first measured** (milestone
/// progenitor-stack (provisional), 2026-09-27). It was eight, whose doc called 32 KiB "generous",
/// and nothing measured it until `crate::progenitor_stack`'s gauge read these peaks on
/// `script/swish-check`, out of 32,768:
///
/// | | aarch64 | riscv64 |
/// |---|---|---|
/// | debug, at the prompt | 19,000 | 18,976 |
/// | debug, `package install` | **32,440** | **32,184** |
/// | release, `package install` | 16,432 | 16,544 |
///
/// Debug, the build `swish-check` and CI boot, had 328 bytes to spare, which is why three lanes in
/// a row hit it. It is twice release because `system_initializer::boot`'s own frame is 12,848
/// bytes unoptimised (3,200 optimised) and stays live under the spawn service, which runs inside
/// it. Twelve pages puts the debug peak at 66% and leaves twice the gauge's floor
/// (`progenitor_stack::HEADROOM_FLOOR`, 8 KiB) before `swish-check` fails.
/// notes/stack/progenitor-stack.md has the frames and why this is a raise rather than a trim.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const INIT_STACK_PAGES: u64 = 12;
const _: () = assert!(INIT_STACK_PAGES <= address_space_map::MAX_STACK_PAGES);

/// **The role that means "boot the system"**, as opposed to milestone 19d's test roles.
///
/// [`boot_progenitor`] passes it as the progenitor's `x0` when it loads [`PROGENITOR_ENTRY`] and
/// grants the boot capability set. The progenitor has one role and ignores it; the value is passed
/// for the symmetry the old `spawn_progenitor` established, when a single function both booted the
/// system and re-entered [`HELLO_ENTRY`] at milestone 19d's test roles (milestone 166 split those
/// two jobs, so this role no longer selects an entry).
///
/// The number is 27 because that is the role `hello`'s retired `init_boot` had, and every 19d role
/// number around it is load-bearing to a test. **Name provisional** (milestone 266).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub const PROGENITOR_ROLE: u64 = 27;

/// **Spawn a milestone-19d/19e test role of [`HELLO_ENTRY`]** (milestone 19d.2c; the boot half of
/// this function became [`boot_progenitor`] at milestone 166).
///
/// The kernel re-enters the one `hello` binary at `role` to play a userspace parent, handing it the
/// bare 19d world at fixed slots: the building budget (slot 0), the kernel's `report` endpoint (slot
/// 1, `WRITE|GRANT` so a role can report and endow a child on it), the console UART's registers
/// (slot 2), the 19d.2b test interrupt (slot 3), and the UART receive interrupt (slot 4). It returns
/// a [`holding::Holding`] over the thread and its building budget, so a test finished with it can
/// hand **2048 frames** back: six of these tests reserve 8 MiB each and the measured aarch64 boot
/// spent 12289 frames on them, **42% of everything the suite never returned** (notes/frames.md).
///
/// **This is no longer the boot path.** Until milestone 166 it was both jobs at once: at
/// [`PROGENITOR_ROLE`] it loaded [`PROGENITOR_ENTRY`] and booted the whole system, and at any other
/// role it loaded `hello`. That sharing is exactly why aarch64's boot once carried slots 1 and 3 the
/// interactive system never used, and DECISIONS §19's silent-divergence risk lived in the split. The
/// boot now goes through [`boot_progenitor`] on all three architectures, which hands the progenitor
/// the same slot layout everywhere; this function only ever plays a test role and grants only the
/// five capabilities those roles use.
///
/// **`hello` still has nine roles**, and splitting them is a follow-on to milestone 291
/// (`fixtures/src/hello.rs` was thirty-one programs wearing one name), tracked as milestone 405
/// (`design/roadmap/0405-nine-init-roles-and-the-entry-the-kernel-picks.md`): six are separate
/// programs waiting to happen, and each would need its own archive entry named here.
///
/// Name: ratified 2026-09-15 (calef, this header). Refused keeping `spawn_progenitor`, the name
/// this function carried until milestone 166 split it in two: the boot half it was named for is
/// [`boot_progenitor`] now, and what stayed here never spawns the progenitor at all. It only ever
/// re-enters [`HELLO_ENTRY`] at one of milestone 19d/19e's test roles, at every call site it has
/// (all of them in `system_tests/src/user/tests.rs`), so the old name pointed at the half that left.
/// `spawn` is the verb this body performs and `hello` the program it performs it on, which makes
/// the name a claim about what the function does rather than about what it used to do, and greps
/// with [`HELLO_ENTRY`] as one family.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn spawn_hello(
    image: &'static [u8],
    role: u64,
    report: crate::sched::RendezvousId,
) -> holding::Holding {
    let (initrd_start, initrd_len) =
        memory::initrd_region().expect("no initrd to hand the progenitor");
    let initrd_pages = initrd_len.div_ceil(FRAME_SIZE);

    // Route the test interrupt (19d.2b) BEFORE spawning the progenitor: the test raises the SGI as soon as
    // this returns, and an interrupt that fires before it is routed is dropped ("unexpected
    // interrupt"), not queued. Setting up the route here means the fire is counted on the routed
    // endpoint even though the progenitor-built child is not yet waiting; the child's WAIT drains it.
    crate::sched::bind_irq(INIT_TEST_SGI, crate::sched::create_rendezvous());
    crate::arch::irq::enable(INIT_TEST_SGI);
    // And the UART receive interrupt (19d.2c): the input driver the progenitor builds waits on it. Route and
    // enable it here, so the progenitor can delegate the Irq cap to that driver. The number is the machine's
    // (uart_irq_and_source; on the JH7110 the QEMU constant armed the wrong PLIC source, see its
    // doc), and the line names the source so a transcript is diagnosable. On QEMU RISC-V the
    // discovered line and INIT_TEST_SGI are the SAME source (see INIT_TEST_SGI), and binding it
    // twice would leave the first endpoint routed to nothing while the test waits on it, so bind
    // once and grant twice.
    let (uart_rx_intid, uart_irq_source) = uart_irq_and_source();
    crate::println!("  uart irq  : {uart_rx_intid} ({uart_irq_source})");
    if uart_rx_intid != INIT_TEST_SGI {
        crate::sched::bind_irq(uart_rx_intid, crate::sched::create_rendezvous());
        crate::arch::irq::enable(uart_rx_intid);
    }

    // Read and MEASURE `hello` here, before the thread is created (milestone 22 phase B.1): the
    // check has to be the thing that decides whether a thread is created at all, not something the
    // new thread does to itself. `trust::require` halts on a mismatch, so past this line the bytes
    // are the ones this kernel image was built against. The program-measurement table (milestone
    // 104) is required for the same reason: the whole archive is mapped into `hello`.
    let boot_fs = match nifefs::Fs::parse(image) {
        Ok(fs) => fs,
        Err(e) => {
            crate::println!("  archive is not a nifefs image: {e:?}");
            crate::sched::exit();
        }
    };
    let Some(init_bytes) = boot_fs.read(HELLO_ENTRY) else {
        crate::println!("  archive has no '{HELLO_ENTRY}' program");
        crate::sched::exit();
    };
    crate::trust::require(HELLO_ENTRY, init_bytes);
    crate::trust::require_program_measurements(&boot_fs);

    // **The building budget is carved here, not inside the thread**, so the caller has a name for it
    // and can reclaim it: a large untyped a role retypes its child's address space, frames and TCB
    // from. Carving it out here changes nothing about what the role gets; it changes who can name it
    // afterwards, the whole difference between 8 MiB spent and 8 MiB lent. See notes/frames.md.
    let build_region = crate::memory_region::create(12288).expect("no building budget for hello");

    let tid = crate::sched::spawn(move || {
        let elf = match Elf::parse(init_bytes) {
            Ok(e) => e,
            Err(e) => {
                crate::println!("  the hello image is not loadable: {e:?}");
                crate::sched::exit();
            }
        };
        // A region big enough for hello's own segments, the initrd's page tables, and slack: a role
        // that builds a child loads whole ELFs with deep call chains.
        let content: u64 = elf
            .segments()
            .map(|seg| {
                let (start, end) = seg.page_range(FRAME_SIZE);
                (end - start) / FRAME_SIZE
            })
            .sum::<u64>()
            + 1
            + initrd_pages / 512
            + crate::revoke::log_pages_for(initrd_pages)
            + INIT_STACK_PAGES
            + 8;
        let mut space = AddressSpace::new(content).expect("no memory for hello");
        map_segments(&mut space, &elf).expect("could not lay out hello");
        // A multi-page stack: a role that parses and builds a child ELF has deep call chains (the
        // loader loop, copy_from_slice, the elf parser), so one page overflows. Map INIT_STACK_PAGES
        // down from USER_STACK_TOP; the entry sp is unchanged (USER_STACK_TOP).
        for k in 0..INIT_STACK_PAGES {
            space
                .map_new(USER_STACK_VA - k * FRAME_SIZE, Flags::user_data())
                .expect("could not map hello's stack");
        }
        #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
        map_timebase_page(&mut space).expect("could not map hello's timebase page");

        // Map the initrd, one page at a time, read-only. These are reserved RAM pages the frame
        // allocator does not own, so this maps rather than allocates. A role that builds a child
        // parses the archive here.
        for i in 0..initrd_pages {
            space
                .map_physical(
                    INITRD_VA + i * FRAME_SIZE,
                    initrd_start + i * FRAME_SIZE,
                    Flags::user_rodata(),
                    crate::revoke::PageMapSource::NoCapability,
                )
                .expect("could not map the initrd into hello");
        }

        crate::sched::adopt_address_space(space);
        // slot 0: the delegable root budget. A role narrows and hands budgets to the children it
        // builds, so the root carries GRANT (milestone 31). Rights only narrow downward from here.
        crate::sched::grant(crate::cap::memory_region_root_cap(build_region))
            .expect("grant untyped");
        // slot 1: the kernel's report endpoint, WRITE|GRANT so a role can report a result and endow
        // a child to send on it. This slot is the boot layout's slot 1 too, which is exactly why
        // [`boot_progenitor`] once carried a report endpoint it never used: the shared function.
        crate::sched::grant(crate::cap::rendezvous_cap(
            report,
            crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant report");
        // slot 2: the UART registers, so a console role can build a driver and hand it the registers
        // (19d.2). WRITE (device access) | GRANT (delegate to the driver).
        //
        // **On x86_64 `UART_PHYS` is zero and this grants a device capability over physical page
        // zero**, which is a foot gun and is marked as one rather than designed away (AGENTS.md's
        // ladder: an exception must say it is an exception). The slot is positional, so declining to
        // grant here would renumber the interrupts below and every role that names them. Nothing
        // reaches it today: every fixture that would map it asks
        // `machine_has_no_device_page_for_the_console()` first and skips.
        crate::sched::grant(crate::cap::device_frame_cap(
            UART_PHYS,
            crate::cap::Rights::WRITE.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant uart device");
        // slot 3: the 19d.2b test interrupt, so the IRQ-delegation role can build an interrupt-driven
        // driver and hand it the Irq cap. The route was set up above, before the spawn; this only
        // grants the cap (a per-thread act). READ (WAIT/ACK) | GRANT (delegate).
        crate::sched::grant(crate::cap::irq_cap_rights(
            INIT_TEST_SGI,
            crate::cap::Rights::READ.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant test irq");
        // slot 4: the UART receive interrupt, for the input driver a console role builds (19d.2c).
        // The same discovered number the route above was bound with, or the cap would name a source
        // no endpoint serves.
        crate::sched::grant(crate::cap::irq_cap_rights(
            uart_rx_intid,
            crate::cap::Rights::READ.union(crate::cap::Rights::GRANT),
        ))
        .expect("grant uart rx irq");

        // A test role has no filesystem, so `x2` (the file-service rights the boot path passes) is 0.
        enter_frame(elf.entry(), USER_STACK_TOP, role, initrd_len, 0)
    })
    .expect("could not spawn hello");

    let mut held = holding::Holding::new();
    held.add_thread(tid);
    held.add_region(build_region);
    held
}

/// **A run of contiguous device pages mapped into a new process before it starts**, for a window
/// too large to spell as [`Mapping`]s (the shell on the firmware screen: a screen's aperture is a
/// thousand pages or more, and the kernel has no heap to build a slice that long in).
///
/// Always device-typed and writable, because the one thing that needs it is a driver's view of a
/// device's memory, and write-combining when [`Self::write_combining`] says so. Like every [`Spawn::maps`] entry the process holds no *name* for it: it cannot
/// map it again, delegate it, or revoke it, which is the property `non_volatile_memory_express_service`
/// and milestone 159's TRNG driver chose spawn-time mappings for. **Name provisional.**
#[derive(Clone, Copy)]
pub struct DeviceRun {
    /// Where the first page lands in the new process.
    pub va: u64,
    /// The first page's physical address. Page-aligned.
    pub phys: u64,
    /// How many pages. The intermediate page tables come out of the address space's own
    /// `AS_OVERHEAD`, so a caller bounds this (`display_service`'s `MAX_APERTURE_PAGES`).
    pub pages: u64,
    /// **Map it write-combining** (`paging::Flags::user_write_combining`) rather than as
    /// registers. True only for memory the process writes and never reads, which on this tree is
    /// one framebuffer aperture: a register window combined would lose the order of its stores.
    /// A required field with no default, so a second caller has to say which it is. Name:
    /// provisional (the screen terminal lane, 2026-10-04).
    pub write_combining: bool,
}

/// Load the initrd program and become it, handed the world described by `spawn`. Never returns.
pub fn run(image: &[u8], spawn: Spawn) -> ! {
    run_with(image, spawn, None, None)
}

/// [`run`], with one [`DeviceRun`] mapped as well. Never returns. **Name provisional.**
pub fn run_with_device_run(image: &[u8], spawn: Spawn, device: DeviceRun) -> ! {
    run_with(image, spawn, Some(device), None)
}

/// [`run`], and the process also holds its own address space, `WRITE`, at `slot` (§255 (each
/// socket is its own capability)): the network stack's spawn, so it can `UNMAP` a closed socket's
/// page. `supervision_protocol::CHILDS_OWN_SPACE` is the progenitor's twin. Never
/// returns. Name provisional.
pub fn run_with_own_space(image: &[u8], spawn: Spawn, slot: u64) -> ! {
    run_with(image, spawn, None, Some(slot))
}

fn run_with(image: &[u8], spawn: Spawn, device: Option<DeviceRun>, own_space: Option<u64>) -> ! {
    // What this process is about to have mapped into it beyond its own image: the `Spawn` windows
    // and a device run. See [`load`] for why the number is taken here rather than asked of
    // each caller.
    let windowed = spawn.maps.len() as u64 + device.as_ref().map_or(0, |d| d.pages);
    let (mut space, entry) = match load(image, windowed) {
        Ok(v) => v,
        Err(e) => {
            crate::println!();
            crate::println!("  refused to load a user program: {e}");
            crate::println!("  the kernel is fine.");
            crate::sched::exit();
        }
    };

    // The extra pages go in BEFORE we hand the address space off: a shared message buffer, or a
    // device's MMIO for a driver. This is the line that puts a UART into a userspace process.
    for m in spawn.maps {
        space
            .map_physical(
                m.va,
                m.phys,
                m.flags,
                crate::revoke::PageMapSource::NoCapability,
            )
            .expect("could not map a Spawn page into the new address space");
    }
    if let Some(d) = device {
        for k in 0..d.pages {
            space
                .map_physical(
                    d.va + k * FRAME_SIZE,
                    d.phys + k * FRAME_SIZE,
                    if d.write_combining {
                        Flags::user_write_combining()
                    } else {
                        Flags::user_device()
                    },
                    crate::revoke::PageMapSource::NoCapability,
                )
                .expect("could not map a device run into the new address space");
        }
    }

    let name = crate::sched::adopt_address_space(space);

    // HAND IT ITS WORLD. Granted in order, so slot 0 is `grants[0]`, and reading the caller's
    // `Spawn` literal tells you the entire authority of the process. There is no path it can
    // say, no uid it can be. A capability system's "environment" is not a variable, it is this.
    for &granted in spawn.grants {
        crate::sched::grant(granted).expect("no free capability slot");
    }
    if let Some(slot) = own_space {
        crate::sched::grant_at(
            slot,
            crate::cap::address_space_cap(name, crate::cap::Rights::WRITE),
        )
        .expect("the own-space slot was already occupied");
    }

    enter_at(entry, spawn.arg0, spawn.arg1, spawn.arg2)
}

/// Drop to EL0 at `entry`, on a fresh stack, with `arg0` in `x0`. Never returns.
///
/// `arg0` reaches `_start` as its first argument (AAPCS64 puts it in `x0`). It is how the kernel
/// tells one binary which of several roles to play, the way a real kernel hands a new process
/// its argc/argv. See the console server, which is the same ELF as its client with a different
/// `arg0`.
/// Drop the **current** thread to EL0 at `entry` on `user_sp`, no arguments (milestone 19c.3).
/// The entry path for a thread started through the TCB object surface, which runs on the freshly
/// scheduled thread rather than the one that called `START`. The address space is already
/// installed (the context switch that scheduled us in used our `space` field). This is `enter_at`
/// with a caller-chosen stack and zero args; `enter_at` is now the exec wrapper over it.
pub fn enter_at_on_current(entry: u64, user_sp: u64, arg0: u64, arg1: u64, arg2: u64) -> ! {
    enter_frame(entry, user_sp, arg0, arg1, arg2)
}

fn enter_at(entry: u64, arg0: u64, arg1: u64, arg2: u64) -> ! {
    enter_frame(entry, USER_STACK_TOP, arg0, arg1, arg2)
}

fn enter_frame(entry: u64, user_sp: u64, arg0: u64, arg1: u64, arg2: u64) -> ! {
    // THE TRAPFRAME IS NOT AN ORDINARY LOCAL, and this cost us an afternoon.
    //
    // It must sit at the TOP OF THIS THREAD'S KERNEL STACK, because that is where the hardware
    // will look for it. `enter_userspace` does `mov sp, x0`, and `exception_restore` leaves
    // SP_EL1 = x0 + 272 across the `eret`. So when the user traps back in, `SAVE_CONTEXT`
    // subtracts 272 and rebuilds the frame **at exactly this address**. It had better be
    // writable, and it had better be a stack.
    //
    // The first version wrote `enter_userspace(&TrapFrame { .. })`, and every field of that
    // struct is a compile-time constant, so Rust CONST-PROMOTED IT INTO .rodata. The kernel
    // set SP_EL1 to read-only memory, and the user's first `svc` faulted trying to write its
    // own trap frame there. See notes/userspace.md: the kernel then walked `sp` DOWNWARD
    // through .rodata and the whole of .text, 272 bytes and one fault at a time, until it fell
    // out of the bottom of the image into writable RAM and could finally tell us.
    let top = crate::sched::current_kernel_stack_top()
        .expect("a user thread needs a kernel stack of its own to be trapped onto");

    // **The frame goes at the very top of the kernel stack, on both ISAs, and it must be ABOVE the
    // live `sp`.** Everything below `sp` belongs to somebody else: a callee's frame, and on a trap
    // the 288/272 bytes the vector subtracts from `sp` to build its own frame. An object parked
    // there is not stored, it is lent.
    //
    // RISC-V used to compute this from the live `sp` instead (`(current_sp().min(top) - size) & !15`),
    // because its TCB entry path is shallow and a frame at the top would have overlapped this
    // function's own stack. That traded a deterministic overlap for an intermittent one, and
    // milestone 71 caught it: `current_sp()` is a real call at opt-level 0, so it returned
    // `sp - 16`, which put the frame at `sp - 304` while `trap.s` builds an S-mode trap frame at
    // `sp - 288`. The two differ by exactly 16 bytes, so the user frame's `x[2]` (the user `sp`)
    // sat precisely on the trap frame's `x[0]` slot, which `trap.s` writes as a literal zero. Any
    // timer interrupt taken between building the frame and consuming it therefore rewrote the whole
    // frame: user `sp` read 0 every time, `sepc` read whatever `t5` held, and `sstatus` read the
    // trap's `scause` (whose UXL bits are 0, an illegal U-mode XLEN). When `t5` happened to be 0 the
    // `sepc == 0` guard in `enter_user` fired; when it did not, the thread `sret`ed to a garbage PC,
    // died on its first instruction, and never answered whoever was waiting on it, which is a
    // lost-wakeup hang with no guard message. See notes/riscv-port.md.
    //
    // The shallow-path problem the old code was avoiding is real, and the fix for it is a
    // reservation rather than a moving target: `user_entry_trampoline` (both ISAs) drops `sp` by a
    // frame's worth before the first Rust frame exists, so this region is off-limits to the entry
    // path by construction. See arch/*/context.s.
    //
    // `thread_trampoline` deliberately does NOT reserve, and the asymmetry is the point. Only the
    // TCB path can be shallow; the exec path reaches here through `run` and the ELF loader, so its
    // frames are always far below the stack top, and reserving would spend a frame's worth on every
    // kernel thread to insure against a depth that cannot happen. If that ever stops being true, the
    // assertion below is what says so.
    let slot = top - size_of::<TrapFrame>() as u64;
    let frame = slot as *mut TrapFrame;

    // And prove it, rather than trusting the reasoning above. This is one check, once per
    // exec, against a bug whose symptom is a nested fault storm that eats the kernel image.
    assert!(
        mmu::translate(frame as u64).is_some_and(|(_, f)| f.is_writable()),
        "the user's TrapFrame at {frame:p} is not in writable memory",
    );

    // **The invariant the milestone 71 fault violated**, checked rather than reasoned about. A slot
    // at or below the live `sp` is one a callee or a trap will build over, and the old RISC-V
    // placement failed this on the very first user entry. Necessary rather than sufficient: this
    // function's own frame sits *above* `sp` and is not covered, which is what the trampoline
    // reservation handles. Cheap enough to keep: one comparison per exec.
    assert!(
        slot >= crate::arch::current_sp(),
        "the user's TrapFrame at {frame:p} is below the live sp: a callee frame or a trap frame \
         will be built over it",
    );

    // SAFETY: `frame` is 16-byte-aligned writable kernel stack (a KernelStack top is page
    // aligned and TrapFrame is a multiple of 16), the user code and stack are mapped, and the
    // user address space is installed. `arch` owns the register layout: we ask for a user-entry
    // frame and hand it back to `arch` to make the jump (notes/riscv-port.md, leak #3).
    unsafe {
        frame.write(TrapFrame::for_user_entry(
            entry,
            user_sp,
            [arg0, arg1, arg2],
        ));
        enter_user(frame)
    }
}

// --- the test programs the kernel loads by name ---
//
// **There used to be five hand-assembled programs here**, three aarch64 and two RISC-V, written as
// `global_asm!` machine code in `.rodata` and copied into a user page by a one-page loader. They
// were honest milestone-7a scaffolding: there was no ELF loader and no filesystem to load from, so
// the "binary" rode inside the kernel image.
//
// They are gone (milestone 19's user-test port). The behaviours are ordinary (yield twice, read a
// forbidden address, spin forever), the toolchain builds them for both targets, and the initrd
// already delivers thirty other programs, so the scaffolding had outlived its reason twice over:
// once when 7c shipped the ELF loader, and again when the second ISA turned "one hand-written
// program" into "two hand-written programs, forever". Keeping them would have meant hand-assembling
// every one of them a second time to run the same tests on RISC-V.
//
// What replaced each:
//
//   - aarch64 `hello`, riscv `USER_HELLO` (yield, yield)  -> `outlaw`, role `OUTLAW_ROUND_TRIP`
//   - aarch64 `outlaw`  (read a kernel address)           -> `outlaw`, role `OUTLAW_READ_KERNEL`
//   - aarch64 `spin`    (loop, no syscall, no stack)      -> the `interrupt_ignorer` binary (DECISIONS §24)
//   - riscv `USER_REPORTER` (invoke a cap, SEND a word)   -> `riscv_least_authority_demo`, which builds a
//     process from the same parts and runs a real ELF through them
//
// This also removed `exec`, the one-page raw-machine-code loader they needed. Every program the
// kernel runs now arrives as an ELF.

/// The `outlaw` program's roles (fixtures/src/outlaw.rs), passed in the first argument register.
///
/// `ROUND_TRIP` yields twice and exits: two syscalls from user mode, where the second can only
/// happen if the return from the first genuinely put the thread back at EL0/U-mode.
// The tests use it on both ISAs; of the two boot tours only RISC-V's has a syscall-count step.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const OUTLAW_ROUND_TRIP: u64 = 0;

/// `READ_KERNEL` reads the address handed to it in the second argument register, which is what makes
/// the program portable: the kernel's own memory lives at a different virtual address on each ISA,
/// and the caller knows which. See `tests::a_user_program_cannot_read_a_kernel_address`.
// The tour uses it, and the shell/bench boots skip the tour.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const OUTLAW_READ_KERNEL: u64 = 1;

/// **Load and run a real compiled ELF at U-mode on RISC-V** (milestone 20, the user-ELF step).
///
/// This takes the bytes of the `least_authority_demo` program (a Rust binary compiled to a riscv64 ELF, delivered
/// as the initrd)
/// and runs them through the kernel's *real* ELF loader. [`load`] parses the file, builds an address
/// space with each `PT_LOAD` segment mapped W^X at the VA it names, and maps a stack; nothing here is
/// riscv-specific except that the loader was just taught to accept `EM_RISCV`. The `least_authority_demo` is granted
/// WRITE on one endpoint as its slot 0, started with the input `n` in its second argument register
/// (`a1`), squares it, and SENDs the answer home.
///
/// Receiving `n*n` proves the whole ELF path works on RISC-V: parse, segment mapping with correct
/// permissions, the entry point, argument passing across the `START` boundary, and the endpoint
/// SEND, all from a program the kernel did not hand-write. `load` is arch-neutral; this is the same
/// code aarch64 runs, now on the RISC-V address space and trap path.
/// The hand-assembled `x86_64` programs. Compiled ones now exist (item 4's hand-off), but these
/// stay: the boot tour runs before any archive is parsed, and a fixture that needs no initrd is
/// what lets the userspace demo run on a `cargo run` with no `-initrd` at all.
#[cfg(target_arch = "x86_64")]
pub mod x86_programs;

/// Where the x86 demo's children put their code and stack: the address-space map's image base and
/// top stack page, where the supervision fixtures put theirs on every architecture, so a reader who
/// has seen one recognises them.
#[cfg(target_arch = "x86_64")]
const X86_DEMO_CODE_VA: u64 = address_space_map::IMAGE_BASE;
#[cfg(target_arch = "x86_64")]
const X86_DEMO_STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
/// The word the reporting child SENDs, and the address the faulting one loads from. Both are
/// distinctive so that a zero anywhere in the report is visibly a failure rather than a plausible
/// value.
#[cfg(target_arch = "x86_64")]
const X86_DEMO_WORD: u32 = 0x0161_0004;
#[cfg(target_arch = "x86_64")]
const X86_DEMO_BAD_ADDR: u32 = 0x00A5_0000;

/// What the x86 userspace demo found. Every field is something a **program in ring 3** or the
/// **kernel's own supervision path** produced, rather than something the tour assumed.
#[cfg(target_arch = "x86_64")]
#[derive(Debug, Clone, Copy)]
pub struct X86UserspaceReport {
    /// The word the reporting child sent, as it arrived on the endpoint. Proves the child reached
    /// ring 3, made a `syscall` that reached the portable dispatcher, and was answered.
    pub reported: u64,
    /// The thread id the kernel stamped on the faulting child's death message.
    pub faulted_tid: u64,
    /// The pc the faulting child died at, from the message. `X86_DEMO_CODE_VA + 5` if the fault
    /// landed on the instruction it was supposed to.
    pub fault_pc: u64,
    /// The address it faulted on, from the message.
    pub fault_addr: u64,
    /// **What the first round of two children cost the frame allocator**, net of their regions
    /// being destroyed.
    pub first_round_frames: isize,
    /// **What an identical second round cost.** This is the number that means something, and the
    /// reason the demo runs twice: a first round pays first-use carves that are not leaks (the
    /// kernel's object budget, the endpoint region, a thread stack the recycler has not seen yet),
    /// and a system that has reached a steady state charges the second round **zero**. It is the
    /// same distinction `thread.rs`'s stack-VA reuse test draws, and the same evidence.
    pub second_round_frames: isize,
}

/// **Build one hand-assembled child out of `region` and start it.** The x86 boot tour's own
/// `build_child_in`, kept beside the demo rather than shared with `supervision_tests` because that
/// module is `#[cfg(test)]` and this runs on an ordinary boot.
///
/// `slot0` is the capability the program's own slot 0 will hold, if any; `fault_ep` goes in the
/// reserved fault slot, so `START` records it as this child's supervision endpoint.
#[cfg(target_arch = "x86_64")]
fn x86_build_child(
    region: u64,
    program: &[u32],
    slot0: Option<crate::cap::Cap>,
    fault_ep: Option<crate::sched::RendezvousId>,
) -> Result<u64, &'static str> {
    let aspace = user_address_space_create(region).ok_or("no aspace for the child")?;

    let code_phys = crate::memory_region::retype_page(region).ok_or("no code frame")?;
    // SAFETY: a fresh frame this region owns, reachable through the direct map; the program is
    // written there and then mapped executable. The kernel cannot address `X86_DEMO_CODE_VA`
    // itself, which is why the frame is written through its physical name instead.
    unsafe {
        let dst = mmu::phys_to_virt(code_phys) as *mut u32;
        for (i, &word) in program.iter().enumerate() {
            dst.add(i).write(word);
        }
    }
    // A no-op on this architecture (the instruction cache is architecturally coherent), and called
    // anyway because the seam is what the other two need and skipping it here would make this code
    // wrong to copy.
    sync_icache(
        mmu::phys_to_virt(code_phys),
        core::mem::size_of_val(program),
    );
    user_address_space_map(
        aspace,
        X86_DEMO_CODE_VA,
        code_phys,
        Flags::user_code(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .map_err(|_| "could not map the child's code")?;

    let stack_phys = crate::memory_region::retype_page(region).ok_or("no stack frame")?;
    user_address_space_map(
        aspace,
        X86_DEMO_STACK_VA,
        stack_phys,
        Flags::user_data(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .map_err(|_| "could not map the child's stack")?;

    let tid = crate::sched::create_thread_control_block(region).ok_or("no tcb")?;
    if let Some(first) = slot0 {
        let slot = crate::sched::thread_control_block_insert_cap(tid, first, None)
            .map_err(|_| "no room for the child's slot 0")?;
        if slot != 0 {
            return Err("the child's capability did not land in slot 0, which its code assumes");
        }
    }
    if let Some(ep) = fault_ep {
        // The spawn-slot convention: a supervision endpoint goes in the reserved fault slot, and
        // the kernel consumes it at START so the child cannot forge fault messages on it.
        let capability = crate::cap::rendezvous_cap(ep, crate::cap::Rights::READ);
        crate::sched::thread_control_block_insert_cap(
            tid,
            capability,
            Some(abi::fault::FAULT_EP_SLOT),
        )
        .map_err(|_| "no room for the fault endpoint")?;
    }
    crate::sched::configure_thread_control_block(
        tid,
        X86_DEMO_CODE_VA,
        X86_DEMO_STACK_VA + FRAME_SIZE,
        aspace,
    )
    .map_err(|_| "could not configure the child")?;
    crate::sched::start_thread_control_block(tid, [0; 3])
        .map_err(|_| "could not start the child")?;
    Ok(tid)
}

/// **Prove there is a userspace on `x86_64`**, which is the claim roadmap item 4 exists to make and
/// is a strictly larger one than item 3's ring-3 probe.
///
/// Two children, because the two halves of "a process" fail differently and a single program that
/// did both could hide one behind the other:
///
///   - **One reports and exits.** Its whole world (address space, code page, stack page, TCB) is
///     carved from one untyped region, it is dispatched to ring 3 by the *scheduler* rather than by
///     a hand-written entry path, it invokes a capability it was granted, and the word it SENDs
///     arrives here. That is the loader-shaped path minus the ELF: every kernel object a process
///     needs, built from a budget, in the order a real spawn builds them.
///   - **One faults.** It loads from an address nothing maps, the page tables refuse it, and the
///     trap path turns that into a supervision message naming the thread, the pc and the address.
///     Until this item the same arm recorded the fault and then panicked, because there was no
///     thread to kill.
///
/// And then both regions are destroyed and the frame count is compared, because a userspace that
/// leaks its processes is not one.
///
/// Name: provisional (milestone 161, roadmap item 4).
#[cfg(target_arch = "x86_64")]
pub fn x86_userspace_demo() -> Result<X86UserspaceReport, &'static str> {
    let before = crate::memory::free_page_frames();
    let round = x86_userspace_round()?;
    let after_first = crate::memory::free_page_frames();
    // The same two children again, from scratch. See `X86UserspaceReport::second_round_frames`.
    x86_userspace_round()?;
    let after_second = crate::memory::free_page_frames();

    Ok(X86UserspaceReport {
        first_round_frames: before as isize - after_first as isize,
        second_round_frames: after_first as isize - after_second as isize,
        ..round
    })
}

/// One round of the demo: build both children, collect what each produced, and give their regions
/// back. Called twice by [`x86_userspace_demo`], which is what turns its frame numbers into
/// evidence.
///
/// **Every kernel object a child needs comes out of that child's own region**, its two endpoints
/// included (`create_rendezvous_from`), so one `DESTROY` reclaims the whole of it and the frame
/// count is an exact statement rather than an approximate one. The first version drew the endpoints
/// from the kernel's shared pool and never collected the reporting child's corpse, and the tour
/// reported sixteen frames a round going missing: correct, and exactly the kind of thing a
/// steady-state number is for.
#[cfg(target_arch = "x86_64")]
fn x86_userspace_round() -> Result<X86UserspaceReport, &'static str> {
    use abi::fault::{EVENT_EXIT, EVENT_FAULT};

    // Sixteen pages is what the supervision fixtures give a child on the other two architectures:
    // an address space's root and tables, a code page, a stack page, a TCB, and here two endpoints.
    let report_region =
        crate::memory_region::create(16).ok_or("no region for the reporting child")?;
    let report_ep =
        crate::sched::create_rendezvous_from(report_region).ok_or("no reporting endpoint")?;
    let reporter_supervisor = crate::sched::create_rendezvous_from(report_region)
        .ok_or("no supervision endpoint for the reporting child")?;
    let reporter = x86_build_child(
        report_region,
        &x86_programs::report(X86_DEMO_WORD),
        Some(crate::cap::rendezvous_cap(
            report_ep,
            crate::cap::Rights::WRITE,
        )),
        Some(reporter_supervisor),
    )?;
    let reported = crate::sched::ipc_receive(report_ep)[0];

    // **Collect the corpse before reclaiming the region**, which is what a supervisor is for and
    // what the first draft of this left out: a region still holding a live TCB is refused, and the
    // refusal is silent because `destroy` has nowhere to report it.
    let exit = crate::sched::ipc_receive(reporter_supervisor);
    if exit[0] != EVENT_EXIT {
        return Err("the reporting child's clean exit did not arrive as an EXIT event");
    }
    crate::sched::reap_supervised(reporter_supervisor, reporter)
        .map_err(|_| "the reporting child's corpse refused to be reaped")?;

    // The faulting child, in a region of its own.
    let fault_region =
        crate::memory_region::create(16).ok_or("no region for the faulting child")?;
    let fault_ep = crate::sched::create_rendezvous_from(fault_region)
        .ok_or("no supervision endpoint for the faulting child")?;
    let child = x86_build_child(
        fault_region,
        &x86_programs::fault(X86_DEMO_BAD_ADDR),
        None,
        Some(fault_ep),
    )?;
    let msg = crate::sched::ipc_receive(fault_ep);
    if msg[0] != EVENT_FAULT {
        return Err("the child's death did not arrive as a FAULT event");
    }
    if msg[1] != child {
        return Err("the fault message named the wrong thread");
    }
    if msg[2] != X86_DEMO_CODE_VA + x86_programs::FAULT_PC_OFFSET {
        return Err("the faulting pc was not the load instruction");
    }
    if msg[3] != X86_DEMO_BAD_ADDR as u64 {
        return Err("the faulting address was not carried in the message");
    }
    crate::sched::reap_supervised(fault_ep, child)
        .map_err(|_| "the faulting child's corpse refused to be reaped")?;

    crate::memory_region::destroy(report_region);
    crate::memory_region::destroy(fault_region);

    Ok(X86UserspaceReport {
        reported,
        faulted_tid: msg[1],
        fault_pc: msg[2],
        fault_addr: msg[3],
        first_round_frames: 0,
        second_round_frames: 0,
    })
}

#[cfg(target_arch = "riscv64")]
pub fn riscv_least_authority_demo(least_authority_demo: &[u8], n: u64) -> Result<u64, LoadError> {
    // The kernel's real loader: parse, build the address space, map the W^X segments and a stack.
    let (space, entry) = load(least_authority_demo, 0)?;
    // `load` returns an owned AddressSpace; the TCB path binds one by registry name, so register it.
    let aspace_name = readopt_user_address_space(space).expect("register the loaded address space");

    // The least_authority_demo's one authority: WRITE on a report endpoint, which it will hold as slot 0.
    let result = crate::sched::create_rendezvous();
    let result_cap = crate::cap::rendezvous_cap(result, crate::cap::Rights::WRITE);

    // Build the thread from parts: a TCB, the cap in slot 0, configure at the ELF's entry, start.
    let thread_control_block_region = crate::memory_region::create(2).expect("no tcb region");
    let tid =
        crate::sched::create_thread_control_block(thread_control_block_region).expect("no tcb");
    let slot =
        crate::sched::thread_control_block_insert_cap(tid, result_cap, None).expect("cap insert");
    assert_eq!(
        slot, 0,
        "the least_authority_demo's report cap must land in slot 0"
    );
    crate::sched::configure_thread_control_block(tid, entry, USER_STACK_TOP, aspace_name)
        .expect("configure");
    // The least_authority_demo reads its input from a1 (the second argument); a0 and a2 are unused.
    crate::sched::start_thread_control_block(tid, [0, n, 0]).expect("start");

    Ok(crate::sched::ipc_receive(result)[0])
}

/// **Start the interrupt-driven UART driver as an unprivileged userspace process** (milestone 20).
///
/// The device-interrupt story's real form: a driver that owns the UART's interrupt by *capability*,
/// not by privilege. The kernel loads `driver` from the archive, builds its address space, maps the
/// NS16550's registers into it device-typed (so the driver reads the byte itself; the kernel is not
/// in the data path), and grants it exactly two capabilities: an `Irq` capability for the UART
/// interrupt (slot 0) and a report endpoint (slot 1). It routes the interrupt to the endpoint the
/// `Irq` cap waits on, starts the driver, and arms the source (PLIC), the receive interrupt (UART),
/// and supervisor external interrupts (`sie.SEIE`).
///
/// Returns the report endpoint. This does **not** block: the caller spawns a receiver so the boot
/// tour continues, and the driver's `WAIT`/read/report/`ACK` loop runs whenever a byte arrives. The
/// `ACK` is the point of the whole exercise: it crosses the `arch::irq` seam (the PLIC on RISC-V, the
/// GIC on aarch64) to re-arm the source, from an unprivileged process holding only a capability.
#[cfg(target_arch = "riscv64")]
pub fn riscv_uart_driver_demo(
    archive: &'static [u8],
    uart_irq: u32,
) -> Result<crate::sched::RendezvousId, LoadError> {
    const DRIVER_UART_VA: u64 = address_space_map::pair_page(0x0070_0000); // must match components/src/serial_driver.rs UART_VA
    const UART_PHYS: u64 = 0x1000_0000; // the NS16550 on QEMU virt

    let fs = nifefs::Fs::parse(archive).expect("initrd is not a nifefs archive");
    let driver_bytes = fs
        .read("serial_driver")
        .expect("archive has no 'serial_driver' program");
    let elf = Elf::parse(driver_bytes).map_err(LoadError::NotLoadable)?;

    // The driver's address space: its segments, a stack, and the UART's registers device-typed.
    let content: u64 = elf
        .segments()
        .map(|seg| {
            let (s, e) = seg.page_range(FRAME_SIZE);
            (e - s) / FRAME_SIZE
        })
        .sum::<u64>()
        + 1
        + INIT_STACK_PAGES
        + 8;
    let mut space =
        AddressSpace::new(content).ok_or(LoadError::Unmappable(MapError::OutOfPageFrames))?;
    map_segments(&mut space, &elf)?;
    for k in 0..INIT_STACK_PAGES {
        space
            .map_new(USER_STACK_VA - k * FRAME_SIZE, Flags::user_data())
            .map_err(LoadError::Unmappable)?;
    }
    // The UART registers, device-typed and user-accessible: the driver reads RBR/LSR directly.
    space
        .map_physical(
            DRIVER_UART_VA,
            UART_PHYS,
            Flags::user_device(),
            crate::revoke::PageMapSource::NoCapability,
        )
        .map_err(LoadError::Unmappable)?;

    let aspace_name = readopt_user_address_space(space).expect("register driver address space");

    // Route the UART interrupt to an endpoint; the Irq cap's WAIT blocks on it. The report endpoint
    // is where the driver SENDs each byte, and where the caller's receiver waits.
    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(uart_irq, irq_ep);
    let report = crate::sched::create_rendezvous();

    let thread_control_block_region = crate::memory_region::create(2).expect("no tcb region");
    let tid =
        crate::sched::create_thread_control_block(thread_control_block_region).expect("no tcb");
    // slot 0: the Irq capability (READ permits WAIT/ACK). slot 1: the report endpoint (WRITE).
    let s0 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::irq_cap_rights(uart_irq, crate::cap::Rights::READ),
        None,
    )
    .expect("insert irq cap");
    assert_eq!(s0, 0, "the Irq cap must land in slot 0");
    let s1 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::rendezvous_cap(report, crate::cap::Rights::WRITE),
        None,
    )
    .expect("insert report");
    assert_eq!(s1, 1, "the report endpoint must land in slot 1");
    crate::sched::configure_thread_control_block(tid, elf.entry(), USER_STACK_TOP, aspace_name)
        .expect("configure");
    crate::sched::start_thread_control_block(tid, [0, 0, 0]).expect("start");

    // Arm the whole chain, now that the driver is running and routed: the source at the PLIC, the
    // receive interrupt at the UART, and supervisor external interrupts in `sie`.
    crate::drivers::plic::enable(uart_irq, crate::arch::irq::boot_s_context());
    crate::console::rx_enable();
    crate::arch::exceptions::enable_external();

    Ok(report)
}

/// **Boot the system: load the progenitor and start it as the first process, on every architecture**
/// (milestone 166, which merged aarch64's `spawn_progenitor` boot half and `riscv64`/`x86_64`'s
/// `riscv_shell_boot` into this one body).
///
/// It loads [`PROGENITOR_ENTRY`], measures it under the trust root (milestone 22 phase B.1) and the
/// program-measurement table it will check its own loads against (milestone 104), builds its address
/// space (its segments, a deep stack, and the whole initrd mapped read-only so it can parse and load
/// the rest by name), endows it with the boot capability set, and starts it. From those capabilities
/// and nothing else, `crates/system_initializer` builds the console server, the input driver, the
/// line discipline and the shell out of the progenitor's own budget and wires them together; the
/// kernel touches none of it. It does not block: the progenitor and its children run on the
/// scheduler while the boot thread parks.
///
/// **The capability slot layout is the same on all three architectures** (milestone 166's point):
/// the delegable root budget at slot 0, the console device at slot 1, the UART receive interrupt at
/// slot 2, the wall clock page read-only at slot 3 (milestone 51's wiring), the inert-configuration
/// page read-only at slot 4 (milestone 47's environment-variable fork, DECISIONS §111), the file
/// service and the page its clients share at slots 5 and 6 when a RedoxFS disk is attached (milestone
/// 50), the virtio-rng trio at 7-9, the graphical terminal stack at 10-12 and the virtio-net trio
/// at 13-15 (milestone 590 (the booted system starts its network stack)) when each is present.
/// That fills sixteen of the table's
/// thirty-two slots at spawn (the GPU and keyboard grants at 17-22 and the machine statistics page
/// at 23 came later, the progenitor's own address space at 28, §249, later still, and the reboot
/// object at 31, milestone 805, last), which is why the progenitor spends the net trio before anything else.
/// `components/src/progenitor.rs`'s single `GRANTS` table reads exactly this. Until milestone 166
/// aarch64's boot carried two extra capabilities at slots 1 and 3 (a report endpoint and a test
/// interrupt) that the interactive system never used, only because its loader was shared with
/// milestone 19d's test roles; [`spawn_hello`] is that shared role path, now boot-free.
///
/// **Only two differences are the hardware's, and only those are `#[cfg]`-gated**:
/// - **The console device (slot 1).** aarch64 and riscv64 grant the UART's registers as a
///   `DeviceFrame` (a page), `WRITE|GRANT` so the progenitor maps them into the console and input
///   drivers it builds. `x86_64` has no page for its console (COM1 is port I/O), so it grants a
///   `PortRange` over `0x3F8..=0x3FF` instead (milestone 299, DECISIONS §121 reversed 2026-09-15):
///   the progenitor delegates it with `CAP_INSERT` rather than `MAP_INTO`, and the kernel's TSS I/O
///   bitmap is what lets the drivers' `in`/`out` reach exactly these ports.
/// - **Arming the interrupt controller.** aarch64's GIC has no boot-hart lottery, so its UART line
///   is enabled inline before the thread is built and its virtio-rng source as its caps are inserted;
///   riscv64 enables the PLIC source and supervisor external interrupts only after the driver is
///   running, because the lottery forbids arming earlier (see [`VirtioBootGrant::intid`]); `x86_64`
///   arms nothing here, because it has no userspace input driver to feed until DECISIONS §149.
///
/// Returns the progenitor's thread, so the caller can say how it left.
///
/// Name: ratified 2026-09-15 (calef, this header). Refused `boot_via_progenitor` (the provisional
/// name from milestone 268, whose `via` did two jobs and has spent both: it disambiguated this entry
/// from aarch64's separate boot path, which milestone 166 unified away, and it gestured at the
/// microkernel indirection, which the sentence "on this path the progenitor **is** the system" says
/// better than a preposition in a name can). `boot` is the term-of-art verb and `progenitor` the
/// noun it acts on, so the name claims this function's own action, load the `progenitor` program,
/// measure it, and start it, rather than the system bring-up `progenitor` itself does next. Greps
/// with [`PROGENITOR_ENTRY`] and [`PROGENITOR_ROLE`] as one family.
// One caller per architecture, all in `kernel::main`'s hand-off (aarch64's default boot, and
// `riscv_hand_over`/`x86_hand_over`). The `allow` is kept for the configurations that reach none of
// them: a `soak` or `job_mix` build replaces the hand-off with its own workload, and `test`/`bench`
// park before it.
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput",
        feature = "network_bench"
    ),
    allow(dead_code)
)]
pub fn boot_progenitor(archive: &'static [u8]) -> Result<crate::thread::ThreadId, LoadError> {
    use crate::cap::Rights;

    // The UART receive interrupt line, from the machine's own description when it gave one
    // (`memory::uart_irq`), else QEMU virt's constant. The source is printed so a bench transcript
    // names where the number came from: a wrong PLIC source once cost a boot on the JH7110
    // (notes/visionfive2.md, BUGS).
    let (uart_irq, uart_irq_source) = uart_irq_and_source();
    crate::println!("  uart irq  : {uart_irq} ({uart_irq_source})");

    let (initrd_start, initrd_len) = memory::initrd_region().expect("no initrd region");
    let initrd_pages = initrd_len.div_ceil(FRAME_SIZE);

    let fs = nifefs::Fs::parse(archive).expect("initrd is not a nifefs archive");
    let init_bytes = fs
        .read(PROGENITOR_ENTRY)
        .unwrap_or_else(|| panic!("archive has no '{PROGENITOR_ENTRY}' program"));
    // Measured boot (milestone 22 phase B.1): the progenitor is this board's boot program too, so
    // it is in the trust root under its own name and checked here, before its address space is
    // built.
    crate::trust::require(PROGENITOR_ENTRY, init_bytes);
    // And the table it measures the six boot components and every spawnable program against
    // (milestone 104). This is the boot path that genuinely uses it: `crates/system_initializer` is
    // the same code aarch64's the progenitor runs, so the two boards extend the chain by the same lines.
    crate::trust::require_program_measurements(&fs);
    let elf = Elf::parse(init_bytes).map_err(LoadError::NotLoadable)?;

    // system_initializer's address space: its segments, a deep stack (it runs an ELF loader that builds three
    // children), and the whole archive mapped read-only so it can load them by name.
    //
    // `log_pages_for(initrd_pages)` is the term that was not here before 2026-09-21: the archive's
    // pages are mapped with `map_physical`, which records now, and a record is paid for out of this
    // space's own region like the page tables beside it. It is the largest single such window in
    // the tree (the aarch64 archive is a few thousand pages), so it is the one place the cost is
    // visible rather than lost in `AS_OVERHEAD`'s slack. See `crate::revoke::log_pages_for`.
    let content: u64 = elf
        .segments()
        .map(|seg| {
            let (s, e) = seg.page_range(FRAME_SIZE);
            (e - s) / FRAME_SIZE
        })
        .sum::<u64>()
        + 1
        + initrd_pages / 512
        + crate::revoke::log_pages_for(initrd_pages)
        + INIT_STACK_PAGES
        + 8;
    let mut space =
        AddressSpace::new(content).ok_or(LoadError::Unmappable(MapError::OutOfPageFrames))?;
    map_segments(&mut space, &elf)?;
    for k in 0..INIT_STACK_PAGES {
        let page = space
            .map_new(USER_STACK_VA - k * FRAME_SIZE, Flags::user_data())
            .map_err(LoadError::Unmappable)?;
        // Painted so the kernel can say how deep this stack has ever been: see
        // `crate::progenitor_stack`, and `script/swish-check`, which fails on too little headroom.
        crate::progenitor_stack::paint_page(k, page);
    }
    // The timebase page, which [`load`] maps for every process it builds and a hand-built
    // address space has to map for itself (see [`map_timebase_page`] for the six call sites
    // that each found this as a page fault). The progenitor reads the clock like any program does.
    #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
    map_timebase_page(&mut space).map_err(LoadError::Unmappable)?;
    for i in 0..initrd_pages {
        space
            .map_physical(
                INITRD_VA + i * FRAME_SIZE,
                initrd_start + i * FRAME_SIZE,
                Flags::user_rodata(),
                crate::revoke::PageMapSource::NoCapability,
            )
            .map_err(LoadError::Unmappable)?;
    }
    let aspace_name =
        readopt_user_address_space(space).expect("register the progenitor's address space");

    // Route the UART receive interrupt to an endpoint; the input driver's Irq cap will WAIT on it.
    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(uart_irq, irq_ep);
    // aarch64's GIC has no boot-hart-lottery hazard, so its UART line is enabled here, the same
    // place and way the old `spawn_progenitor` armed it. riscv64 must wait until the driver is
    // running (the arming block after the start below). `x86_64` arms COM1's IRQ 4 here too, since
    // milestone 505 (an x86_64 input driver that never lets the core idle): the IO APIC entry goes
    // live now, and nothing raises the line until the input driver sets the 16550's receive enable.
    #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
    crate::arch::irq::enable(uart_irq);
    let build_region =
        crate::memory_region::create(12288).expect("no building budget for the progenitor");

    let thread_control_block_region = crate::memory_region::create(2).expect("no tcb region");
    let tid =
        crate::sched::create_thread_control_block(thread_control_block_region).expect("no tcb");
    // slot 0: the delegable root budget (milestone 31), GRANT included so the progenitor can split
    // off a budget for the shell and hand it on; rights only narrow downward. slot 1: the console
    // device (see below), WRITE|GRANT so the progenitor delegates it into the console and input
    // drivers. slot 2: the UART Irq, READ|GRANT so it can delegate it to input.
    let s0 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::memory_region_root_cap(build_region),
        None,
    )
    .expect("insert budget");
    assert_eq!(s0, 0);
    // Slot 1: the console device. On aarch64 and riscv64 it is the UART's registers as a
    // `DeviceFrame` (a page); on `x86_64` the console is port I/O with no page, so it is a
    // `PortRange` over COM1's eight ports instead. See this function's doc for the full split.
    #[cfg(not(target_arch = "x86_64"))]
    let uart_slot = crate::cap::device_frame_cap(UART_PHYS, Rights::WRITE.union(Rights::GRANT));
    // **On `x86_64` the console is a port range, not a page** (milestone 299, DECISIONS §121
    // reversed 2026-09-15). COM1's 16550 lives at I/O ports `0x3F8..=0x3FF`, so `uart_dev` names a
    // `PortRange` capability rather than a device page: the progenitor holds it with `GRANT` and
    // delegates it (`CAP_INSERT`, not `MAP_INTO`) into the console and input drivers it builds, and
    // the kernel's TSS I/O bitmap is what lets their `in`/`out` reach exactly these ports.
    #[cfg(target_arch = "x86_64")]
    let uart_slot = crate::cap::port_range_cap(
        X86_COM1_PORT_BASE,
        X86_COM1_PORT_COUNT,
        Rights::WRITE.union(Rights::GRANT),
    );
    let s1 = crate::sched::thread_control_block_insert_cap(tid, uart_slot, Some(1))
        .expect("insert uart device");
    assert_eq!(s1, 1);
    let s2 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::irq_cap_rights(uart_irq, Rights::READ.union(Rights::GRANT)),
        Some(2),
    )
    .expect("insert uart irq");
    assert_eq!(s2, 2);
    // The clock page (slot 3), read-only, ahead of the filesystem pair so its number is the same on
    // every boot. `READ` is DECISIONS §43's split at this boundary: the progenitor can endow a reader and holds
    // nothing that could set the time. See [`boot_clock_page`].
    let s3 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::page_frame_cap(boot_clock_page(), Rights::READ.union(Rights::GRANT)),
        Some(3),
    )
    .expect("insert the clock page");
    assert_eq!(s3, 3);
    // The inert-configuration page (slot 4), `clock`'s twin (milestone 47, DECISIONS §111): ahead
    // of the filesystem pair for the identical reason, its slot must not depend on whether a disk
    // was attached. See [`boot_config_page`].
    let s4 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::page_frame_cap(boot_config_page(), Rights::READ.union(Rights::GRANT)),
        Some(4),
    )
    .expect("insert the config page");
    assert_eq!(s4, 4);
    // The file service (slot 5) and the page its clients share with it (slot 6), when this boot has
    // a filesystem (milestone 50). GRANT on both, because the progenitor's job with them is to delegate: it
    // narrows the endpoint into the shell and maps the frame into its address space. `a2` carries
    // the rights the endpoint holds, which is also how the progenitor is told there is one at all. `None` is
    // the ordinary case for a run with no RedoxFS disk attached.
    let fs_rights = match program("redoxfs_server").and_then(|redoxfs_server| {
        fs_service::root_directory(fs_service::blk_server_image(), redoxfs_server)
    }) {
        Some((file_ep, file_shared)) => {
            let s5 = crate::sched::thread_control_block_insert_cap(
                tid,
                crate::cap::rendezvous_cap(file_ep, Rights::WRITE.union(Rights::GRANT)),
                Some(5),
            )
            .expect("insert the file service");
            assert_eq!(s5, 5);
            let s6 = crate::sched::thread_control_block_insert_cap(
                tid,
                // **The whole client-window pool, one run** (milestone 599 (a frame per filesystem
                // client channel), calef's option-4 ruling of 2026-09-27): every window, in the
                // slot that held window 0's page. The progenitor never maps it into a client; it
                // slices one window per client with `abi::page_frame::SLICE` and deletes the slice
                // once the client is built.
                crate::cap::page_frame_run_cap(
                    file_shared,
                    crate::cap::page_frame_run_len(fs_service::FILE_POOL_PAGES),
                    Rights::WRITE.union(Rights::GRANT),
                ),
                Some(6),
            )
            .expect("insert the shared file page");
            assert_eq!(s6, 6);
            filesystem_protocol::dir::ALL
        }
        None => 0,
    };
    // The virtio-rng device (slots 7-9, always, even without a disk), when this boot has one
    // (DECISIONS §120's 2026-08-26 amendment). GRANT on all three so system_initializer can
    // delegate them onward to an entropy service it builds, the same shape every other device
    // authority here already takes. `None` exactly as the filesystem pair can be:
    // system_initializer's own probe (`invoke` on an ungranted slot answers `NoSuchSlot`) is what
    // tells it apart from a real one. See [`boot_virtio_rng_device`] for what wiring it costs and
    // why it hands back an un-enabled interrupt.
    //
    // **Explicit slots, not `None`'s first-free** (`thread_control_block_insert_cap`'s own second
    // sense, `notes/abi.md` §4's "the emptiness is load-bearing"), and this is not a style choice:
    // the filesystem pair above is itself conditional, and first-free numbering would silently
    // shift these three down by two on the (ordinary) boot that has virtio-rng but no attached
    // disk. Explicit targets keep slots 7-9 the virtio-rng trio's own regardless of what the
    // filesystem pair did or did not consume. Fixed past the pair's own max reach (slot 6), not
    // slot 5, because the inert-configuration page (slot 4) shifted that pair down by one.
    let virtio_rng = boot_virtio_rng_device();
    if let Some(g) = &virtio_rng {
        let s7 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::virtio_cap_rights(g.vid, Rights::WRITE.union(Rights::GRANT)),
            Some(7),
        )
        .expect("insert the virtio-rng transport");
        assert_eq!(s7, 7);
        let s8 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::irq_cap_rights(g.intid, Rights::READ.union(Rights::GRANT)),
            Some(8),
        )
        .expect("insert the virtio-rng interrupt");
        assert_eq!(s8, 8);
        let s9 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::page_frame_cap(
                g.dma,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            Some(9),
        )
        .expect("insert the virtio-rng DMA page");
        assert_eq!(s9, 9);
        // aarch64's GIC has no hart lottery, so its virtio-rng source is enabled here, inline,
        // exactly as the old `spawn_progenitor` did before the thread started. riscv64's is armed at
        // the PLIC in the block after the start below; `x86_64` arms nothing.
        #[cfg(target_arch = "aarch64")]
        crate::arch::irq::enable(g.intid);
    }
    // **Or the CPU's own seed instruction** (slot 16, milestone 595 (provisional)), when there is
    // no virtio-rng: an entropy service the *kernel* built and proved, granted as its request
    // endpoint, the way the file service in slot 5 is. See [`boot_instruction_entropy`] for why the
    // kernel builds this one rather than the progenitor, and why the grant is never both.
    if virtio_rng.is_none()
        && let Some(request) = boot_instruction_entropy()
    {
        let s16 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::rendezvous_cap(request, Rights::WRITE.union(Rights::GRANT)),
            Some(16),
        )
        .expect("insert the instruction entropy service");
        assert_eq!(s16, 16);
    }
    // **The graphical terminal stack's raw materials** (milestone 600 (provisional); milestone 177 (wire the graphical terminal stack into the real interactive boot)
    // built the stack itself here), when a virtio-gpu is attached: the gpu's confined transport,
    // interrupt and DMA run, and the surface run inside it, in slots 17, 18, 19 and 12. Nothing
    // is built from them at boot (milestone 632 (provisional), calef's 2026-09-30 ruling: the
    // boot stays the minimal UART system and graphics is launched from the swish prompt); the
    // progenitor hands all seven to the shell, which holds them until a `graphical_terminal` session's spawn
    // hands them back for the drivers to be built from, exactly as it hands the machine
    // statistics page to a session that may delegate it.
    //
    // **Why the kernel grants rather than builds.** Milestone 177 spawned the stack kernel-side
    // because a virtio-gpu "needed eleven capability-table slots, one `PageFrame` per DMA page".
    // DECISIONS §102 (a Frame names a run of pages) had already given `PageFrame` a page count,
    // and milestone 142 (a text display good enough that people use it instead of a GUI) made the gpu's
    // region one run and one capability (`display_service::wire_device`), so the premise had
    // expired: the gpu is four capabilities here, and the keyboard three.
    //
    // **Two frame capabilities over one region, deliberately.** The driver gets the whole run and
    // the terminal only the surface after its first page, which is the split
    // `display_service::start_terminal` already makes; nothing lets a holder narrow a run, so the
    // kernel mints both. `None` with no GPU on the bus, or no `gpu_driver` or `display_terminal`
    // in the archive: no device is wired for programs this boot could never run.
    let gpu = if program("gpu_driver").is_some() && program("display_terminal").is_some() {
        display_service::wire_device()
    } else {
        None
    };
    // **And a virtio keyboard, when this boot has a gpu too** (slots 20-22), the rng trio's
    // shape exactly. `None` is milestone 192 (a keyboard on real silicon)'s option A, not an
    // absence: a `graphical_terminal` session then takes its keystrokes from the boot's own UART line
    // discipline, at launch rather than at boot. Wired only beside a GPU, because a keyboard
    // with no screen has no terminal to type into on this boot.
    let keyboard = if gpu.is_some() && program("keyboard_driver").is_some() {
        keyboard_service::wire_device()
    } else {
        None
    };
    if gpu.is_some() {
        crate::println!(
            "  graphics  : a virtio-gpu and {}; the spawn service holds the grants, a `graphical_terminal` launch builds from them",
            if keyboard.is_some() {
                "a virtio keyboard"
            } else {
                "no keyboard (a graphical terminal session's keystrokes come over the UART)"
            }
        );
    }
    // **The machine statistics page** (slot 23, milestone 126 (the `procps` package), DECISIONS §225
    // (`free` sees the machine and your share) part 2), the config page's shape: a frame the kernel
    // keeps its machine-wide counters in, granted unconditionally so its slot never moves, and
    // `READ | GRANT` so the progenitor can hand it to the boot shell and let nobody write it.
    //
    // Slot 23 because every slot below it is named by a grant some boot makes, and the page is
    // granted on every boot. It was the kernel's fault slot until calef raised the table to 32 on
    // 2026-09-27 for exactly this (`crate::cap::CAPABILITY_TABLE_SLOTS`). See
    // `crate::machine_statistics`.
    let s23 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::page_frame_cap(
            crate::machine_statistics::page_phys(),
            Rights::READ.union(Rights::GRANT),
        ),
        Some(23),
    )
    .expect("insert the machine statistics page");
    assert_eq!(s23, 23);
    // **The progenitor's own address space** (slot 28, §249 (a running address space stays
    // nameable), its 2026-10-05 amendment; field name `own_space`, provisional). `WRITE` alone, so
    // the progenitor can `UNMAP` each scratch page it filled for a child once the page is in the
    // child (milestone 95 (an unmap primitive)), and `MAP_INTO` its own space, which `PageFrame::MAP`
    // already let it do. No `GRANT`: nothing else is ever handed authority over the progenitor's
    // memory, and a right it does not hold is one it cannot pass on by mistake. No `ENUMERATE`
    // either, because nothing it does needs to list itself.
    //
    // Granted on every boot so its slot never moves, past the USB keyboard's conditional slot 27 for
    // the reason every conditional group above gives. It names `aspace_name`, which `CONFIGURE`
    // below no longer retires (§249's option A), so this capability goes on resolving while the
    // progenitor runs and goes dead the moment its thread is reaped.
    let s28 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::address_space_cap(aspace_name, Rights::WRITE),
        Some(28),
    )
    .expect("insert the progenitor's own address space");
    assert_eq!(s28, 28);
    // **The reboot object** (slot 31, milestone 805 (`reboot` at the prompt), DECISIONS §251
    // (restarting the machine is a kernel object the progenitor hands out)): the one capability
    // that may restart the machine, minted here and nowhere else. `WRITE | GRANT`: the progenitor
    // never invokes it, but it places `WRITE` in the one child whose manifest declares `reboot`,
    // and delegation only narrows, so the right it hands on has to be one it holds (a `GRANT`-only
    // grant here made every `reboot` spawn fail, found by the first run that typed `reboot`).
    // The method itself checks no right, as §251 says. Granted on every boot so its slot
    // never moves, past `net_stack_report`'s conditional slot 30 for the reason every group above
    // gives. Field name `reboot`, provisional.
    let s31 = crate::sched::thread_control_block_insert_cap(
        tid,
        crate::cap::reboot_capability(Rights::WRITE.union(Rights::GRANT)),
        Some(31),
    )
    .expect("insert the reboot object");
    assert_eq!(s31, 31);
    // **The kernel's ring, its cursor page and its notification** (slots 24 to 26, milestone 342
    // (the kernel and the `console` server drive one UART from two address spaces), calef's
    // ruling F): the ring read-only so the log service can copy kernel lines out and never write
    // them, the cursor read-write so it can say how far it has read, and the notification the
    // kernel signals on append, which the progenitor binds to the service's thread. All three
    // carry `GRANT`, because the progenitor is the spawner and not the reader. Empty on a boot that
    // could not allocate them, which keeps the kernel's old behaviour: every line direct.
    crate::kernel_log::publish();
    if let Some((ring, cursor, notification)) = crate::kernel_log::grants() {
        for (capability, slot, what) in [
            (
                crate::cap::page_frame_run_cap(
                    ring,
                    core::num::NonZeroU64::new(system_log_protocol::kernel_ring::PAGES as u64)
                        .expect("the ring has pages"),
                    Rights::READ.union(Rights::GRANT),
                ),
                24,
                "the kernel's ring",
            ),
            (
                crate::cap::page_frame_cap(
                    cursor,
                    Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
                ),
                25,
                "the kernel ring's cursor page",
            ),
            (
                crate::cap::notification_cap(
                    notification,
                    Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
                ),
                26,
                "the kernel ring's notification",
            ),
        ] {
            let s = crate::sched::thread_control_block_insert_cap(tid, capability, Some(slot))
                .unwrap_or_else(|_| panic!("insert {what}"));
            assert_eq!(s, slot, "{what} landed in the wrong slot");
        }
    }
    // **A USB keyboard** (slot 27, milestone 242 (USB host and HID)): the attach endpoint of a
    // driver the kernel already started on the machine's xHCI controller, `WRITE | GRANT`, so the
    // progenitor can delegate the line discipline's endpoint through it once it has built one and
    // then delete its own copy. Past the kernel ring's floor (slot 26) for the reason every
    // conditional group above gives. Empty on a machine with no controller, one the kernel refused,
    // or one whose driver reported a failure (nobody would receive the delegation). See
    // [`boot_usb_keyboard`].
    let usb_keyboard = boot_usb_keyboard();
    if let Some(k) = &usb_keyboard {
        let s27 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::rendezvous_cap(k.attach, Rights::WRITE.union(Rights::GRANT)),
            Some(27),
        )
        .expect("insert the USB keyboard's attach endpoint");
        assert_eq!(s27, 27);
    }
    // **Or a terminal on the screen the firmware left running** (the shell on the firmware screen,
    // milestone 198's rung 1b), when there is no GPU: slots 10 and 11, the terminal's endpoint and
    // its output page. (They were the graphical stack's slots too, until milestone 600
    // (provisional) moved that stack's construction into the progenitor.) `None` on every machine whose
    // console has no screen, which is every boot but a UEFI one today. See
    // [`boot_screen_terminal`].
    let screen = if gpu.is_none() {
        boot_screen_terminal()
    } else {
        None
    };
    if let Some(t) = &screen {
        let s10 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::rendezvous_cap(t.term, Rights::WRITE.union(Rights::GRANT)),
            Some(10),
        )
        .expect("insert the screen terminal's endpoint");
        assert_eq!(s10, 10);
        let s11 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::page_frame_cap(
                t.out,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            Some(11),
        )
        .expect("insert the screen terminal's output page");
        assert_eq!(s11, 11);
    }
    let insert = |granted, slot: u64, what: &str| {
        let s = crate::sched::thread_control_block_insert_cap(tid, granted, Some(slot))
            .unwrap_or_else(|_| panic!("insert {what}"));
        assert_eq!(s, slot, "{what} landed in the wrong slot");
    };
    if let Some(g) = &gpu {
        insert(
            crate::cap::page_frame_run_cap(
                g.surface,
                display_service::GPU_SURFACE_RUN,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            12,
            "the gpu's surface run",
        );
        insert(
            crate::cap::virtio_cap_rights(g.vid, Rights::WRITE.union(Rights::GRANT)),
            17,
            "the gpu transport",
        );
        insert(
            crate::cap::irq_cap_rights(g.intid, Rights::READ.union(Rights::GRANT)),
            18,
            "the gpu interrupt",
        );
        insert(
            crate::cap::page_frame_run_cap(
                g.dma,
                display_service::GPU_DMA_RUN,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            19,
            "the gpu's DMA run",
        );
    }
    if let Some(k) = &keyboard {
        insert(
            crate::cap::virtio_cap_rights(k.vid, Rights::WRITE.union(Rights::GRANT)),
            20,
            "the keyboard transport",
        );
        insert(
            crate::cap::irq_cap_rights(k.intid, Rights::READ.union(Rights::GRANT)),
            21,
            "the keyboard interrupt",
        );
        insert(
            crate::cap::page_frame_cap(
                k.dma,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            22,
            "the keyboard's DMA page",
        );
    }
    // **The network card** (slots 13-15, milestone 590 (provisional)), when this boot has a
    // virtio-net device on the MMIO bus: the virtio-rng trio's shape exactly, three slots past the
    // graphical stack's own, explicit for the same reason those are (every conditional grant
    // before these would otherwise shift them). GRANT on all three, because the progenitor's only
    // use for them is to delegate them into the `net_stack` it builds and then delete its own
    // copies; see `crates/system_initializer`'s network block. `None` on every real board today and
    // on any run with `NIFE_NET` unset, and the progenitor's probe tells that apart the way it
    // tells a missing virtio-rng apart. See [`boot_virtio_net_device`].
    let virtio_net = boot_virtio_net_device();
    if let Some(g) = &virtio_net {
        let s13 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::virtio_cap_rights(g.vid, Rights::WRITE.union(Rights::GRANT)),
            Some(13),
        )
        .expect("insert the virtio-net transport");
        assert_eq!(s13, 13);
        let s14 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::irq_cap_rights(g.intid, Rights::READ.union(Rights::GRANT)),
            Some(14),
        )
        .expect("insert the virtio-net interrupt");
        assert_eq!(s14, 14);
        let s15 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::page_frame_cap(
                g.dma,
                Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
            ),
            Some(15),
        )
        .expect("insert the virtio-net DMA page");
        assert_eq!(s15, 15);
        // aarch64 arms inline, riscv64 at the PLIC after the start below: the virtio-rng
        // source's split, for its reason.
        #[cfg(target_arch = "aarch64")]
        crate::arch::irq::enable(g.intid);
    }
    // **Or a stack the kernel already built on an `e1000e`** (slots 29 and 30, milestone 198 (a
    // package manager): rung 3a on x86_64), when there is no virtio-net NIC: the `Stack` endpoint,
    // `READ | WRITE | GRANT` as the progenitor's own retype of it is on the virtio path, and the
    // endpoint the lease arrives on, `READ`. The progenitor receives the lease itself, exactly as
    // it does from a stack it built, so both paths reach the prompt through one line of code. Never
    // both: a boot with the virtio trio leaves these empty. See [`boot_e1000e_network`] for why the
    // kernel builds this one.
    //
    // **Or one built on radon's Ethernet port** (milestone 53 (the board's own peripherals: network
    // and storage on real silicon)), in the same two slots, when there is neither: today that is a
    // line saying the port is left alone, until `designware_ethernet_service::PROVEN_ON_SILICON`.
    let kernel_stack = if virtio_net.is_none() {
        let e1000e = boot_e1000e_network().map(|w| (w.stack, w.report));
        #[cfg(target_arch = "riscv64")]
        let e1000e = e1000e.or_else(boot_designware_network);
        e1000e
    } else {
        None
    };
    if let Some((stack, report)) = kernel_stack {
        let s29 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::rendezvous_cap(stack, Rights::ALL),
            Some(29),
        )
        .expect("insert the kernel-built stack's endpoint");
        assert_eq!(s29, 29);
        let s30 = crate::sched::thread_control_block_insert_cap(
            tid,
            crate::cap::rendezvous_cap(report, Rights::READ),
            Some(30),
        )
        .expect("insert the kernel-built stack's lease endpoint");
        assert_eq!(s30, 30);
    }
    // **Say that this boot worked**, if a chooser started it (rung 2b of milestone 198's other
    // half). Here and not a line earlier or later, and the position is the mechanism: the
    // filesystem server above has mounted the installed disk and reported ready, which is as late
    // a criterion as this system can evaluate without a person, and the progenitor has not started,
    // so the disk's one transfer region still has a single user. `install_service::confirm` argues
    // both halves and names the foot gun in the second.
    #[cfg(target_arch = "x86_64")]
    if fs_rights != 0 {
        install_service::confirm();
    }
    crate::sched::configure_thread_control_block(tid, elf.entry(), USER_STACK_TOP, aspace_name)
        .expect("configure");
    // x0 = the boot role (the progenitor has one role and ignores it, but it is passed for the
    // symmetry the old `spawn_progenitor` established); x1 = the archive length; x2 = the file-service rights.
    crate::sched::start_thread_control_block(tid, [PROGENITOR_ROLE, initrd_len, fs_rights])
        .expect("start");

    // Arm the interrupt chain so the input driver's keystrokes flow. aarch64 already did its arming
    // inline above (the GIC's UART line before the thread was built, its virtio-rng source as the
    // caps were inserted), because the GIC has no boot-hart lottery. What is left here is the
    // hardware that must wait until the driver is running:
    //
    // riscv64: the source at the PLIC and supervisor external interrupts in `sie`. The input driver
    // arms the NS16550's own RX interrupt (its IER) when it starts, and re-arms the PLIC source
    // through its Irq cap's ACK.
    //
    // `x86_64`: nothing. There is no userspace input driver to feed until DECISIONS §149 chooses a
    // console, so arming COM1's line would deliver keystrokes to nobody.
    #[cfg(target_arch = "riscv64")]
    {
        crate::drivers::plic::enable(uart_irq, crate::arch::irq::boot_s_context());
        // The virtio-rng device's own source, pinned to the same boot-hart context for the same
        // reason (`notes/harts-and-pes.md`'s hart lottery; see [`VirtioBootGrant::intid`]'s own doc).
        if let Some(g) = &virtio_rng {
            crate::drivers::plic::enable(g.intid, crate::arch::irq::boot_s_context());
        }
        // The NIC's, for the same reason (milestone 590 (provisional)).
        if let Some(g) = &virtio_net {
            crate::drivers::plic::enable(g.intid, crate::arch::irq::boot_s_context());
        }
        crate::arch::exceptions::enable_external();
    }
    #[cfg(target_arch = "x86_64")]
    let _ = (&virtio_rng, &virtio_net);
    Ok(tid)
}

/// **The USB keyboard, when this machine has an xHCI controller** (milestone 242 (USB host and
/// HID)): start its driver and say what it found. `None`, having printed why, for a controller the
/// kernel would not hand over or a driver that failed; `None` silently for a machine with no
/// controller at all, which is every QEMU boot that attached none.
///
/// **A report of no keyboard still grants the attach endpoint**, because the driver keeps
/// watching its ports and a keyboard plugged in after boot is found: the controller is up, so there
/// is something to delegate to.
///
/// **The kernel starts this driver, not the progenitor**, and the reason is the mappings. The
/// driver is handed a dozen register pages and eleven DMA pages as spawn-time mappings, which a
/// process holds no name for and so can neither delegate nor revoke (`non_volatile_memory_express_service`'s
/// choice, for its reason). Built by the progenitor instead, each would be a capability in its
/// table, and a `DeviceFrame` names one page. Name provisional.
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput"
    ),
    allow(dead_code)
)]
fn boot_usb_keyboard() -> Option<usb_keyboard_service::Wiring> {
    let image = program("usb_keyboard_driver")?;
    match usb_keyboard_service::start(image) {
        Ok(w) => {
            usb_keyboard_service::describe(&w.report);
            (w.report[0] != extensible_host_controller_interface::report::FAILED).then_some(w)
        }
        Err(why) => {
            usb_keyboard_service::describe_refusal(why);
            None
        }
    }
}

/// Bringing the console driver up in userspace, and wiring a client to it.
///
/// **This is the milestone-8 payload.** It creates the shared machinery (two endpoints and a
/// shared page), spawns the console *server* as a user process that owns the UART, and returns
/// what a client needs to reach it. The server binary and the client binary are the *same ELF*,
/// told apart by the argument in `x0`.
// The milestone tour is the only consumer, so this is dead in exactly the configurations that
// have no tour: a test build, and the two alternate boot modes. The allow sits on the module
// because the module is one wiring, not a bag of independent items.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "shell", feature = "bench"),
    allow(dead_code)
)]
pub mod console_service;

/// Bringing the virtio block driver up in userspace.
///
/// **Milestone 9's headline.** The kernel enumerates the bus (kernel/src/virtio.rs) to find the
/// block device, then hands a userspace driver everything it needs and nothing it does not: the
/// device's registers, a DMA page, an interrupt, and an endpoint to report what it read. The
/// kernel does not touch the device.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the tour spawns it; the tests drive it
pub mod virtio_service;

/// **A service's memory, remembered so a finished test can hand it back.** The bookkeeping half of
/// DECISIONS §16 object revocation, applied to the services the test boot builds: without it a boot
/// that runs many service-shaped tests runs out of frames, and does so in whichever innocent test
/// happens to allocate next. See the module note and notes/frames.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the tests are the callers; the tour never tears down
pub mod holding;

/// **The RedoxFS filesystem service** (milestone 32 phase 2): three confined processes and the
/// endpoints and shared pages that wire them, spawned by the test that proves the stack end to end.
///
/// ```text
///   disk ──virtio──► block server ──blk IPC──► FS server ──file IPC──► client ──► report to kernel
/// ```
///
/// The kernel builds the wiring and hands each process exactly its world (a `Spawn` literal each);
/// it never sees a filesystem operation, an opcode, or a byte of file data. The FS server owns
/// RedoxFS and its own heap; the block server owns the DMA confinement; the client holds only a
/// directory capability. This is the same shape as `virtio_service` and the console, one level up.
///
/// The service drives the **second** mmio block disk (the RedoxFS image); the first is the nifefs
/// disk the phase-1 driver tests use. `None` if there is no such disk attached to this run.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // spawned only by the phase-2 test
pub mod fs_service;

/// **The block-device roster and the disk surveyor** (milestone 57, notes/block-devices.md).
///
/// Two authorities that every other operating system hands out as one: a **read-only mapping**
/// listing what block devices exist, and a **block-service endpoint** for exactly one of them. A
/// program with the first can see the machine's disks and open none of them; a program with the
/// second was handed one disk and has no way to name a second.
///
/// The kernel's part is small and stops early: scan the buses, write the page, confine one device
/// under a block server, spawn. It never reads a partition table. Every byte of GPT judgement is in
/// `crates/globally_unique_identifier_partition_table`, whose tests run on the host against tables
/// `sgdisk` and macOS `diskutil` wrote.
///
/// Arch-neutral, like the clock and entropy wirings: one portable binary over one host-tested
/// contract, so **both ISAs run literally the same test** (DECISIONS §19).
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tests are its only caller today
pub mod disk_service;

/// **Play an application printing to a display terminal**: put `text` in its output page and
/// `OPERATION_WRITE` it.
///
/// Shared by both of the terminal's wirings (the whole scanout, and a compositor window) because the
/// terminal contract does not know which one it is in: an `OPERATION_WRITE` is an `OPERATION_WRITE`. Returns when
/// the reply arrives, which the contract says means the bytes are on the console's side, so a test
/// needs no polling and no sleep between writes.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the milestone-29 tests are the callers
fn term_print(out: u64, ep: crate::sched::RendezvousId, text: &[u8]) {
    assert!(
        text.len() <= FRAME_SIZE as usize,
        "an OPERATION_WRITE past its output page",
    );
    let base = mmu::phys_to_virt(out);
    for (i, &b) in text.iter().enumerate() {
        // SAFETY: inside the output frame this kernel allocated and shares with the terminal.
        unsafe { core::ptr::write_volatile((base + i as u64) as *mut u8, b) };
    }
    // The bytes must be visible to the terminal before the request that names them.
    //
    // PAIR: no acquire fence, and none is needed. The terminal is blocked in `receive_cap` and the
    // `ipc_call` below is what wakes it, so the kernel's release of the `IPC_TABLES` lock and the
    // terminal's acquire of it are the pair (`spin::Mutex` locks `Acquire` and unlocks `Release`).
    // Redundant, kept: it is one `dmb` on a path that prints a line, and the contract does not
    // forbid a terminal that polls its page instead of blocking. See notes/memory-ordering.md.
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    let w0 = line_editor::proto::req(line_editor::proto::OPERATION_WRITE, text.len() as u64);
    let r = crate::sched::ipc_call(ep, [w0, 0]);
    assert_eq!(
        r[0],
        text.len() as u64,
        "the terminal consumed {} of {} bytes",
        r[0],
        text.len(),
    );
}

/// **The display service** (milestone 29, the display ladder's rung one): a confined virtio-gpu
/// driver and a client that draws, wired by the kernel and then left alone.
///
/// ```text
///   virtio-gpu ──virtio (PCIe, behind the IOMMU)──► gpu_driver ──display IPC──► painter
///        │                                              │                             │
///        └──── DMA: the whole region ───────────────────►│                             │
///                                    the surface (pages 1..) ─────── shared ──────────┘
/// ```
///
/// The kernel's part is the same as every other service here: build the wiring, hand each process a
/// `Spawn` literal, and know nothing about what they do. It never sees a virtio-gpu command, a
/// pixel, or a rectangle. What is new is the **size** of the DMA region, and that is the whole
/// memory story: a framebuffer does not fit in the single page the disk and NIC drivers get, so the
/// region is `1 + graphics_protocol::SURFACE_PAGE_FRAMES` **contiguous** frames, page 0 for the rings and the
/// control buffers and the rest for the surface. Registering the whole run as the driver's DMA region
/// is what keeps the framebuffer inside the grant: the shadow-ring validator bounds every descriptor
/// to it, and `iommu::confine` maps exactly it, so the device can reach the pixels and nothing else.
/// The block server already took two pages this way (milestone 32); this is the same move, wider.
///
/// The client maps only the surface frames. It never sees page 0, so it cannot touch a descriptor
/// ring, and it holds no `Virtio` capability, no interrupt, and no physical address. See
/// notes/framebuffer-contract.md.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// spawned only by the milestone-29 test
pub mod display_service;

/// **The compositor: one screen, several mutually distrusting clients** (milestone 33, the display
/// ladder's rung two).
///
/// ```text
///   display (or a kernel stand-in) ──gfx FLUSH(damage)──► compositor ◄──one doorbell──── window clients
///                                            the scanout, shared ──┘  │                 (a surface each)
///                                                                     └─► one input endpoint per focusable
/// ```
///
/// The kernel's part is what it always is: allocate the frames, mint the endpoints, hand each process
/// a `Spawn` literal, and know nothing about what they do. It never sees a pixel, a window, or a
/// damage rectangle. What is worth reading here is the **shape of the grants**, because the isolation
/// this rung exists to prove is a property of exactly that shape:
///
/// - every client's control page and surface are its own frames, mapped **at the same virtual
///   addresses** in every client. Two clients' surfaces are the same address in different address
///   spaces, so "my neighbour's surface" is not somewhere a client can reach by guessing;
/// - the clients' frames are allocated as **one contiguous run**, deliberately, so that the page just
///   past a client's grant really is its neighbour's memory. That makes the attack in
///   `a_client_holds_no_capability_for_its_neighbours_pixels_or_the_screen` a fair one: the attacker is
///   handed the exact address, the bytes it wants are physically adjacent, and the mapping is the only
///   thing in its way;
/// - the screen and the window list are mapped **read-only** and **only** into a client granted them.
///   That mapping is the screenshot capability and the enumeration capability; there is no verb for
///   either, and a client without the mapping has nothing to ask and nowhere to look.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// spawned only by the milestone-33 tests
pub mod compositor_service;

/// **The keyboard service** (milestone 29's input): a confined userspace virtio-input driver that
/// turns key events into the bytes a terminal understands, and publishes them where the compositor
/// reads them.
///
/// ```text
///   virtio-input ──virtio (PCIe, IOMMU)──► keyboard_driver ──the input ring──► whoever maps it
///                                           └──doorbell COMMIT──► "look at the surfaces"
/// ```
///
/// The grant shape is the whole security argument and it is worth reading beside
/// `compositor_service`: the driver gets the device, its interrupt, its DMA page, the doorbell, and
/// **the ring page**. It does not get any client's endpoint, so it cannot choose who receives what
/// it types; that is focus, and focus is the compositor's decision expressed as which of the input
/// capabilities *it* holds it uses (DECISIONS §33). And the ring is what makes typing possible at
/// all: the doorbell every client holds is content-free, so a client that rang it forever could not
/// produce a single character.
///
/// In the test below the **kernel** plays the compositor, which is the same substitution three of the
/// four rung-two tests make: it holds the doorbell and the ring, so the bytes a real keyboard
/// produced are a value it can read and compare rather than a picture it has to infer.
// The driver is spawned only by the milestone-29 test; the boot calls `wire_device` alone and the
// progenitor builds the driver (milestone 600 (provisional)).
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub mod keyboard_service;

/// **The USB keyboard driver's wiring** (milestone 242 (USB host and HID)): the whole xHCI
/// controller, confined, handed to one EL0 process that turns a boot keyboard's reports into the
/// terminal contract's bytes. What it holds and what it is refused is written in that module's own
/// header. Spawned by [`boot_progenitor`] alone, so it is dead in exactly the builds that function is.
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput"
    ),
    allow(dead_code)
)]
pub mod usb_keyboard_service;

/// **The clock service** (milestone 51 lane A, DECISIONS §43): the RTC's registers, the wall
/// clock's offset, and the propose endpoint, in one confined userspace process.
///
/// The kernel's whole part in wall-clock time is here and it is small: find the RTC in the device
/// tree (by `compatible`, `memory::rtc_region`), allocate one frame for the clock page, and hand
/// the service the registers, the page read/write, and an endpoint. It does not read the clock, does
/// not know what time it is, and has no notion of an offset. Everything after the spawn is
/// userspace agreeing with userspace over `clock_protocol`.
///
/// Arch-neutral, like the display and compositor wiring: the component is one portable binary
/// carrying both RTC drivers, and the *machine* says which one it has, so **both ISAs run literally
/// the same test** (DECISIONS §19).
// The interactive boot calls `start` on both ISAs since milestone 51's wiring lane; the rest of the
// module (the propose helper, the kernel-side page reader) is still the tests' alone.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub mod clock_service;

/// **The clock page the interactive boot hands the progenitor**, and the one place both ISAs agree on what a
/// machine with no clock looks like (milestone 51's wiring; `boot_progenitor`).
///
/// The grant is **unconditional**, and that is the design rather than an oversight. A zeroed page
/// reads as `clock_protocol::state::UNKNOWN` (`a_zeroed_page_reads_as_unknown`), so a boot with no
/// `clock` program in its initrd hands the progenitor a page that honestly says "the machine has no clock it
/// believes" instead of no page at all. That keeps the slot numbering the same on every boot, which
/// matters more than it sounds: the progenitor's capability table is read positionally, and a capability whose *slot*
/// depends on what the machine turned out to have is a wiring nobody can check by reading.
///
/// It is also the DECISIONS §43 split, delivered: the progenitor gets `READ` on a frame. Nothing on this path
/// can hand a child the writable mapping that would let it set the time, because the progenitor never had one.
fn boot_clock_page() -> u64 {
    match program("clock") {
        Some(image) => {
            let wiring = clock_service::start(image);
            // The service publishes the RTC reading and *then* announces, with a blocking send. It
            // does not need to be drained for the page to be right, but an undrained announcement
            // parks the service inside it forever, so it would never serve a proposal. One thread
            // whose whole life is that receive costs nothing and leaves the propose endpoint live.
            let report = wiring.report;
            let _ = crate::sched::spawn(move || {
                crate::sched::ipc_receive(report);
                crate::sched::exit();
            });
            wiring.page_phys
        }
        // No `clock` program packed: allocate the page anyway and leave it zeroed. This is the
        // honest unknown clock, and it is the same state `date`'s test allocates deliberately.
        None => crate::memory::alloc_zeroed()
            .expect("no frame for the clock page")
            .addr(),
    }
}

/// The confined transport, the completion interrupt, and the DMA page's physical base, for a
/// virtio device (the rng, or since milestone 590 (provisional) the NIC) this kernel discovered and
/// wired at boot. Returned to the caller rather than
/// stored, because `crate::sched::grant`'s next-free-slot placement means the caller decides
/// exactly where these land relative to whatever else it has already granted.
struct VirtioBootGrant {
    /// The `Virtio` capability's id (`crate::virtio::register`'s return value).
    vid: usize,
    /// The device's completion interrupt. **Routed** (`crate::sched::bind_irq`) but not yet
    /// **enabled**: the two boards enable an interrupt source differently (aarch64's GIC inline,
    /// riscv64's PLIC pinned to the boot hart's own context via `boot_s_context`, not the
    /// hart-spreading `crate::arch::irq::enable` uses -- `notes/harts-and-pes.md`'s hart lottery
    /// is why, the same reasoning `uart_irq`'s identical two-step split in `boot_progenitor`
    /// already follows), so the caller does that part itself, the same place it already enables
    /// `uart_irq`.
    intid: u32,
    /// The DMA region's physical base. `entropy.rs` (and `net_stack`) needs this as a plain value
    /// (it builds virtio
    /// ring descriptors, which are physical-address-based by the spec, not a fact any capability
    /// exposes), and there is no fourth `START` argument word to carry it across the kernel/progenitor
    /// boundary (`start_thread_control_block`'s own `[u64; 3]`, already spent on
    /// `role`/`initrd_len`/`fs_rights`). So it travels the way the page's *contents* already do: written
    /// into the page itself at [`VIRTIO_DMA_PHYS_OFFSET`], which the progenitor reads back out once,
    /// after mapping the granted frame briefly, and relays to entropy's own `arg1` exactly the way
    /// it already relays `fs_rights`.
    dma: u64,
}

/// Where [`boot_virtio_mmio_device`] (and, since milestone 600 (provisional), the gpu and keyboard
/// wiring in [`display_service`] and [`keyboard_service`]) writes a DMA region's own physical base,
/// inside that same region. Safe for every device granted this way for one reason: no driver's
/// layout reaches the first page's tail. Entropy's ring
/// (`components/src/entropy.rs`'s `Q_DESC`/`Q_AVAIL`/`Q_USED`) and its one pool buffer (`POOL_OFF`
/// 0x400, `POOL_LEN` 256 bytes) end at byte 0x500; `net_stack`'s two rings and four frame buffers
/// (`components/src/virtio_net_transport.rs`'s `BUF_BASE` 0x400 plus four `BUF`s of 0x2C0) end at
/// 0xF00. This sits in the last eight bytes, past all of them, so a future widening of any has room
/// to move without colliding. The value is `abi::virtio::DMA_PHYS_OFFSET`, which every reader
/// shares.
pub(crate) const VIRTIO_DMA_PHYS_OFFSET: u64 = abi::virtio::DMA_PHYS_OFFSET;
const _: () = assert!(VIRTIO_DMA_PHYS_OFFSET + 8 <= FRAME_SIZE);

/// **Write a DMA region's own physical base into its first page**, at [`VIRTIO_DMA_PHYS_OFFSET`].
/// The one place every boot virtio device's region learns where it is; see that constant.
pub(crate) fn write_dma_phys(dma: u64) {
    // SAFETY: `dma` is a fresh frame the caller just allocated, direct-mapped and owned by nobody
    // else yet, and `VIRTIO_DMA_PHYS_OFFSET + 8` is inside `FRAME_SIZE` (the assertion above), so
    // the write stays in the frame.
    unsafe {
        core::ptr::write_unaligned(
            (mmu::phys_to_virt(dma) as *mut u8)
                .add(VIRTIO_DMA_PHYS_OFFSET as usize)
                .cast::<u64>(),
            dma,
        );
    }
}

/// **Discover and wire a virtio-rng device on the MMIO bus, for the interactive boot's own use**
/// (DECISIONS §120's 2026-08-26 amendment: "grant the QEMU-only virtio-rng stopgap"). `None` on a
/// boot with no such device: real hardware (milestone 55's actual target has no virtio-rng at all,
/// §120's own text), or a run with `NIFE_RNG` unset. The whole chain past this point treats that
/// exactly as "this boot has no filesystem" is already treated by [`boot_progenitor`]: an absence
/// the caller can act on, not a failure.
///
/// **Only the MMIO transport**, unlike `entropy_service::start`'s own test-harness wiring, which
/// also offers PCIe: a first cut scoped to what an interactive boot actually needs, on the same
/// "a minimal device surface for the boot a person actually meets" posture already named for the
/// GPU/keyboard/NVMe flags (`components/src/login.rs`'s own BUGS, before this amendment). Widening to
/// PCIe (behind the IOMMU) is real follow-on, not invented here.
///
/// Mirrors `kernel::user::entropy_service::start`'s own kernel-side setup (device discovery, a
/// zeroed DMA frame, the interrupt route, `crate::virtio::register`) up to the point that function
/// spawns the service itself: this one hands the three capabilities back for the **caller** to
/// grant and delegate, because on the interactive boot the caller is the progenitor, not the kernel, and the progenitor
/// is the one that builds the entropy service: `crates/system_initializer`'s own ELF loader, the
/// tree's only one (milestone 96), and that crate's own header says why a second loader would be
/// the wrong shape.
fn boot_virtio_rng_device() -> Option<VirtioBootGrant> {
    boot_virtio_mmio_device(crate::virtio::find_entropy_device()?)
}

/// **An entropy service on the CPU's own seed instruction, for a boot with no virtio-rng**
/// (milestone 595 (provisional); promoted from the proposal
/// `the-x86-64-progenitor-serves-entropy-from-rdseed`). The request endpoint, which
/// [`boot_progenitor`] grants at slot 16, or `None`, and a `None` is said on the console.
///
/// **The kernel confirms the instruction exists, not the progenitor**, which is the question that
/// proposal left open. Three things decided it, and none of them is effort:
///
/// - It is what the tree already does. `entropy_service::is_instruction_backend_available` reads
///   the feature bit from `arch::isa`'s boot record, and `components/src/entropy.rs` says outright
///   that it trusts its spawner's choice of mode. The installer (`install_service`) takes the same
///   service the same way on the booted `x86_64` path.
/// - It is the only answer that works on aarch64 too. `ID_AA64ISAR0_EL1` is not readable at EL0,
///   so a progenitor that ran `CPUID` itself would be an `x86_64` special case with no aarch64 twin
///   (DECISIONS §19). Here the function is arch-neutral: aarch64 with `FEAT_RNG` and no
///   `NIFE_RNG` takes this path as well, and riscv64 answers `None` from the same predicate.
/// - Detection stays in `kernel/src/arch/` (AGENTS.md's rule 1), and the progenitor stays free of
///   `cfg(target_arch)`.
///
/// **The kernel builds the service rather than telling the progenitor to**, because a fact with
/// no capability has no slot to travel in: the progenitor's three `START` words are spent
/// (`system_initializer::BootEndowment::virtio_rng`'s doc), and a flag in a page
/// would be a second format for one bit. A built service is a capability, and a probe on its slot
/// is how the progenitor already tells every optional grant from an absent one. `fs_ep` is the
/// precedent: the kernel wires the file service before the progenitor exists and grants its
/// endpoint.
///
/// **A refusal, not weaker bytes.** No instruction, or a first draw of all zeros
/// (`entropy_protocol::readiness`), and this returns `None` with a sentence saying so. There is no
/// fallback to `RDRAND`/`RNDR` (DRBG output, which `entropy.rs`'s header refuses) and no software
/// generator. The bytes served are the instruction's own, unmixed and not health-tested after the
/// first draw: option A of DECISIONS §137 (a hardware TRNG with no published health-test claim),
/// which is what every other backend here does. Choosing B or C is calef's.
///
/// **`ready` may already be taken.** `entropy_service::ensure` hands the readiness report to
/// whoever wired the service first, and on `x86_64` that can be the installer earlier in this boot.
/// The installer does not read the verdict either, so a condemned service would answer
/// `NO_ENTROPY` to every request rather than serve anything; the progenitor's own first draw (the
/// login password) is then what fails, and it builds no login stack.
fn boot_instruction_entropy() -> Option<crate::sched::RendezvousId> {
    let image = program("entropy")?;
    let Some(w) = entropy_service::ensure(image, entropy_service::Bus::Instruction) else {
        crate::println!(
            "  entropy     : NONE. No virtio-rng device, and this CPU has no seed instruction \
             (RDSEED, RNDRRS), so nothing at the prompt can draw random bytes and there is no login."
        );
        return None;
    };
    if let Some(report) = w.wait_for_ready()
        && report[0] != entropy_protocol::READY
    {
        crate::println!(
            "  entropy     : REFUSED. The seed instruction's first draw was all zeros ({:#x}), so \
             the service is condemned for this boot; nothing at the prompt can draw random bytes.",
            report[0]
        );
        return None;
    }
    crate::println!("  entropy     : the CPU's seed instruction; the progenitor serves it");
    Some(w.request)
}

/// **The same for the network card** (milestone 590 (provisional), the booted system starts its
/// network stack; promoted from the proposal `the-booted-system-has-no-network`). `None` on a boot
/// with no virtio-net device on the MMIO bus: every real board today, and every QEMU run with
/// `NIFE_NET` unset. The progenitor builds `net_stack` from these three and nothing else, exactly
/// as it builds entropy from the rng's three.
///
/// **The MMIO NIC, not the PCIe one**, for [`boot_virtio_rng_device`]'s reason, and with a cost
/// that one does not carry: the MMIO NIC has no IOMMU in front of it, so the confinement of this
/// device's DMA is the transport's shadow-ring validator alone, not the SMMU of DECISIONS §20
/// (IOMMU-backed DMA isolation: one seam, two arch drivers). The runners attach both NICs under
/// `NIFE_NET`; the PCIe one sits unclaimed on this boot. See
/// milestone 590's block for why that is recorded rather than fixed here.
fn boot_virtio_net_device() -> Option<VirtioBootGrant> {
    boot_virtio_mmio_device(crate::virtio::find_net_device()?)
}

/// **A network stack over an `e1000e`, when this boot has one and no virtio-net** (milestone 198
/// (a package manager), whose rung 3a fetch had run on aarch64 and riscv64 only; milestone 494 (a
/// driver for the network card a PC actually has) built the driver and named this as its first
/// follow-on). `None`, having said why, for a part no gate has driven, a link that was down, or a
/// bring-up the driver refused; `None` silently with no such NIC, which is every `virt` boot.
///
/// **The kernel builds this one, not the progenitor**, for [`boot_usb_keyboard`]'s reason: the
/// server is handed BAR0's two queue pages and an eighteen-page DMA region as spawn-time mappings,
/// which it holds no name for and so can neither delegate nor revoke
/// (`e1000e_service`'s header). Built by the progenitor, each page would be a capability in its
/// table. It is also the wiring every `e1000e` gate already runs, so the booted system's stack is
/// the tested one rather than a second copy of it.
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput"
    ),
    allow(dead_code)
)]
fn boot_e1000e_network() -> Option<e1000e_service::Wiring> {
    use e1000e_service::{Absent, NotAtBoot};
    let image = program("net_stack")?;
    let name = |device| ::e1000e::model(device).unwrap_or("an unnamed part");
    match e1000e_service::start_for_boot(image) {
        Ok(w) => {
            crate::println!(
                "  network     : an e1000e ({}), link up; the kernel started net_stack on it, \
                 confined {}",
                name(w.device),
                if w.confined_by_iommu {
                    "by the IOMMU"
                } else {
                    "by its own arithmetic alone (no IOMMU owns it)"
                },
            );
            Some(w)
        }
        Err(NotAtBoot::Absent(Absent::NoController)) => None,
        Err(NotAtBoot::Unproven { device }) => {
            crate::println!(
                "  network     : NONE. An e1000e ({}, 8086:{device:04x}) is on the bus and left \
                 alone: no gate has driven that part yet (milestone 494's bench boot is how one \
                 does)",
                name(device),
            );
            None
        }
        Err(NotAtBoot::NoLink { device }) => {
            crate::println!(
                "  network     : NONE. The e1000e ({}) has no link, and a stack waiting for DHCP \
                 would hold the prompt back for ever",
                name(device),
            );
            None
        }
        Err(NotAtBoot::Absent(why)) => {
            crate::println!("  network     : REFUSED. The e1000e could not be brought up: {why:?}");
            None
        }
    }
}

/// **The booted system's stack over radon's Ethernet port** (milestone 53 (the board's own
/// peripherals: network and storage on real silicon)), [`boot_e1000e_network`]'s twin. Returns the
/// `Stack` endpoint and the lease endpoint, or `None` having said why. Until
/// `designware_ethernet_service::PROVEN_ON_SILICON` is true it touches nothing: radon's boot is the
/// one every other bench session depends on, and a bring-up that hung would take the prompt with it.
#[cfg(target_arch = "riscv64")]
#[cfg_attr(
    any(
        test,
        feature = "bench",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput"
    ),
    allow(dead_code)
)]
fn boot_designware_network() -> Option<(crate::sched::RendezvousId, crate::sched::RendezvousId)> {
    use designware_ethernet_service::NotStarted;
    let port = crate::memory::jh7110_ethernet()?;
    if !designware_ethernet_service::PROVEN_ON_SILICON {
        crate::println!(
            "  network     : NONE. The JH7110's Ethernet port at {:#x} is described and left as \
             the firmware left it: no bench boot has proved this driver yet (milestone 53's \
             runbook, notes/designware-ethernet.md, is how one does)",
            port.port.base,
        );
        return None;
    }
    let image = program("net_stack")?;
    match designware_ethernet_service::start_net_server(image, socket_protocol::NO_LISTEN_GRANT) {
        Ok(w) => {
            crate::println!(
                "  network     : the JH7110's Ethernet port, link up; the kernel started net_stack \
                 on it, confined by its own arithmetic alone (no IOMMU on this SoC)"
            );
            Some((w.stack, w.report))
        }
        Err(NotStarted::NoLink) => {
            crate::println!(
                "  network     : NONE. The JH7110's Ethernet port has no link, and a stack \
                 waiting for DHCP would hold the prompt back for ever"
            );
            None
        }
        Err(why) => {
            crate::println!("  network     : REFUSED. The JH7110's Ethernet port: {why:?}");
            None
        }
    }
}

/// The shared body of [`boot_virtio_rng_device`] and [`boot_virtio_net_device`]: a zeroed DMA frame
/// with its own physical base written at [`VIRTIO_DMA_PHYS_OFFSET`], the interrupt routed but not
/// enabled, and the transport registered with the kernel, confined to that one frame.
fn boot_virtio_mmio_device(d: crate::virtio::VirtioMmioDevice) -> Option<VirtioBootGrant> {
    // Zeroed first, so no stale descriptor or buffer content is visible to the device or to the
    // driver's own first read.
    let dma = crate::memory::alloc_contiguous_zeroed(1)
        .expect("no DMA frame for a boot virtio device")
        .addr();
    // The physical base is written into the tail of the same page, at an offset neither driver's
    // ring-and-buffer layout reaches (see [`VIRTIO_DMA_PHYS_OFFSET`]'s own doc).
    write_dma_phys(dma);
    // Routed, not yet enabled; see [`VirtioBootGrant::intid`]'s own doc for why enabling is the
    // caller's job.
    crate::sched::bind_irq(d.intid, crate::sched::create_rendezvous());
    let vid = crate::virtio::register(
        crate::virtio::Transport::Mmio {
            mmio_phys: d.mmio_phys,
        },
        dma,
        FRAME_SIZE,
        None,
    );
    Some(VirtioBootGrant {
        vid,
        intid: d.intid,
        dma,
    })
}

/// **The inert-configuration page the interactive boot hands the progenitor** (milestone 47's
/// environment-variable fork, DECISIONS §111; `boot_progenitor`).
/// [`boot_clock_page`]'s twin, minus the service: nothing here runs, so there is nothing to spawn
/// and nothing to wait for a report from. The page is assembled once, into a frame nothing else
/// can see, and only then handed to the progenitor; see `environment_protocol`'s own docs for why that
/// ordering needs no seqlock.
///
/// The grant is **unconditional**, [`boot_clock_page`]'s own reason: a fixed slot on every boot,
/// whether or not anything downstream ever declares wanting the page, is what lets the progenitor's
/// capability table stay positional. The values are the conservative universal defaults this
/// tree's kernel test harness for `std` programs already uses
/// (`system_tests/src/user/std_service.rs`): "nothing configured this program's locale or terminal, so
/// tell it the least assuming thing" is the honest baseline, the same posture `boot_clock_page`
/// takes for a machine with no RTC. There is no shell-held default config set yet to pass instead
/// (the "inheritance with visibility" shape design/roadmap/0047-navigation-and-naming.md names);
/// this is the fixed default until one exists.
fn boot_config_page() -> u64 {
    let bytes = environment_protocol::PageBuilder::new()
        .tz("UTC")
        .expect("UTC is not a recognized environment_protocol::domain::KNOWN_TZ member")
        .lang("C")
        .expect("C is not a recognized environment_protocol::domain::KNOWN_LANG member")
        .term("dumb")
        .expect("dumb is not a recognized environment_protocol::domain::KNOWN_TERM member")
        .build();
    // Zeroed before the assembled bytes are written, so nothing left behind by a previous
    // occupant of this physical page is visible through the reserved tail past `PAGE_BYTES`
    // (`ConfigPage` only ever reads the first `PAGE_BYTES`, but a frame's contents are otherwise
    // unspecified until written; the same shape `std_service::start_on` uses).
    let phys = crate::memory::alloc_zeroed()
        .expect("no frame for the config page")
        .addr();
    // SAFETY: `phys` names that frame, direct-mapped and owned by nobody else yet, and `bytes` is
    // `PAGE_BYTES` long, far under `FRAME_SIZE`, so the copy does not run past the frame.
    unsafe {
        core::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            mmu::phys_to_virt(phys) as *mut u8,
            bytes.len(),
        );
    }
    phys
}

/// **The shell's terminal on the screen the firmware left running** (the shell on the firmware
/// screen, milestone 198's rung 1b; `design/roadmap/` has its block).
///
/// Milestone 243 (a machine with no serial port) put the *kernel's* boot tour on a UEFI machine's
/// framebuffer. Since milestone 299 (the serial console becomes a userspace driver) the console is
/// a userspace process that writes COM1, so on a PC with no serial port the tour scrolled past and
/// the prompt appeared nowhere. This puts `display_terminal` on that same screen, served by
/// `framebuffer_driver`, and returns what the progenitor needs to hand the console server so that
/// it writes every byte to the screen as well as to the UART.
///
/// **The order is the handover, and it is the point of the function.** The programs are found
/// first, so a build that lacks one leaves the kernel painting rather than a blank screen. Then
/// [`crate::console::yield_screen`] clears the screen and stops the kernel's `print!` from painting
/// it, under the console lock, and only then is the driver spawned. So there is no moment with two
/// painters, and after this the kernel's own lines (the progenitor's exit, a user fault report) go
/// to the UART alone, including the two this function prints: a line printed on the screen just
/// before the yield would be cleared by it before anybody could read it.
///
/// If the wiring refuses after the yield (a screen too large to map, [`display_service`]'s
/// `MAX_APERTURE_PAGES`), the screen is left blank rather than handed back: the boot goes on over
/// the UART exactly as a machine with no screen does. Recorded here rather than papered over with a
/// second handover path, because the refusal is a bound no screen in the fleet is near.
///
/// Readiness is drained here, the idiom the kernel-built virtio-gpu stack used before milestone 600
/// (provisional) moved it into the progenitor: when this returns, the driver and the terminal are
/// running and the terminal has painted its blank grid.
///
/// Arch-neutral, and `None` on aarch64 and riscv64 today only because nothing there tells the
/// console about a screen: milestone 157 (real display output on the board), the U-Boot
/// `simple-framebuffer` discovery, is what would, and then this function needs no change. **Name
/// provisional.**
fn boot_screen_terminal() -> Option<display_service::TerminalWiring> {
    let driver = program("framebuffer_driver")?;
    let terminal = program("display_terminal")?;
    // **A screen this kernel owns the memory of is not handed away** (milestone 243).
    //
    // The handover's whole shape assumes a UEFI aperture: a BAR on a display adapter, memory no
    // part of this kernel is otherwise in, whose physical range `display_service` maps into a
    // userspace driver. `ramfb` broke that assumption on the two `virt` boards, where the
    // framebuffer is `kernel/src/screen.rs`'s own `.bss` and mapping its range into a driver would
    // hand a userspace process a window onto kernel statics.
    //
    // Checked before the yield rather than after, so a refusal leaves the kernel still painting
    // rather than leaving a screen cleared and unclaimed. It is a range test rather than a flag, so
    // milestone 157's U-Boot aperture (outside the kernel image, like the UEFI one) passes without
    // anybody having to remember to set anything.
    let screen = crate::console::peek_screen()?;
    if crate::screen::is_kernel_memory(&screen) {
        crate::println!(
            "  screen    : kept by the kernel; its framebuffer is kernel memory, not an aperture"
        );
        return None;
    }
    let screen = crate::console::yield_screen()?;
    crate::println!(
        "  screen    : handed to a userspace terminal; the kernel writes the UART alone"
    );
    let w = display_service::start_screen_terminal(driver, terminal, screen)?;
    let [tag, geometry, ..] = crate::sched::ipc_receive(w.driver_report);
    assert_eq!(
        tag,
        graphics_protocol::status::UP,
        "the framebuffer driver did not come up ({tag:#x})",
    );
    let [tag, cells, ..] = crate::sched::ipc_receive(w.term_report);
    assert_eq!(
        tag,
        video_terminal::status::TERM_UP,
        "the display terminal did not come up ({tag:#x})",
    );
    // The driver reports its surface in surface pixels; at the screen's scale that is the whole
    // screen (`display_service::start_screen_terminal`), which is what this line says.
    let scale = screen_console::ScreenConsole::scale_for(screen.width) as u64;
    crate::println!(
        "  screen    : {}x{} pixels of it at scale {scale} served by framebuffer_driver, a {}x{} \
         terminal on it",
        (geometry & 0xffff_ffff) * scale,
        (geometry >> 32) * scale,
        cells & 0xffff_ffff,
        cells >> 32,
    );
    Some(w)
}

/// **The entropy service** (milestone 56, DECISIONS §44): a virtio-rng device, its DMA page, its
/// interrupt, and the request endpoint clients hold, in one confined userspace process.
///
/// The kernel's whole part in randomness is here and it is smaller than the clock's: find an RNG on
/// whichever bus the caller named, confine it to one DMA page, and hand the service the transport,
/// the interrupt, and two endpoints. **The kernel never reads the device and holds no entropy of
/// its own.** Everything after the spawn is userspace agreeing with userspace over
/// `entropy_protocol`.
///
/// The authority split is the point, and it is one sentence: the service holds the device; a client
/// holds an endpoint that means *"you may obtain randomness"*. Those are different powers, and only
/// the second one is safe to hand around. A client cannot program the queue, cannot map the page
/// the device writes into, and cannot ask for anything the service did not ask on its behalf.
///
/// Arch-neutral: one portable binary, both transports, both ISAs (DECISIONS §19).
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the system tests, std_service, the installer and boot_progenitor are its callers
pub mod entropy_service;

/// **radon's SD/MMC block server's wiring** (milestone 53 (the board's own peripherals: network
/// and storage on real silicon)): the kernel ungates the controller and hands a process one page of
/// it, a transfer region and a window of the card. riscv64-only because the controller is the
/// JH7110's; `kernel/src/designware_mobile_storage.rs` carries the parity note. The bench boot is
/// its only caller until `PROVEN_ON_SILICON`.
#[cfg(target_arch = "riscv64")]
#[cfg_attr(not(feature = "storage_bench"), allow(dead_code))]
pub mod designware_mobile_storage_service;

/// **The EL0 NVMe block server's wiring** (milestone 261; DECISIONS §86's option 2a).
///
/// The kernel keeps the admin plane, which is the authority to say where a queue lives, and hands
/// a process the doorbell page and the data plane's pages of one confined DMA region. What it is
/// granted and what it is refused is written out in that module's own header, in the shape
/// milestone 159's TRNG driver established, because the confinement is the claim and the driver is
/// only what exercises it.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tests are its callers
pub mod non_volatile_memory_express_service;

/// **`net_stack` over the `e1000e` NIC** (milestone 494 (a driver for the network card a PC
/// actually has)): the kernel resets the controller and programs its rings, and the process is
/// handed the two queue pages of BAR0 and the confined DMA region, in milestone 261 (the NVMe driver leaves the kernel)'s shape. What
/// it holds and what it is refused is in that module's header.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the tests, the bench boot and boot_progenitor are its callers
pub mod e1000e_service;

/// **`net_stack` over the JH7110's Ethernet port** (milestone 53 (the board's own peripherals:
/// network and storage on real silicon)): `e1000e_service`'s shape for radon. riscv64-only because
/// the JH7110 is; `kernel/src/designware_ethernet.rs` carries the parity note.
#[cfg(target_arch = "riscv64")]
pub mod designware_ethernet_service;

/// **The offer a booted stick makes** (milestone 198 (a package manager, and the trivial install
/// that makes a second customer possible), rung 2a): ask whether to put this system on the
/// machine's own disk, and wire the two confined programs that do it. Boot policy only; nothing in
/// it holds a disk.
///
/// **`x86_64` only, and the gap is the loader's rather than this module's.** It needs two things
/// the other two architectures do not have: a copy of the file this machine booted from
/// (`memory::boot_file_region`, which is `None` wherever `uefi_loader` cannot hand over a second
/// module, and a device-tree handoff has one initrd slot in `/chosen` and no second one), and a
/// console it can read a line back from (`console::read_line`, which the aarch64 console's PL011
/// driver has no receive path for). Both are recorded where they are, and both would have to move
/// before this module could. Written as a `cfg` rather than as a no-op body on purpose: a module
/// that compiled everywhere and could only ever decline on two of three would read as portable.
#[cfg(target_arch = "x86_64")]
pub mod install_service;

/// **The credential service, its provisioner, and its clients** (milestone 56, the credential half;
/// notes/credentials.md).
///
/// The kernel's part is the wiring, and here more than anywhere the wiring *is* the argument. Four
/// processes, and the difference between three of them is one field of a `Spawn` literal:
///
/// | process | slot 0 | what that means |
/// |---|---|---|
/// | `credentialer` | the provision endpoint (READ) **and** the verify endpoint (READ, slot 1) | holds the store |
/// | `credentialer_test_client` provisioner | the **provision** endpoint (WRITE) | may write the store, until the seal |
/// | `credentialer_test_client` client | the **verify** endpoint (WRITE) | may ask a question about the store |
/// | `credentialer_test_client` attacker | the **verify** endpoint (WRITE) | the identical endowment, used otherwise |
///
/// The kernel never sees a secret, holds no store, and computes no hash. It creates two endpoints,
/// two frames, and a budget, and hands each process a different subset. Everything after the spawn
/// is userspace agreeing with userspace over `credential_protocol`.
///
/// **Two frames and not one**, which is the detail worth stating: the provisioner writes plaintext
/// secrets into its page, so a client sharing that frame would read them. The two pages are
/// separate physical frames and neither process is ever given the other's.
///
/// Arch-neutral: one portable binary each, both ISAs (DECISIONS §19). Argon2id is arithmetic on
/// `u64`s and neither the service nor its clients contain a line of assembly.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the milestone-56 credential tests are its callers
pub mod credential_service;

/// **The login service: authentication produces capabilities, not a mutated identity** (milestone
/// 49, DECISIONS §109). The kernel spawns it exactly as it spawns `credentialer`: the archive
/// mapped read-only, a construction budget, and the endpoints it needs, so what is under test is
/// `components/src/login.rs`'s own choices rather than a privileged shortcut.
///
/// Not arch-gated, for `credential_service`'s own reason: `nifefs`, `elf`, and
/// `supervision_protocol::build_child` are portable, and a login service that mints capabilities on
/// one instruction set and not another is not the claim this milestone makes.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the milestone-49 login tests are its callers
pub mod login_service;

/// **A provisioning tool: create an identity and its home subtree together** (milestone 155,
/// DECISIONS §117). Spawned once per identity, against a credential service's still-open provision
/// endpoint and a directory capability wide enough to hold the new subtree, exactly as
/// `credentialer_test_client`'s provisioner role is spawned, except this is the first real caller
/// rather than a test harness.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the milestone-155 provisioning tests are its callers
pub mod identity_provisioner_service;

/// **The NTP client, and the test server that answers it** (milestone 51; DECISIONS §43, §44).
///
/// The kernel's part is the wiring, and the wiring *is* the argument. An NTP client here gets five
/// slots: a report endpoint, the socket contract's endpoint, an untyped budget, the clock service's
/// **propose** endpoint, and the entropy service's endpoint. What it does not get is a mapping of
/// the clock page, in either direction, which is the whole difference between this and a Unix
/// `ntpd` running as root.
///
/// The test server is a separate program holding `READ` on the endpoint the client holds `WRITE`
/// on. Substituting the peer at a capability boundary is how a capability system tests a client:
/// the client's code does not change and cannot tell. See
/// components/src/network_time_client.rs for what that proves and what it leaves to milestone 30's
/// socket-contract tests. It was a role of the client's own binary until milestone 290, and nothing
/// about the substitution depended on that: the boundary is the capability.
///
/// Arch-neutral: three portable binaries, both ISAs (DECISIONS §19).
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tests are its callers
pub mod ntp_service;

/// Milestone 11: hand a process an untyped budget and let it spend it.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the tour and the userspace tests
pub mod memory_region_service;

#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
/// Spin the scheduler until `done()`, or give up after a wall-clock deadline. Returns whether it
/// happened. **Time-based, not a fixed yield count** (DECISIONS §28): with work spread across
/// cores, the test thread's own core is often idle, so a yield returns at once and a fixed count
/// of them elapses in almost no real time, timing out before a parallel result on another core
/// lands. A ~2 s deadline gives the other cores real time to finish while staying far under the
/// 60 s hang watchdog, so a genuine hang still fails.
///
/// It lives **here** rather than in `tests` because six sibling modules use it and that one does
/// not compile on every architecture: `user::tests` needs a real ELF program out of the initrd
/// and is `#[cfg(all(test, initrd))]`, which would have taken this helper down with it on a
/// target that packs none (milestone 161, roadmap item 4). A helper every module uses does not
/// belong inside one of them. Milestone 81 needed it in two of them: running on the physical core makes the
/// yield-count version fail for the *mirror* reason it fails on a loaded host, since a yield on an
/// idle core costs nanoseconds there. See notes/hvf-leg.md.
pub fn wait_for(mut done: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if done() {
            return true;
        }
        crate::sched::yield_now();
    }
    done()
}

/// **The raw-keystroke input primitive** (milestone 169): a real `line_editor` process, wired
/// exactly as the boot path wires it except that the test plays both the input driver and the
/// application, so `OPERATION_RAWMODE` and `OPERATION_READRAW` can be driven directly with real keystrokes.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the milestone-169 raw-mode tests are its only caller
pub mod raw_mode_service;

/// **`rmle`'s wiring** (milestone 169): a real terminal ([`raw_mode_service`]'s own shape) and a
/// real filesystem ([`fs_service::narrow_dir`]'s shape) composed for the one program in this tree
/// that needs both at once.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
// the milestone-169 rmle tests are its only caller
pub mod rmle_service;
