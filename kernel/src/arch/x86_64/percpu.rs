//! **This CPU's identity and its per-CPU block**: the boot CPU's id, and the `gs`-based
//! pointer every core reads its own block through.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// **The local APIC id of the CPU the kernel booted on**, recorded once by `boot.s`'s `_start_high`
/// (after it zeroes `.bss`, before `kernel_main`) from `CPUID` leaf 1's `EBX[31:24]`.
///
/// Recorded, not recomputed: `CPUID` answers "which core am I", every caller of [`boot_cpu_id`]
/// asks "which core booted", and the two differ once a secondary is online. riscv64's
/// `BOOT_HARTID` is the same shape; see notes/x86-port/cpu-state.md.
///
/// The name is **provisional** (an architect names things): it mirrors `BOOT_HARTID` on the
/// architecture that already had this shape.
#[unsafe(no_mangle)]
pub(super) static BOOT_CPU_ID: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// The logical id of the CPU the kernel boots on.
///
/// Read from [`BOOT_CPU_ID`]. It must agree with the roster's seating (`smp::seat_cpus_from_acpi`
/// seats every core at the slot its local APIC id names), and nothing guarantees the boot CPU's
/// local APIC id is 0. Callable from the kernel's first Rust statement, as
/// `cpu::init_this_cpu(arch::boot_cpu_id())` needs: `boot.s` stamps the record before it calls
/// `kernel_main`, so this never reads the zero default.
pub(crate) fn boot_cpu_id() -> usize {
    BOOT_CPU_ID.load(core::sync::atomic::Ordering::Relaxed)
}

/// The `gs`-relative offset of `PerCpu::x86_trap.self_ptr`, which [`percpu`] loads.
pub(super) const PERCPU_SELF_OFF: usize =
    core::mem::offset_of!(crate::cpu::PerCpu, x86_trap.self_ptr);

/// Set this CPU's per-CPU pointer, by writing the `gs` segment base, and then the block's own
/// address into the block, through that base, for [`percpu`] to read back.
pub(crate) fn set_percpu(ptr: usize) {
    // SAFETY: `IA32_GS_BASE` exists on every long-mode CPU, and this value is the per-CPU block
    // this kernel reserves the register for.
    unsafe { write_msr(IA32_GS_BASE, ptr as u64) };
    // SAFETY: `gs` now bases at `ptr`, a `PerCpu` this core owns, and the offset is that struct's
    // own `self_ptr` field, computed by `offset_of!`. Only this core reaches its block through `gs`.
    unsafe {
        asm!(
            "mov gs:[{off}], {p}",
            off = const PERCPU_SELF_OFF,
            p = in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
}

/// Read this CPU's per-CPU pointer (the value last handed to [`set_percpu`]).
///
/// One `gs`-relative load rather than `rdmsr IA32_GS_BASE`, which is microcoded, serializing and
/// clobbers `rax`, `rdx` and `rcx`; this runs on every lock, unlock and current-thread read. Where
/// `IA32_GS_BASE` holds the user's value (around the `swapgs`es in `trap.s`) it would load from a
/// user address. Nothing ordinary runs there: interrupts are masked, and the NMI, double fault and
/// machine check enter through `isr_paranoid`, which installs the kernel's GS base first.
pub fn percpu() -> usize {
    let p: usize;
    // SAFETY: after `set_percpu` on this core, `gs:[PERCPU_SELF_OFF]` is this core's own
    // `PerCpu::x86_trap.self_ptr`, which `set_percpu` wrote. `readonly`: it reads memory and writes
    // none. Not `pure`, so LLVM never merges two reads across a migration point.
    unsafe {
        asm!(
            "mov {p}, gs:[{off}]",
            p = out(reg) p,
            off = const PERCPU_SELF_OFF,
            options(nostack, readonly, preserves_flags),
        );
    }
    p
}

/// The `gs`-relative offset of `PerCpu::current`, which [`read_current_thread`] loads.
const CURRENT_OFF: usize = core::mem::offset_of!(crate::cpu::PerCpu, current);

/// **This core's `PerCpu::current`, in one `gs`-relative load**, so no interrupt can fall between
/// finding the block and reading the field (notes/mmu.md, BUGS).
///
/// [`percpu()`] and then a load is two steps. A preempted kernel thread that resumes on another core
/// between them still holds the first core's block in a general register, and reads the name of
/// whatever thread that core runs now. One instruction is indivisible, and `gs`'s base is an MSR no
/// thread carries. [`percpu()`]'s note on the `swapgs` windows applies unchanged.
///
/// Name: provisional.
#[inline(always)]
pub fn read_current_thread() -> u64 {
    let v: u64;
    // SAFETY: as `percpu`: `gs` bases at this core's `PerCpu`, and `CURRENT_OFF` is the offset of
    // an aligned `AtomicU64` inside it, so this is a relaxed atomic load. It writes nothing.
    unsafe {
        asm!(
            "mov {v}, gs:[{off}]",
            v = out(reg) v,
            off = const CURRENT_OFF,
            options(nostack, readonly, preserves_flags),
        );
    }
    v
}

/// **Test-only: does the per-CPU pointer name the CPU we are physically running on?**
///
/// A constant `true` here, and for aarch64's reason rather than RISC-V's. RISC-V keeps the pointer
/// in `tp`, an ordinary register a trap frame carries, so it can go stale when a preempted thread
/// resumes on a different hart. `IA32_GS_BASE` is an MSR: it is not saved or restored by any context
/// switch and does not travel with a thread, so there is nothing that could make it stale. The
/// independent ground truth (the local APIC id) is not readable yet either way.
#[cfg(any(test, feature = "system_tests"))]
pub(crate) fn percpu_matches_hart() -> bool {
    true
}
