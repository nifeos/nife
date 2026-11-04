---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 339. Count how many of the tree's programs can fault under a shell

Premise re-checked 2026-09-19 and it holds. No note in `notes/` carries the survey or a count, and nothing in the tree lists which
programs have a reachable fault. The program count has moved since the proposal was written:
milestone 291 split `fixtures/src/hello.rs`'s thirty-one roles into programs on 2026-09-14, so
whoever takes this re-derives the denominator rather than quoting 68.

Nothing is owed and nothing is missing. It is a survey of code already in the tree,
and a lane could start it today.

**In brief.** Milestone 235 fixed a shell that hangs forever when a spawned command traps. Nobody
knows how many programs could trigger it. The tree ships 68 programs, and every one that can
fault reached that path. The work is to go through them and produce a number. Which programs have a
reachable fault (an unwrapped index, a `panic!` on bad input, a syscall that returns an error the
program does not handle), and which cannot fault by construction.

## Why this matters

Right now the value of milestone 235's fix is unmeasured. It is either a repair to a defect two
programs could hit or a repair to one that forty could, and those are different claims about how bad
the prompt's behavior was. The block that fixed it cannot say which, and neither can anybody else.

The survey also produces something the fix did not: a list. A program that can fault under a shell
is a program whose failure mode a user will eventually meet. Knowing which ones those are is the
input to deciding whether any of them should not be able to fault at all.

The honest caveat is that this is a measurement rather than a repair. It changes no behavior, and
its whole output is a number and a table in a note. That is a real cost to weigh against promoting
it, and it is why the bar is a number a claim rests on rather than a bug a user hits.

## Index row

Milestone 235 fixed a shell that hangs forever when a spawned command traps, and nobody knows how
many programs could trigger it, so the value of that fix is unmeasured. Two programs could hit the
defect, or forty. The work is to go through the tree's programs and produce a number: which have a
reachable fault (an unwrapped index, a `panic!` on bad input, a syscall error the program does not
handle), and which cannot fault by construction. The survey also yields a list of programs whose
failure mode a user will eventually meet. It changes no behavior, and its output is a number and a
table in a note.
