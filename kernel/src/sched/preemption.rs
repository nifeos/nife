//! **Preemption**: the tick that records a switch is wanted, the deferred half that takes it, and
//! the counts that say preemption is real.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// How many times we have actually taken the CPU away from a thread. The number that says
/// preemption is real.
pub(super) static PREEMPTIONS: AtomicU64 = AtomicU64::new(0);

/// The same count, **per core**, which is the question the global one cannot answer.
///
/// **Its own array rather than a `cpu::PerCpu` field, and that is a measurement rather than a
/// preference.** `PerCpu` is exactly 128 bytes, so `PERCPU[id]` indexes with a shift; one more
/// `AtomicU64` field took it to 136 and cost 150 bytes on the riscv64 `ipc_send_receive` closure,
/// over `script/fastpath-footprint`'s bound. See the assertion beside `cpu::PERCPU`. Nothing on
/// the IPC fastpath reads this, so it has no business sharing a cache line budget with what does.
///
/// Written only by the owning core, in [`count_preemption`], which runs on the timer preemption
/// path and not on any IPC path. Read by anyone.
pub(super) static PREEMPTIONS_PER_CPU: [AtomicU64; cpu::MAX_CPUS] =
    [const { AtomicU64::new(0) }; cpu::MAX_CPUS];

/// Called from the timer IRQ. **Records** that a switch is wanted; does not switch.
///
/// **`#[inline(never)]`, and the reason is a measurement rather than a preference.** On riscv64 the
/// timer interrupt and a syscall arrive through the same `riscv_trap_body`, so anything inlined
/// here lands in a symbol `script/fastpath-footprint` counts **flat**: its bytes are charged to
/// every syscall although no syscall fetches them, which is the over-count milestone 368 (the entry set is flat, so an inlining flip can move 12% into it) records.
/// Keeping this a call rather than an inline puts the tick path's bytes in the tick path's own
/// symbol, where they belong and where the gate can see them for what they are. One `jal` per tick
/// per core, at 100 Hz, against bytes on the line every syscall shares.
#[inline(never)]
pub fn on_tick() {
    cpu::current().need_resched.store(true, Ordering::Relaxed);
    // **Charge this tick to whatever is on this CPU** (milestone 282 (a thread's CPU time, and the `top` it makes possible), DECISIONS §150 (how does a thread's CPU time reach userspace?)). One
    // bounds-checked index and one relaxed increment, which is the whole of the accounting; see
    // [`CPU_TICKS`] for why the counter is an array beside the table rather than a field in it.
    charge_tick();
    // **And the machine's own view of the same tick** (milestone 126, DECISIONS §225): busy or
    // idle, and how many threads were waiting, for `vmstat` and `top`'s summary.
    count_tick();
    // The corruption tripwire, when armed (the board tour's initial ramdisk demo window). One
    // relaxed load when it is not, which is every other tick everywhere. IRQ context is safe for
    // its println: the console's IrqSafeMutex masks interrupts while held, so the interrupted
    // context on this core cannot be mid-print (the irq_notify argument, one lock over).
    // The answer is ignored on purpose: a sampling instrument may skip a beat when another
    // core's pass (or an arm) holds the gate, and the tick must never spin in IRQ context.
    let _ = canary::check();

    // **Timer expiry** (milestone 106, DECISIONS §147): one relaxed load, one counter read and one
    // compare when nothing is due, which is the cost `notes/timed-wait.md` priced before this was
    // built. Only a due tick takes `IPC_TABLES`, so four cores ticking at 100 Hz do not contend for
    // the whole-machine lock to find out that nothing happened.
    if inter_process_communication::timer::is_due(
        EARLIEST_DEADLINE.load(Ordering::Relaxed),
        crate::arch::timer::now(),
    ) {
        expire_timers();
    }

    // **The soak's cross-core hook** (milestone 221, DECISIONS 138's option D). A saturated
    // workload never migrates, because a rendezvous wake is local (§28.2), `wake_load_aware` is
    // reachable only from a device interrupt, and a work steal needs an idle core the machine does
    // not have. The timer is the one event that workload cannot starve, and this is the one
    // architecture-neutral place all three dispatchers already reach in real interrupt context, so
    // a soak build signals a rendezvous from here and its waiters take the whole real wake path
    // down through `irq_notify`. It compiles to nothing anywhere else; see kernel/src/soak.rs.
    #[cfg(feature = "soak_test")]
    crate::soak::signal_waiters();

    // **A kernel line held for the log service, signalled from a context that holds no lock**
    // (milestone 342 (the kernel and the `console` server drive one UART from two address
    // spaces)). One relaxed load when nothing is held, which is almost every tick. The print that
    // held it could not signal: it may have been printing under `IPC_TABLES`. See `kernel_log`.
    crate::kernel_log::signal_if_safe();
    #[cfg(feature = "console_flood")]
    crate::kernel_log::flood_tick();
}

/// The machine statistics page's half of a tick, out of line because every architecture's
/// exception dispatcher is in `script/fastpath-footprint`'s flat `syscall_entry` set and a tick is
/// not a syscall. Lock-free, like `charge_tick`: the idle tid and the run-queue length are this
/// core's own relaxed mirrors.
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

/// **The deferred half of preemption**: switch away if this core's tick asked for it.
///
/// Called by both ISAs' trap dispatchers *after* the handler has returned to the interrupted
/// thread's own stack, which is the whole reason it is a function rather than four lines at the
/// bottom of `handle_irq` where it lived until milestone 124. The handler may have run on this
/// core's interrupt stack, and `schedule()` may only be called on a stack the interrupted thread
/// owns; see `kernel/src/interrupt_stack.rs`.
///
/// Portable, and identical on both architectures, which is the other half of why it moved: the four
/// lines were written twice and had drifted in their comments already.
///
/// **`#[inline(never)]`, for [`on_tick`]'s reason.** Every trap dispatcher ends in this call, so an
/// inlined copy lands in the dispatcher, a symbol `script/fastpath-footprint` counts flat, and
/// charges every syscall for the `schedule()` call site no syscall fetches (milestone 368 (the
/// entry set is flat, so an inlining flip can move 12% into it)). Left to LLVM it inlined into
/// aarch64's `exception_dispatch` once the self-test of milestone 890 (riscv64 and `x86_64` join
/// the shared boot path) moved the codegen units, measured 2026-10-20 (UTC): 0x98 to 0x11c bytes,
/// and `syscall_entry` 1696 to 1828, 7.8% over a 5% band. One branch per trap keeps the bytes where
/// they belong.
#[inline(never)]
pub fn preempt_if_needed() {
    if take_need_resched() && is_running() {
        count_preemption();
        schedule();
    }
}
