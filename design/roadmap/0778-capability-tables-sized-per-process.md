---
status: NOT-STARTED
promoted_from: capability-tables-sized-per-process
raised: 2026-10-04
milestone_dependencies: 754
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 778. Capability tables sized per process

Raised 2026-10-04 (UTC) by milestone 753 (trace the progenitor's login block peak), alongside
calef's choice of a 64-slot table. A proposal, not a ruling. Title and slug are drafts, and so is
every name below; `needs_person` is yes because the answer touches the syscall surface.

## The question

Every thread's capability table is the same fixed size, set by one constant. The progenitor needs
31 slots on a busy boot and most processes need a handful. Should a process's table instead be sized
for that process, and grow when it needs to?

## What this tree does today

A flat array of `CAPABILITY_TABLE_SLOTS` entries inside the thread's TCB page, 32 bytes a slot,
indexed directly by the slot number a syscall names. `crates/capability_table`'s own doc calls it "an fd
table with a type tag on each entry" and refuses seL4's tree "because it costs a great deal of
explanation". Each raise so far (16, 17, 24, 32, and 64 in milestone 754 (the capability table grows
to 64 slots)) was one number, paid in every thread's TCB.

## What seL4 does

Read from the seL4 Reference Manual, version 16.0.0, section 3 (fetched 2026-10-04 UTC).

- A capability table is a CNode object. "When creating a CNode the user must specify the number of
  slots that it will have", it is a power of two, and "each slot requires 2^seL4_SlotBits bytes",
  which "is 16 bytes on 32-bit architectures and 32 bytes on 64-bit architectures".
- A CNode is made like any other object: by `seL4_Untyped_Retype()` on the caller's own untyped
  memory. The caller pays for its own table.
- "A CSpace is a directed graph of CNode objects." A slot can hold a capability to another CNode.
  The kernel stores the root CNode capability in the thread's TCB (`seL4_TCB_SetSpace` takes a
  `cspace_root`).
- "CSpaces are implemented as guarded page tables." A capability address is an integer. Lookup
  compares the CNode capability's guard against the address's most significant bits, then uses the
  next radix bits as the index, and continues into a CNode found there while address bits remain.
- Capabilities move between CNodes through CNode methods: `Mint`, `Copy`, `Move`, `Mutate`,
  `Rotate`, and others.

## What it would cost here (§10)

§10 (process model: capability-based, microkernel) keeps the syscall surface narrow and explicit.
A per-process table adds to it, at minimum:

- a new object type for a table, retyped out of a region like any other object;
- a way to give a thread its root table, which is a new thread method;
- a way to move or copy a capability between two tables, if tables can nest or be replaced;
- capability addresses that are paths rather than indexes, if tables nest. Every lookup on the IPC
  fastpath would then walk at least one more level, which `script/fastpath-footprint` and the
  icount tripwire would have to price.

Revocation's sweeps visit every thread's table today; with nested tables they would walk graphs.
A single resizable table per thread, with no nesting, would avoid the path and the walk. It would
still need the object type and the thread method.

## How 64 slots buys time

The progenitor's measured peak went from 21 (2026-09-02) to 31 (2026-10-03): about ten slots in a
month. Milestone 754 leaves 33 slots of headroom. At that rate, extrapolated rather than measured,
that is around three months before the question returns. That is enough to measure what a growable
table costs on the fastpath before choosing one, instead of choosing at the wall.

## What it would do

Measure first: what the fastpath and a lookup cost for a single resizable table and for a two-level
table, on all three ISAs. Then put the options, with those numbers, to an architect.

## What it is not

Not a change to how many slots any table has today. That is milestone 754 (the capability table grows to 64 slots).

## Index row

Every thread's capability table is one fixed size, paid in each TCB, and the size has been raised by one constant five times. The block asks whether a table should be sized per process and grow on demand, compares the tree's flat array with seL4's design, and leaves the answer open because it touches the syscall surface.
