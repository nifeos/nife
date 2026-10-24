//! **Bringing this CPU's architecture state up**: the GDT, TSS and IDT, and the control-register
//! and PAT settings that close ring 3 off from what it must not reach.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Bring this CPU's architecture state up: the GDT and TSS, then the IDT. The order is forced: an
/// IDT entry names a code selector, so the GDT that selector indexes must be installed before any
/// trap can be delivered through it.
pub fn init() {
    // First, before this core prints anything or adopts the fine map: the fine map's framebuffer
    // pages select PAT entry 1, and on a core that has not run this, entry 1 is write-through.
    give_write_combining_a_page_attribute_entry();
    // SAFETY: called once per CPU during boot, with a valid stack, before interrupts are unmasked.
    unsafe { segments::init() };
    exceptions::init();
    // Per-CPU like the GDT and IDT, and before anything can `syscall`: until this runs, a
    // `syscall` jumps to whatever `IA32_LSTAR` holds, which on a cold machine is zero.
    //
    // SAFETY: `segments::init` above installed the GDT these selectors index, and nothing has
    // entered ring 3 (nothing can: this is the boot CPU's own bring-up).
    unsafe { exceptions::init_syscall() };

    close_performance_counters_to_ring3();
    close_ring3_pages_to_ring0_execution();
}

/// **Program `IA32_PAT` so that `PWT` alone means write-combining**, which is what
/// `paging::Flags::write_combining()` encodes and what the framebuffer is mapped with
/// (`mmu::direct_map_claims`).
///
/// Entry 1 is rewritten without the cache flush the SDM asks for, because no mapping uses it yet:
/// this runs before `mmu::init` builds the fine map, or on a secondary before
/// `mmu::init_secondary` adopts it, and runs on every core so they agree. Without the PAT
/// (`CPUID.1:EDX[16]`), `PWT` alone is write-through and the boot line says so. See
/// notes/x86-port/cpu-state.md.
///
/// Name: provisional (the console-scroll lane).
pub(super) fn give_write_combining_a_page_attribute_entry() {
    /// `CPUID.1:EDX` bit 16: "PAT".
    const CPUID_1_EDX_PAT: u32 = 1 << 16;
    if isa::cpuid(1).edx & CPUID_1_EDX_PAT == 0 {
        crate::println!(
            "  pat         : not offered by cpuid on core {}; the screen is write-through here",
            crate::cpu::id()
        );
        return;
    }
    // SAFETY: CPUID advertised the MSR, so neither access can `#GP`, and every entry in the value
    // is a defined memory type. The one entry it changes is selected by no mapping on this core
    // (see above), so no cached line or TLB entry has the old type to disagree with.
    let found = unsafe { read_msr(paging::x86_64::pat::MSR) };
    if found != paging::x86_64::pat::VALUE {
        // SAFETY: as the read's.
        unsafe { write_msr(paging::x86_64::pat::MSR, paging::x86_64::pat::VALUE) };
    }
    crate::println!(
        "  pat         : {:#018x} on core {} (was {found:#018x}); entry 1 is write-combining",
        paging::x86_64::pat::VALUE,
        crate::cpu::id()
    );
}

/// Establish `CR4.SMEP` set, so ring 0 cannot execute an instruction fetched from a page whose
/// `U/S` bit says it belongs to ring 3.
///
/// `crates/paging`'s decoder reports a user page as not kernel-executable, and that is true only
/// with this bit set. Setting it without `CPUID.(7,0):EBX[7]` is `#GP`, so a CPU that does not
/// offer it is left alone and the boot line says ring 0 can execute ring-3 pages there. Per core:
/// `CR4` is per-CPU state. See notes/x86-port/cpu-state.md.
pub(super) fn close_ring3_pages_to_ring0_execution() {
    /// `CR4.SMEP`: "Supervisor Mode Execution Prevention".
    const SMEP: u64 = 1 << 20;
    /// `CPUID.(7,0):EBX` bit 7 advertises SMEP.
    const CPUID_7_EBX_SMEP: u32 = 1 << 7;

    // Leaf 7 exists only when leaf 0 says the maximum basic leaf reaches it; `isa::init` makes
    // the same check before reading RDSEED out of the same word.
    let offered = isa::cpuid(0).eax >= 7 && isa::cpuid_count(7, 0).ebx & CPUID_7_EBX_SMEP != 0;
    if !offered {
        crate::println!(
            "  cr4.smep    : not offered by cpuid on core {}; ring 0 can execute ring-3 pages here",
            crate::cpu::id()
        );
        return;
    }

    let cr4 = instructions::read_cr4();
    if cr4 & SMEP != 0 {
        return;
    }
    // SAFETY: setting `CR4.SMEP` only *removes* a ring-0 permission the kernel never uses (no
    // kernel code lives in a `U/S` page). CPUID advertised the bit, so the write cannot `#GP`. Paging
    // bits are preserved; no TLB entry is invalidated, and the bit takes effect on the next fetch
    // without one, because SMEP is evaluated against the leaf at fetch time.
    unsafe { instructions::write_cr4(cr4 | SMEP) };
    crate::println!(
        "  cr4.smep    : set on core {}; ring 0 faults on a fetch from a ring-3 page",
        crate::cpu::id()
    );
}

/// Establish `CR4.PCE` clear, so ring 3 cannot execute `RDPMC`.
///
/// An open `PCE` is a second path to a cycle-rate counter (fixed counter 2 runs at the TSC rate)
/// that the kernel never granted, and `boot.s` writes only `PAE` into `CR4`, so the bit is whatever
/// firmware left. It is read first, and the boot console says so when firmware left it set.
/// `CR4.TSD` stays open on purpose: `user_mode_runtime::now()` is `rdtsc`. Per core: `CR4` is
/// per-CPU state. See notes/x86-port/cpu-state.md.
pub(super) fn close_performance_counters_to_ring3() {
    /// `CR4.PCE`: "Performance-Monitoring Counter Enable". Set means `RDPMC` is legal at any
    /// privilege level; clear means ring 0 only.
    const PCE: u64 = 1 << 8;

    let cr4 = instructions::read_cr4();
    if cr4 & PCE == 0 {
        return;
    }

    crate::println!("  cr4.pce     : firmware left RDPMC open to ring 3; closing it");
    // SAFETY: clearing `CR4.PCE` only *removes* a ring-3 permission. It does not touch paging
    // (`PAE`, bit 5, is preserved by the mask), does not invalidate any TLB entry, and cannot make
    // a kernel access illegal, since `RDPMC` at CPL 0 is legal whatever this bit says. Every other
    // bit is written back as it was read.
    unsafe { instructions::write_cr4(cr4 & !PCE) };
}
