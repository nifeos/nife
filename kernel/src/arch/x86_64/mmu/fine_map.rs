//! **Building and checking the fine-grained map**: every mapping the kernel needs, in order,
//! the diagnosis a refused one carries, and the checks made before the hardware is handed it.
//!
//! Cut out of `mmu.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines). `super::current_sp` names the `x86_64` item the index
//! imports, as in `mmu.rs`.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// **Which mapping [`map_everything`] was making when the mapper refused, and what else claims the
/// page it refused.**
///
/// [`MapError`] names the symptom. `AlreadyMapped` on its own cost a bench session on 2026-09-05:
/// the machine had no serial cable, a photograph of the screen was the entire transcript, and the
/// panic distinguished none of the eleven kinds of range this function maps. The pair of ranges is
/// what makes such a photograph a diagnosis rather than a hypothesis, and it is worth more than any
/// particular fix, because the next machine nobody can attach a debugger to is the one after this.
///
/// Name: provisional.
pub(super) struct MapFailure {
    pub(super) what: &'static str,
    pub(super) lo: u64,
    pub(super) hi: u64,
    pub(super) err: MapError,
    /// The first page of `lo..hi` some *other* claim also covers, and that claim. `None` when
    /// nothing else in the enumeration wants it, which for `AlreadyMapped` is itself a finding:
    /// something outside [`direct_map_claims`] put it there.
    pub(super) conflict: Option<(u64, &'static str, u64, u64)>,
}

impl core::fmt::Display for MapFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:?} mapping {} {:#x}..{:#x}",
            self.err, self.what, self.lo, self.hi
        )?;
        match self.conflict {
            Some((pa, other, olo, ohi)) => write!(
                f,
                ": {:#x} is also claimed by {} {:#x}..{:#x}",
                pa, other, olo, ohi
            ),
            None => write!(
                f,
                ": no other claim covers it, so something outside map_everything did"
            ),
        }
    }
}

/// The first page of `lo..hi` that a claim other than `what` also covers.
///
/// Walks the claims rather than the page tables, and walks them by range rather than page by page:
/// a RAM region on a 17 GiB machine is four million pages and this runs inside a panic path.
fn first_conflict(what: &'static str, lo: u64, hi: u64) -> Option<(u64, &'static str, u64, u64)> {
    let mut best: Option<(u64, &'static str, u64, u64)> = None;
    direct_map_claims(&mut |c| {
        if c.what == what || c.hi <= c.lo {
            return;
        }
        let start = c.lo.max(lo);
        let end = c.hi.min(hi);
        if start >= end {
            return;
        }
        if best.is_none_or(|(pa, ..)| start < pa) {
            best = Some((start, c.what, c.lo, c.hi));
        }
    });
    best
}

/// Build every mapping the kernel needs. Mirrors the aarch64 and RISC-V `map_everything`, and the
/// order is the same: physical memory first, then the image's own sections over the top of it.
pub(super) fn map_everything<A, P>(m: &mut Mapper<A, P, Ia32e>) -> Result<(), MapFailure>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    // Steps 1, 1b, 5 and 2, in the order `direct_map_claims` yields them. The closure stops doing
    // work after the first failure rather than returning early, because the enumeration is a
    // callback and there is nothing to return through; the cost is a few dozen no-op calls on the
    // one path that is about to panic.
    let mut failed: Option<MapFailure> = None;
    direct_map_claims(&mut |c| {
        if failed.is_some() {
            return;
        }
        // Blocks for memory, pages for devices: the MTRRs are not read (the BUGS of
        // notes/x86-port/kernel-page-tables.md).
        let result = if c.guarded {
            guarded_direct_map(m, c.lo, c.hi, c.flags)
        } else if c.flags.is_device() {
            direct_map(m, c.lo, c.hi, c.flags, PageSize::Size4KiB)
        } else {
            direct_map(m, c.lo, c.hi, c.flags, largest_leaf())
        };
        if let Err(err) = result {
            failed = Some(MapFailure {
                what: c.what,
                lo: c.lo,
                hi: c.hi,
                err,
                conflict: if err == MapError::AlreadyMapped {
                    first_conflict(c.what, c.lo, c.hi)
                } else {
                    None
                },
            });
        }
    });
    if let Some(f) = failed {
        return Err(f);
    }

    // 3. The kernel image, section by section, at its linked VAs. W^X: text executable and
    //    read-only, rodata neither writable nor executable, everything else writable and never
    //    executable. Until this map is installed the boot table had all of it present, writable AND
    //    executable, in both halves.
    let mut va = |what: &'static str, lo: u64, hi: u64, flags: Flags| -> Result<(), MapFailure> {
        map_range(m, lo, hi, flags).map_err(|err| MapFailure {
            what,
            lo,
            hi,
            err,
            conflict: None, // these are virtual ranges; the claim enumeration is physical
        })
    };
    va(".text", text_start(), text_end(), Flags::kernel_code())?;
    va(
        ".rodata",
        rodata_start(),
        rodata_end(),
        Flags::kernel_rodata(),
    )?;
    va(".data/.bss", data_start(), bss_end(), Flags::kernel_data())?;

    // 4. The stacks. The guard page below each is deliberately NOT mapped, which is the entire
    //    stack-overflow mechanism and is asserted in `verify`: a mapped guard page is protection
    //    that is silently off.
    va(
        "boot stack",
        stack_bottom(),
        stack_top(),
        Flags::kernel_data(),
    )?;
    for id in 0..crate::cpu::MAX_CPUS {
        let (bottom, top) = crate::smp::secondary_stack_span(id);
        va("secondary stack", bottom, top, Flags::kernel_data())?;
    }
    for id in 0..crate::cpu::MAX_CPUS {
        let (bottom, top) = crate::interrupt_stack::span(id);
        va("interrupt stack", bottom, top, Flags::kernel_data())?;
    }
    for id in 0..crate::cpu::MAX_CPUS {
        for k in 0..super::super::ist::VECTORS.len() {
            let (bottom, top) = super::super::ist::span(id, k);
            va("IST stack", bottom, top, Flags::kernel_data())?;
        }
    }

    Ok(())
}

/// Map a range of *physical* addresses into the direct map, **skipping pages already mapped**.
///
/// The one concession to a list nobody promised is disjoint, and it is deliberately not reachable
/// from the ordinary path: see [`Claim::guarded`].
fn guarded_direct_map<A, P>(
    m: &mut Mapper<A, P, Ia32e>,
    pa_start: u64,
    pa_end: u64,
    flags: Flags,
) -> Result<(), MapError>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    for pa in (pa_start..pa_end).step_by(PAGE_SIZE as usize) {
        if m.translate(phys_to_virt(pa)).is_none() {
            m.map(phys_to_virt(pa), pa, flags)?;
        }
    }
    Ok(())
}

/// Map a range of *virtual* addresses to the physical ones they were linked against.
fn map_range<A, P>(
    m: &mut Mapper<A, P, Ia32e>,
    va_start: u64,
    va_end: u64,
    flags: Flags,
) -> Result<(), MapError>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    if va_end <= va_start {
        return Ok(());
    }
    let pages = (va_end - va_start).div_ceil(PAGE_SIZE);
    m.map_range(va_start, virt_to_phys(va_start), pages, flags)
}

/// Map a range of *physical* addresses into the direct map at [`phys_to_virt`], in leaves no
/// larger than `largest`. `paging::Mapper::map_span` puts a block only where it lies wholly inside
/// the range, so this maps exactly the pages it always did, in fewer entries.
fn direct_map<A, P>(
    m: &mut Mapper<A, P, Ia32e>,
    pa_start: u64,
    pa_end: u64,
    flags: Flags,
    largest: PageSize,
) -> Result<(), MapError>
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    if pa_end <= pa_start {
        return Ok(());
    }
    let len = (pa_end - pa_start).next_multiple_of(PAGE_SIZE);
    m.map_span(phys_to_virt(pa_start), pa_start, len, flags, largest)
}

/// Walk the tables in software and check the things that would kill us, **before** the hardware bets
/// the machine on them. The x86 counterpart of the other two architectures' `verify`, and it earns
/// its keep here more than anywhere: a mistake in these tables is not a fault report, it is a page
/// fault taken with a page-table root that cannot describe the fault handler, which escalates to a
/// double fault, then to a triple fault, which QEMU shows as a silent machine reset.
pub(super) fn verify<A, P>(m: &Mapper<A, P, Ia32e>)
where
    A: FnMut() -> Option<u64>,
    P: Fn(u64) -> *mut PageTable,
{
    // The code we are executing right now. If this is wrong, the instruction after `mov cr3` never
    // gets fetched.
    let here = init as *const () as u64;
    let (pa, flags) = m
        .translate(here)
        .expect("the code switching tables is not mapped: we would die on the next fetch");
    assert_eq!(pa, virt_to_phys(here), "our .text maps to the wrong frame");
    assert!(
        flags.is_kernel_executable(),
        "our own .text is not executable"
    );
    assert!(
        !flags.is_writable(),
        "our own .text is writable (W^X violated)"
    );

    // This CPU's stack, which the `mov cr3` does not push to but the return from `install` does.
    let sp = super::current_sp();
    assert!(
        m.translate(sp).is_some(),
        "the current stack is not mapped: the return from `install` would fault"
    );

    // The direct map, asked about a frame the allocator actually owns rather than about the
    // arithmetic. Everything already in flight (the frame bitmap, the ACPI tables, every page table
    // this walk just wrote) is reached this way, so a direct map that came out short is the failure
    // that would look like memory corruption rather than like a page fault.
    let bitmap = memory::bitmap_region().0;
    assert!(
        m.translate(phys_to_virt(bitmap)).is_some(),
        "the frame allocator's own bitmap is not in the direct map"
    );

    // And the identity map is gone, which is half of what this step is for: an identity map is a
    // complete alias of physical memory sitting in the half ring 3 will get. Asked of the PML4
    // entry rather than through `translate`, which would answer `None` for any low address simply
    // because this mapper serves the high half, and so would pass whatever the tables said.
    //
    // SAFETY: `m.root()` is the table this function has been walking, reachable through the direct
    // map like every other table frame.
    let low_half_root = unsafe { (*phys_to_ptr(m.root())).entries[0] };
    assert_eq!(
        low_half_root, 0,
        "the low half of the fine tables is not empty: an identity map survived"
    );

    // The guard page must NOT be mapped, or stack-overflow protection is silently off and the next
    // time anyone finds out is during an overflow.
    assert!(
        m.translate(stack_guard()).is_none(),
        "the guard page IS mapped: stack overflow protection is off"
    );
    for id in 0..crate::cpu::MAX_CPUS {
        assert!(
            m.translate(crate::smp::secondary_stack_guard(id)).is_none(),
            "a secondary's guard page IS mapped: its stack overflow protection is off"
        );
        assert!(
            m.translate(crate::interrupt_stack::guard(id)).is_none(),
            "an interrupt stack's guard page IS mapped: its overflow protection is off"
        );
        for k in 0..super::super::ist::VECTORS.len() {
            assert!(
                m.translate(super::super::ist::guard(id, k)).is_none(),
                "an IST stack's guard page IS mapped: its overflow protection is off"
            );
        }
    }
}
