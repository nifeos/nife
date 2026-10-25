# Can pairing be checked mechanically? Measured, not argued

*Name: provisional. After the parent's own heading.*

The measurement behind the `PAIR:` gate that [`notes/memory-ordering.md`](../memory-ordering.md)
describes. A broad check was built, run and refused, and a narrow one shipped.

Broadly, no. The obvious check is per variable: for every atomic touched with `Acquire`, require
a `Release` somewhere on the same atomic. It was built and run against the tree before this note was
written, and here is what it did on a population of 63:

Seven flags, six of them correct code. It flagged all five spin locks, because a lock pairs a
compare-exchange with a store and the receiver expression (`self.0.locked`, `REG.locked`) is not a
stable identifier. It flagged the `USER_FAULTS` protocol on both ISAs, which is the *best* pair in
the tree, because its acquire side is a `fence` and not a load on that variable. It missed `HWID`,
the one genuine finding, because `HWID.get(id)?.load(...)` does not textually name the same thing
`HWID[i].store(...)` does. A check that fires on the exemplary case and stays quiet on the defective
one is not a weak check, it is an anti-check, and shipping it would have taught every future reader
to add an allow-list entry without looking.

That is the measurement. The reason underneath it is structural and does not improve with effort:

- The partner is frequently not an ordering primitive at all. For most of this tree it is a
  blocking IPC rendezvous whose happens-before edge lives in the kernel's lock
  (`crates/spin_lock`), behind every call site. No AST of the two programs contains it.
- The two halves are in different programs, often different crates, sometimes different languages
  (`fixtures/c/c_seam.c`). Cross-binary dataflow.
- The defect is an absence. Finding 7 was a reader with no fence anywhere. There is no token to
  match on, which is why a grep-shaped tool cannot find the exact bug this milestone is named for.

Narrowly, yes, and it is worth having. Milestone 112 faced the same question about SAFETY
comments and shipped a narrow check that is true plus a recorded limitation, rather than a broad one
that only looks like a gate. Same posture here:

> Every `fence` outside test code must carry a `PAIR:` comment naming where its other half is.

`script/lint` enforces it, over the contiguous comment block immediately above the fence, which is
where `SAFETY:` already lives. Twelve sites today.

It checks the bookkeeping, not the ordering, and that limitation is the design. What it buys is
the forcing function. An author made to write down where the partner lives has to go and look for it.
Looking is precisely the step neither of the two authors on 2026-08-04 took. Both wrote a fence,
both wrote a true comment about what their own side does, and neither opened the other file. Writing
`PAIR: serve_frame in components/src/compositor.rs` requires opening `serve_frame`, at which point the
absence is visible. Four of the twelve markers in this tree changed what their author believed while
being written.

Fences only, not all 63 ordered atomics, and the line is principled rather than a budget. A
`Release` store names its variable, so its partner is one grep away and a reader can find it unaided.
`fence()` names nothing at all. It is the one construct in the language whose meaning is entirely
about code somewhere else, which makes it the site where the partner has to be written by hand.
