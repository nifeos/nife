---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: pidwait-waits-on-a-named-tid
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 667. `pidwait`: a way to wait on a named tid with less authority than `pgrep`'s

*Title and slug are drafts.*
Raised by milestone 126 (the `procps` package), building `pidwait` under DECISIONS §226 (`pidwait`
takes tids and composes with `pgrep`). It is a milestone of its own because it needs a new kernel
method.

The wait primitive is a method on the syscall surface, so it is calef's, and
the options are written but not chosen.

## What is owed

§226 ruled `pidwait` a separate program that is named tids and waits until each has exited,
composing with `pgrep`. The shell has pipes and no `$( … )`, so the composable form is
`pgrep | pidwait`, reading decimal tids on its input. That half needs nothing new.

The other half does. Nothing lets a program observe a named tid's exit with less authority than
`pgrep` holds. `rendezvous::RECEIVE` needs `READ` and would take the death message from the supervisor
it was meant for. Polling `SURVEY` needs `ENUMERATE`, which is `pgrep`'s authority and would undo
the reason §226 made `pidwait` a program of its own.

## The options

Section 4 of `notes/process-view/what-is-left.md` carries them with their costs:

1. A method on the supervision endpoint that blocks until a named member has exited, gated by a new
   right below `ENUMERATE`. It refuses the caller's own tid.
2. The same method under `ENUMERATE`, refused by §226's own reason.
3. A per-child exit capability the spawner retains and hands on; capability-exact, and it cannot
   compose with `pgrep`'s output, which is bytes.
4. Notification objects, §101 (notification objects), signaled by the supervisor on each death.

## Two facts the build found, which any option must answer

- `pgrep | pidwait` puts both in one domain, so `pgrep` prints `pidwait`'s own tid, and a program
  here cannot learn its own tid. The primitive has to refuse or skip the caller.
- The tid `pgrep` prints is the full generational name (`crates/ps`'s `write_thread_id`), so a reused
  slot cannot alias a tid already printed.

Every option blocks in the kernel, so none waits on milestone 106 (a wait that ends on either the
interrupt or the deadline).

## Index row

`pidwait` is ruled a program that waits on named tids, but nothing lets a program observe a tid's exit with less authority than `pgrep` holds. Proposed: a wait primitive, which is a new syscall method and so an architect's call; options are written, none chosen.
