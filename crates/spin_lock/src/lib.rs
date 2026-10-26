//! **The kernel's spinlock**: one flag, taken with an Acquire compare-exchange and released with a
//! Release store, and a guard that hands out the data while it holds the flag. `IrqSafeMutex`
//! (`kernel/src/sync.rs`) wraps it and adds interrupt masking and the lock ranks; this crate is
//! only the exclusion and the happens-before edge, written so loom can check both.
//!
//! The release store is what makes everything a holder wrote visible to the next holder, whose
//! Acquire compare-exchange reads it. Most of the kernel's ordering rests on that pair
//! (notes/memory-ordering.md, "Where this kernel's ordering actually lives"). The loom models below
//! explore two and three contending threads, and one of them runs this crate's own lock with the
//! release store turned Relaxed and passes only when loom reports the race. No fairness: a waiter
//! can lose every round, as with any test-and-set lock. See notes/interleaving/what-was-modeled.md,
//! "`crates/spin_lock`, the kernel's lock".
//!
//! # BUGS
//!
//! - Loom models C11, not aarch64 or riscv64; a clean run is not a proof about the silicon
//!   (notes/interleaving.md).
//! - [`SpinLock::force_unlock`] is outside the models: it exists to break a lock whose holder will
//!   never release it, which is a broken invariant by construction.
//!
//! Name: provisional, minted 2026-10-25 (UTC) by the lane lock-crate. Considered: `kernel_lock`
//! (names the user, not the thing) and `test_and_set_lock` (exact, longer than the field's word).

#![cfg_attr(not(test), no_std)]

// The lines that make this crate model-checkable. Under `--cfg loom` the flag and the cell are
// loom's, which record every access, so the model checker replays the lock under every interleaving
// and every reordering the C11 model permits and fails any two accesses to the data that no
// happens-before edge orders. Under anything else they are the real ones and this crate is exactly
// what the kernel compiles. Nothing but `script/interleaving-check` sets the cfg.
#[cfg(not(loom))]
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
#[cfg(not(loom))]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(loom)]
use loom::cell::UnsafeCell;
#[cfg(loom)]
use loom::sync::atomic::{AtomicBool, Ordering};

/// A spinlock holding a `T`. [`lock`](Self::lock) spins until it holds the flag;
/// [`try_lock`](Self::try_lock) tries once.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lock-crate.
pub struct SpinLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

// SAFETY: the flag gives one guard at a time the data, so sharing the lock between threads moves
// the `T` between them and never shares it; that needs `T: Send`, as for `std::sync::Mutex`. The
// claim is the loom models' (`interleavings` below): no two guards' accesses go unordered.
unsafe impl<T: Send> Sync for SpinLock<T> {}

/// The lock, held. Dropping it releases the lock with a Release store.
///
/// The `PhantomData` makes the guard `Sync` only when `T` is, since a shared guard shares the `T`,
/// and `Send` only when `T` is.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lock-crate.
#[must_use = "dropping the guard releases the lock at once"]
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
    _data: PhantomData<&'a mut T>,
}

/// The pointer a guard reads through. Under loom the cell records the access, so a read that no
/// happens-before edge orders against another guard's write fails the model.
#[cfg(not(loom))]
fn read<T>(cell: &UnsafeCell<T>) -> *const T {
    cell.get()
}

/// The pointer a guard writes through; the write twin of `read`.
#[cfg(not(loom))]
fn write<T>(cell: &UnsafeCell<T>) -> *mut T {
    cell.get()
}

#[cfg(loom)]
fn read<T>(cell: &UnsafeCell<T>) -> *const T {
    cell.with(|p| p)
}

#[cfg(loom)]
fn write<T>(cell: &UnsafeCell<T>) -> *mut T {
    cell.with_mut(|p| p)
}

/// One spin-wait step. Loom needs an explicit yield to know the loop waits on another thread; the
/// kernel wants the architectural pause hint.
#[cfg(not(loom))]
fn pause() {
    core::hint::spin_loop();
}

#[cfg(loom)]
fn pause() {
    loom::thread::yield_now();
}

impl<T> SpinLock<T> {
    /// An unlocked lock holding `value`.
    ///
    /// `const` off the loom path so a kernel static can hold one. Loom's atomics and cells have no
    /// const constructor, which is why this is written twice; the kernel only compiles this one.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lock-crate.
    #[cfg(not(loom))]
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }

    /// An unlocked lock holding `value`. See the `const` twin above for why there are two.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lock-crate.
    #[cfg(loom)]
    pub fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }

    /// **Take the lock**, spinning until it is free. Returns only holding it.
    ///
    /// The weak compare-exchange may fail spuriously, which the loop absorbs; while the flag is
    /// set it spins on a Relaxed load, so waiters read a shared line rather than fight for it.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lock-crate.
    #[inline]
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        loop {
            // Acquire on success pairs with the previous holder's Release store in the guard's drop.
            if self
                .locked
                .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return self.guard();
            }
            while self.locked.load(Ordering::Relaxed) {
                pause();
            }
        }
    }

    /// **Try once to take the lock.** [`None`] means another guard holds it.
    ///
    /// Strong, not weak: a [`None`] is always a refusal, never a spurious failure.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lock-crate.
    #[inline]
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
            // `then`, not `then_some`: a guard built eagerly on the refused path would release a
            // lock it never held when dropped.
            .then(|| self.guard())
    }

    /// Release the lock, whoever holds it, with the same Release store the guard's drop makes.
    ///
    /// # Safety
    ///
    /// For a panic or fault path only. Whatever the holder was doing is half-finished, and if the
    /// holder's guard is still live, two guards can now reach the data at once.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lock-crate.
    pub unsafe fn force_unlock(&self) {
        self.unlock(Ordering::Release);
    }

    fn guard(&self) -> SpinLockGuard<'_, T> {
        SpinLockGuard {
            lock: self,
            _data: PhantomData,
        }
    }

    /// The release store, with its ordering as a parameter only so the falsification model can run
    /// this crate's own lock with it Relaxed. Every caller outside that model passes Release.
    #[inline(always)]
    fn unlock(&self, ordering: Ordering) {
        self.locked.store(false, ordering);
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    #[inline(always)]
    fn drop(&mut self) {
        // Release: publish everything this holder wrote to the next holder's Acquire.
        self.lock.unlock(Ordering::Release);
    }
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        // SAFETY: this guard exists only after a successful Acquire compare-exchange set the flag,
        // and nothing clears it before this guard's drop, so no other guard can reach the data
        // until the Release store there; `&self` ties the borrow to the guard. Checked by the loom
        // models (`interleavings` below), not assumed.
        unsafe { &*read(&self.lock.data) }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: as for `deref` directly above, and `&mut self` makes this borrow the guard's only
        // one.
        unsafe { &mut *write(&self.lock.data) }
    }
}

// --- The ordinary host tests -----------------------------------------------------------------
//
// Single-threaded: what each call means. The interleavings are the loom module below.
#[cfg(all(test, not(loom)))]
mod tests {
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::thread;
    use std::time::Duration;

    use super::*;

    /// Every body runs on a worker with a deadline: `lock` spins until it wins, so a mutant that
    /// breaks the release would hang the suite rather than fail it.
    fn bounded(what: &str, body: impl FnOnce() + Send + 'static) {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            body();
            let _ = tx.send(());
        });
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(()) => {}
            Err(RecvTimeoutError::Disconnected) => {
                panic!("{what}: the body panicked; its own message is above")
            }
            Err(RecvTimeoutError::Timeout) => panic!("{what}: the lock was never released"),
        }
    }

    #[test]
    fn a_held_lock_refuses_try_lock_and_its_drop_releases_it() {
        bounded("a held lock refuses try_lock", || {
            let lock = SpinLock::new(7u32);
            let guard = lock.lock();
            assert_eq!(*guard, 7);
            assert!(
                lock.try_lock().is_none(),
                "a held lock let a second guard in"
            );
            drop(guard);
            assert!(
                lock.try_lock().is_some(),
                "the guard's drop did not release"
            );
        });
    }

    #[test]
    fn a_refused_try_lock_does_not_release_the_holder() {
        bounded("a refused try_lock does not release", || {
            let lock = SpinLock::new(());
            let _held = lock.lock();
            drop(lock.try_lock());
            assert!(
                lock.try_lock().is_none(),
                "dropping a refusal released a lock it never held"
            );
        });
    }

    #[test]
    fn writes_through_the_guard_reach_the_next_guard() {
        bounded("writes reach the next guard", || {
            let lock = SpinLock::new(0u32);
            *lock.lock() += 1;
            *lock.try_lock().expect("free after the drop") += 1;
            assert_eq!(*lock.lock(), 2);
        });
    }

    #[test]
    fn force_unlock_releases_a_lock_whose_guard_is_gone() {
        bounded("force_unlock releases", || {
            let lock = SpinLock::new(0u32);
            core::mem::forget(lock.lock());
            assert!(lock.try_lock().is_none());
            // SAFETY: the only guard was forgotten, so nothing else can reach the data.
            unsafe { lock.force_unlock() };
            assert!(lock.try_lock().is_some());
        });
    }
}

// --- The loom models -------------------------------------------------------------------------
#[cfg(all(test, loom))]
mod interleavings {
    //! Every interleaving of two and three threads taking the lock, and every reordering C11
    //! permits within them. Run with `script/interleaving-check`.
    //!
    //! The data inside the lock is a loom cell read and written only through the guard, so an
    //! execution in which two guards' accesses go unordered fails the run: that is exclusion and the
    //! happens-before edge in one property. A second cell outside the lock, written before the
    //! release and read after the next acquire, is the edge the kernel's IPC rendezvous leans on.

    use loom::cell::UnsafeCell;
    use loom::sync::Arc;
    use loom::thread;

    use super::*;

    /// Non-vacuity: a real atomic that accumulates across every execution loom runs, so the
    /// harness can assert afterwards that the interesting branch was reached at all. See
    /// notes/verification.md's non-vacuity section.
    struct Reached(core::sync::atomic::AtomicBool);

    impl Reached {
        const fn new() -> Self {
            Self(core::sync::atomic::AtomicBool::new(false))
        }
        fn mark(&self) {
            self.0.store(true, core::sync::atomic::Ordering::Relaxed);
        }
        fn assert(&self, what: &str) {
            assert!(
                self.0.load(core::sync::atomic::Ordering::Relaxed),
                "vacuous harness: no execution ever reached {what}, so nothing was checked"
            );
        }
    }

    /// The body of a contention model: `takers` threads each take the lock once and increment the
    /// count inside it, and `seen` marks the count each taker found. The model's own thread is one
    /// of the takers, so loom searches `takers` threads and no more.
    fn contend(takers: usize, seen: &'static [Reached]) -> impl Fn() + Sync + Send + 'static {
        fn take(lock: &SpinLock<usize>) -> usize {
            let mut guard = lock.lock();
            let found = *guard;
            *guard = found + 1;
            found
        }

        move || {
            let lock = Arc::new(SpinLock::new(0usize));
            let others: Vec<_> = (1..takers)
                .map(|_| {
                    let lock = Arc::clone(&lock);
                    thread::spawn(move || take(&lock))
                })
                .collect();
            seen[take(&lock)].mark();
            for h in others {
                seen[h.join().unwrap()].mark();
            }
            assert_eq!(*lock.lock(), takers, "an update was lost");
        }
    }

    /// **Two takers never hold the lock at once**, and the second sees the first's write.
    #[test]
    fn two_takers_are_exclusive_and_ordered() {
        static SEEN: [Reached; 2] = [Reached::new(), Reached::new()];
        loom::model(contend(2, &SEEN));
        SEEN[0].assert("a taker that went first");
        SEEN[1].assert("a taker that went second, after the first's release");
    }

    /// **The three-taker model's preemption bound**: every execution in which the scheduler takes
    /// the processor from a running thread at most once. Yields are free, so a waiter may still
    /// spin any number of rounds.
    ///
    /// Not two, measured: at two, loom reaches an execution in which two waiters yield to each
    /// other past 20,000 scheduling branches while the holder never runs, and stops with "Model
    /// exceeded maximum number of branches". That is loom's limit on spin-wait loops, which its
    /// own message names, not a fault in the lock. The two-taker model has no bound.
    const PREEMPTIONS: usize = 1;

    /// **Three takers never hold the lock at once**: each sees every earlier holder's write.
    #[test]
    fn three_takers_are_exclusive_and_ordered() {
        static SEEN: [Reached; 3] = [Reached::new(), Reached::new(), Reached::new()];
        let mut model = loom::model::Builder::new();
        model.preemption_bound = Some(PREEMPTIONS);
        model.check(contend(3, &SEEN));
        SEEN[0].assert("a taker that went first");
        SEEN[1].assert("a taker that went second");
        SEEN[2].assert("a taker that went third, after two releases");
    }

    /// **A held lock refuses `try_lock`**, and a refused `try_lock` leaves the holder holding.
    #[test]
    fn try_lock_refuses_while_another_guard_holds() {
        static REFUSED: Reached = Reached::new();
        static RAN: Reached = Reached::new();

        loom::model(|| {
            let lock = Arc::new(SpinLock::new(0u32));
            let trier = {
                let lock = Arc::clone(&lock);
                thread::spawn(move || {
                    if let Some(mut guard) = lock.try_lock() {
                        RAN.mark();
                        *guard += 1;
                    } else {
                        REFUSED.mark();
                    }
                })
            };
            *lock.lock() += 1;
            trier.join().unwrap();
            let n = *lock.lock();
            assert!(n == 1 || n == 2, "count {n}");
        });

        REFUSED.assert("a try_lock refused because the other thread held the lock");
        RAN.assert("a try_lock that won");
    }

    /// A write outside the lock, made before the release, and read by the next holder only when
    /// the lock's data says it was made. `unlock` is the release store's ordering.
    fn publish_through_the_lock(unlock: Ordering) {
        let shared = Arc::new((SpinLock::new(false), UnsafeCell::new(0u32)));
        let writer = {
            let shared = Arc::clone(&shared);
            thread::spawn(move || {
                let (lock, payload) = &*shared;
                // SAFETY: the reader touches the payload only after seeing `true` under the lock,
                // which this thread stores after this write; the lock's edge orders the two.
                payload.with_mut(|p| unsafe { *p = 7 });
                let mut guard = lock.lock();
                *guard = true;
                // The release, with the ordering under test: the guard is forgotten and the
                // crate's own `unlock` makes the store its drop would have made.
                core::mem::forget(guard);
                lock.unlock(unlock);
            })
        };
        let (lock, payload) = &*shared;
        if *lock.lock() {
            // SAFETY: see the write above. With a Relaxed `unlock` this read is the unsound half
            // of a deliberate counterexample, and loom is expected to say so.
            payload.with(|p| assert_eq!(unsafe { *p }, 7));
        }
        writer.join().unwrap();
    }

    /// **The happens-before edge**: what a holder wrote before its release, even outside the lock,
    /// is visible to the next holder.
    #[test]
    fn a_release_publishes_what_the_holder_wrote_before_it() {
        loom::model(|| publish_through_the_lock(Ordering::Release));
    }

    /// **The falsification.** The same model with the crate's release store Relaxed must fail.
    ///
    /// If this test ever stops panicking, either loom has stopped modeling the C11 relaxed
    /// ordering or the harness above has stopped depending on the release. Both are things we
    /// would want to hear about, and neither would be visible from the passing test alone.
    #[test]
    #[should_panic(expected = "Causality violation")]
    fn a_relaxed_release_publishes_nothing() {
        loom::model(|| publish_through_the_lock(Ordering::Relaxed));
    }
}
