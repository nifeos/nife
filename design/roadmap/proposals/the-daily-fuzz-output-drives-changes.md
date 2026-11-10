---
status: PROPOSED
raised: 2026-11-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The daily fuzz output drives changes

Raised by lane proposal/the-daily-fuzz-output-drives-changes, at the maintainer's request. The
question is what the daily fuzz report is for once it exists, beyond proving the loop is alive.

Reuse: the release reader in `helpers/fuzz-heartbeat.py`, the open, update and close of a labelled
issue in `helpers/ci_job_times.py sync`, and the hosted schedule of
`.github/workflows/fuzz-heartbeat.yml`. No new dependency.

## Background

The continuous fuzzing host is one x86_64 machine with KVM. It runs the two in-guest syscall
drivers all the time: the kernel-thread driver of milestone 752 (a seeded syscall driver with a
shadow model) and the confined driver of milestone 779 (fuzz the surface a confined process can
reach). Each window builds an image with a seed range baked in, boots one guest, sweeps seeds for 60
seconds and appends one row to `windows.tsv`. A row holds the driver, slot, first seed, seeds run,
milliseconds, status and commit.

Milestone 1026 (where the daily fuzz output goes) ruled that the host publishes one GitHub release a
day, tagged `fuzz-YYYY-MM-DD`. Its two assets are `report.md` and `clean-ranges.tsv`. The report
holds totals over the whole of `windows.tsv`: clean windows and seeds per driver, windows by status,
the newest window's time, and the count and age of held findings. `helpers/fuzz-daily-publish.sh`
builds both files. No release exists yet, because the App key and the publish switch are a person's
steps ([`finishing-the-setup.md`](../../../notes/fuzzing/finishing-the-setup.md)).

The only reader today is `helpers/fuzz-heartbeat.py`, from milestone 1023 (the fuzz loop
dead-man's switch). Each day it checks three things: a release exists for today or yesterday, its
report shows an `ok` window, and the infrastructure share is under 0.5. Nothing compares one week
with the last. Nothing turns a number into a proposal, a model change or a host spec.

Two facts narrow the question, and both were checked against the tree.

- The host runs no `cargo-fuzz` target. The 18 targets that `script/fuzz --list` names run only in
  the merge group's two `fuzz-shard` jobs, 60 seconds each, from the committed seeds, with no corpus
  kept between runs. The daily release says nothing about them.
- The in-guest drivers are coverage-blind ([the driver note](../../../notes/fuzzing/the-syscall-driver.md)).
  Every seed is an independent draw, so a seed corpus cannot grow in the libFuzzer sense. The only
  corpus is the clean ranges. Coverage counters were ruled on 2026-10-31 (UTC) in milestone 1003
  (continuous fuzzing for risk 7's criterion (d)), and milestone 1008 (a fuzz window reports what it
  reached) set their format: two integers per window. Neither is built.

Findings take a separate, private path (milestones 1013, 1014, 1015 and 1020, the fix-lane token,
the advisory reader, the findings rulings and the filer). This proposal does not touch it.

## Prior art, as read

Read on 2026-11-10 (UTC). Anything recalled and not read says so.

- ClusterFuzz, "Analyzing fuzzer performance" (google.github.io/clusterfuzz). A fuzzer statistics
  page groups by day, by fuzzer or by time, with up to 24 hours of lag. The page says "code coverage
  should grow over time", treats frequent timeouts and out-of-memory errors as blockers, and says a
  target that has found nothing in a while deserves a look. Coverage report links sit on that page.
- OSS-Fuzz, "ClusterFuzz" further reading. The statistics dashboard shows speed, coverage and memory
  per target, and the coverage reports highlight the source a target reaches.
- ClusterFuzz glossary. Corpus pruning "removes unnecessary inputs while maintaining the same code
  coverage". ClusterFuzzLite's GitHub Actions page says pruning must run whenever batch fuzzing runs,
  and its example schedule prunes and builds coverage once a day.
- syzbot's dashboard (syzkaller.appspot.com/upstream). One row per fuzzing instance: last active,
  uptime, corpus, coverage, crashes, executions, and the kernel build's commit and freshness.
  syzbot's `docs/syzbot.md` closes a bug only when its fix commit reaches every tracked build. From
  memory, not found in the pages read: syzbot also closes bugs that stop recurring as obsolete.
- FuzzBench's report reference. It plots coverage growth over time with a 95% confidence band,
  ranks fuzzers by median reached coverage, and tests differences with the Mann-Whitney U test and
  the Vargha-Delaney A12 effect size, over many trials.

Where nife differs. The services above serve dashboards built from a full corpus and coverage maps.
nife holds its findings, so what it publishes is one text file of totals a day, with no map. And for
risk 7, milestone 1003 reads a coverage plateau as condition 2 met. In OSS-Fuzz a plateau is a
prompt to look for a blocker. Both readings apply here: a plateau at a small share of the kernel
says the driver's model leaves calls out.

## What the numbers should drive

Each candidate decision, with what can drive it today.

| decision | read from | can drive it today |
|---|---|---|
| A driver made no clean progress in a week | clean seeds per driver, week on week | yes |
| The infrastructure share is rising | windows by status, week on week | yes |
| A driver's coverage stopped growing | edges hit over edges total, per driver | after milestone 1008 |
| The host stopped following main | the newest clean window's commit, against main | after one report line |
| A crate with `unsafe` has no target | a tree census, not fuzz output | not from this output |
| A `cargo-fuzz` target is worth retiring | the merge group's fuzz shard logs | not from this output |

Each row that fires leads to one of four actions, each with an owner. A host problem (progress
stopped, infrastructure share rising, main not followed) becomes a written spec the architect hands
to the homelab agent, as the heartbeat's issue already says. A coverage plateau becomes either a
dated line that condition 2 of risk 7 holds, or a proposal extending the driver's model, whose
unmodelled calls the driver note lists (`REAP`, `SURVEY`, `MemoryRegion` and the rest). A maintainer
lane makes that call, not the reader.

The last two rows need no daily release. A rough census over `crates/` (a grep for `unsafe fn`,
`unsafe {` and `unsafe impl`) finds 26 crates with `unsafe` and a `cargo-fuzz` target reaching 2 of
them. Most of the 26 are kernel internals, such as `paging` and `inter_process_communication`, that
proofs and the in-guest drivers reach instead. So the count is not a gap list. The weekly metrics
already chart `unsafe` by crate and trust boundary, which is the census's home. Retiring a target
reads per-target executions from the fuzz shard logs, where `script/fuzz` prints libFuzzer's final
statistics and nothing reads them. The merge group's `fuzz` job ran at 92% of its budget in 2026W45
(issue #3081), so a retired target saves about a minute of shard time on every merge. Part 3 below carries that reader.

## What the report must carry that it does not

Three additions, each computed over clean windows and none needing a held input.

1. Seeds per second per driver, from the seeds run and milliseconds columns. A fall says the host is
   slower, which the seed totals hide when windows simply take longer.
2. The commit of the newest clean window. Beside main's history it shows the host's freshness, as
   syzbot's dashboard shows its kernel build's freshness.
3. Edges hit and edges total per driver, exactly as milestone 1008 specifies, once the counters
   exist.

The report's totals stay cumulative, so the reader takes each week's figure as the difference of two
releases. A missing release widens the span, and the issue says so.

## What must never appear

The rules of milestone 1016 (the clean-range table does not locate a held seed) and milestone 1008
extend to every new field and to every issue the reader writes.

- No seed, crash input, replay line or held log, in the report, the issue or the reader's output.
- No figure for one slot. A slot's figures after its first held window locate the held seed, which
  is why milestone 1016 withholds that slot's ranges.
- No figure from a held window: no rate, no commit, no edges, no time.
- No coverage map, only the two totals per driver.
- An issue carries only figures the published reports already carry, plus their differences. It names
  no slot, no window time and no class of a held window beyond the report's counts.
- The reader downloads `report.md` alone, never `clean-ranges.tsv`, as the heartbeat already does.

## Options

- A. A weekly hosted workflow reads the last eight releases and keeps one labelled issue per rule:
  it opens the issue when a rule fires, updates it while the rule holds, and closes it when the rule
  stops. Nothing enters the tree. `helpers/ci_job_times.py sync` already does this for CI jobs near
  their budget, with its `near-budget` label. A deleted release shortens the window the reader sees,
  and the issue states that window.
- B. Option A, plus one row a week in `notes/project-metrics/` and a chart in
  `notes/project-metrics.md`, written by `script/metrics` in the daily metrics run. The trend
  survives a deleted release. The figures enter git history, which this project never rewrites.
- C. A script the maintainer lane runs by hand during its survey. Nothing is scheduled, nothing
  opens an issue, and the findings land only when a lane reads them.
- D. Build nothing until the coverage counters exist. The report grows its three lines, and the
  heartbeat stays the only reader.

## Recommendation

Option A. The test of effort does not decide it: B costs one more CSV and chart, and C costs less
than A. A matches how this tree already turns a daily number into work: `ci_job_times.py` reads a
record and keeps one issue per condition, and the heartbeat keeps one issue per broken loop. A
person still turns each issue into an action, and the issue says which kind.

A also keeps fuzz figures where milestone 1026 put them, in releases a maintainer can delete. B puts
them into history for good, and history cannot be rewritten here. Moving from A to B later is
compatible, since B is A plus a record. Taking B back is not. C repeats the failure the heartbeat
fixed: a check that runs only when someone remembers. D leaves two rows of the table unread although
they need nothing new.

## What it builds

1. Part 1, the report. `helpers/fuzz-daily-publish.sh` gains the seeds per second and the newest
   clean commit, and its selftest proves that no held row moves either figure. The edges line
   follows milestone 1008's counters and is not part of this.
2. Part 2, the reader. A weekly helper, provisionally `helpers/fuzz-weekly-review.py`, run from a
   second schedule in `.github/workflows/fuzz-heartbeat.yml`, with the runner's own token and no
   secret. It applies the four rules of the table that this output can drive, and the coverage rule
   switches on by itself at the first report with edges. The label is provisionally `fuzz-review`.
   Like the heartbeat, it stays quiet until the first release. A selftest over fixture reports joins
   `script/lint`.
3. Part 3, the target reader. `helpers/ci_job_times.py`, or a sibling, reads the per-target final
   statistics from the fuzz shard logs and adds a target that ran fewer executions than its floor,
   or added no inputs for four weeks, to the same issue mechanism. It may promote as its own
   milestone.

The heartbeat also lacks one check that milestone 1003 ruled for green: the newest report shows
more seeds run than the one before. Part 2 adds that as the heartbeat's fourth check.

The thresholds are the maintainer's, provisional, and reversing one costs one pull request. The
first set: progress stopped means no rise in clean seeds over seven days. The infrastructure rule
fires on a share up by 0.1 or more week on week. The coverage rule fires on edges hit up by under
0.1% of edges total over fourteen days. The freshness rule fires when the newest clean commit is
older than main's commit of three days ago.

## Cost

| part | size | runs |
|---|---|---|
| 1, the report | about 20 lines of `awk` and a selftest case | on the host, daily, already scheduled |
| 2, the reader | about 300 lines of Python with a selftest, the heartbeat's size | about a minute on a hosted runner a week; eight `report.md` downloads of a few kilobytes |
| 3, the target reader | about 150 lines added to an existing helper | inside the daily metrics run, which already reads the jobs API |

Part 2 adds no machine, credential or self-hosted runner. Unmeasured: the API calls Part 3 adds,
since it reads two job logs per merge group.

## An example

In week 2026W49 the confined driver's clean seeds rise as usual, and its edges sit at 9,310 of
41,882 (22.2%) for the second week running. The rate and freshness rules stay quiet. The coverage
rule fires, and the reader opens one `fuzz-review` issue. It reads: "confined: edges hit 9,310 of
41,882, up 0 over 14 days, from fuzz-2026-11-23 to fuzz-2026-12-07. Rule: coverage stopped growing.
The action is a maintainer's: record condition 2 of risk 7 as holding, or propose a model extension."
A maintainer lane reads the driver note's list of unmodelled calls and proposes a milestone that
adds `REAP` and `SURVEY` to the model. When edges rise again, the reader closes the issue. The
figures are illustrative, since no counter exists yet. The issue names no slot, no seed and no
window.

## What waits on this

Nothing is blocked. Without a reader the release still feeds the heartbeat, and risk 7's
condition 2 still has no one comparing weeks.

## Index row

The daily fuzz release proves the loop is alive and drives nothing. A weekly reader turns its figures into one labelled issue per rule that fires, and the report gains a rate and a freshness line, with no held input anywhere.
