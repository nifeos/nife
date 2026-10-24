//! **Loading an image**: checking an ELF against the image band, mapping its segments and the
//! timebase page, and finding a program in the initial ramdisk. See notes/user-address-spaces.md.
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
    /// end and the stack's base, or linked somewhere else entirely (milestone 206, DECISIONS §171
    /// (where a program image starts) option A). Refused before a page is mapped. Its `Display` is
    /// the sentence the boot prints.
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
/// `image` is the ELF, and `windowed` is how many pages the caller will map into the new space
/// itself (zero for none). Returns the space and the entry point, or the [`LoadError`] that refused
/// the image. Split from [`run`] because this is the part that can fail, and a test can call it
/// without dying.
///
/// Mapped windows cost table and log pages out of this space's own region, so the budget charges
/// `windowed` here, less [`WINDOW_IN_OVERHEAD`], rather than asking each caller to widen it. See
/// notes/user-address-spaces.md.
pub fn load(image: &[u8], windowed: u64) -> Result<(AddressSpace, u64), LoadError> {
    let elf = Elf::parse(image).map_err(LoadError::NotLoadable)?;
    check_image_band(&elf)?;

    // Counted from the file before anything is carved: a binary that lies about its size exhausts
    // its own region and fails to map, spending nobody else's memory.
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

    // The timebase page: `user_mode_runtime::now()`'s rate where there is no `CNTFRQ_EL0` to read
    // (`x86_64` measures it, `riscv64` reads it from the device tree). Mapped rather than carved,
    // so it spends none of `content`.
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

/// **The timebase page's one physical frame**, written once and mapped read-only into every process
/// on `x86_64` and `riscv64`, since the rate does not change while the machine runs. aarch64 needs
/// none: `CNTFRQ_EL0` is architected and open to EL0.
///
/// `None` only if the frame allocator is out of memory. If the rate has not resolved yet the frame
/// stays zeroed, which [`counter_frequency_contract::TimebasePage::hz`] reads as unknown.
#[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
fn timebase_page_phys() -> Option<u64> {
    use core::sync::atomic::{AtomicU64, Ordering};
    static PAGE_PHYS: AtomicU64 = AtomicU64::new(0);

    let cached = PAGE_PHYS.load(Ordering::Acquire);
    if cached != 0 {
        return Some(cached);
    }

    // `alloc_zeroed`: only the first bytes are written, and the whole frame is mapped into every
    // process on this architecture.
    let phys = crate::memory::alloc_zeroed()?.addr();
    let dst = mmu::phys_to_virt(phys) as *mut u8;
    match crate::arch::timer::frequency_checked() {
        Some(hz) => {
            let bytes = counter_frequency_contract::build_page(hz);
            // SAFETY: `dst` names a freshly allocated frame, reachable through the direct map and
            // owned by nobody else yet; `bytes` is `PAGE_BYTES` (24) bytes, far under the frame's
            // `FRAME_SIZE`, so the copy does not run past it.
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len()) };
        }
        // Not yet measured: the frame stays zero, which reads as unknown.
        // SAFETY: as the `Some` arm above: `dst` names a freshly allocated, exclusively owned
        // frame, and `PAGE_BYTES` is far under `FRAME_SIZE`.
        None => unsafe { core::ptr::write_bytes(dst, 0, counter_frequency_contract::PAGE_BYTES) },
    }

    PAGE_PHYS.store(phys, Ordering::Release);
    Some(phys)
}

/// Map the timebase page into `space`, for a process the kernel builds by hand rather than through
/// [`load`].
///
/// [`spawn_elf_child_builder`] and the test harnesses that hand a narrowed archive to one program
/// build their spaces this way, and `load`'s own mapping reaches none of them. One function, so
/// every such site maps the page the same way; see notes/user-address-spaces.md.
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

/// Lay an ELF's loadable segments into `space`, honouring their permissions exactly; the
/// progenitor's userspace loader mirrors it. A read-only segment gets `user_rodata`, not
/// `user_data`, and `.bss` is free because `map_new` zeroes every page.
pub fn map_segments(space: &mut AddressSpace, elf: &Elf) -> Result<(), LoadError> {
    // Enforced here as well as in [`load`], which checks before it sizes a region so a huge image
    // is told it is too large rather than that memory ran out.
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
