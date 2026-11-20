# test_and_set_lock

A test-and-set spinlock for `no_std` code: one flag, taken with an Acquire compare-exchange and
released with a Release store, and a guard that hands out the data while it holds the flag. The
release store makes everything a holder wrote visible to the next holder, whose acquire reads it.
The source is written so the loom model checker can run the real lock, not a copy of it, under every
interleaving of two and three threads.

It is not fair: a waiter can lose every round. It is not reentrant: a second `lock` on the thread
that holds the lock spins forever. It does not poison: a panic while holding drops the guard, which
releases the lock and leaves the data as the panic left it. It does not mask interrupts; code that
takes the lock from an interrupt handler wraps it in a type that does. Its guard is `Send` when `T`
is, which the standard library's is not.

`SpinLock::new` is `const`, so a static can hold one. `lock` spins, `try_lock` tries once, and the
standard library's `get_mut`, `into_inner`, `Default`, `From<T>` and `Debug` behave as they do on
`std::sync::Mutex`. A debug print goes through `try_lock` and prints `<locked>` rather than spin.
`T` may be unsized.

## EXAMPLES

```rust
use test_and_set_lock::SpinLock;

static HITS: SpinLock<u32> = SpinLock::new(0);

let other = std::thread::spawn(|| *HITS.lock() += 1);
*HITS.lock() += 1;
other.join().unwrap();
assert_eq!(*HITS.lock(), 2);
```

## What is proven

There are no Kani harnesses. Five loom models run the crate's own lock:

- Two threads each take the lock once and increment a counter inside it: neither update is lost,
  and every access to the data is ordered against every other.
- The same with three threads, at most one preemption per execution (see BUGS).
- A held lock refuses `try_lock`, and a refused `try_lock` leaves the holder holding.
- A write made outside the lock before the release is visible to the next holder.
- The same model with the release store made Relaxed fails, as it must. That model passes only when
  loom reports the race, so it shows the check can fire.

Loom checks the ordering of every access to the data, which is exclusion and publication in one
property. It does not check that two accesses never overlap in time, because the guard's pointer
leaves loom's closure, a pattern loom's documentation says it cannot track for duration. Loom runs
one thread at a time, so for this lock the ordering check is the whole claim.

Loom models the C11 memory model, not aarch64, riscv64 or x86_64. A clean run is not a proof about
the silicon.

## Building and testing

```sh
cargo test                                            # the host tests and the doc-test
cargo build --lib --target x86_64-unknown-none        # also aarch64-unknown-none-softfloat, riscv64imac-unknown-none-elf
RUSTFLAGS="--cfg loom" cargo test --release --lib     # the five loom models
```

Loom is a dependency only under `--cfg loom`, so no ordinary build resolves or compiles it. The
lockfile still lists loom's tree, about thirty packages, which is why it is large for a crate with
no dependencies. `--release` is for speed only; the models do not depend on it.

A falsification of the acquire side is a manual step. Change the success ordering of the
compare-exchange in `lock` from `Acquire` to `Relaxed` and run the loom command: the four positive
models fail and the should-panic model still passes.

## BUGS

- No fairness, no poisoning, no reentrancy, as above.
- `force_unlock` is outside the models: it breaks a lock whose holder will never release it, which
  is a broken invariant by construction.
- The three-taker model is bounded at one preemption. At two, loom stops at its branch limit on an
  execution in which two waiters yield to each other while the holder never runs; that is loom's
  limit on spin-wait loops, not a fault in the lock.
- The acquire-side falsification above is not a gate; only the release side is checked on every run.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
