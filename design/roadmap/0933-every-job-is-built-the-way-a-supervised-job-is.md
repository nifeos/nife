---
status: NOT-STARTED
promoted_from: every-job-is-built-the-way-a-supervised-job-is
raised: 2026-10-18
milestone_dependencies: 48
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 933. Every job is built the way a supervised job is

Raised by milestone 48 (job control)'s phase one, which could give `&`, `wait`, `kill` and `fg`
only to supervised jobs. Name: provisional.

Reuse: the tree's own supervised-job path (`spawn_interruptible` in the progenitor's spawn loop)
and the screen-narrowed tail's shell-held exit endpoint (§106) are the parts to take; no outside
code applies, since the question is this kernel's spawn protocol.

A supervised job is built by the progenitor out of a region the shell holds, so the shell can tear
it down, and shares a frame with the shell, so the shell can interrupt it and see it finish. Every
other job is built in the progenitor's pool. The progenitor deletes its TCB at `START` (§142 (what
a spawner retains over a child after `START`)). The job answers on the one unbadged result
rendezvous every job shares, and `job_undertaker` holds its supervision endpoint and reaps it. The shell holds
nothing for it once it is spawned.

Suppose every job were built from a region the shell holds, with its exit delivered to an endpoint
the shell holds, as a screen-narrowed tail's already is (§106 (take the `terminal_sink_caretaker`
narrowing)). Then job control would apply to every program. A background job's output would go to
the terminal's sink, `wait` would be a receive on that endpoint and a `REAP`, and `kill -9` would be
`DESTROY`.

What it costs, and why it is an architect's call rather than a lane's:

- The spawn protocol changes. Who reaps a job moves from `job_undertaker` to the shell for these
  jobs. The progenitor learns that a job's file-service window is free from the undertaker's
  "reaped" message (milestone 685 (a job is finished when its memory is back)), which a shell-reaped
  job would never send.
- The pool's memory moves to the shell's budget, so the shell's endowment has to grow by what the
  progenitor's pool shrinks.
- The cooperative tier needs somewhere to live for a program that has no job frame. A `std`
  program's `SIGINT` is the obvious reading, and that is its own question.

## Index row

Job control reaches only supervised jobs, because every other job is built in the progenitor's pool and the shell holds nothing for it. Building every job from a region the shell holds, with its exit on a shell-held endpoint, would give `&`, `wait`, `kill` and `fg` to every program; the spawn-protocol change is calef's call.
