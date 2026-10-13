---
status: NOT-STARTED
raised: 2026-09-24
promoted_from: twelve-decision-gates-cite-only-ruled-sections
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 652. Twelve `DECISION` gates cite only sections that are already ruled

*Title and slug are drafts.*
Raised by milestone 591 (a ruling should make the gate it answers
fail until someone updates it). Its lane converted 27 `DECISION` gates to `DECISION §N` and could
not convert these twelve. Every section each one cites is `DECIDED` or `AMENDED`, so the new token
would fail the day it was written. **Name provisional**: this file's stem is a lane's coinage.

Each fix is prose in one roadmap block, reversible in AGENTS.md's sense. A block that
turns out to hold a fork nobody has written up gets a `PROPOSED` section, and that one needs calef.

## The twelve

| Block | Title |
|---|---|
| 39 | Repository structure for a loosely-coupled OS, and the road to a distribution |
| 48 | Job control: `jobs`, `wait`, `kill`, `fg`, `bg`, and a stopped state |
| 137 | The share as a Mac file server, which is not the same workload as the backup target |
| 172 | A capability-native subprocess primitive: what `cargo`'s "spawn a helper, wait, collect its output" needs, without fork/exec |
| 188 | The IPC fastpath: the gate measures a shape userspace does not use, and three cheaper cuts come before a hand-written path |
| 334 | Color and the pager: the spawn protocol's other two thirds |
| 340 | `script/image-permissions` reports and does not gate, because it is not in the ruleset |
| 356 | Retention declares the thread capability and says nothing about the region |
| 376 | Nothing turns a device back off |
| 388 | An acronym sweep the tree can do at once |
| 391 | Kernel introspection over an endpoint, rather than one syscall per fact |
| 417 | A usurper that reports instead of hanging, so row 26 can be falsified |

## What each one needs

Read the gate paragraph and the block, then do one of three things. If the ruling answered the
block, set the gate to what still stops a start, which is often `NONE`. If the block waits on a fork
nobody has written up, write it as a `PROPOSED` section and gate on `DECISION §N`. If the ruled
section is context and the wait is on something else, say so in the paragraph, as block 39's
already does, and leave `DECISION` bare.

## Index row

Twelve `DECISION` gates cite only ruled sections, so they cannot take milestone 591's checked
`DECISION §N` form; each needs its block read to say what the gate is still waiting on.
