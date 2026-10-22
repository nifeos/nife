# The register of measures: every number this kernel owes itself

*(Milestone 134 (the register of measures). The name `register-of-measures.md` is provisional;
naming is an architect's.)*

This tree measures a great deal and remembers almost none of it. A number gets taken once, written
beside the reasoning that needed it, and stays true only on the day it was written. `notes/counted-claims.md` found three such numbers on 2026-08-14 and all three were wrong.

That convention fixed the class of number a `grep` can re-derive. This register is the other half:
the numbers that need an instrument, a boot, or a walk over the source. It says which ones this
kernel holds itself to, which it merely knows, and which it has defined and cannot yet take.

[`notes/project-metrics.md`](project-metrics.md) is the half that moves: one row per ISO week,
recomputed from git history by `script/metrics`. Since 2026-09-24 it is a deck, and each chart's
argument is routed from [The weekly series](#the-weekly-series) below.

Each row's argument and history are in an appendix under
[`notes/register-of-measures/`](register-of-measures/), [listed at the end](#appendices).

## What belongs here, and the test

A number belongs if something depends on its value and it can move without anybody editing it. Both
halves matter, and the second is the one that cuts.

- *Something depends on its value.* Not "somebody would find it interesting". A decision rests on
  it, a constant is sized against it, a claim in the documentation quotes it, or a customer notices
  when it moves. `documentation::render::LINE_MAX` is 2048 because the longest markdown line was
  1841, so that measurement has a consumer. The kernel's image size, which
  `notes/benchmarks/kernel-footprint-and-caches.md` calls "the number that does not matter", has
  only a reader.
- *It moves on its own.* A constant somebody chose is a decision, and it belongs in
  `design/decisions/`. The stack guard page is 4,096 bytes because a page is 4,096 bytes. The
  deepest chain that can reach that guard is a measure, because the compiler moves it every week
  and nobody is asked.

A register that lists every number in the tree is worthless. So the exclusions are part of this
document, each naming the test it failed, in [their own section](#deliberately-not-in-this-register).

## The three states

Every row is in exactly one state. The state is a property of the instrument, not of the measure's
importance.

| state | means | what happens when the number moves |
|---|---|---|
| gated | an instrument re-takes it, and something fails on a bad move | a red build, at the commit that moved it |
| dated | a named command re-takes it; nothing fails | the recorded value goes stale, silently |
| owed | defined, with its instrument named; no instrument exists yet | nothing, because nothing is measured |

The `dated` rows are the finding. Something depends on each, and a regression arrives as
somebody's data being slow rather than as a red check. Promoting one to `gated` is the work.

`dated` is not a defect by itself. As `notes/counted-claims.md` puts it:

> A wall clock is not a count... dating a measurement is the honest alternative to gating it, and
> the two should not be confused.

A number that costs a forty-minute boot to re-take does not belong in a gate that runs on every
push. The defect is a `dated` row with no date, or with no command.

## Gated

Thirteen instruments. Ten are ceilings that fire when a number grows and stay silent when it falls.
Row 1 is a two-sided drift band and row 9 is a floor.

| measure | instrument | what fails |
|---|---|---|
| icount ticks, 14 benchmarks, both ISAs | `script/bench --check` | drift over 10% from `bench/baseline-*.txt` |
| IPC fastpath instruction footprint | `script/fastpath-footprint --check` | growth over 5% from `bench/fastpath-*.txt` |
| the largest kernel stack frame | `script/stack-frame-check` | any frame over the 4,096-byte guard page |
| the deepest reachable kernel-thread chain | `script/stack-depth-check` | a chain over the 24,576-byte stack |
| kernel stack high-water, at runtime | `script/test`, `report_high_water` | boot 61,440, secondary 16,384, thread 18,432 |
| eleven counted claims (harnesses, syscalls, rights bits, ...) | `script/lint` | a marked number disagreeing with the tree |
| unsafe density outside `kernel/src/arch/` | `script/lint` | over the `count-at-most` ceiling in `notes/unsafe-obligations.md`: 88 blocks per 10,000 code lines on 2026-09-24 (corrected from 94) |
| `unsafe impl Send`/`Sync` claims | `script/lint` | over the ceiling in the same file: 23 on 2026-09-24 (corrected from 17) |
| per-file line coverage | `script/coverage` | any file under the 80% floor |
| Rust file length | `script/lint` (file-length ratchet) | any `.rs` file over 2,000 lines |
| function length | `script/lint` (lint ratchet) | any function over 100 code lines |
| words per document | `script/lint` (prose ratchet) | any document over 3,000 words, unless marked |
| split parents | `script/lint` (parent-index check) | any split parent holding more than an index, unless marked |

Only the density ceiling expresses a direction rather than a limit, and it mixes kernel and
userspace unsafe into one population; the split is plotted weekly with no ceiling of its own. The
thresholds read together,
and the density's lowering history, are in [the rows appendix](register-of-measures/gated-dated-and-owed-rows.md#gated).

## Dated

The command is the point of each row. A dated measurement whose re-taking is folklore is a `dated`
row pretending to be one.

| measure | last taken | the command that re-takes it |
|---|---|---|
| IPC round trip in nanoseconds, both planes | 2026-08-04 | `script/bench --real` |
| filesystem throughput, milestone 38's four phases | 2026-08-18 | `script/bench --real --smp`, with a RedoxFS disk attached |
| primitives against Linux and macOS on the same host | 2026-07-25, spawn row 2026-07-26 (recovered from git on 2026-09-24, not re-taken) | `bench/host/run_linux.sh`, then `script/bench --real` |
| `unsafe {}` blocks inside `kernel/src/arch/` | every run | `script/lint`, which prints it and asserts nothing |
| E1: IPC round trip against thread count | 2026-09-04 (radon, 6 boots); 2026-08-22 (dev Mac) | `cargo xtask bench --real` (`ipc_scale_*` rows); on radon, `script/board-image --bench` and notes/footprint-perturbation.md |
| E2: thread census on the customer path | 2026-08-22 | `cargo xtask test`, the "E2 thread census" line in `a_host_process_connects_to_the_guest_and_is_answered` (both ISAs) |
| E3: IPC fastpath footprint doubled, and the latency it costs | retired by calef, 2026-10-20; last read 2026-09-04 (confounded) | none; E5 is milestone 708 (an executed footprint ladder for radon) |
| E4: application working-set displacement under IPC traffic, 8-pair and 48-pair load | 2026-09-04 (radon, 6 boots); 2026-08-23 (dev Mac) | `cargo xtask bench --real` (`appdisp_*_ipc`/`appdisp_*_ipc96` rows); on radon, `script/board-image --bench` and notes/footprint-perturbation.md |
| per-IPC kernel stack depth, per shape and role | 2026-10-20 (riscv64 release, QEMU and radon at one commit); 2026-09-19 (QEMU, all three ISAs, debug and release) | debug: `script/test`, the `ipc-stack-depth:` lines; release: a `bench,ipc_stack_depth` kernel on one hart (notes/stack-high-water.md, "Per-IPC depth") |
| multi-tasking throughput, jobs per minute against task count, milestone 168 (a multi-tasking workload benchmark) | 2026-10-04 (radon, 5 boots; every point within 1.7%) | `script/board-image --job-mix --tftp`, then `script/board-console`, by `notes/job-mix.md`'s bench-evening procedure; the rehearsal is `script/job-mix` |

Three rows need a sentence before anyone quotes them.

- **E3 is retired with no attributable number** (calef, 2026-10-20 UTC). Milestone 796 (pin the
  hot trap path's placement) leaves its control nothing to move, and the question milestone 188
  (the IPC fastpath) leans on is E5's.
- The milestone 168 row's date belongs to an instrument that changed on 2026-09-19. Risk 4 in
  `design/fatal-risks/README.md` and §96 (process kernel or event kernel) wait on the next radon evening.
- The cross-OS row's date was recovered from git, not from a recorded run. It is the number a
  stranger quotes first, and it has not been re-taken.

The E1 to E4 readings, what each found, and why three of them needed a small-cache board are in
[the cache experiments appendix](register-of-measures/cache-experiments.md). The arguments for the
filesystem, cross-OS, job-mix and arch rows are in [the rows appendix](register-of-measures/gated-dated-and-owed-rows.md#dated).
The arch row carries its rule in one line: a number with a consumer gets a relation; a number with
only a reader gets printed.

## Owed

Eight measures, M5 to M12 ("Tier B"). As of 2026-09-19 they no longer share one blocker. E1 to E4
("Tier A") ran on 2026-08-22 and are `dated` rows above.

| measure | its instrument, checked against the tree 2026-09-19 | what is still missing |
|---|---|---|
| M5, cycles per IPC round trip | exists on all three ISAs: every tick row times `bench::cycles_per_tick` (milestone 74 (cycle counters), milestone 309 (unhalted core cycles on `x86_64`)) | on riscv64, nothing: radon read `250.00` on 2026-09-16, so `call_reply` is about 1,256 cycles. On aarch64, argon's session and calef's `PMCCFILTR_EL0` ruling before any figure is published |
| M6, I-cache misses per IPC | none | an event-counter driver: nothing programs `PMEVTYPER<n>_EL0` or an SBI PMU cache event on any ISA |
| M7, D-cache misses per IPC in the stack region | half: the per-IPC stack depth bounds the bytes, not the misses | the same driver, and a data-address sampler that neither the A57 nor the U74 has |
| M8, TLB misses per IPC | none | the same event-counter driver |
| M9, per-phase cycles across one IPC | the counter: `arch::pmu::cycles()` in-kernel on all three ISAs | phase stamps at trap entry, dispatch, rendezvous, switch and exit; not built |
| M10 to M12 | as milestone 134's block says | unchanged |

Each measure's prediction, and what its outcome settles, is in
design/roadmap/0134-the-measurements-that-decide.md rather than duplicated here. Which rows the
`PMCCFILTR_EL0` ruling touches is in [the rows appendix](register-of-measures/gated-dated-and-owed-rows.md#owed).

## Deliberately not in this register

Each names the half of the test it failed, so the next person does not add
it back.

| number | why it is out |
|---|---|
| the kernel's image size (290,816 bytes on aarch64) | no consumer. notes/benchmarks/kernel-footprint-and-caches.md derives it and then says in its own heading that it is "the number that does not matter": `.text` that never runs during an IPC costs nothing in cache |
| `script/verify`'s wall clock (~47 minutes) | no consumer. Nothing is sized against it; notes/verification.md dates it |
| lines of Rust, crates, user programs, commits | no consumer. AGENTS.md's method figures are scale, and it says so; a gate on them would measure a paragraph |
| `nifefs`'s `NAME_LEN = 32` | does not move on its own. It is a decision with a cost per directory block |
| the number of `#[cfg(kani)]` unsafe blocks (14) | already gated per block, by the clippy configuration milestone 113 (the proofs' own unsafe code is ungated) added |
| `unsafe {}` against `// SAFETY:` parity | measured and refused. `clippy::undocumented_unsafe_blocks` enforces it per block as a hard error; a count comparison disagrees with it in 65 places (38 with a looser regex), and every one is a correct document. notes/unsafe-obligations.md has the reading |
| the CoreMark score | already gated, as a row in `bench/baseline-*.txt` |

Read the parity row before proposing a new gate. A count check that fails correct documents is a
gate that will be deleted, and `script/lint` has already lost three checks with that signature.

## The weekly series

`script/metrics` computes these for every ISO week; the charts are in
[`notes/project-metrics.md`](project-metrics.md). They are `dated` by construction, re-taken daily,
and gate nothing. [Reading the weekly series](register-of-measures/reading-the-weekly-series.md)
comes first: every row is a restatement under today's definitions, not what was reported at the
time.

| series | what it counts | argued in |
|---|---|---|
| fatal risks | the nine risks in `design/fatal-risks/README.md` by experiment status (`RUN`, `NOT-RUN`, `CANNOT-RUN`) | [records by status](register-of-measures/records-by-status.md) |
| fatal risk colors | the verdict word per risk, from appendix frontmatter | [fatal risk colors](register-of-measures/fatal-risk-colours.md) |
| Kani proof harnesses | harnesses, and how many carry a falsification record | [proofs and coverage](register-of-measures/code-proofs-and-coverage.md) |
| unsafe outside `arch/` | blocks per 10,000 code lines, against the gated ceiling | [the unsafe series](register-of-measures/unsafe-series.md) |
| unsafe by trust boundary | the same blocks split kernel, userspace, shared and boot chain | [the unsafe series](register-of-measures/unsafe-series.md) |
| milestones built each week | a flow, read from each block's `Built` date in today's tree | [landed each week](register-of-measures/landed-each-week.md) |
| pull requests merged each week; debt-paydown share | merge commits on `main`; the share, from GitHub labels | [landed each week](register-of-measures/landed-each-week.md) |
| commits and lines by model | commits and lines touched per week, bucketed by the `Co-Authored-By` trailer | [which model wrote it](register-of-measures/which-model-wrote-it.md) |
| what the project costs | person-weeks, lane tokens and hours with a dated shadow price, cash, and context per turn | [project cost](register-of-measures/project-cost.md) |
| decisions, names, proposals, milestones by status | the status each record carries, per week | [records by status](register-of-measures/records-by-status.md) |
| Rust in the tree | code and comment lines, `kernel/src` apart from the rest | [proofs and coverage](register-of-measures/code-proofs-and-coverage.md) |
| `BUGS` sections | markdown headings and Rust doc-comment headings; rising is good | [proofs and coverage](register-of-measures/code-proofs-and-coverage.md) |
| coverage, and the lowest-covered file | the workspace aggregate, and the per-file minimum the floor acts on | [proofs and coverage](register-of-measures/code-proofs-and-coverage.md) |
| ratchet debt | documents over a writing-standards limit, comment blocks over 40 lines, line-number citations | [ratchet debt](register-of-measures/ratchet-debt.md) |
| unexplained benchmark drift | per architecture, the largest icount drift on `main` since a fixed anchor that no record accounts for, and the rows past 5%; a report, not a gate | [the drift report](project-metrics/baseline-drift.md), generated |
| interface stability | public items of the contract and std ABI crates removed or changed, and added; syscall renumbers and renames, format bumps and amendments, co-change | [how stable the interface is](interface-stability.md) |
| homes | packages by where their home stands and whether they have left; tracked files by the same | [package boundaries](package-boundaries.md) |

## EXAMPLES

### Adding a measure to the register

Four steps, worked through for the unsafe census, with its numbers and its first failure, in
[the unsafe series appendix](register-of-measures/unsafe-series.md#adding-a-measure-to-the-register-worked-on-the-unsafe-census):

1. Apply the test above: something depends on it, and it moves on its own.
2. Take the number more than once. A single measurement cannot tell a direction from a level.
3. Choose the relation from the shape of the quantity. Equality for a census somebody maintains,
   `count-at-least` where a deletion is the bad event, `count-at-most` where a drift up is. See
   notes/counted-claims.md.
4. Watch it fail before trusting it. The density ceiling's first marker never fired.

### Re-taking a dated measure

There is no wrapper and there should not be one. Each dated row's command is in its table cell
because the commands are different animals. A `script/measures` that ran all of them would take an
hour and be run by nobody. Copy the cell.

```sh
# the filesystem row, which needs a disk attached
script/bench --real --smp

# then edit the date in this file's table, in the same commit as the numbers
```

If the number moved, the finding is the movement, not the new value. Say what moved and against
what in notes/benchmarks.md, and leave this register holding only the date.

## BUGS

- A `dated` row goes stale silently. This register makes the staleness visible to a reader who opens
  it, and to nobody else. A freshness check would assert a policy nobody has set. If a row's
  staleness starts to matter, promote it to `gated`.
- The register is a ratchet. A measure nobody adds is not tracked, and "the register is complete" is
  never a thing anybody can say. That is the same boundary `notes/counted-claims.md` records.
- The gated and dated rows are maintained by hand. Nothing checks that `script/fastpath-footprint`
  still exists or that `script/bench --real --smp` is still the command. `script/lint` fails when a
  script has no entry in notes/scripts.md, so a renamed instrument cannot vanish from the tree, only
  from this table.
- `patches/std-nife/overlay/` is outside the unsafe census, and it is our code: 37 `unsafe {}`
  blocks in the `std` platform layer, fifteen without a `SAFETY:` comment in the lint's form, and
  no clippy configuration reaches them. The unsafe series' own limits (dilution by safe code, a text
  scanner rather than a parser, and that no count measures the verification argument) are in
  [its BUGS](register-of-measures/unsafe-series.md#bugs).
- E1, E3's latency half and E4 need a real cache. E2's first reading was wrong in a way worth
  copying the fix for, and E4's 8-pair condition could not falsify anything alone, which the
  48-pair condition closed on 2026-08-23. All three are in
  [the cache experiments' BUGS](register-of-measures/cache-experiments.md#bugs).

## Appendices

Each verifies or challenges a row above; none carries this page's argument. The stems are
provisional names.

| appendix | what it holds |
|---|---|
| [gated-dated-and-owed-rows](register-of-measures/gated-dated-and-owed-rows.md) | the argument behind each row of the three tables |
| [cache-experiments](register-of-measures/cache-experiments.md) | E1 to E4 and the per-IPC stack depth, reading by reading, and their BUGS |
| [reading-the-weekly-series](register-of-measures/reading-the-weekly-series.md) | what a weekly row is and is not, how the series stays current, and the dated 2026W39 reading |
| [records-by-status](register-of-measures/records-by-status.md) | fatal risks, decisions, names, proposals and milestones by status |
| [fatal-risk-colors](register-of-measures/fatal-risk-colours.md) | verdict colors by week and the backfill's limits |
| [unsafe-series](register-of-measures/unsafe-series.md) | the unsafe census by week and by trust boundary, and its BUGS |
| [file-size](register-of-measures/file-size.md) | the §266 (a Rust source file stays under 2,000 lines) gate's measure, comments counted |
| [landed-each-week](register-of-measures/landed-each-week.md) | milestones built and pull requests merged each week, and the 2026-09-23 reconciliation |
| [which-model-wrote-it](register-of-measures/which-model-wrote-it.md) | the commits-and-lines-by-model series: why the trailer, the checked three-way sum, and what it cannot say |
| [project-cost](register-of-measures/project-cost.md) | the cost units, the capture deadline, the two dollars, and what a turn costs |
| [code-proofs-and-coverage](register-of-measures/code-proofs-and-coverage.md) | Kani harnesses, Rust lines, `BUGS` sections, coverage and the lowest-covered file |
| [prose-budget](register-of-measures/prose-budget.md) | the §212 (a prose budget) gate's scope, count and BUGS |
