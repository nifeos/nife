---
status: NOT-STARTED
raised: 2026-08-23
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 153. `OutOfMemory` collapses three distinct causes into one error code

Found 2026-08-22 while costing milestone 49's attribution fork: `Untyped::SPLIT`'s
own doc comment already admits it (`crates/abi/src/lib.rs`), but nothing downstream lets a caller,
or a person debugging one, tell the three apart.

**Declined for now (DECISIONS §119, 2026-08-23), for want of a customer**: no caller
is confused by this today, and it was found by pricing milestone 152's future cost rather than by
an actual bug. Same shape as `std::thread::spawn` (§105), hard links (§110), and state handoff
(§116). §119 also records non-binding guidance for whoever eventually has a real customer: new
cause-specific `Error` variants, matching `crates/timetable`'s own `Unbacked`/`Refusal` precedent
and POSIX's `EMFILE`/`ENFILE` split. They are the better-supported shape over a diagnostic query or
leaving it collapsed forever -- not a commitment, re-check it against what that customer's actual
failures look like.

## The gap, checked at the source rather than assumed

`Untyped::SPLIT` (create a new region) returns `Error::OutOfMemory` for three unrelated reasons,
by its own documentation: *"`OutOfMemory` when the budget or region table is exhausted or the
cspace is full"* (`crates/abi/src/lib.rs`, the `SPLIT` doc comment). `RETYPE_OBJ` collapses a
different pair the same way: *"when the untyped is exhausted, the object registry is full, or the
cspace is."* At the syscall layer (`kernel/src/syscall.rs:523`), the kernel-side `Option` from
`untyped::split()` becomes `.ok_or(Error::OutOfMemory)?` with no cause attached; the caller sees one
error code, full stop.

**The three causes are not shades of the same fact.** They point a fix in different directions:

1. Your own region's budget is exhausted. A fact about the caller: ask for a smaller grant, or
   free something first.
2. Your cspace (capability slot table) is full. Also a fact about the caller: it has too many
   capabilities already.
3. `MAX_REGIONS` (256, `kernel/src/untyped.rs`), the system-wide concurrently-live region table,
   is full. A fact about *everyone else on the machine*. Nothing the caller did is wrong, and
   nothing it can do locally fixes it.

A process, or a person debugging one, gets `OutOfMemory` and cannot tell which of these three is
true, which means it cannot tell "I asked for too much" from "some unrelated process elsewhere
exhausted a table I have never heard of."

## Why this is the same shape of defect this tree has already named once

`crates/timetable`'s `Unbacked`/`Refusal` split exists for exactly this reason, in a different
subsystem: *"A `Refusal` is a fact about the line... An `Unbacked` is a fact about the
scheduler... Collapsing the two would tell a person to edit a line that has nothing wrong with
it."* `OutOfMemory` collapses three facts (two about the caller, one about the whole system) the
same way, and nobody has named it here yet.

## What surfaced this

Not found by auditing error codes. Found while pricing milestone 49's attribution fork (DECISIONS
§109 (attribution)) and estimating where the channel model's own cost eventually bites.
`MAX_REGIONS` exhaustion
is the real number that matters for "how many durable sessions can this system support," and tracing
what a caller actually sees when it happens found the three-way collapse.

## The first real customer, 2026-08-26

§119 declined this for want of a customer, on the honest ground that it was found by pricing a
future cost rather than by an actual bug. There is now an actual bug, and it is recorded here as the
evidence that decision asked for rather than as a request to reverse it (that is an architect's
call).

Milestone 49's channel-per-client lane spent two days on a `login` service that answered
`login_contract::DENIED` to a correct password on its second login after start-up. The failing call was
`MemoryRegion::RETYPE`, and the kernel's own implementation is four lines:

```rust
let phys = crate::memory_region::retype_page(region).ok_or(Error::OutOfMemory)?;
let slot = sched::grant(crate::cap::page_frame_cap(phys, Rights::ALL))
    .map_err(|_| Error::OutOfMemory)?; // capability table full
```

The region was fine. The capability table was full, because `MemoryRegion::DESTROY` frees a
region but never the destroyer's own table slot naming it, and the service leaked two slots per
request. Both facts read as one `Error::OutOfMemory` at the call site, and the userspace helper
(`supervision_contract::retype_page_frame_from`) narrows even that to `Err(())`.

What the collapse actually cost, which is the part worth quoting to whoever picks this up:

- Four memory hypotheses were measured and ruled out before the real one was reached: the
  service's construction budget (raised to 16384 pages, no change), its scratch budget (8192, no
  change), `kernel::sched::MAX_RENDEZVOUS`, and `kernel::memory_region::MAX_REGIONS`. Every one of
  them is a thing `OutOfMemory` can mean. None of them was it.
- The capability table was looked at and passed over, because tightening and restoring one slot
  of margin changed nothing, which is exactly what a two-slot-per-request leak does to that
  experiment.
- **What finally resolved it was a temporary kernel-side `println!` in the failing arm**, which is
  to say: reconstructing by hand the distinction the error code had thrown away. `EMFILE` versus
  `ENOMEM` would have ended it in minutes.

This does not by itself argue for a particular shape (§119's three options stand). It argues that
the customer exists.

## What was decided

**Declined for now (DECISIONS §119, 2026-08-23), for want of a customer.** Three shapes were on the
table. New cause-specific `Error` variants (the most direct fix, but a permanent ABI-surface
addition). A separate diagnostic-only query (keeps the hot-path return narrow, costs a second
syscall a caller must know to make). Or leaving it collapsed and documented where a reader meets it
(the `BUGS` entry in `kernel/src/untyped.rs`, unchanged by this decision). §119 (declined for want
of a customer) records non-binding
guidance for whichever of these gets picked up later, favoring the first shape on two precedents
(`crates/timetable`'s own `Unbacked`/`Refusal` split, and POSIX's `EMFILE`/`ENFILE`), without
committing to it now.

## What it unblocks

Nothing else is gated on this. It is named because a durable-session-heavy future (milestone 152)
is exactly the scenario where cause 3 (system-wide exhaustion, not the caller's own fault) would
start being hit by ordinary use rather than only by a misbehaving process. A caller that cannot
tell that apart from its own mistake will be debugged as if it were one.

## Index row

Found while pricing milestone 49's attribution fork: `Untyped::SPLIT` returns `OutOfMemory` for
the caller's own budget exhaustion, a full cspace, or `MAX_REGIONS` (system-wide) exhaustion, and
a caller cannot tell which. The first two are facts about the caller; the third is a fact about
every other live region on the machine. `crates/timetable`'s `Unbacked`/`Refusal` split is the
same shape of fix in a different subsystem, already precedented here. Gate: DECISION, since the
fix touches the shared `Error` enum.
