---
status: REFUSED
raised: 2026-09-20
refused_by: 188, 448
---
# 467. `ReplyRecv` fusion: the IPC round trip in two syscalls instead of three

Refused by milestone 188 (design/roadmap/0188-ipc-fastpath.md), which records it in its Follow-on.
Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that names work a
number, a status and a condition that would change it.

## The refusal, in its own words

From '188. The IPC fastpath: the gate measures a shape userspace does not use, and three cheaper
cuts come before a hand-written path', under `## Follow-on`:

> `ReplyRecv` fusion, which would take the round trip from three syscalls to two. It is a
> syscall-surface change, DECISIONS §10 and §16 govern it, and the block already says it is named
> so it is tracked and not so it is planned. A lane must not take it.
>
> -- design/roadmap/0188-ipc-fastpath.md

## Why it is here rather than only there

A server's loop today is reply, then receive, then handle. Fusing the first two is the standard
microkernel move and it takes the round trip from three syscalls to two. It is also a change to the syscall surface, governed by §10 (process model: capability-based, microkernel) and §16 (object revocation: reclaim the objects a process built), which AGENTS.md treats as a boundary rather than a habit and which this tree puts
in calef's hands rather than a lane's. The block that named it was explicit: it is named so it is
tracked, not so it is planned, and a lane must not take it.

## Revisit

- **Unstated.** The refusal names an authority rather than a trigger. It says which decisions govern
  the change and who makes them, and it does not say what measurement or workload would justify making
  it, so there is nothing here for a bell to ring on. Closing that gap is one sentence of the form "a
  fused round trip is worth the surface when the three-syscall path costs N", and whoever takes the
  next fastpath measurement is the person to write it.

## Index row

The obvious next cut on the IPC fastpath, deliberately named and deliberately not planned, because
it widens the syscall surface. It is recorded with the status that says a lane must not take it, and
with the honest note that no measurement was named that would justify taking it.
