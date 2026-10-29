# nife: user journeys

*Name: ratified 2026-08-26 (calef's own word: "I'd like to start tracking user journeys"), not a
lane's choice to make provisional.*

A journey is an end-to-end, user-facing story, told as an ordered bundle of the milestones (and,
where a step is blocked by one, the decisions) that have to be true at once for someone to actually
live it. `design/roadmap/` tracks work item by item; a journey is the read that answers a different
question than any single milestone can: *could a person actually do the whole thing today, start to
finish?* Journey 1 (login to `kilo`) is the worked example this directory was minted from: no single
milestone was "blocked," but tracing the story end to end found a real, unowned gap (milestone 177)
that nothing surfaced by scanning the roadmap milestone by milestone.

## How this directory works

One journey, one file, named `NN-slug.md` (single digits zero-padded so the directory sorts, the
same convention `design/roadmap/` uses). calef, 2026-08-26, on adding the second journey: numbers
should be the shorthand identifier, cited as **`journey N`** in prose, the same shape `milestone N`
already has. The number is the identity, same as a milestone's or a decision's: `journey 1` is
`01-login-to-kilo.md` and nothing else. Unlike a milestone or a decision, a journey is not currently
cited *from* elsewhere in the tree (it only ever points outward, at scopes that already have one),
so nothing yet depends on a journey number staying fixed the way `§N` citations depend on a decision
number staying fixed; that may change once something cites one.

**A journey may be RETIRED, and that is not the status field this refuses.** A line beginning
`**RETIRED` anywhere in a journey file marks the story as one the project stopped telling;
`script/journeys` reads it and prints `RETIRED` in place of the built count. The distinction is the
one the paragraph below draws: what that paragraph refuses is a hand-written *progress* claim, which
drifts because nobody re-derives it. Retirement is *intent*, and intent has no computable source, so
a human has to write it and a tool can only be told. It was added 2026-08-30 when journey 2 was
retired and the report kept listing it as live work, which is the record and the mechanism
disagreeing, the thing AGENTS.md's ladder exists to stop.

**Ratified as-is on 2026-08-31 (calef), against growing it into a status field.** The question was
real, because `design/roadmap/` minted `REMOVED` the day before and three record types with two
status vocabularies and one marker is an inconsistency a newcomer meets. It was declined on the
roadmap vocabulary's own lesson: `IN-PROGRESS` earned its extra rule by being wrong every single
time it was used, and **one journey has been retired**, so a field would be machinery built for a
population of one. The paragraph below still stands as the trigger: ask again at the second
retirement, when there is something to generalize from.

**A journey has no status field of its own, and that is deliberate.** A hand-written "70% done" is
exactly the kind of claim this tree's own `counted-claims` mechanism exists to replace: a number
nobody re-derives drifts the moment one of its steps moves. `script/journeys` computes a journey's
state by reading the milestone blocks in `design/roadmap/` and the decision files in
`design/decisions/`, the same records `script/roadmap` and `script/decisions` already treat
as ground truth, so a journey file records
only what a human has to write: which steps, in what order, and why each one is on the path.

## Index

GitHub renders this README as the directory index. `script/journeys` is the live view; this table
is the identity mapping only: "what number is this", never "what state is it in". Neither
`design/roadmap/` nor `design/decisions/` keeps an index any more, and the reason they can afford
not to is that nothing there needs a second copy of a number.

| # | journey |
|---|---|
| 1 | [Boot to a login prompt, into `swish`, to editing a file with `kilo`](01-login-to-kilo.md) |
| 2 | [Run a Time Machine backup host](02-time-machine-backup-host.md) **(RETIRED 2026-08-30: the customer moved to borg over SSH)** |
| 3 | [The same story, on real silicon, on all three architectures](03-login-to-kilo-on-silicon.md) |
| 4 | [Install a system composed from packages, and update it from packages](04-install-and-update-from-packages.md) |
| 5 | [Externalize the publish-ours crates, one release at a time](05-crates-released-on-their-own.md) |

## The format

A journey file is a short narrative (what the story is, why it matters) followed by one table:

| step | milestone | decision | what this step needs |
|---|---|---|---|
| 1 | 33 | | one line naming what this step of the story needs from that milestone |

- **step.** Ordinal, for reading order only; `script/journeys` does not enforce that a lower step
  finishes before a higher one starts; some journeys have steps that are genuinely independent
  (journey 1's own steps 3 and 4 are unordered with respect to each other, and its own prose says
  so).
- **milestone.** The milestone number this step's progress is read from. Required.
- **decision.** A `§N` this specific step is additionally blocked by, when the milestone's own
  status does not tell the whole story (a `PARTIAL` milestone can still have the one piece a journey
  needs sitting behind a separately-decided hold; journey 1's own step 4, before DECISIONS §120 was
  amended 2026-08-26, is the worked example). Blank when the milestone's own status is the whole
  answer.
- **what this step needs.** One line, human-written, naming the specific slice of that milestone
  this journey's story needs, not the milestone's full scope. This is the column a milestone's own
  coarse status cannot carry: `PARTIAL` says nothing about *which* part is missing.

## Reading a journey's state

`script/journeys` prints, per step, the milestone's status from its block and (when a decision is
cited) that decision's status from its frontmatter, then a one-line rollup: how many
steps sit on a `BUILT` milestone with no additional decision blocking them. A step whose milestone is
`BUILT` but whose cited decision is not `DECIDED` in the direction the step needs still reads as
blocked; the tool does not try to parse *which way* a decision went; it says what a step needs and
lets a reader hold the decision's own text against it.

**This is a report, not a gate.** Nothing fails a build over a journey's state, the same way
`script/roadmap --ready` reports rather than blocks. A journey can regress if a milestone it depends
on gets re-scoped down to `PARTIAL`, and that is information, not a defect for CI to catch.
