# Project metrics: what moved, week by week

*Name: `notes/project-metrics.md` is ratified (calef, 2026-09-02). He proposed it as
`design/project-metrics.md` and moved it here on the argument that `design/` holds arguments
(`design/fatal-risks/README.md`, the decisions, the roadmap) while `notes/` holds what was measured
(`benchmarks.md`, `mutation-testing.md`, `unsafe-obligations.md`). `script/metrics` and the
directory `notes/project-metrics/` are **provisional**; naming is an architect's, and a lane ships a
provisional name and says so.*

One row per ISO week, computed by `script/metrics` from git history. The data is one CSV per
measure in [`notes/project-metrics/`](project-metrics/), versioned with the code it describes.

**This page is a deck** (calef, 2026-09-24). A chart is a heading, an image, and at most a line or
two saying what it plots and what would otherwise be misread. Everything longer, the definitions,
the arguments behind a measure, the deadlines, the reconciliations and the dated analyses, lives in
[`notes/register-of-measures.md`](register-of-measures.md), whose *The weekly series* table routes
each chart to the appendix that argues it. That register is the half of the pair that does not change, and a caveat that cannot survive one
line belongs there, not in a caption.

## Read this before you read a number

- Every row is a restatement, not a report. The series applies *today's* definitions to old
  commits, which is right for a trend and is not what anybody believed at the time.
- Every figure here is derived by an agent and none of it is externally audited. Where a number
  has been checked two different ways, the register says so and names the method.
- A missing bar means the record did not exist, not that the number was zero.
- Weeks are ISO weeks in UTC, spelled `2026W36` everywhere, and the series starts at 2026W29
  because that is where the first commit falls. There is no 2026W28.
- The charts show the last ten weeks; the CSV keeps every one.
- A milestone is not a fixed unit and a pull request is not a unit of value: read the shape.

## The nine things that would kill nife

![Fatal risks by color](project-metrics/fatal-risks-colors.svg)

- 2026W41: risk 4 went green on 2026-10-05 UTC (calef); milestones 761 and 766 cut the null syscall's growth to four tasks from 48 to 10 ticks.

## unsafe blocks outside kernel/src/arch/

![unsafe density](project-metrics/unsafe.svg)

Blocks per 10,000 code lines outside `kernel/src/arch/`, `script/lint`'s gated census; the dashed
line is the ceiling. Only the ratio, not the count, is about soundness.

## The same unsafe blocks, by trust boundary

![unsafe blocks by trust boundary](project-metrics/unsafe-trust.svg)

The census split by what confines the code: kernel privilege, userspace confinement, both, and
the pre-kernel boot chain. Before 2026W38 the `unclassified` band is crates whose names have since
changed, so read those weeks as undercounts of both sides.

## Debt paydown

![Debt paydown share](project-metrics/debt-paydown.svg)

The share of each week's merged pull requests labeled `debt-paydown` ([what
counts](debt-paydown.md)). The label is chosen at the claim and judged from titles, so a lane that
became a repair is missed.

## Milestones built each week

![Milestones built each week](project-metrics/velocity.svg)

A flow: milestones whose `Built:` date falls in that week, read from today's tree for every week.
It does not reconcile with the `Built` stock (BUGS).
The line is the trailing ten-week mean, drawn rather than stored; it is dashed where fewer than ten
weeks exist behind it.

## Pull requests merged each week

![Pull requests merged each week](project-metrics/pull-requests.svg)

The second flow, counted from `main`'s merge subjects rather than the API, so it backfills to the first commit. 2026W29 and 2026W30 are zeros: the practice starts in
2026W31.
The line is the same trailing mean.

## Merge queue ejections

![Merge queue ejections](project-metrics/merge-queue.svg)

Queue began 2026-08-15; W31 and W32 predate it.

## How long a merge-group build takes

![How long a merge-group build takes](project-metrics/ci-jobs.svg)

<!-- week-notes: script/metrics writes this -->
- 2026W41: prove shards rebalanced on 2026-10-07 UTC by a 40-run median and split three ways (calef, #1802), planned at 15.7 minutes each from 20.5 and 27
- 2026W41: CI job test ran at 89% of its budget on its newest merge-group run; tracking issue #1930
- 2026W42: re-falsify replays two records at a time (#1979); full-scope shards fell from 16.5 to 8.8 minutes
- 2026W42: CI job cpu-matrix-shard ran at 86% of its budget on its newest merge-group run; tracking issue #2055
<!-- /week-notes -->

![Where a merge-group build's time goes](project-metrics/ci-critical-path.svg)

A build is a merge commit's `ci.yml` and `verify.yml` runs (same `head_sha`), first job start to
last job end, green only, from 2026-08-15 (`helpers/ci_job_times.py`). The timeline is the newest full
week.

## Which model wrote it

![Commits each week, by the model that signed them](project-metrics/models-commits.svg)

From the `Co-Authored-By` trailer. Attributed plus unattributed plus merge is every commit that
week; a merge is a pull request landing, not writing. 2026W29 predates
the convention, so it is absent rather than zero.

Only Claude Code writes the trailer, so the table counts one vendor. From 2026-09-29 02:33
to 2026-10-01 05:07 UTC the work ran on z.ai's glm-5.3 under opencode, and its 154 commits read
here as unattributed rather than as a model
([the correction](coes/2026-10-03-the-merge-rate.md)). `script/effort` has the same gap and its
charts show that week's z.ai work as nothing at all.

"Other models" is a trailer the script does not recognize.

## Lines touched, by the model that signed them

![Lines touched each week, by the model that signed them](project-metrics/models-lines.svg)

Added plus removed, not net; volume, not effort. Merges count no lines.

## What a turn costs

![What a turn carried, in thousands of tokens of context](project-metrics/context-per-turn.svg)

Mean prompt per request, from session records that are not in git. A turn costs the size of its context: cache reads are about 98% of every token spent. 2026W29 through 2026W33 are absent, not zero.

## Architecture decisions by status

![Architecture decisions by status](project-metrics/decisions.svg)

From `design/decisions/`. The gray band in the first three weeks is decisions that predate
a status existing at all, counted as having none rather than as `DECIDED`.

## Names by what the tree records about them

![Names by what the tree records about them](project-metrics/names.svg)

What each name's own provenance block says, for every name `script/names` tracks: crates,
programs, packages, scripts, every module (each `.rs` file under a `src/` directory), every
documentation directory and document, and the items that carry a `Name:` block. A name a rule covers
counts as ratified. `Ruled, rename owed` is a provisional name calef has ruled whose rename has not
landed. `No provenance block` is named things with no readable block. Everything but items is counted from
the files, so writing a block moves a name out of that band without changing the bar's height;
an item is counted only where its block exists. The current week's total is checked against
`script/names`. A week is a bar, so a day's rulings show as the current week's bar moving. A rising
`Provisional` band is not debt; the band worth an alarm is `Unrecorded`.

## Milestones by status

![Milestones by status, and proposals waiting for a number](project-metrics/milestones.svg)

From the milestone blocks in `design/roadmap/`, with unnumbered proposals stacked on top as the
eighth series: work identified and not yet entered into the roadmap, which unlike a provisional name
is closer to debt. The two zero weeks are a restatement artifact; watch `NOT-STARTED`, which grows faster than lanes
drain it.

## Rust in the tree

![Rust in the tree, in thousands of lines](project-metrics/lines.svg)

<!-- week-notes: script/metrics writes this -->
- 2026W40: the kernel code series fell 46,665 to 33,192 lines because milestone 609 (the system tests leave the kernel crate), pull request #1404, moved 66 test-only files unchanged into the system_tests crate; no code was deleted
<!-- /week-notes -->

Every tracked `.rs` file outside `vendor/`, with `kernel/src` split from the rest, in thousands of lines (volume, not effort). A line with code and a trailing comment counts as code.

## BUGS sections

![BUGS sections](project-metrics/bugs.svg)

Markdown headings plus Rust doc comments. Rising is good: a `BUGS` section is a limitation written next to the feature it limits.

## Coverage

![Coverage](project-metrics/coverage.svg)

Line coverage, each week measured by its own tree's `script/coverage` on its own pinned nightly, so
this is the one series on the page that is not a restatement. Weeks 2026W30 to 2026W38 were
backfilled that way on 2026-09-19. 2026W29 is empty because no instrument existed yet; the dashed line is the per-file floor `script/coverage` gates on, drawn for scale.

## The lowest-covered file

![Minimum per-file coverage](project-metrics/coverage-floor.svg)

The minimum per-file coverage, which is the number `script/coverage` actually gates on at 80%.
The aggregate can hold steady while one file slides, so this panel predicts a failing build.
2026W31 to 2026W38 were backfilled on 2026-09-24 from each week's own lcov; 2026W30 is empty
because the floor did not exist yet. The provenance is in the register's
[proofs and coverage](register-of-measures/code-proofs-and-coverage.md) appendix.

## Homes

![Packages](project-metrics/homes-packages.svg)

![Files](project-metrics/homes-paths.svg)

## Benchmark drift since a fixed anchor

![Benchmark rows more than 5% above their anchor](project-metrics/baseline-drift.svg)

The tripwire compares each save with the last, so steps under 10% add up unseen. This counts rows
past 5% since fixed anchors (calef, 2026-09-26). Every row and reason: [the drift
report](project-metrics/baseline-drift.md).

<!-- baseline-drift: script/metrics writes this -->
2026W42: aarch64 `spawn_reap` +33.14%, riscv64 `spawn_reap` +28.32%, x86_64 `spawn_reap` +10.59%; 19 rows past 5%.
<!-- /baseline-drift -->

- 2026W39: milestone 447 (a thread's vector registers are its own), #1015, 2026-09-21, added 1 to 3% to switch and IPC rows, taking rows past 5% from 2 to 7 (aarch64), 3 to 6 (riscv64), 1 to 2 (x86_64). Nightly bump #1112 added less.

## Interface stability

![Breaking changes to the interface](project-metrics/interface-stability.svg)

The boundary a program outside the tree is written against, each week: syscall surface moves
and wire-format value changes. [definitions](interface-stability.md),
[per crate](project-metrics/interface-stability.md).

<!-- interface-stability: script/metrics writes this -->
interface, 2026W39 to 2026W42: 10 breaking changes (49 additions); 5 syscall numbers changed, 0 format bumps; 74% co-change.
<!-- /interface-stability -->

## The crates 857 releases

![The crates 857 releases, by how far each has got](project-metrics/crate-release.svg)

![Properties the crate milestones require, proven and owed](project-metrics/crate-proofs.svg)

Milestones 813 to 821; all nine crates start in the tree. Both charts read git alone
(`helpers/crate_release.py`).

## Kani proof harnesses, and what can falsify them

![Kani proof harnesses](project-metrics/harnesses.svg)

- 2026W41: unfalsified fell 56 to 0 across #1697, #1701 and #1713; the middle band is new (§134 (a harness carries a machine-replayable falsification record, or it is not evidence), amended 2026-10-06 UTC). Its two harnesses were examined and no defect in today's code can turn them red; each carries its reason.

A harness with no falsification record at all counts as unfalsified. The denominator is harnesses, not
code: unharnessed code is outside it ([what each ratio is of](denominators.md#the-published-ratios-and-what-each-is-of)).

## Action items the corrections of error still owe

![Action items the corrections of error still owe](project-metrics/coe-actions.svg)

<!-- week-notes: script/metrics writes this -->
- 2026W40: open items fell 3 to 0 on 2026-10-04 UTC. Milestone 641 (a mechanism that reports clean), 725 (count every vendor) and 640 (a gate is not evidence) turned BUILT.
<!-- /week-notes -->

Open while what an item names is not done; the direction is down (`script/roadmap --coe-actions`).
Each week is read from its own commit, so 2026W39 is the first correction, already in this form.

## Writing standards

![Documents over a writing-standards limit](project-metrics/writing-standards.svg)

Documents over each writing-standards limit; exempted ones count
([definitions](register-of-measures/ratchet-debt.md)).

## Comment blocks

![Files with a long comment block](project-metrics/comment-blocks.svg)

![Comment lines past the cap](project-metrics/comment-blocks-excess.svg)

The 40-line comment-block cap: files holding a longer block, then the lines past it.

## Line-number citations

![Line-number citations](project-metrics/line-citations.svg)

Citations by line number, which §277 (cite a heading or a symbol, never a line number) retires. The
line is the code ceiling. Markdown is held at zero, so what remains there carries an exception marker.

## How it stays current

`.github/workflows/metrics.yml` runs `script/metrics --update` daily and opens or refreshes a pull
request if anything changed. Two columns need a build: coverage, taken on Mondays (UTC), and the
interface series, from `helpers/interface_stability.py`. The cost columns come from session records on one laptop, never a workflow. [How the series stays
current](register-of-measures/reading-the-weekly-series.md#how-the-series-stays-current) has the
rest.

## BUGS

- `script/metrics --check` is deliberately not in `script/lint`. Any commit changes `HEAD`, and
  the current week's row records the commit it was taken at, so a gate on it would fail every pull
  request that touched anything. The workflow is the mechanism; this is rung two of `AGENTS.md`'s ladder declining to be rung one,
  said out loud.
- A newly added column can go blank across history if a CSV merge conflict is resolved with
  `--update`, which touches only the current week and any week missing outright, instead of
  `--backfill`. A merge of two branches' concurrently-added columns into one header therefore
  needs a `--backfill` afterward, or the older rows carry empty cells (the register's *The weekly
  series* has the one case). Empty, not zero, is the tell: run `git diff` on `notes/project-metrics/` after merging two metrics
  branches.
- `milestones_built_this_week` will not equal the week-on-week change in the `Built` stock, in
  any week. The gap is the lag between finishing a milestone and flipping its row, and the register
  argues it; nothing gates the two against each other and nothing can.
- A milestone dated in a week the series has no row for is dropped, which is milestone 1 (boot
  to Rust on QEMU `virt`, and print to the PL011 UART) and 2026W28. `script/metrics` prints a stderr
  line naming it since 2026-09-23; before that it was silent. Summing the chart gives one fewer than the tree's dated blocks.
- Two of the three definitions are shared, and the third is checked (milestone 236 (three derivations are
  copied between scripts), 2026-09-03). The `unsafe` census and the comment-and-literal strip the code and comment line split
  is built on live in `helpers/rust_source.py`, which `script/lint` and this script both import, so
  there is one definition and nothing left to drift. The harness count could not be collapsed the
  same way: `script/lint` and `script/falsifications` attribute each harness to a workspace package
  out of `cargo metadata`, and this script reads blobs at revisions nobody has checked out and
  cannot run cargo against them. `script/lint` runs all three derivations and fails on a disagreement, the weaker answer.
- A line inside a multi-line string literal counts as a comment line: wrong in principle,
  negligible here.
- The charts follow the reader's operating system color preference, not GitHub's theme toggle.
  GitHub serves the SVG as an `<img>`, so a media query inside it cannot see the page theme.
- `patches/` is outside the `unsafe` census, inherited from `script/lint` along with its reason.
  That code does run on the machine, so it is a real hole rather than a boundary, and
  `notes/register-of-measures.md` records the blocks it leaves uncounted.
- `names_total` is not the sum of the naming columns, which is the one place a column here
  breaks the pattern the milestone and decision columns set. It is every named thing in that week's
  tree; the four statuses count the ones whose header carries a block that parses. The gap is drawn as a band, explained in the register, and not folded into `Unrecorded` because
  silence is not a claim.
- The naming columns count signatures, not names. A name can be ratified and bad, or provisional
  and perfect. Nothing here reads a name, and `script/names`' own `BUGS` is the longer version: it
  cannot check that a recorded reason is still true, that a date is right, or that a `recorded`
  citation leads anywhere.
- Items are counted by their blocks, not by the code. Every other kind is enumerated from the
  tree, so its count moves only when a file is added or removed. An item counts only where a
  `/// Name:` block exists, because "item" has no definition yet that a tree can be enumerated by:
  2026W42 has 4,798 `pub` items and 331 item blocks. An item block written on old code still
  raises the bar. `helpers/` helpers are outside `script/names`' surfaces, and design/naming.md's
  `BUGS` carries what that leaves uncovered.
- `proposals_unnumbered` is a net count and cannot see the flow. Five proposals have left the
  directory and 81 remain; a flat line fits both a stalled pile and one draining as fast as it fills. The measurement that would tell them apart is the age of the oldest,
  which `script/roadmap --check` prints on every lint run and this column does not carry.
- `merged_pull_requests` can only see GitHub's default merge subject. A merge made any other way
  is not counted and cannot be distinguished from an ordinary merge commit afterwards. The total matched a maintainer's hand count on 2026-09-21, evidence and not proof.
- A band that goes to zero across the whole chart window changes the colors of the bands after
  it. `series_of` drops an all-zero series and the palette is indexed over what survives, so when
  a band leaves the ten-week window every band after it shifts one hue, as the by-model legend's did
  when 2026W30 left. The legend is redrawn with it, so nothing is mislabelled, but a color moves between screenshots.
- The cost and context columns carry seven limitations of their own, from a capture that can
  stop silently to a price [ledger](project-metrics/ledger.md) that re-prices history. They are listed where those columns are
  argued: [what this project costs, *Known limitations*](register-of-measures/project-cost.md#known-limitations).
- Four series were restated on 2026-09-24, and an older screenshot will disagree with each.
  Opus 5.5 got its own column (`opus_5_5`), so this week's commits left *other models*, and Opus 4.8
  now draws inside that band because the palette has eight hues. The fatal-risks chart (since removed in favor of the colors chart, which now stores the total itself) gained
  `fatal_risks_total` and an *unclassified* band, so 2026W36 to 2026W38 read nine rather than five,
  four and five. The coverage floor was backfilled from each week's own tree.
- Nothing here is audited outside this project, as stated at the top.
