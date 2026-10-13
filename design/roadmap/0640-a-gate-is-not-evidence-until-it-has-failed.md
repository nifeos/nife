---
status: BUILT
built: 2026-10-04
raised: 2026-09-23
promoted_from: a-gate-is-not-evidence-until-it-has-failed
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 640. A gate is not evidence until somebody has watched it fail

*Title and slug are drafts.*
`a-gate-is-not-evidence-until-it-has-failed`: ratified 2026-09-23 (calef, reviewing
`notes/corrections/2026-09-23-the-sweep-that-swept-nothing.md` on pull request #1166). It stays
beside `design/roadmap/0641-a-mechanism-reports-its-denominator.md` rather than folding into it,
though both come out of the same correction and the same fifth why (calef, 2026-10-04 UTC, on the
maintainer's recommendation: both shipped as separate checks). Raised by
`notes/corrections/2026-09-23-the-sweep-that-swept-nothing.md`, which asked whether the failing
workflow was ever tested when it was deployed. It was not.

No hardware, no other milestone, no decision owed.

## The argument, which this tree has already made once

§134 (a harness carries a machine-replayable falsification record, or it is not evidence) rules that
a Kani proof is not evidence until somebody has made it go red on purpose and left the patch that
does it beside the harness. The reason is that a proof which cannot fail proves nothing and is
indistinguishable, from outside, from one that can.

A CI gate is a claim of exactly that kind, and this tree requires nothing of it. A new gate
ships against a tree where its defect is absent, so its first result is green, and green is what it
would also report if it could not fire at all. Nobody can tell which they are looking at, and the
first green is the one everybody reads as confirmation that the thing works.

## The two instances that prompted it

The weekly falsification sweep. `falsifications.yml` landed on `main` on 2026-09-01. Its first
execution of any kind was the cron six days later, which refused to run and reported success. It
replayed zero patches in three scheduled runs over three weeks, and the population it was not
checking grew from 40 records to 74 in that window. A dispatch on the day it landed would have shown
it in 35 seconds.

`coe-architect-label.yml`, merged 2026-09-23 so every correction of error reaches calef by
default. Also shipped unexercised. On the first COE it ever saw it detected the file correctly,
failed to apply the label because `gh pr edit` was called without `--repo` in a job with no
checkout, and reported pass. It was found in hours rather than in three weeks only because a
human was reading the log for another reason.

Both are the same shape one level out from the correction's fifth why: the gate's first green is the
*checked nothing* case.

## The proposal

**A workflow that gates or reports on something is not deployed until it has been observed failing
on purpose, once, and the record of that observation lives with it.**

- **The observation is a run, not an argument.** A link to a run where the job went red for the
  reason it exists to catch, or, where a real failure cannot be staged, a run of the underlying
  script against a deliberately broken tree. §134's own standard is a patch that a machine can
  replay; this is weaker on purpose, because a workflow's inputs are not a source tree and a
  replayable version would cost more than it buys.
- It lives with the workflow, in the file's header beside the reasoning already there, which is
  rung three of `AGENTS.md`'s ladder: a record at the thing itself, read by the next person to touch
  it. The alternative, a registry, is the shape milestone 115 (the names that were ratified, and the
  ones that were refused) exists to refuse.
- It applies to new workflows and to a step whose failure arm is added later, since that arm is
  what carries the claim. It does not apply retroactively to the 14 workflows already here; auditing
  those is the denominator proposal's survey.

## What this is not

**Not a required check.** Nothing gates the gate. This is a deployment habit written where the
deployer reads, and a workflow whose header has no such record is a finding for whoever next opens
it rather than a build failure.

**Not a claim that testing a gate is hard.** Both instances above would have been caught by one
`workflow_dispatch` and 35 seconds of reading. The cost is not the obstacle; nobody asking is.

## What was built

- A gate, rung two, which departs from "Not a required check" above on purpose. The ladder asks for
  the highest rung that fits, and a record nobody is made to write is rung four.
  `helpers/observed_failing.py` (name provisional) runs in `script/lint`. Every workflow carries an
  `# Observed failing:` header line citing a run, or a `by script:` staging, or `never.` with a
  reason. The gate requires the record, not the run, so the deployment habit stays a habit for old
  workflows and becomes a requirement for new ones.
- `never` is accepted only when git says `main` already had the file at `4c9cae0a9`, the commit this
  lane was cut from. The test is ancestry, so an old branch cannot slip a new workflow through.
- The backfill, read from each workflow's failure history and logs. 16 of 21 cite a run that went
  red, or fired, for their own reason; `--verify-runs` checked each id against the API. Five say
  `never`, and two of those have red runs that were defects rather than findings.
- `notes/observed-failing.md` (name provisional): the rule, how to add a workflow, EXAMPLES, BUGS.

## What proves it

- CI run 37230967057 (2026-10-04): a probe workflow, new since the cutoff and saying `never`, turned
  the `clippy` job red at this check with its own refusal. The commit after it removes the probe.
- The run before it, 37229594948, was red for a different check and is recorded as not counting.
- Eleven fixtures run before the tree is judged, so a green result is not a checker matching nothing.

## BUGS

In `notes/observed-failing.md`. The record is per file rather than per job or arm, a cited run is
checked only by hand, and whether a red was the right red is prose, not a check.

## Architectural parity

Not applicable: this is CI and lint, with no kernel code on any architecture.

## Follow-on

- **Recorded.** The five `never` workflows are unstaged; each one's own header record says what
  would stage it, which is where the next person to touch it reads.
- **Recorded.** Nothing runs `--verify-runs` on a schedule, so a mistyped run id passes lint;
  `notes/observed-failing.md`'s BUGS.

## Index row

A gate that has never been watched failing is not evidence of anything, and the sweep that swept nothing was never tested when it was deployed. Proposed: require a recorded failure of each new gate, as DECISIONS §134 already does for Kani harnesses.
