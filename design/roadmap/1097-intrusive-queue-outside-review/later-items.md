# P2: later

An appendix to [the milestone](../1097-intrusive-queue-outside-review.md).

## P2: after the first release, or in it if cheap

9. The method surface against what the consumer needs.
   Finding: `inter_process_communication` writes the same drain-and-rebuild loop five times
   (`Rendezvous::remove_sender`, `remove_receiver`, `Notification::remove_waiter`,
   `Futexes::remove`, and `Futexes::take` as a filter), each popping every node and pushing the
   survivors back, which writes every surviving link twice where a targeted unlink writes one.
   The tree's notes record that the choice keeps the crate's contract at push and pop and
   its proved invariants the only ones in play. The timed-wait note already says a `Queue::remove`
   is the shape the next feature wants. It also says the proofs move with the crate. `STAILQ_REMOVE`,
   `cursor_mut_from_ptr` and `list_del` are what a reader expects to find.
   Evidence: `crates/inter_process_communication/src/rendezvous.rs` lines 214 to 244,
   `notification.rs` lines 216 to 230, `futex.rs` lines 90 to 128; `notes/inter-process-communication.md`
   ("Taking a thread back off a queue"); `notes/timed-wait.md` item 1.
   Change: `remove(&mut self, victim: NonNull<T>) -> Option<Unqueued<T>>` (O(n) from the head, one
   link write, the victim never dereferenced beyond its link). Add an `extract_if`-shaped method
   taking `impl FnMut(NonNull<T>) -> bool` and returning the matches as a `Queue<T>`. Each gets its
   own Kani harness against the array model and its own falsification patch. Then the five loops
   become calls. `front() -> Option<NonNull<T>>` and `iter()` over `NonNull<T>` are the other two a
   reader expects; add them only when a consumer asks.
   Door: two-way (additive). Effort: M.

10. The harness does not check link hygiene.
    Finding: the proof checks order, loss, length and emptiness; that a popped node leaves with
    `next == None` gets a check only from a host test. One assertion in the pop arm covers it for every
    interleaving to the bound, and a second falsification patch (drop the `set_next(None)` in
    `pop_front`) shows it red.
    Evidence: `src/lib.rs` lines 320 to 334, 449 to 458.
    Change: `assert!(unsafe { (*token.as_ptr()).next() }.is_none())` after the tag check; the patch.
    Door: two-way. Effort: XS.

11. `Unqueued` and `Queue` are thin on the std traits a reader reaches for.
    Finding: `Unqueued<T>` compares only against `NonNull<T>` and only from the left; two tokens do
    not compare, and `NonNull<T> == Unqueued<T>` does not compile. `Queue<T>` has no `Debug`, so a
    struct holding one cannot derive it (the consumer's `Rendezvous` and `Notification` would want
    it for hang dumps).
    Evidence: `src/lib.rs` lines 101 to 111, 154.
    Change: `PartialEq` and `Eq` between tokens, the mirror impl on `NonNull<T>`, and a `Debug` for
    `Queue` printing `head`, `tail` and `len` without walking (a walk would read nodes under the
    caller's contract).
    Door: two-way (additive). Effort: XS.

12. Prose checks a stranger's README will meet.
    Finding: the crate root's first sentence uses `FIFO` unexpanded, which the template's acronym
    report flags on the crate root and the README; `TCB` and `IPC` appear in the harness and test
    comments only. The `#[doc(alias = "fifo")]` on `Queue` is right and stays: the alias is the
    term a reader searches for, and the spelled-out list refuses `fifo` only in a name.
    Evidence: `src/lib.rs` lines 1, 153.
    Change: "first in, first out (FIFO)" at first use, or the words in `.github/accepted-words`.
    Door: two-way. Effort: XS.

13. The tests and the harness each define their own `N`.
    Finding: two identical `Node` implementations for a test node, one under `cfg(kani)` and one
    under `cfg(test)`, with the same `SAFETY` comment. A shared `#[cfg(any(test, kani))]` module
    halves the duplication and gives the Miri test of item 5 a node to use.
    Evidence: `src/lib.rs` lines 259 to 273, 345 to 358.
    Change: one `test_support` module gated on `any(test, kani)`.
    Door: two-way. Effort: XS.
