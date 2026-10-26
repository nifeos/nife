# Which model wrote it

*An appendix to [the register of measures](../register-of-measures.md). The name is provisional.*

## What it counts

This is the third weekly flow in the deck, and it answers a question the first two cannot.
Velocity says how many milestones landed. The pull-request series says how much landed at all.
This one says who wrote it. calef asked for it on 2026-09-24. Both of those series are in
[landed each week](landed-each-week.md). Like them, every column here is a per-week event, unlike
every stock in the register.

### Why the trailer, and not the richer source that was right there

The agent harness keeps session records with tokens per model. That is a better measure of effort
than anything in this series. calef rejected them as its spine on 2026-09-24, for structural
reasons rather than taste:

- They are not in git. They live under `~/.claude/projects/` on one laptop, and nothing promises to
  keep them. `script/effort`'s own header already records 2026W29 through 2026W33 as
  unrecoverable: five weeks of this project gone.
- They cannot backfill. Git can, and this series reaches the first commit because of it.
- They see one machine. `Claude Fable 5` signs 360 commits in this repository and appears in no
  session record on the machine those records live on.
- `script/metrics` may not read anything but git, by the rule at the top of the script, because
  `.github/workflows/metrics.yml` runs it on a GitHub runner where none of that exists. A
  trailer-based series can live in `script/metrics`; a token-based one cannot. That is why the cost
  columns are captured by a person on patagonia, and have the gaps to show for it.

So this series is cheaper and coarser, and it will still be here in a year. The richer measurement
is [project cost](project-cost.md). Its subsection "The deadline, and what was already gone when
the capture started" is the argument for this one.

### Four buckets, and the sum is checked

Every commit in the repository falls in exactly one of four. `script/metrics` fails loudly if they
do not add up to the week's whole commit count.

- **attributed**, carrying a `Co-Authored-By` trailer, counted per model.
- **unattributed**, authored with no trailer.
- **merge**, created by the platform rather than by anyone.
- **claim**, an empty commit: its tree is its parent's tree.

A sum that must come out right is a denominator that cannot leak. Normalization might drop a model,
a new model name might be swallowed, or a classification rule might be wrong. In each case the
total stops matching and the script stops. Every other miscount this tree has written up was a
count over a set nobody could see. The status token was worth seven invisible milestones for five
days. The naming surface answered confidently about a kind it did not walk. This count is auditable
by arithmetic, by anyone, from the CSV.

A merge commit is a pull request landing, which is worth counting on its own terms. The merge
queue creates the merge commits and `allow_squash_merge` is `false` on this repository, so the
merge bucket is a throughput count. 95 merge commits do carry a trailer, from an agent resolving a
conflict. The merge bucket still wins, because what the commit records is a landing, not a piece of
writing.

A claim is the empty commit a lane makes when it opens its draft, under §90 (the claim is a draft
pull request). It carries no work, so `script/metrics` keeps it out of every model's count whatever
its trailer says. `script/claim` signs it `Claude Code` unless told otherwise, and some lanes sign
it with their model, so the test is the empty tree and not the name. The few CI re-triggers, which
are also empty, count here too.

Merges carry a commit count and no line count. A merge's diff against its first parent is the whole
of what landed on the branch. Every line of it is already counted on that branch's own commits, so
adding it would roughly double the tree.

### Normalization collapses a variant, never a version

`Claude Opus 5 (1M context)` and `Claude Opus 5` are one model reached through two context windows.
So `script/metrics` strips the trailing parenthetical and they share a column. `Claude Fable 5.1`
keeps its own column, because a version is a different model. Folding it into `Claude Fable 5` would
be the silent merge the table exists to prevent.

A trailer value the table has never seen is counted in `other_models` and printed by name on every
run. It is not dropped, and it does not quietly become an existing model. `script/metrics`' own
`MODEL_TRAILERS` records every spelling this repository has carried. `Claude Code` is the tool
rather than a model, and `OpenCode (GLM)` names a harness and a vendor with no version. Both are in
the table as decisions, filed under `other_models`, so they do not warn forever. No record in the
tree says which GLM model the `OpenCode (GLM)` commits ran, and guessing one would be the silent
merge this table refuses.

A new model's name goes into `other_models`, and the script names it on stderr until it has a row,
which is the rule above working: nothing is lost, and the week's sum still balances.
`Claude Opus 5.5` is a version, not a context window, so it has its own column, `opus_5_5`, rather
than joining `opus_5`. `GLM-5.3-Flash` is a second spelling of `GLM 5.3 Flash` and shares its
column.

### Eight hues, more than eight bands

The chart palette has eight fixed slots, and a ninth series folds into another rather than
inventing a color. Opus 5.5 made the commits chart nine. It took Opus 4.8's slot, so no other band
changed color, and Opus 4.8 now draws inside **other models** together with `fable_5_1` and
`other_models`. Both charts fold the same set, so one hue names one model on either. The CSV keeps
every column apart; the fold is drawing only, and the arithmetic is checked against the file.

`script/metrics` counts a commit carrying two trailers once, under the first, and names it on
stderr. Counting it twice would break the sum check. Splitting it in half would put a fraction in a
column of counts. No commit in this repository carries two, measured the same day, so the rule has
never fired. The warning is what would tell us it started to.

### Unattributed hides two different facts, and only one of them is a gap

A commit made before the first trailer ever appeared could not have carried one. A commit made
after it and carrying none is a hole in the record. They are separate columns:
`model_commits_pre_convention` and `model_commits_unattributed`.

The convention starts 2026-07-23 UTC, at `16a5a5c97e40`, signed `Claude Opus 4.8`, ten days after
the first commit. 79 non-merge commits precede it, 61 of them in 2026W29 and 18 in 2026W30. So
2026W29 is absent rather than zero in every model column. That is the rule milestone 519 (what
this project costs) follows for its cost columns. `notes/project-metrics/models.csv` carries empty
cells there rather than noughts. The week has already left the ten-week chart window, so the marker
is a fact about the file today, not something a reader will see drawn.

The post-convention gap is real and it moves. 832 commits since the convention began carry no
trailer, and they are not spread evenly: 2026W34 has 295 of them against 16 in 2026W37. This
column is the honest denominator under every attributed share, and it is on the chart for that
reason. A reader who cannot see the unattributed band will read the attributed shares as the whole
picture.

### Lines are volume, not effort

![Lines touched each week, by the model that signed them](../project-metrics/models-lines.svg)

The chart counts added plus removed: the churn, not the net. Net is the wrong quantity twice over.
A commit that only deletes reads negative. A careful mechanical sweep can net out near nothing
while being the largest diff of the week. The tree's net size already has a series of its own,
"Rust in the tree" in [code, proofs and coverage](code-proofs-and-coverage.md). That one is a
stock; this is a flow, and what flows is how much text was touched.

The ranking lines produce is backwards, and this project's own history showed it one day before
the chart was built. On 2026-09-23, pull request #1168 was the hardest work of the night. It
diagnosed a silently dropped table cell and separated the renderer from the note. It found that
the obvious fix broke `mdr`'s memory grant on x86_64 only, and it deliberately falsified the gate.
It changed about a dozen lines. Pull request #1171 changed 246 files in a careful mechanical sweep.
`AGENTS.md` says the same of its own line count, and the posture carries here: **take the number as
a scale, never as a claim about correctness.**

Commits are the better unit of the two, because of a property of this repository. `AGENTS.md`
requires one purpose per commit, and `allow_squash_merge` is `false`. So a commit here is an
enforced unit of intent rather than an arbitrary chunk of diff. The lane-hygiene caveat in
[landed each week](landed-each-week.md) ("Commits are charted") still applies to commits. It
applies to the line chart as well, and harder.

### What this series cannot tell you

It attributes a commit to the model named in its trailer, and nothing checks that. The harness
writes the trailer, so it is good evidence, but it is not a measurement. Work done by one model
and committed in a session running another is attributed to the second, and nothing here can see
that.

It says nothing about difficulty, and the models were not given interchangeable work. A week where
one model's band dominates may mean it did most of the work. It may instead mean the maintainer
pointed it at most of the lanes. Lane assignment is a person's choice, and it is not in git.
