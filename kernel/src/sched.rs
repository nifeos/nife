//! A round-robin scheduler, and the preemption that makes it mean something.
//!
//! # The whole point of the project, arriving
//!
//! DECISIONS §5, written before a line of kernel existed:
//!
//! > A userspace process is an arbitrary ELF binary. It has its own stack, it never yields, and
//! > it will loop forever because we will write a bug. Under cooperative scheduling, one bad
//! > user program hangs the machine permanently.
//!
//! This file is where that stops being true. The timer fires, the handler calls [`schedule`],
//! and the CPU is **taken away** from a thread that never asked to give it up.
//!
//! There is a test named `a_thread_that_never_yields_is_preempted_anyway`. It spawns a thread
//! whose entire body is `loop { count += 1 }`: no yields, no syscalls, not even a function
//! call. Under any cooperative scheduler that is a hung machine. Here it is a Tuesday.
//!
//! # Three rules, and each of them is a bug if you get it wrong
//!
//! **1. Release the run-queue lock BEFORE switching.** Switch away while holding it and the
//!    lock is now held by a thread that is not running. The next thread to want it spins
//!    forever waiting for a thread that will never be scheduled, because scheduling requires
//!    the lock. A deadlock of a shape that would take a day to find.
//!
//! **2. Interrupts stay masked across the switch.** Between "I decided to switch" and "I
//!    switched" there must be no window for a timer interrupt to decide *again*. And the mask
//!    is per-thread, because each thread's `schedule()` frame lives on its own stack, which is
//!    exactly what makes this work at all.
//!
//! **3. A brand-new thread must unmask interrupts itself.** Every *resumed* thread gets its
//!    interrupt state back from `eret` restoring `SPSR_EL1`. A thread that has never run has no
//!    `SPSR` to restore. `thread_trampoline` does `msr daifclr, #2` for exactly this reason,
//!    and without it the first thread you spawn can never be preempted, which would be a
//!    cooperative scheduler with extra steps.

use core::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, Ordering};

use intrusive_fifo::Unqueued;
use thread_wake_handshake::{SwitchOutVerdict, WakeVerdict};

use crate::cpu;
use crate::sync::{IrqSafeMutex, rank};
use crate::thread::{
    CapabilityTableLock, Context, QuotaToken, State, Thread, ThreadId, Wait, WaitRole, switch_to,
};

/// How many times we have actually taken the CPU away from a thread. The number that says
/// preemption is real.
static PREEMPTIONS: AtomicU64 = AtomicU64::new(0);

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
static PREEMPTIONS_PER_CPU: [AtomicU64; cpu::MAX_CPUS] =
    [const { AtomicU64::new(0) }; cpu::MAX_CPUS];

/// The thread running on **this core** right now.
///
/// Per-CPU as of §11 step 3b (`cpu::PerCpu::current`); it used to be one field on the global
/// `IpcTables`. Reading it is a plain atomic load and needs no lock: it is this core's own slot.
fn current_thread_id() -> ThreadId {
    cpu::current().current.load(Ordering::Relaxed)
}

/// **The running thread's capability table, per core** (provisional name, 2026-10-04 UTC): what
/// [`current_cap`] reads instead of taking `IPC_TABLES` to find the thread.
///
/// Null before this core runs a thread. Written only by [`set_current`], on the owning core, in the
/// same breath as `PerCpu::current`, so the two cannot disagree; read only by the owning core, with
/// interrupts masked.
///
/// **Its own array rather than a `cpu::PerCpu` field**, for [`PREEMPTIONS_PER_CPU`]'s measured
/// reason: `PerCpu` is exactly 128 bytes and one more word makes it 136. **Each entry on a line of
/// its own**, because every core writes its entry on every switch and reads it on every syscall, and
/// a shared line would trade the lock this removes for line traffic between the same cores (the
/// `PerCpu` straddle in notes/job-mix/null-syscall-under-load.md's BUGS is that shape).
///
/// **Why the pointer stays valid without a lock.** It names the page of the thread running on this
/// core, and a running thread's page is not recycled: `Threads::remove` is reached only from
/// [`reap_switched_out`], for a thread its successor has switched off, and from
/// [`reap_region_objects`], whose `region_reap_verdict` refuses a `Ready` or `Running` thread and
/// one any core is still standing on. `schedule` replaces the pointer before the switch that makes
/// the old thread reapable. A thread can no more outlive this pointer than it can outlive its own
/// kernel stack, and for the same reason.
#[repr(align(64))]
struct CurrentCapabilities(AtomicPtr<CapabilityTableLock>);

static CURRENT_CAPABILITIES: [CurrentCapabilities; cpu::MAX_CPUS] =
    [const { CurrentCapabilities(AtomicPtr::new(core::ptr::null_mut())) }; cpu::MAX_CPUS];

/// Make `tid` this core's running thread: its name in `PerCpu::current` and its capability table in
/// [`CURRENT_CAPABILITIES`]. The only writer of either, so a lookup never finds one thread's name
/// beside another's table. Caller holds `IPC_TABLES`.
///
/// # Safety
///
/// `tcb` is `tid`'s page pointer as the thread table stores it (`Threads::pointer`), live.
unsafe fn set_current(tid: ThreadId, tcb: *mut Thread) {
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
fn current_capabilities() -> Option<crate::sync::IrqSafeGuard<'static, crate::cap::CapabilityTable>>
{
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

/// A synchronous IPC rendezvous point: the two wait queues and the pending-signal count.
///
/// **The state machine is the `inter_process_communication` crate**, which owns the queues and the
/// decision logic (send, receive, signal) and carries machine-checked proofs of its one invariant, "at
/// most one wait queue is ever non-empty" (DECISIONS §14, milestone 18; notes/verification.md). The
/// six IPC functions below decide *what* to do by calling the proved logic and spend their own code
/// only on the bookkeeping the queues cannot express (mailboxes, waking a thread onto a run queue,
/// the one-shot Reply that leaves a caller blocked).
///
/// Intrusive as of milestone 14 phase A.3: a wait-queue entry is the TCB itself, threaded through
/// the same link the run queues use, so blocking on an rendezvous cannot allocate and "a thread waits
/// on one rendezvous at a time" is physical (one link). The safety contract for the pointers is the
/// queue discipline at [`thread_control_block_ptr`].
type Rendezvous = inter_process_communication::Rendezvous<Thread>;

/// The most threads that can be alive at once, whole machine (milestone 14 phase A). A documented
/// limit of the image rather than a heap that can be exhausted: spawn past it fails cleanly, the
/// same contract callers already have for out-of-memory.
///
/// # The ledger, and why 256
///
/// **128 until 2026-08-27, and the suite had been sitting on the ceiling for some time without
/// anybody being able to see it.** Milestone 169's leak-fix lane found it the hard way: with its
/// `raw_mode`/`rmle` leak fixed, the aarch64 run reached further than any run before it and
/// stopped dead at `time_tests::a_shell_with_no_usable_clock_times_the_command_anyway`, with
/// **128 of 128 live and 121-123 `Blocked`**, twice, at the same point both times. Not a leak:
/// `thread_leak_police` (no runnable spinner left over) passed both runs. What is alive is the
/// accumulated cost of this tree's many individually-reasonable services that are
/// **intentionally permanent for the boot** (`notes/frames.md`'s "held" list: the FS servers,
/// two credential store instances, `login`, both `net_stack` transports, mDNS, `gpu_driver`,
/// `compositor`, and more since), each accepted on its own merits over many milestones and never
/// once priced against this shared ceiling collectively.
///
/// **What the measurement then showed, and it is worse than the report that prompted it.**
/// [`PEAK_THREADS`] and `kernel::testing`'s closing `threads:` line were built for this raise, so
/// the number is read rather than guessed. On `main`, at 128, the aarch64 suite reports a peak of
/// **exactly 128 with zero spare and still passes**: it is not that the ceiling is about to bite,
/// it is that the ceiling is already refusing spawns and the refusals were being swallowed. Raise
/// the ceiling and nothing else, and the same suite says what it actually wanted:
///
/// | architecture | peak live threads | with the ceiling at 256 |
/// |---|---|---|
/// | aarch64 | **130** | 126 spare |
/// | riscv64 | **129** | 127 spare |
/// | `x86_64` | **57** | 199 spare (its userspace suite is smaller: 192 run, 56 skipped) |
///
/// Those are the figures on the tree this was measured on; they move by a thread or two as the
/// boot's service list changes (the merge that landed while this branch was open took aarch64 to
/// 129), which is exactly why the number is printed by every run rather than only written here.
///
/// So the tightest real demand is 130, and 256 is **1.97x it**. Headroom rather than a fitted
/// number, deliberately, because every milestone that adds a boot service spends some of this and
/// the failure mode is an unrelated test refusing a spawn far from the cause. What that headroom
/// costs is measured below rather than asserted, which is the only reason it can be called cheap.
///
/// # What it costs, per slot, measured
///
/// Nothing here scales with the ceiling *except* through these, and each was checked at 256:
///
/// - **The boot stack, 20 bytes a slot.** [`init`] installs the tables, and an unoptimised build
///   carries the thread table on its frame. That frame was 43,952 bytes at 128 (the deepest in the
///   kernel) and the suite's boot-stack high-water was 54,336 of 65,504, 82%. Installing
///   [`EMPTY_TABLES`] instead of building a `Threads` local and moving it in cut the frame to
///   15,696 at 128 and made the slope 20 bytes rather than ~80; at 256 the high-water is **48,760
///   (74%)**, lower than before the raise. `stack::report_high_water`'s gate is 61,440.
/// - **`kmem::KERNEL_OBJ_PAGES`, 7 pages a *live* thread** (`thread::STACK_PAGES` = 6, plus the
///   TCB page), which is why that carve went 1024 -> 2048. Spent per live thread, not per slot,
///   so unused headroom costs only that constant's own `[u64; KERNEL_OBJ_PAGES]` free stack.
/// - **`ps::MAX_ROWS`**, which `kernel::user::survey_tests` const-asserts is `>= MAX_THREADS`,
///   because a `ps` holding the widest grant must have room for every row. It sizes stack-resident
///   `[Row; MAX_ROWS]` arrays in `ps`, `watch` and `pgrep`; measured with `-Z emit-stack-sizes` at
///   256, `_start` is 4,240 bytes in `ps`, 4,320 in `pgrep` and 8,464 in `watch` (which holds
///   two), against the 12 pages (49,152 bytes) `system_initializer::CHILD_STACK_PAGES` gives every
///   child. 17% at the worst.
/// - **`revoke::MAX_SPACES`** and **`thread::FreeAddressSpace`**, both of which are now written as
///   arithmetic on this constant rather than as a literal that has to be remembered. The second
///   was a bare `[u64; 128]` with a `debug_assert` for a comment; it could have drifted silently.
/// - **Nothing on the `spawn_el0` benchmark, but only after a second fix.** The first attempt at
///   this raise cost that benchmark **+348,133 icount ticks (+16.8%)** and failed `script/bench
///   --check`, through two scans of equal size whose cost tracked this constant rather than what
///   the machine held: `delete_page_frame_caps_where`'s walk of every thread's capability table
///   (141,359) and `revoke`'s registry, whose `MAX_SPACES` is derived from this one (138,584).
///   Both are bounded by live occupancy now (`generational_table`'s `top`, `revoke::Registry`'s),
///   which took the benchmark to **1,212,888, 41% below the 128-slot baseline**, and, the part
///   that matters here, made it **flat against this constant**: doubling again to 512 moves it 587
///   ticks. Re-baselining instead was available and refused, because the cost belonged to slots
///   nothing occupied and would have been paid again by every future raise. notes/benchmarks.md
///   has the attribution table.
/// - **Two `[u64; MAX_THREADS]` scratch arrays in [`reap_region_objects`]**, which took that frame
///   to 4,624 bytes at 256, over `script/stack-frame-check`'s 4,096-byte guard-page ceiling. Both
///   are gone: the function's own comment already prescribed rescanning rather than collecting,
///   for exactly this reason, and these two were the sites it had not been applied to.
///
/// # BUGS
///
/// - **`design/decisions/0096-process-kernel-or-event-kernel.md` prices the confinement claim off
///   the old number** ("`MAX_THREADS` is 128, so kernel stacks total 3.00 MiB, static"). The shape
///   of that argument survives (the bound is still static and still the product of two constants)
///   but the figure does not, and a decision record is not a lane's to edit. Whoever ratifies this
///   raise owes §96 a corrected sentence.
/// - **Revocation still scans every live thread, and `MILESTONE 183` is the fix.** Bounding the
///   sweep by occupancy makes it independent of *this* constant; it does not make it independent
///   of how many threads are actually alive. A boot with real tenancy pays the walk in full. That
///   milestone ("a physical-range index for capability holders, so revocation stops scanning every
///   thread") is the structural answer, and the numbers above are evidence for it rather than
///   against it.
/// - **The peak is a whole-boot high-water, not a per-test charge.** It cannot say *which* test
///   pushed the table up, only that something did, and for this table that is the honest shape:
///   what fills it is what earlier tests deliberately left running. See
///   `kernel::testing`'s `report_thread_peak`, which explains why it reports and does not gate.
pub const MAX_THREADS: usize = 256;

/// **The most threads that were ever alive at once on this boot.**
///
/// The instrument that makes [`MAX_THREADS`] a measured number rather than a felt one, and the
/// reason a future lane does not have to repeat the two instrumented runs that found the
/// ceiling in the first place. It answers the only question a capacity limit poses, "how close
/// is the suite to it", which the refusal itself cannot: by the time a spawn is refused the
/// table is full and every earlier number is gone.
///
/// A high-water mark rather than a ledger, deliberately. Attributing threads to tests the way
/// `kernel::testing`'s frame ledger attributes frames would need a per-test reading, and the
/// answer would be misleading anyway: what fills this table is not one test's spend but the
/// services every earlier test left running on purpose (`notes/frames.md`'s "held" list). One
/// number for the whole boot is the honest shape.
///
/// Updated on the two inserts that can grow the table, under `IPC_TABLES`, so it never races;
/// `fetch_max` rather than a compare-store because the write is cheap and this is not a hot path
/// (a spawn already costs a page).
static PEAK_THREADS: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// **Scheduled on-CPU time, one timer tick at a time, keyed by thread slot** (milestone 282 (a thread's CPU time, and the `top` it makes possible),
/// DECISIONS §150 (how does a thread's CPU time reach userspace?)). `abi::survey::record::CPU_TIME` is what reads it.
///
/// **Why an array beside the table rather than a `u64` on `Thread`**, which is what §150's build
/// note described and is the one place this differs from it. The increment happens in
/// [`on_tick`], in interrupt context, and the only name a core has for its running thread there is
/// a tid in its own per-CPU block. Turning that tid into a `&mut Thread` means `IPC_TABLES`, and a
/// timer interrupt that waits on a lock another core holds is a scheduler-latency hole opened at
/// every tick on every core; `try_lock` is no better, because a dropped sample is not a coarse
/// number, it is a wrong one, and it would be dropped exactly when the machine is busiest. The
/// slot index is already in the tid's low 32 bits (`generational_table`'s packing), so an array
/// keyed by slot needs no lock, no lookup, and no ordering beyond relaxed.
///
/// **Not on `cpu::PerCpu` either**, and that is a measured constraint rather than a preference:
/// milestone 527 (the survey selector, and a thread's placement) grew `PerCpu` by one `u64`, took
/// `size_of::<PerCpu>()` from 128 to 136, and cost riscv64's IPC fastpath 5.4% because `PERCPU[id]`
/// stopped indexing with a shift. `kernel/src/cpu.rs` now const-asserts that size. This counter is
/// per **thread**, not per core, so it was never a candidate for that struct; the note is here
/// because the next person to add a counter will consider it.
///
/// **Two kilobytes of `.bss`**, which is `MAX_THREADS` slots at eight bytes and does not move with
/// the workload.
///
/// # The race, stated rather than assumed (rule 4)
///
/// A core's tick increments only the slot of the thread that core is running, so the **write** is
/// uncontended by construction and needs no cross-core synchronisation. A survey reading a slot
/// another core is incrementing is a relaxed load of a value in flight: it reads a number that was
/// true a moment ago, never a torn one, because the unit is a naturally aligned `u64`. That is the
/// same bargain the per-CPU `TICKS` array already takes, and it is the right one for a statistic
/// nobody branches on.
///
/// # Slot reuse
///
/// The counter is zeroed when a slot is **filled**, not when it is emptied, which is what lets a
/// corpse keep the time it earned until its supervisor reaps it. A survey can report a `DEAD`
/// thread, and reporting it with a fresh occupant's zero (or a previous occupant's total) would be
/// the plausible wrong number this tree already ruled against.
static CPU_TICKS: [core::sync::atomic::AtomicU64; MAX_THREADS] =
    [const { core::sync::atomic::AtomicU64::new(0) }; MAX_THREADS];

/// The slot a generational thread name occupies, for indexing [`CPU_TICKS`].
///
/// `generational_table` packs `(generation << 32) | slot`, so the slot is the low word. No bound
/// check here on purpose: every caller indexes [`CPU_TICKS`] with `get`, so an out-of-range answer
/// is `None` rather than a panic, and `cpu::NO_TID` (`u64::MAX`) lands out of range for free, which
/// is what makes "this core is running nothing" cost no branch of its own.
const fn slot_of(tid: ThreadId) -> usize {
    (tid & 0xffff_ffff) as usize
}

/// **Charge one timer tick to the thread this core is running** (milestone 282 (a thread's CPU time, and the `top` it makes possible)).
///
/// The whole of the accounting, called from [`on_tick`] in interrupt context: one relaxed load of
/// this core's `current`, one bounds-checked index, one relaxed increment. Nothing here can block,
/// allocate, or take a lock, which is the property that let this go on the tick path at all.
///
/// A core running nothing yet (`cpu::NO_TID`) charges nobody, and costs the same branch the bounds
/// check already spends. The idle thread is a thread and is charged like any other; it is not in
/// any supervision domain, so no survey reports it.
fn charge_tick() {
    if let Some(counter) = CPU_TICKS.get(slot_of(current_thread_id())) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// Start a freshly filled slot's CPU time at zero. Called by both inserts, under `IPC_TABLES`.
fn clear_cpu_ticks(tid: ThreadId) {
    if let Some(counter) = CPU_TICKS.get(slot_of(tid)) {
        counter.store(0, Ordering::Relaxed);
    }
}

/// The high-water mark [`PEAK_THREADS`] holds. Printed by the test suite's closing summary.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the closing summary is the only reader
pub fn peak_thread_count() -> usize {
    PEAK_THREADS.load(Ordering::Relaxed)
}

/// The thread table: generational names (`crates/slots`, notes/generational-names.md) over
/// **page-resident** TCBs (milestone 19c.2). Each `Thread` lives at the start of one page from
/// the kernel's own budget (`kmem`), so the static `MAX_THREADS`-sized BSS pool that B.2 built
/// as a scaffold is gone: the kernel reserves no per-thread memory it hasn't been handed, the
/// last uncovered corner of milestone 14's no-open-ended-spending thesis. B.2 named this moment
/// ("the pool upgrades to retype-backed storage behind the table when init lands"); this is it.
///
/// A page's address never changes (direct-mapped, and its `kmem` region is pinned), which
/// supplies the pinning the per-thread `Box` and then the pool both provided: the context-switch
/// assembly and the intrusive queues hold pointers straight into these pages. The table stores
/// the pointer; the generational name is what everything else carries (stale-safe as ever).
///
/// 19c.3 will let a user process retype a TCB from *its own* untyped by the same mechanism, the
/// page merely coming from a different budget; kernel threads keep drawing from `kmem`.
/// A TCB pointer that may cross cores. The pointer itself moving between cores is harmless: the
/// `Thread` it names is touched only under `IPC_TABLES` (which serializes all table access) and, for
/// its queue link, under the intrusive discipline at [`thread_control_block_ptr`]. This is the same soundness the
/// old static `TcbPool`'s `unsafe impl Sync` rested on, now attached to the pointer the table
/// stores rather than a separate array.
#[derive(Clone, Copy)]
struct ThreadControlBlockPointer(*mut Thread);

// SAFETY: see the type's doc; sending the pointer is sound because dereferencing it is gated.
unsafe impl Send for ThreadControlBlockPointer {}

struct Threads {
    table: generational_table::Table<ThreadControlBlockPointer, MAX_THREADS>,
}

impl Threads {
    const fn new() -> Self {
        Self {
            table: generational_table::Table::new(),
        }
    }

    fn get(&self, tid: ThreadId) -> Option<&Thread> {
        let p = self.table.get(tid)?.0;
        // SAFETY: a pointer we stored at insert, into a live kmem page not yet recycled (remove
        // kills the name before recycling); IPC_TABLES serializes access.
        Some(unsafe { &*p })
    }

    fn get_mut(&mut self, tid: ThreadId) -> Option<&mut Thread> {
        let p = self.table.get(tid)?.0;
        // SAFETY: as `get`, and `&mut self` carries IPC_TABLES's exclusivity.
        Some(unsafe { &mut *p })
    }

    /// **A thread's capability table**, for a caller holding `IPC_TABLES` that needs no other part
    /// of the thread. The table is past the `Thread` in its page (`thread::capability_table_of`), so
    /// the reference is derived from the page pointer, never from a `&Thread`.
    fn capabilities(&self, tid: ThreadId) -> Option<&CapabilityTableLock> {
        let p = self.table.get(tid)?.0;
        // SAFETY: the stored page pointer of a live thread; the page outlives `&self`, because
        // only `remove` (which takes `&mut self`) recycles it.
        Some(unsafe { &*crate::thread::capability_table_of(p) })
    }

    /// `get_mut` and [`capabilities`](Self::capabilities) at once, for the sites that write the
    /// `Thread` and its table together. Disjoint by construction: the table is past the struct's
    /// end, so the `&mut Thread` does not cover it.
    fn get_mut_with_capabilities(
        &mut self,
        tid: ThreadId,
    ) -> Option<(&mut Thread, &CapabilityTableLock)> {
        let p = self.table.get(tid)?.0;
        // SAFETY: as `get_mut` for the `Thread` and `capabilities` for the table; the two do not
        // overlap.
        Some(unsafe { (&mut *p, &*crate::thread::capability_table_of(p)) })
    }

    /// **The raw pointer the table stores, which is the start of the thread's TCB page.**
    ///
    /// `get` and `get_mut` narrow it to a reference over `size_of::<Thread>()` bytes, and that is
    /// exactly what the caller must not have when it wants the FP register file: that lives in the
    /// same page, past the end of the struct, so a pointer derived from a reference would be
    /// reaching outside its own provenance (`thread::fp_state_of`'s safety note). This hands back
    /// the pointer the page cast produced, unnarrowed.
    fn pointer(&self, tid: ThreadId) -> Option<*mut Thread> {
        Some(self.table.get(tid)?.0)
    }

    /// Insert: claim a page from the kernel budget, build the `Thread` (carrying its own minted
    /// name) into it, and store the pointer under that name. `None` (page recycled, `f` never
    /// run) if the budget or the table is exhausted.
    fn insert_with(&mut self, f: impl FnOnce(ThreadId) -> Thread) -> Option<ThreadId> {
        let page = crate::kmem::page()?;
        // A kernel thread's TCB page is `kmem`'s and comes home to it at death; if the table is
        // full it never held a Thread, so recycle now.
        let name = self.insert_at(page, f);
        if name.is_none() {
            crate::kmem::recycle(page);
        }
        name
    }

    /// `insert_with`'s place-writing twin: claim a page from the kernel budget and let `build`
    /// construct the Thread into it. Recycles the page on any failure, exactly as `insert_with`
    /// does, so a refused spawn costs nothing.
    fn insert_in_place(
        &mut self,
        build: impl FnOnce(ThreadId, *mut Thread) -> bool,
    ) -> Option<ThreadId> {
        let page = crate::kmem::page()?;
        let name = self.insert_at_in_place(page, build);
        if name.is_none() {
            crate::kmem::recycle(page);
        }
        name
    }

    /// Insert a Thread that already has a page (milestone 19c.3): a user-retyped TCB, whose page
    /// is its creator's region's, not `kmem`'s. On a full table the page is the region's to
    /// account (spend-only), so nothing is recycled here.
    fn insert_from_page(
        &mut self,
        page: u64,
        f: impl FnOnce(ThreadId) -> Thread,
    ) -> Option<ThreadId> {
        self.insert_at(page, f)
    }

    /// **The place-writing insert** (milestone 124): `build` receives the minted name and the TCB
    /// page, and writes the Thread there itself. `false` from `build` means it wrote nothing, and
    /// the slot is left exactly as it was found.
    ///
    /// The difference from `insert_at` is where the Thread is constructed. That one takes a
    /// `FnOnce(ThreadId) -> Thread`, so the value travels through the closure's return and a temporary
    /// before `ptr.write` puts it on the page; a `Thread` is large and a debug build copies at
    /// every hop. This hands the destination down instead. See `Thread::spawn_into`.
    fn insert_at_in_place(
        &mut self,
        page: u64,
        build: impl FnOnce(ThreadId, *mut Thread) -> bool,
    ) -> Option<ThreadId> {
        let ptr = crate::arch::mmu::phys_to_virt(page) as *mut Thread;
        let mut built = false;
        let name = self.table.insert_with(|tid| {
            built = build(tid, ptr);
            if built {
                // The register file beside the struct, in the same page (milestone 447). Here
                // rather than in a `Thread` constructor because a constructor builds a value and
                // this is a fact about a *place*: `kmem` pages are not zeroed, so the `live` flag
                // has to be written before anything reads it. See `thread::init_fp_state`.
                //
                // SAFETY: `ptr` is the start of a page this insert exclusively owns, and `build`
                // has just written a live `Thread` at it.
                unsafe { crate::thread::init_fp_state(ptr) };
                // SAFETY: as above; the name is not yet handed to anyone, so nothing else can
                // reach the table.
                unsafe { crate::thread::init_capability_table(ptr) };
            }
            ThreadControlBlockPointer(ptr)
        })?;
        if !built {
            // `build` declined (no kernel stack). Take the name back out: the slot never held a
            // Thread, so there is nothing to drop, and `remove` here would drop uninitialised
            // bytes. `forget_slot` bumps the generation and frees the slot without touching the
            // TCB page, which is the caller's to recycle. `Table::remove` is exactly right and
            // not a leak: the slot holds a `ThreadControlBlockPointer`, and dropping that drops a pointer. The
            // `Thread` drop lives in `Threads::remove`, which is not on this path because no
            // Thread was ever constructed.
            self.table.remove(name);
            return None;
        }
        self.note_peak();
        self.mint_token(name);
        // A reused slot starts its CPU accounting at zero; see `CPU_TICKS`.
        clear_cpu_ticks(name);
        Some(name)
    }

    /// The shared engine: write the built Thread into `page` and name it. The Thread carries its
    /// own `thread_control_block_kmem`, which `remove` reads to decide whether the page returns to `kmem`.
    fn insert_at(&mut self, page: u64, f: impl FnOnce(ThreadId) -> Thread) -> Option<ThreadId> {
        let ptr = crate::arch::mmu::phys_to_virt(page) as *mut Thread;
        let name = self.table.insert_with(|tid| {
            // SAFETY: a fresh, exclusively-ours page; `write` moves the Thread in, no drop of
            // uninitialized bytes.
            unsafe { ptr.write(f(tid)) };
            // And its register file beside it in the same page; see `insert_at_in_place` for why
            // this is here rather than in a `Thread` constructor.
            //
            // SAFETY: as above, with the `Thread` now live at `ptr`.
            unsafe { crate::thread::init_fp_state(ptr) };
            // And its capability table, past the register file, for the same reason.
            //
            // SAFETY: as above; the name is not yet handed to anyone.
            unsafe { crate::thread::init_capability_table(ptr) };
            ThreadControlBlockPointer(ptr)
        });
        if let Some(name) = name {
            self.note_peak();
            self.mint_token(name);
            // A reused slot starts its CPU accounting at zero; see `CPU_TICKS`.
            clear_cpu_ticks(name);
        }
        name
    }

    /// **Mint a new thread's queue token: the only place in the kernel a token is made**
    /// (milestone 139 (drive the unsafe count down), round 10). Every thread the table holds was
    /// inserted through `insert_at` or `insert_at_in_place`, and both call this once, on the name
    /// they just minted, so every thread has exactly one token for its whole life. From here the
    /// token moves: onto a queue at a push, back onto [`Thread::own_token`] at a pop or a removal
    /// (`hold_token`), and nowhere else. See `thread::Thread::own_token` for the three places it
    /// can be.
    ///
    /// The `unsafe` is `Unqueued::new`'s three obligations, discharged once here instead of at
    /// each of the twenty-one pushes, sends and receives that used to assert them:
    ///
    /// - **Valid while reachable.** The token points at the `Thread` on its TCB page, which lives
    ///   until `Threads::remove`. That is reached only for a `Finished` thread off its CPU
    ///   (`reap_switched_out`), which is on no queue because a running thread holds its own token,
    ///   and for a thread region teardown ends (`finish_blocked_resident`), which unlinks it from
    ///   every wait queue first. Either way the token dies with the page, in `own_token`, or was
    ///   already dropped by the removal that unlinked it.
    /// - **On no queue, and the only token.** The name is a moment old and no other token for this
    ///   page can exist: the previous occupant's died with it.
    /// - **Links touched only by a queue, with no reference live across a queue operation.** Every
    ///   queue holding a `Thread` is behind `IPC_TABLES` or an inbox lock, and the `&mut` below
    ///   ends before this returns.
    fn mint_token(&mut self, name: ThreadId) {
        if let Some(t) = self.get_mut(name) {
            let node = core::ptr::NonNull::from(&mut *t);
            // SAFETY: the three obligations above.
            t.own_token = Some(unsafe { Unqueued::new(node) });
        }
    }

    /// Remove and destroy: drop the TCB in place (its stack, address space, and quota token go
    /// with it), kill the name so no copy of the `ThreadId` ever resolves again, then recycle the page.
    fn remove(&mut self, tid: ThreadId) {
        let Some(&ThreadControlBlockPointer(ptr)) = self.table.get(tid) else {
            return;
        };
        // Read the page's origin BEFORE the drop consumes the Thread. A kernel TCB's page goes
        // home to `kmem`; a user TCB's page belongs to its region (spend-only, reclaimed only at
        // region destroy), so the reaper leaves it.
        // SAFETY: live per the table, exclusive per `&mut self`.
        let from_kmem = unsafe { (*ptr).thread_control_block_kmem };
        // SAFETY: as above. Drop first (KernelStack's unmap-and-recycle, AddressSpace teardown,
        // the QuotaToken), then kill the name, then the page goes home: nothing can reach the
        // dropped Thread afterward.
        unsafe { core::ptr::drop_in_place(ptr) };
        self.table.remove(tid);
        if from_kmem {
            crate::kmem::recycle(crate::arch::mmu::virt_to_phys(ptr as u64));
        }
    }

    fn len(&self) -> usize {
        self.table.len()
    }

    /// Record the table's occupancy against [`PEAK_THREADS`]. Called on the two paths that can
    /// make the table grow, which is every insert: `insert_at` and `insert_at_in_place` are what
    /// the three public inserts delegate to.
    fn note_peak(&self) {
        PEAK_THREADS.fetch_max(self.table.len(), Ordering::Relaxed);
    }

    /// Every live TCB, for whole-table sweeps (revocation). Each live name resolves to a
    /// distinct page pointer, so the `&mut`s are disjoint.
    fn iter_mut(&mut self) -> impl Iterator<Item = &mut Thread> + '_ {
        // SAFETY: each stored pointer is a distinct live page (one page per thread), and
        // `&mut self` carries IPC_TABLES's exclusivity across the whole sweep.
        self.table
            .values()
            .map(|&ThreadControlBlockPointer(p)| unsafe { &mut *p })
    }

    /// [`iter_mut`](Self::iter_mut) with each thread's capability table beside it, for the
    /// revocation sweeps. Disjoint as [`get_mut_with_capabilities`](Self::get_mut_with_capabilities).
    fn iter_mut_with_capabilities(
        &mut self,
    ) -> impl Iterator<Item = (&mut Thread, &CapabilityTableLock)> + '_ {
        // SAFETY: as `iter_mut`, and the table is past each `Thread`'s end in its own page.
        self.table
            .values()
            .map(|&ThreadControlBlockPointer(p)| unsafe {
                (&mut *p, &*crate::thread::capability_table_of(p))
            })
    }

    /// Every live TCB from slot `from` onward, with its slot index, for a **resumable** sweep
    /// (`rendezvous::SURVEY`, milestone 126 (the `procps` package)). The slot is the caller's cursor; see
    /// `generational_table::Table::iter_from` for why a position would not do.
    fn iter_from(&self, from: usize) -> impl Iterator<Item = (usize, &Thread)> + '_ {
        // SAFETY: as `iter_mut`, and shared rather than exclusive: each stored pointer is a
        // distinct live page, and `&self` carries IPC_TABLES for the walk.
        self.table
            .iter_from(from)
            .map(|(slot, _, &ThreadControlBlockPointer(p))| (slot, unsafe { &*p }))
    }
}

struct IpcTables {
    /// The thread table: generational names over page-resident TCBs. See [`Threads`];
    /// design/kernel-objects-from-untyped.md D2 records the path, notes/tcb.md the storage.
    threads: Threads,
    /// Neither the run queue nor `current` live here any more: both moved to per-CPU storage
    /// (`cpu::PerCpu`, DECISIONS §11 steps 3a and 3b), because a single shared queue and a
    /// single "running thread" are exactly what every core would otherwise contend on and
    /// overwrite. What stays is genuinely whole-machine: the thread table and the endpoints.
    ///
    /// Every IPC rendezvous. Indexed by the `usize` inside an `Object::Rendezvous` capability, which
    /// only the kernel mints, so the index is always in range.
    /// **The rendezvous registry** (milestone 19a; design/init-and-granular-spawn.md). An rendezvous
    /// is page-resident now: it lives at the start of a page retyped from some untyped region
    /// (a process's own, via `RETYPE_OBJ`, or the kernel's, via [`create_rendezvous`]), and that
    /// region is pinned so the page can never be freed under a blocked thread. The registry
    /// entry is the page's physical address; the generational name (`crates/slots`, the same
    /// machinery as Tids) is what an `Object::Rendezvous` capability carries, so the day endpoints
    /// can die, stale names will already fail safely.
    rendezvous_table: generational_table::Table<u64, MAX_RENDEZVOUS>,
    /// The kernel's **current** object chunk: where the kernel's endpoints (boot services, tests)
    /// are retyped from, so every rendezvous lives uniformly in a pinned page regardless of who paid.
    /// Carved lazily on the first [`create_rendezvous`] and **replaced when it fills**, which is what
    /// makes the kernel's rendezvous supply grow instead of being a compile-time guess.
    ///
    /// A filled chunk's handle is deliberately forgotten. Its pages stay pinned and its endpoints
    /// stay live, and nothing ever hands a kernel chunk back: kernel endpoints are destroyed only by
    /// tearing down the region hosting them (see the `doomed_eps` walk), and no path tears down a
    /// kernel chunk. If one ever should, this becomes an array and that is the change to make.
    kernel_ep_region: Option<u64>,
    /// How many chunks have been carved, so growth is bounded by something rather than by nothing.
    kernel_ep_chunks: usize,
    /// **The notification registry** (milestone 151 (notification objects), DECISIONS §101 (notification objects)): the rendezvous registry's
    /// shape one object type over. Each entry is the physical address of the page a
    /// [`NotificationPage`] lives at the start of, retyped from its creator's region; the
    /// generational name is what an `Object::Notification` capability carries.
    notification_table: generational_table::Table<u64, MAX_NOTIFICATIONS>,
    /// **The timer registry** (milestone 106 (a wait that ends on either the interrupt or the
    /// deadline), DECISIONS §147 (a timer a userspace service cannot hold)): the notification
    /// registry's shape one object type over. Each entry is the physical address of the page a
    /// [`TimerPage`] lives at the start of; the expiry walk iterates it.
    timer_table: generational_table::Table<u64, MAX_TIMERS>,
}

/// The most endpoints that can exist **at once**: the registry's bound.
///
/// This used to say it capped creations over the kernel's lifetime, on the grounds that rendezvous
/// teardown did not exist. That went stale when object revocation made destruction real: tearing
/// down a region removes every rendezvous whose page lives in it and `generational_table::Table::remove` frees the
/// slot for reuse, so this is a concurrent bound now. Corrected rather than left, because a stale
/// bound is the kind of comment that gets believed during a capacity argument.
pub(crate) const MAX_RENDEZVOUS: usize = 512;

/// **The most rendezvous that were ever live at once on this boot**, and how many of those live at
/// that moment sat on the kernel's own chunks. [`MAX_RENDEZVOUS`]'s instrument, built for
/// `PEAK_THREADS`'s reason by milestone 601 (the region table prints its peak), after the lane of
/// milestone 152 (durable delegation) met this ceiling in `timetable_tests`: a durable test created
/// its report endpoint with [`create_rendezvous`] on every run, and kernel-chunk rendezvous are
/// never freed (see `kernel_ep_region`), so the registry filled across the suite.
///
/// The split is the ledger's first cut. A rendezvous on a kernel chunk lives for the whole boot;
/// one retyped from a region goes when its region is reclaimed. So the kernel-chunk count only
/// ever rises, and it is the part of the peak no teardown can recover.
///
/// Both updated under `IPC_TABLES` at the one insert, so they never race.
///
/// # The ledger: what holds rendezvous at the peak
///
/// Measured 2026-09-26 with a temporary per-test print, on `main` at `484f3ebe` plus #1347:
///
/// | architecture | peak | spare | on kernel chunks | retyped from a region |
/// |---|---|---|---|---|
/// | aarch64 | **505** | 7 | 466 | 39 |
/// | riscv64 | **491** | 21 | 452 | 39 |
/// | `x86_64` | **315** | 197 | 302 | 13 |
///
/// The registry starts each suite empty and the peak is its last reading: it only grows, because
/// almost everything in it sits on a kernel chunk. So the ledger is who called
/// [`create_rendezvous`], aarch64 per module (kernel-chunk count first, then all live):
///
/// | module | kernel chunks | all live | |
/// |---|---|---|---|
/// | `kernel::sched` tests | 46 | 47 | the scheduler's own IPC tests |
/// | `user::tests` | 45 | 46 | the oldest userspace tests, one or two each |
/// | `ntp_tests` | 37 | 37 | |
/// | `login_tests` | 36 | 66 | the other 30 are retyped from session regions |
/// | `compositor_tests`, `display_tests` | 34 | 34 | |
/// | `rm_program_tests`, `dir_capability_tests`, `disk_tests` | 51 | 51 | the file services |
/// | `sink_tests`, `date_tests`, `time_tests` | 40 | 40 | |
/// | 23 other modules | 97 | | |
/// | created between tests | 80 | | services still wiring after a test returned |
///
/// Nothing on a kernel chunk is returned, even when the test that asked for it hands everything
/// else back, so every test that wires a service with [`create_rendezvous`] spends the registry
/// for the rest of the boot. The fix the lane of milestone 152 used on its own test is the
/// pattern: retype the endpoint from the run's own region, so it goes when the region does.
/// `design/roadmap/0671-tests-retype-their-rendezvous-from-their-own-region.md` is that work
/// across the suite. Reports and does not gate, for [`MAX_THREADS`]'s reason.
static PEAK_RENDEZVOUS: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
static KERNEL_CHUNK_RENDEZVOUS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// `(peak live, created on kernel chunks so far)`: see [`PEAK_RENDEZVOUS`]. Printed by the test
/// suite's closing summary.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the closing summary is the only reader
pub fn rendezvous_pressure() -> (usize, usize) {
    (
        PEAK_RENDEZVOUS.load(Ordering::Relaxed),
        KERNEL_CHUNK_RENDEZVOUS.load(Ordering::Relaxed),
    )
}

/// An rendezvous's name: a generational `slots` name over the rendezvous registry (19a). What an
/// `Object::Rendezvous` capability carries. `u64` like a `ThreadId`, and stale-safe the same way.
pub type RendezvousId = u64;

/// The pages in one of the kernel's rendezvous chunks. **Not a ceiling.** When a chunk fills,
/// [`create_rendezvous`] carves another, so this is a batch size and nothing else.
///
/// It used to be a ceiling, and it was the wrong shape of number, because it grew with the SUITE
/// rather than with the system: 64 lasted until the 27+28 merge, 96 until supervision and `std::net`
/// merged the same day, 128 until milestone 33's compositor tests, which wire 26 endpoints across
/// four scenes (a display, a doorbell, a report per client, an input rendezvous per focusable client)
/// and wanted 160. Every parallel branch fit on its own, and the union
/// of their test boots is what crossed the line, which is a cost no branch can see before it merges.
/// So the failure mode was a merge-time panic telling whoever merged to raise a constant, over and
/// over, for a reason none of them caused. Growing on demand retires that whole class of papercut:
/// there is no number to raise, and the only remaining limit is [`MAX_RENDEZVOUS`], which is a real
/// bound with a real meaning.
///
/// 32 pages (128 KiB) is deliberately modest. A normal boot carves exactly one chunk and the rest of
/// the supply is never touched, which is the point of carving lazily.
const KERNEL_EP_CHUNK_PAGES: u64 = 32;

/// How many chunks the kernel will carve before refusing. Derived so the **page supply can never be
/// the binding limit before the registry is**: enough chunks to host [`MAX_RENDEZVOUS`] endpoints, one
/// page each. That is what makes exhaustion always report the honest reason (the registry is full)
/// rather than an arbitrary carve size. Derived rather than written down so the two cannot drift.
const MAX_KERNEL_EP_CHUNKS: usize = MAX_RENDEZVOUS.div_ceil(KERNEL_EP_CHUNK_PAGES as usize);

/// The rendezvous behind a name, or `None` if the name no longer resolves. Caller holds `IPC_TABLES`.
///
/// This used to panic on a miss, because endpoints could not be destroyed (their regions stayed
/// pinned), so a miss was kernel corruption. Object revocation made destruction real: a stale
/// `Rendezvous` capability (its rendezvous reclaimed out from under a holder) is now ordinary user
/// input, so this returns `None` and the callers turn that into a clean error rather than a panic.
///
/// The `'static` is the page's pinned-ness made into a lifetime: while the name resolves the page is
/// pinned and direct-mapped, and `IPC_TABLES` serializes every access to what it holds.
fn rendezvous_of(sched: &IpcTables, ep: RendezvousId) -> Option<&'static mut Rendezvous> {
    let phys = *sched.rendezvous_table.get(ep)?;
    // SAFETY: retyped exclusively for this rendezvous, its region pinned while the name resolves,
    // direct-mapped, and serialized by IPC_TABLES, which every caller holds.
    Some(unsafe { &mut *(crate::arch::mmu::phys_to_virt(phys) as *mut Rendezvous) })
}

/// Mark the current thread's blocking IPC as aborted (a stale rendezvous, or one revoked while it
/// blocked): the syscall layer reads-and-clears this after the primitive returns and hands back an
/// error. A helper because several IPC paths set it. Caller holds `IPC_TABLES`.
///
/// **`#[cold]`, and it is a claim about the callers rather than about this body.** Every call site
/// is the `else` of a `rendezvous_of` that returned `None`, which means the name a program invoked
/// no longer resolves. A healthy IPC never reaches it, and it is reached from four functions that
/// are all on `script/fastpath-footprint`'s closures, so inlining it put the abort path's bytes on
/// the fast path four times over (milestone 188 phase 3).
#[cold]
#[inline(never)]
fn set_ipc_aborted(sched: &mut IpcTables, tid: ThreadId) {
    if let Some(t) = sched.threads.get_mut(tid) {
        t.handshake.abort();
        // **The capability staged for the aborted send goes with it** (the 2026-10-03 security
        // audit's follow-up). A `SEND_CAP` or `CALL` that parked put its delegation, or the Reply
        // the kernel minted, in `outgoing_cap` for the receiver to take. An abort means no receiver
        // ever will: the rendezvous is gone. Left in place, the next plain `SEND` this thread
        // parked on a *different* rendezvous would hand that capability to whoever `RECEIVE_CAP`s
        // there, a delegation the sender made to one endpoint delivered to another. The sender
        // still holds its own copy (`SEND_CAP` narrows a copy, it never moves the source), so
        // nothing is lost by dropping this one.
        t.outgoing_cap = None;
    }
}

/// **Refuse the current thread's send, because the rendezvous carries an interrupt** (milestone 603
/// (provisional), DECISIONS §101 ruling B). An abort, so the syscall layer's existing
/// `take_ipc_aborted` branch is the only one the common path pays for, plus the reason, which only
/// that branch reads ([`take_ipc_refused`]). The sender never parked, so the abort does not weaken
/// the boot-8 gate for anything: it is taken immediately, as a stale endpoint's is.
///
/// `#[cold]` for [`set_ipc_aborted`]'s reason: no healthy IPC reaches it, and it is reached from
/// three functions on `script/fastpath-footprint`'s closures.
///
/// Name: provisional (milestone 603 (provisional)): calef names public items.
#[cold]
#[inline(never)]
fn set_ipc_refused(sched: &mut IpcTables, tid: ThreadId) {
    if let Some(t) = sched.threads.get_mut(tid) {
        t.handshake.abort();
        t.ipc_refused = true;
    }
}

/// **Read and clear why the current thread's aborted send was aborted**: `true` when the rendezvous
/// carries an interrupt and refused it ([`set_ipc_refused`]), `false` when it was stale or revoked.
/// Called by the syscall layer only after [`take_ipc_aborted`] returned `true`, so an IPC that was
/// not aborted never pays for it.
///
/// Name: provisional (milestone 603 (provisional)): calef names public items.
#[cold]
#[inline(never)]
pub fn take_ipc_refused() -> bool {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return false;
    };
    let tid = current_thread_id();
    sched
        .threads
        .get_mut(tid)
        .map(|t| core::mem::take(&mut t.ipc_refused))
        .unwrap_or(false)
}

/// **Read and clear the current thread's IPC-aborted flag** (object revocation). The syscall layer
/// calls this right after an rendezvous IPC primitive returns: `true` means the rendezvous was stale, or
/// revoked while the thread blocked on it, so the caller gets an error instead of the primitive's
/// placeholder result. Kernel-side IPC callers never set it (their endpoints are never revoked), so
/// they need not check it.
pub fn take_ipc_aborted() -> bool {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return false;
    };
    let tid = current_thread_id();
    sched
        .threads
        .get_mut(tid)
        .map(|t| t.handshake.take_aborted())
        .unwrap_or(false)
}

/// Rank **above the allocators**, because the reaper (`finish_switch`) drops a dead `Thread` in
/// its pool slot while holding this, and that drop *frees*: the kernel stack's pages go back to
/// the frame allocator through the kernel MMU lock, and the stack's VA range to its free list.
/// Freeing takes the same locks allocating does, so the rank must sit above them.
///
/// Nothing under this lock **allocates** any more (milestone 14 phase B.2): spawn writes the new
/// `Thread` into a static pool slot, and the queues have been intrusive since A.2, so a queue
/// operation is a couple of pointer writes, from the timer IRQ or anywhere else. §9's
/// no-allocation-in-IRQ rule holds by construction.
static IPC_TABLES: IrqSafeMutex<Option<IpcTables>> = IrqSafeMutex::new(rank::IPC_TABLES, None);

/// The scheduler before anything is in it, as a `const` rather than an expression [`init`] builds.
///
/// Both tables are `const fn` constructors already, so this is a `.rodata` aggregate the installer
/// copies from instead of a value assembled on the boot stack. `init`'s own comment carries the
/// measurement and why it decides how large [`MAX_THREADS`] may be.
const EMPTY_TABLES: IpcTables = IpcTables {
    threads: Threads::new(),
    rendezvous_table: generational_table::Table::new(),
    kernel_ep_region: None,
    kernel_ep_chunks: 0,
    notification_table: generational_table::Table::new(),
    timer_table: generational_table::Table::new(),
};

/// **Per-cpu ring of the last few scheduler events** (first-silicon diagnostics, 2026-08-14; the
/// module name is provisional). A boot-7 bench dump on the VisionFive 2 showed an end state no
/// legal transition sequence produces (a thread `Blocked` at a non-syscall pc; a receiver
/// `Running` as another core's current for ten seconds), and an end state alone cannot say which
/// transition wrote it. This keeps the last [`trace::DEPTH`] events each core performed, and
/// [`dump_threads`] prints them, so the *path* into the wedge is on the serial log.
///
/// Cost and honesty:
///
/// - One relaxed `fetch_add` and one relaxed store per event, on paths that already hold `IPC_TABLES`
///   or run in IRQ context with interrupts masked, so each ring has exactly one writer (its own
///   core) and no entry can tear (one `u64`).
/// - **Compiled out of `--features bench` builds**, so the benchmark numbers the tripwire watches
///   are not measuring the instrument. The board tour build carries no features and keeps it.
/// - A dump reads other cores' rings racily (drain/steal events are recorded outside `IPC_TABLES`);
///   an entry is atomic, so the worst case is an event missing from the tail, never a torn one.
///
/// The bench build gets a no-op twin of the same two-function surface (below), so the call sites
/// are identical in every configuration and nothing here needs a dead-code allow.
#[cfg(not(feature = "bench"))]
mod trace {
    use core::sync::atomic::{AtomicU64, Ordering};

    /// Events per core. 16 is enough to see the whole final approach to a wedge (a block, the
    /// wakes around it, the switch that stranded something) without turning the dump into a log.
    pub const DEPTH: usize = 16;

    /// What happened. The discriminant is packed into the entry's top byte.
    #[derive(Clone, Copy)]
    #[repr(u8)]
    pub enum Event {
        /// `schedule()` picked `tid` and marked it Running on this core.
        SwitchTo = 1,
        /// The thread running here marked itself Blocked (aux = low byte of the rendezvous name).
        BlockSelf = 2,
        /// This core moved `tid` Blocked -> Ready onto a run queue (rendezvous, reply, or abort).
        Wake = 3,
        /// This core saw `tid` still on a cpu and parked the wake (`wake_pending`).
        WakeDeferred = 4,
        /// This core completed a parked wake in `finish_switch`.
        WakeCompleted = 5,
        /// This core pushed `tid` into cpu `aux`'s inbox (placement or load-aware wake).
        PlaceRemote = 6,
        /// This core served a steal: handed `tid` to requester cpu `aux`.
        StealServe = 7,
        /// This core drained its inbox; the tid field carries the count moved.
        InboxDrain = 8,
        /// This core REFUSED a wake of `tid`: the target was parked in IPC and the waker had
        /// delivered nothing (no message, no signal, no abort). The boot-8 gate firing; on a
        /// healthy boot this event never appears, so its presence in a bench dump is the finding.
        WakeRefused = 9,
        /// This core switched into `tid`, and the core it last ran on was a different one
        /// (milestone 219). **The cross-core handoff count**, and the only honest one: a rendezvous
        /// wake makes its peer Ready on the *waker's* core, which is local by construction, so
        /// [`Event::PlaceRemote`] misses every migration a pure IPC workload performs. `aux` is the
        /// core it came from.
        ///
        /// Only ever recorded by a `soak` build, since the `last_cpu` comparison that produces it
        /// is gated (it sits in `schedule()`'s switch and cost 5.7% of `ipc_fastpath` on aarch64
        /// when it was not). The variant itself stays in every build: it is compile-time only, it
        /// costs no bytes, and removing it would leave [`dump`]'s `11 => "moved"` arm naming a
        /// number with nothing to point at.
        #[cfg_attr(not(feature = "soak_test"), allow(dead_code))]
        Migrated = 11,
        /// This core set `ipc_served` on `tid`: a delivery completed the thread's parked IPC.
        /// `aux` names the delivering site (1 send, 2 receive-collect, 3 `send_cap`, 4 `receive_cap`-collect,
        /// 5 call, 6 reply, 7 irq signal, 8 death message, 9 a notification signal to a waiter, 10 a
        /// notification signal to its bound receiver), so a bench dump answers "who served
        /// this thread" by reading the ring instead of inferring it from a frozen syscall count,
        /// which is the inference boots 7 through 9 got wrong (notes/visionfive2.md, fifth stop).
        Served = 10,
    }

    /// How many discriminants [`Event`] has, which sizes the per-core totals below.
    ///
    /// One more than the largest variant, because the discriminants start at 1 so that a zeroed
    /// ring slot is distinguishable from a recorded `SwitchTo`. Read only by the `soak`-gated
    /// counter array; a `const` costs no bytes either way, so it stays visible to every build.
    #[cfg_attr(not(feature = "soak_test"), allow(dead_code))]
    pub const KINDS: usize = 12;

    struct Ring {
        seq: AtomicU64,
        slots: [AtomicU64; DEPTH],
        /// **A total per event kind, beside the ring** (milestone 219). The ring holds sixteen
        /// events, which is the right depth for reading the final approach to a wedge and the
        /// wrong one for a soak: a refused wake three hours into an eight-hour run has scrolled
        /// out of it long before anyone looks. A counter cannot scroll.
        ///
        /// Per core, in the same cache line neighbourhood as that core's own ring, for the reason
        /// the ring is per core: one shared counter array would put a contended atomic on the IPC
        /// fastpath, and a soak whose instrument slows the thing it measures is measuring the
        /// instrument. The reader sums across cores and accepts that the sum is a moment that
        /// never quite existed, which is what [`counted`] says out loud.
        ///
        /// **Behind `feature = "soak_test"`.** Per-core rather than shared was not enough: the extra
        /// load-add-store per event, on paths the IPC fastpath runs, is part of what pushed
        /// `ipc_fastpath` 5.7% over milestone 132's bound on aarch64 when this shipped
        /// unconditionally. Only a soak build reads these, so only a soak build carries them.
        #[cfg(feature = "soak_test")]
        counts: [AtomicU64; KINDS],
    }

    #[allow(clippy::declare_interior_mutable_const)]
    const EMPTY_RING: Ring = Ring {
        seq: AtomicU64::new(0),
        slots: [const { AtomicU64::new(0) }; DEPTH],
        #[cfg(feature = "soak_test")]
        counts: [const { AtomicU64::new(0) }; KINDS],
    };

    static RINGS: [Ring; crate::cpu::MAX_CPUS] = [EMPTY_RING; crate::cpu::MAX_CPUS];

    /// Record an event on the calling core's ring. Every call site runs with interrupts masked
    /// (under `IPC_TABLES` or in IRQ context), so the owning core cannot interleave with itself.
    ///
    /// **`#[inline(never)]`, so the fast paths carry one copy rather than one per event** (milestone
    /// 758 (the IPC fast paths shrink back inside their band), provisional). Inlined, every site
    /// carried its own per-CPU read, `RINGS` bounds check and panic landing pad, sequence bump and
    /// packed store: measured on 2026-10-04 UTC, the ring index alone was the largest single line in
    /// `script/fastpath-footprint`'s closures on riscv64 and the third largest on aarch64 and x86_64,
    /// and an IPC round trip records two or three events in each of four or five functions. Out of
    /// line, a site is its argument moves and a call. The ring is a diagnostic nobody branches on,
    /// so the call's few instructions buy back several hundred bytes of every core's L1i.
    #[inline(never)]
    // In the pinned hot section: milestone 796 (pin the hot trap path's placement).
    #[cfg_attr(
        target_os = "none",
        unsafe(link_section = ".text.hot.sched.trace.record")
    )]
    pub fn record(kind: Event, tid: u64, aux: u8) {
        let ring = &RINGS[crate::cpu::id()];
        let seq = ring.seq.fetch_add(1, Ordering::Relaxed);
        // kind in the top byte, aux below it, the tid's low 48 bits under that. A tid is
        // (generation << 32) | slot with slot < 256; 48 bits keeps 16 bits of generation,
        // plenty to disambiguate in a dump.
        let entry = ((kind as u64) << 56) | ((aux as u64) << 48) | (tid & 0x0000_FFFF_FFFF_FFFF);
        ring.slots[(seq as usize) % DEPTH].store(entry, Ordering::Relaxed);
        // The running total, on this core's own line. Relaxed and non-atomic-read-modify-write in
        // effect (one writer per core), so it costs a load, an add and a store next to the two
        // stores above it. **Soak builds only**, because those three instructions are on the IPC
        // fastpath and nothing but a soak reads what they produce; see the field's own note.
        #[cfg(feature = "soak_test")]
        {
            let total = &ring.counts[kind as usize];
            total.store(
                total.load(Ordering::Relaxed).wrapping_add(1),
                Ordering::Relaxed,
            );
        }
    }

    /// **How many times `kind` has happened on this machine since boot**, summed over every core.
    ///
    /// The one number the soak's heartbeat is actually watching is `WakeRefused`
    /// (`soak`), and the reason it can be watched at all is that this is a total rather than a
    /// window: a refusal at minute nine of an eight-hour run is still in this number at hour eight.
    ///
    /// **The sum is racy and deliberately so.** Cores keep counting while it is read, so the value
    /// is not a snapshot of any single instant. Nothing here needs one: the soak asks "is this
    /// still zero" and "how much did it grow since the last beat", and both survive a reader that
    /// is a few events behind.
    #[cfg(feature = "soak_test")]
    pub fn counted(kind: Event) -> u64 {
        let mut total = 0u64;
        for ring in &RINGS {
            total = total.wrapping_add(ring.counts[kind as usize].load(Ordering::Relaxed));
        }
        total
    }

    /// Print core `cpu`'s ring, oldest first. Racy against that core's own writes, by design.
    pub fn dump(cpu: usize) {
        let ring = &RINGS[cpu];
        let seq = ring.seq.load(Ordering::Relaxed);
        if seq == 0 {
            return;
        }
        let start = seq.saturating_sub(DEPTH as u64);
        crate::print!("    core {cpu} events [{start}..{seq}):");
        for s in start..seq {
            let e = ring.slots[(s as usize) % DEPTH].load(Ordering::Relaxed);
            let (kind, aux, tid) = (e >> 56, (e >> 48) & 0xff, e & 0x0000_FFFF_FFFF_FFFF);
            let name = match kind {
                1 => "switch",
                2 => "block",
                3 => "wake",
                4 => "wake?",
                5 => "wake+",
                6 => "place",
                7 => "steal",
                8 => "drain",
                9 => "refuse",
                10 => "serve",
                11 => "moved",
                _ => "?",
            };
            crate::print!(" {name}:{tid:#x}");
            if matches!(kind, 2 | 6 | 7 | 10 | 11) {
                crate::print!("/{aux}");
            }
        }
        crate::println!();
    }
}

/// The bench build's no-op twin of [`trace`]: same names, same signatures, nothing recorded, so
/// the benchmark numbers the tripwire watches never measure the instrument and the call sites
/// need no `cfg` of their own.
#[cfg(feature = "bench")]
mod trace {
    /// Mirrors the real module's [`Event`](super::trace::Event) variants; carried only so the
    /// call sites name the same paths in both configurations.
    #[derive(Clone, Copy)]
    pub enum Event {
        SwitchTo,
        BlockSelf,
        Wake,
        WakeDeferred,
        WakeCompleted,
        PlaceRemote,
        StealServe,
        InboxDrain,
        WakeRefused,
        Served,
        #[cfg_attr(not(feature = "soak_test"), allow(dead_code))]
        Migrated,
    }

    #[inline]
    pub fn record(_kind: Event, _tid: u64, _aux: u8) {}

    /// Always zero here, because nothing is recorded. The bench boot runs no soak (both diverge
    /// before the other could start), so no caller can be misled by it.
    #[cfg(feature = "soak_test")]
    pub fn counted(_kind: Event) -> u64 {
        0
    }

    pub fn dump(_cpu: usize) {}
}

/// **Scheduler anomaly and activity totals, for a run long enough that a sixteen-event ring is no
/// use** (milestone 219). Every one is a sum over the per-core counters `trace::record` keeps; see
/// [`trace::counted`] for why the sum is racy and why that is fine.
///
/// These are the kernel's numbers, read by the kernel's soak supervisor. They are deliberately not
/// reachable from userspace: a workload that could read its own tripwire is one step from a
/// workload that could clear it.
///
/// Names provisional (this lane's, 2026-09-01; public function names are an architect's call,
/// DECISIONS naming tenet as extended on 2026-08-23).
///
/// **The one that is a finding rather than a statistic.** A refused wake means a waker made a
/// parked receiver `Ready` without delivering anything, and the gate stopped it. It has never
/// fired in the field (see `thread_wake_handshake`'s crate doc, which is honest that the boot-8
/// reading it was built against was later overturned), so a nonzero here on a board is the single
/// most interesting number this kernel can produce.
// Soak builds only (milestone 219). These were `allow(dead_code)` and compiled everywhere, on the
// argument that a function invisible to clippy rots; the measurement overruled it. The counters
// they read are `cfg`-gated because their increments are on the IPC fastpath, so an accessor
// compiled without them would not build either. `script/lint` clippies `--features soak_test` on both
// ISAs, which is what keeps them seen.
#[cfg(feature = "soak_test")]
pub fn wake_refusals() -> u64 {
    trace::counted(trace::Event::WakeRefused)
}

/// How many wakes were parked because the target was still standing on a CPU. Ordinary, and
/// expected to be nonzero under load; a soak reports it so that "the machine was genuinely
/// contended" is a number rather than an assurance.
// Soak builds only (milestone 219). These were `allow(dead_code)` and compiled everywhere, on the
// argument that a function invisible to clippy rots; the measurement overruled it. The counters
// they read are `cfg`-gated because their increments are on the IPC fastpath, so an accessor
// compiled without them would not build either. `script/lint` clippies `--features soak_test` on both
// ISAs, which is what keeps them seen.
#[cfg(feature = "soak_test")]
pub fn wakes_deferred() -> u64 {
    trace::counted(trace::Event::WakeDeferred)
}

/// How many wakes and placements landed a thread on a core other than the waker's. **This is the
/// number that says the workload actually crossed cores**, which is the whole premise of a
/// multicore soak; a run reporting zero here soaked one core very thoroughly and proved nothing
/// about the others.
// Soak builds only (milestone 219). These were `allow(dead_code)` and compiled everywhere, on the
// argument that a function invisible to clippy rots; the measurement overruled it. The counters
// they read are `cfg`-gated because their increments are on the IPC fastpath, so an accessor
// compiled without them would not build either. `script/lint` clippies `--features soak_test` on both
// ISAs, which is what keeps them seen.
#[cfg(feature = "soak_test")]
pub fn remote_placements() -> u64 {
    trace::counted(trace::Event::PlaceRemote)
}

/// **How many times a thread ran on a different core than the one it last ran on.**
///
/// The cross-core handoff number, and the one a soak reports. See [`crate::thread::Thread::last_cpu`] for
/// why [`remote_placements`] could not be it: a rendezvous wake queues its peer on the waker's own
/// core (DECISIONS §28.2), so the placement is local even though the thread has moved.
// Soak builds only (milestone 219). These were `allow(dead_code)` and compiled everywhere, on the
// argument that a function invisible to clippy rots; the measurement overruled it. The counters
// they read are `cfg`-gated because their increments are on the IPC fastpath, so an accessor
// compiled without them would not build either. `script/lint` clippies `--features soak_test` on both
// ISAs, which is what keeps them seen.
#[cfg(feature = "soak_test")]
pub fn migrations() -> u64 {
    trace::counted(trace::Event::Migrated)
}

/// How many threads this machine handed from one core's run queue to another's on request. The
/// work-steal protocol's activity level, and the second of the two cross-core paths a soak is for.
// Soak builds only (milestone 219). These were `allow(dead_code)` and compiled everywhere, on the
// argument that a function invisible to clippy rots; the measurement overruled it. The counters
// they read are `cfg`-gated because their increments are on the IPC fastpath, so an accessor
// compiled without them would not build either. `script/lint` clippies `--features soak_test` on both
// ISAs, which is what keeps them seen.
#[cfg(feature = "soak_test")]
pub fn steals_served() -> u64 {
    trace::counted(trace::Event::StealServe)
}

/// **Spawn a thread exactly as [`spawn`] would, and say which core it landed on** (milestone 240).
///
/// The placement is [`pick_spawn_target`]'s, made here in the same call `spawn` makes it in, so
/// this is `spawn` with the answer kept rather than a second placement policy. **It hands the
/// caller no lever**: there is no argument to steer the choice with, which is deliberate, because
/// the thing milestone 240 is for is reporting the boot-time placement lottery and not overriding
/// it. Thread affinity is an architect's open question and DECISIONS 138 (how a saturated workload
/// is made to hand threads across cores) declined a rebalancer; neither is reopened by a caller
/// that can only read.
///
/// Soak and job-mix builds only, for the same reason the counters above are: nothing else needs
/// it, and a function nothing calls is a function that rots. Milestone 168's sweep is the second
/// caller and it is here for the first one's exact reason, that a workload number taken on real
/// silicon is uninterpretable without the arrangement that produced it.
///
/// Name: provisional (milestone 240 (the soak reports what happened and not where)): calef names
/// public items.
#[cfg(any(feature = "soak_test", feature = "job_mix"))]
pub fn spawn_reporting_placement<F: FnOnce() + Send + 'static>(f: F) -> Option<(ThreadId, usize)> {
    let target = pick_spawn_target();
    spawn_on(target, f).map(|id| (id, target))
}

/// **Where each of `ids` last ran**, written into `out`, or `u8::MAX` for a thread that has not run
/// yet or is gone (milestone 240).
///
/// One lock acquisition for the whole set rather than one per thread, because the caller is a soak
/// heartbeat asking about every worker at once and `IPC_TABLES` is the lock the IPC fastpath takes:
/// twenty-four separate acquisitions once a beat would be twenty-four chances to sit in front of
/// the workload being measured.
///
/// `u8::MAX` means "no answer", and it deliberately means both of the two ways there can be none.
/// A thread that has never been switched to still carries `Thread::last_cpu`'s initial `u8::MAX`,
/// and a thread that has died is not in the table at all; a reader who needs to tell those apart
/// has [`dump_threads`], and a census that guessed between them would be inventing a placement.
///
/// Name: provisional (milestone 240): calef names public items.
#[cfg(feature = "soak_test")]
pub fn last_cpus(ids: &[ThreadId], out: &mut [u8]) {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        out.iter_mut().for_each(|slot| *slot = u8::MAX);
        return;
    };
    for (slot, &id) in out.iter_mut().zip(ids) {
        *slot = sched.threads.get(id).map_or(u8::MAX, |t| t.last_cpu);
    }
}

/// **The boot tour's last-reached stage**, printed in every [`dump_threads`] header (first-silicon
/// diagnostics, 2026-08-15; name provisional).
///
/// Boots 7 through 9 on the VisionFive 2 were called a hang inside the initrd demo because the
/// tour's serial lines after "init : measured, built, started" (the prefix that line carried then;
/// the demo's program is `builder` and the line says so now) never showed at the bench, while
/// the thread dumps kept printing. The dumps' own rows later proved the tour had in fact advanced
/// through the UART-driver step (notes/visionfive2.md, fifth stop), so "which step did the boot
/// thread reach" must not be inferable only from serial lines that can go missing: a breadcrumb
/// the periodic dump repeats survives a lossy or misread log. The riscv tour bumps this at each
/// step; the number-to-step table lives beside the tour in main.rs.
static BOOT_STAGE: AtomicU32 = AtomicU32::new(0);

/// Record that the boot tour reached `stage`. Monotonic by convention, not enforced.
#[cfg_attr(any(target_arch = "aarch64", target_arch = "x86_64"), allow(dead_code))] // the riscv tour is the caller today
pub fn note_boot_stage(stage: u32) {
    BOOT_STAGE.store(stage, Ordering::Relaxed);
}

/// Read the tour stage back. The hang watcher uses it to fall silent once the tour has
/// finished (stage 11, and it was 10 until milestone 159 added the hardware-entropy step after
/// what used to be the last one): boot 13 completed healthily and still printed five dumps of a
/// quiescent machine, which reads as a hang to anyone who has not memorized the watcher.
// Dead since milestone 295: the hang watcher in `riscv_initrd_demo` was the only reader, and that
// function went with the program it loaded. `note_boot_stage` above still has ten callers, so the
// breadcrumb is still WRITTEN and `dump_threads` still prints it from `BOOT_STAGE` directly; what
// has no caller is this accessor. Kept with the canary below, for the reason written there.
#[allow(dead_code)]
pub fn boot_stage() -> u32 {
    BOOT_STAGE.load(Ordering::Relaxed)
}

/// **A corruption tripwire over `IpcTables`'s registries** (first-silicon diagnostics,
/// 2026-08-15; module name provisional). Armed around the initrd demo on the board tour, it
/// re-reads the watched ranges on the timer tick and prints every byte that changed since the
/// last look: address, tick, before and after. A legal change (a spawn writing a fresh `ThreadControlBlockPointer`, a
/// reap bumping a generation) prints as a recognizable delta at a table offset; a stray write
/// prints as bytes nothing in the choreography explains. The instrument does not judge, it shows,
/// because boots 7 through 9 proved the judging is the part that goes wrong.
///
/// What it watches: the thread table and the rendezvous registry (the `slots` arrays and their
/// generations), which are quiescent between spawns and creates. The per-cpu blocks are
/// deliberately NOT watched: `ticks`, `runnable`, `current` and the queues churn on every
/// scheduler entry by design, so a checksum there measures the scheduler working, not corruption.
///
/// Cost and honesty:
///
/// - Unarmed (every build's steady state), the tick-path cost is one relaxed load.
/// - Armed, the owner core re-reads ~13 KiB per tick. Diagnostic-build money, spent only inside
///   the demo window on the board tour.
/// - Compiled out of `--features bench` builds exactly as the event rings are, so the tripwire
///   benchmarks never measure the instrument.
/// - The watched memory is concurrently mutated under `IPC_TABLES` while the check reads it lock-free;
///   a torn read of an in-flight legal write can print as a divergence. That is a false alarm
///   only in the sense that the mutation was legal; the printed delta says so itself.
/// - The instrument's own state (the watch table, the shadow) is serialized by
///   `memory_corruption_canary_gate::Gate`, a one-word state machine with loom-searched guards. Its first protocol
///   was two hand-written flags here, and that pair raced (2026-08-15): a `check` that lost the
///   single-flight slot returned silently having checked nothing, which the kernel test read as
///   a missed corruption (the thead-c906 flake, notes/cpu-models.md BUGS), and a re-arm could
///   rewrite the plan under a checker that had seen `ARMED` but not yet won the slot. See
///   `crates/memory_corruption_canary_gate` for both holes and the harnesses that falsify the old spelling.
#[cfg(not(feature = "bench"))]
// **No caller since milestone 295, and kept deliberately** (2026-09-14). Its one consumer was the
// hang watcher inside `kernel::user::riscv_initrd_demo`, which armed it around that demo's blocking
// receive; calef retired the program that demo loaded and the function went with it. This is the
// `AGENTS.md` "an exception is allowed and must say so" case, written where a reader meets it: the
// instrument stays because it is how a board hang gets diagnosed, it was the evidence that
// overturned the VisionFive 2 "hang" (notes/visionfive2.md, fifth stop), its own serialization is
// loom-checked in `crates/memory_corruption_canary_gate`, and re-deriving it at a bench at 2am is
// the cost this avoids. Unarmed it is one relaxed load on the tick path, which is what makes
// keeping it cheap. Re-point it at a window that can hang and delete this note.
#[allow(dead_code)]
mod canary {
    use core::sync::atomic::{AtomicU64, Ordering};

    use memory_corruption_canary_gate::Gate;

    /// One watched range and where its shadow copy lives.
    #[derive(Clone, Copy)]
    struct Watch {
        base: usize,
        len: usize,
        shadow_off: usize,
    }

    const MAX_RANGES: usize = 4;
    /// Both registries today total ~13 KiB (128 `Option<ThreadControlBlockPointer>` at 16 bytes, 512 `Option<u64>`
    /// at 16 bytes, plus generations); 24 KiB leaves room for growth and the test's scratch.
    const SHADOW_BYTES: usize = 24 * 1024;
    /// Print at most this many diverging bytes over an armed window, so a large legal rewrite
    /// cannot flood the serial log that the dump itself needs.
    const PRINT_CAP: u64 = 48;

    /// Interior-mutable statics whose one owner at a time is a live `memory_corruption_canary_gate` guard:
    /// `arm` writes them holding an `ArmGuard`, `check` reads and writes them holding a
    /// `CheckGuard`, and the gate admits at most one guard of either kind (the exclusion is
    /// loom-checked in `crates/memory_corruption_canary_gate`, where the previous hand-written spelling of this
    /// serialization is also falsified).
    struct Racy<T>(core::cell::UnsafeCell<T>);
    // SAFETY: access is serialized by the gate's guards; see the struct comment.
    unsafe impl<T> Sync for Racy<T> {}

    static GATE: Gate = Gate::new();
    static DIVERGED: AtomicU64 = AtomicU64::new(0);
    static PRINTED: AtomicU64 = AtomicU64::new(0);
    static WATCHES: Racy<([Watch; MAX_RANGES], usize)> = Racy(core::cell::UnsafeCell::new((
        [Watch {
            base: 0,
            len: 0,
            shadow_off: 0,
        }; MAX_RANGES],
        0,
    )));
    static SHADOW: Racy<[u8; SHADOW_BYTES]> = Racy(core::cell::UnsafeCell::new([0; SHADOW_BYTES]));

    /// Arm over `ranges`, snapshotting their current bytes. Caller guarantees the ranges stay
    /// readable while armed (ours are `'static` kernel tables). Serializes itself: any in-flight
    /// check finishes before the plan is touched, and the new plan is published whole. Spins, so
    /// call from thread context (both callers do); the tick side never spins, so a tick landing
    /// mid-arm skips rather than deadlocks.
    pub fn arm(ranges: &[(usize, usize)]) {
        let guard = GATE.arm();
        DIVERGED.store(0, Ordering::Relaxed);
        PRINTED.store(0, Ordering::Relaxed);
        // SAFETY: the ArmGuard is exclusive ownership of these statics (the gate's contract,
        // loom-checked in crates/memory_corruption_canary_gate); no check pass can start until it drops.
        let (watches, count) = unsafe { &mut *WATCHES.0.get() };
        // SAFETY: as above.
        let shadow = unsafe { &mut *SHADOW.0.get() };
        let mut off = 0usize;
        let mut n = 0usize;
        for &(base, len) in ranges.iter().take(MAX_RANGES) {
            assert!(off + len <= SHADOW_BYTES, "canary shadow too small");
            for i in 0..len {
                // SAFETY: caller promises base..base+len readable; volatile because another
                // core may be mid-write under IPC_TABLES (a torn snapshot only costs a printed delta).
                shadow[off + i] = unsafe { core::ptr::read_volatile((base + i) as *const u8) };
            }
            watches[n] = Watch {
                base,
                len,
                shadow_off: off,
            };
            off += len;
            n += 1;
        }
        *count = n;
        drop(guard); // the release store that publishes the plan
    }

    /// Disarm, and QUIESCE: when this returns, no check pass is mid-flight and none can start,
    /// so the caller may repurpose the watched memory. (Today's watched ranges are `'static`, so
    /// the quiescence buys certainty rather than papers over a lifetime; it costs a bounded spin
    /// while at most one in-flight pass finishes.)
    pub fn disarm() {
        GATE.disarm();
    }

    /// How many bytes have diverged since arming. The test hook, and a bench-note number.
    #[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // release builds read it off the serial print
    pub fn divergences() -> u64 {
        DIVERGED.load(Ordering::Relaxed)
    }

    /// Re-read every watched byte against the shadow; print and absorb what changed. Called from
    /// the timer tick (IRQ context, interrupts masked) and from the test. Single-flight, so a
    /// slow check on one core and the next tick on another cannot interleave shadow updates.
    ///
    /// Split in two so the disarmed tick costs no stack. A debug-build prologue reserves the
    /// WHOLE frame before the first instruction of the body runs, early return included, and the
    /// one-piece spelling of this function carried a 592-byte frame onto the interrupted thread's
    /// stack on every tick of every thread, disarmed or not. That frame was one of the middle
    /// frames of the 2026-08-15 thread-stack overflow (thread.rs, `STACK_PAGES`); the tick path
    /// pays ~16 bytes now, and only an armed pass pays for the real work.
    ///
    /// Returns whether a full pass RAN. `false` means disarmed, mid-arm, or another core's pass
    /// holds the slot. The tick ignores the answer (a sampling instrument may skip a beat); a
    /// caller that must observe a completed pass loops until it gets `true`. Returning the
    /// refusal instead of swallowing it is the fix for the c906 flake: the test's decisive check
    /// used to lose the slot to a tick's pass that had read the byte before the flip, and its
    /// silent no-op read as a missed corruption.
    pub fn check() -> bool {
        if !GATE.is_armed_hint() {
            return false; // one relaxed load: every unarmed tick's whole cost
        }
        check_armed()
    }

    /// The armed pass, outlined. `#[inline(never)]` is what keeps [`check`]'s frame from
    /// swallowing this one's; without it the split is cosmetic.
    #[inline(never)]
    fn check_armed() -> bool {
        let Some(guard) = GATE.try_check() else {
            return false;
        };
        // SAFETY: the CheckGuard is exclusive ownership of these statics (the gate's contract,
        // loom-checked in crates/memory_corruption_canary_gate), and taking it saw the arm guard's release, so the
        // plan is whole, never torn.
        let (watches, count) = unsafe { &*WATCHES.0.get() };
        // SAFETY: as above, and mutation is confined to the guard's lifetime.
        let shadow = unsafe { &mut *SHADOW.0.get() };
        let tick = crate::arch::timer::ticks();
        for w in watches.iter().take(*count) {
            for i in 0..w.len {
                // SAFETY: the armed range is 'static kernel memory (arm's contract); volatile
                // because IPC_TABLES-holding writers mutate it concurrently and honestly.
                let now = unsafe { core::ptr::read_volatile((w.base + i) as *const u8) };
                let was = shadow[w.shadow_off + i];
                if now != was {
                    DIVERGED.fetch_add(1, Ordering::Relaxed);
                    if PRINTED.fetch_add(1, Ordering::Relaxed) < PRINT_CAP {
                        crate::println!(
                            "    canary: tick={tick} addr={:#x} (range {:#x}+{:#x} off {:#x}) {was:#04x} -> {now:#04x}",
                            w.base + i,
                            w.base,
                            w.len,
                            i,
                        );
                    }
                    shadow[w.shadow_off + i] = now;
                }
            }
        }
        drop(guard); // the release store the next pass's acquire pairs with
        true
    }
}

/// The bench build's no-op twin of [`canary`], so the call sites carry no `cfg` of their own.
#[cfg(feature = "bench")]
mod canary {
    pub fn arm(_ranges: &[(usize, usize)]) {}
    pub fn disarm() {}
    pub fn check() -> bool {
        false
    }
}

/// Arm the [`canary`] over the thread table and the rendezvous registry, snapshotting under `IPC_TABLES`
/// so the baseline is a consistent cut. The riscv initrd demo arms before parking in its receive and
/// disarms when the receive returns; see notes/visionfive2.md (fifth stop) for what boot 11 does
/// with the output. **No caller since milestone 295**; see the note on `mod canary`.
#[allow(dead_code)]
pub fn canary_arm_registries() {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    let threads = (
        core::ptr::from_ref(&sched.threads.table) as usize,
        size_of_val(&sched.threads.table),
    );
    let endpoints = (
        core::ptr::from_ref(&sched.rendezvous_table) as usize,
        size_of_val(&sched.rendezvous_table),
    );
    canary::arm(&[threads, endpoints]);
}

/// Disarm the [`canary`]. The demo window's other bracket. Quiesces: when it returns, no check
/// pass is in flight on any core. **No caller since milestone 295**; see the note on `mod canary`.
#[allow(dead_code)]
pub fn canary_disarm() {
    canary::disarm();
}

/// Adopt the context we are already running in as thread 0.
///
/// It has no stack of its own and no saved context. **The first switch *away* from it fills
/// that in**, which is why the boot thread needs no special case: a thread's context is written
/// by the act of leaving it.
pub fn init() {
    // **The floating-point unit is shut on this core before any thread exists**
    // (milestone 447 (a thread's vector registers are its own)).
    //
    // Here rather than in `arch::init`, and the reason is a real trap rather than taste. What the
    // invariant is *about* is threads: `crate::fp` marks a thread `live` when it takes the first-use
    // trap, and a core that started with the unit already open never produces one, so every thread
    // on it stays `live == false` and two of them quietly share a register file. This is the
    // function where threads begin to exist, so the property and the mechanism are in the same
    // place. `arch::init` looked like the obvious home and is not one: **RISC-V's boot hart never
    // calls it.** `main`'s RISC-V tour installs `stvec` with `arch::exceptions::init()` directly,
    // reaches `sched::init` and never passes through `arch::init` at all, and OpenSBI hands the
    // kernel a hart with `sstatus.FS` already set. That cost this milestone a red suite and is
    // exactly the shape of failure AGENTS.md's ladder is about: it worked on two architectures and
    // was invisible on the third.
    crate::arch::fp::init();

    // **The machine statistics page, before the first thread** (milestone 126, DECISIONS §225 (`free` sees the machine and your share)),
    // for `fp::init`'s reason: this is where threads begin, so it is where the counters that watch
    // them begin, on all three architectures through the one function each boot path calls.
    crate::machine_statistics::publish();

    let mut sched = IPC_TABLES.lock();

    // **Install the empty tables FIRST, then name the boot thread through them**, rather than
    // building a `Threads` as a local and moving it in afterwards. That ordering is a stack
    // measurement rather than a preference, and it is what lets [`MAX_THREADS`] be raised at all.
    //
    // `IpcTables` is two generational tables and nothing else that matters, and the thread table
    // is `MAX_THREADS` slots wide. Built as a local and then moved into the `Option`, an
    // unoptimised build carries the table twice on the boot stack, so **every slot added to
    // `MAX_THREADS` cost about 80 bytes of boot stack** rather than the ~40 the table itself is.
    // Measured 2026-08-27: at 128 slots this function's frame was 43,952 bytes, the deepest in
    // the kernel and the reason the suite's boot-stack high-water sat at 54,336 of 65,504 (82%);
    // a probe raise to 256 took the high-water to 64,640 (98%) and failed `stack::report_high_water`'s
    // own gate, in a run where nothing else about the boot had changed. Assigning
    // [`EMPTY_TABLES`], a `const`, gives the compiler a `.rodata` aggregate to copy from and
    // leaves the boot thread's insert to go straight into the installed table.
    *sched = Some(EMPTY_TABLES);
    let tables = sched
        .as_mut()
        .expect("the tables were just installed on this line");
    // The table names the boot thread at insert. The first name a fresh table mints is 0 by
    // construction (slot 0, generation 0), so "the boot thread is tid 0" survives, now as a
    // property of the table rather than a hardcoded key.
    let boot_tid = tables
        .threads
        .insert_with(|tid| {
            let mut boot = Thread::boot();
            boot.id = tid;
            boot
        })
        .expect("a fresh table refused its first insert");

    // This core (core 0) is running the boot thread.
    let boot_tcb = tables
        .threads
        .pointer(boot_tid)
        .expect("the boot thread was just inserted");
    // SAFETY: `boot_tid`'s page pointer from the table, live.
    unsafe { set_current(boot_tid, boot_tcb) };

    drop(sched); // release before spawning, which takes the lock itself

    // (The run queue and inbox used to have capacity reserved here, so a push from the timer IRQ
    // could never allocate. The queues are intrusive now: a push is two pointer writes and
    // *cannot* allocate, so there is nothing to reserve. §9's rule became structural.)

    // The idle thread. Its entire body is "wait for an interrupt, then let the scheduler look for
    // work." It is deliberately kept OUT of the ready queue (see cpu::PerCpu::idle): the scheduler picks it
    // only when nothing else is runnable, so it never steals a turn from real work.
    //
    // Built on its own TCB page, as `spawn_on` builds every other kernel thread (milestone 124 (a
    // thread is born where it lives: the spawn path's copies)), rather than as a value carried
    // there: the by-value `Thread::spawn` this used held three `Thread`s in one frame in an
    // unoptimised build, 4368 bytes once the capability table grew to 32 slots, over the 4096-byte
    // guard page (milestone 126 (the `procps` package), 2026-09-27, UTC).
    let mut sched = IPC_TABLES.lock();
    let s = sched.as_mut().unwrap();
    let idle_id = s
        .threads
        .insert_in_place(|tid, dst| {
            // SAFETY: `dst` is the fresh, exclusively-ours TCB page `insert_in_place` claimed.
            unsafe { Thread::spawn_into(|| run_idle(), tid, dst) }
        })
        .expect("could not create the idle thread (no kernel stack, no TCB page, or a full table)");
    drop(sched);
    // NOT pushed onto `ready`: the idle thread is a fallback, not a peer.
    cpu::current().idle.store(idle_id, Ordering::Relaxed);
}

/// Make **this (secondary) core** a scheduler participant.
///
/// The boot core is set up by [`init`]; a secondary calls this once, as it comes online. It adopts
/// the context it is already running on as this core's idle thread (`cpu::current`/`cpu::idle`), and
/// reserves this core's run queue so `schedule()`'s push never allocates from the timer IRQ (§9),
/// exactly as `init` does for the boot core. After this, the core's run queue is empty, so it runs
/// its idle thread until work lands on the queue.
///
/// Interrupts must be masked (the caller has not enabled them yet), which is what `with_runq` needs.
pub fn adopt_secondary_idle() {
    // This core's turn at the line in `init` above: it is about to own threads.
    crate::arch::fp::init();

    let id = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard
            .as_mut()
            .expect("adopt_secondary_idle before sched::init");
        let id = sched
            .threads
            .insert_in_place(|tid, dst| {
                // SAFETY: `dst` is a fresh, exclusively-owned TCB page, per `insert_in_place`.
                unsafe { Thread::write_adopted_current(dst, tid) };
                true
            })
            .expect("thread table full while bringing a core online");
        // This core is currently running that thread.
        let tcb = sched
            .threads
            .pointer(id)
            .expect("the idle thread was just inserted");
        // SAFETY: `id`'s page pointer from the table, live.
        unsafe { set_current(id, tcb) };
        id
    };

    // And it is also this core's idle fallback.
    cpu::current().idle.store(id, Ordering::Relaxed);
    // (No queue capacity to reserve: the queues are intrusive and a push cannot allocate.)
}

/// The reschedule / migration SGI. When one core hands another a thread (via its inbox), it fires
/// this at the target; the target's handler drains its inbox and reschedules. INTID 0, distinct
/// from the rendezvous-bound test SGIs (1 and 2). SMP step 3c.
///
/// aarch64 only in practice: RISC-V's twin path (`arch/riscv64/exceptions.rs`) recognises the IPI
/// from the SBI software-interrupt cause rather than from an interrupt id, so it needs no constant,
/// and `x86_64` sends its own reschedule IPI (`arch/x86_64/irq.rs`) rather than reading one.
#[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
pub const RESCHED_SGI: u32 = 0;

/// Drain this core's migration inbox into its run queue, and request a reschedule.
///
/// Called from the reschedule-SGI handler: another core pushed one or more threads into our inbox
/// and poked us. We move them onto our own (single-owner) run queue and set `need_resched`, so the
/// handler's tail runs `schedule()` and picks them up. IRQ context, so interrupts are masked, which
/// is what `with_runq` needs; we hold nothing else, so taking the inbox is rank-safe (§11).
pub fn drain_inbox() {
    // Every caller is a cross-core interrupt arm (one per architecture), so this is where the
    // machine statistics page counts them (milestone 126).
    crate::machine_statistics::interrupt();
    let mut moved = 0u64;
    let mut inbox = cpu::current().inbox.lock();
    while let Some(thread) = inbox.pop_front() {
        // The token the inbox hands back goes straight onto the run queue. Nothing is dereferenced:
        // the handoff is pure token movement, which is why this needs no `IPC_TABLES`.
        cpu::current().with_runq(|q| q.push_back(thread));
        moved += 1;
    }
    // The inbox is empty now; mirror that under the lock (DECISIONS §28). The threads moved into the
    // run queue, whose own mirror `with_runq` just updated, so the total load is unchanged.
    cpu::current().note_inbox_len(inbox.len());
    // And count them: this is the only place a thread crosses from a remote core's hands into this
    // core's queue, which makes it the one honest observation point for "the placement arrived".
    cpu::current().note_adopted(moved);
    drop(inbox);
    if moved > 0 {
        trace::record(trace::Event::InboxDrain, moved, 0);
        cpu::current().need_resched.store(true, Ordering::Relaxed);
    }
}

/// The raw TCB pointer of a live thread, for queueing (milestone 14 phase A.2). Caller holds
/// `IPC_TABLES`.
///
/// The pointer's validity while queued is the queue discipline, stated once here: a thread on a
/// run queue or inbox is `Ready`, a thread on an rendezvous wait queue is `Blocked` (A.3), the
/// reaper frees only `Finished` threads, and a thread is never two of those at once. The `Box` in
/// the table pins the address (see `IpcTables::threads`), so a pointer taken here is good until
/// the thread is popped, however many queue hops (inbox to run queue) it makes in between.
/// The queue-able pointer to a live thread.
///
/// Returns [`core::ptr::NonNull`] rather than `*mut`, and that is not decoration: the pointer is derived from a
/// `&mut Thread` handed out by the thread table, so **non-nullness is a fact of construction rather
/// than a promise the caller keeps**. Saying so in the type removes null from the intrusive queue's
/// safety contract entirely, which is one of the two things CodeQL's `rust/access-invalid-pointer`
/// alerts were pointing at (milestone 45). What the type still cannot express is that the pointee
/// outlives its time on the queue; that is the caller's rule 2, and no type available here can carry
/// it for an intrusive structure.
fn thread_control_block_ptr(sched: &mut IpcTables, tid: ThreadId) -> core::ptr::NonNull<Thread> {
    core::ptr::NonNull::from(
        sched
            .threads
            .get_mut(tid)
            .expect("thread_control_block_ptr of a dead thread"),
    )
}

/// **Take `tid`'s queue token off its own TCB, to put it on a queue** (milestone 139 (drive the
/// unsafe count down), round 10). Caller holds `IPC_TABLES`.
///
/// The token is there exactly when the thread is on no queue (see `thread::Thread::own_token`), so
/// a `None` here is a thread somebody is about to put on a second queue ([`missing_token`]).
#[inline(always)]
fn take_token(sched: &mut IpcTables, tid: ThreadId) -> Unqueued<Thread> {
    // Two `let`-`else`s rather than a combinator chain: the icount gate measures a debug build,
    // where each closure is a call.
    let Some(t) = sched.threads.get_mut(tid) else {
        missing_token()
    };
    let Some(token) = t.own_token.take() else {
        missing_token()
    };
    token
}

/// **A thread that should hold its token does not**: it is on a queue already (and is about to be
/// put on a second), or it is a running thread whose token went somewhere it should not. Loud in
/// every build, because the alternative is the corrupted list the token exists to rule out.
///
/// One cold function for every site rather than an `expect` at each, measured rather than assumed:
/// an `expect` sets up a message and a location inline, and seven of them were the larger part of
/// what the token added to `script/fastpath-footprint`'s closures.
#[cold]
#[inline(never)]
fn missing_token() -> ! {
    panic!("a thread without its queue token: on a queue already, or its token was lost")
}

/// **Give a token a queue just handed back to the thread it names, and say which thread that is.**
/// Every pop and removal in this file ends here or on another queue, so the thread holds its token
/// again before anything decides what happens to it. Caller holds `IPC_TABLES`.
///
/// This is where the old sites' `(*waiter.as_ptr()).id` went: they read the id through the pointer
/// a queue returned, and so does this, once.
#[inline(always)]
fn hold_token(token: Unqueued<Thread>) -> ThreadId {
    // SAFETY: a token's thread is live for as long as the token exists (`Threads::mint_token`'s
    // first obligation), `IPC_TABLES` (held by every caller) serializes access to every `Thread`,
    // and no other reference to this one is live: the caller has just taken it off a queue.
    let t = unsafe { &mut *token.as_ptr() };
    t.own_token = Some(token);
    t.id
}

/// Put an already-created thread onto core `target`'s run queue. Caller holds `IPC_TABLES`.
///
/// Local: straight onto our own queue (`IPC_TABLES` masks interrupts, which `with_runq` needs). Remote:
/// into the target's inbox, and the SGI (sent after `IPC_TABLES` is released, by the caller) makes it
/// drain. The inbox push under `IPC_TABLES` is rank-safe (INBOX < `IPC_TABLES`), and the inbox's own lock supplies
/// the release/acquire that orders our thread-table insert before the target's drain (§11).
///
/// **Returns the core that owes an SGI**, `Some(target)` when the thread went into a remote inbox
/// and `None` when it went onto this core's own run queue, and that return value is the whole
/// point rather than a convenience. A caller must not decide "was this remote?" a second time by
/// comparing `target` against [`cpu::id()`] again: this function runs under `IPC_TABLES`, which
/// masks interrupts, while the caller's second comparison does not, so the calling thread can be
/// preempted and **stolen onto a different core** in between and the two answers disagree. When
/// they disagree in the direction that skips the SGI, the placed thread sits `Ready` in a remote
/// core's inbox that nothing will ever drain (only the reschedule-SGI handler calls
/// [`drain_inbox`]), the idle target refuses to steal because [`cpu::PerCpu::runnable`] counts that
/// inbox as its own work, and the machine wedges with every core idle. That is the 2026-08-28
/// riscv64 CPU-matrix hang, whose trace ring caught the migration in the act:
///
/// ```text
/// core 2: ... switch:0x0 ... switch:0x1000000076 steal:0x0/3 ...   gave tid 0 away to core 3
/// core 3: ... drain:0x1 switch:0x0 place:0x500000077/2 block:0x0/177 switch:0x4
/// ```
///
/// Tid 0 read `target == cpu::id()` on core 2, was preempted and stolen to core 3, and finished the
/// same `spawn_on` there: the push went to core 2's inbox and the stale "local" answer skipped the
/// poke.
///
/// **A skipped SGI is usually invisible, which is what makes it dangerous.** The next SGI aimed at
/// that core for any reason drains the whole inbox, so a strand is normally repaired within
/// milliseconds and nothing is ever seen. It wedges only when the stranded thread is the work
/// everything else was about to wait on, so no further SGI is generated. Do not take "it has not
/// hung" as evidence that a placement path pokes correctly. See notes/scheduler.md.
#[must_use = "a remote placement owes a reschedule SGI once IPC_TABLES is released, or the thread \
              sits in an inbox nothing drains"]
fn place_on(target: usize, thread: Unqueued<Thread>) -> Option<usize> {
    // A REMOTE parked cpu's inbox is drained by nothing, so placing there is a thread nothing
    // will ever run: the VisionFive 2 first-silicon hang (notes/visionfive2.md, third stop). The
    // online-set sweep removed every count-as-index chooser, and this is the audit lane's
    // tripwire for the next one: loud in debug/test builds, where every merge boots it. The
    // release board build compiles it out and keeps [`dump_threads`]'s parked-inbox line as the
    // field diagnostic.
    //
    // Placing onto ONESELF is exempt, and the exemption is load-bearing, found by this assertion
    // firing on the suite's own boot: a secondary's bring-up probe (`smp::secondary_main` step 6)
    // spawns onto its own core one step before that core sets its online bit, and a local
    // placement goes straight onto the running core's own run queue, which that core drains by
    // definition. The hazard this guards is exactly the remote case.
    debug_assert!(
        target == cpu::id() || {
            let mask = crate::smp::online_harts_mask();
            mask & (1 << target) != 0
        },
        "placement onto parked cpu {target} (online mask {:#b}): nothing drains a parked core's \
         inbox, so this thread would never run",
        crate::smp::online_harts_mask(),
    );
    if target == cpu::id() {
        cpu::current().with_runq(|q| q.push_back(thread));
        None
    } else {
        // Read before the push gives the token away. SAFETY: a token's thread is live while the
        // token exists (`Threads::mint_token`), and IPC_TABLES (held) serializes every `Thread`.
        let tid = unsafe { (*thread.as_ptr()).id };
        // The inbox mutex serializes access to the link.
        let mut inbox = cpu::inbox_of(target).lock();
        inbox.push_back(thread);
        // Mirror the target's inbox depth so it counts as load (DECISIONS §28); under the lock, so
        // the store is serialised with any concurrent drain.
        cpu::of(target).note_inbox_len(inbox.len());
        trace::record(trace::Event::PlaceRemote, tid, target as u8);
        Some(target)
    }
}

/// Spawn a thread and place it on a **specific** core (SMP step 3c).
///
/// The cross-core placement primitive. `spawn` puts work on the calling core; this puts it on
/// `target`, which is what lets the machine actually spread load. A remote target is handed the
/// thread through its inbox and then poked with the reschedule SGI. (Wiring `spawn` itself to
/// round-robin over `target` is the trivial next step, once the mechanism is proven.)
pub fn spawn_on<F: FnOnce() + Send + 'static>(target: usize, f: F) -> Option<ThreadId> {
    let (id, remote) = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut()?;
        // **The Thread is built on its own TCB page, not carried there** (milestone 124). The old
        // shape called `Thread::spawn(f)` for a value and moved it through this closure, and every
        // instantiation of this generic function carried 3888 to 4592 bytes of frame as a result:
        // over the 4096-byte guard page, which is the size at which one frame can step past the
        // guard in a single move and corrupt the neighbouring stack with no fault at all.
        let id = sched.threads.insert_in_place(|tid, dst| {
            // SAFETY: `dst` is the fresh, exclusively-ours TCB page `insert_in_place` claimed; it
            // is aligned for `Thread` and holds no live one, so `write` drops nothing.
            unsafe { Thread::spawn_into(f, tid, dst) }
        })?;
        // Record where it was placed, so a survey can report it (`abi::survey::record::PLACEMENT`).
        // Here rather than inside `place_on`, and that siting is the whole cost argument: `place_on`
        // is also the wake path, so a store there would be a store per wake on the IPC fastpath,
        // which is what put `Thread::last_cpu` behind a soak-build feature. One store per thread
        // creation is free by comparison. `insert_in_place` just returned this id, so the `if let`
        // is belt and braces rather than a case that happens.
        if let Some(t) = sched.threads.get_mut(id) {
            t.placement = target as u8;
        }
        // The placement decision is made ONCE, here, with interrupts masked, and carried out of
        // the critical section as a value. Re-deriving it below from `target != cpu::id()` is the
        // lost-wakeup bug `place_on` documents: this thread can be stolen onto another core
        // between the two reads.
        let remote = place_on(target, take_token(sched, id));
        (id, remote)
    }; // IPC_TABLES released here, before the SGI, so the target's schedule() can take it

    if let Some(target) = remote {
        // Poke the target: its handler drains the inbox we just pushed to and reschedules.
        crate::arch::irq::send_reschedule(target);
    }
    Some(id)
}

pub fn spawn<F: FnOnce() + Send + 'static>(f: F) -> Option<ThreadId> {
    // Placement is the power of two choices (DECISIONS §28): the new thread lands on the lighter of
    // two randomly sampled cores, not always on the spawner's, so work spreads instead of piling on
    // one core beside idle ones (the FS-server starvation lesson). `spawn_on` carries the thread to
    // the chosen core over the §11 inbox/SGI path.
    spawn_on(pick_spawn_target(), f)
}

/// **Power of two choices: which core should a new thread run on** (DECISIONS §28.1). Sample two
/// random cores' runnable counters (relaxed, possibly stale, which §28 accepts) and return the
/// lighter. Near-optimal balancing that reads at most two remote counters no matter how many cores
/// there are, where a full least-loaded scan would contend on every counter and age badly. On a
/// single online core it is a no-op; the two samples may coincide, degrading to one choice harmlessly.
fn pick_spawn_target() -> usize {
    let n = crate::smp::online_count();
    if n <= 1 {
        return cpu::id();
    }
    // The k-th ONLINE cpu, not index k: the online set is not contiguous from zero on real boards
    // (first-silicon bench, 2026-08-14: {1,2,3} online, and modulo-count placed init into parked
    // slot 0's inbox forever). See smp::online_cpus.
    let a = crate::smp::nth_online(cpu::current().rng_next() as usize);
    let b = crate::smp::nth_online(cpu::current().rng_next() as usize);
    if cpu::of(a).runnable() <= cpu::of(b).runnable() {
        a
    } else {
        b
    }
}

/// **Serve a pending work-steal request** (DECISIONS §28.3), at a scheduler entry where interrupts
/// are masked (the reschedule-SGI handler). If an idle core asked this core for work, hand it one
/// thread from our run queue and poke it. We give from the *queue*, never the thread on the CPU, and
/// only if we have one to spare; an empty give leaves the requester to ask again next tick, the
/// bounded cost §28 accepts. Pull-based and lock-free between run queues: the only shared structure
/// touched is the requester's inbox.
pub fn serve_steal_request() {
    // Take the request, which also clears the slot so the next idle core can queue a fresh one. The
    // acquire/release pairing and the read-and-clear live in `steal_request`, where loom checks
    // them (notes/interleaving.md); this function spends its own code on the hand-off.
    let Some(requester) = cpu::current().steal_request.take() else {
        return;
    };
    let requester = requester as usize;
    // One thread off our own queue, if any. `with_runq` keeps the runnable mirror exact.
    let thread = cpu::current().with_runq(|q| q.pop_front());
    if let Some(t) = thread {
        // SAFETY: reading the id of the thread we just popped and hold exclusively.
        let tid = unsafe { (*t.as_ptr()).id };
        trace::record(trace::Event::StealServe, tid, requester as u8);
        // The token we just popped goes to the requester's inbox; the inbox mutex serialises the
        // handoff and orders our pop before the requester's drain (the `place_on` discipline).
        let mut inbox = cpu::inbox_of(requester).lock();
        inbox.push_back(t);
        cpu::of(requester).note_inbox_len(inbox.len());
        crate::arch::irq::send_reschedule(requester);
    }
}

/// **An idle core asks a loaded core for work** (DECISIONS §28.3), from the idle loop. Pick the
/// most-loaded other core and, if it has a queued thread to spare, request one over its steal slot
/// and poke it with the reschedule SGI; the victim serves it at its next scheduler entry, the stolen
/// thread lands in our inbox, and that SGI's drain runs it. One outstanding request per victim
/// (`work_steal_slot::Slot::claim` is a compare-exchange from empty), so a crowd of idle cores
/// collapses to one steal per victim per round.
fn try_initiate_steal() {
    // Do not steal if we have work of our own arriving: our run queue is empty (that is why the idle
    // thread is running), but the inbox may hold threads a remote just handed us that our own next
    // scheduler entry will drain. `runnable` counts those, so this guard defers to our own work.
    if cpu::current().runnable() > 0 {
        return;
    }
    let me = cpu::id();
    let mut victim = None;
    let mut best = 0usize;
    for c in crate::smp::online_cpus() {
        if c != me {
            // Steal only a run-queue backlog, never a victim's inbox in transit (see `runq_len`).
            let r = cpu::of(c).runq_len();
            if r > best {
                best = r;
                victim = Some(c);
            }
        }
    }
    if let Some(v) = victim
        && cpu::of(v).steal_request.claim(me as u32)
    {
        crate::arch::irq::send_reschedule(v);
    }
}

/// **The idle thread's body**, shared by the boot core's spawned idle thread and every secondary's
/// adopted idle context. Each pass: try to steal work from a loaded core (§28), park in `wfi` until
/// an interrupt (the stolen thread's SGI, a spawn's SGI, or the tick), then yield so the scheduler
/// runs whatever arrived. Never returns; it is the fallback the scheduler picks only when this
/// core's run queue is empty.
///
/// Before an idle thread existed, a moment where every thread was blocked waiting for I/O was a
/// kernel panic. It is never in the ready queue, so it never competes with real work, and it is
/// per-CPU as of §11 step 3b, so an idle core parks in its own `wfi`.
///
/// **It deliberately does not drain its own inbox before parking** (DECISIONS §133, calef,
/// 2026-08-28). Doing so would make a missed reschedule-SGI self-healing, and that was refused on
/// purpose: `drain_inbox` has one caller per architecture, the SGI handler, so a missed poke is
/// permanent rather than late, and a permanent wedge announces itself as a watchdog dump naming
/// the stranded thread and the undrained inbox. A self-healing drain would turn the same defect
/// into threads occasionally starting late, which nothing files, bisects or gates. That mattered
/// concretely: the `place_on` stale-locality lost wakeup was found from one such dump and never
/// reproduced in the wild, 0 crossings in over 1,600 `spawn_on` calls across five instrumented
/// runs. The section records what would reopen it.
pub fn run_idle() -> ! {
    loop {
        // **The capability-slot gauge** (milestone 231). Here because this is the one place the
        // kernel reaches after every phase of a boot with nothing else to do; see
        // `cap::report_peak` for why waiting for the mark to settle is what makes it one line.
        crate::cap::report_peak();
        // And the progenitor's stack gauge, from the same place for the same reason.
        crate::progenitor_stack::report_peak();
        try_initiate_steal();
        crate::arch::wait_for_interrupt();
        yield_now();
    }
}

/// Spawn a thread against a **quota**: at most `budget` of these may be alive at once.
///
/// Reserving a slot is an atomic decrement; the slot lives inside the spawned `Thread` as a
/// [`QuotaToken`] and comes back when the thread is reaped. Returns `None` if the budget is
/// exhausted (too many children already alive) OR the kernel is out of memory: the caller cannot
/// tell the two apart, and does not need to: either way it could not spawn, and it must degrade
/// rather than panic. This is the bound that stops a spawn flood or a leaked-thread pile-up from
/// exhausting kernel memory. See notes/quotas.md and notes/security.md.
///
/// # It has no caller today, and that is worth saying plainly
///
/// Its one caller was the kernel-wired `shell_service`'s spawn service, which DECISIONS §28 retired
/// and milestone 41 deleted. Nothing in the kernel spawns against a quota now, because **the bound
/// moved**: a userspace process spawns out of its own untyped budget (§10, §16), so the budget *is*
/// the quota and it is enforced by retyping rather than by a counter. This function is the bound for
/// *kernel* threads, and no kernel thread is currently spawned in a loop by anything untrusted.
///
/// Kept rather than deleted because removing a documented safety mechanism is a design decision, not
/// dead-code triage, and notes/quotas.md and notes/security.md both describe it. Allowed
/// unconditionally and on purpose (DECISIONS §38, disposition 3): there is no configuration in which
/// something calls it, and pretending otherwise with a `cfg` predicate would be the dishonest option.
#[allow(dead_code)]
pub fn spawn_with_quota<F: FnOnce() + Send + 'static>(
    budget: &'static AtomicU32,
    f: F,
) -> Option<ThreadId> {
    // Reserve a slot: decrement only if there is one. A compare-exchange loop, so it is exactly
    // one atomic decrement and it never dips below zero (returning `None` = "quota exhausted").
    let mut remaining = budget.load(Ordering::Relaxed);
    loop {
        if remaining == 0 {
            return None;
        }
        match budget.compare_exchange_weak(
            remaining,
            remaining - 1,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(actual) => remaining = actual,
        }
    }

    // The reserved slot is held by this token from here on, so every early return below hands it
    // back by dropping it, and a thread that is built carries it until it is reaped.
    let token = QuotaToken::new(budget);

    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut()?;
    // Built in place on its TCB page rather than as a value (see `spawn_on`, milestone 124): the
    // by-value path held three `Thread`s in one frame in an unoptimised build.
    let id = sched.threads.insert_in_place(|tid, dst| {
        // SAFETY: `dst` is the fresh, exclusively-ours TCB page `insert_in_place` claimed.
        unsafe { Thread::spawn_into(f, tid, dst) }
    })?;
    // `insert_in_place` just returned this id, so the thread is there.
    if let Some(t) = sched.threads.get_mut(id) {
        t.quota = Some(token); // returned to `budget` when the thread is reaped
    }
    let token = take_token(sched, id);
    // This core's queue: IPC_TABLES held, IRQs masked.
    cpu::current().with_runq(|q| q.push_back(token));
    Some(id)
}

/// Give up the CPU voluntarily.
pub fn yield_now() {
    schedule();
}

/// The current thread exited cleanly (`SYS_EXIT`). Never returns.
pub fn exit() -> ! {
    depart(abi::fault::EVENT_EXIT, 0, 0)
}

/// The current thread faulted (a bad access, an illegal instruction) and is being killed. Never
/// returns. `pc` is the faulting instruction and `addr` the faulting address (0 if the fault class
/// carries none). The arch fault handlers call this from the faulting thread's kernel stack, the
/// same context `exit` runs in, so the departure below is identical bar the event code and words.
pub fn fault(pc: u64, addr: u64) -> ! {
    depart(abi::fault::EVENT_FAULT, pc, addr)
}

/// **A thread's last act: report its death, then leave the CPU forever** (milestone 22, §26).
///
/// Two outcomes, decided by whether the thread was spawned with a supervision rendezvous (its
/// `fault_ep`, set at `START` from the reserved fault slot):
///
///   - **Unsupervised** (`fault_ep == None`): today's behaviour exactly. Mark `Finished` and
///     `schedule()` away; the next thread's `finish_switch` reaps it once it is off this stack.
///   - **Supervised**: build the five-word §26 message, retain it on the corpse for postmortem,
///     deliver it to the supervision rendezvous (waking a waiting supervisor, or parking the corpse
///     on the rendezvous so the message is not lost if none is waiting), and mark the thread `Dead`.
///     A `Dead` corpse is never reaped by `finish_switch`; it persists, registers and address
///     space intact, until the supervisor reaps it with §16 revocation.
///
/// Either way we are still running on the thread's own kernel stack, so we cannot free it here; we
/// only mark state and `schedule()`, exactly as `exit` always has.
fn depart(event: u64, pc: u64, addr: u64) -> ! {
    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("depart before sched::init");
        let current = current_thread_id();

        let (fault_ep, label) = sched
            .threads
            .get(current)
            .map_or((None, 0), |t| (t.fault_ep, t.fault_label));

        // **A thread that departs answers nobody again** (milestone 254). Whatever `CALL` it
        // collected and never replied to is a caller parked on nothing, discoverable only through
        // the reply capability that is about to be dropped with this table. Free those callers
        // here, with the stale-reply sweep `strand_reply_caller` insists on, so they return
        // `Error::Gone` rather than blocking for the life of the machine. This is QNX's headline
        // behaviour and the case notes/blocked-thread-teardown.md's survey found this kernel alone
        // in lacking.
        strand_callers_of(sched, current);

        match fault_ep {
            None => {
                if let Some(t) = sched.threads.get_mut(current) {
                    t.handshake.state = State::Finished;
                }
            }
            Some(ep) => {
                let msg = [event, current, pc, addr, 0];
                if let Some(t) = sched.threads.get_mut(current) {
                    // Retain the message on the corpse (postmortem) and stage it as the mailbox a
                    // parked-corpse delivery will hand the supervisor. Dead: never runs again.
                    t.fault_msg = Some(msg);
                    t.mailbox = msg;
                    t.handshake.state = State::Dead;
                }
                deliver_death(sched, current, ep, msg, label);
            }
        }
        // Not requeued and not removed: we are still on this stack. The switch below leaves it,
        // and for a Finished thread the next thread reaps it; a Dead one waits for the supervisor.
    }

    schedule();
    unreachable!("a departed thread was scheduled again");
}

/// Deliver a corpse's five-word death message to its supervision rendezvous. Caller holds `IPC_TABLES`
/// and has already marked the corpse `Dead` with `msg` in its mailbox.
///
/// This is the ordinary synchronous-send rendezvous (`Rendezvous::send`), reused: if a supervisor is
/// blocked in `RECEIVE`, hand it the message and wake it; if none is, the corpse joins the rendezvous's
/// sender queue with the message in its mailbox, so the notification waits there rather than being
/// lost (the same guarantee an ordinary blocked sender gets, and the reason a data-carrying death
/// uses the sender queue rather than the data-less IRQ signal count). The corpse is never woken:
/// `ipc_receive` recognises a `Dead` sender and leaves it dead after taking its message, the same way
/// it leaves a `CALL` caller blocked. If the rendezvous itself is gone (the supervisor was torn down
/// first), the message is simply dropped, like an interrupt with no live rendezvous.
///
/// `label` is the badge the builder put on the child's supervision capability (milestone 105 (the
/// two forks), §148 (resolves by asking the kernel) as amended). It goes to a supervisor already waiting here through [`hand_over_label`]; a
/// supervisor that arrives later collects it from the corpse in [`collected_without_serving`].
fn deliver_death(
    sched: &mut IpcTables,
    corpse: ThreadId,
    ep: RendezvousId,
    msg: [u64; 5],
    label: u64,
) {
    let Some(rendezvous) = rendezvous_of(sched, ep) else {
        return;
    };
    // The corpse's token: it is still the running thread, on no queue. If it joins the sender queue
    // below it stays put, since nothing wakes or reaps a Dead thread until the supervisor drains it
    // and revokes; otherwise its token comes back and goes home to its TCB, where the reap drops it.
    match rendezvous.send(take_token(sched, corpse)) {
        inter_process_communication::Send::Rendezvous(receiver, me) => {
            hold_token(me);
            let receiver = hold_token(receiver);
            let r = sched.threads.get_mut(receiver).unwrap();
            r.mailbox = msg;
            hand_over_label(r, label);
            r.handshake.serve(); // delivered: this wake passes the boot-8 gate
            trace::record(trace::Event::Served, receiver, 8);
            wake(sched, receiver);
        }
        inter_process_communication::Send::Blocked => {
            // The corpse is parked on the sender queue now, its mailbox already holding `msg`.
            // Record the parking so a dump shows where the death message waits.
            if let Some(t) = sched.threads.get_mut(corpse) {
                t.handshake.wait_on = Some(Wait::Rendezvous(ep, WaitRole::Sender));
            }
        }
        // The supervision rendezvous carries an interrupt (§101 ruling B). Dropped, like a death
        // to a rendezvous that is gone: its `w0` is `EVENT_FAULT` or `EVENT_EXIT`, and `EVENT_FAULT`
        // is 1, which a driver would read as its interrupt. Reachable only by configuring an
        // interrupt's endpoint as a fault endpoint, which needs a `Rendezvous` capability to it that
        // no program is granted today.
        inter_process_communication::Send::Refused(me) => {
            hold_token(me);
        }
    }
}

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
    // The corruption tripwire, when armed (the board tour's initrd-demo window). One relaxed
    // load when it is not, which is every other tick everywhere. IRQ context is safe for its
    // println: the console's IrqSafeMutex masks interrupts while held, so the interrupted
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
fn count_tick() {
    let here = cpu::current();
    let idle = current_thread_id() == here.idle.load(Ordering::Relaxed);
    crate::machine_statistics::tick(
        here.switches.load(Ordering::Relaxed),
        idle,
        here.runnable() as u64 + u64::from(!idle),
    );
}

pub fn take_need_resched() -> bool {
    cpu::current().need_resched.swap(false, Ordering::Relaxed)
}

/// Install the incoming thread's cycle-counter grant on the core about to run it, immediately after
/// its address-space root (milestone 229, DECISIONS §139 option 4).
///
/// **Built only under `test` or `--features cycle_counter_grant`, and both the read (in `schedule`)
/// and this install are `#[cfg]`-gated at the switch site**, not carried through the shared switch
/// tuple. Milestone 139 threaded the grant through the tuple and called this unconditionally on every
/// arch, trusting the optimizer to fold the constant `false` away when no grant is built; milestone
/// 237 made the mechanism a feature and left the fold in place. Under the debug build the icount gate
/// measures, that fold does not happen: the const-`false` tuple element and the empty install stayed
/// in the shipping switch and cost `yield_switch` +35.6 and `ctx_switch` +36.0 ticks per switch on
/// aarch64 (milestone 300). This is the same leak milestone 299 found for the port grant, and the fix
/// is the same shape: `#[cfg]` the read and the install, keep the tuple at its pre-139 width. When the
/// feature is off `PMUSERENR_EL0` (and riscv64's `scounteren.CY`) keeps the closed value milestone 228
/// wrote at boot for the whole life of the kernel, and none of this code exists to touch it. Closing
/// what we claim is closed is right whether or not anyone can be granted an exception, which is why
/// 228's default write is NOT behind the feature and this is. `kernel/Cargo.toml`'s
/// `cycle_counter_grant` block carries the rest of the measurement.
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
fn install_cycle_counter_grant(granted: bool) {
    crate::arch::timer::set_cycle_counter_grant(granted);
}

/// Install the incoming thread's x86 port grant into the core about to run it, at the last point
/// `IPC_TABLES` is held; milestone 315 (a port revoke that reaches every core) moved it there and
/// the call site says why. The lazy write
/// lives in `arch::segments`.
///
/// **`x86_64` only, and both the read (in `schedule`) and this install are `#[cfg]`-gated at the
/// switch site**, not carried through the shared switch tuple. An earlier version threaded the value
/// through the tuple the way the cycle-counter grant then did and trusted the optimizer to fold a
/// constant `None` away on the other two architectures; it did not (icount measured `yield_switch`
/// and `ctx_switch` up 13% on aarch64). The cycle-counter grant turned out to leak the same way for
/// the same reason (the fold does not happen in the debug build the icount gate measures), and
/// milestone 300 gave it this exact treatment; see `install_cycle_counter_grant`. A port grant is
/// present in every x86 build, so keeping it off the other two ISAs' switch path takes a `#[cfg]`,
/// not a constant. See DECISIONS §152's x86-only rationale.
#[cfg(target_arch = "x86_64")]
fn install_port_grant(grant: Option<(u16, u16)>) {
    crate::arch::segments::set_port_range_grant(grant);
}

/// Pick another thread and go there.
///
/// May be called from normal context (a voluntary `yield_now`) or from the tail of the timer
/// IRQ handler (a preemption). The two paths are identical from here down, which is a large
/// part of why this is only forty lines.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(target_os = "none", unsafe(link_section = ".text.hot.sched.schedule"))]
pub fn schedule() {
    // **Not from an interrupt stack** (milestone 124). A switch parks the running `sp` in the
    // outgoing thread's `Context` and resumes it there, arbitrarily later; a per-core interrupt
    // stack cannot promise those bytes will still be the thread's, so a thread parked there would
    // resume on whatever the next interrupt on that core had written. The interrupt path defers its
    // switch to `preempt_if_needed`, which its dispatcher calls one frame outside the trampoline,
    // back on the interrupted thread's own stack.
    //
    // Debug-only because it costs a `sp` read and a scan of `MAX_CPUS` spans on the hottest path in
    // the kernel, and because `script/stack-depth-check` proves the same property statically in CI,
    // on both architectures, by showing no context switch is reachable from the interrupt-stack
    // entry point. This is the runtime half of that pair, for the edges a call graph cannot see.
    debug_assert!(
        !crate::interrupt_stack::contains(crate::arch::current_sp()),
        "schedule() called on an interrupt stack: the outgoing thread would be parked on memory \
         that belongs to a core (see kernel/src/interrupt_stack.rs)",
    );

    // Rule 2: no interrupts across the decision *or* the switch. Between "I chose a thread" and
    // "I am running it" there must be no window for the timer to choose again.
    //
    // The saved state is a local, on **this thread's stack**, which is exactly what makes it
    // correct: when someone eventually switches back to us, `switch_to` returns here, and this
    // frame (with the right `was_enabled` in it) is still sitting where we left it.
    let was_enabled = crate::arch::interrupts::disable();

    // The incoming thread's cycle-counter grant, carried out of the decision block the same way and
    // for the same reason (milestone 300, the fix milestone 299 gave the port grant just above).
    // Written inside the block under the lock, read at the install site after the lock drops; `false`
    // unless the block decides to switch. Built only when a grant can exist (`test` or
    // `--features cycle_counter_grant`), so every shipping build's `schedule()` gains nothing at all
    // and the switch tuple stays at its pre-139 width. See `install_cycle_counter_grant`.
    #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
    let mut next_cycle_counter = false;

    // A labeled block, so every exit path leaves through the SAME point: the guard drops at the
    // block's end and interrupts are restored ONCE, AFTER it. The earlier version called
    // `interrupts::restore(was_enabled)` and `return` from *inside* this block, which re-enabled
    // interrupts while still holding `IPC_TABLES`: a one-instruction window in which a
    // timer could fire, re-enter `schedule()`, and try to take a lock we already held. It was
    // intermittent and it was real; see the lock-rank violation it produced.
    let switch = 'decide: {
        let mut guard = IPC_TABLES.lock();
        let Some(sched) = guard.as_mut() else {
            break 'decide None;
        };

        let current = current_thread_id();

        // **Forcible teardown, before anything else** (DECISIONS §16 amendment; §28 made it
        // load-bearing). A thread `DESTROY` marked killed must never run again. Convert it to a
        // `Finished` corpse here, at the top of the decision, so *every* path below reaps it: not
        // only the switch path, but the "nothing else to run, keep current" path a runaway alone on
        // its core takes. Before §28's scattering, a killed runaway shared a core with other work, so
        // a switch always happened and the old requeue-time check sufficed; once a runaway can be the
        // only thread on its core, that check was unreachable and the runaway spun forever.
        // The *test* stays here, on the hot path, because it is two loads and a compare. The
        // *body* is out of line (milestone 188 phase 3): converting a killed thread and freeing its
        // callers is a teardown, so its bytes have no business in the kernel's hottest function.
        if sched
            .threads
            .get(current)
            .is_some_and(|t| t.killed && t.handshake.state == State::Running)
        {
            finish_killed_current(sched, current);
        }
        let state = sched.threads.get(current).map(|t| t.handshake.state);

        // **Only a still-Running thread goes back on the ready queue.** A thread that reached
        // here after marking itself `Blocked` (it is waiting for IPC), `Finished`, or `Dead` (a
        // supervised corpse) must not be rescheduled, and this one line is what makes blocking work:
        // `schedule()` can be called from the timer IRQ *while* a thread is mid-way through blocking
        // itself, and it must not undo that by helpfully requeueing it.
        let runnable = state == Some(State::Running);

        let idle_tid = cpu::current().idle.load(Ordering::Relaxed);

        let next = match cpu::current().with_runq(|q| q.pop_front()) {
            // The popped token goes home to its thread, which holds it while it runs, and the id is
            // what everything below uses.
            Some(t) => hold_token(t),
            None => {
                if runnable {
                    // Keep it. A thread yielding into an empty run queue simply carries on. (The
                    // idle thread lands here too: nothing to do, so it wfi's again.) No switch.
                    break 'decide None;
                }
                // Current is Blocked or Finished and the ready queue is empty. This is NOT a
                // deadlock: a thread blocked on a device interrupt is waiting for an event that
                // will arrive. Fall back to the idle thread, which wfi's until it does.
                if idle_tid == u64::MAX || current == idle_tid {
                    // No idle thread yet (before init finished), or the idle thread itself is
                    // somehow not runnable, which cannot happen. Either way there is genuinely
                    // nothing to run.
                    match state {
                        Some(State::Finished) => {
                            panic!("the last thread exited; nothing left to run")
                        }
                        _ => panic!("nothing runnable and no idle thread"),
                    }
                }
                idle_tid
            }
        };

        // **The running thread must never come off its own run queue** (boot 8's downstream
        // catastrophe, guarded here on its own merits). The pop above precedes the requeue below,
        // so a legal schedule can never hand back `current`; if it ever does, something queued a
        // thread that was still running, and switching into it would restore `t.context`, a
        // pointer to a frame this very thread has already resumed and consumed: execution
        // time-travels to its previous switch-out point on a reused stack and spins there
        // forever, off every instrument. Debug builds fail loudly; the board build heals by
        // keeping the thread running, which is the only state that is still coherent.
        if next == current {
            heal_self_pop(sched, current);
            break 'decide None;
        }

        // Requeue the outgoing thread if it can still run, but never the idle thread, which lives
        // outside the ready queue. A killed thread is already `Finished` (handled at the top), so it
        // is not runnable here and is reaped by `finish_switch` after the switch, no queue surgery.
        if runnable && current != idle_tid {
            // `preempt` marks Ready and deliberately leaves `on_cpu` set: the thread is in a
            // queue AND still standing on this core until finish_switch runs, which is the one
            // legal overlap and the reason only this core may pop its own queue (the extracted
            // protocol's steal-vs-switch-out rule; see crates/wake_handshake).
            let t = sched.threads.get_mut(current).unwrap();
            t.handshake.preempt();
            // The token a running thread holds on its own TCB. Round robin: the back.
            let Some(token) = t.own_token.take() else {
                missing_token()
            };
            cpu::current().with_runq(|q| q.push_back(token));
        }

        // Running, with on_cpu set until ITS successor's finish_switch, one switch from now.
        // **Did this thread just cross cores?** Recorded here rather than at any wake site, because
        // this is the one place every path to a CPU passes through, whatever moved the thread: a
        // rendezvous wake onto the waker's core, a work steal, a load-aware placement, or spawn.
        // See `thread::Thread::last_cpu` for why the placement counter could not answer this, and
        // for why this is behind `feature = "soak_test"`: it is the hottest line of the hottest
        // function, and shipping it everywhere put `ipc_fastpath` over milestone 132's bound.
        #[cfg(feature = "soak_test")]
        {
            let here = cpu::id();
            let t = sched.threads.get_mut(next).unwrap();
            let from = t.last_cpu;
            t.last_cpu = here as u8;
            if from != u8::MAX && from as usize != here {
                trace::record(trace::Event::Migrated, next, from);
            }
        }
        let next_tcb = sched.threads.pointer(next).unwrap();
        // SAFETY: the page pointer of a live thread, and `IPC_TABLES` (held) serializes every
        // access to its `Thread`. One lookup serves the handshake and `set_current` both.
        unsafe { (*next_tcb).handshake.switch_in() };
        // SAFETY: `next_tcb` is `next`'s page pointer from the table, live.
        unsafe { set_current(next, next_tcb) };
        trace::record(trace::Event::SwitchTo, next, 0);

        // Hand the outgoing thread to the incoming one to finish up AFTER the switch, when it is
        // provably off its stack: reap it if it Finished, clear its on_cpu (and complete a
        // deferred wake) otherwise. Not here, and not by another core: we are still running on
        // its stack this instant. `current` is the local (the outgoing tid); `set_current`
        // above already moved the per-CPU current to `next`. See finish_switch.
        //
        // **And count the switch, through the same block** (milestone 629 (the context-switch statistic stops costing the switch path)): `vmstat`'s `cs` is this
        // word, copied to the machine statistics page by the tick, so the switch pays one add on a
        // block it is already writing rather than a walk to the page. See `PerCpu::switches`.
        let here = cpu::current();
        here.switched_from.store(current, Ordering::Relaxed);
        here.switches.fetch_add(1, Ordering::Relaxed);

        // The incoming thread's low half. A kernel thread gets the empty reserved table, which
        // makes every low address fault, which is exactly right: it has no business down there.
        //
        // **And its current-CPU page, off the same borrow**, which is the whole of this kernel's
        // half of calef's 2026-09-21 ruling that a thread observing itself is a page rather than a
        // crossing. Here rather than at any wake or placement site for `last_cpu`'s reason one
        // field over: this is the one point every path to a CPU passes through, whatever moved the
        // thread. And on this core rather than the one that queued it, which is what makes writer
        // and reader the same hardware thread and the ordering argument a short one
        // (`crates/current_cpu_protocol`). A kernel thread has no space and needs no page: it has
        // no userspace to read one from.
        let next_root = match sched.threads.get(next).unwrap().space.as_ref() {
            Some(space) => {
                space.publish_current_cpu(cpu::id() as u64);
                space.ttbr0()
            }
            None => crate::arch::mmu::reserved_root(),
        };

        // The incoming thread's cycle-counter grant (milestone 229, DECISIONS §139 option 4), read
        // here for the same reason the root is: this is the last point the lock is held. A kernel
        // thread's is `false`, like every user thread nobody granted it to. Written into the
        // `#[cfg]`-gated variable declared above the block, not the switch tuple, so it costs the
        // shipping build nothing at all (milestone 300; see `install_cycle_counter_grant` for why the
        // old tuple-and-fold did not fold in the debug build the icount gate measures).
        #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
        {
            next_cycle_counter = sched.threads.get(next).unwrap().cycle_counter_grant;
        }

        // Copy the raw pointers out before the lock drops. The assembly writes through the
        // context slot and reads the incoming context, and both threads' TCB pages keep their
        // contents pinned.
        //
        // **The TCB page pointer rather than `get_mut`** (milestone 447), and it is one probe per
        // side rather than the two the obvious shape would cost. Each thread's FP register file
        // lives in the free space of the same page, past the end of the struct, so a `&mut Thread`
        // is exactly the wrong thing to hold: its provenance stops at the struct.
        // `Threads::pointer` hands back the page cast unnarrowed and both addresses fall out of it.
        let prev_ptr = sched.threads.pointer(current).unwrap();

        // **The thread pointer, handed from the outgoing thread to the incoming one** (milestone
        // 812, §269 fork 4), here rather than after the lock drops because this core cannot return
        // to user mode between now and the switch (interrupts are masked), and both `Thread`s are
        // only touched under `IPC_TABLES`. aarch64 saves the outgoing register (EL0 can write it)
        // and installs the incoming one; `x86_64` installs only when the value differs; riscv64
        // does nothing, since `tp` rides each thread's trap frame. `arch::thread_pointer` has each.
        //
        // SAFETY: `prev_ptr` and `next_tcb` are live threads' TCB pages from the table, under
        // `IPC_TABLES`; the projection takes no reference to anything but the one field.
        unsafe {
            crate::arch::thread_pointer::hand_over(
                &mut (*prev_ptr).thread_pointer,
                (*next_tcb).thread_pointer,
            );
        }

        // SAFETY: a live thread's TCB page, held under IPC_TABLES. Field projection through a raw
        // pointer, and `fp_state_of`'s contract is exactly what `pointer` returns.
        let (prev_slot, prev_fp): (*mut *mut Context, *mut crate::arch::fp::FpState) = unsafe {
            (
                &raw mut (*prev_ptr).context,
                crate::thread::fp_state_of(prev_ptr),
            )
        };

        // `next_ctx`, and on x86 the incoming thread's port grant (milestone 299) beside it, out of
        // ONE lookup. The two arms are byte-identical bar the port read, and the split is
        // deliberate: the port read goes into the `x86_64`-only variable declared above the block,
        // not the switch tuple, so it costs the other two architectures nothing at all. The
        // cycle-counter grant is carried the same way, by milestone 300 (decompose the icount
        // baseline drift, and re-baseline only what is proven), so neither grant widens this
        // tuple: it is back to its pre-139 width `(prev_slot, next_ctx, next_root)` on every
        // shipping build, plus milestone 447's two register-file pointers.
        #[cfg(not(target_arch = "x86_64"))]
        // SAFETY: as `prev_ptr`; `next` was popped off this core's run queue and marked `Running`
        // inside this same locked block.
        let (next_ctx, next_fp): (*mut Context, *const crate::arch::fp::FpState) = unsafe {
            let next_ptr = sched.threads.pointer(next).unwrap();
            (
                (*next_ptr).context,
                crate::thread::fp_state_of(next_ptr).cast_const(),
            )
        };
        #[cfg(target_arch = "x86_64")]
        // SAFETY: as the arm above, plus the port grant, which is a plain field read.
        let (next_ctx, next_fp, next_port_grant): (
            *mut Context,
            *const crate::arch::fp::FpState,
            Option<(u16, u16)>,
        ) = unsafe {
            let next_ptr = sched.threads.pointer(next).unwrap();
            (
                (*next_ptr).context,
                crate::thread::fp_state_of(next_ptr).cast_const(),
                (*next_ptr).port_range_grant,
            )
        };

        // **The incoming thread's authority to reach x86 I/O ports from ring 3, installed here and
        // not after the lock drops** (milestone 315; it was beside the address-space install until
        // then). The TSS I/O-bitmap grant is the one per-core fact a *revocation* has to be able to
        // take back from a core it is not running on, and `delete_port_range_caps_impl` does that by
        // NMI while holding this lock. Reading the grant under the lock and installing it outside
        // left two windows that broadcast could not close: a core could install a grant the sweep had
        // already cleared, and the NMI could land in the middle of the install. Both shut if every
        // writer of a bitmap holds `IPC_TABLES`, which costs, on a machine where nothing holds a
        // port, one compare inside the critical section instead of one outside it.
        //
        // Nothing can execute a ring-3 `in`/`out` between here and the switch: this core is in the
        // kernel with interrupts masked the whole way.
        #[cfg(target_arch = "x86_64")]
        install_port_grant(next_port_grant);

        Some((prev_slot, next_ctx, next_root, prev_fp, next_fp))
    };
    // Rule 1: THE LOCK IS RELEASED HERE, before the switch. Holding it across `switch_to` would
    // leave it held by a thread that is not running, and the next thread to want it would spin
    // forever waiting for a thread that can only be scheduled by taking the lock.

    if let Some((prev_slot, next_ctx, next_root, prev_fp, next_fp)) = switch {
        // Install the incoming thread's address space FIRST. `TTBR0_EL1` is one register, shared
        // by everybody, and a thread that resumes at EL0 in the previous thread's low half is
        // running a stranger's code. (No-ops, including no TLB flush, when the root is already
        // right, which is every switch between two kernel threads.)
        //
        // SAFETY: `next_root` is `reserved_root()`, or the composed value of the `AddressSpace`
        // owned by thread `next`, which the block above popped off this core's run queue and marked
        // `Running` with `on_cpu` set before releasing `IPC_TABLES`. No other core can pick it up in that
        // state and nothing reaps a thread that is on a CPU, so the root is still live here even
        // though the lock is not held. The lock is released on purpose (rule 1, above), which is
        // exactly why this obligation cannot be a borrow and has to be a sentence.
        unsafe { crate::arch::mmu::switch_user_root(next_root) };

        // And the incoming thread's authority to read the cycle counter, which is the same kind of
        // fact about the same instant: one register, shared by everybody, that has to say what the
        // thread about to run was granted rather than what the last one was. Needs no `unsafe`,
        // because unlike `TTBR0_EL1` this register names no memory and points at nothing that can
        // be freed: getting it wrong opens or closes a counter, it does not hand a thread a
        // stranger's pages. Costs a compare when the value already matches, which is every switch
        // on a machine where nothing is granted. `#[cfg]`-gated, not folded (milestone 300): it
        // exists only in a build that can grant the counter, the same shape as the port grant just
        // below. See `arch::timer::set_cycle_counter_grant` and `install_cycle_counter_grant`.
        #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
        install_cycle_counter_grant(next_cycle_counter);

        // And the register file the two threads are about to share a core over (milestone 447).
        // This is beside `switch_to` rather than inside it because the two save different
        // quantities for different reasons: `switch_to` saves what a *function call* may destroy,
        // and this moves what a *thread* owns. It runs here, as the outgoing thread, with the lock
        // released and interrupts masked, so the register file is already right when the switch
        // lands. `crate::fp::hand_over` has the four cases and the CVE that decided them.
        //
        // SAFETY: `prev_fp` and `next_fp` name the `FpState`s of the outgoing and incoming threads,
        // pinned by the same argument the two lines below make for their contexts.
        unsafe { crate::fp::hand_over(prev_fp, next_fp) };

        // SAFETY: both pointers name live `Context`s owned by boxed `Thread`s in the map, and
        // interrupts are masked so nothing can reorder underneath us.
        //
        // This call does not return here. It returns *in another thread*, at the point where
        // that thread last called `switch_to`. We come back only when somebody switches to us.
        unsafe { switch_to(prev_slot, next_ctx) };

        // We are now the incoming thread, resuming. Reap whoever we switched away from, if it had
        // finished: it is off its stack now, and we are on the same core that set `to_reap`.
        finish_switch();
    }

    crate::arch::interrupts::restore(was_enabled);
}

/// **Convert a killed current thread into a corpse, and free the callers it can no longer answer.**
/// The body of [`schedule`]'s forcible-teardown check, out of line (milestone 188 phase 3): the
/// predicate is two loads on the kernel's hottest path, the body is a teardown that no IPC reaches.
///
/// A thread `DESTROY` marked killed must never run again, and this runs at the top of the decision
/// so *every* path below reaps it: not only the switch path, but the "nothing else to run, keep
/// current" path a runaway alone on its core takes. Before §28's scattering a killed runaway shared
/// a core with other work, so a switch always happened and the old requeue-time check sufficed;
/// once a runaway can be the only thread on its core, that check was unreachable and the runaway
/// spun forever.
///
/// A killed thread never reaches `depart`, so this is the one place its callers can be freed before
/// its capability table goes (milestone 254).
///
/// Caller holds `IPC_TABLES` and has already checked that `current` is killed and `Running`.
#[cold]
#[inline(never)]
fn finish_killed_current(sched: &mut IpcTables, current: ThreadId) {
    if let Some(t) = sched.threads.get_mut(current) {
        t.handshake.state = State::Finished;
    }
    strand_callers_of(sched, current);
}

/// **The running thread came off its own run queue, which cannot legally happen.** [`schedule`]'s
/// guard against boot 8's downstream catastrophe, out of line (milestone 188 phase 3) because it is
/// unreachable on every path this kernel is meant to take.
///
/// The pop precedes the requeue, so a legal schedule can never hand back `current`; if it ever
/// does, something queued a thread that was still running, and switching into it would restore
/// `t.context`, a pointer to a frame this very thread has already resumed and consumed: execution
/// time-travels to its previous switch-out point on a reused stack and spins there forever, off
/// every instrument. Debug builds fail loudly; the board build heals by keeping the thread running,
/// which is the only state that is still coherent.
#[cold]
#[inline(never)]
fn heal_self_pop(sched: &mut IpcTables, current: ThreadId) {
    if cfg!(debug_assertions) {
        panic!("schedule() popped its own current thread from the run queue");
    }
    if let Some(t) = sched.threads.get_mut(current) {
        t.handshake.state = State::Running;
    }
}

/// Reap the thread this core just switched away from, if it had finished.
///
/// The safe half of the two-part reaper. `schedule()` records a finished outgoing thread in this
/// core's `to_reap` *before* the switch; this runs on the incoming thread *after* the switch, when
/// the outgoing thread is provably off its stack (its registers are saved and we are on a
/// different stack). Dropping the `Thread` unmaps its stack and frees its address space, which is
/// exactly why it must not happen while any core still stands on it.
///
/// Called from two places, because a thread can resume two ways: from `schedule()` (an existing
/// thread returning from `switch_to`) and from `thread_entry` (a brand-new thread, which never
/// passes through `schedule()`'s post-switch point). Both run on this core, so both see this core's
/// `to_reap`. See DECISIONS §11 and thread.rs.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.finish_switch")
)]
pub(crate) fn finish_switch() {
    let prev = cpu::current()
        .switched_from
        .swap(cpu::NO_TID, Ordering::Relaxed);
    if prev == cpu::NO_TID {
        return;
    }
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    let Some(t) = sched.threads.get_mut(prev) else {
        return;
    };
    // The predecessor's context is saved now (we are running, so switch_to completed). What that
    // makes legal is `thread_wake_handshake::Handshake::finish_switch`'s verdict: reap a Finished
    // predecessor, complete a wake that was deferred mid-switch-out, or simply clear `on_cpu` so
    // other cores may run it. The transition is the crate's (loom searches it; see
    // notes/interleaving.md); the reap and the queue push are ours.
    let verdict = t.handshake.finish_switch();
    match verdict {
        // Out of line, and `#[cold]` says why: a switch-out reaps only when the outgoing thread
        // FINISHED, which no IPC does. Left inline its bytes sat in `finish_switch`, which is on
        // both IPC closures, and `script/fastpath-footprint` could only exclude the *callees* it
        // reaches (`KernelStack`, `AddressSpace`, `drop_in_place`) and not the arm's own setup.
        // Milestone 188 phase 3: the classification now lives in the code rather than in a regex.
        SwitchOutVerdict::Reap => reap_switched_out(guard, prev),
        SwitchOutVerdict::WakeCompleted => {
            trace::record(trace::Event::WakeCompleted, prev, 0);
            // A deferred wake was deferred precisely because the waker did NOT queue it, so the
            // token is still on the thread. IRQs are still masked on both callers' paths.
            let Some(token) = t.own_token.take() else {
                missing_token()
            };
            cpu::current().with_runq(|q| q.push_back(token));
        }
        SwitchOutVerdict::Cleared => {}
    }
}

/// **Reap the thread this core just switched away from.** The `Reap` arm of [`finish_switch`],
/// out of line because it runs when a thread has ended and never on an IPC.
///
/// **Two short critical sections with the expensive part between them**, since 2026-10-04.
///
/// 1. Under the caller's `IPC_TABLES` guard: take the thread's kernel stack and its address space
///    out, and mark it [`being_reaped`](crate::thread::Thread::being_reaped). It stays in the
///    table, `Finished`, so its name still resolves and region teardown still refuses it.
/// 2. With no lock held: free the stack, then the address space. The stack is six page unmaps,
///    each discharging its TLB obligation; on riscv64 every one is an SBI remote fence that
///    interrupts every other hart and waits for them, and on x86_64 an NMI shootdown round. Until
///    2026-10-04 the stack was freed under `IPC_TABLES`, so every IPC and every capability lookup
///    on every other core waited behind a thread exiting anywhere. The job mix measured it as the
///    cheapest syscall nearly doubling in cost once four cores were busy
///    (notes/job-mix/null-syscall-under-load.md). The address space could never be dropped under
///    the lock: its teardown is `memory_region::destroy` (milestone 14 (kernel objects from
///    untyped) phase B.4), whose §13 revocation sweep takes `IPC_TABLES` itself.
/// 3. Under `IPC_TABLES` again: remove the thread. Only now does it stop occupying its region.
///
/// **Everything the thread owned is gone before step 3, and that order is the point.** Region
/// teardown (`reclaim_region`) relies on a resident thread's space being gone before the thread is
/// (its `being_reaped` refusal holds the region until step 3), and an owner whose `DESTROY` succeeds
/// may reuse the memory at once. Before
/// 2026-10-04 the space was dropped just after the thread was removed, so a `DESTROY` on another
/// core could reclaim the region in between, while the space's page tables (in that region) and
/// its revocation-database entries were still live. The rest of the `Thread` (its quota token, its
/// capability table) still drops under the lock in step 3.
///
/// Nothing else removes a `Finished` thread between steps 1 and 3: the only other remover is region
/// teardown, and `region_reap_verdict` refuses a thread that is being reaped.
///
/// **Since §249 the space is taken out of the address-space registry in step 2**, by the name in the
/// thread's copy, and the region sweep may have taken it already (this thread is a corpse off its
/// stack); `Table::remove` hands it to one of the two.
///
/// # BUGS
///
/// - **A corpse whose TCB is outside region R and whose root is inside it still has a narrow
///   window.** If this reaper takes the space in step 2 and a `DESTROY(R)` on another core runs
///   wholly before the drop finishes, the sweep finds no entry, R comes back, and the drop's
///   `revoke::forget_root` can land on a root that page has been given to since. The sweep taking
///   the space first is the common order and is safe; this is the other order. Before §249 it was open for the whole life of an unreaped corpse rather than for one drop. It is
///   handed to
///   milestone 765 (a destroyed region cannot free the root a running thread walks), whose refusal
///   is the natural place to count a space that is still being dropped.
#[cold]
#[inline(never)]
fn reap_switched_out(mut guard: crate::sync::IrqSafeGuard<'_, Option<IpcTables>>, prev: ThreadId) {
    let Some(sched) = guard.as_mut() else {
        return;
    };
    let (space, stack) = match sched.threads.get_mut(prev) {
        Some(t) => {
            t.being_reaped = true;
            (t.space.take(), t.stack.take())
        }
        None => return,
    };
    drop(guard);

    #[cfg(feature = "lock_wait")]
    let t0 = crate::arch::timer::now();
    drop(stack);
    #[cfg(feature = "lock_wait")]
    crate::lock_wait::stack_freed(crate::arch::timer::now() - t0);
    // The thread kept a copy; the registry owns the space (§249). `None` here means the region
    // sweep already took it from under this corpse, which is the take-once removal working, not a
    // leak. Taken and dropped as two statements so the registry's lock is released before the
    // `Drop`, which takes the revocation, region and ASID locks.
    if let Some(bound) = space {
        let space = crate::user::take_user_address_space(bound.name());
        drop(space);
    }

    let mut guard = IPC_TABLES.lock();
    if let Some(sched) = guard.as_mut() {
        #[cfg(feature = "lock_wait")]
        let t0 = crate::arch::timer::now();
        sched.threads.remove(prev);
        #[cfg(feature = "lock_wait")]
        crate::lock_wait::reaped(crate::arch::timer::now() - t0);
    }
}

/// intid -> rendezvous id + 1 (0 means "not routed"). A hardware interrupt, delivered as a
/// message to whoever holds the matching rendezvous.
///
/// **A plain atomic array, read lock-free from the interrupt handler.** The handler runs in a
/// context where taking a lock to *find out where to send the message* would be one more thing
/// that can go wrong; a bounded array of atomics cannot. 256 covers every INTID we will see
/// (SGIs 0-15, the timer PPI at 30, virtio SPIs in the 40s).
const MAX_INTID: usize = 256;
static IRQ_ROUTES: [AtomicU64; MAX_INTID] = [const { AtomicU64::new(0) }; MAX_INTID];

/// Route a hardware interrupt to an rendezvous. From now on, when `intid` fires, whoever is
/// blocked on `ep` wakes; if nobody is, the signal is remembered so it is not lost.
///
/// **And from now on `ep` refuses every send** (DECISIONS §101 (notification objects), calef's
/// ruling B, 2026-09-26). An interrupt reaches its driver as `w0 = 1`, which is also a word any
/// sender can put in `w0`, so an endpoint that carried both could not tell a driver which one woke
/// it. The refusal is the rendezvous's own (`Rendezvous::bind_to_interrupt`), so it holds for every
/// endpoint bound here whoever created it: the fifteen the kernel creates for its drivers, which
/// no program is ever granted, and the caller-supplied ones `soak::bind_tick_routes` binds, which
/// is why this is done here rather than at each call site. `SEND`, `SEND_CAP` and `CALL` answer
/// [`abi::Error::NotPermitted`]; see `set_ipc_refused`.
///
/// Marked before the route is published, so there is no instant at which the interrupt is live on
/// an endpoint that still accepts a send. A stale `ep` is routed and not marked, exactly as before:
/// `irq_notify` drops a signal to a name that does not resolve, and a name that does not resolve
/// cannot be sent to either.
pub fn bind_irq(intid: u32, ep: RendezvousId) {
    assert!((intid as usize) < MAX_INTID, "intid {intid} out of range");
    {
        let guard = IPC_TABLES.lock();
        if let Some(rendezvous) = guard.as_ref().and_then(|sched| rendezvous_of(sched, ep)) {
            rendezvous.bind_to_interrupt();
        }
    }
    // +1 so 0 keeps meaning "not routed". A name can never be u64::MAX (the registry mints
    // (generation << 32) | slot with slot < 256), so the increment cannot wrap.
    IRQ_ROUTES[intid as usize].store(ep + 1, Ordering::Release);
}

/// The rendezvous an interrupt is routed to, if any. Read from the IRQ handler; lock-free.
pub fn irq_route(intid: u32) -> Option<RendezvousId> {
    if (intid as usize) >= MAX_INTID {
        return None;
    }
    match IRQ_ROUTES[intid as usize].load(Ordering::Acquire) {
        0 => None,
        n => Some(n - 1),
    }
}

/// **Deliver an interrupt as a message.** Called from the IRQ handler.
///
/// If a thread is blocked waiting on the rendezvous, wake it. If not, count the signal so the
/// next `RECEIVE` returns immediately rather than blocking on an interrupt that already happened.
/// **An interrupt is not a rendezvous**: it must not wait for a receiver, and it must not be
/// lost if the receiver is briefly busy.
///
/// Safe to call from IRQ context: it takes `IPC_TABLES`, which the interrupted code
/// cannot have been holding, because `IrqSafeMutex` masks interrupts for exactly as long as it
/// is held. See DECISIONS §9.
pub fn irq_notify(ep: RendezvousId) {
    // A device interrupt routed to a driver, counted for `vmstat`'s `in` column (milestone 126).
    crate::machine_statistics::interrupt();
    // A device-IRQ wake is LOAD-AWARE (DECISIONS §28.2), unlike a rendezvous wake, which stays
    // local. If the woken driver lands on a *remote* core, `wake_load_aware` returns that core so we
    // can poke it after IPC_TABLES is released (the `place_on` discipline: push under the lock, SGI
    // after). The SGI send from IRQ context is a plain controller write, safe here.
    let remote = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");

        // `signal` wakes a waiting receiver or counts the signal; it never blocks or joins a queue. A
        // stale name (the rendezvous an interrupt was bound to has been revoked) is simply dropped: an
        // interrupt with no live rendezvous has nowhere to go, which is not an error.
        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            return;
        };
        if let Some(waiter) = rendezvous.signal() {
            let waiter = hold_token(waiter);
            let t = sched.threads.get_mut(waiter).unwrap();
            t.mailbox = [1, 0, 0, 0, 0];
            t.handshake.serve(); // the signal is the delivery (the boot-8 gate)
            trace::record(trace::Event::Served, waiter, 7);
            wake_load_aware(sched, waiter)
        } else {
            None
        }
    };
    if let Some(target) = remote {
        crate::arch::irq::send_reschedule(target);
    }
}

/// Why creating an rendezvous failed. The two causes need telling apart because they call for opposite
/// responses: a full region means carve more memory and retry, a full registry means give up.
///
/// They used to be one `None`, which is also why the registry-full case leaked a page: the caller
/// could not know the page it had just spent was about to be thrown away.
enum RendezvousFailure {
    /// The region has no page left to retype. Nothing was spent.
    RegionFull,
    /// The registry is at [`MAX_RENDEZVOUS`]. Checked *before* spending a page, so nothing was spent.
    RegistryFull,
}

/// Create an rendezvous **in `region`'s memory** (milestone 19a): one page retyped and pinned, the
/// rendezvous at its start, a fresh generational name in the registry. The shared engine of the
/// `RETYPE_OBJ` syscall and the kernel's own [`create_rendezvous`].
fn try_create_rendezvous_from(region: u64) -> Result<RendezvousId, RendezvousFailure> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(RendezvousFailure::RegistryFull)?;

    // Checked BEFORE the retype, which is a fix and not just tidiness: this used to retype a page and
    // then discover the registry was full, spending the page for nothing. The old comment called that
    // "a process-local loss on its own budget", which is true and is still a leak a caller cannot see
    // or recover. Asking first costs a compare.
    if sched.rendezvous_table.len() >= MAX_RENDEZVOUS {
        return Err(RendezvousFailure::RegistryFull);
    }

    // Rank: MEMORY_REGION (58) under IPC_TABLES (60) is a legal descent; the pin rides in the same lock
    // hold as the carve, so no destroy can race the page away (see retype_object_page).
    let phys = crate::memory_region::retype_object_page(
        region,
        crate::memory_region::ObjectKind::Rendezvous,
    )
    .ok_or(RendezvousFailure::RegionFull)?;

    // The page arrives zeroed, and an all-zero Rendezvous happens to be valid; write it explicitly
    // anyway, because "happens to be" is the kind of truth that stops being one silently.
    // SAFETY: fresh page, exclusively ours, direct-mapped.
    unsafe { (crate::arch::mmu::phys_to_virt(phys) as *mut Rendezvous).write(Rendezvous::new()) };

    // Cannot fail: capacity was checked above under this same lock hold.
    let name = sched
        .rendezvous_table
        .insert_with(|_| phys)
        .ok_or(RendezvousFailure::RegistryFull)?;
    PEAK_RENDEZVOUS.fetch_max(sched.rendezvous_table.len(), Ordering::Relaxed);
    if sched.kernel_ep_region == Some(region) {
        KERNEL_CHUNK_RENDEZVOUS.fetch_add(1, Ordering::Relaxed);
    }
    Ok(name)
}

/// Create an rendezvous in `region`'s memory. `None` when the region is out of budget or the registry
/// is full. The `RETYPE_OBJ` syscall's engine; userspace gets one flat failure because a process
/// cannot act on the difference (it holds one region and cannot enlarge the kernel's registry).
pub fn create_rendezvous_from(region: u64) -> Option<RendezvousId> {
    try_create_rendezvous_from(region).ok()
}

/// Create an IPC rendezvous on the kernel's own budget. Returns the name that goes inside an
/// `Object::Rendezvous`. Chunks are carved lazily and **grown on demand**, so this does not depend on
/// anyone having guessed the suite's eventual size.
///
/// Panics only on a genuinely unrecoverable condition: the registry at [`MAX_RENDEZVOUS`], the chunk
/// bound reached (which cannot happen before the registry fills, by construction), or no memory left
/// to carve from. Every caller is the kernel, so there is no user to return an error to.
///
/// **`pub(crate)`, so the system tests cannot call it, and that is a gate rather than tidiness.**
/// The chunks this carves are never freed, so every endpoint a test made here cost the boot a page
/// for good, and the frame ledger moved in +32 steps charged to whichever later test crossed a chunk
/// boundary (`testing::SUITE_PAGE_FRAME_BUDGET`'s history is mostly that). The kernel's own
/// services create their endpoints once and keep them, which is what this is for. A test carves
/// its endpoints with [`create_rendezvous_from`] from a region it owns and reclaims with
/// [`reclaim_region`], or hands that region to a `user::holding::Holding`; a test whose endpoint
/// must outlive it still uses a region of its own and says so where it does, so the page is charged
/// to that test. The `system_tests` crate depends on this one, so a call from there does not
/// compile.
pub(crate) fn create_rendezvous() -> RendezvousId {
    loop {
        // Take, or lazily carve, the current chunk.
        let region = {
            let mut guard = IPC_TABLES.lock();
            let sched = guard.as_mut().expect("no scheduler");
            match sched.kernel_ep_region {
                Some(r) => r,
                None => {
                    assert!(
                        sched.kernel_ep_chunks < MAX_KERNEL_EP_CHUNKS,
                        "the kernel carved all {MAX_KERNEL_EP_CHUNKS} rendezvous chunks; \
                         with {MAX_RENDEZVOUS} registry slots this should be unreachable",
                    );
                    let r = crate::memory_region::create(KERNEL_EP_CHUNK_PAGES)
                        .expect("no memory for a kernel rendezvous chunk");
                    sched.kernel_ep_chunks += 1;
                    sched.kernel_ep_region = Some(r);
                    r
                }
            }
        };

        match try_create_rendezvous_from(region) {
            Ok(ep) => return ep,
            // The chunk is spent. Drop it and let the next pass carve a fresh one; the loop runs at
            // most twice per call, because a fresh chunk always has a page. Clearing the handle is
            // what "forgotten deliberately" means in the field's doc comment: the pages stay pinned
            // and the endpoints already in them stay live.
            Err(RendezvousFailure::RegionFull) => {
                let mut guard = IPC_TABLES.lock();
                let sched = guard.as_mut().expect("no scheduler");
                // Only clear the handle we just failed on. Another core may already have replaced it.
                if sched.kernel_ep_region == Some(region) {
                    sched.kernel_ep_region = None;
                }
            }
            Err(RendezvousFailure::RegistryFull) => {
                panic!(
                    "out of rendezvous points: {MAX_RENDEZVOUS} live at once, raise MAX_RENDEZVOUS"
                )
            }
        }
    }
}

// --- Notifications (milestone 151, DECISIONS §101) --------------------------------------------
//
// The asynchronous half of IPC: a word a signaller ORs into and a waiter takes. The decision core
// (what a signal, a wait and a poll do) is `inter_process_communication::notification`, proved
// there; what lives here is what the crate cannot see: the registry, the mailboxes, the wake, and
// the TCB binding, which reaches into another object's wait queue. See notes/notification-objects.md.

/// The most notifications that can exist at once, whole machine. A registry bound like
/// [`MAX_RENDEZVOUS`], and half of it, because a notification is a per-thread or per-service
/// doorbell where a rendezvous is a per-conversation object: §101's consumers want about one each.
/// The pages come from the creators' own regions; this bounds only the name table, which is 16 bytes
/// a slot. Raise it when a real workload refuses a `RETYPE_OBJ` here, not before.
const MAX_NOTIFICATIONS: usize = 256;

/// A notification's name: a generational name over the notification registry, what an
/// `Object::Notification` capability carries. Stale-safe like every other name.
pub type NotificationId = u64;

/// **What lives at the start of a notification's page**: the proved state machine, and the one
/// thread it is bound to, if any. The binding sits here and on the thread (`Thread::bound_notification`)
/// because each side needs it without a search: a signal asks "who is bound to me", and a receive
/// asks "what is bound to me".
struct NotificationPage {
    state: inter_process_communication::notification::Notification<Thread>,
    /// Set once by `BIND` and never cleared. A generational name, so a dead thread leaves it stale
    /// and every reader treats a miss as unbound.
    bound: Option<ThreadId>,
}

/// The notification behind a name, or `None` if it no longer resolves. Caller holds `IPC_TABLES`.
/// The `'static` is [`rendezvous_of`]'s, for the same reason: the page is pinned while the name
/// resolves, and `IPC_TABLES` serializes every access.
fn notification_of(sched: &IpcTables, id: NotificationId) -> Option<&'static mut NotificationPage> {
    let phys = *sched.notification_table.get(id)?;
    // SAFETY: retyped exclusively for this notification, its region pinned while the name
    // resolves, direct-mapped, and serialized by IPC_TABLES, which every caller holds.
    Some(unsafe { &mut *(crate::arch::mmu::phys_to_virt(phys) as *mut NotificationPage) })
}

/// Create a notification **in `region`'s memory**: one page retyped and pinned, the object at its
/// start, a fresh name. `RETYPE_OBJ`'s engine, and [`try_create_rendezvous_from`]'s shape, including
/// its fix: the registry is checked *before* a page is spent. `None` when the region or the registry
/// is full; userspace gets one flat `OutOfMemory` for both, as it does for a rendezvous.
pub fn create_notification_from(region: u64) -> Option<NotificationId> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut()?;
    if sched.notification_table.len() >= MAX_NOTIFICATIONS {
        return None;
    }
    let phys = crate::memory_region::retype_object_page(
        region,
        crate::memory_region::ObjectKind::Notification,
    )?;
    // SAFETY: fresh page, exclusively ours, direct-mapped.
    unsafe {
        (crate::arch::mmu::phys_to_virt(phys) as *mut NotificationPage).write(NotificationPage {
            state: inter_process_communication::notification::Notification::new(),
            bound: None,
        });
    }
    // Cannot fail: capacity was checked above under this same lock hold.
    sched.notification_table.insert_with(|_| phys)
}

/// **The mailbox of a receive the bound notification ended.** `(BOUND, word, 0, 0, BOUND)`:
/// §101's `w0 = 2` and the word in `w1`, and the tag again in `w4`, the one register of a receive
/// that no sender can write. See `abi::notification::BOUND`, and notes/notification-objects.md for
/// why `w0` alone is forgeable.
const fn bound_delivery(word: u64) -> [u64; 5] {
    [
        abi::notification::BOUND,
        word,
        0,
        0,
        abi::notification::BOUND,
    ]
}

/// **The bound thread, if it is parked in a receive right now**, with the rendezvous it is parked
/// on. This is the one boolean the crate's `signal` takes on trust, so the argument that it is right
/// lives here.
///
/// "Parked in a receive" is `Blocked` on a rendezvous as a `Receiver` **with nothing delivered
/// yet**, and the last clause is not decoration. A receiver a sender has already served stays
/// `Blocked` with its `wait_on` intact until the wake completes, and that can be a whole context
/// switch later (a wake deferred behind `on_cpu` finishes in `finish_switch`). It is off the queue
/// and its mailbox is full; delivering a notification there would overwrite the message. Every way
/// off a receiver queue marks the thread delivered or aborted in the same critical section (a
/// sender's `serve`, an IRQ signal's `serve`, a drain's `abort`, a bound delivery's `serve`), except
/// `finish_blocked_resident`, which ends the thread (`Finished`, not `Blocked`). So this test is
/// exactly "still linked on that receiver queue", and [`deliver_bound`] asserts it.
///
/// `RECEIVE`, `RECEIVE_CAP` and `Irq::WAIT` all park this way, so the binding wakes all three: one rule
/// for "blocked receiving on an endpoint", which is §101's phrase.
fn bound_receiver(sched: &IpcTables, page: &NotificationPage) -> Option<(ThreadId, RendezvousId)> {
    let tid = page.bound?;
    let t = sched.threads.get(tid)?;
    match t.handshake.wait_on {
        Some(Wait::Rendezvous(ep, WaitRole::Receiver))
            if t.handshake.state == State::Blocked && !t.handshake.is_delivered() =>
        {
            Some((tid, ep))
        }
        _ => None,
    }
}

/// **Deliver `word` to a bound thread parked receiving on `ep`**: unlink it from that rendezvous's
/// receiver queue, fill its mailbox with [`bound_delivery`], and record the delivery. The caller
/// wakes it. Caller holds `IPC_TABLES` and got `(tid, ep)` from [`bound_receiver`] in the same hold.
///
/// **The unlink is `remove_receiver`'s drain-and-repush**, O(receivers queued on `ep`), which is the
/// cost §101's binding pays for reaching into another object's queue with a singly linked FIFO.
/// A server's endpoint usually holds one receiver (the server itself), so this is one pop and one
/// push in the common case.
fn deliver_bound(sched: &mut IpcTables, tid: ThreadId, ep: RendezvousId, word: u64) {
    let ptr = thread_control_block_ptr(sched, tid);
    // `ptr` is only compared by the remove; the token it hands back goes home to the thread.
    let unlinked = rendezvous_of(sched, ep).and_then(|r| r.remove_receiver(ptr));
    debug_assert!(
        unlinked.is_some(),
        "a bound receiver was not on the receiver queue it was parked on"
    );
    if let Some(token) = unlinked {
        hold_token(token);
    }
    let t = sched
        .threads
        .get_mut(tid)
        .expect("bound receiver vanished under IPC_TABLES");
    t.mailbox = bound_delivery(word);
    t.handshake.serve(); // delivered: this wake passes the boot-8 gate
    trace::record(trace::Event::Served, tid, 10);
}

/// Where a notification wake queues the thread it woke. A signal from a thread is a
/// [`wake`]: local, because the signaller's core is warm and the pair is often a conversation. A
/// signal from interrupt context is [`wake_load_aware`], the device-IRQ placement of §28 (SMP
/// placement) step 2, because a timer expiry (milestone 106 (a wait that ends on either the interrupt
/// or the deadline)) or an interrupt carries no such locality.
#[derive(Clone, Copy)]
enum WakePlacement {
    Local,
    // The timer expiry walk (`expire_timers`, milestone 106) and `signal_notification_from_interrupt`.
    LoadAware,
}

/// Wake `tid` by `placement`. Returns the remote core to poke once `IPC_TABLES` is released, as
/// [`wake_load_aware`] does; a local wake never needs one.
fn wake_placed(sched: &mut IpcTables, tid: ThreadId, placement: WakePlacement) -> Option<usize> {
    match placement {
        WakePlacement::Local => {
            wake(sched, tid);
            None
        }
        WakePlacement::LoadAware => wake_load_aware(sched, tid),
    }
}

/// **The one signal path**, for a thread's `SIGNAL` and for a kernel-originated signal alike.
/// Caller holds `IPC_TABLES`. `Err(Gone)` if the name no longer resolves; otherwise the remote core
/// to poke, if the wake placed a thread there.
///
/// **Why one function with a placement rather than two**: milestone 106's timer
/// (DECISIONS §147 (a timer a userspace service cannot hold), `Timer::ARM(deadline, notification)`) will signal from the tick, in interrupt
/// context, on the interrupt stack. Everything here is already legal there, for the reason
/// `irq_notify` is: `IPC_TABLES` masks interrupts while held, a wake is an enqueue and not a switch,
/// and nothing allocates. So the kernel's signal and the user's differ only in where the woken
/// thread is placed, and a second copy of the delivery logic would be the drift the tree keeps
/// paying for.
fn signal_locked(
    sched: &mut IpcTables,
    id: NotificationId,
    bits: u64,
    placement: WakePlacement,
) -> Result<Option<usize>, abi::Error> {
    use inter_process_communication::notification::Signal;
    let page = notification_of(sched, id).ok_or(abi::Error::Gone)?;
    let bound = bound_receiver(sched, page);
    match page.state.signal(bits, bound.is_some()) {
        Signal::Woke(waiter, word) => {
            let tid = hold_token(waiter);
            let t = sched
                .threads
                .get_mut(tid)
                .expect("a notification waiter vanished under IPC_TABLES");
            t.mailbox = [word, 0, 0, 0, 0];
            t.handshake.serve();
            trace::record(trace::Event::Served, tid, 9);
            Ok(wake_placed(sched, tid, placement))
        }
        Signal::ToBound(word) => {
            let (tid, ep) = bound.expect("the crate answered ToBound without a bound receiver");
            deliver_bound(sched, tid, ep, word);
            Ok(wake_placed(sched, tid, placement))
        }
        Signal::Counted | Signal::Empty => Ok(None),
    }
}

/// **`Notification::SIGNAL`**: OR `bits` in, waking a waiter or the bound receiver. Never blocks.
pub fn notification_signal(id: NotificationId, bits: u64) -> Result<(), abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    // A local wake never returns a core to poke.
    signal_locked(sched, id, bits, WakePlacement::Local).map(|_| ())
}

/// **Signal a notification from interrupt context**: the kernel-originated signal milestone 106's
/// timer expiry will call from the tick, and §147's argument says IRQ delivery can use too. Safe
/// from an interrupt handler for [`irq_notify`]'s reason, and placed load-aware for its reason. A
/// stale name is dropped silently, as `irq_notify` drops a revoked route: an expiry with no
/// notification left to signal has nowhere to go, which is not an error.
///
/// **Still no caller outside the tests, and milestone 106 is why.** The timer's expiry walk
/// ([`expire_timers`]) fires several timers under one hold of `IPC_TABLES`, so it calls
/// [`signal_locked`] with this function's placement directly rather than taking and dropping the
/// lock once per timer through here. This entry stays for a kernel source that signals one
/// notification at a time (§147's argument that IRQ delivery could use it); a kernel test exercises
/// it so it is not dead code that merely compiles.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn signal_notification_from_interrupt(id: NotificationId, bits: u64) {
    let remote = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        signal_locked(sched, id, bits, WakePlacement::LoadAware)
            .ok()
            .flatten()
    };
    if let Some(target) = remote {
        crate::arch::irq::send_reschedule(target);
    }
}

/// **`Notification::WAIT`**: take the word, or block until a signal arrives. `Err(Gone)` if the
/// notification is stale, or is destroyed while this thread waits (the region sweep aborts its
/// waiters, exactly as it aborts a rendezvous's).
pub fn notification_wait(id: NotificationId) -> Result<u64, abi::Error> {
    use inter_process_communication::notification::Wait as Waited;
    let immediate = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();
        let page = notification_of(sched, id).ok_or(abi::Error::Gone)?;
        // The running thread's token: if it queues, it stays live, since a thread waiting here is
        // Blocked, which the reaper never frees.
        match page.state.wait(take_token(sched, current)) {
            Waited::Word(word, me) => {
                hold_token(me);
                Some(word)
            }
            Waited::Blocked => {
                let t = sched.threads.get_mut(current).expect("running thread");
                t.handshake.park(Wait::Notification(id)); // only a signal (or an abort) may wake us
                trace::record(trace::Event::BlockSelf, current, id as u8);
                None
            }
        }
    };
    if let Some(word) = immediate {
        return Ok(word);
    }
    schedule(); // blocks; a signal fills our mailbox and wakes us
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let t = sched
        .threads
        .get_mut(current_thread_id())
        .expect("running thread");
    debug_assert!(
        t.handshake.is_delivered(),
        "notification wait resumed with nothing delivered"
    );
    if t.handshake.take_aborted() {
        return Err(abi::Error::Gone);
    }
    Ok(t.mailbox[0])
}

/// **`Notification::POLL`**: take the word without blocking; `0` if nothing was pending.
pub fn notification_poll(id: NotificationId) -> Result<u64, abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let page = notification_of(sched, id).ok_or(abi::Error::Gone)?;
    Ok(page.state.poll())
}

/// **`Notification::BIND`**: bind notification `id` to thread `tid`, once each (§101: "at most one
/// notification may be bound to a TCB", and one bound TCB per notification).
///
/// - `Gone`: the notification is stale, or the thread is gone or already dead.
/// - `NotPermitted`: either side is already bound to something that still exists. A binding whose
///   other half has been destroyed does not count, so a thread whose notification was reclaimed
///   can be bound again.
///
/// **A word already waiting is delivered at once if the thread is already receiving.** Without
/// this, a signal counted before the bind would sit in the word until the thread's *next* receive,
/// which for a server blocked forever in `RECEIVE` is never: not lost, but not delivered either.
pub fn notification_bind(id: NotificationId, tid: ThreadId) -> Result<(), abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let page = notification_of(sched, id).ok_or(abi::Error::Gone)?;
    if page.bound.is_some_and(|b| sched.threads.get(b).is_some()) {
        return Err(abi::Error::NotPermitted);
    }
    let t = sched.threads.get(tid).ok_or(abi::Error::Gone)?;
    if matches!(t.handshake.state, State::Finished | State::Dead) {
        return Err(abi::Error::Gone);
    }
    if t.bound_notification
        .is_some_and(|n| notification_of(sched, n).is_some())
    {
        return Err(abi::Error::NotPermitted);
    }
    page.bound = Some(tid);
    sched
        .threads
        .get_mut(tid)
        .expect("checked above under this hold")
        .bound_notification = Some(id);

    if page.state.word() != 0
        && let Some((tid, ep)) = bound_receiver(sched, page)
    {
        let word = page.state.poll();
        deliver_bound(sched, tid, ep, word);
        wake(sched, tid);
    }
    Ok(())
}

/// **The receive-side half of the binding**: on entry to a receive, a bound thread takes a word
/// that was counted while it was elsewhere, and returns without blocking. `None` when the thread
/// has no live binding or the word is zero. Caller holds `IPC_TABLES`.
///
/// It returns the bare word rather than the five-word delivery because a `u64` comes back in a
/// register and an array through memory; the caller builds [`bound_delivery`] from constants.
///
/// **`#[cold]` and `#[inline(never)]`, and the claim is about who pays.** The receive fastpath
/// tests `bound_notification.is_some()` on the thread it already holds, which is §101's "one load
/// and one compare"; only a bound thread reaches this body. Keeping it out of line keeps its bytes
/// out of `script/fastpath-footprint`'s closure, the same reason `set_ipc_aborted` is out of line.
#[cold]
#[inline(never)]
fn take_bound_signal(sched: &mut IpcTables, tid: ThreadId) -> Option<u64> {
    let id = sched.threads.get(tid)?.bound_notification?;
    match notification_of(sched, id)?.state.poll() {
        0 => None,
        word => Some(word),
    }
}

/// **Tear down every notification whose page lies in `[base, end)`** (a region being destroyed):
/// abort and wake each waiter, then drop the name. The rendezvous sweep's shape, including its rule
/// to rescan rather than list, for its stack-depth reason. A thread bound to a notification destroyed
/// here keeps a stale name, which every reader treats as unbound. Caller holds `IPC_TABLES`.
fn reap_region_notifications(sched: &mut IpcTables, base: u64, end: u64) {
    loop {
        let doomed = sched
            .notification_table
            .iter()
            .find(|&(_, &phys)| base <= phys && phys < end)
            .map(|(name, _)| name);
        let Some(name) = doomed else { break };
        if let Some(page) = notification_of(sched, name) {
            page.state.drain_waiters(|w| {
                let tid = hold_token(w);
                set_ipc_aborted(sched, tid);
                wake(sched, tid);
            });
        }
        sched.notification_table.remove(name);
    }
}

/// The most timers that can exist at once, whole machine: the notification registry's bound, for
/// its reason. A timer is per-waiter (a sleeping thread, a retransmit window), so §147's consumers
/// want about one each. The expiry walk is O(this) at worst, and runs only on a due tick.
const MAX_TIMERS: usize = 256;

/// A timer's name: a generational name over the timer registry, what an `Object::Timer`
/// capability carries. *(Provisional, as the object type is.)*
pub type TimerId = u64;

/// **What lives at the start of a timer's page**: the proved decision core, armed with the
/// notification to signal and the bits to signal it with.
struct TimerPage {
    state: inter_process_communication::timer::Timer<(NotificationId, u64)>,
}

/// **The cached earliest deadline, in counter ticks**: never later than any armed timer's deadline
/// (the invariant `inter_process_communication::timer` proves), `NEVER` when nothing is armed.
///
/// An atomic beside `IPC_TABLES` rather than a field in it, because the tick reads it **without**
/// the lock, which is the whole point: the idle tick is one load and one compare. Every write happens
/// under `IPC_TABLES`, so writers never race each other; a tick on another core may read a value one
/// write stale, and both directions are safe. Stale-high (an arm not yet visible) delays that expiry
/// to a later tick, bounded by the lock release that publishes it; stale-low costs one walk that
/// finds nothing. `Relaxed` is enough for that reason, and the walk itself re-reads everything
/// under the lock.
static EARLIEST_DEADLINE: AtomicU64 = AtomicU64::new(inter_process_communication::timer::NEVER);

/// The timer behind a name, or `None` if it no longer resolves. Caller holds `IPC_TABLES`; the
/// `'static` is [`notification_of`]'s.
fn timer_of(sched: &IpcTables, id: TimerId) -> Option<&'static mut TimerPage> {
    let phys = *sched.timer_table.get(id)?;
    // SAFETY: retyped exclusively for this timer, its region pinned while the name resolves,
    // direct-mapped, and serialized by IPC_TABLES, which every caller holds.
    Some(unsafe { &mut *(crate::arch::mmu::phys_to_virt(phys) as *mut TimerPage) })
}

/// Create a timer **in `region`'s memory**, disarmed. [`create_notification_from`]'s shape,
/// registry checked before a page is spent.
pub fn create_timer_from(region: u64) -> Option<TimerId> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut()?;
    if sched.timer_table.len() >= MAX_TIMERS {
        return None;
    }
    let phys =
        crate::memory_region::retype_object_page(region, crate::memory_region::ObjectKind::Timer)?;
    // SAFETY: fresh page, exclusively ours, direct-mapped.
    unsafe {
        (crate::arch::mmu::phys_to_virt(phys) as *mut TimerPage).write(TimerPage {
            state: inter_process_communication::timer::Timer::new(),
        });
    }
    sched.timer_table.insert_with(|_| phys)
}

/// **`Timer::ARM`**: arm `id` to signal `bits` into `notification` at `deadline` (counter ticks),
/// replacing any pending deadline. A deadline already reached signals now, from here, with a
/// thread's placement (the arming thread's core is warm). `Gone` if either name is stale.
pub fn timer_arm(
    id: TimerId,
    deadline: u64,
    notification: NotificationId,
    bits: u64,
) -> Result<(), abi::Error> {
    use inter_process_communication::timer::{Arm, lowered};
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    // Refuse a dead notification at arm time rather than letting the expiry discover it: the
    // caller can still act on `Gone` now, and nobody can act on it at the deadline.
    notification_of(sched, notification).ok_or(abi::Error::Gone)?;
    let page = timer_of(sched, id).ok_or(abi::Error::Gone)?;
    let now = crate::arch::timer::now();
    match page.state.arm(deadline, (notification, bits), now) {
        Arm::Pending => {
            let cached = EARLIEST_DEADLINE.load(Ordering::Relaxed);
            EARLIEST_DEADLINE.store(lowered(cached, deadline), Ordering::Relaxed);
            Ok(())
        }
        // A local wake never returns a core to poke; the notification was checked above, so a
        // `Gone` here is impossible under this same hold.
        Arm::DueNow((n, b)) => signal_locked(sched, n, b, WakePlacement::Local).map(|_| ()),
    }
}

/// **`Timer::CANCEL`**: disarm `id`. `true` if a deadline was pending and now never fires. The cache
/// is deliberately left where it is: lowering is all an arm does, a walk is what raises it, and a
/// stale-low cache costs one walk that fires nothing. `Gone` if the name is stale.
pub fn timer_cancel(id: TimerId) -> Result<bool, abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    Ok(timer_of(sched, id).ok_or(abi::Error::Gone)?.state.cancel())
}

/// **The expiry walk**, from the tick, in interrupt context, on the interrupt stack: fire every due
/// timer, then recompute the cache exactly. Legal here for [`signal_notification_from_interrupt`]'s
/// reason (a wake is an enqueue, `IPC_TABLES` masks interrupts, nothing allocates), and it goes
/// through the same [`signal_locked`] with the same load-aware placement.
///
/// **Rescan rather than list**, the region sweeps' shape: each step finds the first due timer,
/// disarms it and signals, so no buffer bounds how many may fire in one tick. The cost is O(timers)
/// per firing, paid only on a due tick.
///
/// `#[cold]` and out of line so its bytes stay in its own symbol rather than in `on_tick`'s, for
/// `on_tick`'s own reason (riscv64 counts the trap path flat).
#[cold]
#[inline(never)]
fn expire_timers() {
    use inter_process_communication::timer::{earliest, next_due};
    let mut poke: u64 = 0; // one bit per core a wake placed a thread on
    {
        let mut guard = IPC_TABLES.lock();
        let Some(sched) = guard.as_mut() else { return };
        let now = crate::arch::timer::now();
        loop {
            let fired = next_due(
                sched
                    .timer_table
                    .values()
                    // SAFETY: each value is a live timer page, as `timer_of` argues.
                    .map(|&phys| unsafe {
                        &mut (*(crate::arch::mmu::phys_to_virt(phys) as *mut TimerPage)).state
                    }),
                now,
            );
            let Some((notification, bits)) = fired else {
                break;
            };
            // A notification destroyed while the timer was armed: nobody left to tell, dropped
            // as `signal_notification_from_interrupt` drops a stale name.
            if let Ok(Some(cpu)) =
                signal_locked(sched, notification, bits, WakePlacement::LoadAware)
            {
                poke |= 1 << cpu;
            }
        }
        let cached = earliest(sched.timer_table.values().map(|&phys| {
            // SAFETY: as above.
            unsafe { &(*(crate::arch::mmu::phys_to_virt(phys) as *const TimerPage)).state }
        }));
        EARLIEST_DEADLINE.store(cached, Ordering::Relaxed);
    }
    for cpu in 0..cpu::MAX_CPUS {
        if poke & (1 << cpu) != 0 {
            crate::arch::irq::send_reschedule(cpu);
        }
    }
}

/// **Tear down every timer whose page lies in `[base, end)`**: drop the name. A timer has no
/// waiters of its own, so there is nothing to abort; an armed one simply never fires. The cache may
/// be left stale-low, which is safe. Caller holds `IPC_TABLES`.
fn reap_region_timers(sched: &mut IpcTables, base: u64, end: u64) {
    loop {
        let doomed = sched
            .timer_table
            .iter()
            .find(|&(_, &phys)| base <= phys && phys < end)
            .map(|(name, _)| name);
        let Some(name) = doomed else { break };
        sched.timer_table.remove(name);
    }
}

/// **Which core should a device-IRQ wake place its driver on** (DECISIONS §28.2). The least-loaded
/// online core, with the current (IRQ-handling) core winning ties: only a *strictly* less-loaded
/// core displaces it. That is what makes this load-aware without thrashing. A driver that takes a
/// completion interrupt every request (the block server through a RedoxFS mount) wakes on the same
/// affinity core each time and, since that core is no more loaded than any other while it is the
/// only work, stays there. When a core does pile up (the `std_net` RX path landing beside real work),
/// a strictly-lighter core pulls the driver off it, so the pipeline stops re-concentrating. A full
/// scan is fine: device IRQs are not the spawn hot path and `MAX_CPUS` is small.
fn pick_wake_target() -> usize {
    let here = cpu::id();
    let mut best = here;
    let mut best_load = cpu::current().runnable();
    // The online SET, never `0..count` (first-silicon sweep, 2026-08-14): on the VisionFive 2 the
    // set is {1,2,3}, and the count-as-index loop would read parked slot 0's zeroed `runnable()`,
    // which wins every comparison, so every device-IRQ wake would land in a dead core's inbox. It
    // also never considered online cpu 3. See smp::online_cpus.
    for c in crate::smp::online_cpus() {
        if c == here {
            continue;
        }
        let load = cpu::of(c).runnable();
        if load < best_load {
            best_load = load;
            best = c;
        }
    }
    best
}

/// A **device-interrupt** wake (DECISIONS §28.2): load-aware, not local. Where [`wake`] queues a
/// rendezvous partner on the waker's own core (message in registers, cache warm), an interrupt
/// carries no such locality, and pinning the driver to the IRQ core re-concentrates the pipeline
/// (the `std_net` lesson). So place it on [`pick_wake_target`]'s choice. Returns `Some(target)` when
/// that is a *remote* core, so the caller sends the reschedule SGI after releasing `IPC_TABLES`; `None`
/// when it stayed local or the wake was parked. Caller holds the lock.
fn wake_load_aware(sched: &mut IpcTables, tid: ThreadId) -> Option<usize> {
    let t = sched.threads.get_mut(tid)?;
    // The whole decision (not-blocked, the boot-8 undelivered-wake gate, the switch-out deferral)
    // is `thread_wake_handshake::Handshake::try_wake`, the extracted protocol loom searches on the host
    // (notes/interleaving.md). This function keeps what the crate cannot see: the trace ring, the
    // progress heartbeat, and §28.2's placement policy on the one verdict that queues.
    match t.handshake.try_wake() {
        WakeVerdict::NotBlocked => None,
        WakeVerdict::Refused => {
            trace::record(trace::Event::WakeRefused, tid, 0);
            None
        }
        WakeVerdict::Deferred => {
            // A device-IRQ wake is forward progress too (test builds only). A deferral in this
            // window is rare, and one non-load-aware completion in `finish_switch` is not worth
            // teaching that path a placement policy.
            #[cfg(any(test, feature = "system_tests"))]
            crate::testing::note_progress();
            trace::record(trace::Event::WakeDeferred, tid, 0);
            None
        }
        WakeVerdict::Queue => {
            #[cfg(any(test, feature = "system_tests"))]
            crate::testing::note_progress();
            // Blocked -> Ready: whatever unlinked it handed its token back (`hold_token`).
            let Some(token) = t.own_token.take() else {
                missing_token()
            };
            trace::record(trace::Event::Wake, tid, 0);
            // One placement decision, `place_on`'s: onto this core's own run queue, or into the
            // target's inbox with the inbox-len mirror kept under the inbox lock. Its return value
            // is what tells `irq_notify` to poke the target once IPC_TABLES drops, and it is the
            // only thing that may: a second `target == cpu::id()` comparison outside the lock can
            // disagree with the one that placed (see `place_on`).
            //
            // IPC_TABLES masks interrupts, which `with_runq` needs on the local side.
            place_on(pick_wake_target(), token)
        }
    }
}

/// Move a blocked thread back to the ready queue. Caller holds the lock.
///
/// The decision lives in `thread_wake_handshake::Handshake::try_wake`, extracted so loom can search its
/// interleavings on the host (notes/interleaving.md); this function is the kernel-side half, the
/// queue push and the trace ring. The two rules the verdicts carry, kept here in one breath
/// because this is where a reader meets them: **the undelivered-wake gate** (boot 8: a wake whose
/// critical section delivered nothing has not dequeued the thread from its rendezvous and has
/// nothing for its receive to return, so it is refused and recorded as `refuse:tid` on the ring), and
/// **the wake-before-switch-out deferral** (a thread still on its CPU has a stale saved context,
/// so the wake parks in `wake_pending` and its own core's `finish_switch` completes it once the
/// context is provably saved; found by a 2-in-10 flake, notes/intrusive-queues.md).
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(target_os = "none", unsafe(link_section = ".text.hot.sched.wake"))]
fn wake(sched: &mut IpcTables, tid: ThreadId) {
    if let Some(t) = sched.threads.get_mut(tid) {
        match t.handshake.try_wake() {
            WakeVerdict::NotBlocked => {}
            WakeVerdict::Refused => {
                trace::record(trace::Event::WakeRefused, tid, 0);
            }
            WakeVerdict::Deferred => {
                // A completed rendezvous is forward progress even when its queueing is deferred:
                // keep the hang watchdog's heartbeat alive so a slow-but-live IPC pipeline
                // (std_net) is not read as a deadlock (test builds only).
                #[cfg(any(test, feature = "system_tests"))]
                crate::testing::note_progress();
                trace::record(trace::Event::WakeDeferred, tid, 0);
            }
            WakeVerdict::Queue => {
                #[cfg(any(test, feature = "system_tests"))]
                crate::testing::note_progress();
                // Blocked -> Ready: whatever unlinked it handed its token back (`hold_token`), so
                // it is on the thread; a `None` is a thread still linked into a wait queue.
                let Some(token) = t.own_token.take() else {
                    missing_token()
                };
                trace::record(trace::Event::Wake, tid, 0);
                // Onto this core's queue: a rendezvous wake stays local on purpose (§28.2), the
                // message is in registers and the cache is warm. Every caller (ipc_*, irq_notify)
                // holds IPC_TABLES, so interrupts are masked.
                cpu::current().with_runq(|q| q.push_back(token));
            }
        }
    }
}

/// Widen an ordinary three-word IPC message into the five-word mailbox. Word 3 is the badge on the
/// endpoint capability the sender invoked (0 when unbadged), and word 4 is zero; only a fault/exit
/// message (DECISIONS §26) puts anything else in the top two, and a `RECEIVE` hands all five back.
/// Keeping the mailbox one width means the fault path reuses the same rendezvous machinery rather
/// than growing a parallel one.
///
/// **Why a plain `SEND` carries its badge** (milestone 613 (a system log service), provisional):
/// §230 (badged endpoint capabilities) delivered a badge on `CALL` and `SEND_CAP` only, because its
/// one customer, the file server, is a `CALL` protocol. §242 (a system log) stamps every record
/// from the writer's badge, and a log writer speaks the byte sink, which is a plain `SEND`; without
/// this word the badge a spawner minted would be silently dropped on exactly the path it was
/// minted for. The store is the one `wide` already made (a zero became the badge), the same
/// no-extra-instruction argument `ipc_send_cap`'s word 3 makes.
fn wide(m: [u64; 3], badge: u64) -> [u64; 5] {
    [m[0], m[1], m[2], badge, 0]
}

/// **Send three words to an rendezvous, blocking until a receiver takes them.**
///
/// The synchronous rendezvous, sender's half:
///
/// - **A receiver is already waiting.** Drop the message straight into its mailbox, wake it, and
///   carry on. Nobody blocked; the rendezvous was instantaneous.
/// - **Nobody is waiting.** Park the message in our own mailbox, join the rendezvous's sender
///   queue, mark ourselves `Blocked`, and `schedule()` away. A future receiver will reach into
///   our mailbox, wake us, and we return from `schedule()` as if no time had passed.
///
/// Callable by a kernel thread directly (this function) or by a user thread through the `SEND`
/// method on an rendezvous capability (see syscall.rs). Same code underneath.
#[inline(always)]
pub fn ipc_send(ep: RendezvousId, msg: [u64; 3]) {
    ipc_send_badged(ep, msg, 0);
}

/// [`ipc_send`] through a badged endpoint capability: the receiver's `RECEIVE` sees `badge` in word 3
/// (milestone 613 (a system log service), provisional; see [`wide`] for why a plain send carries
/// it). The syscall layer passes the badge of the capability the sender invoked, so it is the
/// kernel's word and never the sender's. [`ipc_call_badged`] is the same split for `CALL`.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.ipc_send_badged")
)]
pub fn ipc_send_badged(ep: RendezvousId, msg: [u64; 3], badge: u64) {
    // E3's footprint-perturbation experiment (milestone 134): reachable but never taken; see
    // `crate::fastpath_pad` for what this is and why it costs nothing when the feature is off.
    #[cfg(feature = "fastpath_pad")]
    crate::fastpath_pad::maybe_pad();
    let msg = wide(msg, badge);
    let block = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();

        // A stale rendezvous (its region was revoked): mark this send aborted and do not block. The
        // kernel-side `ipc_send` wrapper never hits this (its endpoints are never revoked); the
        // syscall layer reads the flag and returns an error.
        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            set_ipc_aborted(sched, current);
            return;
        };
        // The running thread's token: if it queues, it stays live, since a thread queued on a
        // rendezvous is Blocked, which the reaper never touches. Every verdict that did not queue
        // it hands it back, and it goes home to the thread (`hold_token`).
        match rendezvous.send(take_token(sched, current)) {
            inter_process_communication::Send::Rendezvous(receiver, me) => {
                hold_token(me);
                let receiver = hold_token(receiver);
                let r = sched.threads.get_mut(receiver).unwrap();
                r.mailbox = msg;
                r.handshake.serve(); // delivered: this wake passes the boot-8 gate
                trace::record(trace::Event::Served, receiver, 1);
                wake(sched, receiver);
                false
            }
            inter_process_communication::Send::Blocked => {
                // `send` has already queued `current` as a sender; we record why it is parked.
                let me = sched.threads.get_mut(current).unwrap();
                me.mailbox = msg;
                me.handshake.park(Wait::Rendezvous(ep, WaitRole::Sender)); // only a collecting receiver may wake us
                trace::record(trace::Event::BlockSelf, current, ep as u8);
                true
            }
            // The rendezvous carries an interrupt (§101 ruling B): nothing was delivered or queued.
            inter_process_communication::Send::Refused(me) => {
                hold_token(me);
                set_ipc_refused(sched, current);
                false
            }
        }
    };

    // Block OUTSIDE the lock (rule 1), and only after we have already recorded ourselves as
    // blocked, so a timer-driven `schedule()` in the gap does the right thing either way.
    if block {
        schedule();
    }
}

/// **Receive three words from an rendezvous, blocking until one arrives.** The mirror of
/// [`ipc_send`].
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.ipc_receive")
)]
pub fn ipc_receive(ep: RendezvousId) -> [u64; 5] {
    let immediate = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();

        // A stale rendezvous (revoked): mark aborted and return a placeholder; the syscall layer sees
        // the flag and errors. (A thread revoked *while blocked* below is handled the same way: the
        // reaper sets the flag and wakes it, and it returns its stale mailbox for the layer to drop.)
        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            set_ipc_aborted(sched, current);
            return [0, 0, 0, 0, 0];
        };
        // One lookup serves both things this path needs from the running thread: whether it is
        // bound, and its token (milestone 139 (drive the unsafe count down), round 10, which also
        // retired the raw-pointer read the binding test used to make).
        let me = sched.threads.get_mut(current).expect("running thread");
        let bound = me.bound_notification.is_some();
        let Some(token) = me.own_token.take() else {
            missing_token()
        };
        // **The binding's receive-side half** (milestone 151, DECISIONS §101): a signal counted
        // while this thread was elsewhere ends the receive before it can block. First, as a pending
        // IRQ signal is taken before a queued sender, and for the same reason: it arrived earlier.
        // This `is_some` on the thread already in hand is the one load and one branch §101 priced
        // onto this path; only a bound thread goes further.
        //
        //
        // **An arm of the same decision, not an early return**, and the difference is measured: a
        // `return` from inside the lock hold gave the function a second copy of the unlock path,
        // and cost `ipc_receive` 138 bytes on riscv64 and 174 on `x86_64`. Joining the other arms
        // shares the one release below.
        //
        if bound && let Some(word) = take_bound_signal(sched, current) {
            hold_token(token);
            Some(bound_delivery(word))
        } else {
            // As in ipc_send: the running thread's token, and Blocked-while-queued keeps it live.
            match rendezvous.receive(token) {
                // An interrupt already fired while we were not waiting. Take it and do not block.
                inter_process_communication::Receive::Signal(me) => {
                    hold_token(me);
                    Some([1, 0, 0, 0, 0])
                }
                inter_process_communication::Receive::FromSender(sender, me) => {
                    hold_token(me);
                    let sender = hold_token(sender);
                    let msg = sched.threads.get(sender).unwrap().mailbox;
                    // A **dead sender** (a §26 corpse) and a **caller** (its outgoing cap is the
                    // one-shot Reply a CALL minted, §12 (call/reply IPC)) get their words delivered
                    // and are not woken as a completed send: the corpse never runs again, and the
                    // caller is answered `Gone` (§246 (a plain `RECEIVE` never takes a capability),
                    // PROVISIONAL number). See `collected_without_serving`.
                    let leave_blocked = matches!(
                        sched.threads.get(sender).unwrap().outgoing_cap,
                        Some(c) if matches!(c.object, crate::cap::Object::Reply(_))
                    ) || sched.threads.get(sender).unwrap().handshake.state
                        == State::Dead;
                    if !leave_blocked {
                        // **A plain RECEIVE collected this sender, and a plain RECEIVE delivers no
                        // capability** (milestone 633 (an outside agent attacks the confinement
                        // claim), fatal risk 7's outsider pass, 2026-10-03
                        // UTC). A `SEND_CAP` sender parked its delegation in `outgoing_cap` for a
                        // receiver to take; this receiver did not take it, and the rendezvous is now
                        // complete. Left in place, the delegation would ride the sender's next plain
                        // `SEND` on a *different* rendezvous and reach whoever `RECEIVE_CAP`s there, a
                        // delegation made to one endpoint delivered to another. This is the exact
                        // hazard `set_ipc_aborted` closes on the teardown path; the successful-collect
                        // path does not go through it, so it is closed here too. The sender still
                        // holds its own copy (`SEND_CAP` narrows a copy, never moving the source), so
                        // dropping this one loses nothing, which is `ipc_send_cap`'s documented
                        // cap-table-full semantics: the data word arrives and the capability is dropped.
                        sched.threads.get_mut(sender).unwrap().outgoing_cap = None;
                        // Collected: the sender's rendezvous is complete, which is what lets its
                        // wake through the boot-8 gate.
                        sched.threads.get_mut(sender).unwrap().handshake.serve();
                        trace::record(trace::Event::Served, sender, 2);
                        wake(sched, sender);
                    } else {
                        // A corpse or a caller: neither is woken as a completed send. Out of line,
                        // because neither is on the fastpath, and keeping both arms here put
                        // x86_64's `ipc_send_receive` past `script/fastpath-footprint`'s band.
                        collected_without_serving(sched, sender);
                    }
                    Some(msg)
                }
                inter_process_communication::Receive::Blocked => {
                    // `receive` has already queued `current` as a receiver.
                    let me = sched.threads.get_mut(current).unwrap();
                    me.handshake.park(Wait::Rendezvous(ep, WaitRole::Receiver)); // only a delivering sender may wake us
                    trace::record(trace::Event::BlockSelf, current, ep as u8);
                    None
                }
            }
        }
    };

    match immediate {
        Some(msg) => msg,
        None => {
            schedule(); // blocks; a sender fills our mailbox and wakes us
            let guard = IPC_TABLES.lock();
            let sched = guard.as_ref().expect("no scheduler");
            let t = sched.threads.get(current_thread_id()).unwrap();
            // The boot-8 gate makes an undelivered resume unreachable; this is its tripwire,
            // loud in every QEMU test build, on the path where the strand was observed.
            debug_assert!(
                t.handshake.is_delivered(),
                "receive resumed with nothing delivered"
            );
            t.mailbox
        }
    }
}

/// The x1 value a `RECEIVE_CAP` returns when no capability accompanied the message. Mirrors
/// `abi::rendezvous::NO_CAP`; kept here too so the scheduler names it without reaching into the ABI.
const NO_CAP: u64 = u64::MAX;

/// **A `CALL` met a receiver parked in plain `RECEIVE`** (§246 (a plain `RECEIVE` never takes a
/// capability), PROVISIONAL number; calef's ruling A, 2026-10-04 UTC). The receiver gets the
/// request's words exactly as it would had the caller parked first (`ipc_receive`'s collect), and
/// no capability: a plain `RECEIVE` has no slot it asked to have filled, and the Reply would be one
/// it never reads. The caller is answered `Gone`, since no Reply exists for anyone to send. Caller
/// holds `IPC_TABLES`; `caller` is the running thread and has not parked.
///
/// `#[cold]` and out of line for `set_ipc_aborted`'s reason: every `CALL` server in the tree
/// receives with `RECEIVE_CAP`, so the `CALL` fastpath pays one branch for this and none of its bytes.
#[cold]
#[inline(never)]
fn call_meets_plain_receive(
    sched: &mut IpcTables,
    receiver: ThreadId,
    caller: ThreadId,
    msg: [u64; 2],
    badge: u64,
) {
    let r = sched.threads.get_mut(receiver).unwrap();
    r.mailbox = [msg[0], msg[1], 0, badge, 0];
    r.handshake.serve(); // delivered: this wake passes the boot-8 gate
    trace::record(trace::Event::Served, receiver, 5);
    wake(sched, receiver);
    set_ipc_aborted(sched, caller);
}

/// **A plain `RECEIVE` collected a sender it does not wake as a completed send**: a corpse or a
/// caller. Caller holds `IPC_TABLES`; `rendezvous.receive` has already popped `sender` off the
/// sender queue.
///
/// - A **dead sender** is a fault/exit corpse parked on its supervision rendezvous (DECISIONS §26):
///   its message is delivered and it is never woken, because it is dead-until-reaped. It waits on
///   nothing now; it only awaits its reap.
/// - A **caller** is answered `Gone` (§246 (a plain `RECEIVE` never takes a capability),
///   PROVISIONAL number; calef's ruling A, 2026-10-04 UTC): the sender-first half of
///   [`call_meets_plain_receive`]. `set_ipc_aborted` drops the Reply staged in its `outgoing_cap`,
///   which was the only copy. Until the ruling it was left parked on that Reply until teardown.
///
/// A dead sender's label (milestone 105) is handed to the receiver here, the running thread, since
/// this is the one place the parked-corpse half of a death delivery reaches it. Only a plain
/// `RECEIVE` comes through this function, which is the receive §26 names for a death message.
#[cold]
#[inline(never)]
fn collected_without_serving(sched: &mut IpcTables, sender: ThreadId) {
    if sched.threads.get(sender).unwrap().handshake.state == State::Dead {
        let corpse = sched.threads.get_mut(sender).unwrap();
        corpse.handshake.wait_on = None;
        let label = corpse.fault_label;
        hand_over_label(sched.threads.get(current_thread_id()).unwrap(), label);
    } else {
        set_ipc_aborted(sched, sender);
        wake(sched, sender);
    }
}

/// **Hand a death message's label to the supervisor receiving it** (milestone 105, DECISIONS §148
/// as amended 2026-10-04, ruling R3): argument register 5 of its saved user frame, which is `x5`,
/// `a5` or `r9`. Caller holds `IPC_TABLES`.
///
/// **Beside the mailbox, not in it, and that is the benchmark condition.** Word 3 already carries
/// the fault address and word 4 is reserved for §26.4's resume protocol, so the label needs a sixth
/// word. Widening `Thread::mailbox` to six would make every IPC delivery store one more word; written
/// here, only a death pays, and `ipc_send_receive` and `ipc_call_reply` run no new instruction.
///
/// Only a **plain `RECEIVE`** gets it. A `RECEIVE_CAP` receiver lays its result out differently
/// (`x1` is a slot), and §26 names `RECEIVE` as the death message's receive. Neither does a kernel
/// thread receiving in the kernel, which has no user frame: the bytes at its stack top are its own
/// stack. A thread with an address space only ever reaches a receive by trapping from user mode.
///
/// An ordinary message writes nothing to this register, so a supervisor that wants `0` to mean
/// "not stamped by the kernel" zeroes it before the `RECEIVE` (`user_mode_runtime::receive_fault`
/// does). That is also why a child cannot forge a label: the badge on a capability it sends with
/// arrives in word 3, never here.
///
/// Name: provisional, milestone 105 (the two forks)'s lane, 2026-10-05 (UTC). An alternative, `deliver_fault_label`, was suggested
/// to match its siblings `deliver_death` and `deliver_capability`.
#[cold]
#[inline(never)]
fn hand_over_label(receiver: &crate::thread::Thread, label: u64) {
    if receiver.receiving_cap || receiver.space.is_none() {
        return;
    }
    if let Some(stack) = receiver.stack.as_ref() {
        // SAFETY: `receiver` is a user thread (it has an address space) parked in, or running, a
        // plain RECEIVE it trapped into from user mode, so its frame is at its stack top; it is
        // blocked or is this core's current thread, and IPC_TABLES is held, so nothing else writes it.
        unsafe { crate::arch::exceptions::set_user_arg(stack.top(), 5, label) };
    }
}

/// **The `x4` a receive returns for a `CALL` whose Reply landed at `slot`** (milestone 706 (a
/// `CALL` server can tell a Reply from a delegation), DECISIONS §245 (a `CALL` server tells a Reply
/// from a delegation)): `abi::rendezvous::REPLY_DELIVERED` when the Reply was installed, `0` when
/// the receiver's table was full and there is no Reply to name. Written only on the two paths a
/// `CALL` reaches a receiver (`ipc_call_badged`'s rendezvous and `ipc_receive_cap`'s collect of a
/// parked caller), so a `SEND_CAP` delegation, which reaches the same `x1`, never carries it.
#[inline(always)]
fn reply_tag(slot: u64) -> u64 {
    if slot == NO_CAP {
        0
    } else {
        abi::rendezvous::REPLY_DELIVERED
    }
}

/// **Delegate a capability plus one data word to an rendezvous.** The sender's half of a
/// capability-carrying rendezvous, mirroring [`ipc_send`]. The one thing it adds: at the moment
/// sender and receiver meet, `cap` moves out of the sender and into the receiver's capability table.
///
/// - **A receiver is already waiting.** Insert the capability into its capability table right now, record the
///   slot in its mailbox alongside the data word, and wake it.
/// - **Nobody is waiting.** Park the data word in our mailbox and the capability in `outgoing_cap`,
///   join the sender queue, and block. A future receiver reaches in, takes the capability, and
///   files it in its own capability table.
///
/// If the receiver's capability table is full the capability is dropped and the receiver sees `NO_CAP`; the
/// data word still arrives. The syscall layer has already checked the sender may delegate this
/// capability (it holds `GRANT`) and that the rights only narrow.
///
/// **For a capability the caller minted, not one read from a table.** A copy of a capability the
/// sender holds goes through [`ipc_delegate_cap`], which reads the source under the same hold of
/// `IPC_TABLES` that files the copy; see [`Delegation`] for why the two cannot be separate steps.
/// Since `SEND_CAP` moved to that path (2026-10-04 UTC) the only callers are system tests that
/// hand over a capability they built, hence the `allow`.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn ipc_send_cap(ep: RendezvousId, data: u64, capability: crate::cap::Cap, badge: u64) {
    let sent = ipc_send_cap_from(ep, data, badge, |_, _| Ok(capability));
    debug_assert!(sent.is_ok(), "a minted capability has no source to lose");
}

/// **`SEND_CAP`'s body: send a narrowed copy of a capability the running thread holds.** The source
/// is read, checked and copied under the `IPC_TABLES` hold that delivers or parks the copy, so no
/// revocation sweep can fall between the read and the filing ([`Delegation`]). `Err` is the source's
/// answer (`NoSuchSlot`, `NotPermitted`), with nothing sent; a stale or refusing rendezvous is
/// reported as before, through `take_ipc_aborted`. Name provisional.
pub fn ipc_delegate_cap(
    ep: RendezvousId,
    data: u64,
    delegation: Delegation,
    badge: u64,
) -> Result<(), abi::Error> {
    ipc_send_cap_from(ep, data, badge, |sched, current| {
        let table = sched
            .threads
            .capabilities(current)
            .ok_or(abi::Error::NoSuchSlot)?;
        delegation.derive(&table.lock())
    })
}

/// The body [`ipc_send_cap`] and [`ipc_delegate_cap`] share. `source` runs first, under the hold,
/// and its error returns before the rendezvous is touched, which is the order the syscall layer
/// answered in when it read the source itself.
fn ipc_send_cap_from(
    ep: RendezvousId,
    data: u64,
    badge: u64,
    source: impl FnOnce(&IpcTables, ThreadId) -> Result<crate::cap::Cap, abi::Error>,
) -> Result<(), abi::Error> {
    let block = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();
        let capability = source(sched, current)?;

        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            set_ipc_aborted(sched, current);
            return Ok(()); // stale rendezvous: aborted, syscall layer errors
        };
        // As in ipc_send.
        match rendezvous.send(take_token(sched, current)) {
            inter_process_communication::Send::Rendezvous(receiver, me) => {
                hold_token(me);
                let receiver = hold_token(receiver);
                let (r, caps) = sched.threads.get_mut_with_capabilities(receiver).unwrap();
                if r.receiving_cap {
                    let slot = deliver_capability(caps, capability);
                    // Word 3 carries the sender's badge (milestone 599 (a frame per filesystem client channel)): the same store that used to
                    // write a zero here, so RECEIVE_CAP surfaces it at no extra instruction on this path.
                    r.mailbox = [data, slot, 0, badge, 0];
                    // A capability was installed, so RECEIVE_CAP's x1 is a real slot (milestone 634 (a plain SEND
                    // received by RECEIVE_CAP never hands the receiver a sender-chosen slot)).
                    r.cap_delivered = true;
                } else {
                    // **A plain RECEIVE takes no capability** (§246 (a plain `RECEIVE` never takes
                    // a capability), PROVISIONAL number; calef's ruling A, 2026-10-04 UTC). The data
                    // word arrives and the copy is dropped, the same five words `ipc_receive`
                    // returns when it collects this sender parked (milestone 633). Until the ruling
                    // this arm installed into any parked receiver's table, so a server draining a
                    // child's output lost a slot per delegation, on this arrival order only. The
                    // sender keeps its own copy: `SEND_CAP` narrows a copy, never the source.
                    r.mailbox = [data, 0, 0, badge, 0];
                }
                r.handshake.serve(); // delivered: this wake passes the boot-8 gate
                trace::record(trace::Event::Served, receiver, 3);
                wake(sched, receiver);
                false
            }
            inter_process_communication::Send::Blocked => {
                // `send` queued `current`; we park the data word and the capability to hand over.
                // Word 3 is the badge, read back by the eventual RECEIVE_CAP (milestone 599).
                let me = sched.threads.get_mut(current).unwrap();
                me.mailbox = [data, 0, 0, badge, 0];
                me.outgoing_cap = Some(capability);
                me.handshake.park(Wait::Rendezvous(ep, WaitRole::Sender)); // only a collecting receiver may wake us
                trace::record(trace::Event::BlockSelf, current, ep as u8);
                true
            }
            // As in `ipc_send`. The capability stays with the sender: it was never moved.
            inter_process_communication::Send::Refused(me) => {
                hold_token(me);
                set_ipc_refused(sched, current);
                false
            }
        }
    };

    if block {
        schedule();
    }
    Ok(())
}

/// **File a capability an IPC is delivering in the receiving thread's table**, or [`NO_CAP`] if the
/// table is full (the receiver still gets the data words). Caller holds `IPC_TABLES`; this takes the
/// table's own lock beneath it (rank 60 then 57, `sync::rank::CAPABILITY_TABLE`).
///
/// **Out of line, one copy for the three delivering paths** (`ipc_send_cap`, `ipc_call`'s hand-off
/// to a waiting server, and `ipc_receive_cap`'s collect). Since 2026-10-04 UTC the insert takes a
/// second lock, and inlined at each site that lock's mask, rank check and release grew both
/// `script/fastpath-footprint` closures past their band on aarch64 and riscv64. Name provisional.
#[inline(never)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.deliver_capability")
)]
fn deliver_capability(table: &CapabilityTableLock, capability: crate::cap::Cap) -> u64 {
    table.lock().insert(capability).unwrap_or(NO_CAP)
}

/// **Receive a data word and, if one was sent, a capability.** The mirror of [`ipc_send_cap`], and
/// the receiver's half of delegation. Returns `[data, received_slot, w1, badge]`, where
/// `received_slot` is where an incoming capability landed in *our* capability table, or [`NO_CAP`]
/// if the message carried none, and `badge` is the badge on the endpoint capability the sender
/// invoked (milestone 599, provisional; 0 when the sender's capability was unbadged).
///
/// A capability-carrying send and this share the ordinary sender/receiver queues, so either side
/// may arrive first, exactly as with the plain path.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.ipc_receive_cap")
)]
pub fn ipc_receive_cap(ep: RendezvousId) -> [u64; 5] {
    let immediate = {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();

        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            set_ipc_aborted(sched, current);
            return [0, 0, 0, 0, 0]; // stale rendezvous: aborted, syscall layer errors
        };
        // One lookup for the binding test and the token, as in `ipc_receive`.
        let me = sched.threads.get_mut(current).expect("running thread");
        let bound = me.bound_notification.is_some();
        let Some(token) = me.own_token.take() else {
            missing_token()
        };
        // The binding's receive-side half, exactly as in `ipc_receive`: a server in `RECEIVE_CAP` is the
        // commonest bound thread §101's table names (the FS server, the compositor).
        //
        if bound && let Some(word) = take_bound_signal(sched, current) {
            hold_token(token);
            Some(bound_delivery(word))
        } else {
            match rendezvous.receive(token) {
                // An interrupt signal is not a delegation; it carries no capability and no badge.
                inter_process_communication::Receive::Signal(me) => {
                    hold_token(me);
                    Some([1, NO_CAP, 0, 0, 0])
                }
                inter_process_communication::Receive::FromSender(sender, me) => {
                    hold_token(me);
                    let sender = hold_token(sender);
                    let msg = sched.threads.get(sender).unwrap().mailbox;
                    let capability = sched.threads.get_mut(sender).unwrap().outgoing_cap.take();
                    // A caller's outgoing cap is the one-shot Reply the kernel minted for its CALL (§12); a
                    // SEND_CAP sender's is the capability it chose to delegate. The difference is liveness:
                    // a caller stays blocked awaiting its reply, so it must NOT be woken here; a SEND_CAP
                    // sender's rendezvous is complete the moment we take the cap.
                    let is_reply = matches!(capability, Some(c) if matches!(c.object, crate::cap::Object::Reply(_)));
                    let slot = match capability {
                        Some(c) => {
                            deliver_capability(sched.threads.capabilities(current).unwrap(), c)
                        }
                        None => NO_CAP,
                    };
                    if !is_reply {
                        // Collected: the sender's rendezvous is complete (the boot-8 gate).
                        sched.threads.get_mut(sender).unwrap().handshake.serve();
                        trace::record(trace::Event::Served, sender, 4);
                        wake(sched, sender);
                    }
                    // x0 = word0, x1 = the delivered slot, x2 = word1 (a CALL's second word; 0 for a plain
                    // SEND_CAP, whose sender parked mailbox[1] = 0), x3 = the sender's badge (msg[3],
                    // milestone 599), x4 = REPLY_DELIVERED iff x1 is this caller's Reply (milestone
                    // 706), else 0. Never BOUND: that is the arm above.
                    let tag = if is_reply { reply_tag(slot) } else { 0 };
                    Some([msg[0], slot, msg[1], msg[3], tag])
                }
                inter_process_communication::Receive::Blocked => {
                    let me = sched.threads.get_mut(current).unwrap();
                    // Clear before parking: whoever wakes us sets this iff it installs a
                    // capability, so a plain SEND (which installs none) leaves it false (milestone 634).
                    me.cap_delivered = false;
                    // And say which receive this is, so a sender that meets us may install one
                    // (§246, PROVISIONAL number). Cleared when this receive resumes, below, so a
                    // later plain RECEIVE parks with it false.
                    me.receiving_cap = true;
                    me.handshake.park(Wait::Rendezvous(ep, WaitRole::Receiver)); // only a delivering sender may wake us
                    trace::record(trace::Event::BlockSelf, current, ep as u8);
                    None
                }
            }
        }
    };

    match immediate {
        Some(msg) => msg,
        None => {
            schedule(); // a capability-carrying sender fills our mailbox and wakes us
            let mut guard = IPC_TABLES.lock();
            let sched = guard.as_mut().expect("no scheduler");
            let t = sched.threads.get_mut(current_thread_id()).unwrap();
            debug_assert!(
                t.handshake.is_delivered(),
                "receive_cap resumed with nothing delivered"
            );
            // No longer parked in RECEIVE_CAP (§246, PROVISIONAL number). Cleared here rather than
            // at a plain RECEIVE's park, which keeps the store off `ipc_send_receive`'s closure:
            // with it there, x86_64's closure went over `script/fastpath-footprint`'s 5% band.
            t.receiving_cap = false;
            // The whole mailbox: RECEIVE_CAP's three words, the sender's badge at m[3] (milestone 599),
            // and `w4`, which is `abi::notification::BOUND` when the bound notification ended this
            // receive, `abi::rendezvous::REPLY_DELIVERED` when a CALL's Reply was installed
            // (milestone 706, written by `ipc_call_badged`), and `0` for every other delivery a
            // sender or the kernel's death path makes (milestone 151).
            // **x1 is NO_CAP unless a capability was installed for this delivery** (milestone 634,
            // fatal risk 7). A plain SEND that reached us parked drops its three words straight into
            // the mailbox, so m[1] is the sender's chosen word; returning it would hand a sender a
            // slot number where a CALL server reads a reply slot. A bound-notification delivery is
            // exempt: its x4 == BOUND and its m[1] is the notification word, not a sender's choice.
            let m = t.mailbox;
            if t.cap_delivered || m[4] == abi::notification::BOUND {
                m
            } else {
                [m[0], NO_CAP, m[1], m[3], 0]
            }
        }
    }
}

/// **Call: send two words and block until replied** (milestone 12). The atomic send-and-wait a
/// one-shot reply capability makes safe. At the rendezvous the kernel mints a `Reply` capability
/// naming *this* caller and hands it to the server (through [`ipc_receive_cap`]); we then block,
/// discoverable **only** through that capability, until the server invokes it. Returns the reply
/// words. See DECISIONS §12 and notes/ipc-naming.md.
///
/// If the server's capability table is full the reply cap is dropped (the server sees `NO_CAP`, exactly as a
/// delegated cap would be) and, having no way to answer, the caller blocks until torn down: the same
/// no-timeout limitation as a reply that never comes, and self-inflicted by the server.
///
/// # BUGS
///
/// **A server that is alive and simply never replies blocks its caller forever.** There is no
/// deadline on a `CALL` (that is milestone 106's fork, and L4's answer), and no way to end the
/// server's thread (milestone 133's). What milestone 254 fixed is narrower and is the case the
/// kernel used to get wrong in a way nothing recorded as intended: a server that **stops being able
/// to answer**, by exiting, faulting, being killed, being reaped with its region, or having the
/// rendezvous torn down under it, now frees its callers with [`abi::Error::Gone`] rather than
/// leaving them blocked for the life of the machine. See [`strand_reply_caller`].
///
/// **The reply capability names a thread, not a call**, so what stops a stale one answering a later,
/// unrelated `CALL` is that [`strand_reply_caller`] deletes it, not that [`ipc_reply`]'s guard could
/// tell the two conversations apart. A future path that reached `ipc_reply` without presenting a
/// capability would reopen that. The structural fix is a call identity in the payload:
/// `design/roadmap/0371-a-reply-capability-that-names-a-call.md`.
// Inlined so the unbadged fastpath (`ipc_call_reply`, the shape real services run and the one
// `script/icount` measures) gains no call frame: this forwards to `ipc_call_badged` with badge 0,
// which writes the same mailbox word 3 it always did.
#[inline(always)]
pub fn ipc_call(ep: RendezvousId, msg: [u64; 2]) -> [u64; 3] {
    ipc_call_badged(ep, msg, 0)
}

/// [`ipc_call`] carrying the invoked endpoint capability's badge (milestone 599, provisional). The
/// badge reaches the server's [`ipc_receive_cap`] in the delivered mailbox's word 3; a plain
/// [`ipc_call`] passes 0, the unbadged value. Split out rather than given a parameter on the hot
/// name so the tree's many `ipc_call(ep, msg)` sites (benches, tests, `ipc_stack_depth`) are
/// unchanged and the fastpath's shape is untouched, which `script/icount`'s tripwire is what
/// confirms.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.ipc_call_badged")
)]
pub fn ipc_call_badged(ep: RendezvousId, msg: [u64; 2], badge: u64) -> [u64; 3] {
    // E3's footprint padding, on the CALL side as well as the SEND side (milestone 134, extended
    // 2026-09-04). It was on `ipc_send` alone, and that was the whole of the fastpath when the
    // padding was written; milestone 188 phase 1 then split the footprint gate into two closures
    // and found the CALL/reply one is the larger and **is the shape real services run**, at which
    // point a pad reachable only from `ipc_send` was padding a shape nothing in this tree uses.
    // Measured on riscv64 before this line existed: `ipc_send_receive` 2.10x, `ipc_call_reply` 1.00x.
    // One call site per shape, so the untaken-branch confound the module doc names stays one
    // branch per round trip rather than three.
    #[cfg(feature = "fastpath_pad")]
    crate::fastpath_pad::maybe_pad();
    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();
        let reply = crate::cap::reply_cap(current);

        // `send` decides the rendezvous exactly as a plain SEND: a waiting server, or block. The
        // difference is the caller *always* blocks awaiting the reply, whether or not it met a server.
        let Some(rendezvous) = rendezvous_of(sched, ep) else {
            set_ipc_aborted(sched, current);
            return [0, 0, 0]; // stale rendezvous: aborted, syscall layer errors
        };
        // As in ipc_send; a caller queued here is Blocked until its Reply arrives. A caller that
        // meets a server is blocked too, but on nothing, so its token goes home to its TCB, where
        // the reply's wake finds it.
        match rendezvous.send(take_token(sched, current)) {
            inter_process_communication::Send::Rendezvous(receiver, me) => {
                hold_token(me);
                let receiver = hold_token(receiver);
                // A server is parked in RECEIVE_CAP: hand it the reply cap and the two words now.
                let (r, caps) = sched.threads.get_mut_with_capabilities(receiver).unwrap();
                if !r.receiving_cap {
                    // A plain RECEIVE cannot hold the Reply, so this CALL is answered `Gone`
                    // (§246, PROVISIONAL number). Out of line: no server in the tree does this.
                    call_meets_plain_receive(sched, receiver, current, msg, badge);
                    return [0, 0, 0];
                }
                let slot = deliver_capability(caps, reply);
                // Word 3 is the caller's badge (milestone 599): the same store as before with a
                // value instead of a zero, so the server's RECEIVE_CAP surfaces which client called.
                // Word 4 says x1 is a Reply (milestone 706), which only this path and the collect
                // in `ipc_receive_cap` may say; a SEND_CAP's delivery leaves it 0.
                r.mailbox = [msg[0], slot, msg[1], badge, reply_tag(slot)];
                // A Reply capability was installed, so RECEIVE_CAP's x1 is a real slot (milestone 634).
                r.cap_delivered = true;
                r.handshake.serve(); // delivered: this wake passes the boot-8 gate
                trace::record(trace::Event::Served, receiver, 5);
                wake(sched, receiver);
            }
            inter_process_communication::Send::Blocked => {
                // No server yet; `send` queued us as a sender. Park the words and ride the reply cap
                // in `outgoing_cap` so the eventual RECEIVE_CAP hands it over and, seeing a Reply, leaves
                // us blocked (see ipc_receive_cap).
                // Park the words with the badge at word 3 (milestone 599), where the eventual
                // RECEIVE_CAP reads it back out of this caller's mailbox.
                let me = sched.threads.get_mut(current).unwrap();
                me.mailbox = [msg[0], msg[1], 0, badge, 0];
                me.outgoing_cap = Some(reply);
            }
            // The rendezvous carries an interrupt (§101 ruling B). Return before parking: a caller
            // whose request was refused has no reply coming, and would otherwise wait for ever.
            inter_process_communication::Send::Refused(me) => {
                hold_token(me);
                set_ipc_refused(sched, current);
                return [0, 0, 0];
            }
        }
        // Either way we block until the reply arrives. We are NOT queued as a receiver; the Reply
        // capability, which carries our tid, is the only thing that can wake us.
        let me = sched.threads.get_mut(current).unwrap();
        me.handshake.park(Wait::Rendezvous(ep, WaitRole::Reply)); // only the reply (or an abort) may wake us
        trace::record(trace::Event::BlockSelf, current, ep as u8);
    }

    schedule(); // returns once ipc_reply has filled our mailbox and woken us

    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref().expect("no scheduler");
    let t = sched.threads.get(current_thread_id()).unwrap();
    debug_assert!(
        t.handshake.is_delivered(),
        "call resumed with nothing delivered"
    );
    let m = t.mailbox;
    [m[0], m[1], m[2]] // a reply is two words plus the pad; the fault path owns the top two
}

/// **Reply: deliver two words to a blocked caller and wake it** (milestone 12). The other half of
/// [`ipc_call`], reached by invoking the one-shot Reply capability, which carries the caller's `tid`.
/// The caller is blocked awaiting exactly this. If it is already gone (it cannot be, while blocked,
/// but be defensive), the reply is simply dropped.
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(target_os = "none", unsafe(link_section = ".text.hot.sched.ipc_reply"))]
pub fn ipc_reply(caller: ThreadId, msg: [u64; 2]) {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    if let Some(t) = sched.threads.get_mut(caller) {
        // **Only a caller that awaits a reply is touched** (boot 8's observe-and-strand guard).
        // A Reply names a tid, not a wait state, and this is the one wake site addressed by tid
        // rather than through an rendezvous's wait queue. Delivered to a thread parked as an
        // ordinary receiver (a stale reply whose CALL was aborted long ago, its caller re-parked
        // elsewhere), it would clobber that thread's mailbox and wake it messageless while its
        // TCB is still linked on the rendezvous's wait queue, a double-enqueue on the one intrusive
        // link. Anything not Reply-parked gets nothing, exactly as a reply to a dead caller.
        if !matches!(
            t.handshake.wait_on,
            Some(Wait::Rendezvous(_, WaitRole::Reply))
        ) {
            return;
        }
        // Word 2 is `NO_CAP`: a plain `REPLY` carries no capability, and `CALL` returns word 2 in
        // `x2` so a caller of a `REPLY_CAPABILITY` server can tell "none" from slot 0 (§255
        // (each socket is its own capability)). The store was a zero; it costs the round trip nothing.
        t.mailbox = [msg[0], msg[1], NO_CAP, 0, 0];
        t.handshake.serve(); // delivered: this wake passes the boot-8 gate
        trace::record(trace::Event::Served, caller, 6);
        wake(sched, caller);
    }
}

/// **Reply, carrying one capability** (`abi::reply::REPLY_CAPABILITY`, §255 (each socket is its own
/// capability); name provisional). [`ipc_reply`] plus a copy of the capability in the running
/// thread's `slot`, filed in the caller's table, the slot it landed in delivered as word 2.
///
/// The source is read, checked (`GRANT`, as `SEND_CAP` requires) and filed under one hold of
/// `IPC_TABLES`, the [`Delegation`] rule, so no revocation sweep falls between the read and the
/// filing. The copy keeps the source's rights: a server narrows before it answers (the network stack
/// mints from a `WRITE | GRANT` copy of its own endpoint), because the method has no word left to
/// carry a rights mask.
///
/// `Err` is the source's answer (`NoSuchSlot`, `NotPermitted`), with nothing delivered and the caller
/// still waiting, so the server can still answer it with a plain `REPLY`. `Ok(true)` means the copy
/// is in the caller's table. `Ok(false)` means it is not: a caller no longer waiting gets nothing,
/// exactly as in [`ipc_reply`], and a caller whose table is full gets the two words and `NO_CAP`,
/// the copy dropped. The server learns which, so it can undo what the capability was for.
///
/// **Out of line and off the round trip**: plain `REPLY` does not reach this, so `ipc_call_reply`'s
/// footprint and `ipc_rtt` are unchanged by its existence.
#[inline(never)]
pub fn ipc_reply_capability(
    caller: ThreadId,
    msg: [u64; 2],
    slot: u64,
) -> Result<bool, abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let current = current_thread_id();
    let capability = {
        let ours = sched
            .threads
            .capabilities(current)
            .ok_or(abi::Error::NoSuchSlot)?;
        let table = ours.lock();
        let src = table.get(slot).map_err(|_| abi::Error::NoSuchSlot)?;
        Delegation {
            slot,
            rights: src.rights,
        }
        .derive(&table)?
    };
    let delivered;
    {
        let Some((t, theirs)) = sched.threads.get_mut_with_capabilities(caller) else {
            return Ok(false);
        };
        // `ipc_reply`'s guard, for its reason: only a thread parked awaiting a reply is touched.
        if !matches!(
            t.handshake.wait_on,
            Some(Wait::Rendezvous(_, WaitRole::Reply))
        ) {
            return Ok(false);
        }
        delivered = deliver_capability(theirs, capability);
        t.mailbox = [msg[0], msg[1], delivered, 0, 0];
        t.handshake.serve();
    }
    trace::record(trace::Event::Served, caller, 6);
    wake(sched, caller);
    Ok(delivered != NO_CAP)
}

/// **Delete every unconsumed `Reply` capability naming `caller`, wherever in the machine it sits.**
/// Caller holds `IPC_TABLES`.
///
/// The property both of this kernel's reply-park teardowns maintain, lifted into one function
/// because they arrived a day apart with a copy each and the invariant is the same one:
/// **no unconsumed reply capability names a thread that has left its park.** Milestone 254's
/// [`strand_reply_caller`] runs it *before waking* a stranded caller, which is seL4's
/// `cteDeleteOne(callerCap)` from `cancelIPC`; milestone 133's [`finish_blocked_resident`] runs it
/// on a caller it is about to end and never wake, where the wake half does not apply.
///
/// **`outgoing_cap` goes too**, and it is the half a second copy would forget: a caller that met no
/// server rides its own reply capability there awaiting a `RECEIVE_CAP` that will now never collect
/// it, and a live `Reply` in a hand-off slot is the same forgery one step earlier.
///
/// Cost is O(threads x slots), on a teardown path in both callers.
fn delete_reply_caps_naming(sched: &mut IpcTables, caller: ThreadId) {
    let target = crate::cap::Object::Reply(caller);
    for (t, table) in sched.threads.iter_mut_with_capabilities() {
        table
            .lock()
            .delete_matching(|object: &crate::cap::Object| *object == target);
        if matches!(t.outgoing_cap, Some(c) if c.object == target) {
            t.outgoing_cap = None;
        }
    }
}

/// **Free one caller a server can no longer answer** (milestone 254). `abi::Error::Gone` reaches a
/// rendezvous's *wait queues*, and a `CALL` caller whose request was already taken left those queues
/// at the rendezvous: it is linked on nothing, and [`ipc_reply`] is the only thing that wakes it. So
/// a caller stranded by a server that merely died stayed blocked for the life of the machine, and
/// was itself a region nobody could reclaim. QNX Neutrino has not permitted that since the 1990s
/// ("if the server thread fails, exits, or disappears, the client thread becomes READY, with
/// `MsgSend()` indicating an error"), and nothing in this tree recorded it as intended, which is what
/// made it a defect rather than a fork. See notes/blocked-thread-teardown.md, proposal C.
///
/// **The capability sweep is the larger half, and it must come before the wake.** Waking a
/// reply-parked caller is exactly the path that opens the stale reply capability: [`crate::cap::
/// reply_cap`] mints `Object::Reply(tid)` carrying a generational *thread* name and no call
/// identity, and [`ipc_reply`]'s guard checks the `WaitRole` while discarding the rendezvous. That
/// guard is sound only while nothing can leave a reply park and enter a second `CALL` with an
/// unconsumed `Reply` still naming it, and this function is precisely what creates that path. A hung
/// server holding the stale capability would otherwise forge an answer to a later, unrelated
/// conversation; `L4Re` documents the identical hazard as a consequence of its own finite receive
/// timeouts. So every `Object::Reply(caller)` in the machine is deleted first, which is seL4's
/// `cteDeleteOne(callerCap)` reached from `cancelIPC`. seL4's other answer (never wake the victim,
/// `ThreadState_Inactive`) is not available here, because waking it is the whole point.
///
/// **Only a thread actually parked awaiting a reply is touched**, which is [`ipc_reply`]'s own
/// guard and is what keeps this from clobbering an ordinary receiver's park. Returns whether it
/// did anything, so a scan can use the abort flag as its own termination.
/// **`#[cold]`, and that is a claim rather than a hint.** Every caller of this is a teardown: a
/// thread departing, a kill landing, a region being reaped, a rendezvous going away. Saying so keeps
/// it out of `script/fastpath-footprint`'s closure, which matters because one of the four call sites
/// is the top of [`schedule`], the hottest path in the kernel; inlined there it cost 1,363 bytes of
/// `x86_64` IPC fastpath, a 20% growth, for code that runs when something is being torn down.
#[cold]
#[inline(never)]
fn strand_reply_caller(sched: &mut IpcTables, caller: ThreadId) -> bool {
    let parked = sched.threads.get(caller).is_some_and(|t| {
        matches!(
            t.handshake.wait_on,
            Some(Wait::Rendezvous(_, WaitRole::Reply))
        )
    });
    if !parked {
        return false;
    }
    delete_reply_caps_naming(sched, caller); // the sweep, before the wake
    set_ipc_aborted(sched, caller);
    wake(sched, caller);
    true
}

/// **Every caller `tid` was still holding a reply capability for is freed** (milestone 254): the
/// server-side trigger, for a thread that can no longer answer anybody. Reached from three places
/// where that becomes true and the capabilities are about to stop existing: [`depart`] (the thread
/// exited or faulted, which is QNX's headline case), the forcible-teardown conversion at the top of
/// [`schedule`] (DECISIONS §16's armed kill, which never reaches `depart`), and
/// [`reap_region_objects`]'s removal phase (an `Embryo` or an already-reaped corpse).
///
/// **It reads the table once and then acts**, which is a measured shape rather than a stylistic
/// one. The obvious loop re-resolves `tid` through the generational thread table on every slot,
/// because [`strand_reply_caller`] takes `sched` mutably and deletes out of this very table as it
/// goes; at 24 slots that was 24 generational lookups per departing thread, and `script/bench`
/// priced it at about 830 icount ticks on every `spawn_reap` iteration. One lookup, an array of
/// [`crate::cap::CAPABILITY_TABLE_SLOTS`] victims (512 bytes at 64 slots, 256 at the 32 it was) in this function's own frame (it is `#[inline(never)]`, so the array is never
/// on `reap_region_objects`'s), and the empty-table early-out cost nothing and gave it back.
#[cold]
#[inline(never)]
fn strand_callers_of(sched: &mut IpcTables, tid: ThreadId) {
    let mut victims = [0 as ThreadId; crate::cap::CAPABILITY_TABLE_SLOTS];
    let mut found = 0;
    {
        let Some(caps) = sched.threads.capabilities(tid) else {
            return;
        };
        let t = caps.lock();
        // Overwhelmingly the common case on the `depart` path is a thread holding no reply
        // capability at all; an empty table is the case worth not paying for at all.
        if t.used() == 0 {
            return;
        }
        for slot in 0..t.len() as u64 {
            if let Ok(c) = t.get(slot)
                && let crate::cap::Object::Reply(caller) = c.object
            {
                victims[found] = caller;
                found += 1;
            }
        }
    }
    for &caller in &victims[..found] {
        strand_reply_caller(sched, caller);
    }
}

/// **Every caller reply-parked on `ep` is freed** (milestone 254): the rendezvous-side trigger, for
/// an rendezvous that is being torn down. This is the half `drain_waiters` structurally cannot do,
/// and the reason is the whole defect: a reply park is linked on no queue, so the drain walks past
/// it. `wait_on` records `(ep, WaitRole::Reply)`, so a scan over the thread table answers it, which
/// is Zircon's answer (a closed channel fails the call in flight) reached without touching the
/// server.
///
/// **Rescan rather than list**, which is the opposite choice from [`strand_callers_of`] above and
/// the difference is the bound: that one lists because a capability table is 64 slots, 512 bytes,
/// and this one cannot because the bound here is `MAX_THREADS`, a kilobyte that grows every time
/// the thread ceiling does. Both functions sit on the call chain through
/// [`reap_region_objects`], the deepest frame in the kernel, whose own comment spends a paragraph
/// on this exact array (see also notes/stack-high-water.md). The abort flag
/// [`strand_reply_caller`] sets is what makes the rescan terminate, and it has to be the flag rather
/// than `wait_on`: a wake deferred behind `on_cpu` leaves the park in place until that core's
/// `finish_switch`, so a predicate reading only `wait_on` would spin.
#[cold]
#[inline(never)]
fn strand_callers_awaiting(sched: &mut IpcTables, ep: RendezvousId) {
    loop {
        let stranded = sched
            .threads
            .iter_mut()
            .find(|t| {
                matches!(t.handshake.wait_on, Some(Wait::Rendezvous(on, WaitRole::Reply)) if on == ep)
                    && !t.handshake.ipc_aborted
            })
            .map(|t| t.id);
        let Some(caller) = stranded else { break };
        if !strand_reply_caller(sched, caller) {
            break;
        }
    }
}

/// Delete every `PageFrame` capability naming the run `(phys, count)` from every thread's capability
/// table (§13, widened by §102). Part of revocation: once a frame (or a run of them) is being
/// revoked, no holder may keep a capability that could re-map it. The caller's own cap is deleted
/// too, which is intended: a revoke destroys all access to the page(s).
///
/// **Object equality, not overlap.** A capability matches only if it names exactly this `(phys,
/// count)` run: a narrowed derivative (rights alone differ) still matches, because `derive` never
/// changes the object, but a capability naming a different sub-range of the same physical memory
/// (see DECISIONS §102, "What this does NOT decide") does not, and is left alone. `count: 1` is the
/// pre-§102 single-page case, so every existing caller is unaffected.
///
/// **This is the right sweep for `PageFrame::REVOKE` and the wrong one for reclamation**, and the
/// difference is which question is being asked. `REVOKE` names one capability's run and takes that
/// authority back; whether it should also take an *overlapping* holder's separate capability is an
/// open question (design/decisions/0132-*.md). Reclamation asks the stronger question, "may any
/// capability still name a page this allocator is about to hand out", and only
/// [`delete_page_frame_caps_overlapping`] answers it.
pub fn delete_page_frame_caps(phys: u64, count: u64) {
    let Some(count) = core::num::NonZeroU64::new(count) else {
        return;
    };
    let target = crate::cap::Object::PageFrame(phys, count);
    delete_page_frame_caps_where(|object| *object == target);
}

/// Delete every `PageFrame` capability whose run **overlaps** `[base, base + size)`, from every
/// thread's capability table. The reclamation sweep: `memory_region::destroy` is about to return
/// these pages to an allocator that will hand them out again, so the question is not "who holds
/// exactly this object" but "may anyone still name any page of this range".
///
/// **Overlap rather than equality, because equality was a use-after-free** (found by milestone
/// 142's review). `PageFrame(base, 311)` is not equal to `PageFrame(base, 1)`, nor to any
/// `PageFrame(p, 1)` for a page inside its run, so the pre-existing per-page reclamation sweep
/// walked straight past every run capability §102 made possible: the pages came back to the
/// allocator while a holder still held a capability naming them, and one `PageFrame::MAP` later
/// re-mapped a page table or another process's stack read/write. That is exactly the hole
/// `MemoryRegion::DESTROY` exists to close, reopened by the widening.
///
/// **It also closes an older one, at no extra cost.** [`crate::revoke::revoke_region`] used to
/// delete capabilities one mapped page at a time, driven by the mapping log, so a capability whose
/// page was *never mapped* was never even looked at: nothing recorded it, so nothing found it. A
/// range sweep does not consult the log at all, so a retyped-but-unmapped frame in a destroyed
/// region loses its capability like every other.
///
/// Saturating arithmetic on the run's end: a `(phys, count)` near `u64::MAX` cannot wrap its way
/// out of the comparison, and saturating to `u64::MAX` overlaps everything, which is the safe
/// direction for a sweep whose failure mode is missing a holder.
pub fn delete_page_frame_caps_overlapping(base: u64, size: u64) {
    let end = base.saturating_add(size);
    delete_page_frame_caps_where(|object| match object {
        crate::cap::Object::PageFrame(phys, count) => {
            let run_end = phys.saturating_add(count.get().saturating_mul(page_frames::FRAME_SIZE));
            *phys < end && base < run_end
        }
        _ => false,
    });
}

/// The body both `PageFrame` sweeps share: walk every thread's table and delete every slot whose
/// object satisfies `matches`. The caller's own capability goes too, which is intended in both
/// cases: a revoke destroys all access to the page(s), including the revoker's.
///
/// **[`Thread::outgoing_cap`] goes too**, and until 2026-09-21 it did not. A capability handed to a
/// rendezvous nobody is receiving on yet is in no capability table at all: `ipc_send_cap` parks it
/// in the hand-off slot and blocks the sender, and the next `RECEIVE_CAP` files it in the receiver's
/// own table. So a sweep that reads tables alone left a live capability naming a revoked run in the
/// one place it could not see, and `MemoryRegion::DESTROY` then returned those pages to an allocator
/// while that capability was still on its way to somebody. That is the use-after-free DECISIONS
/// §13 (capability revocation and untyped reclamation) exists to prevent
/// through the slot [`delete_reply_caps_naming`] already sweeps for `Reply`, whose doc comment states
/// the rule this had not been applied to: a live capability in a hand-off slot is the same forgery
/// one step earlier. Found by risk 7's adversarial pass;
/// `kernel::user::revocation_in_flight_tests` is the falsification.
///
/// Dropping the parked capability rather than failing the send is the behaviour `ipc_send_cap`
/// already documents for the other way a hand-off can come up empty (a receiver whose table is
/// full): the data word still arrives and the receiver sees `NO_CAP`. No new error reaches
/// userspace, so this is a fix inside the established model rather than a syscall-surface change.
fn delete_page_frame_caps_where(matches: impl Fn(&crate::cap::Object) -> bool) {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    for (t, caps) in sched.threads.iter_mut_with_capabilities() {
        caps.lock().delete_matching(&matches);
        if matches!(t.outgoing_cap, Some(c) if matches(&c.object)) {
            t.outgoing_cap = None;
        }
    }
}

/// Delete every `DeviceFrame` capability naming `phys` from every capability table **except the calling
/// thread's** (milestone 23, DECISIONS §41). The caller keeps its own, which is the difference
/// between reclaiming a page and taking a device back to hand on; [`crate::revoke::
/// revoke_device_from_others`] has the reasoning.
///
/// **A `DeviceFrame` parked in a hand-off slot goes too** (2026-09-21), for
/// [`delete_page_frame_caps_where`]'s reason, which carries the whole finding: a capability in
/// flight to a receiver is in no capability table, so a sweep that reads tables alone leaves one
/// alive. Spared on the keeper, exactly as its table is.
pub fn delete_device_frame_caps_from_others(phys: u64) {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    let keeper = current_thread_id();
    let target = crate::cap::Object::DeviceFrame(phys);
    for (t, owned) in sched.threads.iter_mut_with_capabilities() {
        if t.id == keeper {
            continue;
        }
        let mut table = owned.lock();
        for slot in 0..table.len() as u64 {
            if table.get(slot).is_ok_and(|c| c.object == target) {
                let _ = table.delete(slot);
            }
        }
        drop(table);
        if matches!(t.outgoing_cap, Some(c) if c.object == target) {
            t.outgoing_cap = None;
        }
    }
}

/// **Take a port range back from every other thread** (milestone 299): delete every
/// `PortRange(base, count)` capability from every table but the caller's, and forget the cached
/// grant on each affected thread so the next context switch to it installs no bitmap and it faults
/// on its next `in`/`out`. The invoker keeps its own, the same take-back asymmetry
/// [`delete_device_frame_caps_from_others`] has and for the same reason (the kernel mints a port
/// capability once, at boot). This is `PortRange::REVOKE`'s body. `x86_64` only, like the
/// `PortRange` object it deletes.
#[cfg(target_arch = "x86_64")]
pub fn delete_port_range_caps_from_others(base: u16, count: u16) {
    delete_port_range_caps_impl(base, count, Some(current_thread_id()));
}

/// **Take a port range back from everyone**, the invoker included. The whole-machine revoke the
/// kernel's own tests use to prove a holder faults after its capability is gone (mirroring
/// [`crate::revoke::revoke_page_frame`]'s test-only whole-machine sweep); no syscall reaches it,
/// because a live driver replacement wants the sparing variant above. `x86_64` only.
#[cfg(target_arch = "x86_64")]
#[cfg_attr(not(any(test, feature = "system_tests", initrd)), allow(dead_code))]
pub fn delete_port_range_caps(base: u16, count: u16) {
    delete_port_range_caps_impl(base, count, None);
}

/// The body of both: walk every thread, delete the matching capability, and clear the cached grant.
/// `keeper` is spared (the take-back's invoker) or `None` (the whole-machine sweep). `x86_64` only.
#[cfg(target_arch = "x86_64")]
fn delete_port_range_caps_impl(base: u16, count: u16, keeper: Option<ThreadId>) {
    {
        let mut guard = IPC_TABLES.lock();
        let Some(sched) = guard.as_mut() else {
            return;
        };
        let target = crate::cap::Object::PortRange(base, count);
        for (t, caps) in sched.threads.iter_mut_with_capabilities() {
            if Some(t.id) == keeper {
                continue;
            }
            let mut table = caps.lock();
            for slot in 0..table.len() as u64 {
                if table.get(slot).is_ok_and(|c| c.object == target) {
                    let _ = table.delete(slot);
                }
            }
            drop(table);
            // A `PortRange` parked in a hand-off slot goes too (2026-09-21), for
            // `delete_page_frame_caps_where`'s reason: a capability in flight to a receiver is in
            // no capability table, so a sweep that reads tables alone leaves one alive.
            if matches!(t.outgoing_cap, Some(c) if c.object == target) {
                t.outgoing_cap = None;
            }
            // Forget the cached grant if it named the revoked range, so switching to this thread
            // installs nothing. x86 only; the field exists nowhere else.
            #[cfg(target_arch = "x86_64")]
            if t.port_range_grant == Some((base, count)) {
                t.port_range_grant = None;
            }
        }
        // Reach the TSS of **every** core that might already hold the revoked bitmap, before
        // releasing the lock (milestone 315). Clearing the cached grant above is not enough on a
        // multi-core machine: a holder running on another core keeps that core's bitmap until its
        // next context switch, so its `in`/`out` kept succeeding for up to a tick after this
        // returned. Milestone 313's audit accepted that window on the reasoning that it could not
        // reopen; at two cores it was red in 7 of 12 runs of the test written to see it.
        //
        // **Inside the lock, and that is the whole of why the broadcast is safe.** The far end
        // writes a core's TSS from an NMI handler, which lands at an arbitrary instruction
        // boundary; holding `IPC_TABLES` across the send is what guarantees no other core is
        // inside `install_port_grant` at that instant, and equally that none can read a grant this
        // sweep has already cleared and install it behind the broadcast's back. `schedule`'s
        // install was moved under the lock in the same change, and `segments::set_port_range_grant_on`
        // states the resulting rule at the writer.
        //
        // The NMI is forced rather than chosen: it is the only message an x86 core takes while it
        // spins for this very lock with interrupts masked (notes/x86-tlb-shootdown.md). A no-op on
        // every architecture with no TSS.
        //
        // The invoker's own core is spared when there is an invoker (`keeper.is_some()`): its
        // installed bitmap is the invoker's own grant, which this sweep deliberately left in the
        // invoker's table (2026-09-24 security audit; `segments::revoke_port_grant_everywhere`).
        #[cfg(target_arch = "x86_64")]
        crate::arch::segments::revoke_port_grant_everywhere(base, count, keeper.is_some());
    }
}

/// Remove a capability from the **current thread's** table. Used to consume a one-shot Reply
/// capability the instant it is invoked (§12), which is what makes a second reply impossible.
///
/// **On `x86_64`, deleting the `PortRange` capability behind the thread's cached port grant also
/// drops the grant** (the audit in milestone 313 (the security audit that was due since August),
/// 2026-09-17). A port range is enforced by
/// `Thread::port_range_grant` and the TSS bitmap the context switch installs from it, not by the
/// capability table, so until this was added a thread that `SYS_CAP_DELETE`d its own port capability
/// kept `in`/`out` access to those ports for the rest of its life: the table said the authority was
/// gone and the switch kept installing it. That is §12's "a consumed capability cannot be used again"
/// failing for the one object whose enforcement lives outside the table, and it was live rather than
/// theoretical: `system_initializer` deletes its console device capability (`cap_delete(g.uart_dev)`)
/// on every boot, which on x86 is exactly this object. The cache is cleared here and this core's TSS
/// is reset at once, so the ports fault on the very next access rather than after a switch.
///
/// **The delete itself takes only the thread's own table lock** (2026-10-04 UTC), as
/// [`current_cap`] does, so the one-shot Reply that every `REPLY` consumes no longer costs a second
/// acquisition of `IPC_TABLES`. Only a deleted `PortRange` then takes `IPC_TABLES`, to clear the
/// cached grant and this core's bitmap under it (milestone 315's reason, below). Between the two
/// steps no user code runs: the thread whose grant it is, is the one executing this syscall, and a
/// core's bitmap only ever permits a range for the thread running on it. A concurrent
/// `PortRange::REVOKE` that clears the grant first leaves this step nothing to do.
///
/// # BUGS
///
/// **Deleting one of two copies of the same `PortRange` disturbs the other.** The grant is one
/// `Option` on the thread rather than a record per capability, so a thread holding the range in two
/// slots loses `in`/`out` the moment it deletes either, while the surviving slot still names the
/// range. §12's "dropping one capability does not disturb the others" (`notes/confinement-claims.md`
/// row 5) is therefore false for this one object on this one architecture. It fails safe (authority
/// is removed, never granted) and no real consumer holds a range twice, so it is recorded rather
/// than fixed; a count or a table scan on delete would close it. Found by milestone 633 (an outside
/// agent attacks the confinement claim)'s second pass.
pub fn delete_current_cap(slot: u64) -> Result<(), crate::cap::Error> {
    // Read before the delete, under the same hold: a deleted slot names nothing.
    let deleted = {
        let mut t = current_capabilities().ok_or(crate::cap::Error::NoSuchSlot)?;
        let capability = t.get(slot)?;
        t.delete(slot)?;
        capability
    };
    #[cfg(target_arch = "x86_64")]
    if let crate::cap::Object::PortRange(base, count) = deleted.object {
        let mut guard = IPC_TABLES.lock();
        let current = current_thread_id();
        let held = guard
            .as_mut()
            .and_then(|sched| sched.threads.get_mut(current))
            .filter(|t| t.port_range_grant == Some((base, count)));
        if let Some(t) = held {
            t.port_range_grant = None;
            // Under the lock, with `delete_port_range_caps_impl`'s broadcast and for its reason
            // (milestone 315): every writer of a TSS port bitmap holds `IPC_TABLES`, so a
            // revocation NMI cannot land inside one. No broadcast is owed here. The caller is the
            // thread whose grant is installed, a core's bitmap only ever permits a range for the
            // thread currently running on it, and that thread is running here, so this core is
            // the only core to tell.
            crate::arch::segments::revoke_installed_port_grant(base, count);
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = deleted; // no capability is enforced outside the table on this architecture
    Ok(())
}

/// Look up a capability in the **current thread's** table.
///
/// The lookup that is the security mechanism. `slot` came from userspace, in a register, and it
/// indexes an array that lives in kernel memory and that userspace has never seen. An empty slot
/// is `NoSuchSlot`, which is not "permission denied": **there is nothing there.**
///
/// **`#[inline(never)]`, because `script/fastpath-footprint` counts it once as a root and
/// `syscall::dispatch` flat** (milestone 758 (the IPC fast paths shrink back inside their band),
/// provisional). Left to LLVM, outlining the lock-order panic made this body small enough to inline
/// into one of `invoke`'s call sites, which put 804 bytes on aarch64's `syscall_entry` for a lookup
/// the closure already counts. Every capability syscall calls it, so one copy is also the cheaper
/// copy to keep warm. Milestone 368 (`script/fastpath-footprint`'s entry set is flat) holds the
/// other instances of this shape.
#[inline(never)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[cfg_attr(
    target_os = "none",
    unsafe(link_section = ".text.hot.sched.current_cap")
)]
pub fn current_cap(slot: u64) -> Result<crate::cap::Cap, crate::cap::Error> {
    // **No `IPC_TABLES` here** (2026-10-04 UTC): the running thread's own table, under its own lock.
    // This was the global lock on every capability syscall, and on radon at four busy cores 41% of
    // these lookups found it held (notes/job-mix/null-syscall-under-load.md).
    //
    // The lock-wait instrument marks this call and its one acquisition, as it did when the lock was
    // `IPC_TABLES`, so `site=current_cap` means the same thing before and after. The closure takes
    // no other lock, so marking the whole call marks exactly that acquisition. See
    // `crate::lock_wait`; absent from every build without `lock_wait`.
    #[cfg(feature = "lock_wait")]
    crate::lock_wait::enter_current_cap();
    let table = current_capabilities();
    #[cfg(feature = "lock_wait")]
    crate::lock_wait::leave_current_cap();
    match table {
        Some(table) => table.get(slot),
        None => Err(crate::cap::Error::NoSuchSlot),
    }
}

/// Hand the current thread a capability. **The only way authority ever enters a process.**
///
/// The running thread's own table, under its own lock and not `IPC_TABLES`, as [`current_cap`].
pub fn grant(capability: crate::cap::Cap) -> Result<u64, crate::cap::Error> {
    current_capabilities().map_or(Err(crate::cap::Error::NoFreeSlot), |mut t| {
        t.insert(capability)
    })
}

/// Hand the current thread a capability **at an explicit slot**, leaving lower slots empty.
///
/// [`grant`] fills the first free slot, which is what an ordinary `Spawn` literal wants: slot 0 is
/// `grants[0]` and reading the literal tells you the whole authority. But some out-of-band
/// conventions (notes/abi.md §4) name a *fixed* slot that a program may hold without holding the
/// ones below it: a std program granted a directory capability but not the network holds slot 4
/// with 2 and 3 empty, and the emptiness is load-bearing (it is how `std::net` knows it has no
/// network). This is the same explicit-target move `ThreadControlBlock::CAP_INSERT` already offers a userspace
/// loader, available to the kernel's own service wiring.
pub fn grant_at(slot: u64, capability: crate::cap::Cap) -> Result<u64, crate::cap::Error> {
    current_capabilities().map_or(Err(crate::cap::Error::NoFreeSlot), |mut t| {
        t.insert_at(slot, capability)
    })
}

/// **A copy of a capability the running thread holds, narrowed, about to be filed somewhere.** The
/// source slot and the rights the copy keeps; [`Delegation::derive`] is the rule. Name provisional.
///
/// # Why the source is a slot and not a capability
///
/// A revocation sweep (`delete_page_frame_caps_where`, `delete_device_frame_caps_from_others`,
/// `x86_64`'s `delete_port_range_caps_impl`) holds `IPC_TABLES` for its whole walk and takes each
/// table's lock beneath it. A delegation used to read its source with [`current_cap`], let go, and
/// file the copy in a second critical section, so a sweep could run entirely between the two: it
/// deleted the source and the copy was filed after it had passed. A `PageFrame` has no generation
/// to make that copy inert, so under `PageFrame::REVOKE` it was authority the revoker had taken
/// back, and under `MemoryRegion::DESTROY` it named pages the allocator was about to reuse (§13's
/// use-after-free). Milestone 761 (capability lookup off the global lock) recorded the gap; `system_tests::user::revocation_window_tests`
/// drove a sweep into it on `SEND_CAP`, `CAP_INSERT` and `SLICE` and all three filed the copy.
///
/// **The invariant now is that a copy is derived from a source read in the same critical section
/// that files the copy, and that critical section excludes every sweep.** A delegation into
/// another thread holds `IPC_TABLES` across both ([`ipc_delegate_cap`],
/// [`thread_control_block_delegate_cap`]), which every sweep holds for its whole walk. A derivation
/// into the running thread's own table ([`grant_derived`]) holds that table's lock across both,
/// which every sweep takes to delete from it. Either way the sweep is wholly before (the source is
/// gone and the delegation answers `NoSuchSlot`, as if it had started after the revoke) or wholly
/// after (the copy is in a table the sweep then walks). Passing the slot rather than a capability
/// is what makes the old shape hard to write again: these functions have no parameter a stale copy
/// could arrive through.
///
/// **Objects with generational names do not need this, and the derivations of them that still
/// read-then-grant are safe for that reason**: `Rendezvous::BADGE` mints a badged copy of an
/// endpoint, and `MemoryRegion::SPLIT` a child of a region, and a copy minted after its object died
/// names a dead generation and fails on use (`crates/slots`, §16 (object revocation)).
#[derive(Clone, Copy)]
pub struct Delegation {
    /// The source's slot in the running thread's table.
    pub slot: u64,
    /// The rights the copy keeps: a subset of the source's.
    pub rights: crate::cap::Rights,
}

impl Delegation {
    /// **The delegation rule**: the source exists, its holder was trusted to pass it on (`GRANT`),
    /// and the copy only narrows. Applied to a table the caller holds under a lock every sweep takes.
    fn derive(self, table: &crate::cap::CapabilityTable) -> Result<crate::cap::Cap, abi::Error> {
        let src = table.get(self.slot).map_err(|_| abi::Error::NoSuchSlot)?;
        if !src.rights.allows(crate::cap::Rights::GRANT) {
            return Err(abi::Error::NotPermitted); // holder may not pass this on
        }
        if !self.rights.is_subset_of(src.rights) {
            return Err(abi::Error::NotPermitted); // delegation may only narrow, never widen
        }
        Ok(crate::cap::Cap {
            object: src.object,
            rights: self.rights,
        })
    }
}

/// **File, in the running thread's own table, a capability derived from one it holds**, reading the
/// source and filing the result under one hold of that table's lock ([`Delegation`] has why).
/// `derive` decides what the copy is from the source as it stands at that instant. `Err` is
/// `derive`'s answer, `NoSuchSlot` for an empty slot, or `OutOfMemory` for a full table. Name
/// provisional.
pub fn grant_derived(
    slot: u64,
    derive: impl FnOnce(crate::cap::Cap) -> Result<crate::cap::Cap, abi::Error>,
) -> Result<u64, abi::Error> {
    let mut table = current_capabilities().ok_or(abi::Error::NoSuchSlot)?;
    let src = table.get(slot).map_err(|_| abi::Error::NoSuchSlot)?;
    let copy = derive(src)?;
    table.insert(copy).map_err(|_| abi::Error::OutOfMemory)
}

/// **Retype a TCB out of `region`** (milestone 19c.3): an embryo thread, page-resident in a
/// page of the creator's own untyped, in the thread table but in no queue and not runnable.
/// Returns its `ThreadId` (what an `Object::ThreadControlBlock` capability carries) or `None` if the region is out of
/// budget or the table is full.
pub fn create_thread_control_block(region: u64) -> Option<ThreadId> {
    let page =
        crate::memory_region::retype_object_page(region, crate::memory_region::ObjectKind::Thread)?;
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut()?;
    let name = sched.threads.insert_from_page(page, |tid| {
        let mut t = Thread::embryo();
        t.id = tid;
        // Remember which region paid for this TCB. It is the region an rendezvous reap reclaims
        // (DECISIONS §32), and here is the only point where the answer is known rather than
        // inferred: the caller named it, and nothing afterwards can tell us as reliably.
        t.thread_control_block_region = Some(region);
        t
    });
    // On a full table the page stays the region's (spend-only); nothing to recycle. The region
    // is already pinned by retype_object_page, so its destroy is refused regardless.
    name
}

/// Tear down every kernel object whose backing page lies in `[base, end)`, so `memory_region::destroy`
/// can reclaim the region (object revocation). `Err` if a **live** thread (`Ready`/`Running`/
/// `Blocked`) sits in the region, or if a dead one is still standing on its kernel stack
/// (`handshake.on_cpu`); the region stays pinned. But the refusal is no longer passive:
/// it **arms the kill** (DECISIONS §16 amendment), marking each live resident thread so the
/// scheduler tears it down at its next preemption, so an owner that retries (the shell's `^C`
/// escalation, §24) reclaims a runaway rather than being told forever to wait for it. `Embryo` and
/// `Finished` threads are removed here (dropped, and their generational names killed, so every
/// outstanding `ThreadControlBlock` capability to them goes stale on its next use).
///
/// **A `Blocked` resident is not refused-and-armed; it is ended** (milestone 133, proposal A,
/// calef 2026-09-03). The arm is spent by `schedule()` only for a thread whose state is `Running`,
/// so a resident blocked on a rendezvous nobody will ever serve was armed and never reaped and the
/// region never came back. The finish phase unlinks each such thread from whatever queue holds it,
/// deletes every outstanding `Reply` capability naming it, and writes it to `Finished` **without
/// waking it**; see [`finish_blocked_resident`] for why not waking it is the security half. The
/// authority is unchanged: it is still the untyped capability, and nothing was added to the
/// syscall surface.
///
/// **The region's endpoints go first, on every pass, refusal or not**, and that ordering is
/// load-bearing rather than tidy: it is what wakes a resident blocked in `RECEIVE` so the armed kill
/// can actually land on it. The long comment at the sweep says why, and notes/frames.md carries the
/// boot it fixed.
///
/// Takes `IPC_TABLES`, so it must run **outside** any teardown `Drop`: this is the caller-driven half of
/// revocation, and `memory_region::destroy` is the `IPC_TABLES`-free half (which is why the reaper's
/// `Drop` -> `destroy` path cannot deadlock against it). See `memory_region::unpin`.
/// What the refuse phase of [`reap_region_objects`] decides about one resident thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionReap {
    /// Free its pages: it can never run again and no core is standing on its stack.
    Reap,
    /// It can still be scheduled. Refuse, and arm DECISIONS §16's kill so the owner's retry
    /// reclaims a runaway rather than being told to wait forever.
    RefuseAndArm,
    /// It is dead but has not left its own kernel stack yet. Refuse, and arm **nothing**: there is
    /// nothing left to doom, and the condition clears on its own one context switch from now.
    RefuseStanding,
    /// **It is `Blocked`, and arming a kill would not reach it** (milestone 133). Unlink it from
    /// whatever queue holds it, sweep the outstanding `Reply` capabilities that name it, and write
    /// it straight to `Finished` without ever letting it run again. That is proposal A's whole
    /// mechanism, and the verdict exists as its own word because the fact that
    /// [`RegionReap::RefuseAndArm`] is *insufficient* for a `Blocked` thread is the defect this
    /// milestone fixes and is not visible from the arm itself.
    ///
    /// Named for what the pass does rather than for the state it found, because the reader who
    /// needs it is the one asking why a `Blocked` resident is not simply armed like the others.
    FinishInPlace,
}

/// **The refuse phase's rule, lifted out so it can be stated and tested without staging a race.**
///
/// The `on_cpu` arm is the one that had to be learned the expensive way, and it is why this is a
/// named function rather than a condition inside the loop. The rule a reader must carry away:
/// **`state` says whether a thread can run again, and `on_cpu` says whether a core is standing on
/// its stack, and freeing a `Thread` unmaps that stack.** Those are different questions; this path
/// asked only the first for months, and the answer to the second is what four CI panics were.
/// See notes/stack/kernel-stack-freed-under-its-owner.md.
///
/// `standing` is `on_cpu`, or'd since 2026-10-04 with
/// [`being_reaped`](crate::thread::Thread::being_reaped): a thread whose stack and address space the
/// reaper is freeing outside `IPC_TABLES` is refused the same passive way, for a different reason
/// with the same shape (the condition clears by itself, and reaping the thread now would let its
/// owner reuse memory the dying thread has not given back yet).
fn region_reap_verdict(state: State, standing: bool) -> RegionReap {
    if matches!(state, State::Ready | State::Running) {
        RegionReap::RefuseAndArm
    } else if state == State::Blocked && !standing {
        // Milestone 133, proposal A. `Blocked` used to sit in the arm above, and being there is
        // what made a permanently blocked resident unreclaimable for the life of the machine: the
        // arm is spent at the top of `schedule()` and only for a thread whose state is `Running`,
        // and a thread blocked on a rendezvous nobody will ever serve does not become `Running`
        // again. The arm was not too weak, it was aimed at a thread that never arrives.
        RegionReap::FinishInPlace
    } else if standing {
        // A `Blocked` thread with `on_cpu` still set reaches here, and that is deliberate. It is
        // mid-switch-out, so its saved context is stale and a core is standing on the stack that
        // freeing its `Thread` would unmap; ending it now is the four-CI-panic bug wearing a new
        // hat. Refusing without arming is exactly right, because the condition clears itself one
        // context switch from now and the owner's next retry finds it `Blocked` and off its stack.
        RegionReap::RefuseStanding
    } else {
        RegionReap::Reap
    }
}

/// **End one permanently blocked resident of a region being destroyed** (milestone 133, proposal
/// A). Unlink it from whatever queue holds it, delete every outstanding `Reply` capability that
/// names it, and write it straight to `Finished`. Caller holds `IPC_TABLES`.
///
/// **It never wakes the victim, and that is a security property rather than an economy.** The
/// research (notes/blocked-thread-teardown.md) found a live hazard that any waking design has to
/// buy its way out of: `cap::reply_cap` mints `Object::Reply(tid)` whose payload is a generational
/// thread name with **no call identity**, and `ipc_reply`'s guard checks the `WaitRole` and
/// discards the rendezvous. That is sound today only because nothing can leave a reply park and
/// enter a second `CALL` while an unconsumed `Reply` still names it. Wake a reply-parked caller,
/// hand it `Gone`, let it call a healthy server, and a hung server's stale `Reply` passes the role
/// check and forges an answer to a different conversation. `L4Re` documents the identical hazard as
/// a consequence of its own finite receive timeouts, and Zircon documents it for
/// `zx_channel_call`'s timeout. **A change that traded a permanent block for a forgeable reply
/// would be a worse defect than the one it fixes.**
///
/// Both of seL4's fixes are taken here, deliberately, and the second is why the first is cheap.
/// The victim is set to `Finished` and never runs another instruction, which is
/// `ThreadState_Inactive` and means no second `CALL` can exist to be forged against. **And the
/// reply capabilities are swept anyway**, through [`delete_reply_caps_naming`], which is
/// `cteDeleteOne(callerCap)` from `cancelIPC` and is the same function milestone 254's
/// [`strand_reply_caller`] runs before *its* wake. The two milestones took one of seL4's answers
/// each, a day apart, and share the invariant: **no unconsumed reply capability names a thread
/// that has left its park.**
///
/// **The role is not trusted to say which queue holds the thread, and must not be.** A `CALL`
/// caller that met no server is recorded as `WaitRole::Reply` and *is* on the sender queue
/// (`ipc_call`'s `Send::Blocked` arm), so the role and the queue genuinely disagree in a case that
/// happens on every unserved call. Both queues are asked; each remove compares pointers and
/// reports whether it found anything, so asking the wrong one costs one drain-and-repush of a
/// short queue and cannot be wrong. This is the sharpest failure mode the research named
/// (`thread_wake_handshake`'s own BUGS says nothing forces a block site to call `park`, so
/// `wait_on` can in principle go stale) narrowed to the one part of it a caller can defend
/// against: the *rendezvous name* is still taken on trust, and a stale one leaves a dangling
/// pointer in a queue, which is why the `debug_assert` below pairs `Blocked` with a recorded wait.
fn finish_blocked_resident(sched: &mut IpcTables, tid: ThreadId) {
    debug_assert!(
        sched
            .threads
            .get(tid)
            .is_none_or(|t| t.handshake.wait_on.is_some()),
        "a Blocked thread with no recorded wait: some block site wrote the state by hand",
    );

    // Unlink. `wait_on` names the object; the pointer identifies the thread on its queue.
    match sched.threads.get(tid).and_then(|t| t.handshake.wait_on) {
        Some(Wait::Rendezvous(ep, _role)) => {
            let ptr = thread_control_block_ptr(sched, tid);
            if let Some(rendezvous) = rendezvous_of(sched, ep) {
                // `ptr` is only compared. Whichever queue held the thread hands its token back,
                // and it goes home to the thread, to be freed with it.
                for token in [
                    rendezvous.remove_sender(ptr),
                    rendezvous.remove_receiver(ptr),
                ]
                .into_iter()
                .flatten()
                {
                    hold_token(token);
                }
            }
        }
        // A thread blocked in `Notification::WAIT` (milestone 151) is linked on that
        // notification's queue, and its page may outlive this region.
        Some(Wait::Notification(id)) => {
            let ptr = thread_control_block_ptr(sched, tid);
            if let Some(page) = notification_of(sched, id)
                && let Some(token) = page.state.remove_waiter(ptr)
            {
                hold_token(token);
            }
        }
        None => {}
    }

    // Sweep the outstanding reply capabilities, sharing milestone 254's function rather than
    // carrying a second copy of the same invariant. This victim is never woken, so the sweep is
    // belt as well as braces; it is here anyway because the thread name is generational and a
    // `Reply` outliving its thread would go stale only on its *next use*, which leaves the tree
    // depending on a slot generation nobody reading `ipc_reply` can see.
    delete_reply_caps_naming(sched, tid);

    // End it. `killed` is set as well as the state, so the flag keeps one meaning: it marks a
    // thread this teardown doomed, whoever ends up observing it. `wait_on` is cleared because the
    // thread is on no queue any more and a `Some` here is what a hang dump reads as "still
    // waiting".
    if let Some(t) = sched.threads.get_mut(tid) {
        t.killed = true;
        t.handshake.state = State::Finished;
        t.handshake.wait_on = None;
    }
}

fn reap_region_objects(base: u64, end: u64) -> Result<(), ()> {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return Err(());
    };
    // A TCB sits at the start of its page, so the page's physical address is the thread pointer
    // translated back. That is the whole test for "this object lives in the region".
    let page_of = |t: &Thread| crate::arch::mmu::virt_to_phys(t as *const Thread as u64);

    // --- Rendezvous phase: the region's endpoints go FIRST, refusal or not. ---
    //
    // **This ordering is what lets `DESTROY` reclaim a region full of blocked servers**, and until
    // 2026-08-16 it could not. The sweep used to sit after the refusal below, so a region holding a
    // process parked in `RECEIVE` was refused forever: the refusal armed §16 (object revocation)'s kill, the kill is spent
    // by `schedule()`, and a `Blocked` thread never reaches `schedule()`. The owner retried until it
    // gave up, and the memory stayed spoken for until the machine stopped. That is the whole reason
    // the aarch64 test boot ran out of frames: `userspace_init_brings_up_the_console_server` builds a
    // console server out of the progenitor's budget and that server blocks in its serve loop, so the progenitor's
    // 2048-frame region was unreclaimable by construction. See notes/frames.md.
    //
    // Sweeping first fixes it because **the wake is already here**: removing an rendezvous drains its
    // wait queues, marks each waiter's IPC aborted and wakes it, which is precisely the transition a
    // blocked resident needs to become schedulable and so to spend the kill the refusal arms one
    // paragraph below. A server whose endpoints came out of the region being destroyed dies; one
    // blocked on somebody else's rendezvous still does not, and `reclaim_region`'s caller is told so by
    // the refusal rather than by a hang (see `user::holding::Holding`'s BUGS).
    //
    // **A refused reclaim was already destructive** and says so in `reclaim_region`'s BUGS: it arms
    // kills on every live resident. This makes the same pass also end the region's endpoints, which
    // is the same commitment one object over: the caller has said this region is going away. What it
    // must not do is *surprise* anyone, which is why it is written here rather than assumed.
    //
    // **Rescan for one at a time rather than listing them all first.** The obvious shape is to walk
    // the table into a `[u64; MAX_RENDEZVOUS]` and then walk that, because `remove` mutates the table
    // and you cannot remove while iterating it. That array is **4096 bytes of a 24 KiB kernel thread
    // stack** (`thread::STACK_PAGES`), and this function is already the deepest frame in the kernel;
    // measured with `-Z emit-stack-sizes` it was 6816 bytes, of which 6144 was three such scratch
    // arrays, against a measured thread-stack high-water of 11672 bytes: this one frame wanted 2104
    // bytes MORE than all the headroom there was. See notes/stack-high-water.md.
    //
    // Rescanning costs O(live endpoints) per removal instead of O(1). That is the right trade here
    // and nowhere near a hot path: this runs when a region is torn down, the table is 512 slots, and
    // a real teardown removes a handful. Stack is the scarce resource, not these comparisons.
    loop {
        let doomed = sched
            .rendezvous_table
            .iter()
            .find(|&(_, &phys)| base <= phys && phys < end)
            .map(|(name, _)| name);
        let Some(name) = doomed else { break };

        // Drain the rendezvous's waiters, **waking each one inside the drain rather than listing
        // them into a `[u64; MAX_THREADS]` first**, which is the same rule the paragraph above
        // states and this line did not follow: that array was 1 KiB at 128 threads and grows with
        // the ceiling, on the frame this function's own comment calls the deepest in the kernel.
        //
        // The wake is safe here for the reason the collected version relied on one line later:
        // `Rendezvous::drain_waiters` pops an entry off its queue *before* calling back, so the
        // thread's one intrusive link is already free and `wake` may push it onto a run queue.
        // `rendezvous_of` returns a `'static` reference, so the rendezvous does not hold the
        // `sched` borrow the callback needs.
        if let Some(rendezvous) = rendezvous_of(sched, name) {
            rendezvous.drain_waiters(|w| {
                let tid = hold_token(w);
                set_ipc_aborted(sched, tid);
                wake(sched, tid);
            });
        }
        sched.rendezvous_table.remove(name);
        // **And the callers the drain structurally cannot reach** (milestone 254). A `CALL` whose
        // request was taken left the sender queue at the rendezvous, so `drain_waiters` above walks
        // straight past it; only `wait_on` still records that it awaits a reply through this
        // rendezvous, which no longer exists.
        strand_callers_awaiting(sched, name);
    }

    // --- Notification phase (milestone 151): the same sweep for the region's notifications. ---
    //
    // After the rendezvous phase and before the finish phase, for the rendezvous phase's reason:
    // a thread blocked in `WAIT` on a notification in this region is aborted and woken here, and so
    // becomes schedulable enough to spend its kill.
    reap_region_notifications(sched, base, end);
    // And its timers (milestone 106), which have no waiters to abort: an armed one just never fires.
    reap_region_timers(sched, base, end);

    // --- Finish phase: end every resident the arm below could never reach (milestone 133). ---
    //
    // **This is proposal A, "`DESTROY` finishes what it starts"** (calef, 2026-09-03;
    // design/roadmap/0133-blocked-thread-teardown.md, and notes/blocked-thread-teardown.md for the
    // four proposals and the survey they came from). The refuse phase below arms DECISIONS §16's
    // kill, `schedule()` spends that kill only for a thread whose state is `Running`, and a thread
    // blocked on a rendezvous nobody will ever serve never becomes `Running` again. So the arm was
    // armed and never landed, the refusal was permanent, and **the region was unreclaimable for
    // the life of the machine**. No privilege fixed that, because it was a scheduler property and
    // not an authorization one, which is why §32's "stronger right" was insufficient rather than
    // merely large.
    //
    // The rendezvous sweep above already rescues the case where the rendezvous the resident waits
    // on came out of *this* region. What it cannot rescue is a resident blocked on somebody
    // else's, which is the ordinary shape of a hung component: a server parked in `RECEIVE` on a
    // client's rendezvous, or a client parked in `CALL` on a server that will never reply.
    //
    // **The authority is unchanged, and that is the whole reason this shape was chosen.** The
    // holder of the untyped capability could already end every `Ready` and `Running` thread in the
    // region and could already leave every `Blocked` one killed-and-refused. It could not only
    // *finish*. Nothing is added to the syscall surface, no new right, and no new error reaches
    // userspace.
    //
    // **Rescan for one at a time**, the same rule the rendezvous sweep states and for the same
    // reason: a `[u64; MAX_THREADS]` worklist is a kilobyte of scratch on the deepest frame in the
    // kernel. Each pass writes one resident to `Finished`, which no longer matches, so this
    // terminates.
    loop {
        let doomed = sched
            .threads
            .iter_mut()
            .find(|t| {
                let phys = crate::arch::mmu::virt_to_phys(&raw const **t as u64);
                base <= phys
                    && phys < end
                    && region_reap_verdict(t.handshake.state, t.handshake.on_cpu || t.being_reaped)
                        == RegionReap::FinishInPlace
            })
            .map(|t| t.id);
        let Some(tid) = doomed else { break };
        finish_blocked_resident(sched, tid);
    }

    // --- Refuse phase: no thread in the region may still be able to run. ---

    // A live thread (Ready/Running/Blocked) in the region: freeing its page would pull the stack,
    // or the running address space, out from under a thread that can still be scheduled. We may not
    // reclaim under it this pass, but the forcible tier of `^C` (DECISIONS §24) needs `DESTROY` to
    // *tear a runaway down*, not merely refuse it. So arm the kill (§16 amendment): mark every live
    // resident thread `killed` and refuse. A killed thread never runs again; the scheduler converts
    // it to a corpse at its next preemption, with no queue surgery here and no core stopping another
    // (each core reaps its own on the timer). The region's owner retries `DESTROY` (the shell's
    // escalation loop already does), and once the runaway has torn down this pass finds it gone and
    // reclaims. A thread that only ever blocks, never scheduled to hit that preemption, is the
    // cooperative tier's job (send it its interrupt rendezvous), not this one.
    let mut live = false;
    // **And a second refusal, which is not about being alive at all**: a thread whose
    // `handshake.on_cpu` is still set. That flag means "a core is standing on this thread's kernel
    // stack", it is cleared by that core's successor in `finish_switch`, and freeing the `Thread`
    // is what unmaps the stack. `finish_switch` is built around exactly this and says so; this
    // path was not, because it reasoned from `state`, and `Dead` genuinely does mean "never runs
    // again". **Never runs again is not the same as off its stack**, and the gap between them is a
    // real window: `depart` marks a supervised thread `Dead`, delivers its death message (waking
    // the supervisor, possibly on another core), releases `IPC_TABLES`, and only *then* calls
    // `schedule()`. A supervisor that reaps inside those few hundred instructions unmapped the
    // stack under the corpse, whose next store then walked the exception vector down to this
    // slot's base. Four CI runs over five days, always the same test, always the same slot, and
    // read as a stack overflow for three of them. See
    // notes/stack/kernel-stack-freed-under-its-owner.md.
    //
    // No kill is armed for this one, deliberately: the thread is already dead, so there is nothing
    // to doom, and the refusal clears on its own one context switch from now. The caller retries,
    // which is `reclaim_region`'s existing contract.
    let mut standing = false;
    //
    // A live thread (Ready/Running/Blocked) in the region: freeing its page would pull the stack, or
    // the running address space, out from under a thread that can still be scheduled. A `Dead`
    // corpse (milestone 22) is *not* live: it never runs again, so it is reapable here exactly like
    // an `Embryo` or a `Finished` thread, which is precisely what "reaped with §16 revocation"
    // (DECISIONS §26) means.
    //
    // **The pin is not what makes dropping a resident's address space safe, and saying it was cost
    // us a double free.** This comment used to argue that the region stays pinned through the reap,
    // so a bound space dropping here is refused by `memory_region::destroy`. That is true for a drop that
    // happens *inside* this function, under `IPC_TABLES`. It is false for the one that matters: the
    // reaper (`finish_switch`) hoists a dead thread's space out, releases `IPC_TABLES`, and drops it
    // afterwards, by which time `reclaim_region` may already have unpinned. What makes it safe is
    // that a space built from a region it does not own never frees that region at all
    // (`user::Backing`), which is a property of the space rather than of the timing.
    for t in sched.threads.iter_mut() {
        let phys = page_of(t);
        if !(base <= phys && phys < end) {
            continue;
        }
        match region_reap_verdict(t.handshake.state, t.handshake.on_cpu || t.being_reaped) {
            RegionReap::RefuseAndArm => {
                t.killed = true;
                live = true;
            }
            RegionReap::RefuseStanding => standing = true,
            // Unreachable: the finish phase above ran this same verdict to exhaustion, so nothing
            // in the region is still `Blocked` and off its stack. Written as the conservative
            // refusal rather than as an `unreachable!`, because the cost of being wrong here is a
            // panic on a teardown path against the cost of one more retry by an owner that already
            // has a retry loop.
            RegionReap::FinishInPlace => {
                t.killed = true;
                live = true;
            }
            RegionReap::Reap => {}
        }
    }
    if live || standing {
        return Err(());
    }
    // --- Removal phase: every object in the region is reapable. ---

    // Threads: `remove` mutates the table, so this cannot remove while iterating. **Rescan for one
    // at a time**, exactly as the rendezvous sweep above does and for the same reason it gives: the
    // obvious `[u64; MAX_THREADS]` list is a scratch array on the kernel's deepest frame that grows
    // every time the thread ceiling does. Each pass removes one resident, so the set shrinks and
    // this terminates; the cost is O(live threads) per removal on a teardown path. Both Embryo and
    // Finished go.
    loop {
        let doomed = sched
            .threads
            .iter_mut()
            .find(|t| {
                let phys = page_of(t);
                base <= phys && phys < end
            })
            .map(|t| t.id);
        let Some(tid) = doomed else { break };
        // **Unlink a corpse from its supervision rendezvous first.** A supervised thread that died
        // with nobody in `RECEIVE` is parked on that rendezvous's *sender* queue holding its death
        // message (DECISIONS §26 implementation note 2), and that rendezvous is the supervisor's, so
        // it is not in this region and the rendezvous sweep above did not touch it. Freeing the TCB
        // while it is still linked there would leave a dangling pointer that the supervisor's next
        // `RECEIVE` would follow into a recycled page. §16's `DESTROY` could already reach this (reap
        // before receiving); §32's rendezvous reap makes it easy to reach, because a supervisor can be
        // told a tid by its builder and never collect the message at all.
        let parked = sched
            .threads
            .get(tid)
            .filter(|t| t.handshake.state == State::Dead)
            .and_then(|t| t.fault_ep);
        if let Some(ep) = parked {
            let ptr = thread_control_block_ptr(sched, tid);
            // `ptr` is only compared; the corpse's token comes home, to be freed with it.
            if let Some(rendezvous) = rendezvous_of(sched, ep)
                && let Some(token) = rendezvous.remove_sender(ptr)
            {
                hold_token(token);
            }
        }
        // **And the callers this thread will never answer** (milestone 254): a resident reaped
        // here is an `Embryo` or a corpse whose table still holds the reply capabilities it
        // collected, and freeing the TCB is what makes them unreachable. `depart` and the kill
        // conversion in `schedule` cover the threads that ran; this covers the rest.
        strand_callers_of(sched, tid);
        sched.threads.remove(tid);
    }

    Ok(())
}

/// **Reclaim an untyped region and every object retyped from it** (object revocation, the region-
/// ownership half). The owner, holding the untyped capability, reclaims: tear the region's objects
/// down (refusing if any is still live), unpin, and return the memory. Generational names make
/// every capability to the now-dead objects stale on next use, so there is no capability tree to
/// walk and no copies to hunt (contrast seL4's CDT; DECISIONS records the choice).
///
/// Must run outside any `Drop`, because the reap takes `IPC_TABLES` (see `reap_region_objects`); the
/// `unpin` + `destroy` that follow are `IPC_TABLES`-free.
///
/// # BUGS
///
/// **`Err` is destructive, and it does not read that way at a call site.** A refusal caused by a
/// live thread arms DECISIONS §16's kill on *every* live thread in the region, so the first call
/// dooms them and the owner's retry reclaims. **Since milestone 133 an `Ok` is destructive in one
/// more way**: a resident that was `Blocked` is ended outright by the call that succeeded, without
/// running another instruction and without returning from its syscall, and a waiter it was queued
/// beside on somebody else's rendezvous simply stops being there. The rendezvous's owner observes
/// a peer that never arrived, which it cannot tell from a client that never called. That is the point (§24's `^C` escalation is built on
/// it), but it makes `reclaim_region(r).is_err()` unusable as a question: asking it kills the
/// answer. A caller that wants to know whether a region is busy without ending what is in it has no
/// such call today. Milestone 72 traced an intermittent lost-wakeup hang to one line of test code
/// that used the refusal as a probe; see `user::tests::reclaim_frees_a_started_then_exited_childs_regions`.
///
/// **It reaches outside the region, and since milestone 254 it reaches one step further.** Tearing
/// down a rendezvous already drained its wait queues and aborted every waiter, wherever those
/// waiters lived; it now also frees every caller reply-parked on that rendezvous, and every caller
/// still named by a reply capability in a thread this reclaim reaps. Those callers observe
/// [`abi::Error::Gone`] from a `CALL` they made to somebody else's rendezvous. That is the same
/// commitment one object over (`reap_region_objects`'s own comment argues it for the wait queues),
/// and it is written here because the surprise, if there is one, lands on a reader of this function.
pub fn reclaim_region(region: u64) -> Result<(), ()> {
    // A region carved into children cannot be reclaimed: its child regions own part of its run and
    // free those pages themselves. The owner must destroy the children first. Refuse before any
    // teardown, so a refused reclaim leaves the region exactly as it was.
    if crate::memory_region::has_children(region) {
        return Err(());
    }
    let (base, size) = crate::memory_region::region_bounds(region).ok_or(())?;
    // Threads first (IPC_TABLES), then the address spaces the region's destruction ends (the
    // registry's lock, which takes IPC_TABLES beneath it to ask about bound threads). Since §249 the
    // registry owns bound spaces too, so this second step also collects the space of every thread
    // the first step removed, and takes a corpse's space whose root is in the region; it leaves a
    // space whose thread can still run, which is milestone 765's to refuse in the first step.
    reap_region_objects(base, base + size)?;
    crate::user::reap_address_spaces_in_region(base, base + size);
    crate::memory_region::unpin(region);
    crate::memory_region::destroy(region);
    Ok(())
}

/// **Collect a corpse a supervision rendezvous supervises** (DECISIONS §32, `rendezvous::REAP`).
///
/// The one thing a supervisor could not previously do without holding the authority to *build* a
/// process. `ep` is the rendezvous the supervisor invoked; `tid` is the id the kernel stamped on the
/// death message. Authorization is the relationship the kernel already tracks (`Thread::fault_ep`,
/// §26 implementation note 1) rather than a new registry: the named thread's recorded supervision
/// rendezvous must *be* the invoked one, which is why the tid needs no badge and no handle. The
/// decision itself is `capability::reap_decision`, proved for every input in that crate.
///
/// Then the reclaim is §16's, unchanged: `reclaim_region` on the region the TCB was retyped from,
/// which is exactly the region name the owner would have passed to `MemoryRegion::DESTROY`. One teardown
/// path, so the two cannot drift. **The pages go back to the region's owner under §13**, which is
/// the builder, not the reaper: a supervisor frees a child's memory without ever being able to spend
/// it, because it never holds a capability to it.
///
/// Takes and releases `IPC_TABLES` before the reclaim, which takes it again: `reap_region_objects` must
/// run with the lock and cannot be called under it.
pub fn reap_supervised(ep: RendezvousId, tid: ThreadId) -> Result<(), abi::Error> {
    let region = {
        let guard = IPC_TABLES.lock();
        let sched = guard.as_ref().ok_or(abi::Error::NotSupervised)?;
        // A stale or recycled tid resolves to `None` here (generational names, `crates/slots`), so
        // it presents exactly as an unsupervised thread and cannot alias a fresh one.
        let t = sched.threads.get(tid);
        let fault_ep = t.and_then(|t| t.fault_ep);
        let dead = t.is_some_and(|t| t.handshake.state == State::Dead);
        match capability::reap_decision(fault_ep, ep, dead) {
            capability::Reap::NotSupervised => return Err(abi::Error::NotSupervised),
            capability::Reap::StillAlive => return Err(abi::Error::StillAlive),
            capability::Reap::Permitted => {}
        }
        // A supervised thread is always one `create_thread_control_block` built out of a region, so this is `Some`;
        // be honest rather than unwrap, and report "nothing here to collect" if it ever is not.
        t.and_then(|t| t.thread_control_block_region)
            .ok_or(abi::Error::NotSupervised)?
    };
    // `NotPermitted` for the same reasons `DESTROY` gives it: the child `SPLIT` its own budget and a
    // child region still owns part of the run, or a racing reap got there first and the name is now
    // stale. A restart policy reads it as "not yet", which is what it means.
    reclaim_region(region).map_err(|_| abi::Error::NotPermitted)
}

/// **Read one entry of the domain a supervision rendezvous supervises** (milestone 126,
/// `rendezvous::SURVEY`). Returns `(next_cursor, tid, word)`, where `word` is the fact `record`
/// selected; a `next_cursor` of `abi::survey::DONE` means the walk is finished and the other two
/// words are 0.
///
/// **`record` is a selector over per-thread facts**, calef's 2026-09-21 ruling: a new fact is a new
/// `abi::survey::record` value rather than a fourth return register, because a register row that
/// must be redesigned at the sixth field is the wrong mechanism at the fourth. The cursor and the
/// tid are the same for every record, so only the third word moves, and a caller wanting two facts
/// walks the domain twice and joins on the tid.
///
/// **An unknown record is refused before the walk**, so a bad selector against an empty domain is a
/// refusal rather than a `DONE` that a reader would print as "nothing here". A plausible wrong
/// answer is worse than an error.
///
/// **The domain is the one level of supervision the kernel already maintains**, so there is no registry
/// to keep in step with reality and no way for the view to disagree with it. Membership is
/// `capability::survey_includes`, which is the same relationship `reap_supervised` authorizes with
/// and is proved in that crate; a thread appears here exactly when its `Thread::fault_ep` *is* the
/// invoked rendezvous. Nothing about the caller's own identity is consulted, because holding the
/// rendezvous with `READ` is the whole of the claim (the rights check is the syscall layer's).
///
/// **One entry per call, and the lock is given back between them.** A survey of a domain with a
/// hundred children would otherwise hold `IPC_TABLES` for a hundred children's worth of work at a
/// userspace program's discretion, which is a scheduler-latency hole a program could open on
/// purpose. The cost is that the survey is a sequence of snapshots rather than one; see the `BUGS`
/// section of notes/process-view.md, which states exactly what that does and does not promise.
pub fn survey_supervised(
    ep: RendezvousId,
    cursor: u64,
    record: u64,
) -> Result<(u64, u64, u64), abi::Error> {
    // Before anything else, including before the lock: a record this kernel does not answer is
    // `BadMethod`, because the selector is part of the method's name. Doing it here rather than at
    // the point of extraction is what makes the refusal independent of whether the domain happens
    // to have a member to extract from.
    if !abi::survey::record::is_known(record) {
        return Err(abi::Error::BadMethod);
    }
    let guard = IPC_TABLES.lock();
    // Before IPC_TABLES exists there is no domain to report, which is "nothing here", not a
    // refusal: the caller's authority was never in question.
    let Some(sched) = guard.as_ref() else {
        return Ok((abi::survey::DONE, 0, 0));
    };
    // A cursor past the table is empty rather than an error (`iter_from`'s contract), so a caller
    // that keeps feeding back what it was given cannot walk off the end into a refusal that would
    // read as "you may not look".
    let from = usize::try_from(cursor).unwrap_or(usize::MAX);
    for (slot, t) in sched.threads.iter_from(from) {
        if capability::survey_includes(t.fault_ep, ep) {
            let word = match record {
                abi::survey::record::STATE => survey_state(t.handshake.state),
                abi::survey::record::PLACEMENT => survey_placement(t.placement),
                abi::survey::record::CPU_TIME => survey_cpu_time(slot),
                // `is_known` refused every other value above, before the walk began. This arm is
                // not dead defensiveness: it is what makes adding a record a loud two-line edit
                // (the constant, and an arm here) instead of a record that silently reports zero
                // because somebody widened `is_known` and stopped.
                _ => return Err(abi::Error::BadMethod),
            };
            return Ok((slot as u64 + 1, t.id, word));
        }
    }
    Ok((abi::survey::DONE, 0, 0))
}

/// **The ticks charged to the running thread so far**, for a test that measures what a wait costs
/// (milestone 106's before-and-after). Read by the thread itself, because a finished thread's slot
/// may be reused before anyone else looks. Test-only: userspace reads the same counter in
/// milliseconds through `SURVEY`.
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
pub fn current_cpu_ticks() -> u64 {
    CPU_TICKS
        .get(slot_of(current_thread_id()))
        .map_or(0, |c| c.load(Ordering::Relaxed))
}

/// The CPU time a survey reports, as an `abi::survey::record::CPU_TIME` word: **milliseconds**.
///
/// Keyed by the walk's own slot rather than by the tid, because the walk already holds it and the
/// two are the same number (`generational_table` packs the slot in a name's low word). Under
/// `IPC_TABLES` with the thread in hand, so the slot is live and its counter is this thread's.
///
/// **Milliseconds are converted here so the unit never leaves the kernel as a tick.** A tick count
/// would make every reader depend on `TICK_HZ`, and would change meaning underneath them the day
/// that constant moved; the conversion costs one multiply and one divide, on a path that already
/// crossed a syscall boundary and took a lock. All three architectures tick at 100 Hz today, so
/// the answer advances in steps of ten.
fn survey_cpu_time(slot: usize) -> u64 {
    let ticks = CPU_TICKS.get(slot).map_or(0, |c| c.load(Ordering::Relaxed));
    // `TICK_HZ` is 100 everywhere, so this is `ticks * 10` and cannot overflow a `u64` short of
    // 58 million years of continuous CPU time.
    ticks * 1000 / crate::arch::timer::TICK_HZ
}

/// The placement a survey reports, as an `abi::survey::record::PLACEMENT` word.
///
/// Widens a cpu id, and turns the kernel's `u8::MAX` "not placed" into `record::NO_CPU`, which is
/// `u64::MAX`. The two sentinels are deliberately not the same number: widening `u8::MAX` would
/// hand userspace **255**, which is a perfectly plausible cpu id for a reader to tally, and this
/// tree's ruling on a counter it could not trust was that a wrong number is worse than no number.
///
/// Unreachable in practice, and mapped anyway rather than guessed at: a supervision endpoint is
/// recorded at `START` (DECISIONS §26 (the fault endpoint: thread death becomes a message a
/// supervisor holds)), so an embryo is not yet in any domain, and a corpse keeps
/// the placement it started with.
const fn survey_placement(placement: u8) -> u64 {
    if placement == u8::MAX {
        abi::survey::record::NO_CPU
    } else {
        placement as u64
    }
}

/// The run state a survey reports, as an `abi::survey` code.
///
/// `Embryo` and `Finished` are unreachable for a supervised thread and are mapped anyway rather
/// than left to a panic or a wildcard: supervision is recorded at `START`, so an embryo has no
/// `fault_ep` to match, and a supervised death goes to `Dead` rather than `Finished`. Reporting
/// them as `READY` and `DEAD` is the honest nearest neighbour if either ever becomes reachable,
/// and the `match` is exhaustive so a seventh state cannot be added without meeting this.
const fn survey_state(state: State) -> u64 {
    match state {
        State::Embryo | State::Ready => abi::survey::READY,
        State::Running => abi::survey::RUNNING,
        State::Blocked => abi::survey::BLOCKED,
        State::Finished | State::Dead => abi::survey::DEAD,
    }
}

/// **Configure an embryo** (milestone 19c (run a real workload), step 19c.3; §249 (a running address
/// space stays nameable)): bind
/// the address space named by `aspace_name` and set the EL0 entry and user stack. Refuses anything
/// but an `Embryo`, so a running thread cannot be reconfigured under itself. `Ok(())` or a reason.
///
/// **The space stays in the registry and keeps its name** (§249's option A). Until 2026-10-05 this
/// moved the space out of the registry into the thread, which retired the name, so no capability
/// could name a running space and `UNMAP` could not reach the window milestone 95 (an unmap
/// primitive) exists to close. Now the thread keeps a copy of what the context switch reads, and the
/// registry records which thread the space is bound to, which is also what refuses a second bind
/// (§249's amendment (b), `WrongObject`; §105 (`std::thread::spawn` stays declined) stands on it).
///
/// The embryo check runs first, alone, so a TCB that is not an embryo answers `WrongObject` before
/// a stale space name answers `NoSuchSlot`, the order this function has always refused in. It runs
/// again inside the bind, under both locks, because the first answer can be stale by then.
pub fn configure_thread_control_block(
    tid: ThreadId,
    entry: u64,
    user_sp: u64,
    aspace_name: u64,
) -> Result<(), abi::Error> {
    configure_thread_control_block_with_thread_pointer(tid, entry, user_sp, aspace_name, 0)
}

/// [`configure_thread_control_block`], and the thread's first thread pointer with it (milestone
/// 812, §269 (how threads share a process) fork 4; Linux's `CLONE_SETTLS`). `CONFIGURE` reads it
/// from the sixth argument register, which every caller built before 812 sends as zero, and zero is
/// what a thread had before there was a field to hold it.
///
/// Refused with `BadPointer` before anything is bound if it is neither zero nor a user address, so
/// a refusal leaves the embryo exactly as it was. Set in the same critical section as the space and
/// the entry, so a `START` racing this `CONFIGURE` sees all three or none.
///
/// Name: provisional (milestone 812 (`std::thread::spawn` runs real threads in one address space)'s
/// lane, 2026-10-10 UTC).
pub fn configure_thread_control_block_with_thread_pointer(
    tid: ThreadId,
    entry: u64,
    user_sp: u64,
    aspace_name: u64,
    thread_pointer: u64,
) -> Result<(), abi::Error> {
    if !is_thread_pointer(thread_pointer) {
        return Err(abi::Error::BadPointer);
    }
    {
        let guard = IPC_TABLES.lock();
        let sched = guard.as_ref().ok_or(abi::Error::NoSuchSlot)?;
        let t = sched.threads.get(tid).ok_or(abi::Error::NoSuchSlot)?;
        if t.handshake.state != State::Embryo {
            return Err(abi::Error::WrongObject); // only an unstarted TCB may be configured
        }
    }

    // **This is the moment a bare address space becomes a thread's**, so it is the moment the
    // current-CPU page belongs in it (calef's 2026-09-21 ruling on a thread observing itself), and
    // `bind_user_address_space` attaches it before handing over the copy. The closure runs under the
    // registry's lock (`ADDRESS_SPACES`, 61) and takes `IPC_TABLES` (60) beneath it, so the
    // registry's bound mark and the thread's copy are written in one critical section.
    crate::user::bind_user_address_space(aspace_name, tid, |bound| {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
        let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
        if t.handshake.state != State::Embryo {
            return Err(abi::Error::WrongObject);
        }
        t.space = Some(bound);
        t.entry = (entry, user_sp);
        t.thread_pointer = thread_pointer;
        Ok(())
    })
}

/// **Is this a value a thread pointer may hold**: zero (none), or an address in the user half.
///
/// Not alignment, and not whether anything is mapped there: the register is a pointer the program
/// dereferences itself, and a bad one faults the program that chose it. The half matters because
/// `x86_64`'s `wrmsr` to `IA32_FS_BASE` raises `#GP` in the kernel on a non-canonical value, and a
/// kernel-half value is a pointer no user access through it could use. The same rule on all three
/// architectures, so a program refused on one is refused on every one (§19 (architectural parity is
/// a tenet)).
fn is_thread_pointer(value: u64) -> bool {
    use paging::PageFormat;
    value == 0 || crate::arch::mmu::Format::is_in_half(paging::Half::Low, value)
}

/// **The calling thread's thread pointer**, for the first entry to user mode (riscv64 writes it
/// into the frame that entry builds; see `arch::thread_pointer::set_initial`). Zero for a thread
/// that has none, and for a kernel thread calling from outside any thread table.
pub fn current_thread_pointer() -> u64 {
    let guard = IPC_TABLES.lock();
    guard
        .as_ref()
        .and_then(|sched| sched.threads.get(current_thread_id()))
        .map_or(0, |t| t.thread_pointer)
}

/// **`ThreadControlBlock::SET_THREAD_POINTER`**: change the thread pointer of the thread `tid`
/// names (milestone 812, §269 fork 4; seL4's `SetTLSBase`).
///
/// Two targets are allowed, and the refusal of the third is what keeps this simple. An **embryo**
/// (not started) takes the value at its first switch in. **The caller itself** takes it now: the
/// field and the register (`arch::thread_pointer::set_live`, given `frame`, the frame the caller
/// returns through) change under one hold of `IPC_TABLES`. One hold, because on aarch64 a switch
/// between the two writes would save the old register over the new field. **Any other started thread is refused with
/// `WrongObject`**: its register lives on whatever core it runs on, or in a saved context or trap
/// frame whose location differs per architecture, and nothing `std` does needs to reach it. seL4
/// allows it; the refusal can be lifted additively if a debugger ever wants it.
///
/// `BadPointer` for a value [`is_thread_pointer`] refuses, before anything changes.
///
/// Name: provisional (milestone 812's lane, 2026-10-10 UTC).
pub fn set_thread_pointer(
    tid: ThreadId,
    value: u64,
    frame: &mut crate::arch::exceptions::TrapFrame,
) -> Result<(), abi::Error> {
    if !is_thread_pointer(value) {
        return Err(abi::Error::BadPointer);
    }
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
    let caller = current_thread_id();
    let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
    if t.handshake.state == State::Embryo {
        t.thread_pointer = value;
    } else if tid == caller {
        t.thread_pointer = value;
        crate::arch::thread_pointer::set_live(frame, value);
    } else {
        return Err(abi::Error::WrongObject);
    }
    Ok(())
}

/// **Grant an embryo the cycle counter** (milestone 229, DECISIONS 139 option 4): the thread this
/// TCB names may read `PMCCNTR_EL0` (aarch64) or the `cycle` CSR (riscv64) from user mode once it
/// runs. `x86_64` already lets every thread read the TSC and this changes nothing there, which is
/// DECISIONS 139 part 3 and a stated exception to §19 rather than a gap.
///
/// **Refuses a non-embryo**, exactly as [`configure_thread_control_block`] and
/// [`thread_control_block_insert_cap`] do, and that refusal is the security property rather than
/// housekeeping: it is what makes this a field in the thread's spawn manifest instead of something
/// a running program can ask for. A timing instrument acquired at will is a timing instrument
/// nobody declared.
///
/// One-way: there is no ungrant, because an embryo starts closed and nothing but this opens it.
///
/// **Nothing calls this today, and that is milestone 229's decision rather than an oversight.**
/// The syscall method that would let a loader call it was deliberately not minted: see
/// `abi::thread_control_block`'s standing note, whose short form is that a method number is
/// irreversible and `seL4_TCB_SetAffinity` is the worked example of one that had to be retired.
/// This is the kernel half of the mechanism, complete and tested, waiting for whoever mints the
/// surface with a requirement in hand.
///
/// `#[inline(never)]` for the reason milestone 156 gives `memory_region_map` and the other
/// spawn-path bodies: this is administration a loader runs once per child, never a step of the IPC
/// round trip, so it does not belong in the bytes `script/fastpath-footprint` bounds. It is not a
/// style choice here, it is a measurement: without it the riscv64 `syscall_entry` set grew 12%
/// against a 5% bound, because the callee folded into `invoke`.
#[inline(never)]
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub fn grant_cycle_counter(tid: ThreadId) -> Result<(), abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
    let t = sched.threads.get_mut(tid).ok_or(abi::Error::NoSuchSlot)?;
    if t.handshake.state != State::Embryo {
        return Err(abi::Error::WrongObject);
    }
    t.cycle_counter_grant = true;
    Ok(())
}

/// **Grant the *running* thread the cycle counter, for tests only** (milestone 229).
///
/// This deliberately breaks the rule [`grant_cycle_counter`] enforces, which is why it is
/// `#[cfg(test)]` and cannot exist in a shipped kernel. It is here because milestone 229 shipped
/// the mechanism without the syscall method that would set it, so there is no honest userspace
/// route to a granted thread and the alternative was to leave the EL0 half of the mechanism
/// unexercised. Same spirit as the `soak` and `fastpath_pad` affordances: a door that exists only
/// in a build nobody runs.
///
/// It writes the register itself as well as the field, because the calling thread is already
/// running and will not pass through `schedule`'s switch again before it drops to EL0. Every later
/// switch back into this thread re-applies the same value from the field, which is the ordinary
/// path.
#[cfg(feature = "system_tests")]
pub fn grant_cycle_counter_to_current() {
    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        let current = current_thread_id();
        sched
            .threads
            .get_mut(current)
            .expect("no current thread")
            .cycle_counter_grant = true;
    }
    crate::arch::timer::set_cycle_counter_grant(true);
}

/// **Record that the running thread has started using the floating-point unit** (milestone 447).
///
/// The scheduler half of [`crate::fp::enable_for_current`], and the only thing that ever sets the
/// flag. Returns false when there is no scheduler or no current thread, which is a trap the caller
/// must turn into a fault rather than return from: the flag is what stops the same instruction
/// taking the same trap forever.
///
/// **Taking `IPC_TABLES` inside a trap handler is ordinary here**, not a liberty. Every syscall
/// does it from the same place, through `syscall::dispatch`, and the argument is the same: this
/// trap came from a thread that was *running*, so this core cannot already hold the lock. What
/// would break that is kernel code executing an FP instruction while holding it, and the kernel is
/// built `softfloat` and executes none.
pub fn mark_current_fp_live() -> bool {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return false;
    };
    let current = current_thread_id();
    let Some(pointer) = sched.threads.pointer(current) else {
        return false;
    };
    // The register file is in the TCB page beside the struct, so this goes through the page pointer
    // rather than through a `&mut Thread`; see `Threads::pointer`.
    //
    // SAFETY: a live thread's TCB page, held under IPC_TABLES.
    unsafe { (*crate::thread::fp_state_of(pointer)).set_live() };
    true
}

/// **Install a capability into an embryo's capability table** (milestone 19c.3): the child's initial
/// authority, granted one slot at a time before it runs. Refuses a non-embryo. Returns the child
/// slot the capability landed in.
///
/// `target` is `None` for first-free placement (the original behaviour) or `Some(slot)` to place
/// the capability in a specific free slot, which a supervisor uses to put a child's supervision
/// rendezvous in the reserved fault slot (milestone 22). A targeted insert into an occupied or
/// out-of-range slot is `OutOfMemory`, so the reservation cannot be quietly overwritten.
///
/// **For a capability the kernel mints, not one read from a table**: `CAP_INSERT` endows an embryo
/// with a copy of one the spawner holds, and that goes through
/// [`thread_control_block_delegate_cap`] ([`Delegation`] has why).
pub fn thread_control_block_insert_cap(
    tid: ThreadId,
    capability: crate::cap::Cap,
    target: Option<u64>,
) -> Result<u64, abi::Error> {
    thread_control_block_insert_from(tid, target, |_, _| Ok(capability))
}

/// **`ThreadControlBlock::CAP_INSERT`'s body: endow an embryo with a narrowed copy of a capability
/// the running thread holds**, the source read under the `IPC_TABLES` hold that files the copy
/// ([`Delegation`]). Name provisional.
pub fn thread_control_block_delegate_cap(
    tid: ThreadId,
    delegation: Delegation,
    target: Option<u64>,
) -> Result<u64, abi::Error> {
    thread_control_block_insert_from(tid, target, |sched, current| {
        let caps = sched
            .threads
            .capabilities(current)
            .ok_or(abi::Error::NoSuchSlot)?;
        delegation.derive(&caps.lock())
    })
}

/// The body both share. `source` runs first, so its answer comes before the embryo's, the order the
/// syscall layer answered in when it read the source itself. Its table lock is released before the
/// embryo's is taken: two tables are never held at once (`sync::rank::CAPABILITY_TABLE`).
fn thread_control_block_insert_from(
    tid: ThreadId,
    target: Option<u64>,
    source: impl FnOnce(&IpcTables, ThreadId) -> Result<crate::cap::Cap, abi::Error>,
) -> Result<u64, abi::Error> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
    let capability = source(sched, current_thread_id())?;
    let (t, caps) = sched
        .threads
        .get_mut_with_capabilities(tid)
        .ok_or(abi::Error::NoSuchSlot)?;
    if t.handshake.state != State::Embryo {
        return Err(abi::Error::WrongObject);
    }
    let landed = match target {
        None => caps.lock().insert(capability),
        Some(slot) => caps.lock().insert_at(slot, capability),
    }
    .map_err(|_| abi::Error::OutOfMemory)?;
    // **The one choke point where a port capability enters a thread** (milestone 299): the boot's
    // own child builder and the progenitor's `ThreadControlBlock::CAP_INSERT` both endow an embryo
    // through here, so caching the grant here is what makes the context switch's port-grant read
    // one field read instead of a capability-table scan. Set only on the `x86_64` build that
    // enforces it, and only after the insert succeeded, so a full table leaves the grant untouched.
    //
    // A thread that is handed more than one port range keeps only the last, which every real
    // consumer is well within (a console driver holds exactly one, for COM1); see this milestone's
    // BUGS. A `PortRange` delegated to an *already running* thread by `SEND_CAP` is likewise not
    // cached here, because no consumer does that and the enforcement is a creation-time grant, the
    // same posture `cycle_counter_grant` takes.
    //
    // **A `PortRange` without `WRITE` grants nothing** (milestone 768 (provisional), calef's ruling
    // of 2026-10-05 UTC on the second outsider pass, DECISIONS §121 (what a device capability is
    // when the device has no page: x86 port I/O)). The TSS I/O bitmap has one bit per port and
    // cannot permit `in` without `out`, so there is no honest read-only grant to install; READ
    // cannot be granted without WRITE, so WRITE is the right that opens the ports and a capability
    // without it opens none. The capability still lands in the table (REVOKE and delegation are
    // unchanged); only the cached grant is withheld.
    #[cfg(target_arch = "x86_64")]
    if let crate::cap::Object::PortRange(base, count) = capability.object
        && capability.rights.allows(crate::cap::Rights::WRITE)
    {
        t.port_range_grant = Some((base, count));
    }
    Ok(landed)
}

/// **Start an embryo** (milestone 19c.3): the no-start-before-whole gate, then make it runnable.
/// Refuses a TCB that is not an embryo, or one with no bound address space or no entry set: a
/// half-built thread must never run. On success the thread gets its kernel stack and entry
/// context and joins this core's run queue.
pub fn start_thread_control_block(tid: ThreadId, args: [u64; 3]) -> Result<(), abi::Error> {
    // **The kernel stack is built before `IPC_TABLES` is taken** (2026-10-04), for the reason the
    // reaper frees one after releasing it: building it maps six pages and allocates their frames,
    // and every other core's IPC and capability lookups waited behind that while it ran under the
    // lock (notes/job-mix/null-syscall-under-load.md). Declared before the guard, so a refusal below
    // releases the lock first and frees the unused stack afterwards. The refusals keep their order:
    // a missing stack is still reported only after the embryo checks have passed.
    let stack = crate::thread::KernelStack::new();
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().ok_or(abi::Error::NoSuchSlot)?;
    let (t, caps) = sched
        .threads
        .get_mut_with_capabilities(tid)
        .ok_or(abi::Error::NoSuchSlot)?;

    if t.handshake.state != State::Embryo {
        return Err(abi::Error::WrongObject); // already started (or not a TCB)
    }
    // WHOLE, or refuse: a bound address space and an entry point. Either missing is a half-built
    // thread, and starting it would drop to EL0 with no low half or no code.
    if t.space.is_none() || t.entry.0 == 0 {
        return Err(abi::Error::NotPermitted); // configure it first
    }

    // **The spawn-slot convention** (milestone 22, DECISIONS §26). If the reserved fault slot holds
    // a Rendezvous capability, this thread is supervised: record it as the fault target
    // and consume the slot, so the child cannot forge fault messages on it (the kernel stays the
    // only sender on this path, §26.5). Supervision is fixed here, at spawn, and never changes.
    //
    // **The capability's badge is the child's label, and it is kept** (milestone 105, DECISIONS
    // §148 as amended 2026-10-04, ruling R3). The builder sets it with `rendezvous::BADGE` before
    // the insert, and the kernel delivers it with the death message so a supervisor of several
    // children can say which one died. Consuming the slot below is also what keeps it from the
    // child: no capability it holds carries it, and nothing else reports it.
    let mut table = caps.lock();
    if let Ok(fault_cap) = table.get(abi::fault::FAULT_EP_SLOT)
        && let crate::cap::Object::Rendezvous(ep, label) = fault_cap.object
    {
        t.fault_ep = Some(ep);
        t.fault_label = label;
        let _ = table.delete(abi::fault::FAULT_EP_SLOT);
    }
    drop(table);

    t.start_args = args; // the child's x0, x1, x2 (19d/19e)
    let Some(stack) = stack else {
        return Err(abi::Error::OutOfMemory); // no kernel stack to be had
    };
    t.arm_for_start(stack);
    t.handshake.state = State::Ready;
    // Placement is the power of two choices (DECISIONS §28), the same as `spawn`: a freshly started
    // user thread lands on the lighter of two sampled cores rather than always the starter's, so a
    // process that spawns a pipeline does not pile it all onto one core. `place_on` enqueues locally
    // or hands the thread to the target's inbox; the SGI that makes a remote target pick it up goes
    // out after IPC_TABLES is released.
    // The decision, once, under the lock. Asking `target != cpu::id()` again after `drop(guard)`
    // unmasks interrupts first, so this thread can be stolen onto `target` in between and the
    // second answer skips the SGI the first one owed (see `place_on`).
    let target = pick_spawn_target();
    // Record it on the thread, so a supervisor can survey it (`abi::survey::record::PLACEMENT`).
    // This is the path every *user* thread takes, so it is the one a survey actually reads: a
    // supervised thread is always one a `START` put on a core. `t` is still borrowed here, which is
    // why this is before `take_token` re-borrows `sched`. The token it takes is the one the thread
    // table minted when this embryo was inserted; `Embryo -> Ready` above happens once, so it is
    // taken once.
    t.placement = target as u8;
    let remote = place_on(target, take_token(sched, tid));
    drop(guard);
    if let Some(target) = remote {
        crate::arch::irq::send_reschedule(target);
    }
    Ok(())
}

/// Hand the current thread an address space, and install it.
///
/// The space goes into the address-space registry, bound to this thread (§249: the registry owns
/// every space), and the thread keeps the copy the context switch reads. From here the reaper takes
/// it out of the registry and drops it when the thread dies, and every context switch back to this
/// thread re-installs it.
///
/// Returns the space's name in the registry, which an address-space capability carries, for a
/// spawn that hands a process its own space (`user::run_with_own_space`).
pub fn adopt_address_space(space: crate::user::AddressSpace) -> u64 {
    let current = current_thread_id();
    // Before `IPC_TABLES`: the registry's lock ranks above it. The thread is live and running, so
    // the region sweep cannot take the entry in the moment before the thread holds its copy.
    let bound = crate::user::register_bound_address_space(space, current);

    // **The current-CPU page is written here as well as at switch-in**, and the test that found
    // this is the reason it is not obvious. `schedule()` writes the page of the thread it is
    // switching TO, which covers every thread that has a space before it first runs. A thread that
    // adopts one *while already running* (the `sched::spawn(|| run(image, ...))` shape every
    // kernel-side spawn uses) was switched in before it had a space at all, so its first user
    // instruction ran against a page nobody had touched and `current_cpu` answered `None` until the
    // next preemption. This is that thread's switch-in, arriving late; `cpu::id()` is the core it
    // is standing on right now, which is exactly what the switch would have written.
    bound.publish_current_cpu(cpu::id() as u64);

    {
        let mut guard = IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        sched
            .threads
            .get_mut(current)
            .expect("no current thread")
            .space = Some(bound);
    }

    // SAFETY: `bound` is the copy of a space the registry now owns, bound to the *current* thread.
    // The current thread is the one executing this line, so it is on a CPU and cannot be reaped,
    // and the space is dropped only once its thread can never be switched in again.
    unsafe { crate::arch::mmu::switch_user_root(bound.ttbr0()) };
    bound.name()
}

/// **Can the thread a bound address space names still run?** (§249; name provisional.) What the
/// region sweep asks before it takes a bound space: `user::reap_address_spaces_in_region` has the
/// rule each answer feeds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Binder {
    /// The name no longer resolves: the thread was reaped. Generational, so it is never a newer
    /// thread in the same slot.
    Gone,
    /// `Dead` or `Finished`, and no core is standing on its stack: it will never be switched in
    /// again, so nothing will install its root.
    Corpse,
    /// Anything else, an embryo included (a `START` would run it), and a corpse still on a core.
    CanRun,
}

/// **Answer [`Binder`] questions under one hold of `IPC_TABLES`**: `f` is given the question and may
/// ask it as often as it likes. One hold rather than one per thread, because the sweep asks about
/// every bound space on every scan. Taken under the address-space registry's lock (61 above 60). A
/// scheduler that does not exist yet has bound nothing, and answers `CanRun` so nothing is freed on
/// a guess.
pub fn with_binders<R>(f: impl FnOnce(&dyn Fn(ThreadId) -> Binder) -> R) -> R {
    let guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_ref() else {
        return f(&|_| Binder::CanRun);
    };
    f(&|tid| match sched.threads.get(tid) {
        None => Binder::Gone,
        Some(t)
            if matches!(t.handshake.state, State::Dead | State::Finished)
                && !t.handshake.on_cpu =>
        {
            Binder::Corpse
        }
        Some(_) => Binder::CanRun,
    })
}

/// The top of the current thread's kernel stack: **where its `TrapFrame` belongs.**
///
/// `None` for the boot thread, which runs on the stack `boot.s` set up and does not own it.
///
/// A user thread's `TrapFrame` is not an ordinary local. It must sit at exactly the address the
/// vector table's `SAVE_CONTEXT` will rebuild it at when the user traps in, because `eret`
/// leaves `SP_EL1` pointing just past it and the hardware does not consult our intentions.
pub fn current_kernel_stack_top() -> Option<u64> {
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    sched
        .threads
        .get(current_thread_id())?
        .stack
        .as_ref()
        .map(|s| s.top())
}

pub fn current() -> ThreadId {
    current_thread_id()
}

/// **The user PC recorded in `tid`'s trap frame** (milestone 71, test support), read from the top of
/// its kernel stack, which is the one address the trap path and the user-entry path must agree on.
/// `None` if the name does not resolve or the thread has no kernel stack of its own.
///
/// This is [`dump_threads`]'s per-thread PC lookup, exposed so a test can assert the agreement
/// rather than only a human reading a hang dump. A thread that has reached user mode reads back a
/// user address here; a zero means nothing wrote a frame where the trap path will look for one.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn user_pc_of(tid: ThreadId) -> Option<u64> {
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    let t = sched.threads.get(tid)?;
    t.stack
        .as_ref()
        .map(|s| crate::arch::exceptions::user_pc(s.top()))
}

/// **Postmortem: read a corpse's retained fault/exit message** (milestone 22, test support). A
/// `Dead` thread keeps its five-word §26 message until the supervisor reaps it, so this proves the
/// corpse's TCB still holds its fault-time state after the notification was delivered. `None` if
/// the name does not resolve or the thread is not a corpse.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn corpse_fault_msg(tid: ThreadId) -> Option<[u64; 5]> {
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    let t = sched.threads.get(tid)?;
    (t.handshake.state == State::Dead)
        .then_some(t.fault_msg)
        .flatten()
}

/// **Is `tid` still in the thread table?** (test support.)
///
/// The narrow question "was *this* thread reaped", which is what a test that spawned one actually
/// wants to know. [`thread_count`] answers a wider one, and the width is a defect when it is used
/// this way: the count is the size of the whole table, so an unrelated process finishing its
/// teardown moves it, and a test waiting for the count to come back to a baseline is really waiting
/// for the rest of the system to hold still. It need not.
///
/// The name is generational, so a reaped thread's `ThreadId` never resolves again even if its slot is
/// reused: `false` here means gone, not "gone or replaced".
/// **`tid`'s capability table, read in place** (test support, milestone 757 (a test
/// kernel fails a process on its Nth retype), provisional name). `None` for a name that does not
/// resolve. A sweep that fails a service's Nth retype reads this before and after each run, because
/// a cleanup path that forgets a `cap_delete` crashes nothing and shows up only here. It lends the
/// table to `read` rather than returning a copy: a copy is a 2 KiB array in every frame that holds
/// it, which at 64 slots put the caller over the guard page (milestone 754 (the capability table
/// grows to 64 slots)). `read` runs under `IPC_TABLES`, so it must not call back into `sched`.
#[cfg(feature = "system_tests")]
pub fn with_capability_table<R>(
    tid: ThreadId,
    read: impl FnOnce(&crate::cap::CapabilityTable) -> R,
) -> Option<R> {
    let guard = IPC_TABLES.lock();
    let t = guard.as_ref()?.threads.capabilities(tid)?.lock();
    Some(read(&t))
}

/// **The root of `tid`'s address space**, `None` for a kernel thread or a name that does not
/// resolve (test support, milestone 779 (fuzz the surface a confined process can reach),
/// provisional name). The confined fuzzer's page oracle walks a fuzzer's mappings from here with
/// `revoke::list_mapping` and `arch::mmu::translate_at`; no syscall hands a process's own root to
/// anybody, which is why the test reads it here.
#[cfg(feature = "system_tests")]
pub fn thread_space_root(tid: ThreadId) -> Option<u64> {
    let guard = IPC_TABLES.lock();
    guard
        .as_ref()?
        .threads
        .get(tid)?
        .space
        .as_ref()
        .map(|s| s.root())
}

/// **The physical page a page-resident kernel object lives in** (test support, milestone 779,
/// provisional name): a rendezvous, a notification or a TCB. `None` for any other object, and for a
/// name that no longer resolves. The confined fuzzer's capability oracle admits an object it was not
/// granted only when this page lies inside a memory region the fuzzers were given, which is how it
/// tells an object a fuzzer made from one it reached. The same test `reap_region_objects` applies.
#[cfg(feature = "system_tests")]
pub fn object_page(object: &crate::cap::Object) -> Option<u64> {
    use crate::cap::Object;
    // An address-space object's page is its root, out of the user-space registry and *before*
    // `IPC_TABLES`, so the two locks never nest (the confined fuzzer retypes these from its own
    // region, milestone 779).
    if let Object::AddressSpace(name) = *object {
        return crate::user::user_address_space_root(name);
    }
    let guard = IPC_TABLES.lock();
    let sched = guard.as_ref()?;
    match *object {
        Object::Rendezvous(id, _) => sched.rendezvous_table.get(id).copied(),
        Object::Notification(id) => sched.notification_table.get(id).copied(),
        Object::ThreadControlBlock(tid) => sched
            .threads
            .get(tid)
            .map(|t| crate::arch::mmu::virt_to_phys(t as *const Thread as u64)),
        _ => None,
    }
}

/// **One service move on a rendezvous that can never park the caller** (test support, milestone
/// 779 (fuzz the surface a confined process can reach), provisional name). The confined fuzzer's
/// conductor must never block: it is the one thread judging every fuzzer, and three separate
/// hangs taught that a check-then-act around `ipc_send`/`ipc_receive_cap` is a race on a
/// multicore machine, because the party the check saw can be gone by the act, and the blocking
/// primitive then parks the conductor with no one left to wake it. This is the act and the check
/// as one: under a single hold of `IPC_TABLES`, deliver `word` to a parked receiver; otherwise
/// collect one queued sender exactly as `ipc_receive_cap` would (any capability it carried is
/// filed in the caller's table, `x1` names the slot); otherwise nothing, with nothing queued and
/// nothing parked. The speculative queue positions the crate's own `send`/`receive` take are
/// taken straight back, under the same hold, with `remove_sender`/`remove_receiver`.
#[cfg(feature = "system_tests")]
pub enum ConductorMove {
    /// Nothing parked either way: nothing happened, nothing was left behind.
    None,
    /// A parked receiver took `[word, 0, 0]`.
    Sent,
    /// A queued sender was collected: its five words, shaped as `RECEIVE_CAP` returns them.
    Took([u64; 5]),
}

/// [`ConductorMove`]'s one operation. See the enum for the contract and the reason.
#[cfg(feature = "system_tests")]
pub fn conductor_move(ep: RendezvousId, word: u64) -> ConductorMove {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut().expect("no scheduler");
    let current = current_thread_id();
    let me = thread_control_block_ptr(sched, current);
    let Some(rendezvous) = rendezvous_of(sched, ep) else {
        return ConductorMove::None; // a stale name: nobody is parked anywhere
    };
    // The running thread's token; `me` is its pointer, for the removes below to compare.
    match rendezvous.send(take_token(sched, current)) {
        inter_process_communication::Send::Rendezvous(receiver, token) => {
            hold_token(token);
            let receiver = hold_token(receiver);
            let r = sched.threads.get_mut(receiver).unwrap();
            r.mailbox = [word, 0, 0, 0, 0];
            r.handshake.serve(); // delivered: this wake passes the boot-8 gate
            trace::record(trace::Event::Served, receiver, 1);
            wake(sched, receiver);
            ConductorMove::Sent
        }
        // An endpoint bound to an interrupt takes no message from anybody (§101 ruling B).
        inter_process_communication::Send::Refused(token) => {
            hold_token(token);
            ConductorMove::None
        }
        inter_process_communication::Send::Blocked => {
            // `send` queued `current` as a sender; take that straight back, under the same hold,
            // so no receiver can match the speculative position. The removal hands back the token
            // the send took, which is what the receive below needs.
            let token = rendezvous
                .remove_sender(me)
                .expect("the send above queued this thread under this same hold");
            match rendezvous.receive(token) {
                inter_process_communication::Receive::FromSender(sender, token) => {
                    hold_token(token);
                    let sender = hold_token(sender);
                    let msg = sched.threads.get(sender).unwrap().mailbox;
                    let capability = sched.threads.get_mut(sender).unwrap().outgoing_cap.take();
                    let is_reply = matches!(capability, Some(c) if matches!(c.object, crate::cap::Object::Reply(_)));
                    let slot = match capability {
                        Some(c) => {
                            deliver_capability(sched.threads.capabilities(current).unwrap(), c)
                        }
                        None => NO_CAP,
                    };
                    if !is_reply {
                        // Collected: the sender's rendezvous is complete (the boot-8 gate).
                        let s = sched.threads.get_mut(sender).unwrap();
                        s.handshake.serve();
                        trace::record(trace::Event::Served, sender, 4);
                        wake(sched, sender);
                    }
                    let tag = if is_reply { reply_tag(slot) } else { 0 };
                    ConductorMove::Took([msg[0], slot, msg[1], msg[3], tag])
                }
                // A pending interrupt signal, as `ipc_receive_cap` would report it.
                inter_process_communication::Receive::Signal(token) => {
                    hold_token(token);
                    ConductorMove::Took([1, NO_CAP, 0, 0, 0])
                }
                inter_process_communication::Receive::Blocked => {
                    // This hold queued `current`; this hold takes it back, token and all.
                    let token = rendezvous
                        .remove_receiver(me)
                        .expect("the receive above queued this thread under this same hold");
                    hold_token(token);
                    ConductorMove::None
                }
            }
        }
    }
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn is_thread_present(tid: ThreadId) -> bool {
    IPC_TABLES
        .lock()
        .as_ref()
        .is_some_and(|s| s.threads.get(tid).is_some())
}

/// **Arm a kill on one thread by name** (test support), the single-thread form of what
/// [`reap_region_objects`] does to a whole region.
///
/// This exists because a test that spawns a **bare** user thread had no way to take it back. A
/// thread spawned into a reclaimable region is torn down by reclaiming the region; a thread spawned
/// with plain [`spawn`] around `user::run` belongs to no region, so there was no handle to end it
/// with. Two tests need a subject that never exits on its own (a spinner whose point is that it
/// never yields, and a child that must hold the free-frame count still while it is read), and both
/// therefore leaked a runnable thread for the rest of the suite. Two spinning threads on a four-hart
/// machine is a scheduling load the rest of the suite then runs under, which is how it presented:
/// `reclaim_frees_a_started_then_exited_childs_regions` starved and tripped its watchdog, on CI,
/// intermittently, far from the tests that caused it.
///
/// **No new syscall and no change to the user-visible surface.** DECISIONS §16's armed kill is the
/// whole mechanism: setting `killed` makes the scheduler convert the thread to a corpse at its next
/// preemption, which is exactly how `DESTROY` and §24's `^C` escalation already work. Rule 3 governs
/// the syscall boundary; this is an in-kernel function for in-kernel tests.
///
/// Returns whether a live thread was found and marked. A `false` means the `ThreadId` did not resolve,
/// which for a generational name means the thread is already gone rather than that the kill failed.
///
/// The kill is **armed, not immediate**: the thread dies at its next preemption, so a caller that
/// needs it actually gone waits for [`is_thread_present`] to go false rather than assuming.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn kill_thread(tid: ThreadId) -> bool {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return false;
    };
    match sched.threads.get_mut(tid) {
        Some(t) => {
            t.killed = true;
            true
        }
        None => false,
    }
}

// The ordinary boot and the system tests call this; the kernel's unit-test boot does neither
// (milestone 609 (the system tests leave the kernel crate)).
#[cfg_attr(test, allow(dead_code))]
pub fn thread_count() -> usize {
    IPC_TABLES.lock().as_ref().map_or(0, |s| s.threads.len())
}

/// **Count the runnable threads that are not the caller and not an idle thread** (test support).
///
/// A leaked one-shot driver that spins forever instead of exiting is `Ready`/`Running` for the rest
/// of the boot; a thread doing legitimate work is `Blocked` on an rendezvous when the system is
/// quiescent. So, from a quiesced probe (yield until pending exits are reaped), this count is the
/// number of leaked spinners: the idle threads (one per core) and the probe itself are the only
/// runnable threads a clean system has. The regression proxy for the test-thread starvation that
/// made the RedoxFS mount overrun the hang watchdog.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn runnable_non_idle_count(&exclude: &ThreadId) -> usize {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return 0;
    };
    let mut idles = [u64::MAX; crate::cpu::MAX_CPUS];
    // Harvest each ONLINE core's idle tid by set membership, not `0..count` (first-silicon sweep,
    // 2026-08-14): with the VisionFive 2's {1,2,3} online, count-as-index misses cpu 3's idle tid,
    // and its idle thread would then be counted as a leaked spinner. The array slot order does not
    // matter; only membership in `idles` does.
    for (slot, c) in idles.iter_mut().zip(crate::smp::online_cpus()) {
        *slot = crate::cpu::of(c).idle.load(Ordering::Relaxed);
    }
    sched
        .threads
        .iter_mut()
        .filter(|t| {
            matches!(t.handshake.state, State::Ready | State::Running)
                && t.id != exclude
                && !idles.contains(&t.id)
        })
        .count()
}

/// Print every thread's scheduler state, for diagnosing a hang. A lost IPC wakeup leaves a thread
/// `Blocked` forever with nothing to wake it; this shows which thread, and the `on_cpu`/`wake_pending`
/// flags that would reveal a botched wake-before-switch-out handoff. Takes `IPC_TABLES`, which is free when
/// the hang is a blocked thread (not a lock deadlock). Used by the test watchdog.
/// Feed every live thread's stack into the high-water accounting (milestone 84). Long-lived
/// service threads (the FS server, the shape of the incident that motivated the measurement) are
/// never reaped, so their stacks are only visible here, not in `KernelStack`'s `Drop`. A thread may
/// be running on another core while its stack is scanned; the scan reads a snapshot, and a racing
/// deepening is at worst under-reported by this run (see `stack::high_water`).
#[cfg(any(test, feature = "system_tests"))]
pub fn scan_live_thread_stacks() {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return;
    };
    for t in sched.threads.iter_mut() {
        if let Some(s) = t.stack.as_ref() {
            // SAFETY: a `KernelStack` this thread still owns, so its pages are mapped until its
            // `Drop` runs, and `KernelStack::new` painted the whole span. `IPC_TABLES` is held, so the
            // thread cannot be reaped out from under the scan.
            let used = unsafe { crate::stack::high_water(s.bottom(), s.top()) };
            crate::stack::note_thread_stack_use(used);
        }
    }
}

#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn dump_threads() {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        crate::println!("  dump_threads: no scheduler");
        return;
    };
    // What this dump can and cannot honestly claim (first-silicon audit, 2026-08-14):
    //
    //   - `state`/`on_cpu`/`wake_pending`/`wait`, and the rendezvous counts, are a CONSISTENT
    //     snapshot: every writer holds IPC_TABLES, which this dump holds.
    //   - `pc` is the trap frame at the thread's stack top, which trap entry writes WITHOUT
    //     IPC_TABLES. For an off-cpu thread it is trustworthy (the frame write happened-before the
    //     state write on that core, and our lock acquire synchronises with its release). For a
    //     thread on a cpu it is a racing read of live state, printed with a `*`.
    //   - the per-core lines read other cores' atomics relaxed. `current` is written under
    //     IPC_TABLES, so it is quiescent while we hold the lock, except for a core mid-switch
    //     (between its IPC_TABLES release and its finish_switch): such a core's `current` names the
    //     incoming thread while the outgoing one still runs for a few more instructions.
    //   - `ticks` is that core's timer-interrupt count. A core whose ticks FREEZE across dumps
    //     is not taking traps at all: wedged with interrupts masked, or stuck inside an SBI call
    //     in M-mode (where delegated S-interrupts cannot preempt), which no other row can show.
    // The stage breadcrumb repeats here on purpose: a serial line printed once can go missing
    // from a bench log (boots 7 through 9 were convicted on exactly that absence), but a dump
    // that fires every couple of seconds re-states how far the tour got each time.
    crate::println!(
        "--- thread dump (hang diagnostic; pc* = on-cpu, racy; tour stage {}) ---",
        BOOT_STAGE.load(Ordering::Relaxed),
    );
    for t in sched.threads.iter_mut() {
        let pc = t
            .stack
            .as_ref()
            .map(|s| crate::arch::exceptions::user_pc(s.top()))
            .unwrap_or(0);
        // The address-space root, which is what tells threads of DIFFERENT PROCESSES apart. Without
        // it a `pc` in this dump is close to useless for a userspace hang: every user program is
        // linked at the same base (`address_space_map::IMAGE_BASE`), so a bare PC resolves plausibly against several
        // binaries at once and invites exactly the wrong conclusion. That is not hypothetical, it
        // cost an hour on 2026-07-30: three spinning threads read equally well as three FS servers
        // in RedoxFS directory code or as one std client looping in `read_to_end`, and the PCs alone
        // could not say which. Threads sharing a root are one process; distinct roots are distinct
        // processes, and 0 is a kernel thread with no user address space.
        let root = t.space.as_ref().map(|s| s.root()).unwrap_or(0);
        crate::print!(
            "  tid={:#06x} state={:?} on_cpu={} wake_pending={} has_outgoing_cap={} pc={:#010x}{} address_space={:#010x}",
            t.id,
            t.handshake.state,
            t.handshake.on_cpu,
            t.handshake.wake_pending,
            t.outgoing_cap.is_some(),
            pc,
            if t.handshake.on_cpu { "*" } else { "" },
            root,
        );
        // The wait reason, written by the same IPC_TABLES-held statement that wrote `Blocked`. A
        // `Blocked` thread with `wait=-` here is the smoking gun for a state byte written outside
        // the block paths (corruption, or a block applied to the wrong TCB): every legal block
        // records what it waits on. See notes/visionfive2.md, fourth bench stop.
        match t.handshake.wait_on {
            Some(Wait::Rendezvous(ep, role)) => crate::println!(" wait={ep:#x}/{role:?}"),
            Some(Wait::Notification(n)) => crate::println!(" wait={n:#x}/Notification"),
            None => crate::println!(" wait=-"),
        }
    }
    // Rendezvous topology: which rendezvous each blocked thread is queued on, so a deadlock shows as a
    // sender with no receiver. Diagnostic only.
    for (name, &phys) in sched.rendezvous_table.iter() {
        // SAFETY: a live rendezvous page, direct-mapped, under IPC_TABLES.
        let ep = unsafe { &*(crate::arch::mmu::phys_to_virt(phys) as *const Rendezvous) };
        let (ns, nr, np) = ep.debug_counts();
        if ns != 0 || nr != 0 || np != 0 {
            crate::println!("  ep={name:#06x} senders={ns} receivers={nr} pending={np}");
        }
    }
    // And each notification with a waiter or a word pending (milestone 151): a thread asleep beside
    // a non-zero word is the notification twin of a sender with no receiver.
    for (name, &phys) in sched.notification_table.iter() {
        // SAFETY: a live notification page, direct-mapped, under IPC_TABLES.
        let n = unsafe { &*(crate::arch::mmu::phys_to_virt(phys) as *const NotificationPage) };
        let (waiters, word) = n.state.debug_counts();
        if waiters != 0 || word != 0 {
            crate::println!(
                "  notification={name:#06x} waiters={waiters} word={word:#x} bound={:?}",
                n.bound
            );
        }
    }
    // The online set, not `0..count` (first-silicon sweep, 2026-08-14): on the VisionFive 2 the
    // count-as-index loop printed parked slot 0 as if it were a live core and hid online core 3.
    for c in crate::smp::online_cpus() {
        let pc = cpu::of(c);
        let inbox_len = pc.inbox.lock().len();
        crate::println!(
            "  core {c}: current={:#06x} idle={:#06x} switched_from={:#06x} need_resched={} inbox_len={} ticks={} steal_req={:?}",
            pc.current.load(Ordering::Relaxed),
            pc.idle.load(Ordering::Relaxed),
            pc.switched_from.load(Ordering::Relaxed),
            pc.need_resched.load(Ordering::Relaxed),
            inbox_len,
            // A core whose tick count holds still between dumps is taking no timer traps: it is
            // spinning with interrupts masked, or parked inside an SBI call in M-mode (a remote
            // fence that never completes looks exactly like this). The one field that tells a
            // wedged core from a scheduler that merely chose not to run somebody.
            crate::arch::timer::ticks_on(c),
            // A steal request that stays claimed dump after dump means the victim never reached
            // a scheduler entry to serve it: the same wedge, seen from a thief's side.
            pc.steal_request.peek(),
        );
        trace::dump(c);
    }
    // A parked slot with a non-empty inbox is a thread nothing will ever run: the exact shape of
    // the VisionFive 2 placement hang (init modulo-counted into slot 0's inbox; that dead inbox in
    // this dump was the clue). Since the online-set sweep no path should produce it, so if this
    // prints, something is picking cpus by count again.
    let online = crate::smp::online_harts_mask();
    for c in (0..crate::cpu::MAX_CPUS).filter(|c| online & (1 << c) == 0) {
        let inbox_len = cpu::of(c).inbox.lock().len();
        if inbox_len != 0 {
            crate::println!(
                "  core {c}: PARKED with inbox_len={inbox_len} (placed on a dead core)"
            );
        }
    }
    crate::println!("--- end thread dump ---");
}

/// **How many senders are parked on an rendezvous.** Test support (milestone 22 phase B.2).
///
/// A negative assertion ("the supervisor sent nothing more") cannot be made with `RECEIVE`, which would
/// block forever on a quiet rendezvous. This is the non-blocking look that lets a test say "and then
/// nothing happened" instead of hanging when the code is right.
#[cfg(feature = "system_tests")]
pub fn rendezvous_waiting_senders(ep: RendezvousId) -> usize {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return 0;
    };
    match rendezvous_of(sched, ep) {
        Some(e) => e.debug_counts().0,
        None => 0,
    }
}

/// **What a death actually did to a thread**: its run state, the supervision rendezvous it was
/// spawned with, and the wait it is recorded as being in. `None` if the thread is no longer in the
/// table at all. Test support (milestone 321). **Provisional name.**
///
/// It exists because a corpse that does not reach its supervision rendezvous fails
/// `rendezvous_waiting_senders(ep) == 1` in a way a transcript cannot explain, and the failure
/// observed on xenon (2026-09-17) was on a bench nobody can re-run with a print added. The three
/// possible histories leave three different marks here, so one line in an assertion message
/// separates them:
///
///   - `Finished` with no `fault_ep`: [`depart`] took the *unsupervised* path, so the supervision
///     endpoint was lost before the fault rather than the delivery failing.
///   - `Dead` with `wait_on == Some((ep, Sender))`: the corpse did park, and something took it off
///     the queue again.
///   - `Dead` with `wait_on == None`: [`deliver_death`]'s `send` met a waiting receiver, so the
///     message was handed over and the corpse never joined the sender queue.
///   - `Ready`, `Running` or `Blocked`: it never departed, whatever was printed about it.
///   - `None`: the thread was reaped and freed.
///
/// This is milestone 318's lesson about assertions against constants, carried one step past the
/// count: knowing the count is 0 still leaves three readings, and this is what decides between them
/// on a machine the person reading the log cannot touch.
/// The three fields [`thread_death_disposition`] reports. **Provisional name.**
///
/// A struct rather than the tuple this started as, and clippy's `type_complexity` asking for the
/// change is the smaller half of the reason. The larger one is that this type exists to be read
/// **in a panic message**, through `{:?}`, by somebody holding a serial log from a machine they
/// cannot touch. A three-field `Debug` names its fields and a three-element tuple does not, so the
/// difference is between `Some((Dead, Some(4), Some((4, Sender))))` and a line that says which of
/// those numbers is the endpoint.
/// **Every field here is read through `Debug` and nowhere else, which the dead-code lint cannot
/// see**: a derived `impl` does not count as a use, so without this the three fields that are the
/// whole point of the type report as never read. The allow is the exception and this is it saying
/// so. It is also the tell that the type is doing one job: if a field ever gets read by code, the
/// allow should shrink rather than stay.
#[cfg(any(test, feature = "system_tests"))]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct ThreadDeathDisposition {
    /// Where the scheduler thinks this thread is.
    pub state: State,
    /// The supervision rendezvous it was started with. `None` on a corpse means [`depart`] took the
    /// unsupervised path, so supervision was lost before the fault rather than in delivery.
    pub fault_ep: Option<RendezvousId>,
    /// The queue it is recorded as parked on, and in which role.
    pub wait_on: Option<Wait>,
}

#[cfg(feature = "system_tests")]
pub fn thread_death_disposition(tid: ThreadId) -> Option<ThreadDeathDisposition> {
    let mut guard = IPC_TABLES.lock();
    let sched = guard.as_mut()?;
    let t = sched.threads.get(tid)?;
    Some(ThreadDeathDisposition {
        state: t.handshake.state,
        fault_ep: t.fault_ep,
        wait_on: t.handshake.wait_on,
    })
}

/// **How many receivers are parked on an rendezvous.** The twin of [`rendezvous_waiting_senders`], and
/// test support for the same reason (milestone 81).
///
/// A test that wants to act *on* a blocked waiter needs to know the waiter is blocked, and "I
/// yielded, so it must have run" is not that knowledge: since DECISIONS §28 the waiter is placed on
/// another core, and on the physical core under HVF a yield on this one returns in nanoseconds. So
/// the wait has to be on the queue itself, which is what this reads.
#[cfg(any(test, feature = "system_tests"))]
pub fn rendezvous_waiting_receivers(ep: RendezvousId) -> usize {
    let mut guard = IPC_TABLES.lock();
    let Some(sched) = guard.as_mut() else {
        return 0;
    };
    match rendezvous_of(sched, ep) {
        Some(e) => e.debug_counts().1,
        None => 0,
    }
}

/// **Test support: a wake with nothing delivered** (the boot-8 injector, 2026-08-14).
///
/// Issues a bare `wake()` against `tid` under `IPC_TABLES`, through the same function every scheduler
/// wake site funnels into, with no message written, no signal counted, and no abort flagged: the
/// transition the VisionFive 2's boot-8 event ring recorded against the boot thread (`wake:0x0`
/// on a boot where no sender to its rendezvous existed). This is deliberately not a hand-rolled
/// state poke: it exercises the real wake path, so whatever `wake()` does about an undelivered
/// wake is what this injects.
#[cfg(any(test, feature = "system_tests"))]
pub fn wake_without_delivery(tid: ThreadId) {
    let mut guard = IPC_TABLES.lock();
    if let Some(sched) = guard.as_mut() {
        wake(sched, tid);
    }
}

/// **The number that says preemption is real**, read by the preemption tests and printed by the
/// milestone tour.
///
/// The alternate boot modes (`shell`, `bench`) each compile the tour out and run no tests, so in
/// those two configurations this genuinely has no caller. That is a property of the boot mode, not
/// evidence the counter is dead, which is why the allow is conditioned on exactly those features
/// rather than written unconditionally.
#[cfg_attr(any(feature = "shell", feature = "bench"), allow(dead_code))]
pub fn preemptions() -> u64 {
    PREEMPTIONS.load(Ordering::Relaxed)
}

pub fn count_preemption() {
    PREEMPTIONS.fetch_add(1, Ordering::Relaxed);
    PREEMPTIONS_PER_CPU[cpu::id()].fetch_add(1, Ordering::Relaxed);
}

/// Preemptions taken **on this core**, which is the question the global counter cannot answer.
///
/// A window with interrupts masked takes none of these however long it lasts, and that property is
/// what `kernel/src/bench.rs`'s `map_new` is built on.
/// See milestone 541 (a timed window that excludes preemption), and
/// `kernel/src/preemption_window_tests.rs`.
///
/// **Its two callers are both conditional**, which is why the allow is the inverse shape of
/// `preemptions()`'s above: that one has a caller in the ordinary build (the milestone tour) and
/// loses it under `shell` and `bench`, while this one has callers only under `test` and `bench`
/// and is genuinely dead in the build that ships. The counter it reads is written unconditionally,
/// so no configuration can make the number stale.
#[cfg_attr(
    not(any(test, feature = "system_tests", feature = "bench")),
    allow(dead_code)
)]
pub fn preemptions_here() -> u64 {
    preemptions_on(cpu::id())
}

/// Preemptions taken on **a named** core, for a reader that must not follow a migrating thread.
///
/// [`preemptions_here`] reads whichever core is running *now*, which is the wrong counter for any
/// observation that straddles a preemption: the preemption is exactly what may move the reader.
/// `kernel/src/preemption_window_tests.rs` samples one core across an unmask for that reason.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn preemptions_on(id: usize) -> u64 {
    PREEMPTIONS_PER_CPU[id].load(Ordering::Relaxed)
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
pub fn preempt_if_needed() {
    if take_need_resched() && is_running() {
        count_preemption();
        schedule();
    }
}

pub fn is_running() -> bool {
    IPC_TABLES.lock().is_some()
}

#[cfg(test)]
mod tests {
    //! Tests for threads, the context switch, and preemption.
    //!
    //! `a_thread_that_never_yields_is_preempted_anyway` is the one this whole project has been
    //! arguing about since DECISIONS §5. Everything else here is scaffolding for it.

    use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    /// **A running thread cannot grant itself the cycle counter** (milestone 229, DECISIONS 139
    /// part 2). The grant is a field in the thread's spawn manifest, and this is the mechanism that
    /// makes that sentence true rather than descriptive: `grant_cycle_counter` refuses anything
    /// that is not an `Embryo`, so the only window in which the bit can be set is before the thread
    /// has ever run.
    ///
    /// It asks on behalf of the caller, which is the strongest available form of the question: this
    /// thread is `Running` by definition of executing this line, and it holds every authority a
    /// kernel thread has. `WrongObject` is the same refusal `CONFIGURE` and `CAP_INSERT` make on a
    /// started thread, reused rather than a new error minted for a new way of being too late.
    #[test_case]
    fn a_running_thread_cannot_be_granted_the_cycle_counter() {
        let me = super::current_thread_id();
        assert_eq!(
            super::grant_cycle_counter(me),
            Err(abi::Error::WrongObject),
            "a live thread was allowed to acquire a timing instrument it was not created with",
        );
    }

    /// Wait for `cond`, bounded by the CLOCK rather than by a yield count.
    ///
    /// These tests used to spin a fixed number of yields and then assert. A yield count is not a
    /// duration: on a loaded host, or once §28's placement scattered work across cores, this core can
    /// burn two hundred cheap yields long before the threads it is waiting on have been scheduled at
    /// all. That is not a hang, it is an impatient observer, and it fails an assertion that describes
    /// the system rather than the test.
    ///
    /// It has now bitten three times in one day, in three different files: the reap waits in
    /// `user.rs`, three spin counts in `smp.rs`, and here. The third time was a gate run failing with
    /// "finished threads were never reaped, left: 11, right: 5" while a leaked QEMU held 199% of the
    /// host, which is exactly the condition a yield count cannot survive and a clock can.
    ///
    /// Two seconds is far beyond any honest completion here (these are milliseconds when the machine
    /// is quiet) and well inside the harness's 90 s per-test ceiling, which remains the backstop for a
    /// genuine hang. Still a leak trap rather than a masked failure: work that never completes times
    /// out, and the caller's assertion then reports what was actually wrong.
    fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
        let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
        while crate::arch::timer::now() < deadline {
            if cond() {
                return true;
            }
            crate::sched::yield_now();
        }
        cond()
    }

    /// Wait for `cond` **without yielding**, budgeted in timer ticks delivered to this core rather
    /// than in wall-clock time.
    ///
    /// For `a_thread_that_never_yields_is_preempted_anyway`, which must not yield (that is the
    /// whole point of it) and is waiting for a *preemption*. A tick is when a preemption can
    /// happen, so the number of preemption opportunities that went by is what the claim is about.
    /// Why that unit survives a contended host where a `timer::now()` deadline does not is
    /// [`crate::testing::TickBudget`]'s argument, and the re-anchoring on migration is its
    /// mechanism; here the migration is additionally the news this test is waiting for.
    ///
    /// If ticks stop arriving altogether this does not return, and the harness's 90 s per-test
    /// ceiling is the backstop: a timer that is not delivering is the arch timer tests' failure to
    /// report, not this one's.
    fn within_ticks(budget: u64, mut cond: impl FnMut() -> bool) -> bool {
        let mut budget = crate::testing::TickBudget::new(budget);
        loop {
            if cond() {
                return true;
            }
            if budget.is_expired() {
                return cond();
            }
            core::hint::spin_loop();
        }
    }

    /// Spin the scheduler until `cond`, bounded by wall-clock, returning whether it happened. Since
    /// DECISIONS §28, work a test spawns runs on *other* cores, so this core is often idle and a
    /// yield returns at once: a fixed count of yields elapses in almost no real time and times out
    /// before the parallel result lands. A ~2 s deadline gives the other cores real time while
    /// staying far under the 60 s hang watchdog, so a genuine lost wakeup still fails.
    fn spin_until(mut cond: impl FnMut() -> bool) -> bool {
        let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
        while crate::arch::timer::now() < deadline {
            if cond() {
                return true;
            }
            super::yield_now();
        }
        cond()
    }

    // --- Raising an interrupt on purpose, on each ISA ---------------------------------------
    //
    // The two delivery tests below (`an_interrupt_becomes_a_message` and
    // `an_interrupt_that_arrives_before_the_wait_is_not_lost`) are portable: the property they check
    // is that the kernel turns an interrupt into a message and does not lose one that arrives early,
    // and neither of those is architectural. Only the *trigger* is, and it is genuinely asymmetric,
    // so it lives here in three small functions rather than in a comment claiming parity.
    //
    // aarch64 has a software-generated interrupt (an SGI), so it needs no device whatsoever.
    //
    // RISC-V has nothing of the kind. `sip.SEIP` is read-only to S-mode (only the PLIC, driven by a
    // real wire, sets it), the PLIC's pending block is read-only by specification, and the one
    // interrupt S-mode can raise on itself is the SBI's IPI, which arrives as a *software* interrupt
    // (`scause` = 1) down a different arm of `riscv_trap_body` than a device's, touching neither
    // `irq_route` nor `irq_notify`. Using it would have looked like parity and proved nothing.
    //
    // So RISC-V uses the smallest real interrupt line it can assert by hand: the console UART's own
    // transmit-empty interrupt, which a 16550 raises the instant it is enabled, because the
    // transmitter of a polling console is always empty. No transfer, no external stimulus, nothing
    // to read back, and it is 16550 architecture rather than a QEMU behaviour, so it should carry to
    // a real part. See `console::raise_uart_interrupt` and notes/interrupts.md.

    /// The interrupt `an_interrupt_becomes_a_message` raises.
    #[cfg(target_arch = "aarch64")]
    fn delivery_irq() -> u32 {
        1 // an SGI: software-triggerable, no hardware behind it
    }
    /// The interrupt `an_interrupt_that_arrives_before_the_wait_is_not_lost` raises. A different SGI
    /// from `delivery_irq` so the two tests cannot see each other's routes.
    #[cfg(target_arch = "aarch64")]
    fn pending_irq() -> u32 {
        2
    }

    /// The NS16550's PLIC source on QEMU `virt`. A board constant, hardcoded identically on
    /// main.rs's boot-tour and shell paths; another board would give its UART a different number,
    /// and this is one of the places that would have to learn it from the device tree.
    ///
    /// On the VisionFive 2 the boot summary reads `uart irq : source 32 (device tree)`, so
    /// `DELIVERY_IRQ = 10` causes `raise_uart_interrupt` on IRQ 10 to never reach the thread
    /// waiting on the route bound to the real IRQ 32. The test now reads the DTB-driven value the
    /// rest of the kernel already uses (`user::uart_irq_and_source`); on QEMU `virt` that gives 10
    /// (via the fallback `UART_RX_INTID`), on the VF2 it gives 32.
    #[cfg(target_arch = "riscv64")]
    fn delivery_irq() -> u32 {
        crate::user::uart_irq_and_source().0
    }
    /// **The same source, deliberately.** RISC-V has exactly one line these tests can assert by
    /// hand, so unlike aarch64's two SGIs the two tests share it. They do not collide: each rebinds
    /// the route to its own rendezvous before raising, and each quiets the line before it returns.
    #[cfg(target_arch = "riscv64")]
    fn pending_irq() -> u32 {
        crate::user::uart_irq_and_source().0
    }

    /// **x86 is aarch64's case, not RISC-V's**, and this is the third answer to the question those
    /// two comments have been circling. The local APIC will deliver any vector to its own CPU on
    /// demand, through the ICR and a real delivery path (the IRR, the ISR, an EOI), so this ISA
    /// needs no device to raise an interrupt by hand and gets two independent sources rather than
    /// one shared line. The number **is the vector**, because a local APIC source has no controller
    /// input to name; see `arch::x86_64::exceptions::x86_trap_body`.
    #[cfg(target_arch = "x86_64")]
    fn delivery_irq() -> u32 {
        crate::arch::irq::SELF_TEST_VECTOR as u32
    }
    /// A second vector, so the two tests cannot see each other's routes. aarch64's two SGIs.
    #[cfg(target_arch = "x86_64")]
    fn pending_irq() -> u32 {
        crate::arch::irq::SELF_TEST_VECTOR_B as u32
    }

    /// Enable the test interrupt at the controller. Nothing is raised yet.
    #[cfg(target_arch = "aarch64")]
    fn arm_test_irq(intid: u32) {
        crate::arch::irq::enable(intid); // SGI: per-core, no target
    }

    #[cfg(target_arch = "riscv64")]
    fn arm_test_irq(intid: u32) {
        // The affinity policy picks (and remembers) which hart's PLIC context this source lands on,
        // exactly as it does for a real driver's line. The handler masks the source when it fires,
        // so this also re-enables it for the second of the two tests.
        crate::arch::irq::enable(intid);
    }

    /// **Nothing to arm.** An IPI is not a line: it has no mask bit at any controller, because
    /// nothing outside the CPU asserts it. `irq::enable` here takes a *legacy IRQ* and writes an IO
    /// APIC redirection entry, which is the wrong device entirely for this source.
    #[cfg(target_arch = "x86_64")]
    fn arm_test_irq(intid: u32) {
        let _ = intid;
    }

    /// Raise it.
    #[cfg(target_arch = "aarch64")]
    fn raise_test_irq(intid: u32) {
        // Self, by asking rather than by assuming core 0: the test thread runs wherever the
        // scheduler put it, and a fixed target is the count-as-index disease in miniature.
        crate::arch::irq::send_sgi(intid, crate::cpu::id());
    }

    #[cfg(target_arch = "riscv64")]
    fn raise_test_irq(intid: u32) {
        // The console UART's line is the only one this ISA can assert by hand, so a caller naming
        // any other source would silently raise the wrong interrupt and then wait for one that
        // never came, which reads as a kernel bug rather than a test bug.
        debug_assert_eq!(
            intid,
            delivery_irq(),
            "riscv can only raise the console UART's own line by hand"
        );
        crate::console::raise_uart_interrupt();
    }

    #[cfg(target_arch = "x86_64")]
    fn raise_test_irq(intid: u32) {
        debug_assert!(
            intid == delivery_irq() || intid == pending_irq(),
            "only the two self-IPI test vectors are raisable this way; {intid} is not one"
        );
        crate::arch::irq::raise_self_interrupt(intid as u8);
    }

    /// Lower it again, so the next test starts from a quiet line.
    #[cfg(target_arch = "aarch64")]
    fn quiet_test_irq() {
        // An SGI is edge-triggered and one-shot: there is no line to lower.
    }

    #[cfg(target_arch = "riscv64")]
    fn quiet_test_irq() {
        crate::console::quiet_uart_interrupt();
    }

    /// An IPI is edge-delivered and one-shot: there is no asserted line to lower, which is aarch64's
    /// SGI case rather than RISC-V's held UART line.
    #[cfg(target_arch = "x86_64")]
    fn quiet_test_irq() {}

    /// A spawned thread actually runs, and its closure's captured state comes with it.
    #[test_case]
    fn a_spawned_thread_runs() {
        static RAN: AtomicBool = AtomicBool::new(false);
        static SAW: AtomicU64 = AtomicU64::new(0);

        let captured = 0xdead_beefu64;
        crate::sched::spawn(move || {
            SAW.store(captured, Ordering::SeqCst);
            RAN.store(true, Ordering::SeqCst);
        })
        .expect("spawn failed");

        // Wait until it has had a turn (on this or another core, since §28).
        spin_until(|| RAN.load(Ordering::SeqCst));

        assert!(RAN.load(Ordering::SeqCst), "the thread never ran");
        assert_eq!(
            SAW.load(Ordering::SeqCst),
            0xdead_beef,
            "the closure's captured value did not survive the switch"
        );
    }

    /// **Object revocation reclaims a region holding an unstarted TCB** (the smallest proof of the
    /// mechanism). Retype a bare embryo into a fresh region, then `reclaim_region`: the TCB is torn
    /// down (its table slot freed, its generational name dead), the region's memory returns, and the
    /// free-frame count lands exactly where it began. No scheduler run, no address space, no reaper
    /// timing: find the object, kill it, unpin, free. The larger cases (a started-then-exited
    /// thread, its address space, the spawn-to-reap loop) build on this one.
    #[test_case]
    fn reclaim_frees_an_embryo_thread_control_blocks_region() {
        let region = crate::memory_region::create(2).expect("a fresh 2-page region");
        // The region's own frames, by name. A machine-wide free count would have this test
        // asserting that nothing else in the kernel allocated or freed while it ran; these two
        // frames are the property itself. See `testing::RegionRun`.
        let run = crate::testing::RegionRun::of(region);
        let tid = crate::sched::create_thread_control_block(region)
            .expect("retype a TCB from the region");

        // The embryo is named, not counted. This pair used to bracket `thread_count()` against a
        // baseline taken above (`threads_before + 1`, then `threads_before`), which is the reaper
        // count's defect in a different test: the headcount is the size of the whole table, so a
        // neighbouring thread exiting between the two reads lands the count BELOW what the
        // assertion demands and blames this embryo for it. `is_thread_present` on the ThreadId this
        // test created is immune by construction, and it is strictly the stronger claim: the old
        // "the TCB's table slot must be freed" could pass with the embryo still in the table, as
        // long as somebody else's thread left in the same window. Fourth appearance of this fix;
        // see notes/load-sensitive-assertions.md and `is_thread_present`'s own doc comment.
        assert!(
            crate::sched::is_thread_present(tid),
            "the embryo should be in the table before reclaim"
        );
        run.assert_held("the live region's pages should still be spent");

        crate::sched::reclaim_region(region)
            .expect("reclaim a region whose only object is an unstarted TCB");

        assert!(
            !crate::sched::is_thread_present(tid),
            "the TCB's table slot must be freed by reclaim"
        );
        run.assert_returned("reclaim must return every one of the region's own frames");
    }

    /// **Object revocation reclaims a region holding an unbound address space** (the address-space
    /// case of piece 1's mechanism). Create a space in its own region, not bound to any TCB, then
    /// reclaim: the space is torn down (its name goes stale, its ASID is freed by `Drop`) and the
    /// region's memory returns exactly to baseline. This is what retires the "an unbound space
    /// leaks" note the registry carried since 19b.
    #[test_case]
    fn reclaim_frees_an_unbound_address_spaces_region() {
        let region = crate::memory_region::create(8).expect("a fresh region");
        let run = crate::testing::RegionRun::of(region); // this region's frames, not the machine's
        let name = crate::user::user_address_space_create(region)
            .expect("an address space from the region");

        assert!(
            crate::user::user_address_space_root(name).is_some(),
            "the space should resolve before reclaim"
        );
        run.assert_held("the live region's pages should still be spent");

        crate::sched::reclaim_region(region).expect("reclaim the space's own region");

        assert!(
            crate::user::user_address_space_root(name).is_none(),
            "the space's name must be stale after reclaim"
        );
        run.assert_returned("reclaim must return every one of the region's own frames");
    }

    /// **`MemoryRegion` SPLIT returns a child's pages to the parent on reclaim (LIFO), so a split parent is
    /// not committed for its lifetime.** Carve a region into two children; the parent refuses reclaim
    /// while they live. Reclaiming a child out of order (not the top of the watermark) leaves a hole;
    /// reclaiming the top child un-bumps the parent, so its budget is re-splittable. Either way a
    /// child's pages go back to the *parent*, not the allocator, so the free-frame count does not move
    /// until the parent itself, now childless, is destroyed.
    #[test_case]
    fn split_returns_child_pages_to_the_parent() {
        let parent = crate::memory_region::create(8).expect("parent region");
        // The parent is a root, so its eight frames are the allocator's own bits: `of` asserting
        // they are used is "create spent the parent's pages", and it names which pages rather than
        // counting the machine's. A child's pages never reach the allocator at all, which is what
        // the `assert_held` calls below say.
        let run = crate::testing::RegionRun::of(parent);

        let child_a = crate::memory_region::split(parent, 4).expect("split child a"); // [0,4)
        let child_b = crate::memory_region::split(parent, 4).expect("split child b"); // [4,8), the top
        assert_ne!(child_a, child_b);
        assert!(crate::memory_region::has_children(parent));
        assert!(
            crate::sched::reclaim_region(parent).is_err(),
            "a parent with live children refuses reclaim",
        );
        assert!(
            crate::memory_region::split(parent, 1).is_none(),
            "a fully-carved parent cannot split further",
        );

        // Reclaim out of order (child_a is not the top): a hole, its pages returned to the parent,
        // nothing to the allocator.
        crate::sched::reclaim_region(child_a).expect("reclaim child a (leaves a hole)");
        run.assert_held("a reclaimed child returns pages to the parent, not the allocator");
        assert!(
            crate::memory_region::has_children(parent),
            "one child still lives"
        );

        // Reclaim the top child: the parent un-bumps and is childless, and its budget re-splits.
        crate::sched::reclaim_region(child_b).expect("reclaim child b (the LIFO top)");
        assert!(
            !crate::memory_region::has_children(parent),
            "no children remain"
        );
        let child_c =
            crate::memory_region::split(parent, 4).expect("the LIFO-returned pages re-split");
        crate::sched::reclaim_region(child_c).expect("reclaim child c");

        // Nothing reached the allocator until now: destroying the childless root parent frees the
        // whole run, the hole included, exactly once.
        crate::sched::reclaim_region(parent).expect("destroy the now-childless root parent");
        run.assert_returned("the root parent's pages return to the allocator");
    }

    /// **A destroyed region's table slot is reused** (generational regions). Create and destroy a
    /// region far more times than the table has slots: without reuse the 257th `create` would fail
    /// with the table full, the lifetime cap that made a long-running system untenable. With reuse
    /// each `destroy` frees the slot, so one free slot serves the whole loop, and the free-frame
    /// count nets to zero every iteration. This is the property that lets the kernel run workloads
    /// that come and go without end.
    #[test_case]
    fn destroyed_region_slots_are_reused() {
        // Comfortably more than MAX_REGIONS (256): without reuse this exhausts the table well before
        // the end. With reuse, one freed slot serves every iteration.
        //
        // The frame half of the claim is asked per iteration and about that iteration's own page,
        // which is both narrower than the old machine-wide delta and stricter: a leak of one page
        // in one round fails on the round that leaked it, rather than being netted out by another
        // test freeing a page somewhere in the 320.
        for _ in 0..320 {
            let r = crate::memory_region::create(1)
                .expect("a region slot must be reused, not exhausted");
            let run = crate::testing::RegionRun::of(r);
            crate::memory_region::destroy(r);
            // No round number in the message: the frame's own address is in it, and that says
            // which iteration far more usefully than a counter would.
            run.assert_returned("destroying a region did not return its page");
        }
    }

    /// **Object revocation reclaims a region holding an idle rendezvous.** An rendezvous nobody is
    /// blocked on is torn down with its region: removed from the registry (its name goes stale, so
    /// every Rendezvous capability to it fails), and its page returned. Frames back to baseline.
    #[test_case]
    fn reclaim_frees_a_regions_idle_rendezvous() {
        let region = crate::memory_region::create(2).expect("region");
        let run = crate::testing::RegionRun::of(region); // this region's frames, not the machine's
        let _ep = crate::sched::create_rendezvous_from(region).expect("rendezvous from region");
        run.assert_held("the live region's pages should still be spent");
        crate::sched::reclaim_region(region)
            .expect("reclaim a region with only an idle rendezvous");
        run.assert_returned("the idle rendezvous's region must give back every frame it held");
    }

    /// **A thread blocked on an rendezvous wakes with an error when the rendezvous is revoked.** Rather
    /// than refuse the reclaim (the old safe subset) or strand the waiter, revocation drains the
    /// rendezvous's wait queue, marks each waiter aborted, and wakes it: the reclaim *succeeds*, and the
    /// woken thread's blocking IPC reports the rendezvous is gone (`take_ipc_aborted`) instead of
    /// returning a message it never received. This is the richer semantic, folded into the IPC core.
    #[test_case]
    fn a_blocked_waiter_wakes_with_an_error_when_its_rendezvous_is_revoked() {
        static ABORTED: AtomicBool = AtomicBool::new(false);
        static WOKE: AtomicBool = AtomicBool::new(false);
        ABORTED.store(false, Ordering::SeqCst);
        WOKE.store(false, Ordering::SeqCst);

        let region = crate::memory_region::create(2).expect("region");
        let ep = crate::sched::create_rendezvous_from(region).expect("rendezvous from region");

        // A thread that blocks receiving on the rendezvous, then records whether it was aborted.
        crate::sched::spawn(move || {
            let _ = crate::sched::ipc_receive(ep);
            ABORTED.store(crate::sched::take_ipc_aborted(), Ordering::SeqCst);
            WOKE.store(true, Ordering::SeqCst);
        })
        .expect("spawn a waiter");

        // **The waiter must be queued on the rendezvous before the reclaim**, or there is nothing to
        // wake and the test passes on a fiction. This used to be one `yield_now()`, on the premise
        // (written when the machine was single core, stale since DECISIONS §28 scattered placement)
        // that yielding hands this core to the waiter. It does not: the waiter is on another core,
        // and a yield here only says *this* core had nothing else to do.
        //
        // Milestone 81 is where that came due. Under TCG the round-robin between vCPUs made one
        // yield enough often enough to look deliberate; on the physical core under HVF the four
        // vCPUs are four host threads running at once, this core's yield returns in nanoseconds,
        // and the reclaim ran before the waiter had ever been scheduled ("the revoked waiter never
        // woke"). Same defect the milestone-78 family had, found by a *faster* machine rather than
        // a loaded one: a yield count is not a duration in either direction.
        assert!(
            wait_for(|| crate::sched::rendezvous_waiting_receivers(ep) == 1),
            "the waiter never blocked on the rendezvous, so the reclaim had nothing to wake",
        );

        // Reclaiming the rendezvous's region now succeeds: the waiter is woken with an error, not left
        // to strand the reclaim.
        crate::sched::reclaim_region(region)
            .expect("reclaim wakes the blocked waiter rather than refusing");

        // Clock-bounded, not yield-bounded: see `wait_for`. Since §28 the waiter is on another
        // core, so this core's fifty yields can elapse before it has been scheduled at all.
        assert!(
            wait_for(|| WOKE.load(Ordering::SeqCst)),
            "the revoked waiter never woke"
        );
        assert!(
            ABORTED.load(Ordering::SeqCst),
            "the woken waiter did not see its IPC aborted",
        );
    }

    /// How many threads are parked awaiting a reply *through* `ep`. Test support for milestone
    /// 254, and it cannot be asked of the rendezvous itself, which is the entire defect: a `CALL`
    /// caller whose request was taken is linked on no queue, so `debug_counts` cannot see it and
    /// only `wait_on` still records that it is waiting.
    fn reply_parked_callers(ep: super::RendezvousId) -> usize {
        let guard = super::IPC_TABLES.lock();
        let Some(sched) = guard.as_ref() else {
            return 0;
        };
        sched
            .threads
            .iter_from(0)
            .filter(|(_, t)| {
                matches!(t.handshake.wait_on, Some(super::Wait::Rendezvous(on, super::WaitRole::Reply)) if on == ep)
            })
            .count()
    }

    /// How many live `Reply` capabilities anywhere in the machine still name `caller`: the sweep's
    /// own assertion (milestone 254). `outgoing_cap` counts too, because a caller that met no
    /// server rides its own reply capability there awaiting a `RECEIVE_CAP` hand-off, and a live one
    /// left in that slot is the same forgery one step earlier.
    fn outstanding_reply_capabilities(caller: super::ThreadId) -> usize {
        let guard = super::IPC_TABLES.lock();
        let Some(sched) = guard.as_ref() else {
            return 0;
        };
        let target = crate::cap::Object::Reply(caller);
        let mut found = 0;
        for (_, t) in sched.threads.iter_from(0) {
            let table = sched.threads.capabilities(t.id).unwrap().lock();
            for slot in 0..table.len() as u64 {
                if table.get(slot).is_ok_and(|c| c.object == target) {
                    found += 1;
                }
            }
            drop(table);
            if matches!(t.outgoing_cap, Some(c) if c.object == target) {
                found += 1;
            }
        }
        found
    }

    /// **A caller whose rendezvous is torn down mid-`CALL` returns `Gone` rather than blocking for
    /// the life of the machine** (milestone 254), and **the stale reply capability the server was
    /// still holding is gone before the caller can run again.**
    ///
    /// This test hung before milestone 254. `Error::Gone` reached a rendezvous's *wait queues*, and
    /// the server here has already collected the request, so the caller left those queues at the
    /// rendezvous: `drain_waiters` walked straight past it and `ipc_reply` was the only thing left
    /// that could wake it. `a_blocked_waiter_wakes_with_an_error_when_its_rendezvous_is_revoked`,
    /// two tests up, is the case that always worked, and the difference between them is exactly one
    /// collected message.
    ///
    /// **The second assertion is the one that matters**, and a version of this test carrying only
    /// the first would pass while the kernel was strictly worse than before it. Waking a
    /// reply-parked caller is what lets it enter a second `CALL` with an unconsumed `Reply` still
    /// naming it, and `Object::Reply` carries a generational thread name with no call identity, so
    /// the server's held capability would answer the *next* conversation. `current_cap` is the same
    /// lookup `abi::reply::REPLY` does before it reaches `ipc_reply`, so a capability that is gone
    /// here is a reply that cannot be sent at all.
    #[test_case]
    fn a_reply_parked_caller_wakes_with_an_error_when_its_rendezvous_is_reclaimed() {
        static COLLECTED: AtomicBool = AtomicBool::new(false);
        static RELEASE: AtomicBool = AtomicBool::new(false);
        static HELD_BEFORE: AtomicBool = AtomicBool::new(false);
        static HELD_AFTER: AtomicBool = AtomicBool::new(true);
        static CHECKED: AtomicBool = AtomicBool::new(false);
        static WOKE: AtomicBool = AtomicBool::new(false);
        static ABORTED: AtomicBool = AtomicBool::new(false);
        static CALLER: AtomicU64 = AtomicU64::new(0);

        let region = crate::memory_region::create(2).expect("region");
        let ep = crate::sched::create_rendezvous_from(region).expect("rendezvous from region");

        // The server: collect the request, keep the one-shot Reply, and never answer. It stays
        // alive on purpose, so the only thing that can free the caller is the rendezvous going
        // away; a server that returned here would depart, which is the *other* trigger and the
        // next test's subject.
        crate::sched::spawn(move || {
            let m = crate::sched::ipc_receive_cap(ep);
            let slot = m[1];
            HELD_BEFORE.store(crate::sched::current_cap(slot).is_ok(), Ordering::SeqCst);
            COLLECTED.store(true, Ordering::SeqCst);
            while !RELEASE.load(Ordering::SeqCst) {
                crate::sched::yield_now();
            }
            HELD_AFTER.store(crate::sched::current_cap(slot).is_ok(), Ordering::SeqCst);
            CHECKED.store(true, Ordering::SeqCst);
        })
        .expect("spawn a server");

        crate::sched::spawn(move || {
            CALLER.store(super::current_thread_id(), Ordering::SeqCst);
            let _ = crate::sched::ipc_call(ep, [1, 2]);
            ABORTED.store(crate::sched::take_ipc_aborted(), Ordering::SeqCst);
            WOKE.store(true, Ordering::SeqCst);
        })
        .expect("spawn a caller");

        // Clock-bounded, never yield-counted: since DECISIONS §28 both threads are on other cores.
        // Both conditions matter. `COLLECTED` says the request was *taken*, which is what puts the
        // caller off every wait queue and is the whole premise; `reply_parked_callers` says it is
        // parked awaiting the reply that will never come.
        assert!(
            wait_for(|| COLLECTED.load(Ordering::SeqCst) && reply_parked_callers(ep) == 1),
            "the server never collected a request from a reply-parked caller",
        );
        assert!(
            HELD_BEFORE.load(Ordering::SeqCst),
            "the server was never handed a reply capability, so this test proves nothing",
        );

        crate::sched::reclaim_region(region)
            .expect("reclaim frees the reply-parked caller rather than refusing");

        assert!(
            wait_for(|| WOKE.load(Ordering::SeqCst)),
            "the stranded caller never woke: this is the defect milestone 254 fixes",
        );
        assert!(
            ABORTED.load(Ordering::SeqCst),
            "the freed caller did not see its CALL aborted, so it returns a reply it never got",
        );
        assert_eq!(
            outstanding_reply_capabilities(CALLER.load(Ordering::SeqCst)),
            0,
            "a Reply capability still names the freed caller: the next CALL can be forged",
        );

        RELEASE.store(true, Ordering::SeqCst);
        assert!(
            wait_for(|| CHECKED.load(Ordering::SeqCst)),
            "the server never re-checked its reply capability",
        );
        assert!(
            !HELD_AFTER.load(Ordering::SeqCst),
            "the server kept a live reply capability naming a caller that has moved on",
        );
    }

    /// **A server that dies frees the callers it will never answer** (milestone 254), which is
    /// QNX Neutrino's headline behaviour: *"if the server thread fails, exits, or disappears, the
    /// client thread becomes READY, with `MsgSend()` indicating an error"*. The trigger here is
    /// `depart` rather than the rendezvous teardown above, and the rendezvous outlives the server,
    /// so nothing else in the kernel is even looking at the caller.
    ///
    /// The reply capability dies with the table it lived in, so the sweep has nothing left to
    /// delete by the time the thread is gone; `outstanding_reply_capabilities` asserts that
    /// directly rather than assuming it, because the ordering (sweep, then wake) is what makes the
    /// claim true and an ordering is exactly the sort of thing a refactor loses.
    #[test_case]
    fn a_server_that_exits_frees_the_caller_it_never_answered() {
        static COLLECTED: AtomicBool = AtomicBool::new(false);
        static HELD: AtomicBool = AtomicBool::new(false);
        static WOKE: AtomicBool = AtomicBool::new(false);
        static ABORTED: AtomicBool = AtomicBool::new(false);
        static CALLER: AtomicU64 = AtomicU64::new(0);

        let region = crate::memory_region::create(2).expect("region");
        let ep = crate::sched::create_rendezvous_from(region).expect("rendezvous from region");

        // Collect the request, confirm the reply capability is live, and then simply return: the
        // server exits holding it, which is the case the survey found this kernel alone in
        // stranding.
        crate::sched::spawn(move || {
            let m = crate::sched::ipc_receive_cap(ep);
            HELD.store(crate::sched::current_cap(m[1]).is_ok(), Ordering::SeqCst);
            COLLECTED.store(true, Ordering::SeqCst);
        })
        .expect("spawn a server");

        crate::sched::spawn(move || {
            CALLER.store(super::current_thread_id(), Ordering::SeqCst);
            let _ = crate::sched::ipc_call(ep, [3, 4]);
            ABORTED.store(crate::sched::take_ipc_aborted(), Ordering::SeqCst);
            WOKE.store(true, Ordering::SeqCst);
        })
        .expect("spawn a caller");

        assert!(
            wait_for(|| COLLECTED.load(Ordering::SeqCst)),
            "the server never collected the request",
        );
        assert!(
            HELD.load(Ordering::SeqCst),
            "the server was never handed a reply capability, so this test proves nothing",
        );
        assert!(
            wait_for(|| WOKE.load(Ordering::SeqCst)),
            "the caller was still parked after its server exited",
        );
        assert!(
            ABORTED.load(Ordering::SeqCst),
            "the freed caller did not see its CALL aborted",
        );
        assert_eq!(
            outstanding_reply_capabilities(CALLER.load(Ordering::SeqCst)),
            0,
            "a Reply capability outlived the caller's CALL",
        );

        // The rendezvous outlived its server, which is the point of this case; put its region back.
        assert!(
            wait_for(|| crate::sched::reclaim_region(region).is_ok()),
            "the rendezvous's region never came back",
        );
    }

    /// **A wake with nothing delivered must not complete a parked receiver's `RECEIVE`** (boot 8,
    /// VisionFive 2, 2026-08-14). The bench dump's shape: the boot thread, parked in `ipc_receive`
    /// on the report rendezvous, took a `wake:0x0` on a boot where no sender to that rendezvous
    /// existed, and its receive neither completed with a message nor re-parked. The receive tail reads
    /// the mailbox unconditionally after `schedule()` returns, so an undelivered wake completes
    /// the receive with whatever the mailbox happened to hold, and the receiver's TCB is still
    /// linked on the rendezvous's wait queue (the waker that owns the unlink never ran), which is
    /// the intrusive one-link invariant broken in kernel memory.
    ///
    /// The claim: a `Blocked` IPC thread may only become `Ready` by the hand that completed its
    /// rendezvous (message staged, signal counted, or abort flagged). An undelivered wake is
    /// refused, the receiver stays parked, and a real sender still reaches it afterwards.
    /// The canary tripwire's contract, both halves: an unchanged watched range reports nothing,
    /// and a byte flipped behind its back is counted (and printed) on the next check. The scratch
    /// is this test's own static, so the live registries are never poked; arming over the real
    /// tables is `canary_arm_registries`, which is plain plumbing over the same `arm`.
    ///
    /// Both checks LOOP until a pass actually runs, and the loop is the fix for a real flake
    /// (thead-c906, 2026-08-15; notes/cpu-models.md BUGS): `check()` is single-flight, timer
    /// ticks on other cores call it too (secondaries are online here), and this test's decisive
    /// call used to lose the slot to a tick's pass that had read the scratch byte *before* the
    /// flip. The old `check()` swallowed that refusal and the flip went uncounted; now it says
    /// `false` and the test insists on a pass of its own.
    #[test_case]
    fn the_canary_reports_a_byte_that_changed_behind_its_back() {
        use core::sync::atomic::AtomicU8;
        static SCRATCH: [AtomicU8; 32] = [const { AtomicU8::new(0xA5) }; 32];
        let base = SCRATCH.as_ptr() as usize;
        super::canary::arm(&[(base, 32)]);
        while !super::canary::check() {
            core::hint::spin_loop();
        }
        assert_eq!(
            super::canary::divergences(),
            0,
            "an unchanged range must not diverge"
        );
        SCRATCH[7].store(0x5A, Ordering::Relaxed);
        // The timer's own check may absorb the flip before this completed pass; either way the
        // count is visible here (this core ran a full pass after the store, and taking the gate
        // acquires whatever an earlier pass counted before releasing it).
        while !super::canary::check() {
            core::hint::spin_loop();
        }
        assert!(
            super::canary::divergences() >= 1,
            "a flipped watched byte must be reported"
        );
        super::canary::disarm();
    }

    #[test_case]
    fn a_wake_without_delivery_cannot_complete_a_parked_receive() {
        static GOT: AtomicU64 = AtomicU64::new(u64::MAX);
        static DONE: AtomicBool = AtomicBool::new(false);
        GOT.store(u64::MAX, Ordering::SeqCst);
        DONE.store(false, Ordering::SeqCst);

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = crate::sched::create_rendezvous_from(region).expect("no rendezvous from region");
        let tid = crate::sched::spawn(move || {
            let m = crate::sched::ipc_receive(ep);
            GOT.store(m[0], Ordering::SeqCst);
            DONE.store(true, Ordering::SeqCst);
        })
        .expect("spawn receiver");

        // Queued on the rendezvous, not "probably scheduled by now" (the milestone-81 lesson).
        assert!(
            wait_for(|| crate::sched::rendezvous_waiting_receivers(ep) == 1),
            "the receiver never parked on the rendezvous"
        );

        // The injection: the real wake path, nothing delivered.
        crate::sched::wake_without_delivery(tid);

        // The receiver must stay parked. Held for half a second of yields rather than one look,
        // because the spurious completion needs the receiver to be scheduled first.
        let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 2;
        while crate::arch::timer::now() < deadline {
            assert!(
                !DONE.load(Ordering::SeqCst),
                "a wake with nothing delivered completed the receive (it returned {:#x})",
                GOT.load(Ordering::SeqCst),
            );
            crate::sched::yield_now();
        }
        assert_eq!(
            crate::sched::rendezvous_waiting_receivers(ep),
            1,
            "the undelivered wake took the receiver off the rendezvous"
        );

        // And the rendezvous still works: a real sender completes the same receive with its message.
        crate::sched::ipc_send(ep, [81, 0, 0]);
        assert!(
            wait_for(|| DONE.load(Ordering::SeqCst)),
            "the real message never arrived after the refused wake"
        );
        assert_eq!(
            GOT.load(Ordering::SeqCst),
            81,
            "the receive completed with something other than the real message"
        );
        assert!(
            wait_for(|| !crate::sched::is_thread_present(tid)),
            "the receiver never exited",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// **A reply only wakes a caller that awaits one** (boot 8's observe-and-strand guard). A
    /// `Reply` capability names a tid, not a wait state. `ipc_reply` used to deliver to any
    /// `Blocked` thread with that tid: invoked against a thread parked as an ordinary rendezvous
    /// receiver (a stale reply whose CALL was long since aborted, with the caller re-parked
    /// elsewhere), it clobbered the mailbox and woke the thread messageless while its TCB was
    /// still linked on the rendezvous's wait queue. Same strand as the test above, reached through
    /// the one wake site addressed by tid rather than by rendezvous.
    #[test_case]
    fn a_reply_to_a_thread_parked_as_a_receiver_is_dropped() {
        static GOT: AtomicU64 = AtomicU64::new(u64::MAX);
        static DONE: AtomicBool = AtomicBool::new(false);
        GOT.store(u64::MAX, Ordering::SeqCst);
        DONE.store(false, Ordering::SeqCst);

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = crate::sched::create_rendezvous_from(region).expect("no rendezvous from region");
        let tid = crate::sched::spawn(move || {
            let m = crate::sched::ipc_receive(ep);
            GOT.store(m[0], Ordering::SeqCst);
            DONE.store(true, Ordering::SeqCst);
        })
        .expect("spawn receiver");

        assert!(
            wait_for(|| crate::sched::rendezvous_waiting_receivers(ep) == 1),
            "the receiver never parked on the rendezvous"
        );

        // A reply aimed at a thread that is not awaiting a reply: dropped, like a reply to a
        // dead caller.
        crate::sched::ipc_reply(tid, [0xDEAD, 0]);

        let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 2;
        while crate::arch::timer::now() < deadline {
            assert!(
                !DONE.load(Ordering::SeqCst),
                "a stray reply completed a receiver's receive (it returned {:#x})",
                GOT.load(Ordering::SeqCst),
            );
            crate::sched::yield_now();
        }

        // The mailbox was not clobbered and the rendezvous still works.
        crate::sched::ipc_send(ep, [81, 0, 0]);
        assert!(
            wait_for(|| DONE.load(Ordering::SeqCst)),
            "the real message never arrived after the dropped reply"
        );
        assert_eq!(
            GOT.load(Ordering::SeqCst),
            81,
            "the receive completed with the stray reply's words, not the real message"
        );
        assert!(
            wait_for(|| !crate::sched::is_thread_present(tid)),
            "the receiver never exited",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// Several threads take turns.
    #[test_case]
    fn threads_round_robin() {
        static COUNTS: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];
        static STOP: AtomicBool = AtomicBool::new(false);

        let mut tids = [0 as crate::thread::ThreadId; 3];
        for (t, c) in tids.iter_mut().zip(&COUNTS) {
            *t = crate::sched::spawn(move || {
                while !STOP.load(Ordering::SeqCst) {
                    c.fetch_add(1, Ordering::SeqCst);
                    crate::sched::yield_now();
                }
            })
            .expect("spawn failed");
        }

        // Wait ON the property (every thread has run), clock-bounded, rather than asserting after
        // a fixed 300 yields. A yield count is not a duration: §28 scatters these threads across
        // cores, and on a contended host this core can burn its yields before a starved vCPU has
        // run its thread at all, which failed a gate run once as "thread {i} never ran" and passed
        // on the re-run. Widening a wait on the property itself only delays noticing (the smp.rs
        // wait_for argument); the watchdog stays the backstop for a genuine starvation.
        let all_ran = || COUNTS.iter().all(|c| c.load(Ordering::SeqCst) > 0);
        let ran = wait_for(all_ran);
        STOP.store(true, Ordering::SeqCst);
        assert!(ran, "a spawned thread never ran");

        // Wait for the exits, so three threads mid-teardown are not what a later test's frame or
        // thread accounting finds in flight.
        assert!(
            wait_for(|| tids.iter().all(|&t| !crate::sched::is_thread_present(t))),
            "the round-robin threads were never reaped"
        );
    }

    /// **THE TEST.**
    ///
    /// From DECISIONS §5, written before a single line of this kernel existed:
    ///
    /// > A userspace process is an arbitrary ELF binary. It has its own stack, **it never
    /// > yields**, and it will loop forever because we will write a bug. Under cooperative
    /// > scheduling, one bad user program hangs the machine permanently.
    ///
    /// So: a thread whose entire body is a tight loop. **No `yield_now`. No syscall. Not even a
    /// function call**: nothing a cooperative scheduler could possibly hook.
    ///
    /// Under async/await, or Go before 1.14, or any cooperative runtime, this thread takes the
    /// CPU and never gives it back, and the machine is gone. The only thing that can take it
    /// back is a timer interrupt landing between two instructions of that loop and switching
    /// the stack out from under it.
    ///
    /// If this test passes, the argument was right and the kernel can host untrusted code.
    /// If it hangs, it was wrong.
    #[test_case]
    fn a_thread_that_never_yields_is_preempted_anyway() {
        static SPINNING: AtomicU64 = AtomicU64::new(0);
        static STOP: AtomicBool = AtomicBool::new(false);
        static OTHER_RAN: AtomicBool = AtomicBool::new(false);

        // Pin both threads to THIS core, the one the test thread busy-waits on, so this stays a
        // *same-core* preemption test after DECISIONS §28 made the default `spawn` scatter work
        // across cores. The claim under test is that a never-yielding thread cannot monopolize the
        // core it is on; if the spinner ran on some other idle core the timer would never have to
        // preempt anything here, and the test would prove nothing. `spawn_on(cpu::id())` keeps the
        // spinner, the polite thread, and the waiter contending for one core, as they always did.
        let here = crate::cpu::id();

        // The hostile thread. This is the arbitrary ELF binary, in miniature.
        crate::sched::spawn_on(here, || {
            while !STOP.load(Ordering::Relaxed) {
                SPINNING.fetch_add(1, Ordering::Relaxed);
                // Deliberately nothing else. No yield. No call. Nothing to cooperate with.
            }
        })
        .expect("spawn failed");

        // **Wait for the hostile thread to reach a CPU before the polite one exists.**
        //
        // The order used to be: spawn both, wait for the polite thread, set STOP, and only then
        // sample `SPINNING > 0`. That sample is a race the test creates for itself: if the polite
        // thread gets its turn first, STOP is set before the spinner has ever been scheduled, the
        // spinner exits its loop without incrementing anything, and the run goes red with "the
        // spinner never ran at all" while the kernel did nothing wrong. It failed CI exactly that
        // way on 2026-08-04 (`sifive-u54`), on a pull request that changed no kernel code.
        //
        // The spinner running is a **precondition** of the claim, not the claim, so it is waited
        // on rather than sampled. Waiting for it here also strengthens what follows: the polite
        // thread's turn can now only have come from preempting a thread that was genuinely running.
        assert!(
            within_ticks(200, || SPINNING.load(Ordering::Relaxed) > 0),
            "the spinner never reached a CPU in 200 tick periods: it was placed on a run queue \
             and never scheduled, which is a placement or wake failure rather than a preemption one"
        );

        // Counted from here, so the preemptions this test claims are the ones that gave the polite
        // thread its turn, not the ones that started the spinner.
        let preemptions_before = crate::sched::preemptions();

        // A well-behaved thread that just wants a turn.
        crate::sched::spawn_on(here, || {
            OTHER_RAN.store(true, Ordering::SeqCst);
        })
        .expect("spawn failed");

        // And now we wait, WITHOUT yielding either. If preemption does not work, nobody moves and
        // the budget runs out. The budget is 200 *delivered ticks* rather than one second of wall
        // clock: preemption opportunities are what this claim is counted in, and a host that
        // deschedules the emulator produces fewer of them, never more. See `within_ticks`.
        assert!(
            within_ticks(200, || OTHER_RAN.load(Ordering::SeqCst)),
            "TWO HUNDRED TICKS AND THE POLITE THREAD NEVER RAN. The spinner still owns the CPU, \
             which means preemption is not working and a single bad program can hang this \
             machine. This is precisely the failure DECISIONS §5 predicted for \
             cooperative scheduling."
        );

        // **And the spinner has to have actually spun before we stop it**, or the test is vacuous:
        // a polite thread running on a core nobody was monopolizing says nothing about preemption.
        // Stopping it the instant the polite thread reported is a race the two orderings decide,
        // and on the physical core under HVF (milestone 81) it came out the other way: the polite
        // thread ran first, `STOP` was set, and the spinner was killed before its first increment
        // ("the spinner never ran at all"). Its own second, since the wait above may have spent all
        // of the first.
        let spin_deadline = crate::arch::timer::now() + crate::arch::timer::frequency();
        while SPINNING.load(Ordering::Relaxed) == 0 {
            assert!(
                crate::arch::timer::now() < spin_deadline,
                "the spinner never ran at all, so nothing was monopolizing this core and the \
                 polite thread's turn proves nothing about preemption"
            );
            core::hint::spin_loop();
        }

        STOP.store(true, Ordering::Relaxed);

        assert!(
            crate::sched::preemptions() > preemptions_before,
            "the CPU was never taken away from anyone: no preemption happened"
        );

        // Let the spinner notice STOP and exit, so it does not haunt the rest of the suite.
        for _ in 0..50 {
            crate::sched::yield_now();
        }
    }

    /// A finished thread's stack is unmapped and its frames returned.
    ///
    /// The reaping cannot happen in `exit()`: a thread cannot unmap the stack it is standing
    /// on. It happens in `schedule()`, from the *next* thread, once we are safely off it. Every
    /// kernel has something called a reaper, and this is why.
    #[test_case]
    fn a_finished_thread_is_reaped_and_its_memory_returned() {
        // Reaping is proven per thread, by `is_thread_present` on the Tids THIS test spawned, not
        // by the global table headcount returning to a baseline. `thread_count()` is the size of
        // the whole table, so a neighbour's teardown finishing late moves it: it failed on CI as
        // "left: 5, right: 6", a count BELOW its baseline, which eight reaped threads cannot
        // produce but one baseline-counted thread exiting mid-test does. Same shape as the
        // `reclaim_frees_a_started_then_exited_childs_regions` fix; see
        // notes/load-sensitive-assertions.md.
        //
        // Where the reuse probe below found its own stack, **reported by the thread itself**. A test
        // cannot read the stack out of a thread it spawned, because by the time it looks the thread
        // may already have been reaped, which is the very thing this test waits for. A thread taking
        // the address of one of its own locals has no such race.
        static PROBE_SP: AtomicU64 = AtomicU64::new(0);

        fn batch_of_eight() {
            let mut tids = [0 as crate::thread::ThreadId; 8];
            for t in &mut tids {
                *t = crate::sched::spawn(|| {}).expect("spawn failed");
            }
            // Let them all run and exit, and let the reaper catch up. Clock-bounded, not yield-bounded:
            // §28 can place these on other cores, and a Finished thread is only removed when its own
            // core switches away from it, so no number of yields *here* can make that happen.
            assert!(
                wait_for(|| tids.iter().all(|&t| !crate::sched::is_thread_present(t))),
                "finished threads were never reaped"
            );
        }

        // The FIRST batch legitimately costs a couple of frames: the stack area is a fresh
        // region of virtual address space, so `map_page` has to build an L2 and an L3 page
        // table for it. Those are a one-time cost, not a leak: `unmap_page` frees the leaf
        // mapping but leaves the intermediate tables standing, on purpose (notes/teardown.md).
        batch_of_eight();

        // **Reuse is asserted directly, because the frame count below cannot do it.** Measured
        // 2026-08-17: with the `FREE_STACK_ADDRESS_SPACE` push deleted from `KernelStack::drop`, which IS the
        // milestone-6 bug this test is named for, the entire aarch64 leg passed, this test included.
        // The reason is arithmetic rather than luck. A slot is `STACK_SLOT_SPAN`, 28 KiB, so eight
        // of them consume 224 KiB of fresh address space, and a leaked page table costs a *frame*
        // only when the bump crosses a 2 MiB L3 boundary. 224 KiB is 11% of one table's span, so
        // the frame count can see the defect only when the batch happens to straddle a boundary,
        // and that is worse than 11% random: where `NEXT_STACK_VA` stands here is a function of how
        // many threads the tests BEFORE this one spawned, which is fixed for a given tree. So for
        // any given tree the frame count either always catches the defect or always misses it, and
        // which one is decided by unrelated code upstream. The frame assertion below is the outcome;
        // this is the mechanism, and at that batch size only the mechanism is observable.
        //
        // The claim: a thread spawned after the first batch has been reaped lands BELOW the
        // watermark that stood before it, which is what "it reused a dead thread's range" means.
        //
        // **ONE thread, and the count is the whole argument.** The first version of this asserted it
        // for all eight of a batch and failed on a clean kernel, on thread 1, two slots above the
        // watermark. That was correct behaviour and a wrong assertion: the watermark is the
        // high-water mark of *concurrent* live threads, so a batch whose threads happen to be reaped
        // later relative to spawning legitimately needs more slots than the previous batch did, and
        // bumps it. Asserting over a batch conflated reuse with concurrency, which is this
        // milestone's own defect ("a wait written against something wider than the property")
        // committed while fixing it. Recorded in notes/load-sensitive-assertions.md rather than
        // quietly corrected, because reproducing the family from the inside is the useful part.
        //
        // One thread cannot exceed a high-water mark that eight just set. `is_thread_present` going
        // false already implies the push happened (`Threads::remove` runs `KernelStack::drop` before
        // it removes the table entry), so the free list holds up to eight of the first batch's slots
        // when this spawns, and a single pop cannot drain it. A neighbour spawning here can only
        // RAISE the watermark, which makes the claim easier: the failure direction is one-way, which
        // is the discipline the rest of this test was rebuilt for.
        //
        // It runs BEFORE the frame baseline below so that the settle loop absorbs its own stack
        // frees, rather than leaving them in flight inside the window the frame assertion measures.
        let watermark = crate::thread::stack_area_span().1;
        PROBE_SP.store(0, Ordering::SeqCst);
        let probe = crate::sched::spawn(|| {
            let local = 0u64;
            PROBE_SP.store(&local as *const u64 as u64, Ordering::SeqCst);
        })
        .expect("spawn failed");
        assert!(
            wait_for(|| !crate::sched::is_thread_present(probe)),
            "the stack-reuse probe was never reaped"
        );
        let probe_sp = PROBE_SP.load(Ordering::SeqCst);
        assert!(
            probe_sp != 0,
            "the stack-reuse probe never reported which stack it got"
        );
        assert!(
            probe_sp < watermark,
            "a thread spawned after eight were reaped was given FRESH stack address space: its sp \
             {probe_sp:#x} is at or above the {watermark:#x} watermark that stood before it, so a \
             dead thread's range was not reused and an L2 plus an L3 page table leak per 2 MiB \
             consumed, forever"
        );

        // Sample the frame baseline only once it has STOPPED MOVING: a reaped thread's stack
        // frames are freed by `finish_switch` on whatever core reaps it, a beat after the thread
        // leaves the table, so reading `used` the instant the Tids are gone races the first
        // batch's own frees. Two agreeing samples a yield apart mean nothing is in flight, and
        // `wait_for`'s deadline keeps a genuinely unstable allocator a failure rather than a spin.
        let used = || crate::memory::stats().unwrap().used;
        let mut last = used();
        assert!(
            wait_for(|| {
                crate::sched::yield_now();
                let prev = core::mem::replace(&mut last, used());
                prev == last
            }),
            "frame accounting never settled after the first batch"
        );
        let before = last;

        // The SECOND batch must allocate NOTHING it keeps. The page tables exist, and the dead
        // threads' virtual address ranges went back on the free list, so eight new threads land
        // in the same addresses with the same tables.
        //
        // If this ever regresses, the kernel leaks two frames of page tables per 2 MiB of stack
        // address space consumed, forever, and threads come and go.
        batch_of_eight();

        // `<=`, not `==`, and the direction is the argument: a leak leaves `used` ABOVE `before`
        // and never comes back, so the wait times out and fails. A neighbour's late teardown
        // landing in this window can only FREE frames, pushing `used` below `before`, and holding
        // still is not a property this test can demand of the rest of the machine.
        //
        // **This comment used to end "sensitivity to the milestone-6 bug is unchanged: every
        // leaked frame keeps `used() <= before` false", and that was true of the arithmetic and
        // false about the bug.** It is unchanged from the `==` form, which is what it was written
        // to defend, and both forms are near-blind: the defect leaks page tables per 2 MiB of
        // address space and eight threads consume 224 KiB, so there is usually no leaked frame for
        // either form to see. Corrected 2026-08-17 by deleting the VA push and watching the leg go
        // green. What this assertion is genuinely responsible for is the leak that *does* show at
        // this batch size, a per-thread frame the reaper failed to return; the reuse claim above is
        // what covers the defect in the test's name.
        //
        // The number in the message is the one the wait DECIDED on, not a fresh sample. Re-reading
        // `used()` to format the panic races the frames still arriving, so a genuine timeout could
        // report a delta of zero: `saturating_sub` clamped the negative case away instead of making
        // it impossible, and "leaked 0 frames" is the same unreadable diagnostic as the "-52" this
        // milestone is named for, minus the sign that gave it away. `wait_for` re-evaluates the
        // predicate once after its deadline, so a `false` return leaves `seen > before` and the
        // count below cannot be zero. See notes/load-sensitive-assertions.md.
        let mut seen = before;
        let came_back = wait_for(|| {
            seen = used();
            seen <= before
        });
        assert!(
            came_back,
            "a second batch of eight threads leaked {} frames: stack address ranges are not \
             being reused, so page tables accumulate forever",
            seen - before
        );
    }

    /// Every thread stack has a guard page.
    ///
    /// A thread stack is 24 KiB (under half the boot stack's), and threads are where deep
    /// recursion actually happens. Milestone 3's stack overflow hung the machine for 150
    /// seconds; a guard page turns the same bug into an instant fault naming the exact byte.
    #[test_case]
    fn every_thread_stack_has_a_guard_page() {
        use crate::arch::mmu;
        use crate::thread::{KernelStack, STACK_PAGES};

        let stack = KernelStack::new().expect("could not allocate a thread stack");

        assert_eq!(
            mmu::translate(stack.guard()),
            None,
            "a thread stack's guard page IS MAPPED: an overflow would silently eat whatever is \
             below it"
        );

        // And the stack itself is real, writable memory directly above the hole.
        for i in 0..STACK_PAGES as u64 {
            let va = stack.bottom() + i * 4096;
            let (_, flags) = mmu::translate(va).expect("thread stack page is not mapped");
            assert!(flags.is_writable());
            assert!(
                !flags.is_kernel_executable(),
                "a thread stack is EXECUTABLE"
            );
        }
    }

    /// **A fatal fault can name the stack it fell off**, for all three kinds of kernel stack.
    ///
    /// Milestone 78. The guard pages already worked; nothing *said* so. Two `cpu matrix` runs died
    /// with `unexpected RISC-V trap: scause=0xf stval=0xffffffd0001fe000 from_user=false`, which
    /// reads as a memory-system fault, and it took hand arithmetic against `thread.rs`'s slot span
    /// to discover that the address was the base of a thread stack's guard page. `guard_page_at` is
    /// that arithmetic, in the kernel, so the machine says it instead.
    ///
    /// **That `stval` is one of only two addresses this project's guard-page faults ever used**,
    /// and the other is aarch64's `0xffff0010001b3000`. An earlier version of this paragraph read
    /// that as proof of a fixed-site writer, on the argument that "a depth-driven overflow does not
    /// repeat an address". **It does, and exactly this one** (2026-08-17): a fault that reaches the
    /// exception vector's own frame store walks `sp` down a frame at a time and stores upward in
    /// aligned steps, so the terminal store lands on the guard base exactly, whatever `sp` was
    /// doing. The address carried no information; the *slot number* did, and it survived a change
    /// to the slot span. See notes/stack/kernel-stack-freed-under-its-owner.md.
    ///
    /// The three kinds are allocated three different ways (a linker symbol, a `.bss` array, a slot
    /// in the virtual area 64 GiB up), so this checks one of each rather than trusting one to stand
    /// for the others. It also checks the *negatives*, because a classifier that answered
    /// `Thread(0)` for every address would pass the positive half and be worse than nothing.
    #[test_case]
    fn a_guard_page_fault_names_its_stack() {
        use crate::arch::mmu;
        use crate::stack::{GuardPage, guard_page_at};
        use crate::thread::{KernelStack, STACK_SLOT_SPAN};

        assert_eq!(guard_page_at(mmu::stack_guard()), Some(GuardPage::Boot));
        assert_eq!(
            guard_page_at(mmu::stack_guard() + 4095),
            Some(GuardPage::Boot),
            "the last byte of the guard page is still the guard page"
        );
        assert_eq!(
            guard_page_at(mmu::stack_bottom()),
            None,
            "the first usable byte of the stack is not a guard page"
        );

        // Slot 1's guard address is a static layout fact, not a runtime one: the classifier does
        // range math over addresses that exist for every slot, online or parked, so this needs no
        // core 1 at all. (An earlier comment here justified the index by core 1 being online,
        // which was the wrong reason and read as an online-set assumption.)
        let g = crate::smp::secondary_stack_guard(1);
        assert_eq!(guard_page_at(g), Some(GuardPage::Secondary(1)));

        // A live thread stack, so the watermark provably covers it.
        let stack = KernelStack::new().expect("could not allocate a thread stack");
        let slot = (stack.guard() - crate::thread::stack_area_span().0) / STACK_SLOT_SPAN;
        assert_eq!(guard_page_at(stack.guard()), Some(GuardPage::Thread(slot)));
        assert_eq!(
            guard_page_at(stack.bottom()),
            None,
            "the stack's own first page reported as its guard page"
        );
        assert_eq!(
            guard_page_at(stack.top() - 8),
            None,
            "the top of the stack reported as a guard page"
        );

        // Kernel text is not a stack of any kind.
        assert_eq!(guard_page_at(guard_page_at as *const () as u64), None);
    }

    /// **A dead thread that has not left its own kernel stack is not reapable**, and that one
    /// clause is the whole of the bug that produced four `*** KERNEL STACK OVERFLOW ***` panics in
    /// CI over five days without any stack ever overflowing.
    ///
    /// `depart` publishes a supervised thread as `Dead` and delivers its death message, waking the
    /// supervisor, *before* it reaches `switch_to`. A supervisor that reaps in that window used to
    /// free the `Thread`, and `KernelStack::drop` unmaps six pages with a real `tlbi` under a core
    /// that is still running on them. The corpse's next store faulted, the exception vector's own
    /// frame store faulted on the same dead stack, and the vector walked `sp` down one 272-byte
    /// frame at a time until it landed in the mapped stack below, which is why the reported address
    /// was the slot base every single time and never moved.
    ///
    /// The rule is stated over `(state, on_cpu)` rather than over a live thread table because the
    /// window is a few hundred instructions wide on two cores, which is not a thing a test can
    /// stage. It reproduced on a desk only with a deliberate spin loop inserted in `depart`; see
    /// notes/stack/kernel-stack-freed-under-its-owner.md. What this pins is the claim, so the
    /// next person to edit that loop meets `on_cpu` as a requirement rather than as a detail.
    #[test_case]
    fn a_dead_thread_still_standing_on_its_stack_is_not_reapable() {
        use super::{RegionReap, State, region_reap_verdict};

        // Off its stack: the states that mean "never runs again" really are reapable, which is
        // what makes region teardown work at all.
        for state in [State::Dead, State::Finished, State::Embryo] {
            assert_eq!(
                region_reap_verdict(state, false),
                RegionReap::Reap,
                "{state:?} with no core on its stack must be reapable",
            );
        }

        // Still on its stack: refused, and refused WITHOUT arming a kill, because there is nothing
        // left to kill and the condition clears itself one context switch from now.
        for state in [State::Dead, State::Finished] {
            assert_eq!(
                region_reap_verdict(state, true),
                RegionReap::RefuseStanding,
                "{state:?} still standing on its kernel stack must not have that stack unmapped",
            );
        }

        // A thread that can still be scheduled is the older refusal, and it still arms the kill.
        for state in [State::Ready, State::Running] {
            assert_eq!(
                region_reap_verdict(state, false),
                RegionReap::RefuseAndArm,
                "a live {state:?} thread must be refused and armed",
            );
            assert_eq!(
                region_reap_verdict(state, true),
                RegionReap::RefuseAndArm,
                "being on a cpu must not downgrade a live thread's refusal to the passive one",
            );
        }

        // **`Blocked` left the arm on 2026-09-03** (milestone 133, proposal A), and it left for a
        // reason this assertion is the record of: the arm is spent by `schedule()` only for a
        // thread whose state is `Running`, so arming a thread that never runs again armed nothing
        // and refused forever. It is now finished in place.
        assert_eq!(
            region_reap_verdict(State::Blocked, false),
            RegionReap::FinishInPlace,
            "a blocked resident must be ended, because arming it never reaches it",
        );

        // And a `Blocked` thread still standing on its kernel stack is the one that must NOT be
        // finished in place: freeing its `Thread` unmaps the stack a core is standing on, which is
        // the same four-CI-panic bug the `Dead` case above pins. The refusal is passive, so the
        // owner's next retry finds it off its stack and ends it then.
        assert_eq!(
            region_reap_verdict(State::Blocked, true),
            RegionReap::RefuseStanding,
            "a blocked thread mid-switch-out must not have its stack unmapped",
        );
    }

    /// **The slots are contiguous, so one stack's guard page begins where the previous stack
    /// ends**, and a fault report has to be able to say which of the two it is looking at.
    ///
    /// This is the geometry that made two 2026-08-16 guard-page faults ambiguous. Both landed on a
    /// slot's guard page at offset 0 and 8, and the report said "sp went 4096 bytes past the
    /// bottom" without ever reading `sp`. Those same two addresses are also the first two words
    /// **above the top of the stack in the slot below**, which is a completely different bug with
    /// a completely different fix, and nothing printed could tell them apart.
    ///
    /// `thread_stack_site` is what lets the report place `sp` in the same units as the faulting
    /// address. The assertions below are the arithmetic that claim rests on: a slot's usable span
    /// measured from its own bottom, its guard page as negative offsets, and the join, where slot
    /// `n`'s guard base is exactly one past slot `n-1`'s last usable byte.
    #[test_case]
    fn a_slots_guard_page_begins_where_the_slot_below_it_ends() {
        use crate::stack::thread_stack_site;
        use crate::thread::{KernelStack, STACK_PAGES, STACK_SLOT_SPAN};

        let stack = KernelStack::new().expect("could not allocate a thread stack");
        let (area, _) = crate::thread::stack_area_span();
        let slot = (stack.guard() - area) / STACK_SLOT_SPAN;

        assert_eq!(
            thread_stack_site(stack.bottom()),
            Some((slot, 0)),
            "the lowest usable byte is zero bytes above the bottom"
        );
        assert_eq!(
            thread_stack_site(stack.top() - 8),
            Some((slot, (STACK_PAGES * 4096) as i64 - 8)),
            "the last usable word is one word short of the stack's size above the bottom"
        );
        assert_eq!(
            thread_stack_site(stack.guard()),
            Some((slot, -4096)),
            "the guard page's base is a whole page below the bottom"
        );

        // The join. `top` is exclusive, so it is the next slot's guard base, and the site function
        // must report it as *that* slot's guard rather than as this one's stack.
        assert!(
            slot >= 1,
            "the first stack allocated in the suite is not slot 0"
        );
        assert_eq!(
            thread_stack_site(area + slot * STACK_SLOT_SPAN),
            Some((slot, -4096)),
        );
        assert_eq!(
            thread_stack_site(area + slot * STACK_SLOT_SPAN - 8),
            Some((slot - 1, (STACK_PAGES * 4096) as i64 - 8)),
            "the word below a slot's guard base belongs to the previous slot's stack, and a report \
             that cannot say so cannot tell an overflow from a store past a neighbour's top",
        );

        // Outside the area entirely: kernel text is not a thread stack.
        assert_eq!(
            thread_stack_site(thread_stack_site as *const () as u64),
            None
        );
    }

    /// **The rendezvous, receiver-first.** A thread blocks on an empty rendezvous, and stays
    /// blocked, and a *later* sender is what frees it: carrying the message.
    #[test_case]
    fn a_receiver_blocks_until_a_sender_arrives() {
        static GOT: AtomicU64 = AtomicU64::new(0);
        static RECEIVED: AtomicBool = AtomicBool::new(false);

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

        let tid = super::spawn(move || {
            let msg = super::ipc_receive(ep); // nobody is sending yet: this BLOCKS
            GOT.store(msg[0], Ordering::SeqCst);
            RECEIVED.store(true, Ordering::SeqCst);
        })
        .expect("spawn failed");

        // Let the receiver run and block. It must NOT have received anything: there is no sender.
        for _ in 0..50 {
            super::yield_now();
        }
        assert!(
            !RECEIVED.load(Ordering::SeqCst),
            "a receiver returned from an rendezvous nobody had sent to",
        );

        // Now send. This should hand the receiver its message and wake it.
        super::ipc_send(ep, [0xABCD, 0, 0]);

        // Clock-bounded, not yield-bounded: see `wait_for`.
        assert!(
            wait_for(|| RECEIVED.load(Ordering::SeqCst)),
            "the receiver never woke"
        );
        assert_eq!(
            GOT.load(Ordering::SeqCst),
            0xABCD,
            "wrong message delivered"
        );
        assert!(
            wait_for(|| !crate::sched::is_thread_present(tid)),
            "the receiver never exited",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// **The rendezvous, sender-first.** The other order: a sender blocks on an rendezvous with no
    /// receiver, and a later receiver collects the parked message and wakes it.
    #[test_case]
    fn a_sender_blocks_until_a_receiver_arrives() {
        static SENT_RETURNED: AtomicBool = AtomicBool::new(false);

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

        let tid = super::spawn(move || {
            super::ipc_send(ep, [0x1234, 0x5678, 0x9abc]); // nobody receiving yet: BLOCKS
            SENT_RETURNED.store(true, Ordering::SeqCst);
        })
        .expect("spawn failed");

        for _ in 0..50 {
            super::yield_now();
        }
        assert!(
            !SENT_RETURNED.load(Ordering::SeqCst),
            "a send returned before anyone received it",
        );

        let msg = super::ipc_receive(ep); // collects the parked message, wakes the sender
        // Five words now (the top two are the fault path's, DECISIONS §26); an ordinary send fills
        // the first three and leaves the rest zero.
        assert_eq!(
            msg,
            [0x1234, 0x5678, 0x9abc, 0, 0],
            "wrong message received"
        );

        // Clock-bounded, not yield-bounded: see `wait_for`. This one has evidence rather than a
        // theory behind it: under eight spinning host processes it failed here on `rv64` on
        // 2026-08-04, because fifty yields on an idle core are microseconds and the sender was on
        // a vCPU the host had descheduled.
        assert!(
            wait_for(|| SENT_RETURNED.load(Ordering::SeqCst)),
            "the sender never woke after its message was taken",
        );
        assert!(
            wait_for(|| !crate::sched::is_thread_present(tid)),
            "the sender never exited",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// **A request and a reply, over two endpoints.** The shape milestone 8's console server
    /// will have: a client sends a request and blocks for the answer; a server loops on the
    /// request rendezvous, does the work, and replies on the reply rendezvous.
    ///
    /// All three message words survive the round trip, which is what proves the receiver's
    /// `x1`/`x2` handling and the mailbox are correct end to end.
    #[test_case]
    fn a_request_gets_a_reply() {
        static ANSWER: AtomicU64 = AtomicU64::new(0);
        static DONE: AtomicBool = AtomicBool::new(false);

        let region = crate::memory_region::create(2).expect("no region for two test rendezvous");
        let req = super::create_rendezvous_from(region).expect("no rendezvous from region");
        let rep = super::create_rendezvous_from(region).expect("no rendezvous from region");

        // The server: receive n on `req`, send n + 1 back on `rep`.
        let server = super::spawn(move || {
            let m = super::ipc_receive(req);
            super::ipc_send(rep, [m[0] + 1, m[1], m[2]]);
        })
        .expect("spawn failed");

        // The client.
        let client = super::spawn(move || {
            super::ipc_send(req, [41, 0, 0]);
            let answer = super::ipc_receive(rep);
            ANSWER.store(answer[0], Ordering::SeqCst);
            DONE.store(true, Ordering::SeqCst);
        })
        .expect("spawn failed");

        assert!(
            spin_until(|| DONE.load(Ordering::SeqCst)),
            "the request/reply never completed"
        );
        assert_eq!(
            ANSWER.load(Ordering::SeqCst),
            42,
            "the server computed the wrong answer"
        );
        assert!(
            wait_for(|| !crate::sched::is_thread_present(server)
                && !crate::sched::is_thread_present(client)),
            "the request/reply test's own threads had not finished when it returned",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// **Milestone 19c.1: the kernel cannot spend beyond its boot carve, for stacks.** Spawn a
    /// batch of threads and let them reap; the frame allocator's free count must return to
    /// exactly where it started, because kernel stacks now come from the kernel's own budget
    /// region (`kmem`, carved once) and recycle within it, not from the allocator. This is the
    /// milestone-14 no-open-ended-kernel-spending thesis extended to the last thing it missed;
    /// before 19c.1 this test would show four stacks' worth of frames gone per batch.
    ///
    /// The carve itself happens on the very first spawn ever (the idle thread, at boot), so by
    /// the time this test runs the region exists and steady state is flat.
    #[test_case]
    fn kernel_stacks_do_not_touch_the_frame_allocator_in_steady_state() {
        // Each spawn is followed to its own reap by name. This used to take `thread_count()` as a
        // baseline and spin `while thread_count() > baseline { yield_now() }`, which is the whole
        // family in three lines: the headcount is moved by every other test's teardown, so the loop
        // could exit at once (a neighbour reaping first) and leave this batch's stacks in flight,
        // or never exit at all (a neighbour's thread outliving the batch) with no clock to stop it,
        // spinning until the harness's 90 s ceiling with a message about kernel stacks.
        // `is_thread_present` on the ThreadId each spawn returned asks the narrow question, and
        // `wait_for` supplies the bound the yield loop never had.
        let settle = |tid| {
            assert!(
                wait_for(|| !crate::sched::is_thread_present(tid)),
                "a spawned thread was never reaped, so the frame count below would be read \
                 mid-teardown"
            );
        };

        // Warm up: reach steady state (first spawn after boot may still be settling VAs).
        for _ in 0..2 {
            settle(super::spawn(|| {}).expect("warmup spawn"));
        }

        let free_before = crate::memory::stats().unwrap().free();
        for _ in 0..6 {
            settle(super::spawn(|| {}).expect("spawn failed"));
        }

        // `>=`, not `==`, and the direction is the argument, the same one the reaper test's frame
        // half carries. The defect this guards spends allocator frames on kernel stacks, which
        // drives `free` DOWN and keeps it there, so the wait times out and fails exactly as
        // before. A neighbour's late teardown landing in this window can only FREE frames, pushing
        // `free` ABOVE the baseline, and equality additionally demanded that the rest of the
        // machine hold still for the duration, which is not a property this test is responsible
        // for. See notes/load-sensitive-assertions.md.
        //
        // And the number in the message is the one the wait decided on, for the reason the reaper
        // test's frame half carries at length: a re-sampled `saturating_sub` reports zero when the
        // frames land between the wait giving up and the panic being formatted, which is a red run
        // whose message denies there is anything wrong with it.
        let mut seen = free_before;
        let recovered = wait_for(|| {
            seen = crate::memory::stats().unwrap().free();
            seen >= free_before
        });
        assert!(
            recovered,
            "six threads came and went and the frame allocator lost {} frames: a kernel stack is \
             still drawing from the allocator instead of the kernel budget",
            free_before - seen,
        );
    }

    /// **Milestone 19a: an rendezvous retyped from a region carries IPC, and pins its region.**
    /// The kernel-level half of the granular-construction story: `create_rendezvous_from` carves a
    /// page, the rendezvous lives in it, rendezvous works over it exactly as over a kernel-wired
    /// rendezvous, and `memory_region::destroy` refuses the now-pinned region, because freeing the page
    /// under a live rendezvous would dangle every queued thread. The refusal is measured, not
    /// assumed: the allocator's free count must not move.
    #[test_case]
    fn a_retyped_rendezvous_carries_ipc_and_pins_its_region() {
        use core::sync::atomic::{AtomicU64, Ordering};
        static GOT: AtomicU64 = AtomicU64::new(0);

        let region = crate::memory_region::create(2).expect("no region");
        let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");
        let kernel_ep = super::create_rendezvous();
        assert_ne!(ep, kernel_ep, "registry names collide");

        super::spawn(move || {
            GOT.store(super::ipc_receive(ep)[0], Ordering::SeqCst);
        })
        .expect("spawn failed");
        super::ipc_send(ep, [0x2A, 0, 0]);
        spin_until(|| GOT.load(Ordering::SeqCst) != 0);
        assert_eq!(
            GOT.load(Ordering::SeqCst),
            0x2A,
            "no rendezvous over the retyped rendezvous"
        );

        let free_before = crate::memory::stats().unwrap().free();
        crate::memory_region::destroy(region);
        assert_eq!(
            crate::memory::stats().unwrap().free(),
            free_before,
            "destroy reclaimed a pinned region hosting a live rendezvous",
        );
    }

    /// **Milestone 12: a call gets a reply, over one rendezvous, via a one-shot Reply cap.**
    ///
    /// The client `CALL`s and blocks; the server `RECEIVE_CAP`s (receiving the request word plus a
    /// kernel-minted `Reply` cap naming the caller), answers through that cap, and consumes it. One
    /// rendezvous, not the two the pre-`Call` pattern needs, and the server was never wired to this
    /// client.
    #[test_case]
    fn a_call_gets_a_reply() {
        static ANSWER: AtomicU64 = AtomicU64::new(0);
        static DONE: AtomicBool = AtomicBool::new(false);

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

        let server = super::spawn(move || {
            let m = super::ipc_receive_cap(ep); // [n, reply_slot, second_word]
            let slot = m[1];
            let crate::cap::Object::Reply(caller) = super::current_cap(slot).unwrap().object else {
                panic!("RECEIVE_CAP of a CALL did not deliver a Reply capability");
            };
            super::ipc_reply(caller, [m[0] + 1, 0]);
            super::delete_current_cap(slot).expect("consume the one-shot reply");
        })
        .expect("spawn failed");

        let client = super::spawn(move || {
            let r = super::ipc_call(ep, [41, 0]);
            ANSWER.store(r[0], Ordering::SeqCst);
            DONE.store(true, Ordering::SeqCst);
        })
        .expect("spawn failed");

        assert!(
            spin_until(|| DONE.load(Ordering::SeqCst)),
            "the call never returned"
        );
        assert_eq!(ANSWER.load(Ordering::SeqCst), 42, "wrong reply");
        assert!(
            wait_for(|| !crate::sched::is_thread_present(server)
                && !crate::sched::is_thread_present(client)),
            "the call/reply test's own threads had not finished when it returned",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// **Milestone 12: a reply reaches the caller that called, not another.**
    ///
    /// Two clients call and block at once; the server answers each through *its* Reply cap. Client A
    /// (sent 100) must get 111 and client B (sent 200) must get 211. A shared reply rendezvous cannot
    /// guarantee this: whichever client's `RECEIVE` runs grabs the reply. The Reply cap, naming the
    /// specific blocked caller, makes misrouting unrepresentable.
    #[test_case]
    fn a_reply_reaches_the_caller_that_called() {
        static GOT_A: AtomicU64 = AtomicU64::new(0);
        static GOT_B: AtomicU64 = AtomicU64::new(0);

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

        // The server: field two calls, reply each caller its own word + 11, via its own cap.
        let server = super::spawn(move || {
            for _ in 0..2 {
                let m = super::ipc_receive_cap(ep);
                let (word, slot) = (m[0], m[1]);
                let crate::cap::Object::Reply(caller) = super::current_cap(slot).unwrap().object
                else {
                    panic!("not a reply cap");
                };
                super::ipc_reply(caller, [word + 11, 0]);
                super::delete_current_cap(slot).unwrap();
            }
        })
        .expect("spawn failed");

        let client_a = super::spawn(move || {
            let r = super::ipc_call(ep, [100, 0]);
            GOT_A.store(r[0], Ordering::SeqCst);
        })
        .expect("spawn failed");
        let client_b = super::spawn(move || {
            let r = super::ipc_call(ep, [200, 0]);
            GOT_B.store(r[0], Ordering::SeqCst);
        })
        .expect("spawn failed");

        spin_until(|| GOT_A.load(Ordering::SeqCst) != 0 && GOT_B.load(Ordering::SeqCst) != 0);
        assert_eq!(
            GOT_A.load(Ordering::SeqCst),
            111,
            "client A got the wrong caller's reply"
        );
        assert_eq!(
            GOT_B.load(Ordering::SeqCst),
            211,
            "client B got the wrong caller's reply"
        );
        assert!(
            wait_for(|| !crate::sched::is_thread_present(server)
                && !crate::sched::is_thread_present(client_a)
                && !crate::sched::is_thread_present(client_b)),
            "this test's own threads had not finished when it returned",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// A blocked thread is genuinely off the CPU: other threads keep running while it waits.
    ///
    /// If `Blocked` were not respected in `schedule()`, if a blocked thread were helpfully
    /// requeued: this would still pass, so it is not the whole story (the two rendezvous tests
    /// above are). But it is the cheap, direct statement of what blocking is *for*: a waiting
    /// thread must not burn the CPU.
    #[test_case]
    fn other_threads_run_while_one_is_blocked() {
        static PROGRESS: AtomicU64 = AtomicU64::new(0);
        static STOP: AtomicBool = AtomicBool::new(false);

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

        PROGRESS.store(0, Ordering::SeqCst);
        STOP.store(false, Ordering::SeqCst);

        let blocked = super::spawn(move || {
            super::ipc_receive(ep); // blocks forever (nobody sends); must not starve the worker
        })
        .expect("spawn failed");

        let worker = super::spawn(|| {
            while !STOP.load(Ordering::SeqCst) {
                PROGRESS.fetch_add(1, Ordering::SeqCst);
                super::yield_now();
            }
        })
        .expect("spawn failed");

        // Clock-bounded, not yield-bounded: see `wait_for`. It failed here on `sifive-u54` and
        // `rva22s64` under eight spinning host processes on 2026-08-04, which is the same lesson
        // `threads_round_robin` learned three tests up: a hundred yields is not a duration, and on
        // a contended host this core burns them before the worker's vCPU has run at all.
        let progressed = wait_for(|| PROGRESS.load(Ordering::SeqCst) > 0);
        STOP.store(true, Ordering::SeqCst);

        assert!(
            progressed,
            "a worker made no progress while another thread was blocked on IPC",
        );

        // Free the blocked receiver so it does not sit in the rendezvous queue forever, and wait for
        // BOTH threads to actually be gone. Twenty yields used to be the wait, which is the same
        // "count is not a duration" defect one level down: this test's teardown landing late is
        // precisely the neighbouring state that made other tests' frame and thread accounting fail
        // (notes/load-sensitive-assertions.md).
        super::ipc_send(ep, [0, 0, 0]);
        assert!(
            wait_for(|| !crate::sched::is_thread_present(blocked)
                && !crate::sched::is_thread_present(worker)),
            "this test's own threads had not finished when it returned",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// **An interrupt becomes a message.** DECISIONS §10 and notes/interrupts.md, executed.
    ///
    /// A thread blocks waiting on an interrupt it can only name through an rendezvous. We raise the
    /// interrupt from software, the kernel's handler turns it into a notification, and the blocked
    /// thread wakes. This is the exact path a userspace driver takes when a real device interrupts.
    ///
    /// **The two ISAs raise it differently, and they are not twins in what they cost to raise.** See
    /// [`raise_test_irq`]: aarch64 sends itself a GIC SGI, which needs no device at all; RISC-V has
    /// no SGI, so it makes the console UART assert its own line into the PLIC with one register
    /// write. The kernel path under test is the same on both (the handler routes the interrupt to an
    /// rendezvous and signals it), and RISC-V's leg additionally covers the PLIC claim/mask/complete
    /// handshake that an SGI on aarch64 does not reach. What RISC-V gives up is aarch64's "minus the
    /// device" property. The alternative there was the SBI's IPI, which arrives as a *software*
    /// interrupt down a different arm of the trap dispatcher and would not have touched
    /// `irq_route`/`irq_notify` at all, so it would have proved less while looking like more.
    #[test_case]
    fn an_interrupt_becomes_a_message() {
        static WOKE: AtomicBool = AtomicBool::new(false);

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");
        super::bind_irq(delivery_irq(), ep);
        arm_test_irq(delivery_irq());

        let tid = super::spawn(move || {
            super::ipc_receive(ep); // blocks until the interrupt fires
            WOKE.store(true, Ordering::SeqCst);
        })
        .expect("spawn failed");

        // Let the waiter run and block. It must NOT have woken: no interrupt yet.
        for _ in 0..50 {
            super::yield_now();
        }
        assert!(
            !WOKE.load(Ordering::SeqCst),
            "the thread woke before the interrupt fired",
        );

        // Fire it. The controller delivers it, the handler routes it to `ep`, the waiter wakes.
        raise_test_irq(delivery_irq());

        let woke = spin_until(|| WOKE.load(Ordering::SeqCst));
        quiet_test_irq();
        assert!(
            woke,
            "a hardware interrupt fired and the thread waiting on it never woke",
        );
        assert!(
            wait_for(|| !crate::sched::is_thread_present(tid)),
            "the interrupt waiter never exited",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// **A spawn quota caps how many children a spawner can have alive, and replenishes on death.**
    ///
    /// This is the resource-exhaustion bound from the security audit: a process cannot make the
    /// kernel spawn without limit. Two threads block on an rendezvous nobody drains, holding their
    /// slots; a budget of two is then exhausted and a third spawn is refused. Waking one lets it
    /// exit and be reaped, which returns its slot, and a spawn succeeds again.
    #[test_case]
    fn a_spawn_quota_caps_live_children_and_replenishes_on_reap() {
        use core::sync::atomic::AtomicU32;
        static BUDGET: AtomicU32 = AtomicU32::new(2);

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

        // Two children that block forever (nobody sends), each holding a quota slot.
        let first = super::spawn_with_quota(&BUDGET, move || {
            super::ipc_receive(ep);
        });
        assert!(first.is_some(), "first child should fit in the budget",);
        let second = super::spawn_with_quota(&BUDGET, move || {
            super::ipc_receive(ep);
        });
        assert!(second.is_some(), "second child should fit in the budget",);

        // Let them run and block, so both slots are genuinely held.
        for _ in 0..50 {
            super::yield_now();
        }

        // The budget is spent: a third spawn is refused, not panicked, not over-committed.
        assert!(
            super::spawn_with_quota(&BUDGET, || {}).is_none(),
            "the budget was exhausted but a third child spawned anyway",
        );

        // Wake one child. It returns from ipc_receive, its closure ends, it exits and is reaped,
        // and its QuotaToken drops, returning the slot. Clock-bounded (milestone 81): the 100
        // yields this used to spend are microseconds on the physical core, well before another
        // core has run the woken child to completion.
        super::ipc_send(ep, [0, 0, 0]);
        // Clock-bounded, not yield-bounded: see `wait_for`. The slot comes back when the child is
        // *reaped*, which happens on whichever core it ran on, so a yield count here measures this
        // core's idleness rather than that child's teardown. Waiting on the budget itself is also
        // the exact property: the assertion below is the confirmation, not the wait.
        assert!(
            wait_for(|| BUDGET.load(Ordering::Relaxed) > 0),
            "a child exited but its quota slot was never returned to the budget",
        );

        // A slot is free again.
        assert!(
            wait_for(|| super::spawn_with_quota(&BUDGET, || {}).is_some()),
            "a child exited but its quota slot was never returned",
        );

        // Clean up: wake the other blocked child so it does not sit forever.
        super::ipc_send(ep, [0, 0, 0]);
        assert!(
            wait_for(|| !crate::sched::is_thread_present(first.unwrap())
                && !crate::sched::is_thread_present(second.unwrap())),
            "this test's own children had not finished when it returned",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// A signal that arrives while nobody is waiting is **remembered, not lost.** An interrupt is
    /// not a rendezvous: if it fires a hair before the driver calls `WAIT`, the driver must still
    /// see it. The `pending` count is what closes that window.
    ///
    /// Raised the same two ways as `an_interrupt_becomes_a_message`, with the same caveat about
    /// what each ISA's raise does and does not cost (see [`raise_test_irq`]).
    #[test_case]
    fn an_interrupt_that_arrives_before_the_wait_is_not_lost() {
        use crate::arch::exceptions::ROUTED_IRQS;

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");
        super::bind_irq(pending_irq(), ep);
        arm_test_irq(pending_irq());

        // Fire it with NOBODY waiting. The signal must be counted.
        let routed = ROUTED_IRQS.load(Ordering::Relaxed);
        raise_test_irq(pending_irq());
        // Wait for the handler to have actually run, rather than for a fixed number of yields: a
        // yield elapses in no real time on an idle core (DECISIONS §28), and under SMP the interrupt
        // may be taken on another core entirely, so counting yields here would be counting nothing.
        let delivered = spin_until(|| ROUTED_IRQS.load(Ordering::Relaxed) > routed);
        quiet_test_irq();
        assert!(
            delivered,
            "the interrupt was raised but the handler never routed it, so this test could not \
             reach the question it exists to ask",
        );

        static SAW: AtomicBool = AtomicBool::new(false);
        let tid = super::spawn(move || {
            super::ipc_receive(ep); // must return immediately: the signal is pending
            SAW.store(true, Ordering::SeqCst);
        })
        .expect("spawn failed");

        assert!(
            spin_until(|| SAW.load(Ordering::SeqCst)),
            "an interrupt that fired before the WAIT was lost",
        );
        assert!(
            wait_for(|| !crate::sched::is_thread_present(tid)),
            "the interrupt waiter never exited",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// **An interrupt raised while its line is masked is delivered when the driver ACKs.** The
    /// third delivery property, and the one a real driver leans on hardest: the handler masks a
    /// routed line when it fires and the driver's `Irq::ACK` unmasks it, so anything the device
    /// raises while the driver is busy arrives during the mask, and the ACK is the only thing that
    /// can deliver it. Nothing else will: the driver goes straight back to `Irq::WAIT`.
    ///
    /// The RISC-V leg is the reason this test exists. QEMU's PLIC does not re-evaluate delivery when
    /// an enable bit is written, so a source that went pending while disabled sat pending, enabled
    /// and undelivered after the ACK, until some *other* PLIC event happened to re-evaluate it. The
    /// USB keyboard stalled mid-line that way about one boot in thirty, and a byte typed on the UART
    /// released it (`drivers::plic::enable`'s doc has the mechanism; notes/usb.md has the
    /// history). This test raises nothing else in between, which is what makes it deterministic.
    ///
    /// The line is lowered and raised again while masked, rather than held, because QEMU's PLIC
    /// latches a source's pending bit only when its line rises: a line held high across the
    /// claim would never go pending again under the emulator at all, fixed or not, and the test
    /// would be asking a different question. On x86_64 the self-IPI has no mask, so the second
    /// raise is simply delivered; the property holds there trivially and the leg proves the
    /// portable half (two raises, two messages).
    ///
    /// Name: provisional (the USB keyboard lost-wakeup lane, 2026-10-05).
    ///
    /// Falsification: replayable `kernel/falsifications/sched.tests.an_interrupt_raised_while_its_line_is_masked_is_delivered_at_the_ack.patch`
    #[test_case]
    fn an_interrupt_raised_while_its_line_is_masked_is_delivered_at_the_ack() {
        use crate::arch::exceptions::ROUTED_IRQS;

        let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
        let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");
        super::bind_irq(pending_irq(), ep);
        arm_test_irq(pending_irq());

        // The first raise: the handler routes it and masks the line, exactly as for a driver.
        let routed = ROUTED_IRQS.load(Ordering::Relaxed);
        raise_test_irq(pending_irq());
        let first = spin_until(|| ROUTED_IRQS.load(Ordering::Relaxed) > routed);
        quiet_test_irq();
        assert!(
            first,
            "the first interrupt was never routed, so this test could not reach its question",
        );
        // **Let the first handler finish.** `ROUTED_IRQS` moves before the handler's `complete`,
        // and on RISC-V a completion re-evaluates the PLIC. If that hart's completion landed after
        // the ACK below, it would deliver the second interrupt itself and the test would pass for
        // a reason that is not the ACK, which is how the first falsification run came back green.
        // The completion is a few instructions behind the count; 20 ms is margin, not a guess at it.
        let settle = crate::arch::timer::now() + crate::arch::timer::frequency() / 50;
        while crate::arch::timer::now() < settle {
            super::yield_now();
        }

        // The second raise lands while the line is masked: the device spoke while its driver was
        // busy. Then the driver's ACK, which is `arch::irq::enable`, the same call `Irq::ACK` makes.
        let routed = ROUTED_IRQS.load(Ordering::Relaxed);
        raise_test_irq(pending_irq());
        arm_test_irq(pending_irq());
        let second = spin_until(|| ROUTED_IRQS.load(Ordering::Relaxed) > routed);
        quiet_test_irq();
        assert!(
            second,
            "an interrupt raised while its line was masked was never delivered after the ACK \
             unmasked it: the driver would sleep in WAIT with its device's interrupt pending",
        );

        // Both are messages: a driver waiting now collects two signals and does not block.
        static SAW: AtomicU64 = AtomicU64::new(0);
        let tid = super::spawn(move || {
            super::ipc_receive(ep);
            super::ipc_receive(ep);
            SAW.store(2, Ordering::SeqCst);
        })
        .expect("spawn failed");
        assert!(
            spin_until(|| SAW.load(Ordering::SeqCst) == 2),
            "two interrupts were routed but the waiter did not collect two signals",
        );
        assert!(
            wait_for(|| !crate::sched::is_thread_present(tid)),
            "the interrupt waiter never exited",
        );
        crate::sched::reclaim_region(region).expect("test region would not reclaim");
    }

    /// The kernel's rendezvous supply grows past one chunk, and a retired chunk's endpoints keep working.
    ///
    /// This exists because `KERNEL_EP_PAGES` used to be a ceiling that grew with the *test suite*
    /// rather than the system, so every few merges someone hit a panic telling them to raise a
    /// constant for a reason no single branch had caused. Growth on demand retires that, and this test
    /// is what keeps it retired.
    ///
    /// Creating `KERNEL_EP_CHUNK_PAGES + 1` endpoints crosses a chunk boundary wherever in the current
    /// chunk we happen to start, so the carve-a-new-chunk path is exercised rather than assumed. The
    /// second assertion is the one that matters more: an rendezvous minted *before* the transition must
    /// still resolve afterwards, which is what proves that forgetting a filled chunk's handle
    /// (deliberate, see the field's doc comment) does not orphan the endpoints living in it.
    #[test_case]
    fn the_kernels_rendezvous_supply_grows_past_one_chunk() {
        let mut names = [0u64; super::KERNEL_EP_CHUNK_PAGES as usize + 1];
        for slot in names.iter_mut() {
            *slot = super::create_rendezvous();
        }

        // Distinct names: a chunk transition that handed back the same page twice would show here.
        for (i, &a) in names.iter().enumerate() {
            for &b in &names[i + 1..] {
                assert_ne!(a, b, "two endpoints share a name across a chunk transition");
            }
        }

        // Every one still resolves, including the earliest, which is in a chunk we have since retired.
        let mut guard = super::IPC_TABLES.lock();
        let sched = guard.as_mut().expect("no scheduler");
        for (i, &ep) in names.iter().enumerate() {
            assert!(
                super::rendezvous_of(sched, ep).is_some(),
                "rendezvous {i} stopped resolving after the supply grew",
            );
        }
    }
}
