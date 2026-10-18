//! **Loading an image**: checking an ELF against the image band, mapping its segments and the
//! timebase page, and finding a program in the initial ramdisk.
//!
//! Cut out of `user.rs` unchanged by milestone 872 (the kernel's user-mode file is split along its
//! seams), which left the parent an index.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

use super::*;

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
                counter_frequency_contract::PAGE_VA,
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
/// `load`), the frame is allocated anyway and left zeroed, which [`counter_frequency_contract::TimebasePage::hz`]
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
            let bytes = counter_frequency_contract::build_page(hz);
            // SAFETY: `dst` names a freshly allocated frame, reachable through the direct map and
            // owned by nobody else yet; `bytes` is `PAGE_BYTES` (24) bytes, far under the frame's
            // `FRAME_SIZE`, so the copy does not run past it.
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len()) };
        }
        // Not yet measured: leave the frame zeroed (retype_page-equivalent frames from
        // `memory::alloc` are not guaranteed pre-zeroed the way `Untyped::retype_page` promises,
        // so this writes zero explicitly rather than assuming it).
        //
        // SAFETY: as the `Some` arm above: `dst` names a freshly allocated, exclusively owned
        // frame, and `PAGE_BYTES` is far under `FRAME_SIZE`.
        None => unsafe { core::ptr::write_bytes(dst, 0, counter_frequency_contract::PAGE_BYTES) },
    }

    PAGE_PHYS.store(phys, Ordering::Release);
    Some(phys)
}

/// Map the timebase page into `space`, if this process needs one built directly rather
/// than through [`load`]. Several kernel-side functions build a top-level process's own
/// `AddressSpace` by hand instead of calling `load` ([`spawn_elf_child_builder`], and every
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
            counter_frequency_contract::PAGE_VA,
            phys,
            Flags::user_rodata(),
            crate::revoke::PageMapSource::NoCapability,
        )?;
    }
    Ok(())
}

/// Lay an ELF's loadable segments into `space`, honouring their permissions exactly (milestone 19d
/// factored this out of `load` so [`spawn_elf_child_builder`] shares it; the progenitor's userspace
/// loader mirrors it). A read-only segment gets `user_rodata`, not `user_data`: a loader that
/// widens permissions is a loader you cannot reason about. `.bss` is free because `map_new` zeroes
/// every page.
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
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11).
/// His words: "B for #1998." Refused `initrd` (the foreign spelling, which stops at the boundary
/// where it arrives).
#[cfg_attr(feature = "bench", allow(dead_code))] // the bench boot runs no user programs
pub fn initial_ramdisk() -> Option<&'static [u8]> {
    let (start, size) = memory::initial_ramdisk_region()?;

    // SAFETY: the region came from the device tree, it is inside RAM, the frame allocator has
    // been told it is forbidden, and the direct map names it. Nothing else will ever write here.
    Some(unsafe {
        core::slice::from_raw_parts(mmu::phys_to_virt(start) as *const u8, size as usize)
    })
}

/// The bytes of the program named `name` inside the initial ramdisk archive (milestone 19f (run a
/// real workload)). The initial ramdisk is a nifefs image carrying the progenitor plus the programs
/// it loads. The milestone tour and the kernel-side service demos ask for whichever program they
/// wire, by name, one program per demo. [`spawn_elf_child_builder`] and [`boot_progenitor`] instead
/// take the whole archive, because the progenitor parses the rest itself.
/// Returns `None` if there is no initial ramdisk, it will not parse, or it holds no such program.
// Used by the milestone tour, the kernel-wired virtio/console/shell demos, and the tests that load
// a user program; dead only in the bench boot, which runs no user programs.
#[cfg_attr(feature = "bench", allow(dead_code))]
pub fn program(name: &str) -> Option<&'static [u8]> {
    nifefs::Fs::parse(initial_ramdisk()?).ok()?.read(name)
}

/// Where the kernel maps the initial ramdisk read-only into the progenitor's address space
/// (milestone 19d): the progenitor reads the ELF to parse it here. A runtime window on the
/// address-space map (milestone 206 (a program image has under 896 KiB)): the kernel places it for
/// a program that did not choose the address.
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11).
/// His words: "B for #1998." Refused `INITRD_VA` (the foreign spelling, which stops at the boundary
/// where it arrives).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // becomes the boot path at 19d.2; test-driven until then
pub const INITIAL_RAMDISK_VA: u64 = address_space_map::runtime_window(0x2000_0000);
