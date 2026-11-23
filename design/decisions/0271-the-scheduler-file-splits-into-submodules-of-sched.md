---
status: DECIDED
raised: 2026-10-08
decided: 2026-10-11
ratified_by: calef
---

# 271. The scheduler file splits into submodules of `sched`

Raised 2026-10-08 (UTC) as the `unwritten` decision dependency of
[milestone 840 (the scheduler file is split along its seams)](../roadmap/0840-the-scheduler-file-is-split-along-its-seams.md),
whose block listed four questions for an architect. Written by lane
`milestone/840-the-scheduler-file-is-split-along-its-seams`, which brought a recommendation on each
through the maintainer. *(Section number provisional until the merge queue lands it.)*

## The ruling

calef, 2026-10-11 (UTC), in the maintainer session: *"Accept all four sched split
recommendations."*

1. **Submodules of `sched`, not siblings.** Every seam takes `&mut IpcTables` under the one
   `IPC_TABLES` lock. A child module can read its parent's private items, so as submodules the
   table's fields stay private to `sched`; as siblings they would become `pub(crate)` and the whole
   kernel could reach them. Milestone 812 (`std::thread::spawn` runs real threads in one address space) had already cut `sched/configure.rs`, `sched/futex.rs`
   and `sched/process.rs` this way. The recommendation holds at equal cost, so it is not about
   effort.
2. Capability deletion stays in `sched`. `kernel/src/revoke.rs` was the candidate home. The
   deleters walk every thread's capability table under the scheduler's lock, and moving them out
   would move a lock boundary, which milestone 840 does not do.
3. Region reaping gets its own file, rather than sharing one with thread control block
   construction. It is its own seam: a verdict per thread, the objects a region pins, and the
   supervised reap and survey built on them.
4. The size target is milestone 840's: no new file over 1,500 lines, and the parent,
   `kernel/src/sched.rs`, under 3,000. That is tighter than §266 (a Rust source file stays under
   2,000 lines), and the cut meets it.

Every module name ships marked `Name: provisional`, as the naming
authority (`nifeos/nife:notes/skills/naming-authority/SKILL.md`) has a lane do. Ruling on them is still
calef's, and nothing here ratifies one.

## What it does not decide

A file split, not a lock split. `IPC_TABLES` stays one lock at one rank, and no function changes
behavior. Splitting the lock would need a lock-order argument and is not this section.
