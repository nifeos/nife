---
status: SUPERSEDED
raised: 2026-09-20
superseded_by: 740
promoted_from: the-census-writes-its-own-row
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 532. The mutation census should write its own row, rather than a person remembering to

Overtaken by commit `c2d34982f` (2026-10-03, milestone 740): the weekly mutation workflow's
`record` job appends each census to the tree through a pull request, so capture no longer depends on
someone running `script/mutation-census --add-run`. That is this block's option 2. The text below is
the block as filed.

Promoted from the proposal `the-census-writes-its-own-row`, filed 2026-09-20, on calef's instruction
of 2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own,
unedited except for this paragraph: the argument is its author's and promotion is not the moment to
improve it. Written by milestone 518 (a census that cannot be attributed is a number nobody can act
on)'s lane, from the gap its own `BUGS` section names. The record it built is rung three of
AGENTS.md's ladder, a thing somebody has to remember, and the tenet says to move up a rung when you
notice that shape.

`.github/workflows/mutation.yml` has `permissions: contents: read` today, and
every workflow in this tree makes that same least-privilege choice deliberately. Changing it for one
job is an architect's call, not a lane's.

## The gap

`script/mutation-census --add-run <id>` captures a finished census in one command. Nothing makes
anybody run it. The failure this is trying to prevent has already happened four times: the weekly
workflow ran, the per-crate numbers existed for the length of one step summary, and the tree kept
nothing. `design/fatal-risks/README.md`'s risk 3 then had to record that it could not say which crates
caused a fall, because the numbers that would have said were gone.

The backfill only worked because GitHub still held the artifacts. Its default retention is 90 days.
The same recovery attempted in December 2026 would have found nothing at all, so the window in which
a missed capture is fixable is short and closes silently.

## What it would take

`mutation.yml`'s `report` job already downloads every shard and aggregates them. One more step runs
`script/mutation-census --add` against the same directories and commits the result. The choices,
which is what makes this a decision:

1. Commit to `main` from the workflow. Needs `contents: write`. Smallest mechanism, and it puts
   a bot commit on the trunk of a repository whose history is otherwise all reviewed work.
2. Open a pull request. Needs `contents: write` and `pull-requests: write`, plus a branch per
   census. A human still has to merge it, which is a smaller thing to remember than running a
   command, but is not nothing.
3. Upload the CSV fragment as a long-lived artifact and leave the commit to a person. No new
   permission. It moves the deadline out rather than removing it, and the deadline is the problem.

**Recommendation: 2.** A census is a fact about the tree and a fact about the tree should arrive the
way every other one does, through a diff somebody sees. The rate moved two points between two
censuses and nobody noticed for five days; a pull request is the notification as well as the record.
**If calef says no**, option 3 buys time and the habit stays a habit, which is where it is today, so
nothing gets worse.

Would we still choose 2 if all three cost the same? Yes. Option 1 is cheaper to build than 2 and
that is not why 2 wins. 2 wins because a bot that can push to `main` is a standing authority and a
bot that can open a pull request is a request.

## What is blocked until it is answered

Nothing. `script/mutation-census --add-run` works today and the record is backfilled. This is about
whether the next census is captured by a mechanism or by somebody remembering.

## Index row

`script/mutation-census --add-run <id>` captures a finished census in one command.
