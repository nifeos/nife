# Briefs: self-contained instructions for recurring work

A brief is self-contained instructions for a recurring task, with the judgment already made and the
known failure modes named. Nothing about who reads it. Hand one to a lane the ordinary way:

    helpers/open-lane.sh <worktree> briefs/<brief>.md

or read it yourself and follow it by hand. Both are the same document, because self-containment is
what a brief is for, not a concession to a weaker reader.

## A brief is for every reader, whatever its model

A brief is not reserved for a cheaper model or for work a gate can check. calef, 2026-09-23:
*"Why are the briefs reserved for just the low powered agents?"*

The evidence is `notes/effort-levels.md`: five
`--effort` levels, twenty runs, one real query against this repository. Correctness was explained
100% by the wording of the prompt and 0% by the effort level. An ambiguous wording produced the
identical wrong answer at every level up to and including `max`; a disambiguated wording produced
the correct answer at every level down to and including `low`. A brief is exactly that
disambiguation, done once and kept. It is worth as much to the most capable model reading it as to
the cheapest, because the thing it fixes is not a reasoning gap, it is an instruction gap.

The other half: the four gates, the ratchet reading the committed tip rather than the working tree,
the gloss-on-the-same-line rule and the `script/verify` memory ceiling. They get retyped into lane
brief after lane brief unless they are written down. That holds for lanes on the most capable model
as much as
any. `briefs/gate-a-lane.md` holds them for that reason, not because of anything about model tier.

## Why these exist as files rather than as something typed each time

On 2026-09-22 a delegated rebase cost $0.055, was correct first time, needed no redo, and
replaced about thirteen of a maintainer's tool calls with four. It worked for one reason: the
brief encoded judgement that had already been made. Take `main`'s baselines, never hand-merge
them, accept the retired index's deletion, and stop rather than guess at anything else. Following
that is lookup, not judgement, and a brief is what turns judgement into something lookup can execute,
whoever or whatever is doing the looking up.

Written from memory each time, a brief loses a clause a month, and the clause it loses is the one
that stops a wrong conflict resolution shipping as housekeeping.

## The gates go to CI, not to this laptop

`briefs/gate-in-ci.md` is the one every other brief defers to, and it is here for a reason that
is about hardware rather than about models. Gating locally means three architectures of QEMU and
`script/verify` at about 3.5 GB a harness, on an 8-core M3 with 16 GB, and that caps the lane count
at three or four.

Lanes are asynchronous: nobody sits waiting on one. So a CI round trip of 23 to 29 minutes costs
a resource this project has in abundance and saves the one that actually binds. That trade is only
available to lanes, which is why it is a brief rather than a rule for everything.

The cheap gates stay local: `script/lint`, `script/roadmap --check`, `script/fmt` and
`script/citations --ratchet` run in seconds, need no emulator, and catch most failures before a
half-hour run is spent on them. `briefs/gate-a-lane.md` has the order and the traps for those four.

## What belongs here

Work that recurs, whose failure modes are already known, and whose correctness a gate can check.
Rebases, triage, sweeps, the boilerplate every lane's gating step repeats. Not a one-off design
question, not prose a reader will trust on its own authority, not anything touching the syscall
surface or a wire format. Those stay judgement calls made fresh, by whoever is making them, because
no brief can encode a decision that has not been made yet.

Whether the *executor* is a rented open-weight model or the session's own frontier model is a
separate question, answered by DECISIONS §202 (mechanical work goes to a cheaper model) and the
decision recording the measurements behind it. It is a question about routing capacity, not
about what a brief is for. A brief's job is to be correct instructions; who is cheapest to run them
on today is a cost problem that can change weekly without this directory changing at all.

## The briefs

| file | what it's for |
|---|---|
| `bench-session.md` | Run a bench session on radon, xenon or argon from a lane: what needs a go, what needs calef's hands, and the ranked list of sessions ready to run. Provisional name. |
| `gate-a-lane.md` | The four cheap gates every lane runs before pushing: order, traps, exit criteria. Provisional name. |
| `gate-in-ci.md` | Push the heavy gates (QEMU, `script/verify`) to GitHub Actions instead of this laptop. |
| `rebase-onto-main.md` | Rebase a branch onto `origin/main`, with the three known conflict classes named. |
| `review-a-paydown.md` | Review one paydown pull request from its diff alone: the five recurring error classes, in order of harm, and the verdict line. Provisional name. |
| `run-a-milestone-wide.md` | Run one large milestone across many concurrent lanes: what binds (the maintainer's loop, not the host), the partition into disjoint scope files, and the remedies in order of payoff. Provisional name. |
| `session-start.md` | What a maintainer session does first: are the watchers alive, read what they found, and the tokens rules (lane count, model, the watch list, the CI-poll hook). |
| `survey-the-queue.md` | Read-only report on the state of the merge queue. |
| `triage-a-failing-check.md` | Read-only diagnosis of why a CI check failed, and this repository's specific log traps. |

## The rule every brief here follows

Say what to do about the failures that are known, and say "stop" about everything else. A brief
that tries to anticipate every case teaches its reader to improvise; a brief that names three
resolutions and demands an abort on the fourth gets a clean worktree back when it meets something
new. The rebase brief is the worked example.

## BUGS

- `--bare` skips `AGENTS.md`. Under the old framing this was a limitation to work around, the
  cost of handing work to a rented model. It is now the reason this directory exists at all.
  `--bare` is just the sharpest case of a fact true of every reader, including a maintainer three
  hours into a session who has stopped rereading `AGENTS.md` from the top. A brief carries what it
  needs: that citations want glosses grounded in the target's H1, that a lane never edits another
  milestone's block, that invented names are provisional. Nothing guarantees any reader has
  that context loaded, human, cheap model, or frontier model alike.
- Nothing here measures whether CI gating actually raised the lane ceiling. The argument is
  sound and the memory arithmetic is measured, but the number that would settle it is concurrent
  lanes sustained without an out-of-memory kill, and that has not been run yet.
- Nothing here measures the redo rate, which is the number that decides whether any of this
  saves anything. A delegated task that passes its gates and must be redone by hand costs more than
  never having delegated it. One sample so far, and it needed no redo.
- `notes/effort-levels.md` measured one task shape, a bounded read-only grep-and-tabulate query,
  and says nothing yet about whether a brief's payoff holds for a multi-step edit or a genuinely
  ambiguous judgement call rather than an ambiguous instruction. Cited above as the best evidence
  available, not as a settled generalisation.
