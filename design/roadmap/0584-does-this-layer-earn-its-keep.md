---
status: NOT-STARTED
raised: 2026-09-23
promoted_from: does-this-layer-earn-its-keep
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 584. A mechanism owes evidence that it caught something a person would have missed

*(Number minted at promotion.)* Promoted from the proposal `does-this-layer-earn-its-keep`, filed
2026-09-23, on calef's instruction of 2026-09-24 to promote the proposals. The text below is the
proposal's own, unedited except for this paragraph, the `## Index row` section, the correction about
pull request #1170, and the nine layer headings, which read `### 1. The watchers` and so looked like
milestone headings to `script/roadmap --check`; they now read `### Layer 1: the watchers`: the
argument is its author's and promotion is not the moment to improve it. On 2026-10-12 (UTC) the
layers and the notes on the measure table moved whole into
[layers.md](0584-does-this-layer-earn-its-keep/layers.md) under §212 (a prose budget), and a
paragraph and a sentence that restated others were cut. As filed: raised by calef the same day: *"I
also want to have metrics on our actions so that we can assess the effectiveness of our various
layers. They need to demonstrate their value."* Written by a lane after four mechanisms were found
on one day reporting success while checking nothing.

No hardware, no other milestone. One decision was put to calef and he deferred
it on 2026-09-24; the deferral, its reason and its trigger are the last section of this file, and
nothing in the first increment waits on it.

*The slug `does-this-layer-earn-its-keep` is provisional, and so is the instrument name proposed
below. Naming is an architect's; a lane ships a provisional name and says so.*

## This is the same argument, one rung up

It is a rule this tree has already made twice, applied to the next category up.

- §134 (a harness carries a machine-replayable falsification record) rules that a Kani proof is
  not evidence until somebody has made it go red on purpose and left the patch beside the harness.
  Its reason: a proof that cannot fail proves nothing and is indistinguishable, from outside, from
  one that can.
- `a-gate-is-not-evidence-until-it-has-failed` (ratified by calef 2026-09-23, in this directory)
  moves that from a harness to a CI workflow. A new gate ships against a tree where its defect is
  absent, so its first result is green, and green is what it would report if it could not fire at
  all.
- This is the third instance. A harness must show it *can* fail. A gate must show it *has*
  failed, once, on purpose. A mechanism must show that over its lifetime it has caught
  something a person would have missed, and it must be able to say so in a number rather than in
  its own header.

The escalation is in what the evidence is about. §134's record is about capability. The gate
proposal's record is about deployment. This one is about yield: not "could it fire" but "did it,
how often, and what did it catch."

## The four findings that force it, all 2026-09-23

Each is a mechanism that reported success while doing nothing, and no two are the same defect.

- `falsifications.yml` replayed nothing for three weekly runs and reported success each time.
  Twenty-two days. *(Corrected 2026-09-24 from "twenty-three": the window starts at the
  workflow's deployment, which is pull request #603's merge to `main` on 2026-09-01, not the
  commit's author time on 2026-08-31. Pull request #1166's correction-of-error record,
  `notes/corrections/2026-09-23-the-sweep-that-swept-nothing.md`, has the derivation.)* The population it was not checking grew from 40 records to 74 in that window.
  Nobody could tell *swept and found nothing* from *swept nothing*, because the workflow emitted the
  same result for both.
- `helpers/trunk-health.sh` reads CI's conclusion to say when `main` goes red. `ci.yml` skipped
  `build + test` for documentation-only commits, so `main` was red for hours with every signal
  green. The watcher's own header claimed *"The signal was never missing."* Pull request #1170
  corrected that claim and merged on 2026-09-24. *(Corrected 2026-10-12 from "claims" and "is
  correcting": the proposal was filed before #1170 merged.)*
- `helpers/merge-drain.sh` logs a snapshot and never an event. A pass writes
  `10 armed, 7 stalled, of 17 unheld`. There are 3,355 passes on record in
  `~/Library/Logs/nife/merge-drain.log` and not one of them answers "how often did the drain act."
  The maintainer summed the snapshot across passes, got 4,967, and caught the meaninglessness only
  afterwards.
- The falsification worklist counts harnesses, so 342 lines of code carrying no harness at all
  left the ratio clean. The denominator was the population of the thing being measured rather than
  the population it was supposed to cover.

## The distinction that makes any of this possible: events, not snapshots

`notes/project-metrics.md` already carries this correction and it is not re-argued here. Its
velocity section states it in one line: a stock read late is merely stale; a flow read late lands
in the wrong bucket, which is worse than stale because it still looks like history.

The same page shows how rare flows currently are. `notes/project-metrics/weekly.csv` has 58 columns.
Three of them are flows (`milestones_built_this_week`, `merged_pull_requests`, and the cost
columns, which are a capture rather than a derivation). Every other column is a stock: what the tree
holds at a revision.

That is exactly the wrong shape for this question. "How many harnesses are falsified" is a stock and
it is already tracked. "How many times did the sweep replay one, and how many went red" is a flow,
it is the number that separates the two readings of `falsifications.yml`, and nothing in this tree
emits it.

So every measure proposed below is an event count, or the row says why it cannot be one. A
mechanism that can only report its current state is, for this purpose, mute. `merge-drain.sh`'s
3,355 snapshot lines are the proof: complete, honest, retained for weeks, and unable to answer the
one question asked of them.

## The layers

Enumerated from the tree on 2026-09-23, not from memory: the watchers, the queue hold, the blocking gates, the scheduled cadences, the proof
and falsification machinery, the ledgers that already measure a layer, the briefs, the role
structure, and the records layer. The last four are where the honesty is. Each layer's members and what would
count as a catch in it are in
[layers.md](0584-does-this-layer-earn-its-keep/layers.md).

## The measure per layer, and where there is none

| layer | measure | shape | honest? |
|---|---|---|---|
| 1. watchers | actions and transitions emitted as event lines, counted per week | event | yes |
| 2. queue hold | holds placed, releases, and the eligible set at the moment of each hold | event | yes |
| 3. blocking gates | per-check-name red conclusions on pull requests, and whether the branch changed after | event | yes, with a clock |
| 4. scheduled cadences | runs, red runs, and work items each run actually processed | event | yes |
| 5. proofs | harness failures against real changes, separated from replay-patch failures | event | yes, and expected to read near zero |
| 6. ledgers | already measured; extend nothing | event | yes, already |
| 7. briefs | none | none | no, and no proxy is offered |
| 8. role structure | none | none | no, and no proxy is offered |
| 9. records | misses only: corrections that cite a missing record | event, wrong sign | partial, and the asymmetry is the finding |

The notes on each row of the table are in
[layers.md](0584-does-this-layer-earn-its-keep/layers.md).

## The uncomfortable half

A mechanism that has never caught anything is either unnecessary or blind, and from outside those
two are indistinguishable. `falsifications.yml` looked like both for three weeks and was the
second one. A workflow that has never gone red is the same sentence: it might be guarding a
property nothing violates, or it might have a `gh` call missing `--repo`, which is what
`coe-architect-label.yml` turned out to be on its first real input.

So this proposal names the consequence and does not wire it up: removing a layer stays a
legitimate outcome of the measurement, and nothing automates it. If a `script/lint` check has
rejected nothing in six months, and the failure it guards against is one the tree can no longer
represent, the check is maintenance cost with a green badge on it. `script/lint` has deleted three
checks already and the tree is better for it.

Whether a zero-catch count is a reason to delete or only a finding is deferred, not settled
(calef, 2026-09-24). The last section of this file records the ruling, why the measurement had to
come first, and what brings the question back. Read that before quoting the paragraph above as a
policy: it is the hazard this proposal is watching for, not a rule anybody has adopted.

Two exemptions, written now because the test would otherwise delete the best machinery here.
They survive the deferral: they are facts about what a count can mean, not positions on what to do
about one, so they hold whichever way the deferred question is eventually answered.

1. Rung-one mechanisms are exempt by construction. `AGENTS.md`'s ladder puts *make the wrong
   state unrepresentable* at the top, and its worked example is
   milestone 50 (pipes and redirection), which turned
   `InputDeclaration::Required` into a variant that carries `writes_while_reading`, so a program that
   writes while it reads cannot be declared without saying so. That mechanism will catch zero
   things forever, because the defect it prevents cannot be typed. A zero-catch count on a rung-one
   mechanism is the mechanism working perfectly. This test applies only to mechanisms that operate
   after the fact, which is rungs two through four: gates, watchers, cadences, records. Deterrence
   and blindness produce the same number and only the rung tells them apart.
2. A low-frequency, high-consequence mechanism is not judged on count. `at-risk-check.sh` guards
   the only failure in this system that destroys rather than delays. One catch a year justifies it.
   The measure is still worth taking, because zero catches *and* a lane that lost work would mean it
   is blind.

The first thing the measurement will report is that most layers have no number, and that is
information rather than a reason to postpone.

## The first increment

One layer, instrumented, surfaced where the measures already live. Not nine.

Take layer 1, the watchers. It is the cheapest by a wide margin: the action sites exist, the
change is a line each, the log files already exist and are already retained, and one of the five is
being instrumented right now by another lane, which means the shape can be copied rather than
designed.

1. Fix the event vocabulary in a note, as a short table: one prefix per event kind, one line per
   event, the fields each carries. Do this first and in the same commit as nothing else, because the
   cost of a second format is paid by every later reader.
2. Emit events from the four watchers this proposal may touch: `trunk-health.sh`,
   `lane-claim-check.sh`, `at-risk-check.sh`, and `script/cadence-check`. Do not touch
   `merge-drain.sh`; `maintainer/what-the-machinery-did` holds it. Fold its format into the
   vocabulary table afterwards, or ask it to adopt the table, whichever lands second.
3. Count them. A provisional `script/what-fired` that reads the log files and prints events per
   kind per week. It is a reader, not a gate; nothing fails.
4. Surface it in the two places that already exist. Flow columns in
   `notes/project-metrics/weekly.csv`, drawn on
   notes/project-metrics.md (what moved, week by week), which is the page for numbers that move;
   and a row apiece in notes/register-of-measures.md (every number this kernel owes itself),
   almost certainly `dated` rather than `gated`, because nothing should fail when a watcher has a
   quiet week.

The immediate follow-on, and it is a separate lane because it has a deadline the first does not:
layer 3's per-check-run capture, since ninety days of history is being deleted while this sits.

What is deliberately not in the first increment. Layer 5 needs the replay-versus-real
distinction designed before anything is counted, and getting that wrong produces a number that
conflates the two, which is the defect this proposal is about. Layers 7 and 8 are refused above.

## What this does not solve

Attribution. Separating automation's actions from calef's is a different question, handled
separately by `maintainer/what-the-machinery-did`. It is a prerequisite for some of these
measures and a substitute for none.

Which measures depend on it:

- Layer 3's red-then-fixed count. Whether the fix came from the lane that broke it, from a
  maintainer session, or from calef changes what the number means, and today every commit and every
  pull request in this repository carries calef's name whether he wrote it or not.
- Layer 6's ledgers, which already record delegated work and are already implicitly about who
  did what.
- Layer 2's holds, if the question is ever "who held the queue" rather than "how often was it
  held".

Which do not, and this is why the first increment was chosen as it was:

- Layer 1's event counts. A watcher runs under `launchd` with no human in the loop; every event
  it emits is the machine's by construction.
- Layer 4's cadence runs. Same.

And this proposal does not gate anything. Nothing here fails a build. `AGENTS.md` names the
reason in the section on identified work: no check can tell an intention from an observation, and
the layer being measured here is partly prose. This is rung three, a record at the thing itself,
plus a reader that totals the records.

## The ruling: measure first, and the question that comes back

Put to calef as a binary (a reason to delete, or only a finding to write down). His answer on
2026-09-24 was neither, and recording it as either would misstate it:

> I think we want to measure as a first step and then follow up with remediation separately. We
> don't understand the data yet and need to process it. So I'd add it to our metrics page so that we
> can review it together and ask questions. An action might be to delete, but we probably won't
> automate that at first.

What that settles, and it is most of the proposal. Measure first: the first increment proceeds
exactly as written above. The numbers go on the metrics page so they can be read and questioned
together, rather than triggering anything. Remediation is separate work, not a consequence
wired into the measurement. And deletion is explicitly not ruled out; it is simply not
automated, and not yet.

What is deferred, stated as a deferral rather than as a posture. Whether a zero-catch count on a
rung-two or rung-three mechanism, over some window, is a reason to remove it. That is an open
decision, and this tree's convention is that an open decision lives in a file rather than in a
conversation. "A finding only" is a settled posture and is not what was ruled; this is an open
question with a trigger, and a reader who takes the difference casually will later quote a deferral
as a decision.

Why the measurement had to come first, and this defeats the argument this file originally made.
The proposal argued, and the maintainer agreed and went further by proposing that each mechanism's
window be stated in advance, that the question must be settled before the numbers exist or it gets
settled while looking at a specific check somebody is attached to. calef's counter is the better
argument: we do not yet understand the data, and a threshold chosen in ignorance is no better than
one chosen under attachment. Nobody can pick a defensible answer to "how long is too long with
zero catches" without knowing what the distribution looks like across nine layers that have never
been counted. The original argument is kept here rather than deleted, because it was wrong about the
ordering and right about the hazard.

The trigger that brings it back. When layer 1's event counts have been on
notes/project-metrics.md (what moved, week by week) long enough to show a shape across the
layers that have numbers, and calef has read them. That is a judgment about the data rather than a
week count, and putting a week count here would be the same threshold-in-ignorance the ruling
refuses. The first increment's own step 4 is what makes the trigger reachable.

The hazard to carry into that conversation, which the deferral does not dissolve: decide on
the class, not on an instance. The reason the question was raised early is that deciding about a
specific check while looking at it is deciding under attachment, and waiting for the data does not
remove that pressure, it postpones meeting it. Whoever takes this up should write the rule for
rung-two and rung-three mechanisms as a class first, then apply it, rather than reading nine rows
and arguing about the smallest number.

## Index row

Instruments the mechanisms this tree relies on (watchers, gates, cadences, proofs) to emit events rather than snapshots, so each can show it has caught something a person would have missed. Prompted by four mechanisms found on 2026-09-23 reporting success while checking nothing, it is the third step after §134's falsification records and the gate-must-have-failed rule; the first increment is the watchers alone, counted onto the metrics page, and whether a zero-catch count justifies removing a mechanism is deferred by calef until the data exists.
