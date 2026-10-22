//! The kernel's lock.
//!
//! # The deadlock this exists to prevent
//!
//! A plain spinlock in a kernel that takes interrupts is a hang waiting for a schedule.
//! On **one core**:
//!
//! ```text
//!   kernel code:  ALLOCATOR.lock()   <- acquired
//!                 ...working...
//!        TIMER INTERRUPT
//!   handler:      ALLOCATOR.lock()   <- spins
//!                                       spins waiting for a lock that only the code it
//!                                       interrupted can release, and that code cannot
//!                                       run until the handler returns.
//!                                       Dead. Permanently. On one core.
//! ```
//!
//! This is not a race. It is a **guaranteed** hang the moment the timing lines up.
//!
//! The fix: **mask interrupts for as long as the lock is held.** The interrupt cannot fire,
//! so it cannot try to take the lock. This is Linux's `spin_lock_irqsave`.
//!
//! # Two orderings that are the entire point
//!
//! **Acquire: mask interrupts FIRST, then take the lock.** The other order leaves a window
//! where we hold the lock with interrupts still enabled, which is precisely the deadlock.
//!
//! **Release: drop the lock FIRST, then restore interrupts.** The other order leaves a
//! window where interrupts are live and we still hold the lock. Same deadlock, arrived at
//! from the other side.
//!
//! Both windows are one or two instructions wide, and both are fatal.
//!

//! # Restore, do not enable
//!
//! [`IrqSafeGuard`] restores the interrupt state that was in effect when the lock was
//! taken. It does **not** simply enable interrupts on release.
//!
//! The difference matters when a lock is taken inside a context that already had interrupts
//! masked (an interrupt handler, or an outer lock): enabling on release would unmask interrupts
//! *inside an interrupt handler*. Hence Linux's `irqsave`/`irqrestore`.
//!
//! See notes/locking.md and DECISIONS §9.

use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::Ordering;

use crate::arch::interrupts;

/// # Lock ranking: the rule from DECISIONS §9, enforced by the machine
///
/// Two locks need a global order, or **AB-BA deadlock**. Every lock carries a **rank**, and **you
/// may only acquire a lock strictly LOWER than everything you currently hold.** A cycle is then
/// **unrepresentable** ([notes/deadlock.md](../notes/deadlock.md)), with no cycle hunting as in
/// Linux's `lockdep`; FreeBSD (WITNESS) and Solaris use the same mechanism.
///
/// ## The hierarchy
///
/// ```text
///   61  ADDRESS_SPACES         user-built address spaces
///        |
///   60  IPC_TABLES      the thread table and endpoints
///        |
///   59  INBOX, MAPPINGS, KMEM   inboxes; the revocation registry; the kernel object budget
///        |
///   58  MEMORY_REGION         the untyped regions
///        |
///   57  CAPABILITY_TABLE  one thread's capability table (one lock per thread)
///        |
///   55  STACK_VA        free thread-stack addresses
///        |
///   30  FRAMES, RAM     the physical memory map
///        |
///   20  IRQ_CONTROLLER  the interrupt controller (the GIC, or the PLIC)
///        |
///   10  CONSOLE         the leaf: everyone may take it, it takes nothing
/// ```
///
/// Two locks at the **same** rank may never be nested (`R < R` is false): equal rank declares no
/// order between them.
///
/// The nestings this permits, and they are the ones that actually happen:
///
/// - **MAPPINGS (59) → `MEMORY_REGION` (58)**: recording a mapping retypes a log page from the
///   paying process's own region, while holding the registry lock.
/// - **`IPC_TABLES` (60) → `CAPABILITY_TABLE` (57)**: an IPC that delivers a capability, and every
///   revocation sweep, locks another thread's table while holding the thread table. The reverse
///   is refused: a holder of a capability table takes nothing (see the rank's own note).
/// - **anything → CONSOLE (10)**: a panic prints while holding a lock, so the console is the leaf.
pub mod rank {
    /// The user-built address spaces (milestone 19b): the registry behind `Object::AddressSpace`
    /// capabilities. **Top of the order**: a `MAP_INTO` holds this while drawing page tables
    /// from the space's region (`MEMORY_REGION`, 58) and writing the revocation record (MAPPINGS, 59),
    /// and it never touches `IPC_TABLES` (capability grants happen after release).
    pub const ADDRESS_SPACES: u32 = 61;

    /// The thread table and the endpoints.
    ///
    /// **Above everything that frees**, because the reaper drops a dead thread's kernel stack
    /// while holding it (frames back to the allocator, the VA range to its list). Nothing under
    /// it allocates: the queues are intrusive and the TCBs are a static pool (milestone 14), so
    /// `schedule()` is safe from the timer interrupt by construction (§9).
    pub const IPC_TABLES: u32 = 60;

    /// A core's migration inbox (SMP step 3c). The one cross-core scheduler structure: another
    /// core locks it to hand this core a thread, and this core drains it in its reschedule-SGI
    /// handler. **Just below `IPC_TABLES`**, because placing a thread on a remote core is done
    /// while holding `IPC_TABLES` (it reads the thread table). A push is two pointer writes (the
    /// queues are intrusive) and cannot allocate. Inboxes are all this one rank and are never
    /// nested: a core locks at most one inbox at a time (the target's), so `R < R` being false
    /// forbids the only cycle. Shares the rank with MAPPINGS, with which it is also never
    /// nested (inbox traffic is scheduling; the registry is syscalls and revocation).
    pub const INBOX: u32 = 59;

    /// Memory regions (milestone 11; fixed table since 14 B.1). **Below MAPPINGS**, so
    /// recording a mapping may retype a log page from the payer's region while the registry is
    /// held (phase C). Below `IPC_TABLES`, so it may be taken from a syscall that has no
    /// thread-table-or-endpoint business.
    pub const MEMORY_REGION: u32 = 58;

    /// **One thread's capability table** (provisional name, from the proposal "capability lookup off
    /// the global lock", 2026-10-04 UTC). Every thread's table has its own lock at this rank, in the
    /// free space of its TCB page (`thread::capability_table_of`).
    ///
    /// **The order, written down because the speed depends on it:**
    ///
    /// - The running thread's own lookups (`sched::current_cap`, `grant`, `grant_at`) take this
    ///   lock **alone**, with `IPC_TABLES` not held. That is the whole point: before 2026-10-04
    ///   every syscall's lookup took the global lock, and at four busy cores on radon 41% of them
    ///   found it held (notes/job-mix/null-syscall-under-load.md).
    /// - Everything that touches **another** thread's table (an IPC that delivers a capability,
    ///   `ThreadControlBlock::CAP_INSERT`, the revocation sweeps) takes it **under `IPC_TABLES`**,
    ///   which is what keeps a capability in transit between two tables, or a sweep across all of
    ///   them, atomic against every other cross-table operation.
    /// - **A holder takes nothing.** The table is a fixed array of `Copy` entries, so no operation
    ///   on it allocates, frees or prints. Two tables are never held at once (equal rank refuses
    ///   it), and `IPC_TABLES` is never taken under one (60 > 57 refuses it). Both refusals are this
    ///   module's ranking check, so an AB-BA between a lookup and a sweep is unrepresentable.
    ///
    /// **Below `MEMORY_REGION` (58) and the 59s**, not because anything nests them today but so a
    /// caller holding one of those could still look a capability up; and above everything the
    /// table's holder might reach on a panic path, which is only `CONSOLE`.
    pub const CAPABILITY_TABLE: u32 = 57;

    /// The kernel's own object budget (milestone 19c.1): `kmem`, the region kernel stacks draw
    /// from. **Above `MEMORY_REGION`** (it carves and retypes from its region while holding this lock,
    /// so KMEM -> `MEMORY_REGION` must strictly decrease) and **below `IPC_TABLES`** (a stack's `new`/`Drop`
    /// runs from spawn and the reaper, which hold `IPC_TABLES`). Shares rank 59 with INBOX and
    /// MAPPINGS, never nested with either: inbox traffic is scheduling, the mapping registry is
    /// user bookkeeping, and this is the kernel buying its own pages.
    pub const KMEM: u32 = 59;

    /// The virtio transport table (DMA confinement; fixed since 14 B.1). Taken from a syscall
    /// with no other lock held; the rank records where that syscall sits, not any nesting need.
    pub(crate) const VIRTIO: u32 = 56;

    /// The IOMMU driver's device (milestone 16b): the SMMUv3's stream table and queues, the
    /// RISC-V IOMMU's device directory and queues, whichever this arch has. A pure leaf: `attach`
    /// and `take_fault` do register and in-memory-queue work and lock nothing beneath them, and
    /// the DMA domain's page-table frames are allocated *before* this lock is taken (see
    /// `crate::iommu::confine`), so the lock is never held across an allocation. Below VIRTIO
    /// because a PCI device is confined from the same bring-up path that registers its transport,
    /// though the two locks are never actually nested (confine runs before DEVICES is taken).
    pub const IOMMU: u32 = 54;

    /// The free list of thread-stack virtual addresses (a fixed array since 14 B.1).
    ///
    /// **Below `IPC_TABLES`**, because a `KernelStack`'s `Drop` runs from the reaper, which
    /// holds `IPC_TABLES`.
    pub const STACK_VA: u32 = 55;

    /// The revocation registry (§13; reworked at 14 phase C): which address spaces live, and
    /// where their mapping logs are. **Above `MEMORY_REGION`**, because recording a mapping retypes a
    /// log page from the paying space's region under this lock. Never held together with `IPC_TABLES`
    /// in either order: the capability sweep (`IPC_TABLES`) and the unmap sweep (this) run one after
    /// the other, and the reaper drops address spaces outside `IPC_TABLES` (see `finish_switch`).
    pub const MAPPINGS: u32 = 59;

    /// The kernel's own page tables (`mmu::map_page` / `unmap_page`).
    ///
    /// Single-core, `kernel_mapper()` needed no lock: the callers happened not to race. SMP breaks
    /// that (two cores spawning threads both mutate the shared TTBR1 tables), so mapping is now
    /// serialized. **Below `IPC_TABLES`** (a `KernelStack`'s `Drop` unmaps from under `reap`, which
    /// holds `IPC_TABLES`) and **below `STACK_VA`** (a stack's `new` maps pages), and **above the allocators**
    /// (mapping allocates intermediate page-table frames). See DECISIONS §11.
    pub(crate) const KERNEL_MMU: u32 = 45;

    pub(crate) const PAGE_FRAMES: u32 = 30;
    pub const RAM: u32 = 30;

    /// The ASID allocator (milestone 15). Taken alone at address-space creation and teardown;
    /// near the leaves because it needs nothing beneath it.
    pub(crate) const ASIDS: u32 = 15;

    /// The interrupt controller, whichever one this architecture has: the GIC on aarch64, the PLIC
    /// on RISC-V.
    ///
    /// Taken by the IRQ handler, which by our own rule (DECISIONS §9) holds nothing and
    /// allocates nothing. So it can sit low, just above the console: the handler may still
    /// `println!` a diagnostic while holding it.
    ///
    /// **One rank, one story, deliberately.** It was `GIC` until the PLIC needed a lock of its own
    /// (the enable-bit read-modify-write; see drivers/plic.rs). The two drivers are mutually
    /// exclusive at *compile* time (`drivers/mod.rs` gates each to its ISA), so they are not two
    /// locks that must be ordered against each other, they are one lock role with two
    /// implementations. Giving that role two names at the same number would invite the reader to
    /// wonder which comes first, when the answer is that they never coexist. Renamed rather than
    /// duplicated, which is also the direction §17's HAL-leak cleanup pushes: portable machinery
    /// should not be named after one ISA's controller.
    ///
    /// `x86_64` never takes it: the local APIC's claim is one register read with no shared state to
    /// guard, so the rank has two implementations rather than three.
    #[cfg_attr(target_arch = "x86_64", allow(dead_code))] // the GIC and PLIC drivers are its only takers
    pub const IRQ_CONTROLLER: u32 = 20;

    /// The ISA record (milestone 60): what the machine said it is, written once at boot.
    ///
    /// A leaf, and above the console because the boot print reads it and then prints. It copies the
    /// record out and releases before the first `println!`, so the two are never actually nested,
    /// but the ordering is stated rather than relied on to stay accidental.
    pub(crate) const ISA: u32 = 12;

    /// The `x86_64` ACPI walk (milestone 890 (riscv64 and `x86_64` join the shared boot path)):
    /// what the tables said, written once by `memory::init` and copied out by the bring-up steps
    /// after it. A leaf for the ISA record's reason, and above the console because a reader may
    /// print while it holds it.
    #[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))] // ACPI is x86_64's description
    pub(crate) const ACPI: u32 = 13;

    /// Why the last region reclaim refused (milestone 321), test builds only. A leaf written under
    /// `IPC_TABLES` and read with nothing held; it takes nothing beneath it. Name: provisional.
    #[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
    pub(crate) const RECLAIM_REFUSAL: u32 = 11;

    pub const CONSOLE: u32 = 10;

    /// Holding nothing.
    pub(crate) const NONE: u32 = u32::MAX;
}

/// The lowest rank currently held is kept in **this core's per-CPU block**
/// (`cpu::current().held_rank`), not a global.
///
/// It used to be a single `static`. That was correct on one core and a bug on two: a second
/// core taking a lock would clobber the first core's held-rank, and the ranking would start
/// reporting violations that never happened, which is worse than not checking. Moving it
/// per-CPU (DECISIONS §11, step 1) fixes that. It is still only ever touched by its owning
/// core with interrupts masked, so the atomic is for interior mutability, not synchronization.
///
/// What is the lowest-ranked lock we currently hold? Test support.
#[cfg_attr(not(test), allow(dead_code))] // the ranking tests; also a debugger's friend
pub(crate) fn current_rank() -> u32 {
    crate::cpu::current().held_rank.load(Ordering::Relaxed)
}

/// Would taking a lock of this rank violate the hierarchy? Test support.
///
/// Exists because the violation itself is an `assert!`, and an assert in a kernel test is a
/// dead kernel. This lets the tests check the *predicate* without pulling the trigger.
#[cfg_attr(not(test), allow(dead_code))] // the ranking tests are the only callers
pub(crate) fn would_violate(rank: u32) -> bool {
    rank >= current_rank()
}

/// Forget everything we thought we held.
///
/// # Safety
///
/// **Panic and fault paths only**, alongside `console::force_unlock`.
///
/// If we panic while holding the console lock (rank 10), then this core's held-rank is 10, and
/// the panic handler's own attempt to print would try to take rank 10 again. `10 < 10` is
/// false, so the ranking would fire a *lock-order violation panic inside the panic handler*, and
/// we would lose the original message to a recursive panic.
///
/// The bookkeeping is a debugging aid. It must never be the thing that stops us saying what
/// went wrong. Resets only the calling core's block, which is exactly right: a fault is handled
/// on the core that took it.
pub(crate) unsafe fn force_reset_ranks() {
    crate::cpu::current()
        .held_rank
        .store(rank::NONE, Ordering::Relaxed);
}

/// **The lock-order panic, out of line and shared by every lock site** (milestone 758 (the IPC
/// fast paths shrink back inside their band), provisional). An inline `assert!` with a formatted
/// message built its two-argument `fmt::Arguments` at every `lock()` the optimizer inlined, about
/// 50 bytes a site on riscv64 and more on x86_64, and every IPC primitive takes `IPC_TABLES` at least
/// once: measured on 2026-10-04 UTC it was the second-largest single line in
/// `script/fastpath-footprint`'s closures on all three ISAs. The check itself (one compare, one
/// branch) is unchanged; only the bytes no healthy kernel ever fetches moved here.
///
/// Name: provisional, like every new function in this tree until an architect rules.
#[cold]
#[inline(never)]
fn lock_order_violation(rank: u32, held: u32) -> ! {
    panic!(
        "LOCK ORDER VIOLATION: taking a rank-{} lock while holding rank {}. \
         Locks must be acquired in strictly decreasing rank. See kernel/src/sync.rs.",
        rank, held,
    );
}

/// A spinlock that masks interrupts while it is held, and enforces a global lock order.
///
/// **Every lock in the kernel should be one of these.** See the discipline in
/// DECISIONS §9, particularly: keep the critical section short, because interrupts are
/// off for the whole of it.
pub struct IrqSafeMutex<T> {
    inner: spin::Mutex<T>,
    rank: u32,
}

// SAFETY: same reasoning as any mutex. The lock provides the exclusion.
unsafe impl<T: Send> Sync for IrqSafeMutex<T> {}
// SAFETY: as for `Sync` directly above: the lock provides the exclusion.
unsafe impl<T: Send> Send for IrqSafeMutex<T> {}

impl<T> IrqSafeMutex<T> {
    /// `rank` comes from [`rank`]. See that module: the number is not decoration, it is the
    /// thing that makes an AB-BA deadlock unrepresentable.
    pub const fn new(rank: u32, value: T) -> Self {
        Self {
            inner: spin::Mutex::new(value),
            rank,
        }
    }

    // **`#[inline]`, so every codegen unit gets its own copy to decide on** (milestone 599 (a frame
    // per filesystem client channel), 2026-09-26). Unmarked, this generic is emitted once and can be
    // inlined only inside the codegen unit that owns it, and which unit that is depends on a
    // metadata hash Cargo derives partly from the host triple. The same commit then built two
    // different aarch64 kernels: the lock inlined at every site on a macOS host, outlined per
    // instantiation on CI's Linux one, 25 KiB of text apart, and `script/fastpath-footprint`
    // reported a 16% "shrink" that was only the coin landing the other way. With the hint each call
    // site gets LLVM's ordinary cost decision, the same on every host (checked by building under
    // two metadata seeds that split the old way). `#[inline(always)]` was tried and refused: it
    // inlined sites LLVM had declined and pushed `memory::init` past the 4 KiB guard page.
    #[inline]
    pub fn lock(&self) -> IrqSafeGuard<'_, T> {
        // ORDER: mask first, THEN acquire. Reversing these reintroduces the deadlock.
        let irqs_were_enabled = interrupts::disable();
        self.lock_masked(irqs_were_enabled)
    }

    /// [`lock`](Self::lock) after the caller has already masked interrupts, handing over the state
    /// it found so the guard restores that. Private: [`lock_found`] is the one caller with a reason.
    #[inline]
    fn lock_masked(&self, irqs_were_enabled: bool) -> IrqSafeGuard<'_, T> {
        // From here to the matching restore in `drop`, interrupts are off, so this core is the
        // only thing that can touch its own held-rank.
        let held_rank = &crate::cpu::current().held_rank;
        let held = held_rank.load(Ordering::Relaxed);

        // The check stays inline and its failure does not: see `lock_order_violation`.
        if self.rank >= held {
            lock_order_violation(self.rank, held);
        }

        held_rank.store(self.rank, Ordering::Relaxed);

        // The job mix's lock-wait instrument (`crate::lock_wait`), and nothing in any other build.
        // `try_lock` first is the same compare-and-swap `lock` opens with, so an acquisition that
        // finds the lock free costs what it always did; only one that finds it held is timed.
        #[cfg(feature = "lock_wait")]
        let guard = match self.inner.try_lock() {
            Some(guard) => guard,
            None => {
                let t0 = crate::arch::timer::now();
                let guard = self.inner.lock();
                crate::lock_wait::contended(self.rank, crate::arch::timer::now() - t0);
                guard
            }
        };
        #[cfg(not(feature = "lock_wait"))]
        let guard = self.inner.lock();

        IrqSafeGuard {
            guard: ManuallyDrop::new(guard),
            irqs_were_enabled,
            previous_rank: held,
        }
    }

    /// Break the lock open, whoever holds it.
    ///
    /// # Safety
    ///
    /// **For the panic and fault paths only, and nothing else, ever.**
    ///
    /// If we panic while holding the console lock (a fault taken in the middle of a
    /// `println!`, say), then the panic handler's own attempt to print would take that same
    /// lock and hang. We would lose the one message that mattered, at the exact moment we
    /// needed it. Linux does the same thing and calls it `bust_spinlocks`.
    ///
    /// The caller must accept that whatever the previous holder was doing is now
    /// half-finished, and that its data may be inconsistent. That is an acceptable trade
    /// when the alternative is a silent hang, and an unacceptable one at any other time.
    pub(crate) unsafe fn force_unlock(&self) {
        // SAFETY: this function's own `# Safety` contract is exactly the one this call needs; it forwards, it does not weaken.
        unsafe { self.inner.force_unlock() }
    }
}

/// **Find a lock with interrupts masked, then take it, masking once** (provisional name,
/// 2026-10-04 UTC, from the proposal "capability lookup off the global lock").
///
/// For a lock whose *identity* is per-core state: `find` reads this core's block and names the lock
/// to take. The read must happen with interrupts masked, or the thread could be preempted and resumed
/// on another core between finding this core's block and reading it, and would take the lock that
/// core's thread owns. [`IrqSafeMutex::lock`] masks too late for that, and masking here and again
/// inside `lock` is a second save and restore of the interrupt state on the cheapest syscall's path.
/// `None` from `find` restores the state and takes nothing. The guard restores the state found
/// before `find` ran, so the whole of it is one critical section.
#[inline]
pub(crate) fn lock_found<'a, T>(
    find: impl FnOnce() -> Option<&'a IrqSafeMutex<T>>,
) -> Option<IrqSafeGuard<'a, T>> {
    let irqs_were_enabled = interrupts::disable();
    if let Some(mutex) = find() {
        Some(mutex.lock_masked(irqs_were_enabled))
    } else {
        interrupts::restore(irqs_were_enabled);
        None
    }
}

pub struct IrqSafeGuard<'a, T> {
    guard: ManuallyDrop<spin::MutexGuard<'a, T>>,
    irqs_were_enabled: bool,
    previous_rank: u32,
}

impl<T> Drop for IrqSafeGuard<'_, T> {
    /// `#[inline(always)]` because every IPC drops one of these on the hot path, and it must land in
    /// `.text.hot` with its caller. Left to LLVM, whether it is inlined depends on how the crate
    /// falls into codegen units, which unrelated changes move: riscv64's CI build outlined it on
    /// 2026-10-14 (UTC) and aarch64's and `x86_64`'s on 2026-10-20, each time as its own symbol
    /// outside `.text.hot`, and each time `script/fastpath-footprint` refused it. A macOS build of
    /// the same commit can still inline it, so a local pass of that gate does not show the outlining.
    #[inline(always)]
    fn drop(&mut self) {
        // ORDER: release the lock, THEN restore interrupts. Reversing these leaves a window
        // where an interrupt can fire while we still hold the lock. Same deadlock.
        //
        // SAFETY: we drop the guard exactly once, here, and never touch it again.
        unsafe { ManuallyDrop::drop(&mut self.guard) };

        // RESTORE the rank we found, not `NONE`. Exactly the same reasoning as the interrupt
        // state one line below: a lock released inside an outer lock must not report that we
        // are now holding nothing. Interrupts are still masked here (restored below), so this
        // core still owns its per-CPU block.
        crate::cpu::current()
            .held_rank
            .store(self.previous_rank, Ordering::Relaxed);

        // RESTORE, not enable. See the module docs.
        interrupts::restore(self.irqs_were_enabled);
    }
}

impl<T> Deref for IrqSafeGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T> DerefMut for IrqSafeGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the kernel's lock.
    //!
    //! `irq_safe_mutex_restores_rather_than_enables` is the important one, and it was verified
    //! against a deliberately broken `restore()`: it fails with "dropping the guard ENABLED
    //! interrupts inside an IRQ-disabled context". A test that cannot fail is not a test.

    /// The lock must mask interrupts for as long as it is held.
    ///
    /// If it doesn't, a timer interrupt can land inside a critical section, try to take the
    /// same lock, and spin forever waiting for code that cannot run until it returns. On one
    /// core. Permanently. See notes/locking.md.
    #[test_case]
    fn irq_safe_mutex_masks_interrupts_while_held() {
        use crate::arch::interrupts;
        use crate::sync::{IrqSafeMutex, rank};

        static M: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 7);

        interrupts::enable();
        assert!(interrupts::is_enabled(), "test setup: IRQs should be on");

        {
            let guard = M.lock();
            assert_eq!(*guard, 7);
            assert!(
                !interrupts::is_enabled(),
                "IRQs are still live while the lock is held: this is the deadlock"
            );
        }

        assert!(
            interrupts::is_enabled(),
            "IRQs were not restored after the guard dropped"
        );
    }

    /// **The important one.** The guard must RESTORE the previous state, not enable.
    ///
    /// A lock taken inside a context that already had interrupts masked (an interrupt
    /// handler, or inside an outer lock) must not unmask them on release. Blindly enabling
    /// would turn interrupts back on *inside an interrupt handler*, and the resulting fault
    /// is one you will not enjoy explaining.
    ///
    /// This is exactly why Linux's is called `irqsave`/`irqrestore` rather than
    /// `irqoff`/`irqon`, and it is the single easiest thing to get wrong here.
    #[test_case]
    fn irq_safe_mutex_restores_rather_than_enables() {
        use crate::arch::interrupts;
        use crate::sync::{IrqSafeMutex, rank};

        static M: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        // Pretend we are inside an interrupt handler: IRQs already masked.
        let outer = interrupts::disable();
        assert!(!interrupts::is_enabled());

        {
            let _guard = M.lock();
            assert!(!interrupts::is_enabled());
        }

        assert!(
            !interrupts::is_enabled(),
            "dropping the guard ENABLED interrupts inside an IRQ-disabled context"
        );

        interrupts::restore(outer);
    }

    /// Nesting must not corrupt the state either.
    #[test_case]
    fn nested_locks_restore_correctly() {
        use crate::arch::interrupts;
        use crate::sync::{IrqSafeMutex, rank};

        // A is taken first, so A must OUTRANK B. Before the ranking existed this test could
        // nest any two locks in any order; now the hierarchy is part of the type's contract and
        // the test has to declare which one is the outer.
        static A: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::STACK_VA, 1);
        static B: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 2);

        interrupts::enable();

        {
            let a = A.lock();
            assert!(!interrupts::is_enabled());
            {
                let b = B.lock();
                assert!(!interrupts::is_enabled());
                assert_eq!(*a + *b, 3);
            }
            // The INNER guard dropped. It must not have re-enabled interrupts, because the
            // outer one is still held.
            assert!(
                !interrupts::is_enabled(),
                "the inner guard re-enabled IRQs while the outer lock is still held"
            );
        }

        assert!(
            interrupts::is_enabled(),
            "the outer guard failed to restore"
        );
    }

    // --- lock ranking (DECISIONS §9) ---

    /// Holding nothing means anything may be taken.
    #[test_case]
    fn holding_nothing_permits_any_rank() {
        use crate::sync::{current_rank, rank, would_violate};

        assert_eq!(current_rank(), rank::NONE, "a previous test leaked a lock");
        assert!(!would_violate(rank::KERNEL_MMU));
        assert!(!would_violate(rank::CONSOLE));
    }

    /// The rank tracker follows the locks.
    #[test_case]
    fn taking_a_lock_records_its_rank() {
        use crate::sync::{IrqSafeMutex, current_rank, rank};

        static M: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        assert_eq!(current_rank(), rank::NONE);
        {
            let _g = M.lock();
            assert_eq!(current_rank(), rank::PAGE_FRAMES);
        }
        assert_eq!(
            current_rank(),
            rank::NONE,
            "the guard did not restore the rank"
        );
    }

    /// **The rule.** While holding a lock, you may only take a strictly LOWER one.
    ///
    /// If every acquisition strictly decreases, a cycle is *unrepresentable*. Not unlikely.
    /// Impossible. That is why this is prevention rather than detection: it destroys the
    /// circular-wait condition outright (notes/deadlock.md), instead of building a graph and
    /// hunting for cycles the way Linux's lockdep does.
    #[test_case]
    fn the_hierarchy_permits_only_strictly_decreasing_ranks() {
        use crate::sync::{IrqSafeMutex, rank, would_violate};

        static FRAMES: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        let _g = FRAMES.lock();

        // Lower: fine. This is the panic path printing while holding the allocator.
        assert!(
            !would_violate(rank::CONSOLE),
            "console must be takeable from anywhere"
        );

        // Higher: forbidden. Taking the heap while holding frames is the other half of an
        // AB-BA deadlock waiting to be written.
        assert!(
            would_violate(rank::KERNEL_MMU),
            "rank 50 while holding rank 30 must be refused"
        );

        // EQUAL: also forbidden, and this is the subtle one. Same rank means we have declared
        // no order between the two locks, so nesting them would be choosing one at random.
        assert!(
            would_violate(rank::RAM),
            "two locks of equal rank must never nest: we never said which comes first"
        );
    }

    /// Nesting restores the OUTER rank, not `NONE`.
    ///
    /// Exactly the same shape as the interrupt save/restore two lines away in `drop`, and wrong
    /// in exactly the same way if you get it wrong: releasing an inner lock must not report
    /// that we are now holding nothing, or the next acquisition would be checked against the
    /// wrong ceiling.
    #[test_case]
    fn releasing_an_inner_lock_restores_the_outer_rank() {
        use crate::sync::{IrqSafeMutex, current_rank, rank};

        static OUTER: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::STACK_VA, 0);
        static INNER: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        let _o = OUTER.lock();
        assert_eq!(current_rank(), rank::STACK_VA);
        {
            let _i = INNER.lock();
            assert_eq!(current_rank(), rank::PAGE_FRAMES);
        }
        assert_eq!(
            current_rank(),
            rank::STACK_VA,
            "dropping the inner guard reported that we hold nothing, while the outer lock is \
             still held"
        );
    }
}
