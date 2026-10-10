# Futex wait and wake

*Written 2026-10-10 (UTC) by milestone 812 (`std::thread::spawn` runs real threads in one address
space)'s lane. The ruling is §269 (how threads share a process) fork 2; the wire contract is
`abi::address_space::WAIT` and `WAKE`, and the flags are `abi::futex`.*

## What it is for

`std`'s locks on Linux, the BSDs, Fuchsia and WASI are built on one primitive: park this thread on a
word if the word still holds what I saw, and wake up to `n` threads parked on a word. `Mutex::new`
is `const` and makes no syscall, so the kernel cannot know a lock exists until the first time it
is contended. A futex needs no object, which is why §269 chose it over a capability per lock.

## The contract, in short

| | `WAIT(va, flags, expected)` | `WAKE(va, flags, count)` |
|---|---|---|
| returns | `0` woken, `1` the word differed | how many were woken |
| `flags` | `PRIVATE` with `SIZE_U32`, nothing else (futex2's bit layout) | same |
| the capability | the caller's own space, with `READ` | same |
| `va` | 4-aligned, user half, mapped readable | 4-aligned, user half; need not be mapped |

Refusals come in a fixed order: `BadMethod` (a form §269 reserves: shared, or another size), then
`NotPermitted`, `BadPointer`, `WrongObject`. `WAIT` returns `Gone` if the thread's region is
destroyed under it.

## Why it is correct

The kernel reads the word and parks the thread under one hold of `IPC_TABLES`, and `WAKE` takes
the same lock. A waker stores the new value and then wakes. If the store lands before the waiter's
hold, the lock's release and acquire order it before the waiter's read, and the waiter returns `1`
without sleeping. If it lands after, the waiter is already parked when `WAKE` looks. The kernel's
load is `Acquire`, so whatever the waker wrote before its store is visible to the thread once it
returns.

The queues are `crates/inter_process_communication/src/futex.rs`: 64 buckets of intrusive FIFOs,
keyed by `(space name, address)`. A wake takes waiters oldest first and leaves the others in
their order. A thread torn down while parked is unlinked by `sched::finish_blocked_resident`.

## How it is proved

`system_tests/src/user/futex_tests.rs`, on all three architectures, with a hand-written waiter per
ISA:

- a parked waiter stays parked until its word is woken, ignores a wake of the next word, and then
  returns `1` at once for a word that has changed;
- a waiter whose region is destroyed leaves the table, counted by links, not by key;
- every reserved form and foreign word is refused with its error.

The first two have replayable falsifications. The host tests in the crate cover key collision,
order, `max`, and removal.

## BUGS

- **No timeout.** `std`'s `futex_wait` takes one, for `park_timeout`, `Condvar::wait_timeout` and
  timed lock attempts. `WAIT` has no fourth word for it. A deadline could ride in the sixth argument
  register, the way `CONFIGURE`'s thread pointer does, and be served by the timers milestone 106 (a
  wait that ends on either the interrupt or the deadline) built. Until then those `std` calls cannot
  be written, which the exit test does not need.
- **The waker in the tests is the kernel**, not a second user thread, because two threads cannot
  share a space until the join is built. The syscall layer's checks are proved by a refused caller.
  The milestone's exit test is the end-to-end proof.
- **The word is read through a page-table walk that does not hold the space's own lock.** If a
  concurrent revoke freed the frame between the walk and the load, the load reads one stale word of
  a frame being freed. Nothing read is returned, but whether the thread sleeps depends on it, which
  is a one-bit observation. Not argued either way yet; the fix is to walk under the lock the
  revocation path takes.
- **A wake walks its whole bucket**, so its cost is the bucket's length, not `count`. Bounded by
  `MAX_THREADS / 64` on average; measure before enlarging.
- **Wakes place the woken thread on the waker's core**, as a notification's do. That is right for a
  lock handoff, where the waker is about to block, and wrong for a broadcast to many threads. Not
  measured.
