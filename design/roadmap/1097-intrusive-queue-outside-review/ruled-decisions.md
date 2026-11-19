# Ruled decisions

An appendix to [the milestone](../1097-intrusive-queue-outside-review.md). Each decision below is
the rule in force.

## Decision 1. The `Send` bound on `Queue`

Background. A `Queue<T>` is two pointers and a count. Raw pointers never cross a thread boundary
by themselves, so the crate states by hand that every queue may. A published crate cannot see its
callers: a node holding an `Rc` is `!Send`, and a queue of those crossing threads is what `!Send`
exists to stop. Nothing in the kernel needs the looser form; `Thread` is `Send`.

Options: A, `unsafe impl<T: Node + Send> Send for Queue<T> {}`; B, keep the unbounded form and
document the caller's duty; C, drop `Send` and let each consumer wrap the queue.

Prior art. `std::collections::LinkedList<T>`, `Vec<T>` and `Box<T>` are `Send` only when
`T: Send`, and `intrusive-collections` bounds `SinglyLinkedList<A>` the same way. Where nife
differs: nothing. Example: under A, a queue of nodes that hold an `Rc<Log>` no longer compiles when
handed to a worker thread.

Door: one-way at the first release. A later release can loosen A without breaking anyone, while
tightening B needs a major version.

Decided by the maintainer on 2026-11-19 (UTC), provisional: A. The crate implements it with a
compile-fail doctest on `Queue`.

## Decision 2. The seven public names the release publishes

Background. The ratified block covers the crate name and `Queue`. `Unqueued` is provisional, its
three methods are the lane's, and `Node`, `next`, `set_next`, `push_back`, `pop_front`, `is_empty`
and `len` carry no status. After the first release every one is a name other code spells, and a
rename is a major version.

Recommendation: ratify them as they stand. `push_back`, `pop_front`, `is_empty` and `len` are
`VecDeque`'s spellings, which the standard-library rule asks for. `as_ptr` is `NonNull`'s. `Node`
says what the implementing type is; the alternatives in the field name the glue rather than the
thing (`Adapter` in `intrusive-collections`, `Linked` in `cordyceps`). `Unqueued` says what the
token proves, and it reads correctly at every call site that passes or receives one. `as_non_null`
mirrors `Box::as_non_null`, which is still unstable in the pinned nightly (feature
`box_as_non_null`, issue 157345), so the name matches an unstable std name. `NonNull`'s own
`as_ptr` is stable.

Prior art: the rows of the milestone's prior-art table; `heapless::spsc::Queue` spells `enqueue` and
`dequeue`, a spelling the standard-library rule weighs against.

Concrete example: `let first = ready.pop_front()?; inbox.push_back(first);` reads as the thread
leaving one queue for another, with the token named nowhere because the types carry it. A name
that said `dequeue` or `take_front` would read as well but would stop matching `VecDeque`.

Decided by the maintainer on 2026-11-14 (UTC), provisional: the seven keep their spelling as they
stand.

Ratified by the architect on 2026-11-19 (UTC): option A, all seven as they stand (`Node` with
`next` and `set_next`, `push_back`, `pop_front`, `is_empty`, `len`, and `Unqueued` with `new`,
`as_ptr` and `as_non_null`). The crate's `Name:` blocks read ratified; the move strips them.

## Decision 3. Review clearance and the repository

Background. Clearing a crate review is the go to create its repository under nifeos. This crate had
no review package and no extraction list named it. `inter_process_communication` cannot leave
before it (milestone 1095 (`inter_process_communication`: what an outside maintainer would fix
before it leaves)).

Options: A, a lane writes the package and adds the crate to the externalization list, and the
architect clears once decisions 1 and 2 have their rulings, with no tag and no publication; B,
clear together with `inter_process_communication`; C, hold until the maintainer rules item 9.

Prior art. `generational_table`, `block_roster` and `file_allocation_table` each moved on a cleared
review with no tag. Where nife differs: nothing outside nife links the crate. Example: under A, the
kernel and `inter_process_communication` both pin `nifeos/intrusive_queue` by revision.

Door: one-way: a repository that exists has a name others link to.

Decided by the maintainer on 2026-11-19 (UTC), provisional: A. The package is
`notes/crate-reviews/intrusive-queue.md`, and the go to create the repository is the architect's.
