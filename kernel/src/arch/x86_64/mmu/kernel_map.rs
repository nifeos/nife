//! **The live kernel tables**: building the fine-grained map at boot, adopting it on a
//! secondary CPU, and mapping, unmapping and translating kernel pages under its lock.
//!
//! Cut out of `mmu.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines). `super::instructions` and `super::isa` name the
//! `x86_64` items the index imports, as in `mmu.rs`.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The physical address of the page-table root the CPU is currently walking (`CR3`, with the
/// PCID/flag bits masked off).
pub(crate) fn current_root() -> u64 {
    read_cr3() & 0x000f_ffff_ffff_f000
}

/// Print what the MMU is doing, one line, on every boot. The x86 twin of the other two
/// architectures' summaries.
pub(crate) fn print_summary() {
    let root = KERNEL_ROOT.load(Ordering::Relaxed);
    if root == 0 {
        crate::println!(
            "  mmu             : 4-level paging on (cr3 {:#x}), boot map: 4 GiB identity + high alias",
            current_root(),
        );
    } else {
        crate::println!(
            "  mmu             : fine W^X 4-level map installed (cr3 {:#x}), image {:#x}, direct map {:#x}",
            current_root(),
            KERNEL_VA_BASE,
            DIRECT_MAP_BASE,
        );
        // The cost, measured, and the leaf size that set it: RAM goes in the largest block the
        // machine offers and everything else in 4 KiB pages (the BUGS of
        // notes/x86-port/kernel-page-tables.md).
        crate::println!(
            "                  : {} KiB of page tables (ram in {} blocks), no identity map, guard pages are holes",
            TABLE_FRAMES.load(Ordering::Relaxed) * PAGE_SIZE / 1024,
            match largest_leaf() {
                PageSize::Size1GiB => "1 GiB",
                PageSize::Size2MiB => "2 MiB",
                PageSize::Size4KiB => "4 KiB",
            },
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The fine-grained kernel map (milestone 161, roadmap item 1). The third instance of the same
// shape aarch64 and riscv64 already have: build a root, walk it with `paging::Mapper`, check the
// things that would kill us in software, then hand the root to the hardware.
// ---------------------------------------------------------------------------------------------

/// The kernel's fine-map root, saved by [`init`] so a secondary CPU can adopt it and so the kernel
/// mapper does not have to read `CR3` back (which will name a *process* root once ring 3 exists).
pub(super) static KERNEL_ROOT: AtomicU64 = AtomicU64::new(0);

/// How many frames the fine map cost, root and intermediate tables together. Reported on every
/// boot rather than left to be estimated: it is the number blocks exist to shrink.
static TABLE_FRAMES: AtomicU64 = AtomicU64::new(0);

/// **The largest leaf this machine's direct map may use**: 1 GiB where `CPUID` leaf 0x80000001
/// reports `Page1GB` (EDX bit 26), 2 MiB everywhere else, since every long-mode CPU has 2 MiB
/// pages. Asked of the hardware rather than assumed, because a `PS` bit in a PDPT entry on a CPU
/// without `Page1GB` is a reserved bit, and the fault it raises is taken through tables that
/// cannot describe the fault handler. The extended range's own maximum (leaf 0x80000000) is
/// checked first, because a leaf past it returns another leaf's answer rather than zeros.
pub(super) fn largest_leaf() -> PageSize {
    const PAGE_1GB: u32 = 1 << 26;
    if super::isa::cpuid(0x8000_0000).eax >= 0x8000_0001
        && super::isa::cpuid(0x8000_0001).edx & PAGE_1GB != 0
    {
        PageSize::Size1GiB
    } else {
        PageSize::Size2MiB
    }
}

/// Serializes edits to the kernel's live tables: two CPUs must not mutate them at once. Same role
/// and lock rank as the other two architectures' `KERNEL_MMU`.
static KERNEL_MMU: crate::sync::IrqSafeMutex<()> =
    crate::sync::IrqSafeMutex::new(crate::sync::rank::KERNEL_MMU, ());

/// **Replace the boot map with a fine-grained one**: W^X kernel sections, the guard pages
/// unmapped, device registers device-typed, a direct map over all of physical memory, and no
/// identity map.
///
/// The fine table maps the same kernel VAs to the same frames as the boot table, so the `mov cr3`
/// is seamless; [`verify`] checks that before the switch. COM1 is an I/O port, not a mapping, so
/// nothing here can silence the console.
pub fn init() {
    let free_before = memory::free_page_frames();
    let root = memory::alloc()
        .expect("no frame for the root page table")
        .addr();
    // SAFETY: a fresh frame from the allocator, reachable through the boot tables' direct map. Zero
    // it before the hardware could ever walk it.
    unsafe {
        (*phys_to_ptr(root)).entries = [0; paging::ENTRIES];
    }

    // SAFETY: `root` is zeroed and page-aligned; `phys_to_ptr` is valid because `boot.s` installed
    // the direct map over the low 4 GiB before any Rust ran, so every frame the mapper allocates is
    // addressable through it (notes/x86-port/kernel-page-tables.md's BUGS has the >4 GiB case).
    let mut mapper = unsafe {
        Mapper::<_, _, Ia32e>::new(
            root,
            Half::High,
            || memory::alloc().map(|f| f.addr()),
            phys_to_ptr,
        )
    };

    if let Err(f) = map_everything(&mut mapper) {
        // Not `expect`: the message is the whole transcript on a machine with no serial cable, so
        // it names the two ranges rather than the symptom. See `MapFailure`.
        panic!("failed to build the kernel page tables: {f}");
    }
    verify(&mapper);
    TABLE_FRAMES.store(
        (free_before - memory::free_page_frames()) as u64,
        Ordering::Relaxed,
    );

    // SAFETY: `verify` just walked these tables and asserted they cover this function's own code,
    // this CPU's stack, and the direct map every pointer already in flight was derived through.
    unsafe { install(root) };

    KERNEL_ROOT.store(root, Ordering::Relaxed);
}

/// Install the tables rooted at physical `root` by writing `CR3`.
///
/// Writing `CR3` invalidates every non-global TLB entry on this CPU, and `CR4.PGE` is clear, so in
/// practice it invalidates everything: there is no stale boot-table translation left afterwards.
///
/// # Safety
/// `root` must be a complete kernel map covering the currently-executing code, the current stack,
/// and any memory touched before the next barrier; otherwise the instruction after the write is
/// fetched through a table that does not describe it, which on this architecture is a page fault
/// escalating to a triple fault and a silent machine reset.
pub(super) unsafe fn install(root: u64) {
    // SAFETY: the caller's contract.
    unsafe { super::instructions::write_cr3(root) };
}

/// Adopt the kernel map on a secondary CPU. Every CPU shares one kernel root (`CR3` names the whole
/// address space, unlike `TTBR0`/`TTBR1`'s split), so there is nothing per-CPU to build: the
/// trampoline (`boot.s`'s `secondary_boot`) already installed `boot_pml4`, and this just switches
/// to the same fine root the boot core is already running on. Called from `secondary_main`'s step
/// 3, same as the other two architectures.
pub(crate) fn init_secondary() {
    let root = KERNEL_ROOT.load(Ordering::Relaxed);
    assert_ne!(
        root, 0,
        "a secondary CPU reached init_secondary before init"
    );
    // SAFETY: the primary built this root, verified it, and is running on it.
    unsafe { install(root) };
}

/// The kernel's live page tables, as a `Mapper`. **Call only while holding [`KERNEL_MMU`].**
#[allow(clippy::type_complexity)]
fn kernel_mapper() -> Mapper<impl FnMut() -> Option<u64>, fn(u64) -> *mut PageTable, Ia32e> {
    let root = KERNEL_ROOT.load(Ordering::Relaxed);
    // SAFETY: `root` is the fine kernel table `init` built; the direct map makes `phys_to_ptr` valid
    // for every table frame.
    unsafe {
        Mapper::new(
            root,
            Half::High,
            || memory::alloc().map(|f| f.addr()),
            phys_to_ptr,
        )
    }
}

/// Map one page in the kernel's address space.
pub(crate) fn map_page(va: u64, pa: u64, flags: Flags) -> Result<(), MapError> {
    let _guard = KERNEL_MMU.lock(); // exclusive: two CPUs must not mutate the tables at once
    kernel_mapper().map(va, pa, flags)
}

/// Unmap one page in the kernel's address space, invalidate the TLB, and return the physical frame
/// (the caller's to free; the mapper never owned it).
#[allow(dead_code)]
pub(crate) fn unmap_page(va: u64) -> Result<u64, MapError> {
    let _guard = KERNEL_MMU.lock(); // exclusive: see map_page
    let (pa, flush) = kernel_mapper().unmap(va)?;
    flush.flush(flush_tlb);
    Ok(pa)
}

/// Invalidate the TLB entry for `va`, **on every online CPU**.
///
/// The x86 instruction is `invlpg`, which unlike aarch64's `tlbi ..., is` is local to the CPU that
/// executes it. So this is the local invalidate plus [`shoot_down_others`], the same shape RISC-V's
/// `flush_tlb` has (an `sfence.vma` plus an SBI RFENCE) and for the same reason: without the remote
/// half, a core that cached a translation for `va` keeps using it after this one has handed the
/// frame away.
///
/// Local first, so this core's own page-table write is retired before anyone is told to look.
pub(crate) fn flush_tlb(va: u64) {
    // Getting TLB maintenance wrong means a stale translation, which is the memory-unsafety that
    // matters here rather than Rust's.
    super::instructions::invlpg(va);
    shoot_down_others(va);
}

/// Translate a kernel virtual address through the live kernel tables.
pub fn translate(va: u64) -> Option<(u64, Flags)> {
    let _guard = KERNEL_MMU.lock();
    kernel_mapper().translate(va)
}

/// **Is this kernel address mapped, asked without taking a lock?**
///
/// For fault handlers, which is why it does not go through [`translate`]: a handler that has already
/// lost the machine must not block on a lock whose holder might be the thread that just died. Same
/// disposition and same caller (`stack::print_text_words`) as the RISC-V twin.
pub(crate) fn is_mapped(va: u64) -> bool {
    let root = KERNEL_ROOT.load(Ordering::Relaxed);
    if root == 0 {
        // Before `init`, the boot map covers the low 4 GiB three ways over and nothing is a hole.
        return true;
    }
    // SAFETY: `root` is the live kernel root; the direct map makes `phys_to_ptr` valid; a translate
    // allocates nothing, so the `|| None` allocator is never called.
    let mapper = unsafe { Mapper::<_, _, Ia32e>::new(root, Half::High, || None, phys_to_ptr) };
    mapper.translate(va).is_some()
}
