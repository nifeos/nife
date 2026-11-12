# P2: later

An appendix to [the milestone](../1095-inter-process-communication-outside-review.md).

## P2: after the move, before the first publish

10. A timer and a futex table live in a crate named for inter-process communication.
    Finding: the crate root calls itself "the decision core of the kernel's IPC" and then lists
    a timer and futex wait queues. A stranger looking for either would not look here, and one who
    arrives for the rendezvous meets two subsystems that are not communication between processes.
    The four share a reason to be together, which the note states and the README should. They are
    the four things the kernel's IPC-tables lock protects. Each is a safe state machine over the
    same `Unqueued<T>` tokens with the same proof shape.
    Evidence: `src/lib.rs` lines 1 to 2; `notes/inter-process-communication.md` ("The timer",
    "The futex queues").
    Change: D2 below. Nothing moves before the architect rules.
    Door: one-way at publish (the crate name is a claim about its contents). Effort: XS to
    document, M to split.

11. The one-queue invariant could hold by construction.
    Finding: `Rendezvous` holds two `Queue<T>` (three words each), a `u32` and a `bool`, about 56
    bytes on a 64-bit target (counted, not measured), and three harnesses exist to show that at
    most one queue is non-empty. seL4's endpoint has one queue and a state tag, so the property
    needs no proof. `enum Waiting<T> { Nobody, Senders(Queue<T>), Receivers(Queue<T>) }` beside
    the count and the flag would make `one_queue_invariant_holds` trivially true, remove about 16
    bytes per object, and leave `send`, `receive` and `signal` as a `match` on the tag. The cost is
    rewriting the seed, the three invariant harnesses and the `remove_*` pair, and the kernel's
    debug dump.
    Evidence: `src/rendezvous.rs` lines 48 to 62 and 180 to 182; `src/verification.rs` `seed`.
    Change: an option for the maintainer, not a must. If taken, before publish, since it changes
    which proofs the README lists.
    Door: two-way (the layout lives in kernel pages, not on any wire). Effort: M.

12. `Futexes` as a stranger reads it.
    Finding, one line each:
    - The const parameter is `B`; parameters follow the naming rules, and `BUCKETS` is the word
      the documentation uses.
    - `Futexes<T, 0>` compiles and panics on the first `park` (`% B` with `B == 0`). A
      `const { assert!(B > 0) }` in `new` refuses it at compile time.
    - `queued()` returns a count; std's spelling is `len`.
    - `waiters()` takes `&mut self` to count, because `Queue` has no iterator; a `Queue::iter`
      yielding `NonNull<T>` in `intrusive_queue` (a pointer walk that dereferences nothing but the
      link) would make `waiters` and `take`'s scan read-only. That change lands in the dependency.
    - The type is a plural noun; `FutexTable` names the thing (Linux's `futex_hash_bucket` table,
      `parking_lot_core`'s hash table). `Key::space` is a generic word for the address space's
      name; `address_space` says which.
    Evidence: `src/futex.rs` lines 47 to 57, 76 to 80, 132 to 150.
    Change: the renames go in the package (item 1); the `const` assertion and the iterator are
    the maintainer's.
    Door: the names are one-way at publish; the assertion and the iterator two-way. Effort: S.

13. `Timer::deadline` answers with a sentinel.
    Finding: `deadline()` returns `NEVER` (`u64::MAX`) for "nothing armed", and `earliest` folds
    the same sentinel. The kernel needs the sentinel at exactly one place, the `AtomicU64` cache,
    and nowhere else. Std's spelling of "maybe" is `Option<u64>`. A timer armed at exactly
    `u64::MAX` and a disarmed one are then distinguishable, which today they are not (the
    documentation says so and calls it the same thing said the other way round).
    Evidence: `src/timer.rs` lines 47 to 49, 111 to 114, 150 to 152.
    Change: `deadline() -> Option<u64>`, `earliest(...) -> Option<u64>`, and `NEVER` kept for the
    kernel's cache word with `lowered` and `is_due` unchanged. The three timer harnesses adapt in
    a few lines.
    Door: one-way at publish (a public signature). Effort: S.

14. The pending count's saturation has no host test.
    Finding: `Rendezvous::signal` saturates `pending` at `u32::MAX` and no test reaches it; the
    proofs reach it only through Kani's overflow check (item 8). `tests.rs` is inside the crate
    and can set `e.pending = u32::MAX` directly, as the notification test does for its word.
    Evidence: `src/rendezvous.rs` line 318; `src/tests.rs`.
    Change: one test: saturate, signal, assert the count stays unchanged and `receive` still drains
    one. A likely kill for one of item 9's survivors.
    Door: two-way. Effort: XS.

15. Five copies of the test node.
    Finding: the node struct `N` (or `T`), its `unsafe impl Node`, and a `mint` or `token` helper
    appear five times: `tests.rs`, `verification.rs`, the notification tests and proofs, and
    the futex tests. Each copy carries its own SAFETY paragraph for the same two obligations.
    Evidence: `src/tests.rs` lines 9 to 31; `src/verification.rs` lines 4 to 32;
    `src/notification.rs` lines 256 to 283 and 396 to 415; `src/futex.rs` lines 157 to 190.
    Change: one `#[cfg(any(test, kani))] mod test_node` with the struct, the impl and the two
    helpers, and the two obligations stated once above it.
    Door: two-way. Effort: S.

16. Documentation details a stranger trips on.
    Finding: the futex module heads its examples `# EXAMPLES` while the others write
    `# Examples`, rustdoc's convention. `Key::address` says "4-aligned" and nothing in the type
    enforces it; the kernel checks before it builds a key. `Notification::is_idle` ignores the word
    and `Rendezvous::is_idle` ignores the count, which both say. A reader comparing the two
    `debug_counts` tuples still meets `(usize, usize, u32)` and `(usize, u64)` with no field names.
    A small struct with named fields reads better in a hang dump and costs nothing.
    Evidence: `src/futex.rs` lines 10 and 51; `src/rendezvous.rs` line 192;
    `src/notification.rs` line 163.
    Change: the heading, "4-aligned, which the caller checks", and named diagnostic structs if the
    kernel's dump code is willing.
    Door: two-way. Effort: XS.

17. Two Kani toolchain warnings the README would otherwise have to explain.
    Finding: `cargo kani` prints `trailing semicolon in macro used in expression position` twice,
    at an `assert!` used as a match-arm expression; Kani's own `assert` macro raises it. Braces
    around the arm body remove the warning.
    Evidence: `src/notification.rs` line 364; `src/verification.rs` line 117.
    Change: `Signal::ToBound(_) => { assert!(!had_waiter && bound) }` and the same at the other
    site.
    Door: two-way. Effort: XS.
