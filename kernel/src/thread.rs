//! Kernel threads.
//!
//! # What a thread actually is
//!
//! From notes/registers.md, milestone 1, before any of this existed:
//!
//! > **A thread is a stack plus a set of register values.** That is not a metaphor. It is the
//! > complete and literal definition.
//!
//! And that is exactly what a [`Thread`] is here: a [`KernelStack`], and a single **stack
//! pointer** naming the place on that stack where its registers are saved. Nothing else. The
//! `context` field is 8 bytes, and it is the whole of a suspended thread's CPU state, because
//! everything else is sitting on the stack it points at.
//!
//! # Every thread gets a guard page
//!
//! Milestone 3 blew the boot stack, wrote through `.bss` and `.data` into `.text`, and hung
//! the machine for 150 seconds with no output. Milestone 4 gave the *boot* stack a guard page,
//! and the same bug became an instant, precise fault naming the exact byte that went too far.
//!
//! Thread stacks get one too, and it is not decoration: **a thread stack is 24 KiB**, well under
//! half the boot stack's, and threads are where deep recursion actually happens. This is the
//! first non-test user of `mmu::map_page` / `mmu::unmap_page`, which we built at milestone 4
//! ahead of any caller precisely so the discipline (break-before-make, an un-ignorable TLB
//! flush) would be right the first time.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use page_frames::FRAME_SIZE;
use paging::Flags;

use crate::arch::mmu;
use crate::sync::{IrqSafeMutex, rank};

pub type ThreadId = u64;

/// 24 KiB. Six pages.
///
/// This was 16 KiB (Linux's arm64 number) until 2026-08-15, when CI overflowed it on both ISAs
/// (aarch64 run 31907966383, riscv64 thead-c906 run 31910308865, both attempt 1, both on loaded
/// 2-core hosts). Linux's 16 KiB is sized for optimized code; this suite runs the kernel at debug
/// codegen, where frames are severalfold larger, and the measured arithmetic no longer fit:
///
///   - deepest standing path the suite reaches on a thread stack: ~11.7 KiB
///     (the high-water report, notes/stack-high-water.md)
///   - residue of blocking from that depth (`ipc_receive` 656 + `IPC_TABLES.lock` 256 + `schedule` 448
///     + the switch): ~1.4 KiB, resident for as long as the thread stays blocked
///   - one preemption landing at the deepest point (trap frame 272 + dispatch + GIC/PLIC claim
///     + `canary::check` + `schedule` + a contended `IPC_TABLES.lock` spin): ~2.3 KiB
///
/// Total ~15.5 KiB against a 16 KiB stack, and the CI evidence is the sum coming out past 16 KiB:
/// the guard page caught an exception-entry push at `sp` = bottom - 4096, mid-cascade, with the
/// interrupted context spinning in `IPC_TABLES.lock` (the symbolized fault sites are in
/// notes/stack-high-water.md). The overflow is load-correlated because a loaded host multiplies
/// timer preemptions per guest instruction, so one eventually lands on the deepest frame of the
/// deepest thread. Six pages leave ~8 KiB above the measured worst case; the cost is at most
/// 2 more frames per live thread. The guard page below still turns "too small" into a legible
/// fault rather than silent corruption, and the high-water gate (stack.rs) still alarms well
/// before the guard.
pub const STACK_PAGES: usize = 6;

/// Where kernel thread stacks live, virtually: **the architecture's answer**, because the address
/// is the architecture's (rule 1).
///
/// It has to be far above the direct map, so a stack address can never collide with the virtual
/// *name* of a physical one. This was `KERNEL_VA_BASE | 0x10_0000_0000` here, computed portably,
/// which was right on two architectures whose kernel base is a half base with room above it and
/// silently the identity on `x86_64`, where `KERNEL_VA_BASE` already carries that bit and every kernel
/// thread stack would have landed on the kernel image. See each `arch::mmu::THREAD_STACK_AREA`.
const STACK_AREA: u64 = mmu::THREAD_STACK_AREA;

/// One thread's slot in [`STACK_AREA`]: the guard page, then [`STACK_PAGES`] of stack. Every slot
/// is this wide and every base is a multiple of it from `STACK_AREA`, including the reused ones
/// (`FREE_STACK_ADDRESS_SPACE` hands back the slot base, never an interior address), which is what lets a
/// fault handler turn an address back into "slot N, this far into its guard page". See
/// [`crate::stack::guard_page_at`].
pub const STACK_SLOT_SPAN: u64 = (STACK_PAGES as u64 + 1) * FRAME_SIZE;

static NEXT_STACK_VA: AtomicU64 = AtomicU64::new(STACK_AREA);

/// The thread-stack area as `(base, watermark)`: every stack slot ever handed out lies below the
/// watermark, and nothing else in the kernel map lies in the span at all.
///
/// Reads one relaxed atomic and nothing else, deliberately: the caller is a fault handler that has
/// already lost the machine, so it may not take a lock. A slot allocated concurrently on another
/// core can be missing from the range, which costs a diagnosis and never a wrong one.
pub fn stack_area_span() -> (u64, u64) {
    (STACK_AREA, NEXT_STACK_VA.load(Ordering::Relaxed))
}

/// The `id` a constructor writes before the thread table has named the thread. Deliberately
/// `u64::MAX` (= `cpu::NO_TID`), which the generational table can never mint, so a thread that
/// somehow escaped naming resolves to nothing instead of to slot 0. Every insert path overwrites
/// it via `Table::insert_with` (milestone 14 phase A; design/kernel-objects-from-untyped.md).
pub const UNNAMED: ThreadId = u64::MAX;

/// Stack address ranges from threads that have exited.
///
/// **Reusing these is not a micro-optimization.** Bump-allocating virtual addresses forever
/// means every 2 MiB of address space consumed permanently costs an L2 and an L3 page table,
/// because `unmap_page` frees the leaf mapping but leaves the intermediate tables standing, on
/// purpose (see `paging::Mapper::unmap` and notes/teardown.md). Threads come and go; the tables
/// would only ever accumulate.
///
/// Handing the address range back means a new thread lands in page tables that already exist,
/// and the whole system reaches a steady state. A test asserts that a second batch of threads
/// costs **exactly zero** additional frames.
static FREE_STACK_ADDRESS_SPACE: IrqSafeMutex<FreeAddressSpace> =
    IrqSafeMutex::new(rank::STACK_VA, FreeAddressSpace::new());

/// A fixed stack of reusable stack-VA ranges (milestone 14 phase B.1). Bounded by construction:
/// a range is pushed only when a thread dies and popped when one spawns, so the free count can
/// never exceed the most threads that ever lived at once, which the scheduler caps at
/// [`crate::sched::MAX_THREADS`]. The array is **sized from that constant** rather than to a
/// literal matching it, and the debug assert is the cross-check.
///
/// It was `[u64; 128]` with the constant named only in this comment until 2026-08-27, which is a
/// coupling nothing enforced: raising the ceiling without finding this line would have overflowed
/// the bound the assert claims, and in a release build the `else` branch below would have quietly
/// leaked VA ranges instead. The array does not need to be told twice.
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
        // One page of virtual address space for the guard, plus the stack itself. The guard's
        // VA is simply never mapped, which is the entire mechanism.
        let span = STACK_SLOT_SPAN;

        // Reuse a dead thread's address range if there is one, so the page tables covering it
        // are already built. Only bump into fresh address space when there isn't.
        let base = FREE_STACK_ADDRESS_SPACE
            .lock()
            .pop()
            .unwrap_or_else(|| NEXT_STACK_VA.fetch_add(span, Ordering::Relaxed));

        let guard = base;
        let bottom = base + FRAME_SIZE;
        let top = bottom + STACK_PAGES as u64 * FRAME_SIZE;

        let mut pages = [0u64; STACK_PAGES];
        for (i, slot) in pages.iter_mut().enumerate() {
            // From the kernel's own budget (19c.1), recycled from dead stacks, not the frame
            // allocator. This is what makes "the kernel cannot spend beyond its boot carve"
            // true of stacks, the last open-ended kernel draw milestone 14 had not closed.
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

        // Paint the whole stack for the high-water report (milestone 84): every page is mapped and
        // no thread has run on it, so there is no live portion to skip.
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
    pub fn bottom(&self) -> u64 {
        self.bottom
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        // Measure before unmapping (milestone 84). The reaper runs this on the successor's stack,
        // never the one being scanned. Skip a partial build (some pages never mapped, so a scan
        // would fault) and note it was never painted or used anyway.
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

            // `unmap_page` discharges the TLB obligation with a real `tlbi`. It has to, and the
            // reason is right here: this virtual address is about to be handed to a **different
            // thread's stack**. A stale translation would let the new thread read (and write)
            // the dead thread's saved registers. See notes/page-tables.md.
            if mmu::unmap_page(va).is_ok() {
                crate::kmem::recycle(phys); // home to the kernel budget, not the frame allocator
            }
        }

        // Hand the address range back, so the next thread lands in page tables that already
        // exist. The physical pages were recycled above; this returns the *names*.
        FREE_STACK_ADDRESS_SPACE.lock().push(self.guard);
    }
}

/// **Where a thread is in its life.** The enum itself lives in `crates/wake_handshake` now,
/// because the block/wake transitions that read and write it were lifted there for loom to search
/// (the fourth bench stop's retrofit; see that crate's header and notes/interleaving.md). The
/// kernel keeps its vocabulary: `State` here is exactly `thread_wake_handshake::RunState`, and nothing in
/// `sched.rs` reads any differently than it did.
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
/// unlinks a blocked thread, the hang dump's wait column).
///
/// **An enum rather than a `(u64, WaitRole)` pair since milestone 151 (notification objects)**, because a thread can now
/// wait on two kinds of object and the two names live in two registries. A pair with a
/// `Notification` role would have carried a notification's name in a field every existing reader
/// treats as a rendezvous name, and a teardown that resolved it in the rendezvous table would have
/// found a stranger. Here the name cannot be read as the wrong kind (AGENTS.md's ladder, rung 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// Parked in IPC on a rendezvous, as the given side of it.
    Rendezvous(crate::sched::RendezvousId, WaitRole),
    /// Parked in `WAIT` on a notification (milestone 151), on its wait queue.
    Notification(crate::sched::NotificationId),
}

pub struct Thread {
    pub id: ThreadId,

    /// **The block/wake handshake**: `state` plus the `on_cpu`/`wake_pending`/`wait_on`/
    /// `ipc_served`/`ipc_aborted` protocol that used to be five loose fields here. Lifted into
    /// `crates/wake_handshake` so loom can search its interleavings on the host, and embedded so
    /// the kernel **calls** the checked transitions rather than mirroring them (the regions-crate
    /// precedent). Every access is under `IPC_TABLES`, exactly as before; the crate's header carries
    /// the protocol's rules, its races and its BUGS.
    pub handshake: thread_wake_handshake::Handshake<Wait>,

    /// **Which core this thread last ran on** (milestone 219), or [`u8::MAX`] if it has never run.
    ///
    /// One byte, written at every `switch_in`, read at the same place to decide whether this turn
    /// is a migration. It exists because nothing else in the kernel could answer "did this
    /// workload actually cross cores", and the number that looked like it could does not: a
    /// rendezvous wake makes its peer `Ready` on the **waker's** core (DECISIONS §28.2), which is
    /// local by construction, so `trace::Event::PlaceRemote` never fires for the migration it
    /// nonetheless performs. Measured on 2026-09-01: a four-core IPC soak doing 65,000 round trips
    /// a second reported `remote` frozen at 23 while threads moved between cores continuously.
    ///
    /// Not part of the block/wake protocol, so deliberately not in `Handshake`: that crate models
    /// the transitions loom searches, and this is an observation about them.
    ///
    /// **Behind `feature = "soak_test"`, and that is not tidiness.** The write sits in `schedule()`'s
    /// switch, which is the hottest line of the hottest function, and shipping it unconditionally
    /// cost **5.7% of `ipc_fastpath`'s footprint on aarch64** (5788 -> 6120 bytes), over milestone
    /// 132's 5% bound, with `riscv64` and `x86_64` growing 4.7% and 4.6% behind it. That gate exists
    /// because of Liedtke's cache-footprint argument (`script/fastpath-footprint`'s header), and an
    /// instrument that only a soak build reads has no business being on every IPC in every build.
    #[cfg(feature = "soak_test")]
    pub last_cpu: u8,

    /// **Which core this thread was placed on when it started**, or [`u8::MAX`] if it has not
    /// started. The fact `abi::survey::record::PLACEMENT` reports.
    ///
    /// `sched::spawn_reporting_placement` has handed this to the in-kernel job-mix supervisor since
    /// milestone 240 (the soak reports what happened and not where) as a return value, which is only
    /// available to a caller that did the spawning.
    /// A userspace supervisor did not do the spawning, so a field on the thread is what gives the
    /// same fact a path out through a survey, and is what unblocks moving that supervisor out of
    /// the kernel.
    ///
    /// **Written once, at [`crate::sched::spawn_on`] and [`crate::sched::start_thread_control_block`],
    /// and never on the IPC path.** That is the whole reason this one is unconditional where
    /// `Thread::last_cpu` above is behind a soak-build feature (not a link, because that field
    /// does not exist in a build without `soak_test` and rustdoc would not resolve it): `last_cpu` is written in
    /// `schedule()`'s switch and cost 5.7% of `ipc_fastpath`'s footprint on aarch64, over milestone
    /// 132's bound. A placement is one store per thread creation, which is a cold path by
    /// definition, so the same information that was too expensive to keep continuously is free to
    /// keep once.
    ///
    /// **So it is placement, not location**, and the record's documentation says so to its reader
    /// rather than leaving the distinction here. A thread stolen onto another core, or woken onto
    /// its waker's (DECISIONS §28 (SMP placement: local wakes), sub-point 2), still reports where it was placed.
    ///
    /// The value is a cpu **id**, which is a position in the online mask and not an index into a
    /// range: `cpu_set`'s header has the VisionFive 2 boot this distinction cost three boots to
    /// diagnose.
    ///
    /// Name: provisional. calef names public items.
    pub placement: u8,

    /// **The saved general-purpose state of this thread**: one stack pointer.
    ///
    /// Everything the calling convention promises a callee preserves lives on the stack it points
    /// at, pushed there by `switch_to`. Eight bytes. That is what "a thread is a stack plus a set
    /// of register values" means when you write it down.
    ///
    /// **This line used to say "the ENTIRE saved CPU state", and milestone 447 (a thread's vector
    /// registers are its own) made that false** rather than merely incomplete. The other half is
    /// the FP/SIMD register file, four times the size, and it is neither in this struct nor on the
    /// stack this points into: it lives in the free space of this thread's own TCB page. See
    /// [`fp_state_of`]. The sentence is corrected here rather than quietly widened, because a
    /// reader who believed the old one would go looking for the vector registers on that stack.
    pub context: *mut Context,

    /// `None` for the boot thread, which runs on the stack `boot.s` set up and does not own it.
    ///
    /// Never *read*, and that is the point: it exists to be **dropped**. When the reaper removes
    /// a finished `Thread` from the map, this field's `Drop` unmaps four pages, discharges the
    /// TLB obligation, frees four frames, and hands the address range back. Ownership doing the
    /// work, exactly as notes/heap.md described it: the compiler proving the free happens once,
    /// at the right moment.
    ///
    /// So this one stays allowed unconditionally and on purpose: there is no configuration in which
    /// anything reads it, and that is the design rather than a gap (DECISIONS §38, disposition 3).
    ///
    /// **One exception to "dropped by the reaper's `remove`"**, since 2026-10-04: the reaper
    /// (`sched::reap_switched_out`) takes the stack out under `IPC_TABLES` and drops it after
    /// releasing the lock, because freeing it is six page unmaps, each with a TLB shootdown that
    /// on riscv64 and x86_64 is a synchronous round of inter-processor interrupts, and every other
    /// core's IPC and syscalls waited behind it. [`Thread::being_reaped`] is what keeps the thread
    /// from looking gone while that happens.
    #[allow(dead_code)]
    pub stack: Option<KernelStack>,

    /// **This thread's kernel stack and address space have been taken out and are being freed,
    /// without `IPC_TABLES` held** (2026-10-04; name provisional). Set by the reaper between taking
    /// them and removing the thread, and the thread stays in the table, `Finished`, the whole time.
    ///
    /// Region teardown reads it as "a core is still standing on this stack": `DESTROY` refuses the
    /// region passively and the owner's retry succeeds once the reaper has removed the thread. That
    /// ordering is the point. An owner that sees `DESTROY` succeed may spend the memory at once (the
    /// job mix's `spawn` job builds its next child immediately), so everything the dead thread held
    /// must be back before it stops occupying the region. Removing the thread first and freeing
    /// the stack afterwards ran QEMU's job mix out of kernel memory within a subrun
    /// (notes/job-mix/null-syscall-under-load.md).
    pub being_reaped: bool,

    /// The low half of memory, as far as this thread is concerned. `None` for a kernel thread,
    /// which has no business at a low address at all.
    ///
    /// **`TTBR0_EL1` is one register and it is global; threads are not.** So the context switch
    /// installs this on the way in, exactly as it installs a stack and a register file. A user
    /// thread that kept running while another thread swapped `TTBR0` would find its own code
    /// replaced by a stranger's, which is not a hypothetical: see notes/userspace.md.
    ///
    /// **A copy, not the space** since §249 (a running address space stays nameable): the
    /// address-space registry owns every space, so a capability can keep naming this one while the
    /// thread runs, and the thread keeps what the context switch reads ([`crate::user::BoundSpace`]),
    /// because the switch must not take the registry's lock. The reaper takes the space out of the
    /// registry by the name in this copy and drops it, unless the region sweep took it first; the
    /// removal is take-once, so whichever comes second finds nothing.
    pub space: Option<crate::user::BoundSpace>,

    // **Everything this thread can name is not a field any more** (2026-10-04 UTC): it lives past the
    // end of this struct in the same page, behind its own lock. See [`capability_table_of`].
    /// **The IPC message this thread most recently sent or received.** Five words.
    ///
    /// A sender parks its message here before blocking; a receiver reads it here after being
    /// woken. It is a `Thread` field rather than a stack local precisely because the rendezvous
    /// happens across two threads at two different times: the sender deposits it and blocks, and
    /// the receiver, running later, reaches into the sender's `Thread` to collect it. See
    /// sched.rs.
    ///
    /// Three words carry ordinary IPC; the extra two exist for the five-word fault/exit message a
    /// dead thread's corpse delivers to its supervisor (DECISIONS §26, abi's `fault` module).
    /// Ordinary sends leave words 3 and 4 zero, and `RECEIVE` hands all five back, so only a
    /// supervisor ever reads the top two.
    pub mailbox: [u64; 5],

    /// **A slot in a spawner's quota, or `None` for a thread nobody bounded.** Reaped with the
    /// thread, which is how the slot comes back. See [`QuotaToken`].
    ///
    /// Never read, like [`Self::stack`]: it exists to be dropped, and its `Drop` returns the slot.
    /// It is `None` on every thread today, because `sched::spawn_with_quota` has had no caller since
    /// §28 retired the kernel-wired shell; see that function for why the mechanism stays.
    #[allow(dead_code)]
    pub quota: Option<QuotaToken>,

    /// **A capability parked here mid-delegation.** When a thread does a capability-carrying send
    /// (`SEND_CAP`) and no receiver is waiting, it blocks with the capability stashed here, exactly
    /// as `mailbox` stashes the data words. The receiver, running later, reaches in, `take()`s it,
    /// and inserts it into its own capability table. `None` for every ordinary send. See sched.rs.
    ///
    /// **BUGS: the revocation sweeps clear this slot, and the teardown paths do not.**
    /// `sched::delete_page_frame_caps_where`, `delete_device_frame_caps_from_others`,
    /// `delete_port_range_caps_impl` and `delete_reply_caps_naming` all drop a parked capability
    /// (`notes/confinement-claims.md` row 30). `depart`, `finish_blocked_resident` and
    /// `reap_region_objects` never touch it. That is safe today only because a thread that is
    /// running, exiting or being reaped always has it `None`: a parked sender is unparked through
    /// `set_ipc_aborted`, which clears it. A new wake path that bypasses `set_ipc_aborted` would
    /// leave a live capability in a corpse, and `ipc_receive_cap`'s `take()` on any sender would
    /// deliver it. Found by milestone 633 (an outside agent attacks the confinement claim)'s second
    /// pass, reasoned from the code; clearing it in the three teardown paths is one line each.
    pub outgoing_cap: Option<crate::cap::Cap>,

    /// **Did the delivery this thread is about to read install a capability?** Set by the paths
    /// that put one in this thread's table while it was parked in `RECEIVE_CAP` (`ipc_send_cap`,
    /// `ipc_call_badged`), cleared when it parks there. `ipc_receive_cap` reads it to decide `x1`: a
    /// delivery that installed nothing returns `NO_CAP`, never the sender's data word. Without it a
    /// plain `SEND` that reached a parked `RECEIVE_CAP` receiver left its second word in `x1`, where a
    /// `CALL` server reads a reply slot (fatal risk 7). See `sched::ipc_receive_cap`.
    // Added by milestone 634 (a plain SEND received by RECEIVE_CAP never hands the receiver a
    // sender-chosen slot).
    pub cap_delivered: bool,

    /// **Is the receive this thread is parked in a `RECEIVE_CAP`?** `true` only between
    /// `ipc_receive_cap`'s park, where it is set beside the `cap_delivered` reset, and that
    /// receive's resume, where it is cleared; so a plain `RECEIVE` always parks with it `false`.
    /// A sender that meets a parked receiver reads it to decide whether a capability may be
    /// installed at all: a plain `RECEIVE` never takes one, whichever side reached the rendezvous
    /// first (§246 (a plain `RECEIVE` never takes a capability), PROVISIONAL number; calef's
    /// ruling A, 2026-10-04 UTC). Before it, `ipc_send_cap` and `ipc_call_badged` installed into
    /// any parked receiver's table, so the answer depended on arrival order. Meaningful only while
    /// the thread is parked as a receiver. See `sched::ipc_send_cap`.
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

    /// **This thread's queue token, while nothing else holds it** (milestone 139 (drive the unsafe
    /// count down), round 10; `intrusive_fifo::Unqueued`). Every live thread has exactly one token,
    /// minted once when the thread table inserts it, and the token is always on a queue (this
    /// thread is `Ready` on a run queue or inbox, or `Blocked` on a wait queue) or here. Here means
    /// on no queue: a running thread (the pop that chose it handed the token home through
    /// `sched::hold_token`, and the requeue in `schedule` takes it from here), an embryo, a thread
    /// blocked on nothing (a caller awaiting its Reply), a thread whose wake was deferred until its
    /// core finishes switching it out, a corpse, or an idle thread off its CPU.
    ///
    /// A queue push takes the token by value, so a thread can reach a queue only through the token
    /// its last transition left here. `sched::wake` takes it from here.
    ///
    /// Ruling C's letter had the running thread's token in a per-core current slot; this field is
    /// the deviation round 10's report puts to calef, and this comment describes what is.
    ///
    /// Name: provisional (milestone 139, round 10): calef names fields a reader meets.
    pub(crate) own_token: Option<intrusive_fifo::Unqueued<Thread>>,

    /// **Where this thread's EL0 execution begins** (milestone 19c.3), set by `ThreadControlBlock::CONFIGURE`
    /// on an embryo, consumed by `START` to build the entry context. `(0, 0)` for a kernel
    /// thread, which never drops to EL0 and runs its closure instead.
    pub(crate) entry: (u64, u64), // (entry_va, user_sp)

    /// **May this thread read the CPU's cycle counter from user mode** (milestone 229, DECISIONS
    /// 139 option 4): the per-thread grant the context switch enforces.
    ///
    /// Set once, on an embryo, by `sched::grant_cycle_counter`, and never afterwards: the whole
    /// point of a creation-time grant rather than a method on a live thread is that a program
    /// cannot acquire a timing instrument it was not created with. `false` on every kernel thread
    /// and on every user thread, which today is all of them, because milestone 229 deliberately
    /// shipped this mechanism without the syscall method that would set it (see
    /// `abi::thread_control_block`'s module note for why a method number was not spent).
    ///
    /// `sched::schedule` reads it beside the incoming thread's address-space root and hands it to
    /// `arch::timer::set_cycle_counter_grant`, which writes the enable only when it differs from
    /// what the core already holds. So a machine where nothing is granted pays one compare.
    ///
    /// *(Field name provisional: names are an architect's.)*
    ///
    /// **Built only under `test` or `--features cycle_counter_grant`** (milestone 237). The grant
    /// costs `sched::schedule` 192 bytes it was spending for an instrument nothing can request, so
    /// the whole mechanism is a measurement build the way `soak` is; `kernel/Cargo.toml`'s feature
    /// block is where the reasoning and the measured cost live. Every other `cfg` on this mechanism
    /// spells the same predicate, and `test` is in it so milestone 229's proofs keep compiling it.
    #[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
    pub(crate) cycle_counter_grant: bool,

    /// **The x86 I/O ports this thread may reach from ring 3** (milestone 299, DECISIONS §121
    /// reversed 2026-09-15), as `(base, count)`, or `None` for the overwhelming majority of threads
    /// that hold no port capability.
    ///
    /// This is the per-thread "holds a port capability" fact the lazy TSS-bitmap enforcement is
    /// built on. `sched::schedule` reads it beside the incoming thread's address-space root and
    /// hands it to `arch::segments::set_port_range_grant`, which writes the current CPU's TSS I/O bitmap
    /// **only when it differs** from what that CPU already holds. On a machine where one process (the
    /// console driver) ever holds a port capability, that is `None` on both sides of nearly every
    /// switch and costs one compare, which is the whole point of the lazy form (§121's 2026-08-25
    /// refinement, binding here): the naive always-write cost ~2,682 ns/switch and this pays it only
    /// on the rare switch that crosses a holder.
    ///
    /// Set by `sched::thread_control_block_insert_cap` when a `PortRange` capability is inserted into
    /// this thread (the choke point the progenitor's `CAP_INSERT` and the boot's own child builder
    /// both pass through), cleared by `sched::delete_port_range_caps*` on revocation, and cleared by
    /// `sched::delete_current_cap` when the thread drops the capability itself (milestone 313's
    /// audit found that path leaving the grant installed for the thread's whole life). **`x86_64`
    /// only**: the field, and every path that reads it, is compiled out on the architectures that
    /// have no port space, so the switch path there is byte-for-byte what it was.
    ///
    /// *(Field name provisional: names are an architect's.)*
    #[cfg(target_arch = "x86_64")]
    pub(crate) port_range_grant: Option<(u16, u16)>,

    /// **The child's initial `x0`, `x1`, `x2`** (milestone 19d/19e): the words `START` hands the
    /// new EL0 thread in its first registers, so a loader can pass a child its role plus data (a
    /// worker's input, a driver's DMA address). All zero for a kernel thread.
    pub(crate) start_args: [u64; 3],

    /// **This thread's thread pointer** (milestone 812 (`std::thread::spawn` runs real threads in
    /// one address space), §269 (how threads share a process) fork 4): `TPIDR_EL0` on aarch64, `tp`
    /// on riscv64, the `FS` base on `x86_64`. The one register a program's thread-local storage
    /// hangs off, so two threads of one space must each have their own.
    ///
    /// **The kernel sets it, the same way on all three architectures.** `CONFIGURE` gives the first
    /// value (Linux's `CLONE_SETTLS`), `ThreadControlBlock::SET_THREAD_POINTER` changes it later
    /// (seL4's `SetTLSBase`), and `sched::schedule` installs it at every switch in through
    /// `arch::thread_pointer::hand_over`. Zero, the value every thread had before this field, for a
    /// thread nobody gave one. Always a user address or zero, which every writer checks: `x86_64`'s
    /// `wrmsr` faults on a non-canonical value.
    ///
    /// Where the hardware lets userspace write the register itself (aarch64 and riscv64), this copy
    /// can lag the register while the thread runs; the hand-over saves the register back on
    /// aarch64, and on riscv64 the trap frame is the live copy. `arch::thread_pointer` in each
    /// architecture says which. *(Field name provisional.)*
    pub(crate) thread_pointer: u64,

    /// **Did this thread's TCB page come from `kmem`** (recycle it on death) or from a user
    /// process's own region (leave it; the region reclaims it at destroy)? True for every
    /// kernel-created thread; false for a user-retyped TCB (19c.3). The page-origin half of the
    /// same owned-vs-borrowed question kernel stacks answered with "one owner" (notes/tcb.md).
    pub(crate) thread_control_block_kmem: bool,

    /// **Marked for forcible teardown** (DECISIONS §16 amendment): a region's owner called
    /// `MemoryRegion::DESTROY` while this thread was still live in it, so the thread is doomed. The
    /// scheduler converts a killed thread to a corpse at its next preemption instead of requeueing
    /// it (see `schedule`), so a runaway that never checks its endpoint is torn down without
    /// yanking it out of a queue or stopping another core: each core reaps its own on the timer.
    /// This is the forcible tier of `^C` (§24), where the shell's escalation retries `DESTROY`
    /// until the killed thread has self-terminated and the region is object-free.
    pub(crate) killed: bool,
    /// **Where this thread's fault/exit is reported** (milestone 22, DECISIONS §26), or `None` for
    /// an unsupervised thread. Set once at `START` from the child's reserved fault slot (abi's
    /// `FAULT_EP_SLOT`) and never afterward: supervision is granted at spawn only, so the
    /// relationship is fixed and visible in how the thread was built (§26.2). When the thread
    /// faults or exits, the kernel delivers a five-word message here and the corpse goes `Dead`
    /// until reaped; a thread with `None` dies and is reaped immediately, today's behaviour.
    pub(crate) fault_ep: Option<crate::sched::RendezvousId>,
    /// **The label the builder set on this thread's supervision capability** (milestone 105
    /// (the two forks), DECISIONS §148 (resolves by asking the kernel) as amended 2026-10-04,
    /// ruling R3), or `0` when the capability was unbadged or the thread is unsupervised. Read at
    /// `START` from the badge on the capability in the reserved fault slot, beside
    /// [`Self::fault_ep`], and never afterward. The slot is consumed at the same moment, so the child
    /// never holds a capability carrying it and cannot learn it.
    ///
    /// It travels with the death message and nowhere else: `depart` hands it to the supervisor in
    /// argument register 5 of a plain `RECEIVE`, beside the five mailbox words rather than in
    /// them, so ordinary IPC stores nothing more than it did (§148's benchmark condition).
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

/// **The page is the bound, and the compiler is what checks it.**
///
/// A `Thread` is always constructed at the start of a whole 4096-byte page it exclusively owns:
/// `Threads::insert_at` and `insert_at_in_place` both take `phys_to_virt(page) as *mut Thread`, and
/// every route into the table (`insert_with` from `kmem`, `insert_from_page` from a user region's
/// retyped object page) goes through one of the two. Milestone 754's 64-slot table leaves less of the page
/// unused than before (the assertion below is the live number), so the register file is free: it is not memory this milestone asked anyone for, it is
/// memory that was already allocated and idle.
///
/// This assertion is the whole of the mechanism that keeps that true. Grow `Thread` past the point
/// where the register file no longer fits beside it and the kernel does not build.
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
pub unsafe fn fp_state_of(thread: *mut Thread) -> *mut crate::arch::fp::FpState {
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
/// # Safety
/// As [`fp_state_of`], and the register file must not already hold anything worth keeping.
pub unsafe fn init_fp_state(thread: *mut Thread) {
    // SAFETY: the caller's contract; one aligned write of a `Copy` value inside the page.
    unsafe { fp_state_of(thread).write(crate::arch::fp::FpState::INITIAL) };
}

/// **Everything a thread can name, behind its own lock** (provisional name, from the proposal
/// "capability lookup off the global lock", 2026-10-04 UTC).
///
/// It starts **empty**, and that is the whole of DECISIONS §10 (the capability-based process model) expressed as an initializer. Under
/// Unix a fresh process inherits every file descriptor its parent held, and can `open()` anything
/// its uid permits. Here it can name *nothing at all* until somebody hands it something. It lives in
/// kernel memory and userspace never sees a byte of it. Userspace sees an integer. That is the
/// entire unforgeability mechanism, and it is a bounds check.
///
/// **Its own lock, at `rank::CAPABILITY_TABLE`, so the running thread's lookup does not take
/// `IPC_TABLES`.** Until 2026-10-04 this was a field of [`Thread`], and so every syscall's lookup
/// (`sched::current_cap`) took the global lock to read a table that has exactly one owner. On radon
/// at four busy cores 41% of those lookups found it held. The order between the two locks is
/// written at the rank.
pub type CapabilityTableLock = crate::sync::IrqSafeMutex<crate::cap::CapabilityTable>;

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

/// An empty table behind an unheld lock, as a constant operand: [`init_capability_table`] copies it
/// straight into the page rather than building a temporary the size of the table on its own frame
/// (the reason milestone 754 (the capability table grows to 64 slots) gives for an empty table constant).
#[allow(clippy::declare_interior_mutable_const)] // only ever moved into a page, never borrowed
const EMPTY_CAPABILITY_TABLE: CapabilityTableLock = crate::sync::IrqSafeMutex::new(
    crate::sync::rank::CAPABILITY_TABLE,
    crate::cap::CapabilityTable::new(),
);

/// The address of `thread`'s capability table.
///
/// # Safety
///
/// As [`fp_state_of`]: `thread` must point to a live `Thread` **at the start of its own TCB page**,
/// as the table stores it, never one derived from a reference.
pub unsafe fn capability_table_of(thread: *mut Thread) -> *const CapabilityTableLock {
    // SAFETY: the caller's contract. Inside the page by the assertion above, and aligned because the
    // offset is a multiple of the lock's alignment and a page is aligned to far more.
    unsafe { thread.cast::<u8>().add(CAPABILITY_TABLE_OFFSET).cast() }
}

/// Give a freshly-born thread its empty capability table. Called beside [`init_fp_state`] by both
/// `Threads` inserts, for its reason: a `kmem` page is not zeroed.
///
/// # Safety
///
/// As [`capability_table_of`], and no other core may yet be able to reach the table: the thread is
/// being born, so its name has not been handed to anyone.
pub unsafe fn init_capability_table(thread: *mut Thread) {
    // SAFETY: the caller's contract; one aligned write inside the page, over bytes nothing reads.
    unsafe {
        capability_table_of(thread)
            .cast_mut()
            .write(EMPTY_CAPABILITY_TABLE);
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
            thread_control_block_kmem: true,
            killed: false,
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

    /// Adopt the context a **secondary core** is already running on as a thread, the way
    /// [`boot`](Self::boot) does for core 0.
    ///
    /// Same shape as `boot`: no stack of its own (it runs on the core's `smp` boot stack), a null
    /// context filled by the first `switch_to` away from it, `Running`. This becomes that core's
    /// idle thread, so it is never in a run queue; the scheduler falls back to it when the core's
    /// queue is empty. See smp.rs and `sched::adopt_secondary_idle`.
    ///
    /// **Written into `dst`, not returned** (milestone 754 (the capability table grows to 64 slots)): a returned `Thread` is two copies in an
    /// unoptimised build, and at 64 capability slots two copies is more than the guard page.
    ///
    /// # Safety
    ///
    /// `dst` is writable, aligned for `Thread`, and holds no live `Thread`.
    #[inline(never)]
    pub unsafe fn write_adopted_current(dst: *mut Thread, id: ThreadId) {
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
                thread_control_block_kmem: true,
                killed: false,
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

    /// A new thread, ready to run `f` the first time it is scheduled.
    ///
    /// **The closure lives on the new thread's own stack** (milestone 14 (kernel objects from untyped) phase B.3): `spawn_into` is
    /// generic, so `f` is moved at its concrete type into the top of the fresh stack, above the
    /// faked switch frame. No heap, no vtable: `x19` carries the closure's address and `x20` a
    /// monomorphized [`call_closure::<F>`] that knows how to call it. The old shape boxed the
    /// closure twice (a `dyn` fat pointer does not fit one register); both allocations are gone,
    /// and the memory is freed by being the thread's stack.
    /// **Build a thread directly into `dst`, rather than returning one by value** (milestone 124).
    ///
    /// A `Thread` is a large value: `CapabilityTable<Object, 16>` alone is 384 bytes, and a debug build
    /// copies rather than elides at every move. Returning one travelled through
    /// `Thread::spawn`'s frame, `spawn_on`'s local, a closure capture, that closure's return, and
    /// finally `ptr.write`, and each hop was a real memcpy through a stack temporary. The
    /// instantiations of `sched::spawn_on` measured 3888 to 4592 bytes, **over the 4096-byte guard
    /// page on both ISAs**, which is the size at which a frame can step past the guard in one move
    /// and corrupt the neighbouring stack without ever faulting (notes/stack.md).
    ///
    /// Writing through a pointer the caller already has removes the hops. The destination is the
    /// TCB page `Threads::insert_at` claimed, which is where the thread was always going to live.
    ///
    /// Returns `false` and writes nothing if the kernel stack could not be allocated, which is the
    /// same failure `spawn` reported as `None`.
    ///
    /// # Safety
    ///
    /// `dst` must be writable, aligned for `Thread`, and hold no live `Thread`: this *writes*
    /// rather than assigns, so nothing is dropped. `Threads::insert_at` satisfies all three with a
    /// fresh page it exclusively owns.
    pub unsafe fn spawn_into<F: FnOnce() + Send + 'static>(
        f: F,
        id: ThreadId,
        dst: *mut Thread,
    ) -> bool {
        // Bounds at compile time, per monomorphization: a capture that does not comfortably fit
        // the stack is refused at build, not at runtime. 1 KiB is generous (captures here are a
        // few words) while leaving the 24 KiB stack its headroom.
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

        // `false` rather than `?`: this returns a bool now, and the failure is the one `spawn`
        // used to report as `None`. Nothing has been written to `dst` at this point.
        let Some(stack) = KernelStack::new() else {
            return false;
        };

        // The closure's slot: at the very top of the stack, aligned down to 16 so the switch
        // frame below it keeps `sp` 16-aligned (notes/stack.md). Bytes above the initial `sp`
        // are never touched by the thread's own execution, so the value is safe there until
        // `call_closure` moves it out.
        let closure_at = (stack.top() - size_of::<F>() as u64) & !15;

        // SAFETY: inside the just-mapped stack; `write` moves `f` (no drop of the original).
        unsafe { (closure_at as *mut F).write(f) };

        // Fake a `switch_to` frame just below the closure, so that the very same `ret` that
        // resumes an existing thread also *starts* a new one. There is no separate "first
        // run" path: the trampoline just happens to be what `x30` points at.
        let context = (closure_at - size_of::<Context>() as u64) as *mut Context;

        // The closure lives on the new stack (`closure_at`); its concrete type was erased, so we
        // also hand over the monomorphized shim that knows how to call it. `arch` owns which
        // registers carry them and which trampoline `switch_to`'s first `ret` lands in.
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

    /// **The struct literal, in one frame for every closure type rather than one per closure type**
    /// (milestone 126 (the `procps` package), 2026-09-27, UTC). An unoptimised build materialises the `Thread` (and the
    /// `CapabilityTable::new()` inside it) as stack temporaries before copying them to `dst`, so
    /// wherever this literal sits, its frame carries roughly two `Thread`s. Inside the generic
    /// [`spawn_into`](Self::spawn_into) that cost was paid by *every* monomorphization, on top of
    /// that closure's own capture, and raising `crate::cap::CAPABILITY_TABLE_SLOTS` from 24 to 32
    /// (and milestone 754 (the capability table grows to 64 slots) from 32 to 64, which doubles it again) put two of them over the 4096-byte guard page (`script/stack-frame-check`:
    /// `spawn_into::<fs_service::spawn_fs_server>` at 4112). Here, non-generic and never inlined,
    /// the temporaries exist once, in a frame that holds nothing else.
    ///
    /// It stays a struct literal on purpose: the compiler checks it for completeness, which the
    /// field-by-field alternative milestone 447 refused would give up.
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
                thread_control_block_kmem: true,
                killed: false,
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
    pub fn embryo() -> Self {
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
            thread_control_block_kmem: false, // a user-retyped TCB page; the region owns it
            killed: false,
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
    pub fn arm_for_start(&mut self, stack: KernelStack) {
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

/// Where a new thread actually begins, in Rust.
///
/// Called by `thread_trampoline` with the closure's stack address in `x0` and its monomorphized
/// caller in `x1`. If the thread is torn down before it ever runs, the closure's destructors do
/// not run (its captures are leaked in place); true of the boxed version before it, and fine for
/// what kernel threads capture (ids, statics), but a real constraint worth knowing about.
#[unsafe(no_mangle)]
extern "C" fn thread_entry(closure: *mut (), call: extern "C" fn(*mut ())) -> ! {
    // We are a brand-new thread, resuming for the first time. The thread this core switched away
    // from to start us may have finished; reap it now, off its stack, exactly as a resuming thread
    // does after `switch_to`. A new thread does not pass through `schedule()`'s post-switch point,
    // so this is the only place that reap happens for it. See sched::finish_switch.
    //
    // We arrive with IRQs masked (the trampoline no longer unmasks early: doing so before this
    // call stranded the predecessor when a timer IRQ overwrote `switched_from`; see context.s).
    // `finish_switch` therefore runs masked, as it must. Only now, once it has completed, do we
    // unmask, so this kernel thread's closure is preemptible.
    crate::sched::finish_switch();
    crate::arch::interrupts::enable();

    call(closure);

    crate::sched::exit();
}
