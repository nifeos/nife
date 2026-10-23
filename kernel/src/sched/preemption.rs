//! Preemption: the tick that records a switch is wanted, the deferred half that takes it, and the
//! counts that say preemption is real. See notes/threads.md ("Preemption is four lines").
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// How many times a thread has had the CPU taken away: the number that says preemption is real.
pub(super) static PREEMPTIONS: AtomicU64 = AtomicU64::new(0);

/// The same count, per core, which the global one cannot answer.
///
/// Its own array rather than a `cpu::PerCpu` field: `PerCpu` is held to 128 bytes so `PERCPU[id]`
/// indexes with a shift, and one more field cost riscv64's `ipc_send_receive` closure 150 bytes,
/// over `script/fastpath-footprint`'s bound (the assertion is beside `cpu::PERCPU`). Written only by
/// the owning core, in [`count_preemption`], on the timer path; read by anyone.
pub(super) static PREEMPTIONS_PER_CPU: [AtomicU64; cpu::MAX_CPUS] =
    [const { AtomicU64::new(0) }; cpu::MAX_CPUS];

/// Record that this core wants a switch, and run the tick's other accounting; does not switch.
///
/// `#[inline(never)]`: riscv64's timer interrupt and syscalls share `riscv_trap_body`, which
/// `script/fastpath-footprint` counts flat, so inlined bytes here would be charged to every
/// syscall (notes/benchmarks/fastpath-footprint-gate.md).
///
/// Context: interrupt context, from the timer IRQ. Takes `IPC_TABLES` only when a timer is due.
#[inline(never)]
pub fn on_tick() {
    cpu::current().need_resched.store(true, Ordering::Relaxed);
    // Charge this tick to whatever is on this CPU (§150); see [`CPU_TICKS`].
    charge_tick();
    // The machine's own view of the tick, for `vmstat` and `top`'s summary (§225).
    count_tick();
    // The corruption tripwire, one relaxed load when unarmed. Its println is safe here because the
    // console's `IrqSafeMutex` masks interrupts; its answer is ignored, because a sampling
    // instrument may skip a beat and the tick must never spin in interrupt context.
    let _ = canary::check();

    // Timer expiry (§147): only a due tick takes `IPC_TABLES`, so idle ticks on every core do not
    // contend for it (notes/timed-wait.md priced this check).
    if inter_process_communication::timer::is_due(
        EARLIEST_DEADLINE.load(Ordering::Relaxed),
        crate::arch::timer::now(),
    ) {
        expire_timers();
    }

    // A soak build signals a rendezvous from the tick, the one event a saturated workload cannot
    // starve, so its waiters cross cores through `irq_notify` (§138 option D; kernel/src/soak.rs).
    #[cfg(feature = "soak_test")]
    crate::soak::signal_waiters();

    // A kernel line held for the log service is signalled here, where no lock is held: the print
    // that held it may have been under `IPC_TABLES` (`kernel_log`).
    crate::kernel_log::signal_if_safe();
    crate::kernel_log::stall_tick();
    #[cfg(feature = "console_flood")]
    crate::kernel_log::flood_tick();
}

/// The machine statistics page's half of a tick: lock-free, from this core's own relaxed mirrors.
/// Out of line, because every exception dispatcher is in `script/fastpath-footprint`'s flat set.
#[inline(never)]
pub(super) fn count_tick() {
    let here = cpu::current();
    let idle = current_thread_id() == here.idle.load(Ordering::Relaxed);
    crate::machine_statistics::tick(
        here.switches.load(Ordering::Relaxed),
        idle,
        here.runnable() as u64 + u64::from(!idle),
    );
}

pub(crate) fn take_need_resched() -> bool {
    cpu::current().need_resched.swap(false, Ordering::Relaxed)
}

pub(crate) fn count_preemption() {
    PREEMPTIONS.fetch_add(1, Ordering::Relaxed);
    PREEMPTIONS_PER_CPU[cpu::id()].fetch_add(1, Ordering::Relaxed);
}

/// The deferred half of preemption: switch away if this core's tick asked for it.
///
/// Called by every trap dispatcher after the handler has returned to the interrupted thread's own
/// stack, because the handler may have run on this core's interrupt stack and `schedule()` runs
/// only on a stack the interrupted thread owns (`kernel/src/interrupt_stack.rs`).
///
/// `#[inline(never)]` for [`on_tick`]'s reason: an inlined copy would charge every syscall for a
/// `schedule()` call site no syscall fetches (notes/benchmarks/fastpath-footprint-gate.md).
#[inline(never)]
pub fn preempt_if_needed() {
    if take_need_resched() && is_running() {
        count_preemption();
        schedule();
    }
}
