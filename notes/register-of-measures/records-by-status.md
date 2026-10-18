# Fatal risks, decisions, names, proposals and milestones by status

*An appendix to [the register of measures](../register-of-measures.md). The name is provisional.*

## The nine things that would kill nife

*Removed 2026-10-03 UTC: this status chart and `fatal-risks.csv`. `fatal_risks_total` now lives in `fatal-risks-colors.csv`.*

From `design/fatal-risks/README.md`, by Experiment status: the field calef ratified on 2026-09-23, with
three values and no fourth. `RUN` means the experiment has been performed. `NOT-RUN` means it has
not and could be. `CANNOT-RUN` means it cannot be performed at all. `script/fatal-risks` fails on any
other word, so this can be charted as an enumeration rather than read out of a sentence.

The field says whether an experiment happened. It never says what it found. That half of the
2026-09-23 proposal was refused on 2026-09-23 and commissioned on 2026-09-29; the appendix beside
this one argues it. `GREEN`, `AMBER`, `MEASURED` and `AUDITED` are in the file, in prose, beside
the argument that earns them; the colors chart counts the first two as words, and the rest stay
prose.

`CANNOT-RUN` is the one value that carries a judgment anyway, and it is the file's own. Risk 8
cannot be observed until milestone 198 (a package manager, and the trivial install that makes a
second customer possible) lands. **A fatal risk that cannot be tested is the most dangerous state a
fatal risk can be in.**

This replaced a tested/untested pair on 2026-09-23. The pair's refusal of a verdict column was right
when it was written. It said a color series would be "a script reading a sentence and guessing".
That was true: of four statuses, three ended in GREEN or AMBER and risk 7's in neither. What changed
is not the reading but the thing read. The pair also had a simpler problem. All nine entries carried
a status line by 2026-09-23, so it sat at nine and zero and told a reader nothing.

Weeks before the field existed are read through the words the file used then. So the early bars are
shorter than nine: a risk with no status line at all counts in none of the three. `MEASURED` and
`AUDITED` are read as `RUN` for those weeks, and `NOT YET`, `UNRUN` and `UNTESTED` as `NOT-RUN`,
`NOT-RUN` and `CANNOT-RUN`. None of those five words may be written today.

**The bars reach nine from 2026W36, because the chart counts every risk and not only the classified
ones.** Until 2026-09-24 it drew only the three status columns, so 2026W36 to 2026W38 read five,
four and five, and calef read that, fairly, as the number of risks: *"We've had 9 things that would
kill nife longer than just this week."* All nine were on `main` from pull request #588, which
landed 2026-08-31 UTC. `fatal_risks_total` now counts the risks in each week's file, and the gap
between it and the three columns is drawn as **unclassified**, the shape the naming chart already
uses for a name with no provenance block. The gap is arithmetic, not a stored column: it is not a
value `Experiment status` can hold. In those weeks it is risks 4, 5, 8 and 9, which had no status
line, and in 2026W37 risk 3 as well, whose line said `STALE`. That meant the old result had expired, which says nothing about which of
the three the risk was.

2026W35 and earlier are zero, and zero is right. The file was written on 2026-08-30 and its commit
carries that date, but a week is counted from `main` at the week's end, and `main` did not have it
until the merge early on 2026-08-31 UTC, which is 2026W36.

## Architecture decisions by status

From each decision's frontmatter in `design/decisions/`. The gray band in the first three weeks is the honest bucket. A
decision was then a `## N.` heading in one 5,320-line `DECISIONS.md`, and nothing said whether it
still held. Milestone 114 (split `DECISIONS.md`, and give a decision a status) is where a status
exists at all. Counting those early decisions as `DECIDED` would invent a claim the record never
carried, so they are counted as having no status.

There were 139 decisions in eight weeks, of which 17 are `AMENDED` and 3 `SUPERSEDED` at 2026W36.
Twenty revised or replaced out of 139 says the vocabulary is doing work. A tree where nothing was
ever amended would mean either that every first answer was right or that nobody went back.

`PROPOSED` is the queue waiting on an architect, and it stays small (10, 4, 8, 2, 3). It is a queue
depth rather than a backlog.

## Names by what the tree records about them

From the provenance block in each named thing's own header, for every name `script/names` tracks.
Milestone 115 (the names that were ratified, and the ones that were refused) put the block there: a
crate's `src/lib.rs`, a program's, a `script/` entry point's comment, a Cargo package's manifest.
A module's block is its `//!` header, an item's a `/// Name:` paragraph, and a documentation
directory's and its documents' the directory's README. A name a rule on
`design/naming/convention-rulings.md` covers counts as `ratified`.

**The total counts files, not blocks.** Every `.rs` file under a `src/` directory is a module, and
every document under `design/`, `notes/` or `briefs/` counts, block or no block. Writing a block
moves a name between bands; only adding or removing a file moves the total.
`helpers/module_block_ratchet.py` holds the unblocked ones to a list that only shrinks. Items are
the exception: one counts only where its block exists, until "item" has a definition.

`script/names` and this series share `helpers/name_census.py`, and `script/metrics` fails when the
current week's totals disagree.

The four words are what a block *says*:

- `Ratified` is calef ruling, with a date and what was refused.
- `Recorded` is the tree arguing the name somewhere and nobody ever putting it to him.
- `Provisional` is whoever coined it saying out loud that they expect it to change.
- `Unrecorded` is nothing outside the block saying why the name is what it is.

`Ruled, rename owed` is a `provisional` block saying calef has ruled and the rename has not landed
(`helpers/name_ruled.py`). It is no longer his question, so it is drawn apart.

The total is every named thing, and the statuses do not have to add up to it. `No provenance block` is the
difference: named things with no block, or a block that fails `script/names`' form checks. That
differs from `Unrecorded`, which is a block saying the history is silent. The two are kept apart for
the same reason the decisions chart above will not read a statusless decision as `DECIDED`.

In the first three weeks that band is the whole bar: 83 named things at 2026W29 and 300 at
2026W31, none carrying provenance, because the convention that records it did not exist until
2026-08-04. The first blue bar is a convention arriving, not 72 names ratified in a week. Backfilling them was
refused: an invented ratification would put a false claim in the record of who claimed what. The
band stays large until 2026W39, when module, directory and document blocks were first written.

The three steepest weeks are files being made. 2026W39 adds 537 names: 245 documents and 29
directories from the §212 (a prose budget) splits, which turned long documents
into directories of pages, and 193 item blocks. 2026W41 adds 699, 345 of them documents from more
splits and 165 modules. 2026W42 adds 579, 415 of them modules from the §266 (a Rust source file
stays under 2,000 lines) splits.

2026W36 is one milestone doing one thing. `Unrecorded` goes from 64 to 2, `Recorded` from 9 to 37
and `Provisional` from 26 to 66. That is milestone 264 (sixty names git history cannot justify, and
the research that would let calef rule on them). It gave every name nobody had written a reason for
one that says something. The bar is the height it would have been without it. What changed is what
the tree can say about the names in it.

### A rising `Provisional` band is not debt

Nothing in this tree fails because a name is unratified. `AGENTS.md`:

> `script/names --unratified` is a worklist rather than a wall precisely so that an unratified name
> never blocks anyone's build.

`script/names --check` gates on a block being *present*, never on it saying `ratified`. That is
deliberate: a gate that demanded the queue be drained would block every unrelated merge behind a
review nobody can hurry.

A provisional name is what `AGENTS.md` tells a lane to ship when it needs a name and the decision is
calef's. It converts an expensive decision into a cheap one by refusing to pretend it is settled. A
lane that coins a name, argues it, records what it refused and marks it provisional has done what the
convention asks. A lane that quietly ships a name without saying it is unsigned has not. That lane
will not show up on this chart at all, which is the limit of what a count of signatures can see.

So the `Provisional` band going up means lanes are naming things and being honest that nobody ruled. It is a
queue depth against one person's attention, the project's scarcest resource. A growing queue says
the tree is growing faster than one reviewer rules on it. It says nothing about the names being
wrong. The band worth an alarm is `Unrecorded`, a name nobody anywhere argued for, and this chart
has it in single digits.

## Unnumbered proposals

74 at 2026W36, and zero in every week before it: the `proposals_unnumbered` column in the CSV.
The counts in this section were each taken on a different day, and each says which. The pile grew
to 83 at 2026W37 and fell to 2 at 2026W38 when most of it was promoted to numbers, so no two of
them should be read against each other without their dates.
`design/roadmap/proposals/` was created on 2026-09-04 (UTC) by milestone 247 (follow-on work named by a
finished milestone goes nowhere, and this is the third time).

Since 2026-09-19 they are drawn on top of the milestones chart, as the eighth series. The column
had been collected every week and drawn nowhere before that. The bar totals on that chart now include them. On 2026-09-18, mid-week, 2026W38 read
431: 324 numbered milestones and 107 proposals. The finished week reads 515 and 2, after the
promotions. The jump at 2026W36 is the pile appearing when the directory
did, not a burst of milestones. They sit on top because they are work that has not entered the
roadmap yet. A new slot is appended so that no existing series changes color.

Nothing else in the deck could count these, which is why the column exists. The milestones chart
keys on a milestone number. It reads the blocks in `design/roadmap/` for a revision that has no
index, and the index rows for one that does. (A week counted before a stale index was regenerated
undercounts by however many milestones landed in between. It self-corrects on the next
regeneration, and past weeks are read from their own revisions.) A proposal is *defined* by not
having a number. A lane that finds work it is not doing writes `design/roadmap/proposals/<slug>.md`.
Concurrent lanes collide over the number, not the authority, so an integrator assigns the number at
promotion. The pile was invisible to every column in the deck by construction, not by oversight.

### What a rising line means here

Do not carry the naming reading across. A provisional name costs nothing while it sits, because
nothing is waiting on it. An identified piece of work nobody has scheduled is a different object:
something was found wrong or missing, and the finding is parked. That is closer to debt. But the
count alone cannot tell you whether the pile is stalling, for two reasons.

It is a net count, and the flow is gross. On 2026-09-11, five proposals had left the directory
since it existed, so 86 had been written and 81 remained. They left in two ways. One was promoted to a number the
ordinary way: milestone 256 (x86_64 places PCI BARs in a hardcoded window, and on xenon that window
is RAM). The others were done by a lane that picked the file up and fixed the thing. Sometimes that
lane filed a narrower proposal in its place: `the-tcb-capability-that-outlives-start` became a fix
plus `the-region-half-of-the-retention-declaration`. A flat line would fit a stalled pile and one
draining as fast as it fills, and nothing here tells them apart.

Age is the tell, and age is not in this column. `script/roadmap`'s own header says so. A gate on age
("no proposal older than N days") would be routed around by not writing proposals, which is worse.
So it prints the count and the date of the oldest on every `script/lint` run. On 2026-09-11 the
oldest was dated 2026-09-03 and the directory was a week old, so nothing had had time to go stale.
The oldest predates the directory only by time zone: both are the commit of 2026-09-03 19:56
Pacific, which is 2026-09-04 in UTC. **The number to
watch is this one going up while the oldest date stops moving.** `script/roadmap --proposed` lists
them oldest first and is the view that answers it.

A rising line is still better than what it replaced. Without it this work lives in lane reports, which
are read once, by one person, on the day they are written. `AGENTS.md` records the cost.
Milestone 90 (a guard page under the per-CPU secondary stacks) exists only because calef happened to be at his
desk the day a report named it. Milestone 94 (the untracked-work sweep, and the convention that ends
the category) swept the tree for this category, then left its own inventory in a pull request body
for twelve days. A count of 74 visible proposals (2026W36) is worse than 74 scheduled milestones.
It is far better than 74 findings nobody can enumerate.

## Milestones by status

From the milestone blocks in `design/roadmap/`, and from the single `design/roadmap.md` in weeks
before the split. The retired README index is not read (calef, 2026-09-24); the blocks alone
reproduce every week but 2026W38, where the index lacked the row for milestone 518 (a census that cannot be attributed is a number
nobody can act on). The two zero weeks are a restatement artifact,
and the sharpest one in the deck. There really was a roadmap in 2026W30: `design/roadmap.md` landed
2026-07-22 with ten rows in it. It had no status column. A milestone's state was prose inside a
cell, phrased a dozen ways: `Built`, then `Built (frame scope)`, then `Built:` followed by a
paragraph on which half. The status vocabulary was later minted to fix exactly that. It is
unparseable now for the same reason it was unreadable then. The first bar this chart can draw is the
week the column exists.

Rows 1 to 11 were backfilled into the roadmap later by milestone 76 (split the roadmap:
`design/roadmap/README.md` as index, one file per milestone). Its index half was retired 2026-09-21,
and the one-file-per-milestone half is the roadmap today. Eleven milestones had been built before
the first bar; none of them is in it.

The interesting line is `NOT-STARTED`, not `BUILT`. Reading the rows as they stand at 2026W36, built
milestones went 26, 63, 73, 97, 103, 125 across the six weeks the roadmap has existed, a steady
rate. Not-started went 16, 35, 32, 43, 62, 79. **The roadmap is growing faster than the lanes drain
it**, and the gap widened most in the last two weeks. That is a project generating its own work. It
is the number to watch if the ranking function ever needs defending. A backlog that grows faster
than it is consumed is only healthy while something is choosing the order.

`REMOVED` appears for the first time in 2026W36, with two rows. The token was minted 2026-08-30,
when milestone 54 (a network file service a Mac can actually mount) was deleted and the six words
then available could only lie about it.

Two statuses were missing from this chart's vocabulary before 2026-09-20, and one had been missing
for five days without anyone noticing. `script/metrics` keys the count on a fixed list. A token it
does not hold is counted as nothing rather than as an error, so `milestones_total` went short by
exactly the blocks it could not see. `SUPERSEDED` was minted 2026-09-15 and never added. Seven
blocks were invisible, and on 2026-09-20 2026W38's total read 435 where the tree had 442. That is
before the 42 refusals below, and a different day from the 431 in the proposals section. Milestone 448 (a refusal
gets a number, a status, and a condition that would change it) added `SUPERSEDED` and `REFUSED`
together. It restated the history with `--backfill`, so the correction reaches the weeks already
written, not only the next one. A chart that undercounts silently looks exactly like a chart that is
right.

`REFUSED` will move every denominator in the deck, and it is not work appearing. Milestone 448
backfilled 42 blocks for refusals that name executable work. The roadmap went from 444 milestones
to 487, which is those 42 plus milestone 448's own block (its "before and after" table, 2026-09-20;
corrected 2026-09-24, when this sentence read as though 444 + 42 made 487). None of them is a backlog item. They are excluded from `script/roadmap --ready`, from the
gate classification and from every count that reads as outstanding. The ready count was 120 before
and 120 after. The numbers that move are the totals a stranger quotes, so they are recorded in the
register rather than left to be rediscovered as a cliff in a bar chart. When this was written on 2026-09-20, the column reached
this chart only when the index table was regenerated at merge, because the chart read index rows and a lane never edits that table. The index
was retired on 2026-09-21.
