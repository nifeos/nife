---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 329. The 5.13x record-level number was measured against a request size nothing ships

Premise re-checked 2026-09-19 and it holds. `bench/record-level-sweep.sh` is still in the tree, `filesystem_contract::fs::TRANSFER_PAGES`
is still 16 (64 KiB). Milestone 138 (close the read gap)'s block still publishes 5.13x as its headline read ratio
in its opening paragraph and at four more sites, every one of them taken at 4 KiB. Nothing has re-run
the sweep at the shipped size.

The sweep script is in the tree, the shipped configuration is in the tree, and the
run is one command on patagonia.

**In brief.** Re-run the record-level sweep at the transfer size the system actually uses: `sh
bench/record-level-sweep.sh 3 0 1 5` with `TRANSFER_PAGES = 16`. Milestone 138's step 1 chose record
level 1 on 4 KiB evidence, and its step 3 then made 64 KiB the default request. So the 5.13x figure
step 1 published is a ratio about a contract the system stopped using two steps later, and the
record level it selected was selected under the same stale conditions.

## Why this matters

Two separate things are wrong, and the second is the one that costs.

The number is the smaller problem. A headline benchmark figure that describes a configuration
nothing ships is exactly what AGENTS.md's benchmark posture exists to prevent: measure, do not
argue, and say where a number is not apples-to-apples. This one is not even about the current
system, and it sits in a finished block where a reader will take it as current. Facts that leave
the machine are the irreversible category, and a quoted 5.13x is one.

The choice is the larger problem. Record level 1 won on 4 KiB requests. Nothing establishes that it
still wins at 64 KiB, and there is a plausible reason it might not: a larger request amortizes
per-record overhead differently, which is the whole mechanism the record level trades against. If a
different level wins at the shipped size, the system is running the wrong configuration today and
the sweep is what finds out. If level 1 still wins, the sweep costs one run and the block gets a
number that is true.

## What it would take

One sweep run, then an edit to milestone 138's block replacing the stale ratio with the measured
one and saying which size each was taken at. If the winning level changes, the configuration change
is a second, separate commit, because a benchmark correction and a behavior change should not be
one entry in `git blame`.

## Index row

Milestone 138's step 1 chose record level 1 on 4 KiB evidence. Its step 3 then made 64 KiB the
default request. So its 5.13x headline is a ratio about a contract the system stopped using, and a
quoted number has left the machine. If a different record level wins at 64 KiB, the system runs the
wrong configuration today. One run of `sh bench/record-level-sweep.sh 3 0 1 5` at
`TRANSFER_PAGES = 16` settles both.
