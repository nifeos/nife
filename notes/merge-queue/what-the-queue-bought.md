# What the queue bought, measured on 2026-08-16

*Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `measured-2026-08-16`.*

An appendix to [notes/merge-queue.md](../merge-queue.md), which keeps the conclusion. This file
holds the measurement in full: the windows, the table, the example, the five caveats, the prover's
tail, and what it corrected about the milestone.

## The measurement

It was taken 2026-08-16, and it is the definition of done of milestone 119 (the merge queue is the
bottleneck, and the long pole is one prover). The block already had the before-median and the
sharding. It said plainly that the after-median had to come from a run of pull requests, rather than
from the first one on the new path. There are eighteen now.

Where the numbers come from: the GitHub REST API, three endpoints. They are the merged pull requests,
each one's timeline events, and every workflow run's jobs with their start and finish. Nothing here is
read off a dashboard or remembered.

The two windows, and why the before one starts where it does. The proof shards landed on `main` with
#159 at 2026-08-14T05:22Z. The queue's first group build ran at 2026-08-15T21:46Z. So the before
window is the 40.4 hours between them. It holds the *current* prover constant, and measures only what
the queue changed. The after window is the 10.2 hours from the first group build to
2026-08-16T08:00Z, where this snapshot stops.

| | before: sharded, no queue | after: the queue |
|---|---|---|
| window | 08-14T05:22 to 08-15T21:46 (40.4 h) | 08-15T21:46 to 08-16T08:00 (10.2 h) |
| pull requests landed | 39 | 18 |
| "land this" to merged, median | 17.0 min (n=29, auto-merge armed) | 12.3 min (n=17, last enqueue) |
| gap between consecutive merges, median | 15.8 min (n=34) | 10.8 min (n=17) |
| merges per elapsed hour | 0.97 | 1.76 |
| opened to merged, median | 47.0 min (n=38) | 160.9 min (n=18) |
| CI job-minutes per landed pull request | 122 | 157, or 109 with the storm hour removed |
| runs on `main` that went red | 2 of 120 since 08-13 | 0 of 30 |

The row that got worse says nothing about the queue, and saying so is the point. Opened-to-merged
counts everything that happened to a pull request, including how long it sat before a person enqueued
it. Its after-window median of 160.9 minutes decomposes. The median from *first* enqueue to merged is
112.1 minutes, and from *last* enqueue to merged is 12.3. The difference is one afternoon's storm of
evictions, plus eleven re-enqueues that were operator error. The queue's own cost is the 12.3.

## EXAMPLE: five pull requests, one cycle

The clearest thing in the data. At 2026-08-15T23:42:51 through :57, five pull requests were enqueued
within six seconds of each other. GitHub built five chained candidates concurrently from 23:43:07,
each containing one more entry than the last. #204 and #205 landed at 23:50:37; #207, #208 and #209
landed at 00:03:32.

That is five pull requests, 20.6 minutes from enqueue to the last merge. At the before-window median
of 17.0 minutes each, serialized, the same five would have been about 85 minutes. Under the
up-to-date rule of §73 (ten admin minutes), each merge
would have staled the other four at least once. So the real before-cost is higher than that, and is
the thing the ordering brain in [what-the-merge-queue-took-over.md](what-the-merge-queue-took-over.md) was written to manage.

## The caveats, and there are five

- The samples are small, and each is one afternoon: 39 landings against 18, both from the same week,
  both from lanes run by the same architect. This is a measurement of this tree in August, not a
  general result about merge queues.
- Runner contention varies and is not controlled. The same `CI` job, on candidates that differ only
  in which pull requests they contain, ranged from 6.6 to 23.5 minutes across the 44 group builds.
  Any single comparison of two runs is inside that noise; only the medians are worth reading.
- One storm inflates every early after-number. Between 21:46 and 22:34 on 08-15, twenty-five
  candidate builds failed CI for one reason. `script/lint`'s branch-prefix check rejected the queue's
  own `gh-readonly-queue/*` branches, so every candidate was ejected and rebuilt. That was 678
  job-minutes. The pull requests caught in it carry a two-hour first-enqueue-to-merged that is the
  gate's bug rather than the queue's behavior. #217 fixed it, and was merged directly, outside the
  queue, because the queue could not land anything until it was.
- Several re-enqueues on 08-16 were operator error, not eviction. There were eleven re-enqueues
  across seventeen pull requests, nine of which needed more than one. Some were the queue ejecting a
  candidate; others were a person removing and re-adding one. The timeline records both as the same
  event pair, so this measurement cannot separate them, and does not try.
- The after window's composition is not the before window's. It holds the day's largest change
  (#210, the SMB service), and three that waited on calef for a decision. That pulls opened-to-merged
  up, and leaves the enqueue-to-merged numbers alone, which is why both are in the table.

## The prover is the long pole, but only for the changes that reach it

Group builds run `CI` and `verify` concurrently, so the landing waits on whichever finishes last.

| | CI, median | verify, median |
|---|---|---|
| all 44 group builds | 12.2 min | 3.6 min |
| the 19 after the storm | 10.7 min | 0.6 min |
| the 6 where the proofs actually ran | 11.2 min | 16.7 min |

Twelve of the nineteen post-storm builds finished `verify` in under two minutes, because the scope
job proved that nothing in the change could reach a harness. So the median landing is now CI-bound
rather than prover-bound. That is the scoping and the sharding working exactly as milestone 119
predicted. It is a real change from the block's 2026-08-05 measurement that "a merge cycle is the
Kani job plus noise".

What is left is the tail, and in the tail the prover decides the landing. In those six builds it ran a
median 5.6 minutes past a `CI` that was already green.

And that tail is almost entirely false positives. Re-running the `--affected-since` predicate over the
seventeen changes that landed since 08-14 having run the full suite: for all five of the post-storm
ones, no file in the change was inside any harness crate's dependency closure. They proved everything
because of files the predicate cannot attribute to a crate, and so runs by default:

| landing | what made it prove the whole suite | harness crates it could reach |
|---|---|---|
| #207 | `Cargo.lock`, `Cargo.toml` (a new workspace member) | none |
| #208 | `art/cobble-first-draft.jpg` | none |
| #210 | `Cargo.lock`, `Cargo.toml`, `scripts/qemu-runner-*.sh` | none |
| #218 | `Cargo.lock`, `Cargo.toml` | none |
| #219 | `scripts/merge-drain.sh` | none |

Three levers follow, ranked by what the counts say and by how much judgment each needs. Together they
account for twelve of the seventeen:

1. `scripts/` is not `script/`, and the predicate only knows the singular. A change to
   `merge-drain.sh` proves twenty crates. Nothing under `scripts/` is an input to `cargo kani`. The
   QEMU runners belong to `xtask test`, and `kani-lint-shim/` belongs to `script/lint`'s clippy pass.
   That is the same argument the existing `script/` case already makes. Three of seventeen, and it is
   one branch in the `elif` that handles `script/` today.
2. `Cargo.lock` and the workspace `Cargo.toml`: seven of seventeen, the largest bucket, and the one
   that needs judgment rather than a line. Adding a workspace member cannot change a harness's
   closure; bumping a dependency version can. The honest version parses the lock diff for changed
   package entries, and tests those against the closure. It wants its own lane.
3. Binary and data files: two of seventeen (`art/`, `bench/baseline-*.txt`). Same shape as the
   documentation case the predicate already handles.

On 2026-08-14 more shards was not the lever (CI runs three since 2026-10-07). `glob` was atomic at
15.0 minutes of a 30.3-minute suite, so two shards reach 15.1 and four reach 15.0. The measured
group-build `verify` when the proofs run is 16.7 minutes, which is that floor plus Kani's install.
Nothing under it comes from arranging CI differently. It comes from the unwind bound in one `glob`
harness, or from not proving crates a change provably cannot reach.

## What the measurement corrected about the milestone

The queue does not amortize CI cost across a group; it amortizes wall clock. Milestone 119's block
expects "N pull requests cost one test cycle instead of N". The ruleset as configured
(`max_entries_to_build: 5`) builds one candidate *per entry*, concurrently, each running the full
`CI` and `verify`. That is why cost per landed pull request is flat across the two windows (122
minutes before, 109 after with the storm removed), while wall clock nearly halved. The saving is real,
and it is a different saving from the one predicted.

Cost still matters even though this repository is public and its Actions minutes are free. What it
actually buys is concurrent runners, and the spread in identical `CI` jobs above is what contention
looks like. Trading it back is a setting rather than a project. `max_entries_to_build: 1` would build
one group of up to five pull requests once, at one fifth the cost, and would pay for it in bisection
when a group fails. Nobody has needed that yet.

## Do the two watchers still earn their keep

`merge-drain.sh` did, and its job changed twice rather than ended. In this window it was the
enqueuer: every landing entered the queue through the arming call it made. On 2026-10-03 the arming
went, and the drain became a detector (milestone 727 (a queue eviction goes to a maintainer session),
provisional).

`trunk-health.sh` is closer to superseded, and the number is honest about how little it proves. Zero
of the thirty runs on `main` since the queue went live were red, against two of a hundred and twenty
in the three days before. Ten hours is not evidence that the trunk cannot go red. The class of
failure that remains is one the queue never sees: a flake, and the scheduled workflows (toolchain
bump, drift, mutation), which run against `main` on a timer and are not part of any merge. Keep it;
expect it to speak rarely.
