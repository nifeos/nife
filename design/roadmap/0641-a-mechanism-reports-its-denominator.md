---
status: BUILT
raised: 2026-09-23
built: 2026-10-04
promoted_from: a-mechanism-reports-its-denominator
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 641. A mechanism that reports clean says over how many units, and zero is loud

*Title and slug are drafts.*
`a-mechanism-reports-its-denominator`: ratified 2026-09-23 (calef, reviewing
`notes/corrections/2026-09-23-the-sweep-that-swept-nothing.md` on pull request #1166). Raised by
`notes/corrections/2026-09-23-the-sweep-that-swept-nothing.md`, whose fifth why reaches a habit
rather than a bug and whose action items must resolve to something under §210 (a correction of
error, and its action items are decisions, proposals or milestones).

No hardware, no other milestone, no decision owed. Built 2026-10-04 (UTC) by `lane/641-mechanism-denominator`; the per-site labels and the ratio survey are in `notes/denominators.md`.

## The claim

**From outside, a check that examined nothing looks exactly like a check that found nothing.** That
is not an observation about one defect; it is the default state of every gate on the day it lands,
before its subject exists, and nothing in this tree's review asks about it. The reviewer's question
is "does this catch the defect". The question that goes unasked is "what does this do when its input
set is empty".

Six instances turned up in one day, 2026-09-23:

- **Milestone 401 (a gate that selects the set it judges can pass by checking nothing)** swept
  `script/` for the class and guarded eight selectors, enumerated in `notes/empty-selectors.md`.
- `script/ci-build`'s tier selector runs zero checks and prints `ci-build: all pass` with exit 0
  if the tier column is renamed. Reproduced independently twice, by 401's lane and by pull request
  #1134.
- The draft gate. `ci.yml`'s own comment: a skipped job still posts a conclusion and still
  satisfies a required check, so skipping is never the safe default.
- The weekly falsification sweep, which replayed zero patches in all three of its scheduled runs
  and reported success each time, because the transcript it was writing dirtied the tree its own
  guard protects and `continue-on-error` discarded the refusal.

- The labeler for corrections of error, `coe-architect-label.yml`, merged the same day. It
  detected a new COE record correctly and then failed to apply the label, because `gh pr edit` was
  called without `--repo` in a job with no checkout. Its deliberate never-fail arm reported the
  failure to the log and the job to GitHub as a pass. Fixed on the branch that found it, which was
  the branch writing the correction of error about the other four.

- And the falsification ratio, which is not a gate at all but the same defect in a measurement.
  It counts harnesses, so code with no harness is absent from its denominator and a crate can read
  100% falsified while most of it is unproved. Milestone 524 (the three x86_64 boot gates) added 338
  lines to `crates/machine_discovery/src/x86_64.rs` on 2026-09-21 with no harness of their own, and
  the number did not move. The milestone 319 (the crate that parses firmware
  had no proofs, and three of its first ones were false) lane found and reported it on pull request
  #1155, which closed that instance and not the class.

Milestone 401 fixed the `script/` half. The workflow half is untouched, and it is the half where
the swallowing is explicit and deliberate rather than accidental.

## What this would look for, measured rather than asserted

Counted on `main` at 2026-09-23 across `.github/workflows/`, 14 files:

| Construct | Count | Files |
|---|---|---|
| `continue-on-error: true` | 5 | `falsifications.yml`, `metrics.yml` (2), `toolchain-bump.yml` (2) |
| `|| true` | 19 | 8 workflows, most of them in `ci.yml` (6) and `toolchain-bump.yml` (4) |
| `if: always()` | 6 | `mutation.yml` (5), `falsifications.yml` |
| `tee` into the checkout | 1 | `falsifications.yml`, and it is the one this came from |

The `tee` row is worth keeping in the table precisely because it is now 1 and was the whole defect.
The other three constructs are the population, and none of them is wrong in itself.

## The distinction to make, per site

Is this suppressing a verdict or an outcome? They wear the same clothes and only one is safe.

- A verdict. "The sweep ran, and something in the result wants a human." §134 (a harness carries
  a machine-replayable falsification record, or it is not evidence) rules that a survivor is a
  worklist entry and not a defect in the preceding commit, so a survivor must not go red.
  Suppressing that is correct and stays.
- An outcome. "The thing did not run." Suppressing that is never correct, and today every
  `continue-on-error` site suppresses both, because a step's exit status is one channel carrying two
  claims.

The separation does not need new machinery. A report that states its denominator is enough. A
sweep that says it replayed N records cannot claim N when it replayed none. A job that asserts
its own N is non-zero fails loudly on the one case `continue-on-error` must not hide, while staying
silent on the verdict it was written to let through.

## What building it looks like

1. Walk the 30 sites above and label each verdict or outcome, in a note the way
   `notes/empty-selectors.md` did for `script/`. The label is the deliverable; most sites will need
   no change.
2. Give each reporting job a denominator. `script/falsifications --sweep`, `script/mutation`,
   `script/audits --due` and `script/stranger-test --due` all already know how many units they
   examined. Have them print it in a shape a workflow can read back, and have the workflow assert
   it is not zero. This is rung two of `AGENTS.md`'s ladder, a gate that fails loudly, and it does
   not touch the verdict.
3. Consider rung one where it is cheap. A script that exits non-zero when its own unit count is
   zero needs no workflow cooperation at all, and makes the wrong state unrepresentable from the
   workflow's side. Where it is not cheap, say so and take rung two.

## And the same defect in the tree's own metrics

**A reported percentage must say what it is a percentage of.** The four workflow constructs above
hide a zero in an exit status. A ratio hides one in a denominator, and the second is worse because
the number is published, quoted and used to answer `design/fatal-risks/README.md`'s risk 2 (the proofs
prove trivia and the real bugs live where Kani cannot reach). `script/falsifications` reports 63 of
180 harnesses, `script/metrics` carries several ratios of the same family, and none of them states
the population it drew from or what it excluded.

The survey is the deliverable, not the fix. Walk the ratios this tree publishes, and for each
one write down what its denominator counts and what falls outside it, beside the number. Whether a
better denominator exists for the falsification ratio (lines, functions, public surface) is a real
question with real costs and this proposal deliberately does not answer it; naming which numbers
have the problem is what makes answering it possible.

## What this deliberately is not

**Not a ban on `continue-on-error`.** The reason it is in `falsifications.yml` is good and is
written in the file. A proposal that removed it would turn every survivor into a red check somebody
has to silence, which is the outcome §134 argued against.

**Not a sweep of every script.** Milestone 401 did `script/`. This is the workflows, and the two
together are the tree's mechanisms. Extending it further would be speculative.

**Not a claim that any other site is currently broken.** Only the falsification sweep is known to
have been. The other 29 are a population to label, and labeling them is most of the value, because
the label is what the next person to add a `|| true` will read.

## What was built

- A gate, rung two. `helpers/workflow_swallows.py` (name provisional), run by `script/lint` as check
  12b. Every `continue-on-error` carries a `# outcome:` label on its line: `re-raised`, `reported`, or
  `exception, <reason>`. The first two are checked against the job, and a test on
  `steps.<id>.conclusion` is refused because it can never fire. A pipeline into `tee` must have
  pipefail. A new workflow that hides a script's exit fails lint.
- The 13 sites labeled. The five in `metrics.yml` were outcome swallows with nothing downstream, so
  a coverage step failing every Monday would have left a carried cell standing. They now share a
  last step that fails the job after the record is written.
- Rung one where it was cheap. `script/falsifications --sweep` exits 4 on an empty selection, where
  it printed `nothing to sweep` and exited 0; `--affected-since` keeps zero as clean. `script/audits`
  fails an empty cadence table.
- Two outcome swallows found by labeling the `|| true` population. `architect-label.yml` read a
  crash of its rules helper (Python's exit 1) as "no rule fired"; the helper now crashes with 3 and
  an unexamined diff is labeled. `ci.yml`'s nine documentation-only predicates read zero changed
  files as documentation only; each now needs a non-empty set.
- The falsification ratio, as the block directed: a survey of the eleven published ratios and what
  each denominator excludes, in `notes/denominators.md`. `script/falsifications` and the harnesses
  chart now state the denominator beside the number.

## What proves it

- `helpers/workflow_swallows.py --selftest`: 17 fixtures, each failing case first-class, plus an
  empty workflow directory that must fail.
- Replayed against `24a1e0a7b`, the sweep as it first landed, the gate fires on both of its
  constructs: the unlabeled `continue-on-error` and `| tee sweep.txt` without pipefail.
- `script/falsifications --sweep no_such_package` exits 0 at the base commit and 4 on this branch.
- `helpers/architect-label-rules.py --base-rev deadbeef` exits 1 at the base commit, the same as
  "nothing fired", and 3 on this branch.

## BUGS

`notes/denominators.md` has them. The two that matter: a label can be wrong, since the gate makes
the decision written rather than correct; and `|| true` is labeled by family there, not gated.

## Architectural parity

Not applicable. This is CI and record tooling; no kernel capability changed.

## Follow-on

- **Recorded.** `|| true` and selectors inside `run:` blocks are ungated; `notes/denominators.md`'s
  BUGS.
- **Recorded.** Whether the falsification ratio wants a better denominator (lines, functions,
  public surface) is open, as the block intended; the survey in `notes/denominators.md` is what
  makes it answerable.
- **Recorded.** The exceptions stay quiet: the watcher reports warn and the nightly installs say
  nothing; `notes/denominators.md`'s BUGS.

## Index row

From outside, a check that examined nothing looks like a check that found nothing. Every
`continue-on-error` in a workflow now says where its failure goes, checked by `script/lint`. A
pipeline into `tee` has pipefail. The falsification sweep and the audit cadence fail on an empty
selection, and every published ratio states what it is of (`notes/denominators.md`).
