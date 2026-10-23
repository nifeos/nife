//! **The three stacks the CPU switches to on its own**: one per core for the NMI (vector 2), the
//! double fault (8) and the machine check (18), named by the TSS's Interrupt Stack Table (IST).
//!
//! None of the three can trust the `rsp` it interrupted. Each stack is 16 KiB over an unmapped
//! 4 KiB guard page, in the `(NOLOAD)` region `.ist_stacks`; the word above each frame holds the
//! core's per-CPU pointer, so `isr_paranoid` finds the kernel's GS base from its own `rsp`. The
//! NMI handler must take no exception that returns, since any `iretq` reopens NMIs;
//! `exceptions::x86_paranoid_dispatch` panics if an NMI lands on an NMI stack. See
//! notes/x86-port/cpu-state.md, which holds the detail of each BUGS entry.
//!
//! Name: provisional: milestone 915 (NMI, double fault and machine check run on their own stacks),
//! 2026-10-13 UTC. `ist` is the architecture's own term for the table these stacks hang from.

//! # BUGS
//!
//! - `script/stack-depth-check` does not measure these stacks.
//! - They cost 3 x 20 KiB x `MAX_CPUS` (480 KiB) of address space whether or not the cores exist.

use core::cell::SyncUnsafeCell;

use crate::cpu::MAX_CPUS;

/// The usable stack under each IST entry: four pages, `crate::interrupt_stack::SIZE`'s size and
/// for its reason (the fatal path prints a register dump from here).
pub(crate) const SIZE: usize = 4 * 4096;

/// The unmapped page below each stack.
const GUARD: usize = 4096;

/// One stack's slot: its guard page, then the stack that grows down into it.
const SLOT: usize = GUARD + SIZE;

/// How far below a stack's top `TSS.ist` points. The 16 bytes above hold the per-CPU pointer (the
/// lower word) and one word of padding that keeps the IST value 16-byte aligned, which the CPU
/// requires of the frame it pushes and the System V ABI requires at `isr_paranoid`'s `call`.
pub(crate) const TOP_RESERVE: u64 = 16;

/// The IST vectors this module serves, in slot order: `TSS.ist[k]` is `VECTORS[k]`'s stack, and the
/// IDT gate for `VECTORS[k]` selects IST entry `k + 1` (the IDT encodes 0 as "do not switch").
pub(crate) const VECTORS: [u8; 3] = [8, 2, 18];

/// The IST entry (one-based, as the IDT encodes it) the gate for `vector` selects, or 0 for a
/// vector that runs on the stack it interrupted.
pub(crate) const fn slot_for(vector: usize) -> u8 {
    let mut k = 0;
    while k < VECTORS.len() {
        if VECTORS[k] as usize == vector {
            return k as u8 + 1;
        }
        k += 1;
    }
    0
}

/// The page-aligned layout of the region: per core, one slot per entry of [`VECTORS`].
#[repr(C, align(4096))]
struct Stacks([[[u8; SLOT]; VECTORS.len()]; MAX_CPUS]);

/// The stacks. A `SyncUnsafeCell` for calef's ruling of 2026-10-09 on milestone 891 (a mutable
/// static becomes an atomic, a lock or a recorded cell): the CPU switches to these by address. It
/// must be a cell rather than a plain static for `crate::interrupt_stack::STACKS`'s reason, since
/// an immutable static of plain arrays lands in `.rodata`, which the fine map makes read-only.
/// Plain access is sound because each core touches only its own three slots: as stacks, through
/// `rsp`, after the CPU's IST switch on that core, and the one per-CPU word [`install`] writes
/// before any IST vector can be taken there.
#[cfg_attr(target_os = "none", unsafe(link_section = ".ist_stacks"))]
static STACKS: SyncUnsafeCell<Stacks> =
    SyncUnsafeCell::new(Stacks([[[0; SLOT]; VECTORS.len()]; MAX_CPUS]));

/// The low end of core `id`'s slot `k` (zero-based, an index into [`VECTORS`]): its guard page.
/// Indexing bounds-checks both numbers.
pub(crate) fn guard(id: usize, k: usize) -> u64 {
    // SAFETY: `&raw const` of an element place projects the address, bounds-checked, without
    // reading the slot or making a reference to memory a core may be running on.
    (unsafe { &raw const (*STACKS.get()).0[id][k] }) as u64
}

/// Core `id`'s stack `k` as `(bottom, top)`: the slot above its guard page.
pub(crate) fn span(id: usize, k: usize) -> (u64, u64) {
    let bottom = guard(id, k) + GUARD as u64;
    (bottom, bottom + SIZE as u64)
}

/// The whole region, `(start, end)`, for the check against what the linker reserved.
#[cfg(test)]
fn region() -> (u64, u64) {
    let start = STACKS.get() as u64;
    (start, start + (MAX_CPUS * VECTORS.len() * SLOT) as u64)
}

/// **Point this core's three IST entries at its own stacks**, and write this core's per-CPU pointer
/// into the word above each one's frame.
///
/// Called from `segments::init`, once per core, after `cpu::init_this_cpu` and the GDT reload and
/// before `exceptions::init` loads the IDT whose gates select these entries.
///
/// # Safety
/// Once per core, on that core, before any IST vector can be delivered to it, with `percpu` this
/// core's own per-CPU block.
pub unsafe fn install(id: usize, percpu: u64) {
    for k in 0..VECTORS.len() {
        let ist_top = span(id, k).1 - TOP_RESERVE;
        // SAFETY: `ist_top` is 16 bytes below the top of this core's own stack `k`, inside the slot,
        // 8-aligned, and nothing has run on the stack yet (no IST vector can be delivered before
        // the IDT this core loads after this call).
        unsafe { (ist_top as *mut u64).write_volatile(percpu) };
        // SAFETY: one stack per IST entry, never shared with another vector or another core.
        unsafe { super::segments::set_interrupt_stack(k as u8 + 1, ist_top) };
    }
}

/// Whether `addr` lies on some core's NMI stack. A core only ever stands on its own, so asking
/// about every core's is the same question without needing to know which core this is, which is
/// what lets `exceptions::x86_paranoid_dispatch` ask it before trusting anything reached through
/// `gs`.
pub(crate) fn is_on_an_nmi_stack(addr: u64) -> bool {
    const NMI: usize = slot_for(2) as usize - 1;
    (0..MAX_CPUS).any(|id| {
        let (bottom, top) = span(id, NMI);
        (bottom..top).contains(&addr)
    })
}

/// Test-only: set by [`tests`] just before it sends this core an NMI, and cleared by the NMI
/// handler when it serves that NMI, so the test can tell its own NMI from a shootdown and the
/// handler does not count it in `exceptions::NMIS_UNCLAIMED`.
#[cfg(test)]
static TEST_NMI_PENDING: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Test-only: the per-CPU pointer (`super::percpu()`, one `gs`-relative load) the NMI handler read
/// while serving the test's NMI.
#[cfg(test)]
static TEST_NMI_SAW_PERCPU: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// Test-only half of the NMI handler: if [`tests`] is waiting on an NMI, record what `gs` names and
/// tell it the NMI arrived. Returns whether there was one to serve.
#[cfg(test)]
pub(super) fn serve_test_nmi() -> bool {
    use core::sync::atomic::Ordering;
    if !TEST_NMI_PENDING.load(Ordering::Acquire) {
        return false;
    }
    TEST_NMI_SAW_PERCPU.store(super::percpu(), Ordering::Relaxed);
    TEST_NMI_PENDING.store(false, Ordering::Release);
    true
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::Ordering;

    use super::*;

    /// **The NMI, double-fault and machine-check gates select this module's three entries, and no
    /// other gate selects any.** Read from the IDT the CPU uses.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.ist.tests.the_nmi_double_fault_and_machine_check_gates_select_their_own_stacks.patch`
    ///
    /// Name: provisional (milestone 915, 2026-10-13 UTC).
    #[test_case]
    fn the_nmi_double_fault_and_machine_check_gates_select_their_own_stacks() {
        for (k, &vector) in VECTORS.iter().enumerate() {
            assert_eq!(
                super::super::exceptions::gate_ist(vector as usize),
                k as u8 + 1,
                "vector {vector}'s gate does not select IST entry {}",
                k + 1
            );
        }
        for vector in (0..256).filter(|v| !VECTORS.contains(&(*v as u8))) {
            assert_eq!(
                super::super::exceptions::gate_ist(vector),
                0,
                "vector {vector}'s gate selects an IST entry no stack was built for"
            );
        }
    }

    /// What the decoy GS base reads as a per-CPU pointer: no `PerCpu` lives there.
    const DECOY_PERCPU: usize = 0xdec0_7dec_0700_0001;

    /// A stand-in for a GS base that is not the kernel's: the value a user program left there, as
    /// `IA32_GS_BASE` holds in the windows around `swapgs` that trap.s's header names. Mapped and
    /// readable, so a handler that trusts it reads [`DECOY_PERCPU`] rather than faulting, and the
    /// test reports a wrong answer instead of a crash.
    static DECOY: [usize; super::super::PERCPU_SELF_OFF / 8 + 1] =
        [DECOY_PERCPU; super::super::PERCPU_SELF_OFF / 8 + 1];

    /// Send this core an NMI while `IA32_GS_BASE` holds [`DECOY`]'s address. Returns the per-CPU
    /// pointer the handler read and the GS base the NMI's `iretq` left behind, or `None` if the NMI
    /// never arrived.
    ///
    /// Nothing between the two `IA32_GS_BASE` writes may read `gs`: interrupts are masked, the
    /// APIC send reads a static, and the wait reads an atomic. An NMI from another core's shootdown
    /// can still land in the window, which is the case under test.
    fn an_nmi_under_a_decoy_gs_base() -> Option<(usize, u64)> {
        let me = super::super::irq::local_apic_id();
        let was = crate::arch::interrupts::disable();
        // SAFETY: `IA32_GS_BASE` exists on every long-mode CPU. Between the decoy write and the
        // restore nothing on this core reads `gs` except the NMI handler under test.
        let real = unsafe { super::super::read_msr(super::super::IA32_GS_BASE) };
        // SAFETY: the decoy is mapped and readable, and nothing reads `gs` until the restore below
        // except the NMI handler, whose entry is what this test is about.
        unsafe { super::super::write_msr(super::super::IA32_GS_BASE, DECOY.as_ptr() as u64) };
        TEST_NMI_PENDING.store(true, Ordering::Release);
        super::super::irq::send_nmi(me);
        let mut spins = 0u64;
        while TEST_NMI_PENDING.load(Ordering::Acquire) && spins < 1 << 28 {
            core::hint::spin_loop();
            spins += 1;
        }
        // SAFETY: as above.
        let after = unsafe { super::super::read_msr(super::super::IA32_GS_BASE) };
        let arrived = !TEST_NMI_PENDING.swap(false, Ordering::AcqRel);
        // SAFETY: puts back the value read above, this core's own per-CPU pointer.
        unsafe { super::super::write_msr(super::super::IA32_GS_BASE, real) };
        crate::arch::interrupts::restore(was);
        arrived.then(|| (TEST_NMI_SAW_PERCPU.load(Ordering::Relaxed), after))
    }

    /// **An NMI taken while the GS base holds a value that is not the kernel's still finds this
    /// core's per-CPU block**, because `isr_paranoid` loads the kernel's GS base from the IST
    /// stack's top word instead of deciding from the saved `CS`. The saved `CS` here says ring 0,
    /// as it does in the windows around `swapgs`, so a `CS`-guarded `swapgs` would not fire and
    /// the handler would read the decoy.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.ist.tests.an_nmi_finds_its_per_cpu_block_whatever_the_gs_base_held.patch`
    ///
    /// Name: provisional (milestone 915, 2026-10-13 UTC).
    #[test_case]
    fn an_nmi_finds_its_per_cpu_block_whatever_the_gs_base_held() {
        let ours = super::super::percpu();
        let (saw, _) = an_nmi_under_a_decoy_gs_base().expect("the self-NMI never arrived");
        assert_ne!(
            saw, DECOY_PERCPU,
            "the NMI handler read its per-CPU pointer through the decoy GS base"
        );
        assert_eq!(
            saw, ours,
            "the NMI handler found a per-CPU block that is not this core's"
        );
    }

    /// **An NMI gives back the GS base it found**, so the interrupted code (here, a window that
    /// holds a user's GS base on its way to `swapgs` or `iretq`) resumes with its own value.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.ist.tests.an_nmi_leaves_the_gs_base_it_found.patch`
    ///
    /// Name: provisional (milestone 915, 2026-10-13 UTC).
    #[test_case]
    fn an_nmi_leaves_the_gs_base_it_found() {
        let (_, after) = an_nmi_under_a_decoy_gs_base().expect("the self-NMI never arrived");
        assert_eq!(
            after,
            DECOY.as_ptr() as u64,
            "the NMI returned with a different GS base from the one it interrupted"
        );
    }

    /// The linker reserved at least what this module lays out. A region smaller than the stacks
    /// would put the last core's machine-check stack over whatever the linker placed next, with no
    /// fault at all. `interrupt_stack`'s test of the same name holds the same line one region over.
    #[test_case]
    fn the_ist_stack_region_is_the_size_the_linker_reserved() {
        unsafe extern "C" {
            static __ist_stacks_start: core::ffi::c_void;
            static __ist_stacks_end: core::ffi::c_void;
        }
        let start = (&raw const __ist_stacks_start) as u64;
        let end = (&raw const __ist_stacks_end) as u64;
        let (ours_start, ours_end) = region();
        assert_eq!(
            start, ours_start,
            "the IST stacks do not start where the linker put them"
        );
        assert!(
            ours_end <= end,
            "the IST stacks ({} bytes) do not fit the {} bytes the linker reserved",
            ours_end - ours_start,
            end - start,
        );
    }

    /// **Every online core's TSS names three IST stacks of its own**: each entry nonzero, inside
    /// that core's stack for that vector, with the core's per-CPU pointer in the word above it; and
    /// each stack's guard page unmapped in the live kernel tables.
    ///
    /// Red if `install` is never called: `TSS.ist` then holds 0, and a gate that selects an IST
    /// entry switches to a stack at address 0.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.ist.tests.every_online_core_has_its_own_three_interrupt_stack_table_entries.patch`
    ///
    /// Name: provisional (milestone 915, 2026-10-13 UTC).
    #[test_case]
    fn every_online_core_has_its_own_three_interrupt_stack_table_entries() {
        let online = crate::smp::online_harts_mask();
        for id in (0..MAX_CPUS).filter(|id| online & (1 << id) != 0) {
            for (k, &vector) in VECTORS.iter().enumerate() {
                let entry = super::super::segments::interrupt_stack(id, k as u8 + 1);
                let (bottom, top) = span(id, k);
                assert!(entry != 0, "cpu {id}: vector {vector}'s IST entry is 0");
                assert_eq!(
                    entry,
                    top - TOP_RESERVE,
                    "cpu {id}: vector {vector}'s IST entry {entry:#x} is not its own stack's \
                     ({bottom:#x}..{top:#x})"
                );
                // SAFETY: a word inside core `id`'s own stack `k`, written once by `install`.
                let word = unsafe { (entry as *const u64).read_volatile() };
                assert_eq!(
                    word,
                    core::ptr::from_ref(crate::cpu::of(id)) as u64,
                    "cpu {id}: vector {vector}'s stack does not carry that core's per-CPU pointer"
                );
                assert!(
                    super::super::mmu::translate(bottom).is_some()
                        && super::super::mmu::translate(top - 8).is_some(),
                    "cpu {id}: vector {vector}'s stack is not mapped"
                );
            }
        }
    }

    /// **Every IST stack's guard page is a hole in the live kernel map**, so an overflow faults
    /// instead of running into the stack below it (another vector's, or another core's). Asked of
    /// every core's slots, online or not, since the map is built for all of them.
    ///
    /// Falsification: replayable `kernel/falsifications/arch.x86_64.ist.tests.every_interrupt_stack_table_guard_page_is_unmapped.patch`
    ///
    /// Name: provisional (milestone 915, 2026-10-13 UTC).
    #[test_case]
    fn every_interrupt_stack_table_guard_page_is_unmapped() {
        for id in 0..MAX_CPUS {
            for (k, &vector) in VECTORS.iter().enumerate() {
                let g = guard(id, k);
                assert!(
                    super::super::mmu::translate(g).is_none(),
                    "cpu {id}: vector {vector}'s IST guard page {g:#x} is mapped"
                );
                assert!(
                    super::super::mmu::translate(g + GUARD as u64 - 8).is_none(),
                    "cpu {id}: the top of vector {vector}'s IST guard page is mapped"
                );
            }
        }
    }
}
