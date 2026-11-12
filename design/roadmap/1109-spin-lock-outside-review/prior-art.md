# Prior art

An appendix to [the milestone](../1109-spin-lock-outside-review.md).

How the crate sits against what a reader already knows. Recent downloads are the crates.io API's
last-90-day figure at the time of the review. The Linux and BSD rows are from memory of those
sources and were not re-read for this review; treat them as unverified.

| lock | shape | fairness | what it has that this crate lacks | what this crate has that it lacks |
|---|---|---|---|---|
| `spin` 0.12.3 (192 M recent / 795 M all-time) | test-and-test-and-set, `RelaxStrategy` generic, optional `lock_api` impl | none | `is_locked`, `get_mut`, `into_inner`, `as_mut_ptr`, `try_lock_weak`, `SpinMutexGuard::leak`, `T: ?Sized`, `Debug`, `Default`, `From<T>`, `Display` on the guard, `RwLock`, `Once`, `Lazy`, ticket and fair variants | atomics loom can substitute; five models and an in-crate falsification; four host tests with a deadline |
| `lock_api` 0.5.0 (275 M / 1.18 G) | the `RawMutex` trait and a generic `Mutex<R, T>` | by the raw lock | a guard with `map`, `MappedMutexGuard`, `ArcMutexGuard`; one guard type shared by every lock | no trait, no dependency |
| `spinning_top` 0.4.0 (20 M / 64 M) | `lock_api` over a test-and-set flag, with `Backoff` | none | everything `lock_api` gives | the same as against `spin` |
| `std::sync::Mutex` | OS futex or pthread | by the OS | poisoning, `get_mut`, `into_inner`, `Default`, `From<T>`, `Debug`, `T: ?Sized`, a `!Send` guard | spins; `const fn new` on bare metal; no allocation; model-checked |
| `spin-lock` 0.2.1 (15 / 5.8 k, last published two years before this review) | a spinlock crate | unverified | nothing this crate wants | the crates.io name that `spin_lock` resolves to (item 4) |
| Linux `qspinlock` (unverified) | MCS queue, 4 bytes | fair | fairness, `spin_lock_irqsave` and the lockdep ranks (which the kernel's `InterruptSafeMutex` supplies above this crate), lock statistics | model-checked happens-before edge at the crate boundary |
| FreeBSD `mtx_lock_spin`, OpenBSD `mtx_enter` (unverified) | owner word, interrupts masked or IPL raised by the lock itself | none | owner recording for `MTX_RECURSE` and witness | the split between the flag and the interrupt discipline, which is what lets loom check the flag alone |

Where this crate differs on purpose: the atomics are loom's under `cfg(loom)` and `core`'s
otherwise. One source is therefore both the thing the kernel compiles and the thing the model
checker replays. Milestone 953 could not get that from `spin` or `lock_api`. What that costs: no
fairness (a waiter may lose every round, as stated), no `?Sized`, no `lock_api` interoperability,
and a thinner accessor surface than any crate in the table. A concrete example of the gap: a reader
coming from `std::sync::Mutex` writes `lock.into_inner()` or `lock.get_mut()` to read a test's final
count. The reader finds neither and reaches for `*lock.lock()`. That works and is what the crate's
own tests do, but it is not what the standard library taught them.
