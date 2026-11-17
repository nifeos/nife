# Milestone 584 appendix: the nine layers, and the notes on each measure

An appendix to [milestone 584 (a mechanism owes evidence that it caught something a person would
have missed)](../0584-does-this-layer-earn-its-keep.md), moved out of that page on 2026-10-12 (UTC)
under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md). The main
page is complete without it. This is the proposal's taxonomy of the layers this tree has, with what
would count as a catch in each, and its notes on the rows of the measure table, as filed on
2026-09-23.

## The taxonomy: the layers this tree actually has

Enumerated from the tree on 2026-09-23, not from memory. Nine layers, and the last four are where
the honesty is.

### Layer 1: the watchers

`helpers/merge-drain.sh`, `helpers/trunk-health.sh`, `helpers/lane-claim-check.sh`,
`helpers/at-risk-check.sh`, and `script/cadence-check`. Two run unattended under `launchd`; the rest
are run by a session.

What counts as a catch. The watcher acted or announced a transition that nobody was going to
notice. That means a pull request enqueued that had been sitting, a stall notice posted, or a
red-trunk transition reported. It also means a branch found with no claiming pull request, a
worktree found holding uncommitted work, or a scheduled workflow found to have stopped producing a
result.

### Layer 2: the queue hold

Pull request #1170 (`maintainer/when-main-goes-red`), unmerged as of this writing: hold the queue
when `main` is red and land the fix alone.

What counts as a catch. A hold placed, and under it, a pull request that would otherwise have
been enqueued onto a broken trunk. The second half is what makes it a catch rather than an action,
and it is countable at the moment the hold is placed because the eligible set is exactly what
`merge-drain.sh` computes.

### Layer 3: the blocking gates

`script/lint` carries roughly fifty named checks. `ci.yml` and its siblings put nineteen check names
on a pull request, of which eleven block, per
notes/check-inventory.md (does anything run it, does it block, and what does green mean).

What counts as a catch. A check went red on a pull request, and the branch then changed before
it merged. Red-then-fixed is the catch; red-then-abandoned is a different and also interesting
event; red-then-merged-anyway does not happen here because the required set blocks.

### Layer 4: the scheduled cadences

`falsifications.yml`, `mutation.yml`, `audit-cadence.yml`, `undefined-behavior-check.yml`,
`toolchain-drift.yml`, `vendor-watch.yml`, `stranger-cadence.yml`, `metrics.yml`, `verify.yml`.

What counts as a catch. A run that went red for the reason the cadence exists, or that opened a
pull request nobody had asked for. `script/cadence-check` already answers the weaker question, which
is whether a cadence is still producing a result at all. Its own header records that
`mutation testing` had failed four times out of four over four weeks with nothing turning red about
it.

### Layer 5: the proof and falsification machinery

145-plus Kani harnesses, §134's per-harness falsification records, `script/verify`,
`script/mutation`.

What counts as a catch. A harness that went red against a real change rather than against its
own replay patch. §134's records establish that each harness *can* fail; they say nothing about
whether any of them has ever *caught* anything, and that is the gap this layer has to close about
itself.

### Layer 6: the ledgers that already measure a layer

`script/rule-violations` totals strikes against each documented rule and flags the ones past three.
`script/redo-rate` measures how often delegated work has to be done again. Both are hand-fed, and
both say so in their own headers: `rule-violations` states plainly that *it cannot see a violation
happen*.

What counts as a catch. These are not mechanisms that catch; they are mechanisms that
measure mechanisms, and they are the closest prior art in the tree to what this proposal asks
for. Their design is the model to copy: a ledger a person appends to, honest that its numerator is
what somebody reported.

### Layer 7: the briefs

`briefs/gate-in-ci.md`, `merge-and-cleanup.md`, `rebase-onto-main.md`, `session-start.md`,
`survey-the-queue.md`, `triage-a-failing-check.md`.

What would count as a catch. A lane that did not go wrong. That is the honest answer and it is
also the reason this layer has no measure; see below.

### Layer 8: the role structure

Maintainer, developer, steward. §90 (the claim is a draft pull request), and its draft-as-claim
rule. The lane-count rule against the collision surface. The ladder in `AGENTS.md`.

What would count as a catch. A collision that did not happen, a milestone two lanes did not both
take, a piece of work that did not fall through a crack.

### Layer 9: the records layer

`design/decisions/`, the roadmap, the name provenance blocks, `BUGS` sections,
`design/roadmap/proposals/`, which exists because of
milestone 247 (follow-on work named by a finished milestone goes nowhere).

What would count as a catch. A decision that was not re-litigated because the reason was
written down. A name that was not re-refused. A limitation somebody did not rediscover.

## Notes on the measure table

The table itself stays on the parent page; these notes explain its rows.

### Rows 1 and 2 are the cheap ones

The action sites already exist in the code. What is missing is one line written at each of them,
with a stable prefix a `grep` can count. `merge-drain.sh` computes `armed` and `stalled` on every
pass; what it does not do is say *this pass enqueued #1143*. The lane
`maintainer/what-the-machinery-did` is adding event lines to that file right now. This proposal
does not edit `merge-drain.sh`; it names the vocabulary those lines should share with the other
four watchers, so the second one to be instrumented does not invent a second format.

### Row 3 has a clock on it

GitHub retains Actions run history for ninety days. `notes/project-metrics.md` already learned
this the expensive way and says so: the merged-pull-request series is counted from git rather than
from the API precisely because an API-derived series *"would have started in June and could never
have been extended backwards."* Per-check-run conclusions are not in git and cannot be recovered
from it. So this row is measurable, it is measurable only forwards, and every week nobody starts
it is a week deleted rather than deferred.

### Row 5 will read near zero, and that is the point

A Kani harness in a tree where the property holds is silent by construction. Separating "went red
because a lane broke the property" from "went red because §134's replay patch was applied" is the
only way to tell a proof that guards something from a proof that restates a tautology, and §134's
own reverse pass already found one of the latter: `capability_table::subset_is_reflexive` proves
`a & !a == 0`, which no plausible implementation error breaks.

### Rows 7 and 8 have no measure, and inventing one would be worse

A brief's value is a lane that did not go wrong, and that is unobservable. The nearest available
proxy is `script/redo-rate`'s ledger, filtered by whether a lane read the brief. It should not be
used. Lanes are not randomized, n is small, the ledger is hand-fed by the same maintainer who writes
the briefs, and every one of those is a reason the number would move for reasons other than the
brief. A ratio computed that way would be quoted long after the caveat was forgotten.

The same holds for the role structure. Merge conflicts per week looks like a measure of it and
is not: `AGENTS.md`'s own lane-count rule says conflicts scale with *files* rather than with lanes,
so the number moves with which subsystems happen to be in flight.

This tree has a recorded case of exactly the failure a proxy would be. `notes/register-of-measures.md`
carries a row for an `unsafe`/`SAFETY:` parity count that was measured and refused: it disagreed
with the tree in 65 places and every one of them was a document that was right. A check that fails
correct work is not a weak check, it is a check that gets deleted, and the register states that
`script/lint` has already lost three with that signature. An admitted gap survives; a dishonest
measure gets deleted and takes the question with it.

### Row 9 can only report its misses, and the asymmetry is worth stating

A decision record's successes are silent. Its failures are loud and are already written down:
`notes/corrections/` holds them, one file per correction of error. So this layer's only honest
number is a failure count, which cannot be read as effectiveness in either direction. A quarter
with no corrections citing a missing record means either the records are working or nobody wrote a
correction. That is the same ambiguity `falsifications.yml` had, and naming it is all that can be
done about it here.
