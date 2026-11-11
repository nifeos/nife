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
/// over `script/fastpath-footprint`'s bound (the assertion is beside `cpu::PERCPU`), and nothing on
/// the IPC fastpath reads this. Written only by the owning core, in [`count_preemption`], on the
/// timer path; read by anyone.
pub(super) static PREEMPTIONS_PER_CPU: [AtomicU64; cpu::MAXIMUM_CPUS] =
    [const { AtomicU64::new(0) }; cpu::MAXIMUM_CPUS];

/// How many ticks each core remembers in [`sample_tick`]'s ring.
#[cfg(feature = "system_tests")]
const TICK_SAMPLE_DEPTH: usize = 8;

/// One core's last [`TICK_SAMPLE_DEPTH`] ticks: the thread each one interrupted and the `pc` it
/// interrupted it at. One writer, the owning core's timer interrupt; read racily by the dump.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane fuzz-budget-waits.
#[cfg(feature = "system_tests")]
struct TickSamples {
    seq: AtomicU64,
    tid: [AtomicU64; TICK_SAMPLE_DEPTH],
    pc: [AtomicU64; TICK_SAMPLE_DEPTH],
}

#[cfg(feature = "system_tests")]
#[allow(clippy::declare_interior_mutable_const)]
const NO_TICK_SAMPLES: TickSamples = TickSamples {
    seq: AtomicU64::new(0),
    tid: [const { AtomicU64::new(0) }; TICK_SAMPLE_DEPTH],
    pc: [const { AtomicU64::new(0) }; TICK_SAMPLE_DEPTH],
};

#[cfg(feature = "system_tests")]
static TICK_SAMPLES: [TickSamples; cpu::MAXIMUM_CPUS] = [NO_TICK_SAMPLES; cpu::MAXIMUM_CPUS];

/// **Remember where this core's tick found its thread**: the thread running here
/// and the `pc` the timer interrupt interrupted, kernel or user, for the thread dump's
/// `ticks interrupted` line.
///
/// A budget-expired wait whose threads are runnable and taking turns on a core shows only switch
/// events, and a kernel thread's dump `pc` is its user frame, which it does not have. These samples
/// say where such a thread was each time the tick came: one place every time is a thread spinning
/// there, and many places is a thread making slow progress. System tests only: the fuzz drivers'
/// budgets are the reader, and every other build keeps the tick as it was.
///
/// Context: interrupt context, from the timer interrupt, on the owning core only.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane fuzz-budget-waits.
#[cfg(feature = "system_tests")]
#[inline(never)]
pub fn sample_tick(interrupted_pc: u64) {
    let ring = &TICK_SAMPLES[cpu::id()];
    let slot = (ring.seq.fetch_add(1, Ordering::Relaxed) as usize) % TICK_SAMPLE_DEPTH;
    ring.tid[slot].store(current_thread_id(), Ordering::Relaxed);
    ring.pc[slot].store(interrupted_pc, Ordering::Relaxed);
}

/// [`sample_tick`]'s twin for every build that is not a system test: it records nothing.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane fuzz-budget-waits.
#[cfg(not(feature = "system_tests"))]
#[inline(always)]
pub fn sample_tick(_interrupted_pc: u64) {}

/// Print core `cpu`'s last ticks, oldest first, as `tid@pc`. Racy against that core's own writes,
/// as the event ring is.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane fuzz-budget-waits.
#[cfg(feature = "system_tests")]
pub(super) fn dump_tick_samples(c: usize) {
    let ring = &TICK_SAMPLES[c];
    let seq = ring.seq.load(Ordering::Relaxed);
    if seq == 0 {
        return;
    }
    crate::print!("    core {c} ticks interrupted:");
    for s in seq.saturating_sub(TICK_SAMPLE_DEPTH as u64)..seq {
        let slot = (s as usize) % TICK_SAMPLE_DEPTH;
        crate::print!(
            " {:#x}@{:#x}",
            ring.tid[slot].load(Ordering::Relaxed),
            ring.pc[slot].load(Ordering::Relaxed),
        );
    }
    crate::println!();
}

/// Record that this core wants a switch, and run the tick's other accounting; does not switch.
///
/// `#[inline(never)]`: riscv64's timer interrupt and syscalls share `riscv_trap_body`, which
/// `script/fastpath-footprint` counts flat, so inlined bytes here would be charged to every
/// syscall (notes/benchmarks/fastpath-footprint-gate.md).
///
/// Context: interrupt context, from the timer interrupt. Takes `IPC_TABLES` only when a timer is
/// due.
#[inline(never)]
pub fn on_tick() {
    cpu::current().need_resched.store(true, Ordering::Relaxed);
    // Charge this tick to whatever is on this CPU (§150 (how does a thread's CPU time reach
    // userspace)); see [`CPU_TICKS`].
    charge_tick();
    // The machine's own view of the tick, for `vmstat` and `top`'s summary (§225 (`free` sees the
    // machine and your share)).
    count_tick();
    // The corruption tripwire, one relaxed load when unarmed. Its println is safe here because the
    // console's `InterruptSafeMutex` masks interrupts; its answer is ignored, because a sampling
    // instrument may skip a beat and the tick must never spin in interrupt context.
    let _ = canary::check();

    // Timer expiry (§147 (a timer a userspace service cannot hold)): only a due tick takes
    // `IPC_TABLES`, so idle ticks on every core do not contend for it (notes/timed-wait.md priced
    // this check).
    if inter_process_communication::timer::is_due(
        EARLIEST_DEADLINE.load(Ordering::Relaxed),
        crate::arch::timer::now(),
    ) {
        expire_timers();
    }

    // A soak build signals a rendezvous from the tick, the one event a saturated workload cannot
    // starve, so its waiters cross cores through `interrupt_notify` (§138 (how a saturated
    // workload is made to hand threads across cores) option D; kernel/src/soak.rs).
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
/// `#[inline(never)]` for [`on_tick`]'s reason: inlined into aarch64's `exception_dispatch` on
/// 2026-10-20 (UTC), after milestone 890 (riscv64 and `x86_64` join the shared boot path), it took
/// that symbol from 0x98 to 0x11c bytes and `syscall_entry` from 1,696 to 1,828 B, 7.8% over a 5%
/// band (notes/benchmarks/fastpath-footprint-gate.md).
#[inline(never)]
pub fn preempt_if_needed() {
    if take_need_resched() && is_running() {
        count_preemption();
        schedule();
    }
}
