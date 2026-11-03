---
status: PROPOSED
raised: 2026-11-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The Miri run keeps its budget, and a slow test names itself

Raised by lane/ub-check-3100 on 2026-11-03 (UTC), while it cleared issue #3100, the scheduled
undefined-behavior check's repeated red. The integrator mints the number at promotion.

Reuse: `xtask/src/suite.rs`'s `undefined_behavior_check`, milestone 663 (bound the host pass)'s
process-group deadline, and libtest's own time options. It takes no new tool.

## Background

`script/undefined-behavior-check` runs every host crate's tests under Miri, one `cargo miri test`
over the workspace, in one hosted job with `timeout-minutes: 240`. Miri interprets the program, and
a test that takes milliseconds natively can take an hour. Nothing measures any one test's share of
that budget, so the budget fails only as a whole: the job runs out of time and GitHub cancels it.
A cancellation names no test.

`cargo miri test` also stops at the first failing binary. So a red run hides every cost behind the
crate that failed, and a run that stays red long enough stops measuring anything.

## What the run looked like

Every number here comes from #3100's logs or the lane that cleared it.

- The runs of 2026-10-05 and 2026-10-12 hit 240 minutes and GitHub cancelled them. In those logs,
  `designware_mobile_storage`'s divider sweep over fifty million targets took 8,002 s, and
  `graphics_contract`'s whole-surface digest tests took about 3,700 s.
- From 2026-10-19, a leak in `capability_manifest_note` failed the run in two minutes. That hid
  every slower crate for three weeks.
- With `--no-fail-fast`, and with each remaining crate then timed alone, the lane found about twenty
  more tests costing from twelve minutes to over four hours each. Among them were a 77-minute
  doctest and two crates of three hours apiece. Each had arrived since the last green run, and each
  author had a passing native suite.
- After #3105's samples and skips, the lane estimates the whole run at about three hours,
  sequentially, against the 240-minute limit.

The convention in `notes/undefined-behavior.md` (a `cfg(miri)` sample, or an ignore with its
reason) worked every time someone applied it. What it lacks is a trigger: the next expensive test
shows itself only by spending the budget.

## What it changes

Three parts, each a lane.

1. **A per-test budget, in Miri's clock.** The run passes libtest's `--ensure-time` with a critical
   time for unit tests, integration tests and doctests. A test that finishes past it fails and
   names itself. Under isolation, Miri's monotonic clock counts interpreted work, not host time, so
   the same commit gives the same verdict on any runner. #3100's lane saw this directly: the
   `file_allocation_table` suite reported "finished in 828.76s" on two separate runs, to the
   hundredth. The budget is therefore a property of the test, not of the runner it ran on.
   `--report-time` prints every test's time in the same clock, and the run keeps that table, so the
   next sample has its number before anyone asks.
2. **A deadline for a test that never finishes.** `--ensure-time` judges a test only once it ends.
   Milestone 663's process-group bound, a SIGTERM and then a SIGKILL to the whole group when the
   deadline passes, covers the test that never ends. It fails the run with the command named, well
   before GitHub's cancellation.
3. **The runner's cores.** Miri interprets each process on one thread, and the hosted arm64 runner
   has four cores. `undefined_behavior_check` splits the crates into four groups, balanced by the
   times part 1 records. It runs one `cargo miri test` per group at once, each in its own target
   directory, and joins their verdicts. That spends no second runner, which is the cost milestone
   808 (every gate accounts for its time) refused for sharding across jobs.

Part 1 comes first. It alone turns the next cancellation into a named failing test.

## What it costs

- Part 1: an hour of xtask work, plus one measurement. The lane checks that the pinned nightly still
  has `--ensure-time` and its `RUST_TEST_TIME_UNIT`, `RUST_TEST_TIME_INTEGRATION` and
  `RUST_TEST_TIME_DOCTEST` thresholds, and that `MIRIFLAGS` forwards them with
  `-Zmiri-env-forward`, since Miri does not forward the host environment. Both are recalled, not
  re-read. The threshold is a two-way door, and it should start strict: ten minutes of Miri time per
  test is the suggestion. The lane sets it from the first `--report-time` table, not from a guess,
  and fixes or skips each test over it in the same pull request.
- Part 2: milestone 663's work, applied to one more invocation.
- Part 3: four target directories compile shared dependencies four times. #3100's lane measured
  the cost locally: a hardlinked copy of a built target directory rebuilt nothing, so the job can
  build once and copy. The win depends on balance. The longest group, not the sum, sets the run's
  length.
- The standing cost: a test author whose test costs more than the budget under Miri writes a sample
  or an ignore with a reason before it merges. They do that today, but only after the weekly run
  fails.

## A concrete case

The divider sweep is the example. Natively it checks fifty million targets in about four seconds.
Under Miri it took 8,002 s, more than half the job's 240 minutes, and nobody saw it. Its run was
cancelled first, and then the leak in front of it failed the run. Under part 1 the first weekly run
after it merged would have failed with one line: `the_divider_never_runs_the_card_faster_than_asked`,
over the ten-minute critical time. The 9,973 stride #3105 gave it takes 6 s.

The checksum tests in `graphics_contract` would have failed the same way at about 3,700 s
together. Part 3 then works on the sum: three hours sequential, split four ways, comes to under an
hour if the groups balance. That holds only while no single crate approaches the hour, and part 1
keeps every test under its budget, though a crate can still hold many tests.

## Prior art

- **cargo-nextest.** Its `slow-timeout` with `terminate-after` kills a test that runs too long and
  names it, and `cargo miri nextest run` runs each test as its own Miri process, in parallel. It is
  the closest existing shape, and this proposal still refuses it, for the reasons milestones 663 and
  970 (a mutant that hangs is its own verdict) recorded. It is a dependency, which §46 (thin
  primitives or whole subsystems) makes a decision. It also does not run doctests, and the
  `graphics_contract` doctest took 77 minutes here. Its timer is also wall-clock,
  so its verdict moves with the runner. Part 1's clock does not.
- **Miri's own flags.** None of them bounds time. The flags that would make a red go away,
  `-Zmiri-ignore-leaks` and `-Zmiri-disable-isolation`, turn off the checks the run exists for, and
  `notes/undefined-behavior.md` refuses both. Miri's deterministic clock under isolation is the one
  feature this proposal builds on.
- **The Rust standard library's Miri run.** rust-lang/rust runs Miri over `core`, `alloc` and `std`
  as a CI job of its own, apart from the ordinary test jobs. Its tests shrink themselves under
  `cfg(miri)` and skip with `#[cfg_attr(miri, ignore)]`, the convention this tree already uses.
  That is recalled, not re-read; the lane should check how that job bounds its time.

Where nife differs: the tree has one runner's budget for 112 crates, and a weekly cadence. A slow
test costs a week of signal before anyone sees it, which is why the budget has to fail per test and
not per job.

## Index row

The weekly Miri run spent its 240 minutes without naming a test, and a red run hid every cost
behind it. Proposed: a per-test budget in Miri's deterministic clock, milestone 663's deadline for a
test that never ends, and the runner's four cores.
