---
status: DECIDED
raised: 2026-10-29
decided: 2026-10-29
ratified_by: calef
---

# 318. A journey's status is a field, and its progress stays computed

*Section number provisional, and the title and slug are a lane's coinage; the merge queue mints the
number at merge. Ruled 2026-10-29 (UTC).*

A journey file opens with frontmatter carrying two keys, `status` and `raised`, plus `retired` when
the status is `RETIRED`. `status` takes exactly one of five words: `PROPOSED`, `WORKING`,
`PARTIAL`, `DELIVERED`, `RETIRED`. The number stays in the filename and the title in the H1.
`script/journeys --check` holds every file to the schema and to four rules about the milestones it
cites, and `script/journeys` computes the journey's progress from those milestones and the cited
decisions on every run. `script/lint` runs that check, so a file that breaks the schema fails CI and
the pre-push hook rather than waiting for a reader to run the command.

| key | values |
|---|---|
| `status` | `PROPOSED`, `WORKING`, `PARTIAL`, `DELIVERED`, `RETIRED` |
| `raised` | `YYYY-MM-DD`, UTC |
| `retired` | `YYYY-MM-DD`, UTC, on `RETIRED` alone |

## Why a journey has a status field

`design/journeys/README.md` refused one on 2026-08-31, and the refusal named its own trigger: ask
again at the second retirement, when there is a population to generalize from. The trigger did not
decide this. A different lack did.

The tool could not tell two states apart. Journey 3 read `9/12 built` and journey 4 read `0/4
built`, and neither line said whether anybody was on the work. A count answers "how far", and a
reader of a journey asks "is this still wanted, and is anyone moving it". The file had no place for
that answer. Retirement was the one exception, carried by a bold marker in the body, so the project
could say "stop" and had no way to say "start".

The field is not a progress claim, and the count does not move into it. `status` records intent and
attention: what the project means to do, and whether a lane holds it. The number of steps on a
`BUILT` milestone stays computed, because a hand-written count drifts the moment one step moves,
which is the reason the directory refused a count in the first place. That reason still bites, and
the schema keeps it: there is no progress token and no percentage.

## The four rules the check pins down

A status that nothing compares against the steps is a word a writer chooses. Four rules give it
teeth:

1. The vocabulary. `status` holds one of the five words, and a file with no `status` fails. A sixth
   word is a schema change, which is an architect's call and not a lane's spelling.
2. The `retired` pairing. A `RETIRED` journey carries a `retired` date, and a journey with any
   other status carries none. Retirement without a date leaves a reader unable to tell when the
   story stopped, and a date on a live journey is a second status in disguise.
3. `DELIVERED` means every cited milestone is `BUILT`. The status claims a person can do the whole
   thing today, and the blocks are the evidence. A journey whose steps are not all built cannot
   make that claim, whatever a writer believes.
4. `PROPOSED` means no cited milestone is `BUILT`. The word means the story is intended and
   nothing of it exists. A journey with a built step is under way and says so.

The first tells a writer what to type. The next three stop a status from disagreeing with the
records it summarizes, which is the failure the roadmap's own status field ended.

## Why `IN-PROGRESS` is not reused

`notes/roadmap.md` gives `IN-PROGRESS` to a milestone with active work on a branch, and obliges the
block to carry a `branch` key that `script/roadmap --check` resolves against the tree. A journey
spans lanes and often spans repositories. It has no single branch to name, and a `branch` key that
held one lane's branch would say something false about the other steps. `WORKING` carries the part
that transfers, that somebody is on it, without borrowing a key the journey cannot fill.

The two vocabularies stay separate for the same reason they are separate records: a journey is a
read across milestones, not a milestone. `PARTIAL` and `RETIRED` mean what they mean in both,
because those two facts do transfer.

## Why `script/journeys` derives the delivered date

The latest `built` date among the milestones a `DELIVERED` journey cites gives its delivered date,
and `script/journeys` computes that on every run. The milestone blocks already hold every build
date, and a `delivered:` key would be a second copy of a fact those blocks own. Two copies drift,
and the drift stays invisible until somebody compares them. Deriving the date also makes the answer
follow a re-scoped milestone without an edit to the journey file.

The same argument refuses a `title:` and a `number:` key. The filename carries the number, the H1
carries the title, and `script/journeys --check` fails a pair that disagrees, which is the rule a
milestone's block and a decision's file already follow. A third copy buys nothing and gives a writer
one more place to be wrong.

## What it refuses

A hand-written count, a percentage, or any field-shaped line in the prose that restates the status
or a date. A journey's body says what the story is and what each step needs; the frontmatter says
where the story stands, and the report says how far the steps have got. Each fact has one home.
