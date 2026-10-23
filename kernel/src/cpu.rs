//! Per-CPU state: each core's private block, reached in one instruction.
//!
//! The architecture keeps a pointer to this core's [`PerCpu`] in a register
//! ([`crate::arch::set_percpu`]). Only the owning core touches its block, with interrupts masked;
//! the inbox and the steal request are the parts other cores reach. See notes/per-cpu.md and
//! DECISIONS §11 (per-CPU run queues).

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use intrusive_fifo::Fifo;

use crate::sync::{IrqSafeMutex, rank};
use crate::thread::{Thread, ThreadId};

/// A `current`/`idle` slot holding no thread. Tids are small integers from 0 up, so `u64::MAX`
/// can never collide with a real one.
pub const NO_TID: ThreadId = u64::MAX;

/// The number of per-CPU seats: one past the highest core id the kernel hands out.
///
/// A bound on hart ids, not core counts, since each core is seated at the slot its id names. One
/// name for `current_cpu_contract::CPU_ID_BOUND`, which userspace shares. Each seat costs a block
/// and a 68 KiB secondary stack slot whether or not a core fills it. See notes/per-cpu.md.
pub const MAX_CPUS: usize = current_cpu_contract::CPU_ID_BOUND;

/// One core's private data.
///
/// Each block starts on its own [`FALSE_SHARING_SPAN`] on every architecture, and its size stays a
/// power of two; the assertions below [`PERCPU`] and [`FALSE_SHARING_SPAN`] hold both. 256 on
/// `x86_64`, where the struct is 160 bytes. See notes/per-cpu.md.
#[cfg_attr(target_arch = "x86_64", repr(align(256)))]
#[cfg_attr(not(target_arch = "x86_64"), repr(align(128)))]
pub struct PerCpu {
    /// The lowest lock rank this core holds (`rank::NONE` for none). Touched only by this core with
    /// interrupts masked. See [`crate::sync`].
    pub held_rank: AtomicU32,

    /// The thread running on this core (`NO_TID` before the core schedules). Written only by this
    /// core; atomic because a remote core may read it (the reaper asks whether a thread is current
    /// anywhere).
    pub current: AtomicU64,

    /// This core's idle thread (`NO_TID` before it exists): what runs when this core's run queue
    /// is empty. Each core gets its own, so an idle core parks in its *own* `wfi`.
    pub idle: AtomicU64,

    /// Set by this core's timer tick, read on this core's return from the IRQ, so one core's tick
    /// cannot make another reschedule (DECISIONS §9 (locking)'s record-and-defer).
    pub need_resched: AtomicBool,

    /// The thread this core just switched away from, finished by the thread it switched to
    /// (`NO_TID` between switches). See `sched::finish_switch`.
    ///
    /// A `Finished` predecessor is reaped, and a wake that caught a `Blocked` one mid-switch-out
    /// (`thread_wake_handshake::Handshake`'s `wake_pending`) is completed, only after the switch,
    /// when the thread is provably off its stack. Only this core touches it, with interrupts masked.
    pub switched_from: AtomicU64,

    /// This core's run queue: the threads ready to run here, in round-robin order.
    ///
    /// Intrusive (notes/intrusive-queues.md): a push neither allocates nor fails, and entries are
    /// TCB pointers kept alive because a queued thread is `Ready` and only `Finished` threads are
    /// reaped. No lock: only this core touches it, with interrupts masked, which is what makes the
    /// `UnsafeCell` sound. Access it through [`with_runq`](Self::with_runq).
    runq: UnsafeCell<Fifo<Thread>>,

    /// This core's migration inbox: threads handed to it by other cores.
    ///
    /// The one cross-core scheduler structure. A remote core locks it, pushes the TCB and sends a
    /// reschedule SGI; this core drains it in the handler. See [`inbox_of`] and `sched::drain_inbox`.
    pub inbox: IrqSafeMutex<Fifo<Thread>>,

    /// This core's run-queue length, mirrored so other cores may read it (§28 (SMP placement)). Kept exact
    /// by the owner after every [`with_runq`](Self::with_runq); read relaxed and possibly stale, on
    /// purpose. The thread on the CPU is not counted.
    runq_len: AtomicUsize,

    /// This core's inbox length, mirrored the same way and updated under the inbox lock. Counted in
    /// [`runnable`](Self::runnable) so a core with a full inbox and an empty run queue does not read
    /// as idle to placers and thieves.
    inbox_len: AtomicUsize,

    /// A pending work-steal request from an idle core (§28): this core hands one runnable thread to
    /// the requester's inbox at its next scheduler entry. One request at a time per victim.
    ///
    /// The one lock-free cross-core protocol in the scheduler. The field is `work_steal_slot`'s
    /// type, which loom checks on the host (notes/interleaving.md).
    pub steal_request: work_steal_slot::Slot,

    /// How many context switches this core has made since boot: `vmstat`'s `cs` column.
    ///
    /// Incremented by `sched::schedule` on every switch and copied to the machine statistics page
    /// by this core's tick (`sched::count_tick`), so the page lags by up to one tick. Written only
    /// by this core, with interrupts masked.
    pub switches: AtomicU64,

    /// This core's placement PRNG for §28's two choices: xorshift32, seeded in [`init_this_cpu`].
    /// Advanced by [`rng_next`](Self::rng_next) through `cpu::current()`. Deterministic, so the
    /// icount instrument is reproducible.
    rng: AtomicU32,

    /// `x86_64` only: this core's trap-entry scratch, which `trap.s` reaches `gs`-relative at
    /// offsets from `core::mem::offset_of!`. `IA32_GS_BASE` already names this block, so no second
    /// per-CPU array is needed. Only this core touches it, so plain `u64`s in `UnsafeCell`s.
    #[cfg(target_arch = "x86_64")]
    pub x86_trap: X86TrapPerCpu,
}

/// See [`PerCpu::x86_trap`].
#[cfg(target_arch = "x86_64")]
pub(crate) struct X86TrapPerCpu {
    /// The address of this core's `TSS.rsp0` field, written once by `segments::init`.
    /// `isr_restore` writes the outgoing thread's kernel-stack top through it on every return to
    /// ring 3.
    pub tss_rsp0_ptr: UnsafeCell<u64>,
    /// The `syscall` path's kernel stack top, loaded by `x86_syscall_entry`. `isr_restore` keeps
    /// it in step with `TSS.rsp0` on every return to ring 3.
    pub syscall_kernel_rsp: UnsafeCell<u64>,
    /// The interrupted user `rsp`, parked by `x86_syscall_entry` between its `swapgs` and the push
    /// into the trap frame, because `syscall` does not switch stacks.
    pub syscall_user_rsp: UnsafeCell<u64>,
    /// This block's own address, written by `arch::x86_64::set_percpu`, so `arch::x86_64::percpu`
    /// is one `gs`-relative load instead of an `rdmsr` (Linux's `this_cpu_off`).
    pub self_ptr: UnsafeCell<u64>,
}

// SAFETY: as `PerCpu` itself: only the owning core ever touches its own instance.
#[cfg(target_arch = "x86_64")]
unsafe impl Sync for X86TrapPerCpu {}

#[cfg(target_arch = "x86_64")]
impl X86TrapPerCpu {
    const fn new() -> Self {
        Self {
            tss_rsp0_ptr: UnsafeCell::new(0),
            syscall_kernel_rsp: UnsafeCell::new(0),
            syscall_user_rsp: UnsafeCell::new(0),
            self_ptr: UnsafeCell::new(0),
        }
    }
}

// SAFETY: the only non-`Sync` field is `runq`, and the whole contract of this type is that a
// `PerCpu` block is touched only by its owning core, with interrupts masked (see `with_runq`). No
// two cores ever reach the same block, so there is no cross-core data race to guard against, and
// the atomics handle this core's own interrupt-vs-mainline reentrancy on the fields that need it.
unsafe impl Sync for PerCpu {}

impl PerCpu {
    const fn new() -> Self {
        Self {
            held_rank: AtomicU32::new(rank::NONE),
            current: AtomicU64::new(NO_TID),
            idle: AtomicU64::new(NO_TID),
            need_resched: AtomicBool::new(false),
            switched_from: AtomicU64::new(NO_TID),
            runq: UnsafeCell::new(Fifo::new()),
            inbox: IrqSafeMutex::new(rank::INBOX, Fifo::new()),
            runq_len: AtomicUsize::new(0),
            inbox_len: AtomicUsize::new(0),
            steal_request: work_steal_slot::Slot::new(),
            switches: AtomicU64::new(0),
            rng: AtomicU32::new(1), // reseeded per core in init_this_cpu; never left 0 (xorshift)
            #[cfg(target_arch = "x86_64")]
            x86_trap: X86TrapPerCpu::new(),
        }
    }

    /// Run `f` with exclusive access to this core's run queue, and return its result.
    ///
    /// Interrupts must be masked (debug-asserted): with single-owner access, that makes the `&mut`
    /// exclusive. Afterwards it mirrors the queue's length into `runq_len`.
    ///
    /// Context: interrupts masked, including the reschedule-SGI handler's inbox drain; takes no
    /// lock.
    pub(crate) fn with_runq<R>(&self, f: impl FnOnce(&mut Fifo<Thread>) -> R) -> R {
        debug_assert!(
            !crate::arch::interrupts::is_enabled(),
            "run queue touched with interrupts enabled: single-owner safety needs them masked",
        );
        // SAFETY: interrupts masked (asserted) and single-owner, so this `&mut` is exclusive.
        let q = unsafe { &mut *self.runq.get() };
        let r = f(q);
        // Relaxed: readers tolerate staleness, and the queue itself is single-owner.
        self.runq_len.store(q.len(), Ordering::Relaxed);
        r
    }

    /// Record this core's inbox length after a push or drain.
    ///
    /// Context: the inbox lock held, by the pushing core or this core's drain.
    pub(crate) fn note_inbox_len(&self, len: usize) {
        self.inbox_len.store(len, Ordering::Relaxed);
    }

    /// This block's index in [`PERCPU`], for the per-core arrays beside it ([`ADOPTED`]).
    fn index(&self) -> usize {
        (core::ptr::from_ref(self) as usize - PERCPU.as_ptr() as usize)
            / core::mem::size_of::<PerCpu>()
    }

    /// Count `n` threads adopted from this core's inbox onto its run queue.
    ///
    /// Context: the owning core's drain, under the inbox lock.
    pub(crate) fn note_adopted(&self, n: u64) {
        ADOPTED[self.index()].fetch_add(n, Ordering::Relaxed);
    }

    /// How many threads this core has taken out of its own inbox since boot. Relaxed: a stale read
    /// answers "has it happened yet" late, not wrongly.
    #[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
    pub fn adopted(&self) -> u64 {
        ADOPTED[self.index()].load(Ordering::Relaxed)
    }

    /// This core's load for a placement decision (DECISIONS §28.1): queued plus inbox threads.
    /// Relaxed and possibly stale by design.
    pub(crate) fn runnable(&self) -> usize {
        self.runq_len.load(Ordering::Relaxed) + self.inbox_len.load(Ordering::Relaxed)
    }

    /// This core's run-queue backlog alone, for a steal decision (DECISIONS §28.3): a thief never
    /// takes work still in a victim's inbox. Relaxed and possibly stale.
    pub fn runq_len(&self) -> usize {
        self.runq_len.load(Ordering::Relaxed)
    }

    /// Advance this core's placement PRNG and return the next value.
    ///
    /// # BUGS
    ///
    /// `sched::spawn` calls it with interrupts enabled, so a preemption between the load and the
    /// store can lose an update or store into a block the thread has left. Neither breaks placement.
    pub(crate) fn rng_next(&self) -> u32 {
        let mut x = self.rng.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng.store(x, Ordering::Relaxed);
        x
    }
}

/// How many threads each core has adopted out of its own inbox, monotonic since boot.
///
/// A count, not a depth like `inbox_len`, so a push and drain between two reads still show. It exists
/// for `smp::tests::work_can_be_placed_on_every_cpu`, which asserts delivery to the named core
/// because stealing may move the thread before it runs. An array beside `PerCpu`, because a drain
/// is on no hot path (notes/per-cpu.md).
static ADOPTED: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// The per-CPU blocks, one per core, static so a core's block exists before any allocator.
static PERCPU: [PerCpu; MAX_CPUS] = [const { PerCpu::new() }; MAX_CPUS];

/// `PerCpu`'s size stays a power of two, so `PERCPU[id]` indexes with a shift.
///
/// One field more once cost riscv64's `ipc_send_receive` 150 bytes and failed
/// `script/fastpath-footprint`. Per-core state the fastpath does not need goes in its own array.
/// See notes/per-cpu.md.
const _: () = assert!(
    core::mem::size_of::<PerCpu>().is_power_of_two(),
    "PerCpu's size must be a power of two so PERCPU[id] indexes with a shift; see the note above \
     this assertion for the 150 bytes of IPC fastpath the last violation cost",
);

/// The span within which one core's writes slow another core's reads of a neighbouring word, and
/// the unit [`PerCpu`] is aligned to.
///
/// 128 on every architecture: Apple's line, and a pair of lines on 64-byte parts that prefetch in
/// pairs. Measured as about 1.7 ns per trap on an Apple M3 when blocks shared lines
/// (notes/per-cpu.md).
const FALSE_SHARING_SPAN: usize = 128;

/// No two cores' `PerCpu` blocks share a [`FALSE_SHARING_SPAN`]: an alignment of at least the span
/// puts each element of [`PERCPU`] on its own. Fails if a `repr(align)` is lost or lowered.
const _: () = assert!(
    core::mem::align_of::<PerCpu>() >= FALSE_SHARING_SPAN,
    "PerCpu must be aligned to at least FALSE_SHARING_SPAN so no two cores' blocks share a cache \
     line; see the note on FALSE_SHARING_SPAN for the cost that was measured",
);

/// Point this core's per-CPU register at block `id`, and seed its placement PRNG.
///
/// Call once per core, before that core takes its first lock, because the lock path reads
/// `current().held_rank`: before `console::init` on the boot core, first thing on a secondary.
/// Panics if `id` is not below [`MAX_CPUS`].
pub(crate) fn init_this_cpu(id: usize) {
    assert!(id < MAX_CPUS, "cpu id {id} exceeds MAX_CPUS {MAX_CPUS}");
    crate::arch::set_percpu(&PERCPU[id] as *const PerCpu as usize);
    // Deterministic per core, so the cores do not sample in lockstep. `| 1` keeps it nonzero,
    // which xorshift requires.
    PERCPU[id].rng.store(
        0x2545_F491 ^ (id as u32).wrapping_mul(0x9E37_79B9) | 1,
        Ordering::Relaxed,
    );
}

/// This core's private block, in one instruction.
///
/// From preemptible code it may name a core the caller has since left: fine for a hint, wrong for
/// the running thread's name, which `arch::read_current_thread` reads in one step
/// (notes/scheduler.md, "The costs migration made real").
pub fn current() -> &'static PerCpu {
    // SAFETY: `init_this_cpu` set TPIDR_EL1 to the address of an element of the `'static`
    // `PERCPU` array, and nothing else ever writes TPIDR_EL1. The one window where this would be
    // wrong is before `init_this_cpu` has run on this core, which is exactly why that call is the
    // first thing `kernel_main` (and each secondary's entry) does, ahead of any lock.
    unsafe { &*(crate::arch::percpu() as *const PerCpu) }
}

/// This core's logical id: its index into `PERCPU`.
pub fn id() -> usize {
    let base = PERCPU.as_ptr() as usize;
    // Wrapping, because it cannot wrap: the register holds `&PERCPU[id]`, at or above `base`. Were
    // that broken, the quotient lands far past `MAX_CPUS` and the next index refuses it.
    crate::arch::percpu().wrapping_sub(base) / core::mem::size_of::<PerCpu>()
}

/// Another core's migration inbox, by id. This is the one place a core reaches into a *different*
/// core's `PerCpu` block, and only the inbox (a real lock), never the run queue. See DECISIONS §11.
pub(crate) fn inbox_of(id: usize) -> &'static IrqSafeMutex<Fifo<Thread>> {
    &PERCPU[id].inbox
}

/// Any core's block, by id, for read-only diagnostics (the hang dump). Reading another core's
/// atomics is racy but best-effort, which is all a post-mortem dump needs.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn of(id: usize) -> &'static PerCpu {
    &PERCPU[id]
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::Ordering;

    use super::*;

    /// Whatever core this runs on set up its per-CPU pointer, and `current()` reaches that core's
    /// block and no other.
    ///
    /// It does not assert which core it runs on: tests are not pinned, and §28's stealing may move
    /// the test thread (notes/scheduler/test-flakes.md). `boot_cpu_id()` is read rather than assumed
    /// 0, since on RISC-V QEMU picks the boot hart.
    #[test_case]
    fn percpu_is_self_consistent_on_whatever_cpu_we_run() {
        let me = id();
        assert!(me < MAX_CPUS, "cpu::id() returned {me}, out of range");
        assert_eq!(
            current() as *const PerCpu,
            &PERCPU[me] as *const PerCpu,
            "current() does not point at this core's own block"
        );
        // The boot core's block is reachable by index from any core, which is what `of()` promises
        // and what the cross-core paths (IPI, stealing) rely on.
        let boot = crate::arch::boot_cpu_id();
        assert_eq!(
            of(boot) as *const PerCpu,
            &PERCPU[boot] as *const PerCpu,
            "of(boot) does not reach the boot core's block"
        );
    }

    /// Between tests this core's block records no lock held. The rank tests in sync.rs exercise a
    /// full lock cycle.
    #[test_case]
    fn held_rank_lives_in_the_percpu_block() {
        assert_eq!(current().held_rank.load(Ordering::Relaxed), rank::NONE);
    }
}
