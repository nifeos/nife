---
status: SUPERSEDED
raised: 2026-09-19
superseded_by: 148
---

# 164. Whether the kernel resolves a tid it already sent to the supervisor that received it

Milestone 105 (the two forks named and left) named two forks; one of them was already decided
before the block was written, which is recorded below. This section is the other one.

## The correction that comes first, because it changes what is owed

Milestone 105's fork one is answered and built. The block (2026-08-04) asks whether reclamation
and construction are separable rights, and proposes *"a new rights bit below `WRITE`, or a distinct
`Untyped::REAP` method."* [§32](0032-reap-without-build.md) (a supervisor may collect a corpse
without being able to build one) decided it on 2026-07-29, six days earlier, and decided it
better than either option: authorization is the supervision relationship, not a rights bit and
not a registry.

`crates/abi`'s `rendezvous::REAP` is that decision shipped. Its own documentation states the
property the supervision tree wanted and could not express (quoted from the doc comment on
`rendezvous::REAP`):

> A supervisor therefore needs no capability to the child's memory, and gains none: the reclaimed
> region returns to its owner under §13 region ownership, which is the **builder**. A supervisor can
> free a child's memory; it cannot spend it.

So the block restated a settled question as open. That is the defect milestone 435 exists to find,
and it is the more dangerous direction of it: a gate that says `DECISION` for a reason that has
been answered spends calef's attention on a decision he already made.

## What is being decided, which is fork two

Whether the kernel owes a supervisor the ability to resolve the identity it already sends. The
fault message names a dead thread by tid ([§26](0026-fault-endpoint.md), thread death becomes a
message a supervisor holds). No method turns a tid into something the builder holds, so a
supervisor with several children receives a tid and cannot say which child it belongs to.

## Whether the premise is still true, measured 2026-09-19

Yes, and the tree records it where a reader meets it. There is no `NAME` method anywhere in
`crates/abi`. `crates/ps`'s own limits say it plainly, in its module documentation:

> A process has no name here, so there is no `CMD` column. This system has `arg0` in `Spawn` and no
> display name at all, so the columns are the tid and the run state and that is everything.

One thing has moved and it narrows the question rather than answering it. Milestone 126 added
`rendezvous::SURVEY`, which walks the supervision subtree and returns a tid and a run state per
entry, authorized by the same relationship `REAP` is. So a supervisor can now *enumerate* its
children's tids. It still cannot say which of them is the filesystem server, because nothing in the
kernel holds a name to return.

## The options

| | shape | cost |
|---|---|---|
| **A** | **`Tcb::NAME`**, a method turning a tid into a handle the builder set. | A new method within the established model. **Discloses nothing new**: the tid is already in the fault message, so this reveals no fact the supervisor did not receive. It does put a string, or a builder-chosen word, in the kernel, which is state the kernel does not keep today. |
| **B** | **Per-child fault endpoints.** | Already refused once by §26.5, and for a reason that has not changed: synchronous rendezvous means `RECV` blocks on one endpoint, so this costs a supervisor thread per child or a wait-any primitive that does not exist. |
| **C** | **The builder reports the tid it created**, over a userspace protocol. | No kernel change at all. It makes the supervision relationship depend on a protocol between builder and supervisor rather than on anything the kernel guarantees, and a supervisor that trusts a builder's report is trusting a party it may be supervising. |

Recommendation: none, deliberately. This is a syscall-surface question, which AGENTS.md puts in
the category that reaches calef as options rather than with a winner, because a recommendation on
the surface is most of the way to a decision. What can be said without deciding it is that the
argument between A and C is not about cost (C is free, A is a method) but about who is trusted,
and that is the question to rule on.

What would make C safe is worth naming as a third position: if the builder and the supervisor
are the same principal, C's objection evaporates entirely, and the tree's current tree runs one
sub-server at a time precisely because it has not had to answer this.

## How reversible it is

A is the surface and is not reversible. C is a userspace convention between two programs, which
is the *anything two programs agree on* category: cheaper than the surface, not free.

## What is blocked until this is answered

Milestone 105, which is now one fork rather than two, and through it milestone 23 (the
component OS with live replacement), which is the first consumer with more than one child. The
supervision tree runs one sub-server at a time today and the spawner-issued handle
`sub_server_supervisor` uses works for exactly that reason.

Amendment, 2026-09-27, milestone 126 (the `procps` package: who else is running). Two more
consumers, found by that milestone's lane and recorded in
[`notes/process-view/what-is-left.md`](../../notes/process-view/what-is-left.md) section 2: `w`'s
`WHAT` column and `ps`'s missing `CMD` column. Both ask this section's exact question, a name for a
tid already held, so neither is a new fork and both wait on this one.
