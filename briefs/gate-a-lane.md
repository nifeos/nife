**Provisional name.** Run the four cheap gates on a lane's work before pushing it. You are in the
branch's git worktree, your work is written, and you are ready to prove it locally holds together.
Do the work; do not ask questions.

**Why this brief exists rather than four commands typed from memory.** Every lane brief in this
directory ends with some version of this sequence, and a maintainer retyped its clauses into lane
brief after lane brief on 2026-09-22 and 2026-09-23: the order, the fact that the citation ratchet
reads the committed tip rather than the working tree, the exact shape a gloss has to take, the
memory ceiling that keeps `script/verify` off this machine. Written from memory each time, one of
those clauses gets dropped, and the one that gets dropped is usually the ratchet's commit-first
rule, because it is the one whose failure looks like the tool being broken rather than like a
missed step.

## The four gates, in this order

    script/lint
    script/roadmap --check
    script/fmt
    script/citations --ratchet

Run them in that order because the first three are cheap and independent, and `script/fmt` can fix
what `script/lint` only reports; running it after `lint` means you gate the corrected tree instead
of gating once, formatting, and having to gate again. `script/citations --ratchet` goes last because
it reads **committed** state (see below), so it is the one gate a fix-and-rerun loop can silently
stop trusting.

All four run in seconds on a normal-sized change, need no emulator, and catch most of what would
otherwise be discovered thirty minutes into a CI run (`briefs/gate-in-ci.md`).

## `script/citations --ratchet` reads the committed tip, not your working tree

This is the trap that costs real time, and it costs it in a specific, confusing shape: you fix a
gloss, rerun the ratchet, and read back the **identical failure**, because your fix is sitting
uncommitted and the ratchet diffed the last commit, not the file on disk. It says so when it
happens (`citations: N modified file(s) not checked; --ratchet reads the committed tip. Commit
first.`), but that message is easy to miss under everything else a gate prints.

**Commit before you run `script/citations --ratchet`.** If it fails, fix the file, commit again
(a checkpoint commit is fine here; you squash before the final report anyway), and rerun.

## What the ratchet actually wants: a gloss, on the same line, grounded in the target's own first line

A `milestone N` or `§N` on a line your branch **adds** needs a gloss somewhere in that file, in the
form:

    §92 (a caretaker is supervised by the client it serves)

The gloss sits in parentheses immediately after the number (one optional comma allowed between
them), on the same line or wrapping onto one continuation line, never introduced with a colon or a
dash and never left for a later sentence to explain. It has to be grounded: built substantially
from words in the target record's own first line (its H1) or body, not a paraphrase from memory. If
you are not sure what a number's own first line says, look at
`design/decisions/<NNNN>-*.md` or `design/roadmap/<NNNN>-*.md` (four digits, zero-padded) and quote it, rather than guessing at what
the citation is probably about.

When the ratchet fails, fix the prose, never the gate. A failing citation means the sentence
citing it does not say what the target is; write a gloss that does. It does not mean the number is
wrong or the check is being pedantic.

One number glossed once per file covers every later bare mention of that number in the same file.
You do not need to gloss every occurrence, only the first.

## The follow-through contract: PENDING at dispatch, DONE when executed

Gating a lane's work ends in a push, a ready flip and an auto-merge request, and on 2026-09-29
four of those endings sat unexecuted for one to two hours each because they lived in the session's
memory (see [notes/coes/2026-09-30-lane-follow-through.md](../notes/coes/2026-09-30-lane-follow-through.md)).
The contract, binding from dispatch time:

- Every dispatch writes a PENDING file beside its lane log, named for the lane, holding the
  exact commands this brief's gates precede: the push (with `--force-with-lease` for a rebase),
  `gh pr ready`, `gh pr merge --auto`. At dispatch time, not when the lane reports.
- Executing them flips it to DONE. A PENDING file that exists is work owed. Answer no board
  question while one sits unexecuted, unless you defer it out loud with a reason.
- Tail the log sixty seconds after dispatch. A connection error or a permission rejection
  means the lane never started: redispatch, and record the death in the PENDING file. A dead
  dispatch looks identical to a busy one otherwise.
- Setup ends in dispatch. Creating the worktree, claim commit and draft pull request is one
  act that is not finished until the lane is running. A claim commit with no lane and no DONE
  marker is a stalled claim; raise it, do not pass it.

## Heavy gates go to CI, not here

Do not run `script/verify` or `script/test` (or anything else that starts QEMU) as part of this
brief. `script/verify` reaches about 3.5 GB for a single Kani harness; this machine has 16 GB, so at
most one full `script/verify` runs at a time on it, and never alongside another lane's gate. That
ceiling is why the heavy gates are a separate brief: `briefs/gate-in-ci.md` pushes them to GitHub
Actions, which has memory to spare and where nobody is waiting on the wall-clock.

## Leave no QEMU running

If anything in your session started an emulator (it should not have, under this brief, but check
anyway before you report):

    pgrep -l qemu

Before killing anything found this way, walk its parent chain up, not just check that a process
exists:

    ps -o pid,ppid,command -p <pid>

A QEMU whose parent is a live harness process is another lane's gate in progress, not a leak; killing
it fails that lane's run for a reason it cannot see from inside itself. Kill the tree at its root
(the harness process), not the QEMU child alone, or a loop script simply starts another one. If two
processes are contending for the same image file, `lsof target/nifefs.img` (or the relevant image)
names the actual holder faster than guessing from process names.

## The toolchain link is your worktree's own

`script/test` (which this brief does not run) and some of the cheap gates link the patched std
farm as `nife-dev-<your worktree's directory>-<hash>`, a `rustup` name no other worktree computes
(notes/std.md). It takes nothing from another lane and needs no relink afterward. Do not touch the
bare `nife-dev` link: lanes cut before 2026-10-10 still write it, and only the maintainer removes it.

## Scratch files carry a lane-unique prefix

Every file you write outside the worktree is shared with other lanes. That includes `/tmp` and the
session scratchpad, since every lane a session launches gets the same one. A patch file standing in
for `git stash`, a pull request body and a gate log all live there. Prefix each with your lane or
milestone, as in `586-pr-body.md`, never `pr-body.md`. Two lanes that both write
`/tmp/pr-body.md` push each other's text.
[design/tenets/git-in-a-worktree.md](../design/tenets/git-in-a-worktree.md) has the stash half.

## If you add or change a `script/` command

`script/lint` requires every command under `script/` to have an entry in `notes/scripts.md`; it
greps that file for the command's name and fails the lint gate if it is missing. If your work adds a
new `script/` entry point or renames one, update `notes/scripts.md` in the same commit, before
running `script/lint`, or the first gate in this brief will fail on it.

## EXAMPLES

`script/lint` and `script/roadmap --check` print a report either way; `script/fmt` and a clean
`script/citations --ratchet` print nothing beyond their one summary line when there is nothing to
fix. A run against this tree looked like this (trimmed; `script/lint` alone runs to about 2,000
lines):

    $ script/lint
    ...
    naming: no -d names, no daemons, contract crates spelled one way, branch prefix recognised,
            no #[path] module shared by two binaries, docs lowercase and hyphenated,
            every name carries its provenance, and exactly one block per file
    ...
    shellcheck: 83 sh scripts, no errors or warnings
    sh -n: every #!/bin/sh script parses under this machine's own /bin/sh
    $ echo $?
    0

    $ script/roadmap --check
    roadmap: 565 milestones, 566 files, status vocabulary clean, every '## Index row' parses, ...
    roadmap: 89 pieces of work are outstanding on a PARTIAL milestone (script/roadmap --outstanding)
    ...
    $ echo $?
    0

    $ script/fmt
    $ echo $?
    0

    $ git commit -am "checkpoint"
    $ script/citations --ratchet
    citations: every citation on the 217 lines this branch adds says what it cites
    $ echo $?
    0

A lane that hits the commit-first trap, and the fix:

    $ script/citations --ratchet
    citations: notes/scheduler.md:88: §92's gloss does not match §92
    $ vim notes/scheduler.md   # add the gloss, grounded in §92's own first line
    $ script/citations --ratchet
    citations: 1 modified file(s) not checked; --ratchet reads the committed tip. Commit first.
    citations:   notes/scheduler.md
    $ git commit -am "gloss §92"
    $ script/citations --ratchet
    citations: every citation on the lines this branch adds says what it cites

## BUGS

- This brief does not run `script/test` or `script/verify`, on purpose, and cannot tell you
  whether your change actually builds or boots. Green here is necessary, not sufficient;
  `briefs/gate-in-ci.md` is what proves the rest.
- The QEMU check is best-effort. `pgrep -l qemu` matches both `qemu-system-aarch64` and
  `qemu-system-riscv64`; it will not catch an emulator started under a name this brief did not
  anticipate, and it says nothing about a leaked process on a machine other than the one you are
  running on (`cordoba`, a board rig).
- **Nothing here measures whether following this brief actually stops the clause-dropping it was
  written to fix.** The evidence for writing it down is `notes/effort-levels.md`'s finding that
  correctness comes from prompt wording rather than from the reader's capability; whether this
  particular brief holds up over many lanes, rather than the one measured task, is unmeasured.
