---
status: DECIDED
raised: 2026-10-21
decided: 2026-10-27
ratified_by: calef
---

# 313. `ENUMERATE` on a domain covers which of its threads share a process

*Number provisional until the merge queue lands it. Written as a maintainer commit on
`milestone/942-ps-top-process-grouping` (the lane may not write this directory), from the ruling
of 2026-10-27 (UTC), about 22:20.*

Milestone 942 (`ps` and `top` show which process each thread belongs to) needed to know whether
learning that two threads share an address space and a capability table is a new authority.

## The decision

1. No new right. `ENUMERATE` on a supervision domain covers thread-to-process membership. The
   fact is a new `SURVEY` record selector (`abi::survey::record::PROCESS`, provisional), by the
   rule §150 (how does a thread's CPU time reach userspace?) and §204 (how userspace asks where a
   thread runs) set: a new fact about threads the viewer can already name is a new selector value,
   not a new register, method or right.
2. The viewer sees its domain's share of a process, never the whole. The label means "these
   threads of the domain share a process". A thread outside the domain does not appear, and a
   program that counts rows per label counts the threads in its domain, not the process's true
   member count. A confined viewer learns no sibling exists elsewhere.
3. The label is a name for one walk. The kernel answers an ordinal over the domain (the first
   process the walk meets is 1) and hands out no process identity: no `ProcessId`, no address, and
   nothing stable across two runs of `ps`.

## Why

`START` fixes a thread's endpoint (§223 (the process view is the supervision domain)), so
siblings may have different supervisors. The membership relation among tids the viewer already
holds adds no object and no name, and counting inside the domain means it adds no existence either.
A true total would turn `ps` into a probe for threads outside its domain, which §223 refuses by
construction.

## Reversibility

A selector value is cheap to add and a program written against the old values keeps working, so
a later ruling can amend this. A stable process identity would be a new thing a program holds, and that is a
separate decision.
