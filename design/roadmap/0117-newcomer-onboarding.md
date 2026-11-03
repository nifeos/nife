---
status: BUILT
raised: 2026-08-05
built: 2026-09-19
---
# 117. The stranger test: could someone build this and understand it without asking

Run 6 went on 2026-09-19 and its findings have homes;
`script/apropos` is placed where a newcomer meets it; `script/names` counts its worklist one way.
See [Why this is `BUILT`, and the one way it is not](#why-this-is-built-and-the-one-way-it-is-not)
below for what that does and does not settle, and in particular that the cadence has never
actually fired.

Minted 2026-08-05 by calef, to put the third principle to a test rather than leave it as an
aspiration. The rubric was written 2026-08-14 ([notes/stranger-test.md](../../notes/stranger-test.md))
and run 1 went the same day. Run 2 went 2026-08-16 (pull request #219), and runs 3, 4 and 5 went
2026-08-18. Run 5 was the first conducted through `script/stranger-test` rather than rebuilt by
hand. Eleven defects were fixed between the first two runs. The two documents the block predicted
both exist: `CONTRIBUTING.md` at the repository root, and a reading order at the top of
`README.md`. Both are still provisional, because a reading order is a claim about what matters and
those are an architect's.

The question, which is the principle's own wording: *could a competent stranger, with only this
repository, reach a passing build and a correct mental model without opening a chat window?* Where
the answer is no, that is a bug in the tree and not in the stranger.

## Appendices

This page is complete on its own. The record of each run as the block told it at the time moved to
[0117-newcomer-onboarding/](0117-newcomer-onboarding/README.md) on 2026-10-12 (UTC):

- [first-plan.md](0117-newcomer-onboarding/first-plan.md): the build and mental-model halves, the
  candidate rubric, and the worklist the block predicted before any run.
- [run-3.md](0117-newcomer-onboarding/run-3.md): run 3's isolation fix and findings, its five
  handoffs, the lane that took the cheap ones, and how this block's status went stale twice.
- [run-4.md](0117-newcomer-onboarding/run-4.md): run 4's eight of eight, the leak that moved into
  the tree, and why the worklist was never the blocker.
- [harness-and-cadence.md](0117-newcomer-onboarding/harness-and-cadence.md): what
  `script/stranger-test` holds and does not, and how the monthly tripwire reads the record.
- [run-5.md](0117-newcomer-onboarding/run-5.md): the machine-wide contamination, `script/apropos`
  not firing, the seven handoffs, and the two lanes that answered them.
- [run-6.md](0117-newcomer-onboarding/run-6.md): run 6's headline results and the three items it
  closed.

## Why this needs a milestone rather than good intentions

Every principle in CLAUDE.md names a mechanism that holds it when nobody is watching, and this one's
mechanism was missing. "Write good docs" is rung four of the ladder: a note, relying on somebody
remembering. This milestone is the gate.

It also cannot be self-assessed, and that is the whole difficulty. calef cannot take this test; he
wrote the system. Nor can any agent that has worked in this tree, which by 2026-08-05 is most of
them. An agent that spent a night merging pull requests here knows why `nife-dev` is a symlink,
what a lane is, and that `script/lint` fails on a branch prefix. Knowing the answer disqualifies
you from being the instrument.

## The instrument: a stranger with no context

Spawn an agent that has never seen this tree, hand it the repository and nothing else, and give it
a task. No brief explaining the conventions, no pointer to the right note, no answer to any
question it asks. Its confusion is the measurement.

Three things make this a real test rather than theater:

- It must be a fresh context, not a summarized one. A handoff that says "read CLAUDE.md first"
  has already given away the finding that a newcomer would not know to.
- Every question it asks is a defect, recorded verbatim. The questions are the deliverable, more
  than the score is.
- Every wrong answer it is confident about is a worse defect, because a document that misleads
  costs more than one that is silent. Milestone 97 (citations that name what they cite) found six
  citations pointing at the wrong record; the same failure in prose is what this looks for.

The test has two halves. The build is mechanical: from a clean clone, does `script/setup` then
`script/test` reach green, following only what the repository says? The mental model is not, so
its rubric was written before the first run, where nobody can grade generously afterwards. It is
graded against what the tree says, never against what a maintainer knows. The output is a worklist
of defects, not new prose. Then fix what the run finds and run it again, which is the difference
between an audit and a milestone. [first-plan.md](0117-newcomer-onboarding/first-plan.md) has the
candidate rubric and the predicted worklist.

## What was built

- The harness, `script/stranger-test`, landed 2026-08-18. It clones the tree inside the stranger's
  working directory, withholds the answer key (notes/stranger-test.md), puts logs in a sibling
  directory, shadows `pkill` and `killall` to the clone's QEMU, and probes the isolation before it
  spends anything. The stranger is told it is being measured. `--smoke` exercises every stage
  cheaply. The name is provisional.
- The cadence: monthly, decided by calef on 2026-08-18 and made mechanical the same day.
  `script/stranger-test --due` exits 1 when a run is owed. `.github/workflows/stranger-cadence.yml`
  asks it at 08:00 UTC each Monday, and `--check` in `script/lint` keeps the record well formed.
  Both numbers come from notes/stranger-test.md alone: the cadence sentence and the newest run
  heading. It is notification, never execution, because a run spends real budget and ends in a
  debrief a person scores. calef ruled on 2026-11-03 that the cadence pauses until the kernel
  leaves nife: one line in the note makes `--due` report "suspended" and exit 0, and the milestone
  that moves the kernel deletes it.
- `CONTRIBUTING.md` and the `README.md` reading order, both landed 2026-08-18, both provisional.
- `script/apropos`, named in `README.md` and `CONTRIBUTING.md` since 2026-09-19.
- Fixes for every run's findings, each homed where a reader meets it: the host-load sampler beside
  timing failures, `notes/adding-a-program.md` corrected by walking it, the `std` farm assertion,
  and `script/names` printing one worklist count.

[harness-and-cadence.md](0117-newcomer-onboarding/harness-and-cadence.md) has the harness's design,
the three defects found by testing it, and how the tripwire was watched firing.

## The six runs

| run | date | what it showed |
|---|---|---|
| 1 | 2026-08-14 | the rubric's first use; eleven defects fixed before run 2 |
| 2 | 2026-08-16 | unscorable on five of eight rows: its strangers inherited `AGENTS.md` at turn zero |
| 3 | 2026-08-18 | isolation fixed in the tree and leaked by the harness's own log names |
| 4 | 2026-08-18 | eight of eight; the leak moved into the tree's `BUGS` tables |
| 5 | 2026-08-18 | first through the harness; the machine, not the tree, contaminated the build |
| 6 | 2026-09-19 | milestone 150 (eight hand-maintained lists) worked; a wrong diagnosis cost the most |

What the runs taught, in order:

- Isolation is the hard part. Each of the first four runs leaked differently. Run 3 fixed the
  ancestor-directory leak and was told its run number by its own log file names
  ([run-3.md](0117-newcomer-onboarding/run-3.md)). Run 4's stranger learned it was being tested
  from `notes/adding-a-program.md`'s `BUGS` rows, which name earlier strangers' programs because
  the conventions require it ([run-4.md](0117-newcomer-onboarding/run-4.md)). So from run 5 on, the
  stranger is told it is being measured, and only the answer key is withheld.
- The worklist was never the blocker. Every run produced a fixable list and most were fixed, but
  "run it again" had no mechanism until the harness and the cadence existed.
- Adding a program needed eight hand-maintained lists, and three successive strangers nominated
  fixing it. That became milestone 150, and run 6's stranger used it without trouble.
- The harness isolates a tree, not a machine. Run 5's first `script/test` compiled `std` from
  another worktree through the account-wide `nife-dev` link, and the gate then named files in a
  checkout the stranger had never seen ([run-5.md](0117-newcomer-onboarding/run-5.md)).
- Placement matters more than existence. `script/apropos` was built to close the
  `design/decisions/` gap, and five runs never ran it. Run 5 never opened `AGENTS.md` at all,
  apparently because `CONTRIBUTING.md` summarized it.
- Run 4's stranger named the habit: the project answers structural problems with another document,
  which its own ladder calls the worst available move.

The block's own record kept proving its subject. Its status went stale twice in the same way
([run-3.md](0117-newcomer-onboarding/run-3.md)), and duplicated counts it fixed rotted within days.

## Why this is `BUILT`, and the one way it is not

The block's own test for `BUILT` was a run 6 conducted under the cadence rather than because
somebody thought of it. Run 6 happened, two days after `--due` began exiting 1, and its record is in
[notes/stranger-test.md](../../notes/stranger-test.md). It did not happen because the cadence fired.
The weekly workflow could not go red before 2026-09-21, and a maintainer's brief is what started it.

That is recorded rather than glossed, and its home is a proposal
(`design/roadmap/0495-a-due-cadence-reaches-nobody.md`) rather than this block. The gap is shared
with the audit cadence, which had been red five Mondays running with nobody acting. It is a fault in
how any scheduled "due" reaches a person, not in the stranger test. What this milestone owns exists
and has been exercised: the instrument, the rubric, the harness, the cadence sentence, the tripwire
and six runs through it. What it does not own is the watcher.

Run 6's details and the three items it closed are in
[run-6.md](0117-newcomer-onboarding/run-6.md). M2 was absent for the fourth run in six and
`notes/net.md` still went unopened, so run 7 is the measurement of the `script/apropos` placement.

## Limitations

- An agent is not a person, and its failure modes are not identical. It will not get bored, will
  not give up out of frustration, and will read further than a human would before asking. So the
  result is a lower bound on the friction a human would meet, and the milestone should say so
  wherever it reports a number.
- The tripwire believes the headings. A run never written up leaves `--due` red, which is the
  safe direction. A heading nobody earned turns it green, the same hole `script/audits` names.
- The harness cannot make the operator un-read `AGENTS.md`, cannot stop the tree leaking that a
  test exists, and leaves the withheld note one `git show` away. That last one is measured: the
  summary reports whether the transcript mentions it.
- The `$10.56` run 5 cost is the stranger process alone. The lane that pre-registers, watches,
  debriefs and scores is the larger half.

## Scope note

Not a documentation rewrite. The tree's documentation is unusually good and this milestone must not
turn into polishing it. The deliverable is the set of places a stranger *actually* fell down, which
is a much smaller and much more specific list than a review would produce.

Not a beginner's tutorial either. The reader is a competent systems engineer who has never seen
this project. The test is not whether they understand a page table; it is whether this repository
tells them what *this* system does with one.

## Follow-on

- **Done.** Run 6 went on 2026-09-19 and is recorded in notes/stranger-test.md with every finding
  homed. `script/apropos` is named in `README.md` and `CONTRIBUTING.md`, with examples, and no
  longer fails every search on six long filenames. `script/names` prints one worklist count
  everywhere (69 of 222 at the time), provisional included, and design/naming.md's `BUGS` entry is
  removed. See [run-6.md](0117-newcomer-onboarding/run-6.md).
- **Milestone 495.** A due cadence reaches nobody, and the watcher cannot tell due from dead:
  milestone 495 (a cadence that says "due" reaches nobody), `design/roadmap/0495-a-due-cadence-reaches-nobody.md`.
- **Milestone 516.** `uefi-test` can exit 1 after its own suite passed:
  milestone 516 (uefi-test can exit 1 after its own), `design/roadmap/0516-uefi-test-exits-one-after-a-passing-suite.md`.
- **Milestone 150.** The eight hand-maintained lists, nominated by three successive strangers, have
  a tracked home in milestone 150 (adding a program should not need eight hand-maintained lists),
  and run 6 found it working.
- **Recorded.** The CLI's default model refuses the task text, a stranger that waits on background
  work ends its own run, a stranger can write to the machine, and `script/lint` announces the
  withholding. All are in `script/stranger-test`'s `BUGS`, each naming what would change it and
  whose call that is.
- **Recorded.** Removal still has no page, and the argument-plus-input fork stays undecided. Both
  are in `notes/adding-a-program.md`'s `BUGS`; the fork is also in the planner's own comment and a
  passing host test in `crates/grant_plan`. Whether such a program is wanted at all is an
  architect's.
- **Recorded.** Two diagnostics are built and still unexercised in anger. The host-load sampler in
  `xtask/src/main.rs` runs on both kernel legs, and `notes/load-sensitive-assertions.md` carries its
  `BUGS`, but no recorded run saw it fire on a red leg. The farm-contamination assertion has not
  met a real contaminated farm; host coverage and a clean `script/test` verify it.
- **Recorded.** `CONTRIBUTING.md` and `README.md`'s reading order are both still provisional,
  because a reading order is a claim about what matters and those are an architect's.
- **Done.** Run 4's placement finding is fixed at the table: the `hello` entry in
  `xtask/src/main.rs` says riscv's "init" is the `builder` demo. `README.md`'s counts carry
  count-at-least markers that `script/lint` checks. And two of run 5's handoffs that rotted again
  after 2026-08-22 (`script/gates` stages, the toolchain pin) were fixed by milestone 252 (a
  `PARTIAL` block nobody re-reads), which deleted the duplicated fact so neither can rot a third
  time.

## Index row

The third principle says a newcomer must succeed without asking anyone, and nothing held it. An
agent with no context is handed the repository and nothing else; its questions are the deliverable
and its confident wrong answers are the worse defect. Six runs through 2026-09-19;
`script/stranger-test` is the harness and a weekly workflow says when the next one is owed, monthly
per calef (2026-08-18), paused until the kernel leaves nife (2026-11-03). Run 6 found milestone
150's one-place program declaration working on first contact and a stranger that provisioned the
machine chasing a wrong diagnosis the tree invited; `script/apropos` is now placed where newcomers start. The cadence has never fired on its own: how a
"due" reaches a person is a proposal shared with the audit cadence
