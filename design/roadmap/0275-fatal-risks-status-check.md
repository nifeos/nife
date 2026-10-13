---
status: BUILT
raised: 2026-09-11
built: 2026-09-11
---
# 275. A gate that diffs `design/fatal-risks/README.md` against the roadmap it cites

Built as `script/fatal-risks`, wired into `script/lint`. It found all four of the findings of a
review of `design/fatal-risks/README.md` against the tree, the file's own `BUGS` warning ("nothing
gates this file") firing four separate ways in one pass, and the corrections were part of this
milestone rather than a follow-on.

## What the 2026-09-10 review found, because this milestone exists to stop finding it by hand

Four instances, one review, no gate involved in catching any of them:

- **Risk 2** cited milestone 191 as `RUN, 2026-08-30. AMBER`, and that was true. Milestone 191's own
  roadmap block still read `NOT-STARTED`, eleven days after the work merged (PR #589), which is the
  status-in-two-places defect `§76`'s own sweep found before, recurring in the one place nothing
  compares the two records. It read `NOT-STARTED` for twelve days, because nothing corrected it
  between the review and this milestone's build; this gate's first run found it and the correction
  landed with the gate.
- **Risk 3** said re-running the mutation sweep was cheap because "the weekly workflow already
  publishes the report." The workflow had not succeeded once since 2026-08-03: a runaway mutant
  exhausts the CI runner's memory and kills the shard, inside the timeout meant to catch it. Already
  diagnosed in `mutation.yml`'s own header on 2026-09-03; `design/fatal-risks/README.md` was not updated for
  eight more days.
- Risk 6 said "unmeasured, nothing timestamps the step" for hours after the instrument that
  measures it (`design/roadmap/0306-time-the-hw-entropy-step.md`) merged and gave a real number.
- Risk 9 cited milestone 164 as blocking (no `fs_server`) ten days after 164 was `BUILT`, and
  cited milestone 177's premise a day after that premise stopped holding (`DECISIONS §149`).

The pattern across all four: each is a fact `design/fatal-risks/README.md` states about a milestone or
decision it names, and every one of the four went stale the moment the thing it named changed status,
because nothing reads the two together. `script/decisions --check` and `script/roadmap --check` both
prove a citation *resolves*; neither asks whether what `fatal-risks.md` says about what it resolved to
is still true.

## What this needs

1. Extract every milestone and decision citation in `design/fatal-risks/README.md`, the same way
   `script/citations` already extracts them tree-wide, but scoped to this one file since its citations
   carry a stated status claim (`RUN`, `MEASURED`, `unmeasured`, `blocking`, and similar) that a bare
   `§N` cross-reference elsewhere in the tree does not.
2. Compare the claimed status against the roadmap's or decisions' own recorded status. The
   simplest version: a risk marked with a specific milestone's status word (e.g. "milestone 191...
   AMBER" implying 191 is done) should fail loudly if that milestone's own `Status:` line disagrees
   in a way that matters (`NOT-STARTED` when the risk entry assumes it ran, `SUPERSEDED` when the risk
   entry assumes it stands).
3. This cannot be fully mechanical, and the block should say so rather than overpromise: risk 9's
   two stale claims were about a *premise* (does 177 need the graphical stack) that changed for a
   reason no status field encodes. The gate's honest scope is catching the *first* kind of drift
   (a status word disagreeing with the roadmap), not the second (an argument's premise being
   overtaken). Naming the boundary now is cheaper than a gate that claims more than it checks.
4. Where in `script/lint` or `script/gates` this belongs, and whether it runs on every push or on
   a cadence: `design/fatal-risks/README.md` does not change often, so a cheap weekly check (the shape
   `script/cadence-check` already uses for scheduled workflows) may fit better than a per-push cost
   on every unrelated pull request.

## What got built, and what the four open questions above resolved to

`script/fatal-risks`, **name provisional**, with the provenance block every `script/` entry carries.
Three modes: a report, `--check` (wired into `script/lint`), and `--selftest`.

Eight checks, each anchored on a marker the file already writes, because the alternative is
reading a status claim out of free prose and that is a false-positive generator. A risk whose
`**Status: RUN**`/`MEASURED` names an experiment milestone that is `NOT-STARTED`; a risk whose
experiment is explicitly *not* run naming one that is `BUILT` (the rule `script/roadmap` already
keeps for a `Gate: MILESTONE N`); a struck row in the running order owned to a milestone that never
started; a status word the file spells out (`milestone 161 is PARTIAL`) that the record contradicts;
a cited path that does not exist; a cited proposal changed after the entry that cites it was last
dated; and a risk entry whose own latest date precedes the `Built` date of a milestone it cites.

Question 1 (extraction) reused `script/citations`' shape rather than inventing a second parser,
and the records come from `script/roadmap` and `script/decisions`' own report output rather than
a second derivation of what a status is. Only the `Built` column is read from the index, because the
reports do not carry a date, and that column's shape is already enforced by `script/roadmap --check`.

Question 2 (what "disagrees in a way that matters" means) came out as a vocabulary split rather
than a similarity measure. `NOT-STARTED` is the only token that can contradict "this ran";
`BUILT`/`REMOVED` are the only ones that can contradict "this has not run yet"; and `PARTIAL` sits
in neither on purpose, because a partial milestone is honest evidence for either claim. That one
choice is most of what keeps the check quiet, which is what the `BUGS` section below demands.

Question 3 (this cannot be fully mechanical) is unchanged and is stated in three places now: the
script's header, its `--check` failure message, and `design/fatal-risks/README.md`'s own `BUGS` entry, which
this milestone struck through for the mechanical half only and which now says in as many words
that a green run is not a warrant that the arguments still hold.

**Question 4 (where it runs) resolved to `script/lint`, per push, not a cadence.** The precedent that
decides it is `script/cadence-check`'s own header, which is the nearby script that answered the other
way and says why: *"a dead cadence is not a defect in the commit that happens to trip it."* Drift here
is the opposite and always is. It is caused by an in-tree edit, either a milestone's status moving or
this file being written against a tree that has since changed, so the commit that trips it is the
commit that caused it and the lane holding it can fix it in the same breath. That is the definition
of a lint. The measured cost is about a second, nearly all of it the two report subprocesses.

The cost of that choice, stated rather than discovered: 35 milestones are cited by
`design/fatal-risks/README.md`, so roughly one lane in ten that turns a cited milestone `BUILT` will trip
the "as of" check and owe a dated correction paragraph. That is the intended bill. The file's own
idiom for paying it already exists (*"Two corrections, 2026-09-03, from the §86 research lane"*) and
the failure message names it.

## How it is known to be able to fail

`script/fatal-risks --selftest`, eleven fixtures, run by `script/lint` beside `--check`. Eight
positive fixtures must each come back red on exactly their own check; three negative ones must stay
quiet. One of the eight is a check that *refuses* rather than answers: a shallow clone has no commit
touching a tracked file, so the proposal check would pass vacuously in one and reports instead, which
is `script/roadmap`'s own lesson from the day its merged-branch check "spent a day passing in CI
while being unable to fail". The negatives are the more interesting half, since this gate's stated bias is that a false
positive costs more than a false negative: a `RUN` verdict over a `PARTIAL` milestone, a *pending*
running-order row over a `BUILT` owner (row 5 owns risk 3 to milestone 85, which is built, while its
experiment is a re-run that has not happened), and a milestone built before the citing entry's own
date.

And all three of the 2026-09-10 review's mechanical findings were reproduced against the real
file, by reverting each correction in turn and watching `--check` go red:

| reverted | what the check said |
|---|---|
| milestone 191 back to `NOT-STARTED` | two findings, `experiment-ran` at risk 2 and `running-order` at the table: *"A struck row and a NOT-STARTED owner cannot both be true"* |
| risk 9's milestone-164 correction | `as-of`: *"risk 9 was last dated 2026-08-30 and cites milestone 164, which the roadmap records as BUILT on 2026-09-01"* |
| risk 6's instrument correction | `proposal-moved`: *"risk 6 was last dated 2026-09-05 and cites `design/roadmap/0306-time-the-hw-entropy-step.md` as work that would answer it; that proposal was last changed 2026-09-10"* |

The selftest is what makes that repeatable. The three reverts above are a person's experiment, run
once, and by this file's own standard that is an attestation rather than evidence.

## BUGS

- **Nothing here catches an entry that was never wrong syntactically but became wrong in substance**,
  the risk 9 shape above. That gap should stay named rather than quietly scoped away once the
  mechanical half is built. It was not: the script's header, its failure message and
  `design/fatal-risks/README.md`'s `BUGS` all say it. The worked example is risk 9's *other* stale claim,
  that milestone 177's premise was overtaken by `DECISIONS §149`, which no status field encodes and
  which this gate reports nothing about.
- The "as of" check reads a risk section's latest stated date, which is a proxy for when somebody
  last looked at it. A lane that fixes a typo without touching a date leaves the proxy correct; one
  that adds a date while changing nothing else silences a real finding. The proxy is the file's own
  convention rather than something invented here (every verdict in it is dated), and it is still a
  proxy. It also fires on a gap of one day: risk 3 was dated 2026-08-03 against milestone 85's
  `Built` date of 2026-08-04, and the finding happened to be real for a reason the check could not
  see, which is luck rather than mechanism.
- **The selftest proves each check can fire, not that it fires on everything it should.** A fixture
  is one worked example per check. A prose shape the script does not recognize is caught by the
  "as of" check or by nobody, and no fixture can tell you which.
- **A false negative is cheap here and a false positive is not.** `design/fatal-risks/README.md` is meant to
  be read and trusted; a gate that cries wolf on a status word that moved for a harmless reason
  (a milestone amended, not overturned) teaches the next person to ignore it, which is worse than the
  drift this milestone exists to catch.

## Follow-on

- **Done.** `script/fatal-risks`, wired into `script/lint`, with the four corrections its first run
  demanded: milestone 191's status, and dated corrections to risks 3, 6 and 9.
- **Recorded.** The premise gap, in this block's `BUGS` below and in `design/fatal-risks/README.md`'s own
  `BUGS`, which is struck through for the mechanical half and explicit that the other half is still
  a person's job.
- **Recorded.** `script/fatal-risks`' name is provisional and its `Name:` block carries the
  refusals (`risk-check`, `falsifications-check`, `fatal-risk-drift`). `script/names --unratified`
  is the worklist.
- **Done.** Risk 3's re-run, which this gate made visible, is done: milestone 277 (bound what one
  mutant may allocate) bounded the runaway mutant that blocked the weekly mutation workflow, and
  risk 3 is MEASURED, AMBER.
- **Milestone 306.** Risk 6's remaining half is measured: milestone 306 (time the hw-entropy step)
  timed it on one boot of radon at 955,223 bytes/s (2026-09-16), and risk 6's third bullet carries
  it.

## Index row

`script/fatal-risks`, in `script/lint`, diffs `design/fatal-risks/README.md` against the roadmap
it cites, because a review found the same status-in-two-places defect four separate times in one
file, one pass, with nothing catching any of them.
