# The roadmap's vocabularies: why each word exists

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/roadmap.md](../roadmap.md), which carries each vocabulary's tables and rules.
This page carries the history behind them: the failures that minted each status word, the reasons
for the dependency fields, and how the follow-on dispositions arrived.

## The status vocabulary

A detail block may narrate its state in prose (that is where the evidence belongs). But the
`status` key is what answers "where do we stand", and since 2026-09-21 it is the only record of it
there is. The rule here once read *"if the two disagree, the column is wrong and the block is
right"*. That was the correct answer to a question that should not have existed: two hand-maintained
records, both looking authoritative, and a reader with no way to tell which was lying. Milestone 69
(split `kernel/src/user.rs` by service) had a row saying `NOT-STARTED` while its block said
`BUILT`. Milestone 294 (the index is generated, not hand-maintained) dissolved the question by
deriving the column; retiring the column finished the job.

### `REMOVED`

`REMOVED` was minted 2026-08-30 (calef), when the code of milestone 54 (A network file service a
Mac can actually mount) was deleted, and the six words then available could only lie about it.
`NOT-STARTED` denies it was ever built. That erases the one demonstration this project has that a
real Mac mounted a share served by this kernel. `OPTIONAL` reads as available to pick up.
`RECORDED` means the work was deliberately *not* taken, and here it was taken and then reversed.
Leaving it `BUILT` makes the column claim a network file service that does not exist. And this
table's own rule says that when column and block disagree, the column is wrong.

It carries the same extra obligation `IN-PROGRESS` does, and for the same reason: the block must
say when the code went and why. A status that records a deletion is the one nobody will remember to
explain. A year later, the difference between "removed because the customer left" and "removed
because it never worked" is the entire content of the row.

One word, not two. Built-then-deleted and specified-then-abandoned are genuinely different, and
splitting them was refused. The case has occurred once, and this vocabulary's own history is that a
word earns its keep by being used correctly. Milestones 54 and 55 both take `REMOVED`, and their
blocks say which kind they were. The cost of that choice showed up immediately, and is why the date
is optional. 54 was `BUILT` and has one, 55 was `PARTIAL` and has none, and the gate rejected the
first draft of this rule within a minute of it being written.

### `REFUSED`

`REFUSED` was minted 2026-09-20 (calef), with the same complaint behind it that minted `REMOVED`:
the existing words could only lie. This tree carried 140 `- **Refused.**` bullets across 98
milestone blocks, and exactly two of them named a milestone. So a refusal that named real work
lived in the last section of a finished block and was read once. `RECORDED` was the nearest word,
and it says something else. It means the analysis was captured and the work deliberately not taken,
and it carries no condition and nothing asks it for one. The whole value here is the condition, so
a status that did not demand one would have reproduced the bullet in a bigger font. `NOT-STARTED`
says pending where this says declined, and `OPTIONAL` reads as available to pick up.

It carries an extra obligation nothing else does: a `## Revisit` section. Its bullets open with one
of three words. `Condition.` says what would make this worth reopening. `Nothing.` says it is
refused permanently, on a principle. `Unstated.` says the original names no condition and none can
honestly be inferred. Silence is the one answer that is not allowed. Silence and "we thought about
it and nothing would change our minds" look identical to the next reader and mean opposite things.

The condition is not a promise to build. A promise is exactly what §71 (A limitation is promoted
when it stops being a fact and becomes a plan) refuses to let a `BUGS` entry become. It is the
difference between a dead end and a door with a bell on it, and `script/roadmap --revisit` is the
bell. See milestone 448 (a refusal gets a number, a status, and a condition that would change it).

### `IN-PROGRESS`

`IN-PROGRESS` earned its extra rule by being wrong every single time it was used (2026-08-17). A
sweep checked all six rows carrying it and found six false. Milestones 58, 80, 112 and 115 had
merged, 115 thirteen days earlier, and 47 and 117 named no branch because nobody held them. The
same sweep audited nine `NOT-STARTED` blocks against the artifacts they named and found all nine
honest, so this is not general rot. It is one token, and the reason it rots is structural rather
than careless. It is a manually-maintained cache of a fact that lives somewhere else and expires on
its own. Merging a lane deletes its branch, which is exactly the moment the token becomes false, and
nothing was watching. Naming the branch is what makes the claim falsifiable at all, which is why it
is required rather than suggested.

The stronger fix is to retire the token, and that is an architect's call rather than a lane's.
§90 (the claim is a draft pull request; the status flip is a gate) made a lane's first act a draft
pull request. So `gh pr list --draft` already answers "who is on this right now", and cannot go
stale, because merging removes the row. A status token duplicating that fact is the lower rung by
construction. Until that is decided, the check is the tripwire.

### The `Built` date, and why it is UTC

The `Built` column is the date a milestone turned `BUILT`. It is empty for every other status but
`REMOVED`, which keeps it: the milestone did turn `BUILT` on that date, and nothing later makes that
untrue. Dates in this tree are UTC (calef, 2026-08-04), because they arrive from three sources that
disagree. A git author date is local to whoever committed. A lane deriving a date reads whatever
`git log` gives it. And a maintainer typing "today" means their own midnight. An evening commit in
California is already tomorrow in UTC. That is how the row of milestone 22 (trusted init: verify
it, and shrink what a broken one can do) came to read a day ahead of the commit that flipped it, and
looked like a defect. One zone, stated here, and the disagreement stops being a puzzle.

## The dependency fields

`unwritten` is a recorded limitation. 41 blocks carried a bare `DECISION` gate at the switch,
naming no section, so each is waiting on a fork nobody has written up. The honest value is the
word, not an invented section. Writing one up and replacing the word is how it closes, and the full
report lists them under `DECISION`.

A dependency is never cleared. It stays true after its target lands, and whether it still blocks is
computed from the target's own status. The `Gate: MILESTONE N` line this replaced failed the build
the day N turned `BUILT`. That obliged an edit in a block nobody in the landing lane was reading,
and turned unrelated branches red (the argument of §207 (the roadmap is a graph, and the block says so in fields a script can
walk)). A decision works the same way: once its section
is ruled, it no longer blocks, and nothing is edited to make that so.

`HARDWARE` was four things, and the three machine fields are why. The old token covered a board
not yet here, a board here that needs somebody at it, one specific machine for a like-for-like
comparison, and a rented account. The one that never discharges by waiting is the person, and a
field named for hardware hid it from every count: 33 of the 34 `HARDWARE` gates turned out to need a
person. The migration read all 34 by hand.

## The follow-on dispositions

The follow-on vocabulary is the `## Follow-on` section every finished block answers (milestone 247
(follow-on work named by a finished milestone goes nowhere, and this is the third time),
2026-09-03). The dependency fields say what stops a milestone starting. This says what happened to
the work the milestone named on its way out. It exists because that work is the thing this project
keeps losing, three times on the record:

- Milestone 90 (A guard page under the per-CPU secondary stacks) exists only because calef happened
  to be at his desk the day a report named its finding.
- Milestone 94 (the untracked-work sweep, and the convention that ends the category) swept the tree
  for this exact category, and left its own inventory in a pull request body for twelve days.
- Milestone 244 (the largest crate in the tree is proved by nothing a mutation can reach) named an
  unvouched-binary hazard and a design fork that surfaced only because calef asked for them by hand.

### `PARTIAL` blocks, and `Outstanding.`

`PARTIAL` was added by milestone 252 (A `PARTIAL` block claims work is remaining and nobody
re-reads it), and it is the harder half. A finished block is written once and closed, so its
section is a record. A `PARTIAL` block's is a standing claim about the future, edited as pieces
land, and nothing re-reads what it still asserts. Its prose is also what a lane reads when deciding
what to pick up. So a stale one does not merely misinform: it offers work that does not exist.
Milestone 16 (real hardware and IOMMU-backed driver isolation) listed three things as remaining. Two
of them were finished, and the block said otherwise for weeks.

`Outstanding.` is the word a `PARTIAL` block needs that a finished one does not. The other seven
say where work *went*. A `PARTIAL` block's commonest honest answer is that it has not gone anywhere
and is still this milestone's own scope. None of the seven can say that without lying. A lane forced
to choose would write the comfortable word or leave the item out: the silence this gate exists to
stop, arriving through the gate. It is refused on a `BUILT` or `REMOVED` block. There `Recorded.`,
`Proposed.` and `Milestone N.` already cover the ground, and a softer synonym would only give them
somewhere to hide.

`None.` is refused on a `PARTIAL` block, which is the one thing this gate can prove rather than
merely enumerate. `PARTIAL` means work remains, so a block answering that nothing is outstanding has
contradicted its own status word. Either name what is left, or the status is stale and the row wants
`BUILT` with a date. That is the milestone-69 defect (row and block disagreeing) one level in.

### What the gate can and cannot do

That is unchanged. It checks that the claims are enumerated and that each resolves to something
that exists. It cannot check that a claim is still true. What catches staleness is a person writing
the dispositions out, which is what both sweeps demonstrated: 247's found three items already built,
and 252's found more.

An explicit refusal is a success here. The defect being attacked is silence, not the absence of
a milestone, and `None.` is meant to be the cheapest sentence in the roadmap.

It does not weaken the `BUGS` convention and cannot. `Recorded.` points at a limitation recorded
beside the feature; it never replaces one. Consider an over-strict version of this rule, where every
observation had to resolve to a milestone. It would make the honest thing expensive to write and
thin the `BUGS` sections out, which costs more than the burial does. Nothing here reads prose
looking for intent: AGENTS.md priced that at `git grep -w TODO`'s 82% false-positive rate.

### `Proposed.` and `Done.`

`Proposed.` is the one that carries milestone 247's whole subject, and it went through two shapes in
a day. A block that wrote its follow-on work down honestly and had nobody pick it up fits none of the
other words. `Recorded.` lies about intent and `Refused.` lies about the decision, so a lane forced
to choose writes the more comfortable one. Three lanes that would not lie left the item out instead,
which is the burial arriving through the gate. It was first spelled `Unclaimed.` and took only
prose, because a lane could not mint a milestone number and had nothing to point at. That
constraint went the same day, and the disposition now resolves to a file somebody wrote.

`Done.` came from the same afternoon and four lanes, for work a block named that two ordinary
commits then finished. Each of those lanes resolved it by leaving the item out, which is the silence
this gate exists to stop, arriving through the gate itself.
