//! Kernel threads: a [`KernelStack`] with an unmapped guard page beneath it, and one saved stack
//! pointer naming where the thread's registers are on that stack. See notes/threads.md and
//! notes/thread-control-block.md.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use page_frames::FRAME_SIZE;
use paging::Flags;

use crate::arch::mmu;
use crate::sync::{IrqSafeMutex, rank};

pub type ThreadId = u64;

/// A kernel thread stack's size in pages: 24 KiB. The measured worst case at the suite's debug
/// codegen (the deepest path, the residue of blocking there, and one preemption at that point) is
/// about 15.5 KiB, which Linux's 16 KiB does not hold under load; six pages leave about 8 KiB above
/// it. notes/threads.md has the arithmetic. The guard page turns an overflow into a fault, and the
/// high-water gate (`stack.rs`) alarms well before it.
pub(crate) const STACK_PAGES: usize = 6;

/// Where kernel thread stacks live, virtually: the architecture's answer, because the address is
/// the architecture's (rule 1). It must be far above the direct map, so a stack address never
/// collides with the virtual name of a physical page. See each `arch::mmu::THREAD_STACK_AREA`.
const STACK_AREA: u64 = mmu::THREAD_STACK_AREA;

/// One thread's slot in [`STACK_AREA`]: the guard page, then [`STACK_PAGES`] of stack. Every slot
/// is this wide and every base is a multiple of it from `STACK_AREA`, including the reused ones
/// (`FREE_STACK_ADDRESS_SPACE` hands back the slot base, never an interior address), which is what lets a
/// fault handler turn an address back into "slot N, this far into its guard page". See
/// [`crate::stack::guard_page_at`].
pub(crate) const STACK_SLOT_SPAN: u64 = (STACK_PAGES as u64 + 1) * FRAME_SIZE;

static NEXT_STACK_VA: AtomicU64 = AtomicU64::new(STACK_AREA);

/// The thread-stack area as `(base, watermark)`: every stack slot ever handed out lies below the
/// watermark, and nothing else in the kernel map lies in the span at all.
///
/// Reads one relaxed atomic and nothing else, deliberately: the caller is a fault handler that has
/// already lost the machine, so it may not take a lock. A slot allocated concurrently on another
/// core can be missing from the range, which costs a diagnosis and never a wrong one.
pub(crate) fn stack_area_span() -> (u64, u64) {
    (STACK_AREA, NEXT_STACK_VA.load(Ordering::Relaxed))
}

/// The `id` a constructor writes before the thread table has named the thread. Deliberately
/// `u64::MAX` (= `cpu::NO_TID`), which the generational table can never mint, so a thread that
/// somehow escaped naming resolves to nothing instead of to slot 0. Every insert path overwrites
/// it via `Table::insert_with` (milestone 14 phase A; design/kernel-objects-from-untyped.md).
pub(crate) const UNNAMED: ThreadId = u64::MAX;

/// Stack address ranges from threads that have exited, handed to the next thread so it lands in
/// page tables that already exist. `unmap_page` leaves intermediate tables standing, so
/// bump-allocating addresses forever would cost an L2 and an L3 table for every 2 MiB consumed. A
/// test asserts that a second batch of threads costs zero additional frames.
static FREE_STACK_ADDRESS_SPACE: IrqSafeMutex<FreeAddressSpace> =
    IrqSafeMutex::new(rank::STACK_VA, FreeAddressSpace::new());

/// A fixed stack of reusable stack-VA ranges. A range is pushed when a thread dies and popped when
/// one spawns, so the count never exceeds the most threads alive at once; the array is sized from
/// [`crate::sched::MAX_THREADS`], and the debug assert is the cross-check.
struct FreeAddressSpace {
    vas: [u64; crate::sched::MAX_THREADS],
    len: usize,
}

impl FreeAddressSpace {
    const fn new() -> Self {
        Self {
            vas: [0; crate::sched::MAX_THREADS],
            len: 0,
        }
    }

    fn pop(&mut self) -> Option<u64> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.vas[self.len])
    }

    fn push(&mut self, va: u64) {
        debug_assert!(
            self.len < self.vas.len(),
            "more dead stack ranges than MAX_THREADS"
        );
        if self.len < self.vas.len() {
            self.vas[self.len] = va;
            self.len += 1;
        } // else: leak the VA range rather than corrupt; unreachable per the bound above
    }
}

/// The saved thread context (`arch::Context`) and the context switch (`arch::switch_to`) are
/// arch-specific by nature: a context *is* a particular CPU's callee-saved register set. `thread.rs`
/// treats a `Context` as opaque, it only stores one and hands it to `switch_to`, and builds a fresh
/// one through the two `for_*_thread` constructors in `arch`. Re-exported here so the thread
/// subsystem's callers (`sched`) keep naming them through `crate::thread`. See notes/riscv-port.md.
pub use crate::arch::{Context, switch_to};

/// A stack, with an unmapped page beneath it.
///
/// The frame list is a fixed array (milestone 14 phase B.1): a kernel stack is always exactly
/// [`STACK_PAGES`] frames, so there was never anything dynamic about it but the container.
pub struct KernelStack {
    guard: u64,
    bottom: u64,
    top: u64,
    /// The physical pages backing the stack, from the kernel's own budget (`kmem`, milestone
    /// 19c.1). Physical addresses, not `PageFrame`s, because they belong to the kernel object
    /// region and return to it (recycled) rather than to the frame allocator: the kernel's
    /// stack spending is bounded by a boot carve now, not open-ended. `0` marks a page that was
    /// never mapped (a partial-build failure path).
    pages: [u64; STACK_PAGES],
}

impl KernelStack {
    pub fn new() -> Option<Self> {
        // The guard page's address is simply never mapped, which is the whole mechanism.
        let span = STACK_SLOT_SPAN;

        // A dead thread's range comes first: its page tables already exist.
        let base = FREE_STACK_ADDRESS_SPACE
            .lock()
            .pop()
            .unwrap_or_else(|| NEXT_STACK_VA.fetch_add(span, Ordering::Relaxed));

        let guard = base;
        let bottom = base + FRAME_SIZE;
        let top = bottom + STACK_PAGES as u64 * FRAME_SIZE;

        let mut pages = [0u64; STACK_PAGES];
        for (i, slot) in pages.iter_mut().enumerate() {
            // From the kernel's own budget (`kmem`), not the frame allocator, so stacks stay inside
            // the kernel's boot carve.
            let Some(phys) = crate::kmem::page() else {
                return None; // `pages` so far are recorded; Drop recycles what we did map
            };
            let va = bottom + i as u64 * FRAME_SIZE;

            if mmu::map_page(va, phys, Flags::kernel_data()).is_err() {
                crate::kmem::recycle(phys); // never mapped: straight back to the budget
                return None; // Drop handles the earlier, mapped pages
            }
            *slot = phys;
        }

        // Paint the whole stack for the high-water report; no thread has run on it yet.
        //
        // SAFETY: the loop above mapped every page of `[bottom, top)` and returned early on any
        // failure, and this `KernelStack` has not been handed to a thread yet, so nothing is on it.
        #[cfg(any(test, feature = "system_tests"))]
        unsafe {
            crate::stack::paint(bottom, top);
        };

        Some(KernelStack {
            guard,
            bottom,
            top,
            pages,
        })
    }

    /// Where `sp` starts. The stack grows **down** from here (notes/stack.md).
    pub fn top(&self) -> u64 {
        self.top
    }

    /// The unmapped page below the stack. Test support.
    #[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
    pub fn guard(&self) -> u64 {
        self.guard
    }

    /// The lowest usable byte. Test support.
    #[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
    pub(crate) fn bottom(&self) -> u64 {
        self.bottom
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        // Measure before unmapping, on the reaper's stack rather than this one. A partial build has
        // unmapped pages a scan would fault on, and was never painted or used.
        #[cfg(any(test, feature = "system_tests"))]
        if self.pages.iter().all(|&p| p != 0) {
            // SAFETY: every page is mapped (the `all` above is exactly that check), still mapped
            // because the unmap loop below has not run, and `new` painted the whole span.
            let used = unsafe { crate::stack::high_water(self.bottom, self.top) };
            crate::stack::note_thread_stack_use(used);
        }

        for (i, &phys) in self.pages.iter().enumerate() {
            if phys == 0 {
                continue; // a page a failed build never mapped
            }
            let va = self.bottom + i as u64 * FRAME_SIZE;

            // `unmap_page` must invalidate the translation: this address goes to a different
            // thread's stack next, and a stale entry would expose the dead thread's saved registers
            // (notes/page-tables.md).
            if mmu::unmap_page(va).is_ok() {
                crate::kmem::recycle(phys); // home to the kernel budget, not the frame allocator
            }
        }

        // The physical pages went back above; this returns the address range.
        FREE_STACK_ADDRESS_SPACE.lock().push(self.guard);
    }
}

/// Where a thread is in its life: `thread_wake_handshake::RunState`, whose transitions live in
/// `crates/wake_handshake` so loom can search them (notes/interleaving.md).
pub use thread_wake_handshake::RunState as State;

/// **Which side of a rendezvous a blocked thread is waiting as.** The role half of the
/// handshake's [`wait_on`](thread_wake_handshake::Handshake::wait_on) payload, recorded at the same
/// instant `state` goes `Blocked` and by the same code,
/// so a hang dump can say *what kind* of wait a thread is in rather than only that it waits.
///
/// `Reply` is a `CALL` caller: it is waiting for its one-shot Reply capability to be invoked, and
/// (in the rendezvous-met case) it sits on **no** endpoint queue at all, which is exactly the wait
/// a dump could previously not distinguish from a lost wakeup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitRole {
    /// Parked on an endpoint's sender queue (a `SEND`/`SEND_CAP` with no receiver, or a corpse
    /// holding its death message).
    Sender,
    /// Parked on an endpoint's receiver queue (a `RECEIVE`/`RECEIVE_CAP` with nothing to take).
    Receiver,
    /// A `CALL` caller blocked until `REPLY`; queued as a sender only if no server was waiting.
    Reply,
}

/// **A reserved slot in a spawner's resource budget, returned when this thread dies.**
///
/// A process (the shell, say) that spawns children can be given a quota: at most N children alive
/// at once. Reserving a slot is an atomic decrement; a `QuotaToken` holds that reservation, and
/// its `Drop` gives it back. Because the token lives inside the `Thread`, the slot is returned at
/// exactly the moment the reaper drops the thread: a well-behaved child that exits frees its slot,
/// and a child that blocks forever keeps holding it, which is correct: it is still consuming a
/// thread, a stack, and an address space. This is what bounds kernel memory against a spawn flood
/// or a leaked-thread accumulation without any per-tick bookkeeping. See notes/quotas.md.
pub struct QuotaToken(&'static AtomicU32);

impl QuotaToken {
    /// Called only by `sched::spawn_with_quota`, which has no caller of its own today.
    #[allow(dead_code)]
    pub fn new(budget: &'static AtomicU32) -> Self {
        QuotaToken(budget)
    }
}

impl Drop for QuotaToken {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

/// What a blocked thread waits on. The payload of [`thread_wake_handshake::Handshake::wait_on`],
/// opaque to that crate, matched by the kernel (`ipc_reply`'s reply-role check, the teardown that
/// unlinks a blocked thread, the hang dump's wait column). An enum, so a notification's name can
/// never be read as a rendezvous name and resolved in the wrong registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// Parked in IPC on a rendezvous, as the given side of it.
    Rendezvous(crate::sched::RendezvousId, WaitRole),
    /// Parked in `WAIT` on a notification (milestone 151), on its wait queue.
    Notification(crate::sched::NotificationId),
    /// Parked in `AddressSpace::WAIT` on a futex (milestone 812 (`std::thread::spawn` runs real
    /// threads in one address space)), under this key in the scheduler's futex table. The key is
    /// the whole identity: there is no futex object to name.
    Futex(inter_process_communication::futex::Key),
    /// **Stopped by `ThreadControlBlock::SUSPEND`** (DECISIONS §300), on no queue: only `RESUME`
    /// serves this wait, so every other wake is refused by the handshake's gate. A thread parks
    /// here only from `Ready` or `Running`; one suspended inside an IPC finishes that wait first.
    /// Name: provisional (lane/48-tcb-suspend-resume, 2026-10-18 UTC).
    Resume,
}

pub struct Thread {
    pub id: ThreadId,

    /// The block/wake handshake: `state` plus the `on_cpu`, `wake_pending`, `wait_on`, `ipc_served`
    /// and `ipc_aborted` protocol, embedded from `crates/wake_handshake` so the kernel calls the
    /// transitions loom checks. Every access is under `IPC_TABLES`; the crate's header carries the
    /// protocol's rules, races and BUGS.
    pub handshake: thread_wake_handshake::Handshake<Wait>,

    /// Which core this thread last ran on, or [`u8::MAX`] if it has never run: written at every
    /// `switch_in` to count migrations, which `trace::Event::PlaceRemote` cannot see because a
    /// rendezvous wake places its peer on the waker's core. Behind `soak_test`, because the write
    /// sits in `schedule()`'s switch and costs `ipc_fastpath` more footprint than
    /// `script/fastpath-footprint` allows. See notes/thread-control-block.md.
    #[cfg(feature = "soak_test")]
    pub last_cpu: u8,

    /// Which core this thread was placed on when it started, or [`u8::MAX`] if it has not started:
    /// the fact `abi::survey::record::PLACEMENT` reports. Written once, at
    /// [`crate::sched::spawn_on`] and [`crate::sched::start_thread_control_block`], never on the
    /// IPC path, so unlike `last_cpu` it is unconditional. Placement, not location: a stolen or
    /// woken thread still reports where it was placed. A cpu id, not an index into a range
    /// (`cpu_set`). See notes/thread-control-block.md.
    ///
    /// Name: provisional. calef names public items.
    pub placement: u8,

    /// The saved general-purpose state of this thread: one stack pointer. Everything the calling
    /// convention preserves is on the stack it points at, pushed there by `switch_to`. The FP/SIMD
    /// register file is neither here nor on that stack: it is in the TCB page ([`fp_state_of`]).
    pub context: *mut Context,

    /// The kernel stack, `None` for the boot thread, which runs on the stack `boot.s` set up. Never
    /// read: it exists to be dropped, and its `Drop` unmaps and frees the stack (§38 (a suppression
    /// is scoped to an item and carries a reason), disposition 3). The reaper takes it out under
    /// `IPC_TABLES` and drops it after releasing the lock, because each unmap's TLB shootdown is an
    /// inter-processor interrupt on riscv64 and x86_64; [`Thread::being_reaped`] keeps the thread
    /// from looking gone meanwhile.
    #[allow(dead_code)]
    pub stack: Option<KernelStack>,

    /// This thread's kernel stack and address space have been taken out and are being freed without
    /// `IPC_TABLES` held (name provisional). Set by the reaper between taking them and removing the
    /// thread, which stays in the table, `Finished`, meanwhile. Region teardown reads it as "a core
    /// is still standing on this stack" and refuses `DESTROY` until the reaper is done, because an
    /// owner that sees `DESTROY` succeed may spend the memory at once
    /// (notes/job-mix/null-syscall-under-load.md).
    pub being_reaped: bool,

    /// The low half of memory, as far as this thread is concerned; `None` for a kernel thread. The
    /// context switch installs it on the way in, because the translation-table register is global
    /// and threads are not (notes/userspace.md). A copy ([`crate::user::BoundSpace`]), not the
    /// space: the address-space registry owns every space, and the switch must not take its lock.
    /// The reaper takes the space out of the registry by this name unless the region sweep took it
    /// first.
    pub space: Option<crate::user::BoundSpace>,

    // The capability table is not a field: it lives past this struct in the same page, behind its
    // own lock. See [`capability_table_of`].
    /// The IPC message this thread most recently sent or received: five words. A sender parks its
    /// message here before blocking and the receiver collects it later, so it cannot be a stack
    /// local. Ordinary IPC uses three words; the top two carry the five-word fault/exit message a
    /// corpse delivers to its supervisor (§26 (the fault endpoint), abi's `fault` module).
    pub mailbox: [u64; 5],

    /// **A slot in a spawner's quota, or `None` for a thread nobody bounded.** Reaped with the
    /// thread, which is how the slot comes back. See [`QuotaToken`].
    ///
    /// Never read, like [`Self::stack`]: it exists to be dropped, and its `Drop` returns the slot.
    /// It is `None` on every thread today, because `sched::spawn_with_quota` has had no caller since
    /// §28 retired the kernel-wired shell; see that function for why the mechanism stays.
    #[allow(dead_code)]
    pub quota: Option<QuotaToken>,

    /// A capability parked here mid-delegation: a `SEND_CAP` with no receiver waiting blocks with
    /// it here, and the receiver `take()`s it into its own table. `None` for every ordinary send.
    ///
    /// # BUGS
    ///
    /// - Latent: the teardown paths never clear this slot, safe only while every wake of a parked
    ///   sender goes through `set_ipc_aborted` (notes/thread-control-block.md).
    pub outgoing_cap: Option<crate::cap::Cap>,

    /// Did the delivery this thread is about to read install a capability? Set by the paths that
    /// put one in this thread's table while it was parked in `RECEIVE_CAP` (`ipc_send_cap`,
    /// `ipc_call_badged`), cleared when it parks there. `ipc_receive_cap` reads it to decide `x1`:
    /// a delivery that installed nothing returns `NO_CAP`, never the sender's data word, which a
    /// `CALL` server would read as a reply slot (fatal risk 7). See `sched::ipc_receive_cap`.
    pub cap_delivered: bool,

    /// Is the receive this thread is parked in a `RECEIVE_CAP`? Set at `ipc_receive_cap`'s park and
    /// cleared at its resume, so a plain `RECEIVE` parks with it `false`. A sender meeting a parked
    /// receiver reads it: a plain `RECEIVE` never takes a capability, whichever side arrived first
    /// (§246 (a plain `RECEIVE` never takes a capability)). Meaningful only while the thread is
    /// parked as a receiver. See `sched::ipc_send_cap`.
    // Name: provisional, this lane's; calef names public items.
    pub receiving_cap: bool,

    /// **Why the last aborted send was aborted, when the reason was a refusal** (milestone 603
    /// (provisional), DECISIONS §101 (notification objects) ruling B). Set beside `handshake.abort()` when a `SEND`,
    /// `SEND_CAP` or `CALL` named a rendezvous that carries an interrupt, and read-and-cleared by
    /// the syscall layer only after `take_ipc_aborted` has already said `true`. So an IPC that was
    /// not aborted never reads it, which is what keeps the refusal off the fastpath: the syscall
    /// layer's common case is the one branch it already had.
    pub ipc_refused: bool,

    /// **The intrusive queue link** (milestone 14 phases A.2/A.3; notes/intrusive-queues.md).
    /// When this thread is on a run queue, a migration inbox, or an endpoint wait queue, this
    /// points at the next thread in it; `None` otherwise. One link, so a thread can be on at most
    /// one queue, which is not a limitation but the scheduler's state machine made physical:
    /// Ready threads are on exactly one run queue or inbox, Blocked threads on at most one
    /// endpoint queue, Running/Finished threads on none. Touched only by the queue that holds
    /// the thread, under that queue's synchronization.
    pub(crate) next: Option<core::ptr::NonNull<Thread>>,

    /// This thread's queue token while no queue holds it (`intrusive_fifo::Unqueued`). Every live
    /// thread has one token, minted when the table inserts it, and it is always on a queue or here.
    /// A queue push takes it by value, so a thread reaches a queue only through the token its last
    /// transition left here. Here means on no queue: running, an embryo, awaiting a Reply, a
    /// deferred wake, a corpse, or an idle thread off its CPU. See notes/thread-control-block.md.
    ///
    /// Name: provisional (milestone 139, round 10): calef names fields a reader meets.
    pub(crate) own_token: Option<intrusive_fifo::Unqueued<Thread>>,

    /// **Where this thread's EL0 execution begins** (milestone 19c.3), set by `ThreadControlBlock::CONFIGURE`
    /// on an embryo, consumed by `START` to build the entry context. `(0, 0)` for a kernel
    /// thread, which never drops to EL0 and runs its closure instead.
    pub(crate) entry: (u64, u64), // (entry_va, user_sp)

    /// May this thread read the CPU's cycle counter from user mode. Set once, on an embryo, by
    /// `sched::grant_cycle_counter`, so a program cannot acquire a timing instrument it was not
    /// created with; no syscall method sets it, so it is `false` on every thread. `sched::schedule`
    /// hands it to `arch::timer::set_cycle_counter_grant`, which writes only on a change. Built
    /// only under `test` or `cycle_counter_grant`, whose cost `kernel/Cargo.toml`'s feature block
    /// records. Field name provisional.
    #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
    pub(crate) cycle_counter_grant: bool,

    /// The x86 I/O ports this thread may reach from ring 3, as `(base, count)`, or `None`. Set when
    /// a `PortRange` capability is inserted (`sched::thread_control_block_insert_cap`), cleared by
    /// `sched::delete_port_range_caps*` and by `sched::delete_current_cap`. `sched::schedule` hands
    /// it to `arch::segments::set_port_range_grant`, which rewrites the CPU's TSS I/O bitmap only
    /// when it differs, so a switch between non-holders costs one compare. `x86_64` only. Field
    /// name provisional.
    #[cfg(target_arch = "x86_64")]
    pub(crate) port_range_grant: Option<(u16, u16)>,

    /// **The child's initial `x0`, `x1`, `x2`** (milestone 19d/19e): the words `START` hands the
    /// new EL0 thread in its first registers, so a loader can pass a child its role plus data (a
    /// worker's input, a driver's DMA address). All zero for a kernel thread.
    pub(crate) start_args: [u64; 3],

    /// This thread's thread pointer (`TPIDR_EL0`, `tp` or the `FS` base), which a program's
    /// thread-local storage hangs off. `CONFIGURE` gives the first value,
    /// `ThreadControlBlock::SET_THREAD_POINTER` changes it, and `sched::schedule` installs it at
    /// every switch in through `arch::thread_pointer::hand_over`. A user address or zero, which
    /// every writer checks, because `x86_64`'s `wrmsr` faults on a non-canonical value. Where
    /// userspace can write the register itself, this copy can lag while the thread runs;
    /// `arch::thread_pointer` says which copy is live. Field name provisional.
    pub(crate) thread_pointer: u64,

    /// **The process this thread joined**, or `None` for a thread that is a process of its own
    /// (milestone 812 (`std::thread::spawn` runs real threads in one address space), §269 (how
    /// threads share a process) forks 3 and 5). Set once, at a `CONFIGURE` given a process
    /// capability, and cleared only if the process's page is reclaimed before this embryo starts.
    /// *(Field name provisional.)*
    pub(crate) process: Option<crate::sched::ProcessId>,

    /// **Where this thread's capabilities are**: its own table, in its own TCB page, or its
    /// process's, in the process's page (milestone 812). Written by `init_capability_table` and by
    /// the join, both under `IPC_TABLES`, and read only through [`capability_table_of`] under the
    /// same lock or by the core this thread runs on. A process page is never reclaimed while a
    /// member points here (`sched::process`), so the pointer cannot outlive what it names.
    /// *(Field name provisional.)*
    pub(crate) capability_table: *const CapabilityTableLock,

    /// **This thread's current-CPU page slot in its process's space**, as `(space name, slot)`, or
    /// `None` for a thread that is a process of its own and reads its space's slot 0 (milestone 812,
    /// §269 fork 6). Given back when the thread is reaped. *(Field name provisional.)*
    pub(crate) thread_page: Option<(u64, u8)>,

    /// **Did this thread's TCB page come from `kmem`** (recycle it on death) or from a user
    /// process's own region (leave it; the region reclaims it at destroy)? True for every
    /// kernel-created thread; false for a user-retyped TCB (19c.3). The page-origin half of the
    /// same owned-vs-borrowed question kernel stacks answered with "one owner" (notes/thread-control-block.md).
    pub(crate) thread_control_block_kmem: bool,

    /// **Marked for forcible teardown** (DECISIONS §16 amendment): a region's owner called
    /// `MemoryRegion::DESTROY` while this thread was still live in it, so the thread is doomed. The
    /// scheduler converts a killed thread to a corpse at its next preemption instead of requeueing
    /// it (see `schedule`), so a runaway that never checks its endpoint is torn down without
    /// yanking it out of a queue or stopping another core: each core reaps its own on the timer.
    /// This is the forcible tier of `^C` (§24), where the shell's escalation retries `DESTROY`
    /// until the killed thread has self-terminated and the region is object-free.
    pub(crate) killed: bool,
    /// **`ThreadControlBlock::SUSPEND` has been ordered and `RESUME` has not** (DECISIONS §300 (the
    /// paused state, a TCB verb, silent)).
    /// The order is recorded here and carried out where the thread next meets the scheduler:
    /// `schedule` parks a running thread that carries it, and the pick parks a popped one rather
    /// than switching into it, so no path to a CPU runs a thread with this set. A thread parked
    /// that way is `Blocked` on [`Wait::Resume`]. `notes/suspend-and-resume.md` has the cases.
    /// Name: provisional (lane/48-tcb-suspend-resume, 2026-10-18 UTC).
    pub(crate) suspend_requested: bool,
    /// **An armed kill waits** (`sched::defer_kill`): this kernel thread is building a user address
    /// space that is still a local on its own stack, and a thread converted to a corpse here is never
    /// resumed, so that local would never be dropped. `schedule` leaves [`killed`](Self::killed)
    /// armed while this is set, and `sched::allow_kill` carries it out once the space is adopted and
    /// the reaper can free it. Name: provisional (lane/frame-ledger-overrun, 2026-10-22 UTC).
    pub(crate) kill_deferred: bool,
    /// **Where this thread's fault/exit is reported** (milestone 22, DECISIONS §26), or `None` for
    /// an unsupervised thread. Set once at `START` from the child's reserved fault slot (abi's
    /// `FAULT_EP_SLOT`) and never afterward: supervision is granted at spawn only, so the
    /// relationship is fixed and visible in how the thread was built (§26.2). When the thread
    /// faults or exits, the kernel delivers a five-word message here and the corpse goes `Dead`
    /// until reaped; a thread with `None` dies and is reaped immediately, today's behavior.
    pub(crate) fault_ep: Option<crate::sched::RendezvousId>,
    /// The label the builder set on this thread's supervision capability, or `0` (§148 (resolves by
    /// asking the kernel), ruling R3). Read at `START` from the badge in the reserved fault slot,
    /// which is consumed then, so the child never learns it. It travels only with the death
    /// message: `depart` hands it to the supervisor in argument register 5 of a plain `RECEIVE`,
    /// beside the mailbox words, so ordinary IPC stores nothing more (§148's benchmark condition).
    ///
    /// Name: provisional, milestone 105 (the two forks)'s lane, 2026-10-05 (UTC). Chosen to sit beside `fault_ep` and `fault_msg`;
    /// `fault_badge` was considered and set aside, because §148 calls the word a label.
    pub(crate) fault_label: u64,

    /// **The untyped region this TCB's page was retyped out of** (DECISIONS §32), or `None` for a
    /// kernel-created thread whose page came from `kmem`. Recorded at `create_thread_control_block`, which is the one
    /// place the answer is known for certain, and it is what an endpoint reap reclaims: the same
    /// region name the region's owner would have passed to `MemoryRegion::DESTROY`, so there is one
    /// teardown path and not two. A supervisor never sees this number and holds no capability to it;
    /// naming the region is the kernel's job precisely because the supervisor cannot.
    ///
    /// It goes stale like any other region name (the slot's generation bumps at destroy), so a
    /// second reap of the same corpse finds nothing to reclaim rather than somebody else's region.
    pub(crate) thread_control_block_region: Option<u64>,

    /// **The fault/exit message this thread's corpse carries** (milestone 22), retained after death
    /// so a test can prove a `Dead` TCB still holds its fault-time state. Set when the thread dies
    /// with a `fault_ep`; `None` while it lives. The words are the §26 format
    /// `[event, tid, pc, addr, reserved]`, the same five the supervisor received.
    pub(crate) fault_msg: Option<[u64; 5]>,

    /// **The notification bound to this thread** (milestone 151, DECISIONS §101 (notification objects)), or `None`. Set
    /// once by `Notification::BIND`, never cleared: §101 has no unbind. A signal on it wakes this
    /// thread out of a receive on any endpoint, and a receive checks it on entry, which is the one
    /// load and one branch §101 priced onto the IPC fastpath.
    ///
    /// A generational name, so a destroyed notification leaves this `Some` and stale: every reader
    /// resolves it and treats a miss as unbound, which is also what lets `BIND` succeed again on a
    /// thread whose notification is gone. *(Field name provisional.)*
    pub(crate) bound_notification: Option<crate::sched::NotificationId>,
}

/// **Where a thread's FP/SIMD register file lives: the free space of its own TCB page**
/// (milestone 447, a thread's vector registers are its own).
///
/// Byte offset from the start of the page, which is also the address of the [`Thread`]. Rounded up
/// to `FpState`'s alignment, which is 16 on every architecture because the save instructions
/// require it (`stp q`, `fxsave`).
const FP_STATE_OFFSET: usize =
    size_of::<Thread>().next_multiple_of(align_of::<crate::arch::fp::FpState>());

/// The page is the bound, and the compiler checks it. A `Thread` is always built at the start of a
/// 4096-byte page it exclusively owns, because every route into the table (`insert_with` from
/// `kmem`, `insert_from_page` from a user region's object page) goes through `Threads::insert_at`
/// or `insert_at_in_place`. Its FP register file lives in the page's idle space past the struct.
/// Grow `Thread` past the point where the register file fits beside it and the kernel does not
/// build.
const _: () = assert!(
    FP_STATE_OFFSET + size_of::<crate::arch::fp::FpState>() <= paging::PAGE_SIZE as usize,
    "a Thread plus its FP register file no longer fits in one TCB page"
);

/// The address of `thread`'s FP register file.
///
/// # Safety
///
/// `thread` must be a pointer to a live `Thread` **at the start of its own TCB page**, as the table
/// stores it. Deriving this from a `&Thread` or `&mut Thread` would be wrong rather than merely
/// unidiomatic: a reference's provenance covers the struct, and this address is past its end. Take
/// the raw pointer out of the table (`Threads::pointer`) and pass that.
pub(crate) unsafe fn fp_state_of(thread: *mut Thread) -> *mut crate::arch::fp::FpState {
    // SAFETY: the caller's contract. The offset is inside the page by the assertion above, and the
    // result is aligned because `FP_STATE_OFFSET` is a multiple of `FpState`'s alignment and a page
    // is aligned to far more than that.
    unsafe { thread.cast::<u8>().add(FP_STATE_OFFSET).cast() }
}

/// Put a freshly-born thread's register file into its initial state.
///
/// Called once, by whichever `Threads` insert wrote the `Thread`, because those are the two places
/// that hold the page pointer. **A `kmem` page is not zeroed**, so without this a thread's `live`
/// flag would be whatever the last owner of the page left there, and `crate::fp::hand_over` would
/// read it on the very first switch.
///
/// # BUGS
/// Stale: `kmem::page` and `memory_region::retype_object_page` both zero the page they hand out.
///
/// # Safety
/// As [`fp_state_of`], and the register file must not already hold anything worth keeping.
pub(crate) unsafe fn init_fp_state(thread: *mut Thread) {
    // SAFETY: the caller's contract; one aligned write of a `Copy` value inside the page.
    unsafe { fp_state_of(thread).write(crate::arch::fp::FpState::INITIAL) };
}

/// Everything a thread can name, behind its own lock at `rank::CAPABILITY_TABLE`, so the running
/// thread's lookup does not take `IPC_TABLES` (name provisional). It starts empty: a fresh thread
/// can name nothing until somebody hands it something (§10 (process model: capability-based,
/// microkernel)). Userspace sees only a slot index, so unforgeability is a bounds check. The order
/// between the locks is written at the rank.
pub(crate) type CapabilityTableLock = crate::sync::IrqSafeMutex<crate::cap::CapabilityTable>;

/// **Where a thread's capability table lives: past its FP register file, in the same TCB page.**
///
/// **Outside the [`Thread`] struct, and that is the soundness argument rather than a layout
/// preference.** The running thread reads its own table with `IPC_TABLES` not held, while another
/// core holding `IPC_TABLES` may hold a `&mut Thread` for the very same thread (a revocation sweep's
/// `iter_mut`, an IPC writing its mailbox). A `&mut` asserts that nothing else touches any byte it
/// covers, interior mutability or not, so a lock that lived inside the struct would be read by one
/// core under another core's exclusive reference. Past the end of the struct, no reference to the
/// `Thread` covers it, which is the provenance rule [`fp_state_of`] already follows for the same
/// page.
const CAPABILITY_TABLE_OFFSET: usize = (FP_STATE_OFFSET + size_of::<crate::arch::fp::FpState>())
    .next_multiple_of(align_of::<CapabilityTableLock>());

/// The page is the bound for the table too, checked by the compiler as for the FP register file.
const _: () = assert!(
    CAPABILITY_TABLE_OFFSET + size_of::<CapabilityTableLock>() <= paging::PAGE_SIZE as usize,
    "a Thread, its FP register file and its capability table no longer fit in one TCB page"
);

/// **Nothing in a capability table needs dropping**, which is what lets `Threads::remove` recycle
/// the page after dropping only the `Thread`. Were a capability ever to own something, this fails
/// the build at the place that would otherwise leak it.
const _: () = assert!(
    !core::mem::needs_drop::<CapabilityTableLock>(),
    "a capability table now needs dropping; Threads::remove must drop it before recycling the page"
);

/// An empty table behind an unheld lock, the image [`write_empty_capability_table`] copies.
///
/// **A static, copied by pointer, and not a constant written with `ptr::write`.** The gate's -O0
/// build does not inline `write`, so a constant operand is first built into a temporary the size
/// of the table and then copied again into the page: two copies of 2 KiB on every thread's birth.
/// One copy from here took 1,194 instructions off each riscv64 `spawn_reap` iteration.
/// Never locked or borrowed as a lock: a copy of an unheld lock's bytes is a fresh unheld lock.
static EMPTY_CAPABILITY_TABLE: CapabilityTableLock = crate::sync::IrqSafeMutex::new(
    crate::sync::rank::CAPABILITY_TABLE,
    crate::cap::CapabilityTable::new(),
);

/// Write an empty, unheld capability table at `table`.
///
/// # Safety
///
/// `table` is valid for a write of a `CapabilityTableLock`, aligned, and reachable by no other core.
pub(crate) unsafe fn write_empty_capability_table(table: *mut CapabilityTableLock) {
    // SAFETY: the caller's contract for the destination; the source is a live static that nothing
    // writes, and the two cannot overlap because the destination is not a static.
    unsafe { core::ptr::copy_nonoverlapping(&raw const EMPTY_CAPABILITY_TABLE, table, 1) };
}

/// The address of `thread`'s capability table.
///
/// # Safety
///
/// As [`fp_state_of`]: `thread` must point to a live `Thread` **at the start of its own TCB page**,
/// as the table stores it, never one derived from a reference.
pub(crate) unsafe fn capability_table_of(thread: *mut Thread) -> *const CapabilityTableLock {
    // SAFETY: the caller's contract; one field read through the page pointer, no reference to the
    // `Thread` formed. Since milestone 812 (`std::thread::spawn` runs real threads in one address
    // space) the table may be its process's rather than its own, so the address is stored rather
    // than computed; `init_capability_table` writes it before the thread's name is handed out.
    unsafe { (*thread).capability_table }
}

/// **The table in `thread`'s own TCB page**, whether or not the thread uses it: the one a thread
/// that is a process of its own reads, and the one a joining embryo's capabilities move out of.
///
/// # Safety
///
/// As [`capability_table_of`].
pub(crate) unsafe fn own_capability_table_of(thread: *mut Thread) -> *const CapabilityTableLock {
    // SAFETY: the caller's contract. Inside the page by the assertion above, and aligned because the
    // offset is a multiple of the lock's alignment and a page is aligned to far more.
    unsafe { thread.cast::<u8>().add(CAPABILITY_TABLE_OFFSET).cast() }
}

/// Give a freshly-born thread its empty capability table. Called beside [`init_fp_state`] by both
/// `Threads` inserts, for its reason: a `kmem` page is not zeroed.
///
/// # BUGS
/// Stale, as [`init_fp_state`]'s: both page sources zero the page.
///
/// # Safety
///
/// As [`capability_table_of`], and no other core may yet be able to reach the table: the thread is
/// being born, so its name has not been handed to anyone.
pub(crate) unsafe fn init_capability_table(thread: *mut Thread) {
    // SAFETY: the caller's contract; one aligned write inside the page, over bytes nothing reads.
    unsafe {
        let own = own_capability_table_of(thread);
        write_empty_capability_table(own.cast_mut());
        (&raw mut (*thread).capability_table).write(own);
    }
}

// SAFETY: plain storage of the link, nothing else, which is all the queue's contract asks.
unsafe impl intrusive_fifo::Node for Thread {
    fn next(&self) -> Option<core::ptr::NonNull<Self>> {
        self.next
    }
    fn set_next(&mut self, next: Option<core::ptr::NonNull<Self>>) {
        self.next = next;
    }
}

// SAFETY: a Thread is only ever touched under IPC_TABLES.
unsafe impl Send for Thread {}

impl Thread {
    /// The thread we are already running on, at `sched::init`.
    ///
    /// It has no stack of its own (it uses the boot stack) and no saved context yet: the first
    /// `switch_to` *away* from it is what fills that in. Which is the neat part: a thread's
    /// context is written by the act of leaving it, so the boot thread needs no special case
    /// beyond a null placeholder.
    pub fn boot() -> Self {
        Thread {
            id: UNNAMED, // named 0 by the table's first insert (see generational_table::Table)
            handshake: thread_wake_handshake::Handshake::on_cpu_now(), // adopted mid-run: standing on its CPU
            #[cfg(feature = "soak_test")]
            last_cpu: u8::MAX,
            placement: u8::MAX, // overwritten by the placement decision at spawn or START
            context: core::ptr::null_mut(),
            stack: None,
            space: None,
            mailbox: [0; 5],
            quota: None,
            outgoing_cap: None,
            cap_delivered: false,
            receiving_cap: false,
            ipc_refused: false,
            being_reaped: false,
            next: None,
            own_token: None,
            entry: (0, 0), // a kernel thread; never enters EL0 by this path
            start_args: [0; 3],
            thread_pointer: 0,
            process: None,
            capability_table: core::ptr::null(),
            thread_page: None,
            thread_control_block_kmem: true,
            killed: false,
            kill_deferred: false,
            suspend_requested: false,
            fault_ep: None,
            fault_label: 0,
            thread_control_block_region: None,
            fault_msg: None,
            bound_notification: None,
            #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
            cycle_counter_grant: false,
            #[cfg(target_arch = "x86_64")]
            port_range_grant: None,
        }
    }

    /// Adopt the context a secondary core is already running on as a thread, as
    /// [`boot`](Self::boot) does for core 0: no stack of its own, a null context the first
    /// `switch_to` away fills, `Running`. It becomes that core's idle thread and is never on a run
    /// queue (`smp.rs`, `sched::adopt_secondary_idle`). Written into `dst` rather than returned,
    /// because an unoptimized build copies a returned `Thread` through stack temporaries
    /// (notes/threads.md).
    ///
    /// # Safety
    ///
    /// `dst` is writable, aligned for `Thread`, and holds no live `Thread`.
    #[inline(never)]
    pub(crate) unsafe fn write_adopted_current(dst: *mut Thread, id: ThreadId) {
        // SAFETY: the caller's contract.
        unsafe {
            dst.write(Thread {
                id,
                handshake: thread_wake_handshake::Handshake::on_cpu_now(), // adopted mid-run: standing on its CPU
                #[cfg(feature = "soak_test")]
                last_cpu: u8::MAX,
                placement: u8::MAX, // overwritten by the placement decision at spawn or START
                context: core::ptr::null_mut(),
                stack: None,
                space: None,
                mailbox: [0; 5],
                quota: None,
                outgoing_cap: None,
                cap_delivered: false,
                receiving_cap: false,
                ipc_refused: false,
                being_reaped: false,
                next: None,
                own_token: None,
                entry: (0, 0), // a kernel thread; never enters EL0 by this path
                start_args: [0; 3],
                thread_pointer: 0,
                process: None,
                capability_table: core::ptr::null(),
                thread_page: None,
                thread_control_block_kmem: true,
                killed: false,
                kill_deferred: false,
                suspend_requested: false,
                fault_ep: None,
                fault_label: 0,
                thread_control_block_region: None,
                fault_msg: None,
                bound_notification: None,
                #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
                cycle_counter_grant: false,
                #[cfg(target_arch = "x86_64")]
                port_range_grant: None,
            });
        }
    }

    /// Build a thread ready to run `f` the first time it is scheduled, directly into `dst`. Returns
    /// `false`, writing nothing, if the kernel stack could not be allocated.
    ///
    /// `f` is moved at its concrete type to the top of the new stack, above a faked `switch_to`
    /// frame, and a monomorphized [`call_closure::<F>`] is handed over to call it: no heap and no
    /// vtable. Writing through `dst` rather than returning avoids the copies that put
    /// `sched::spawn_on`'s frames past the guard page in a debug build. See notes/threads.md.
    ///
    /// # Safety
    ///
    /// `dst` must be writable, aligned for `Thread`, and hold no live `Thread`: this *writes*
    /// rather than assigns, so nothing is dropped. `Threads::insert_at` satisfies all three with a
    /// fresh page it exclusively owns.
    pub(crate) unsafe fn spawn_into<F: FnOnce() + Send + 'static>(
        f: F,
        id: ThreadId,
        dst: *mut Thread,
    ) -> bool {
        // Per monomorphization: a capture that does not comfortably fit the 24 KiB stack fails the
        // build.
        const {
            assert!(
                size_of::<F>() <= 1024,
                "spawn closure captures more than 1 KiB; pass a reference to static state instead"
            );
        };
        const {
            assert!(
                align_of::<F>() <= 16,
                "spawn closure over-aligned for a stack slot"
            );
        };

        // Nothing has been written to `dst` yet.
        let Some(stack) = KernelStack::new() else {
            return false;
        };

        // At the top of the stack, aligned down to 16 so the switch frame below keeps `sp`
        // 16-aligned (notes/stack.md). The thread never touches bytes above its initial `sp`.
        let closure_at = (stack.top() - size_of::<F>() as u64) & !15;

        // SAFETY: inside the just-mapped stack; `write` moves `f` (no drop of the original).
        unsafe { (closure_at as *mut F).write(f) };

        // A faked `switch_to` frame below the closure, so the `ret` that resumes a thread also
        // starts it.
        let context = (closure_at - size_of::<Context>() as u64) as *mut Context;

        // The closure's type was erased; the shim restores it. `arch` decides which registers carry
        // them.
        let call_shim = (call_closure::<F> as extern "C" fn(*mut ())) as usize as u64;
        // SAFETY: the stack was just mapped read/write, and this is inside it.
        unsafe {
            context.write(Context::for_kernel_thread(closure_at, call_shim));
        }

        // SAFETY: the caller's contract, passed on unchanged: `dst` is writable, aligned, and holds
        // no live Thread.
        unsafe { Self::write_kernel_thread(dst, id, context, stack) };
        true
    }

    /// The struct literal, in one non-generic, never-inlined frame rather than one per closure
    /// type. An unoptimized build materializes the literal as stack temporaries worth about two
    /// `Thread`s, which inside the generic `spawn_into` every instantiation paid on top of its
    /// capture, past the guard page for some (`script/stack-frame-check`). A literal, so the
    /// compiler checks it for completeness.
    ///
    /// # Safety
    ///
    /// As [`spawn_into`](Self::spawn_into): `dst` is writable, aligned for `Thread`, and holds no
    /// live `Thread`.
    #[inline(never)]
    unsafe fn write_kernel_thread(
        dst: *mut Thread,
        id: ThreadId,
        context: *mut Context,
        stack: KernelStack,
    ) {
        // SAFETY: the caller's contract: `dst` is writable, aligned, and holds no live Thread.
        unsafe {
            dst.write(Thread {
                id,
                handshake: thread_wake_handshake::Handshake::ready(),
                #[cfg(feature = "soak_test")]
                last_cpu: u8::MAX,
                placement: u8::MAX, // overwritten by the placement decision at spawn or START
                context,
                stack: Some(stack),
                space: None, // a kernel thread until it calls `user::exec`
                mailbox: [0; 5],
                quota: None,
                outgoing_cap: None,
                cap_delivered: false,
                receiving_cap: false,
                ipc_refused: false,
                being_reaped: false,
                next: None,
                own_token: None,
                entry: (0, 0), // a kernel thread; becomes a user process via exec, not this path
                start_args: [0; 3],
                thread_pointer: 0,
                process: None,
                capability_table: core::ptr::null(),
                thread_page: None,
                thread_control_block_kmem: true,
                killed: false,
                kill_deferred: false,
                suspend_requested: false,
                fault_ep: None,
                fault_label: 0,
                thread_control_block_region: None,
                fault_msg: None,
                bound_notification: None,
                #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
                cycle_counter_grant: false,
                #[cfg(target_arch = "x86_64")]
                port_range_grant: None,
            });
        }
    }

    /// **A TCB object, retyped but not started** (milestone 19c.3). No stack, no saved context,
    /// no address space, no entry: `Embryo`. `CONFIGURE` fills in the space and entry, `START`
    /// builds the stack and context and makes it `Ready`. `thread_control_block_kmem` records that this TCB's
    /// page is a user region's, not `kmem`'s, so the reaper leaves it for the region.
    pub(crate) fn embryo() -> Self {
        Thread {
            id: UNNAMED,
            handshake: thread_wake_handshake::Handshake::embryo(),
            #[cfg(feature = "soak_test")]
            last_cpu: u8::MAX,
            placement: u8::MAX, // overwritten by the placement decision at spawn or START
            context: core::ptr::null_mut(),
            stack: None,
            space: None,
            mailbox: [0; 5],
            quota: None,
            outgoing_cap: None,
            cap_delivered: false,
            receiving_cap: false,
            ipc_refused: false,
            being_reaped: false,
            next: None,
            own_token: None,
            entry: (0, 0),
            start_args: [0; 3],
            thread_pointer: 0,
            process: None,
            capability_table: core::ptr::null(),
            thread_page: None,
            thread_control_block_kmem: false, // a user-retyped TCB page; the region owns it
            killed: false,
            kill_deferred: false,
            suspend_requested: false,
            fault_ep: None,
            fault_label: 0,
            thread_control_block_region: None,
            fault_msg: None,
            bound_notification: None,
            #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
            cycle_counter_grant: false,
            #[cfg(target_arch = "x86_64")]
            port_range_grant: None,
        }
    }

    /// Install this embryo's kernel stack and build its entry context, ready to first run at EL0
    /// (milestone 19c.3, the guts of `START`). The stack is kernel-owned (19c.1: a kernel stack
    /// is kernel infrastructure whoever the thread serves); the context is a faked `switch_to`
    /// frame whose trampoline drops to EL0 at `entry` on `user_sp`, exactly as `thread_trampoline`
    /// starts a kernel thread's closure. The caller builds `stack`, before taking `IPC_TABLES`
    /// (2026-10-04; see `sched::start_thread_control_block`).
    pub(crate) fn arm_for_start(&mut self, stack: KernelStack) {
        let (entry, user_sp) = self.entry;
        let context = (stack.top() - size_of::<Context>() as u64) as *mut Context;
        // SAFETY: the stack was just mapped read/write, and this is inside it. `arch` owns the
        // mapping from (entry, user sp, args) onto registers and the EL0 first-run trampoline.
        unsafe {
            context.write(Context::for_user_thread(entry, user_sp, self.start_args));
        }
        self.stack = Some(stack);
        self.context = context;
    }
}

/// Where a **user** thread starts, in Rust (milestone 19c.3): the EL0 mirror of `thread_entry`.
/// Reaps whoever we switched away from (a new thread skips `schedule`'s post-switch point), then
/// drops to EL0 at `entry` on `user_sp`. The address space was installed by the context switch
/// that scheduled us in (from our `space` field), so `TTBR0` already names it.
#[unsafe(no_mangle)]
extern "C" fn user_thread_entry(entry: u64, user_sp: u64, arg0: u64, arg1: u64, arg2: u64) -> ! {
    crate::sched::finish_switch();
    crate::user::enter_at_on_current(entry, user_sp, arg0, arg1, arg2)
}

/// The monomorphized bridge between "an address on a stack" and "a closure of type `F`".
///
/// `Thread::spawn_into` erases the closure's type when it parks it on the new stack; this function,
/// instantiated per closure type and passed through `x20`, is where the type comes back. It
/// moves the closure out of its stack slot and calls it; the captures drop normally when the
/// call returns.
extern "C" fn call_closure<F: FnOnce()>(closure: *mut ()) {
    // SAFETY: `closure` is the `F` that `Thread::spawn_into` placed on this very stack, and this is
    // the single read of it: the slot is dead bytes afterward, above `sp`, touched by nobody.
    let f = unsafe { closure.cast::<F>().read() };
    f();
}

/// Where a new thread actually begins, in Rust: `thread_trampoline` passes the closure's stack
/// address and its monomorphized caller. A thread torn down before it ever runs leaks its closure's
/// captures in place; kernel threads capture only ids and statics.
#[unsafe(no_mangle)]
extern "C" fn thread_entry(closure: *mut (), call: extern "C" fn(*mut ())) -> ! {
    // A new thread skips `schedule()`'s post-switch point, so it reaps its predecessor here. It
    // arrives with IRQs masked, and `finish_switch` must run masked: a timer IRQ before it could
    // overwrite `switched_from` and strand the predecessor (context.s). Only then is the closure
    // made preemptible.
    crate::sched::finish_switch();
    crate::arch::interrupts::enable();

    call(closure);

    crate::sched::exit();
}
