//! The thread running on this core: its id, and its capability table held per core so a syscall's
//! lookup takes no global lock. See notes/thread-control-block.md ("The running thread, per core").
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The thread running on **this core** right now.
///
/// Per-CPU, and needs no lock. The read is the architecture's (`arch::read_current_thread`),
/// because it must be indivisible from a preemptible caller: a thread migrated between finding its
/// per-CPU block and reading the field would get another thread's name.
/// `smp::tests::a_migrated_kernel_thread_reads_its_own_name` is the test.
///
/// `#[inline(always)]` on riscv64 so it lands in `.text.hot` beside its IPC callers; the other two
/// inline it unforced, and forcing it grows aarch64's `syscall_entry` past its band.
#[cfg_attr(target_arch = "riscv64", inline(always))]
pub(super) fn current_thread_id() -> ThreadId {
    crate::arch::read_current_thread()
}

/// The running thread's capability table, per core (provisional name, 2026-10-04 UTC): what
/// [`current_cap`] reads instead of taking `IPC_TABLES` to find the thread.
///
/// Null before this core runs a thread. Written only by [`set_current`] and read only by the owning
/// core with interrupts masked. Its own array, one cache line an entry, because `PerCpu` is held to
/// 128 bytes and a shared line would trade the lock for line traffic. The pointer stays valid
/// because a running thread's page is never recycled. See notes/thread-control-block.md ("The
/// running thread, per core").
#[repr(align(64))]
pub(super) struct CurrentCapabilities(AtomicPtr<CapabilityTableLock>);

pub(super) static CURRENT_CAPABILITIES: [CurrentCapabilities; cpu::MAX_CPUS] =
    [const { CurrentCapabilities(AtomicPtr::new(core::ptr::null_mut())) }; cpu::MAX_CPUS];

/// Make `tid` this core's running thread: its name in `PerCpu::current` and its capability table in
/// [`CURRENT_CAPABILITIES`]. The only writer of either, so a lookup never finds one thread's name
/// beside another's table. Forced inline on riscv64 and `x86_64` and given a `.text.hot` section on
/// aarch64, so it lands beside its IPC callers whatever LLVM decides.
///
/// Context: caller holds `IPC_TABLES`.
///
/// # Safety
///
/// `tcb` is `tid`'s page pointer as the thread table stores it (`Threads::pointer`), live.
#[cfg_attr(any(target_arch = "riscv64", target_arch = "x86_64"), inline(always))]
#[cfg_attr(
    all(target_os = "none", target_arch = "aarch64"),
    unsafe(link_section = ".text.hot.sched.set_current")
)]
pub(super) unsafe fn set_current(tid: ThreadId, tcb: *mut Thread) {
    cpu::current().current.store(tid, Ordering::Relaxed);
    // SAFETY: the caller's contract; an address computation, nothing is read.
    let table = unsafe { crate::thread::capability_table_of(tcb) };
    if let Some(entry) = CURRENT_CAPABILITIES.get(cpu::id()) {
        entry.0.store(table.cast_mut(), Ordering::Relaxed);
    }
}

/// **The running thread's capability table, locked**, with `IPC_TABLES` not held. `None` before
/// this core runs a thread. See [`CURRENT_CAPABILITIES`] for why the pointer is valid, and
/// [`crate::sync::lock_found`] for why the entry is read with interrupts masked.
#[inline]
pub(super) fn current_capabilities()
-> Option<crate::sync::IrqSafeGuard<'static, crate::cap::CapabilityTable>> {
    crate::sync::lock_found(|| {
        let table = CURRENT_CAPABILITIES
            .get(cpu::id())?
            .0
            .load(Ordering::Relaxed);
        // SAFETY: non-null means the table of the thread running on this core, which is the
        // caller, and a running thread's page is not recycled (see `CURRENT_CAPABILITIES`). The
        // `'static` is a lie told only as long as the guard, which the caller drops before it can
        // stop being the running thread (no caller blocks or switches holding it).
        unsafe { table.as_ref() }
    })
}
