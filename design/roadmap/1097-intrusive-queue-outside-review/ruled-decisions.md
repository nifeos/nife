# Ruled decisions

An appendix to [the milestone](../1097-intrusive-queue-outside-review.md). The decision below is
the rule in force; the milestone's "Open decisions" holds the ones still calef's.

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

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door, which the architect may
revisit. The seven keep their spelling as they stand. Nothing outside nife acts on the names until
the first release. Ratifying them is calef's, under the milestone's "Open decisions".
