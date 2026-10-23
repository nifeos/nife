//! This hart's identity and its per-CPU block: the trap stash `sscratch` points at, the boot hart's
//! id, and the `tp`-based per-CPU pointer. See notes/riscv-port.md, the per-hart `tp` hazard.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The per-hart trap stash `sscratch` points at, from which `trap.s` recovers the kernel `tp` and
/// stack after a U-mode trap arrives with the user's `tp`.
///
/// `trap.s` reads the fields at fixed offsets 0, 8, 16 and 24, checked below. Each hart touches
/// only its own entry, so the atomics give interior mutability, not cross-core synchronization.
#[repr(C)]
pub(super) struct TrapStash {
    /// The current thread's kernel-stack top; where a U-mode trap lands. Set on every return to
    /// U-mode (trap.s `trap_return`), so it always names the thread about to run in U-mode.
    kernel_sp: AtomicUsize,
    /// This hart's `PerCpu` pointer: the kernel `tp` the trap entry restores.
    percpu: AtomicUsize,
    /// Two scratch words the trap entry uses to free registers before it has a stack.
    scratch0: AtomicUsize,
    scratch1: AtomicUsize,
}

impl TrapStash {
    const fn new() -> Self {
        Self {
            kernel_sp: AtomicUsize::new(0),
            percpu: AtomicUsize::new(0),
            scratch0: AtomicUsize::new(0),
            scratch1: AtomicUsize::new(0),
        }
    }
}

// trap.s hardcodes these offsets; keep them honest.
const _: () = {
    assert!(core::mem::offset_of!(TrapStash, kernel_sp) == 0);
    assert!(core::mem::offset_of!(TrapStash, percpu) == 8);
    assert!(core::mem::offset_of!(TrapStash, scratch0) == 16);
    assert!(core::mem::offset_of!(TrapStash, scratch1) == 24);
};

/// One trap stash per hart, indexed like `cpu::PERCPU`; static, so the first trap needs no
/// allocator.
pub(super) static TRAP_STASH: [TrapStash; crate::cpu::MAX_CPUS] =
    [const { TrapStash::new() }; crate::cpu::MAX_CPUS];

/// The physical hart id OpenSBI booted the kernel on (`a0` at `_start`), stashed by boot.s. Not
/// necessarily 0, so the SMP bring-up reads it.
#[unsafe(no_mangle)]
pub(super) static BOOT_HARTID: AtomicUsize = AtomicUsize::new(0);

/// The physical hart id the kernel booted on (see [`BOOT_HARTID`]).
pub fn boot_hartid() -> usize {
    BOOT_HARTID.load(Ordering::Relaxed)
}

/// The logical id of the boot hart, which on RISC-V is its physical hart id. The SMP bring-up
/// starts every other hart.
pub(crate) fn boot_cpu_id() -> usize {
    boot_hartid()
}

/// Set this hart's per-CPU pointer to `ptr` in `tp`, record it in this hart's [`TrapStash`], and
/// point `sscratch` at the stash, so `trap.s` can recover it after a U-mode trap.
pub(crate) fn set_percpu(ptr: usize) {
    // Set `tp` first so `cpu::id()` (which reads `tp`) resolves this hart's index into TRAP_STASH.
    // SAFETY: `ptr` is this hart's `PerCpu`, which is what `tp` must name.
    unsafe { instructions::write_tp(ptr) };
    let stash = &TRAP_STASH[crate::cpu::id()];
    stash.percpu.store(ptr, Ordering::Relaxed);
    let stash_ptr = stash as *const TrapStash as usize;
    // SAFETY: `sscratch` now names this hart's stash; trap.s reads it as `&TrapStash` on every trap.
    unsafe { instructions::write_sscratch(stash_ptr) };
}

/// Read this hart's per-CPU pointer (the value last handed to [`set_percpu`]).
pub fn percpu() -> usize {
    instructions::read_tp()
}

/// The offset of `PerCpu::current`, which [`read_current_thread`] loads from `tp`.
const CURRENT_OFF: usize = core::mem::offset_of!(crate::cpu::PerCpu, current);
const _: () = assert!(
    CURRENT_OFF < 2048,
    "PerCpu::current must sit within `ld`'s signed 12-bit offset from tp"
);

/// This hart's `PerCpu::current`, in one `ld` from `tp`, so a migration cannot fall between finding
/// the block and reading the field (notes/mmu.md, BUGS).
///
/// `tp` is right at every S-mode instruction: `switch_to` never touches it, and `trap.s` returns to
/// S-mode with the live hart's `tp`.
///
/// Name: provisional.
#[inline(always)]
pub fn read_current_thread() -> u64 {
    let v: u64;
    // SAFETY: in S-mode `tp` is this hart's `PerCpu` (see above), and `CURRENT_OFF` is the offset of
    // an aligned `AtomicU64` inside it, so this is a relaxed atomic load. It writes nothing.
    unsafe {
        core::arch::asm!(
            "ld {v}, {off}(tp)",
            v = out(reg) v,
            off = const CURRENT_OFF,
            options(nostack, readonly, preserves_flags),
        );
    }
    v
}

/// Whether `tp` names the hart this runs on, checked against `sscratch`, which never migrates. Test
/// builds only.
///
/// A mismatch is a thread carrying its origin hart's `tp` (DECISIONS §28 (SMP placement)). Both
/// reads run masked, so a preemption cannot split them across harts.
#[cfg(any(test, feature = "system_tests"))]
pub(crate) fn percpu_matches_hart() -> bool {
    let was_enabled = crate::arch::interrupts::disable();
    // `sscratch` holds `&TRAP_STASH[hart]` in S-mode (trap.s keeps it so).
    let sscratch = instructions::read_sscratch();
    // Under the same mask, so both reads name one hart.
    let hart_from_tp = crate::cpu::id();
    crate::arch::interrupts::restore(was_enabled);
    let base = TRAP_STASH.as_ptr() as usize;
    let hart_from_sscratch = (sscratch - base) / core::mem::size_of::<TrapStash>();
    hart_from_sscratch == hart_from_tp
}
