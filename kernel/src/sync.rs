//! The kernel's lock: a spinlock that masks interrupts while held and enforces a lock order.
//!
//! Mask interrupts first, then take the lock; release the lock first, then restore interrupts.
//! Either other order leaves a window where this core holds the lock with interrupts live, and a
//! handler that wants the same lock spins forever. The guard restores the interrupt state it found
//! rather than enabling, so a lock released inside a handler does not unmask it. See
//! notes/locking.md and DECISIONS §9 (locking).

use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::Ordering;

use crate::arch::interrupts;

/// Lock ranks: a lock may only be taken strictly below every rank this core already holds.
///
/// A cycle is then unrepresentable, so an AB-BA deadlock cannot be written (notes/deadlock.md).
/// Two locks at the same rank never nest. Why each rank sits where it does, and which locks it is
/// taken under, is in notes/locking.md, "The ranks, and why each sits where it does".
pub mod rank {
    /// The user-built address spaces, behind `Object::AddressSpace`. Top of the order: a `MAP_INTO`
    /// takes `MEMORY_REGION` and `MAPPINGS` under it, and never `IPC_TABLES`.
    pub const ADDRESS_SPACES: u32 = 61;

    /// The thread table and the endpoints. Above everything that frees, because the reaper drops a
    /// dead thread's kernel stack while holding it; nothing under it allocates.
    pub const IPC_TABLES: u32 = 60;

    /// A core's migration inbox, locked by another core to hand it a thread. Below `IPC_TABLES`,
    /// under which placement runs. A core locks at most one inbox at a time.
    pub const INBOX: u32 = 59;

    /// The memory regions. Below `MAPPINGS`, which retypes a log page from the payer's region, and
    /// below `IPC_TABLES`.
    pub const MEMORY_REGION: u32 = 58;

    /// One thread's capability table, one lock per thread in its TCB page (provisional name).
    ///
    /// The running thread's own lookups take it alone; another thread's table is taken only under
    /// `IPC_TABLES`; a holder takes nothing. See notes/locking.md for why that order is the speed.
    pub const CAPABILITY_TABLE: u32 = 57;

    /// The kernel's own object budget, the region kernel stacks draw from. Above `MEMORY_REGION`,
    /// which it retypes from while held; below `IPC_TABLES`, under which spawn and the reaper run.
    pub const KMEM: u32 = 59;

    /// The virtio transport table, taken from a syscall with no other lock held.
    pub(crate) const VIRTIO: u32 = 56;

    /// The IOMMU driver's device: the SMMUv3's or RISC-V IOMMU's tables and queues. A leaf; the DMA
    /// domain's page-table frames are allocated before it is taken (`crate::iommu::confine`).
    pub const IOMMU: u32 = 54;

    /// The free list of thread-stack virtual addresses. Below `IPC_TABLES`, because a
    /// `KernelStack`'s `Drop` runs from the reaper.
    pub const STACK_VA: u32 = 55;

    /// The revocation registry (§13 (capability revocation)): which address spaces live, and where their mapping logs are.
    /// Above `MEMORY_REGION`; never held with `IPC_TABLES` in either order.
    pub const MAPPINGS: u32 = 59;

    /// The kernel's own page tables (`mmu::map_page` / `unmap_page`), shared by every core. Below
    /// `IPC_TABLES` and `STACK_VA`, whose holders map and unmap stacks; above the allocators.
    pub(crate) const KERNEL_MMU: u32 = 45;

    pub(crate) const PAGE_FRAMES: u32 = 30;
    pub const RAM: u32 = 30;

    /// The ASID allocator, taken alone at address-space creation and teardown.
    pub(crate) const ASIDS: u32 = 15;

    /// The interrupt controller: the GIC on aarch64, the PLIC on RISC-V. Taken by the IRQ handler,
    /// which may print while holding it. One rank for two drivers that never coexist; `x86_64`
    /// never takes it. See notes/locking.md.
    #[cfg_attr(target_arch = "x86_64", allow(dead_code))] // the GIC and PLIC drivers are its only takers
    pub const IRQ_CONTROLLER: u32 = 20;

    /// The ISA record, written once at boot. A leaf above the console; the boot print copies it out
    /// and releases before printing.
    pub(crate) const ISA: u32 = 12;

    /// The `x86_64` ACPI walk, written once by `memory::init`. A leaf above the console.
    #[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))] // ACPI is x86_64's description
    pub(crate) const ACPI: u32 = 13;

    /// Why the last region reclaim refused, test builds only. A leaf written under `IPC_TABLES`.
    /// Name: provisional.
    #[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
    pub(crate) const RECLAIM_REFUSAL: u32 = 11;

    pub const CONSOLE: u32 = 10;

    /// Holding nothing.
    pub(crate) const NONE: u32 = u32::MAX;
}

/// Return the lowest lock rank this core holds, `rank::NONE` for none. Test support.
///
/// The rank lives in this core's `PerCpu` block, touched only by its own core with interrupts
/// masked.
#[cfg_attr(not(test), allow(dead_code))] // the ranking tests; also a debugger's friend
pub(crate) fn current_rank() -> u32 {
    crate::cpu::current().held_rank.load(Ordering::Relaxed)
}

/// Would taking a lock of `rank` violate the order? Test support: the violation itself is an
/// `assert!`, and a kernel test cannot survive one.
#[cfg_attr(not(test), allow(dead_code))] // the ranking tests are the only callers
pub(crate) fn would_violate(rank: u32) -> bool {
    rank >= current_rank()
}

/// Forget every rank this core holds.
///
/// # Safety
///
/// **Panic and fault paths only**, alongside `console::force_unlock`. A panic while holding the
/// console (rank 10) would otherwise fail the ranking on the panic handler's own print and lose
/// the message to a recursive panic. Resets only the calling core's block.
pub(crate) unsafe fn force_reset_ranks() {
    crate::cpu::current()
        .held_rank
        .store(rank::NONE, Ordering::Relaxed);
}

/// Panic for a lock taken out of order. Out of line and cold, so no `lock()` site carries the
/// formatted message (notes/locking.md, "Why the lock path carries inline hints").
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
/// Every lock in the kernel should be one of these. Keep the critical section short: interrupts are
/// off for the whole of it (DECISIONS §9).
pub struct IrqSafeMutex<T> {
    inner: spin::Mutex<T>,
    rank: u32,
}

// SAFETY: same reasoning as any mutex. The lock provides the exclusion.
unsafe impl<T: Send> Sync for IrqSafeMutex<T> {}
// SAFETY: as for `Sync` directly above: the lock provides the exclusion.
unsafe impl<T: Send> Send for IrqSafeMutex<T> {}

impl<T> IrqSafeMutex<T> {
    /// Make a lock of `rank`, from [`rank`], holding `value`.
    pub const fn new(rank: u32, value: T) -> Self {
        Self {
            inner: spin::Mutex::new(value),
            rank,
        }
    }

    /// Mask interrupts, then take the lock, and return the guard that undoes both.
    ///
    /// Panics if this core holds a lock of `self`'s rank or lower. `#[inline]` so each codegen unit
    /// decides for itself, the same on every host (notes/locking.md).
    ///
    /// Context: any, interrupt handlers included.
    #[inline]
    pub fn lock(&self) -> IrqSafeGuard<'_, T> {
        // ORDER: mask first, THEN acquire. Reversing these reintroduces the deadlock.
        let irqs_were_enabled = interrupts::disable();
        self.lock_masked(irqs_were_enabled)
    }

    /// [`lock`](Self::lock) after the caller has masked interrupts, with the state it found, which
    /// the guard restores. Private: [`lock_found`] is the one other caller. Always inlined, so `lock`
    /// is one call in the gate's -O0 build.
    #[inline(always)]
    fn lock_masked(&self, irqs_were_enabled: bool) -> IrqSafeGuard<'_, T> {
        // Interrupts are off until the guard's `drop`, so nothing else touches this held-rank.
        let held_rank = &crate::cpu::current().held_rank;
        let held = held_rank.load(Ordering::Relaxed);

        // The check stays inline and its failure does not: see `lock_order_violation`.
        if self.rank >= held {
            lock_order_violation(self.rank, held);
        }

        held_rank.store(self.rank, Ordering::Relaxed);

        // `try_lock` is the compare-and-swap `lock` opens with, so only a contended acquisition
        // pays for the timing.
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

    /// Break the lock open, whoever holds it, so a panic can still print (Linux's
    /// `bust_spinlocks`).
    ///
    /// # Safety
    ///
    /// **For the panic and fault paths only, and nothing else, ever.** Whatever the previous holder
    /// was doing is now half-finished and its data may be inconsistent.
    pub(crate) unsafe fn force_unlock(&self) {
        // SAFETY: this function's own `# Safety` contract is exactly the one this call needs; it forwards, it does not weaken.
        unsafe { self.inner.force_unlock() }
    }
}

/// Mask interrupts, find a lock with `find`, and take it, masking once (provisional name).
///
/// For a lock whose identity is per-core state: with interrupts masked, the thread cannot migrate
/// between reading this core's block and taking the lock it names. Returns `None`, with the state
/// restored, when `find` does. The guard restores the state found before `find` ran.
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
    /// Release the lock, restore the rank found, then restore interrupts.
    ///
    /// `#[inline(always)]` so it lands in `.text.hot` with its caller on every build
    /// (notes/locking.md, "Why the lock path carries inline hints").
    #[inline(always)]
    fn drop(&mut self) {
        // ORDER: release the lock, THEN restore interrupts. Reversing these leaves a window
        // where an interrupt can fire while we still hold the lock. Same deadlock.
        //
        // SAFETY: we drop the guard exactly once, here, and never touch it again.
        unsafe { ManuallyDrop::drop(&mut self.guard) };

        // The rank we found, not `NONE`: an inner lock's release must not report that we hold
        // nothing. Interrupts are still masked, so this core still owns its block.
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

    /// The lock masks interrupts for as long as it is held (notes/locking.md).
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

    /// The guard restores the interrupt state it found rather than enabling.
    ///
    /// A lock released where interrupts were already masked, in a handler or under an outer lock,
    /// must leave them masked.
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

        // A is taken first, so A must outrank B.
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

    /// While holding a lock, only a strictly lower rank may be taken (notes/deadlock.md).
    #[test_case]
    fn the_hierarchy_permits_only_strictly_decreasing_ranks() {
        use crate::sync::{IrqSafeMutex, rank, would_violate};

        static FRAMES: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        let _g = FRAMES.lock();

        // Lower: the panic path printing while holding the allocator.
        assert!(
            !would_violate(rank::CONSOLE),
            "console must be takeable from anywhere"
        );

        assert!(
            would_violate(rank::KERNEL_MMU),
            "rank 50 while holding rank 30 must be refused"
        );

        // Equal: also refused, because equal rank declares no order.
        assert!(
            would_violate(rank::RAM),
            "two locks of equal rank must never nest: we never said which comes first"
        );
    }

    /// Releasing an inner lock restores the outer rank, not `NONE`, or the next acquisition would
    /// be checked against the wrong ceiling.
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
