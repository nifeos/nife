---
status: BUILT
promoted_from: the-fuzz-host-replays-a-red-ci-shard
raised: 2026-11-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1038. The fuzz host replays a red CI shard

Raised by lane/fuzz-setup-reconcile on 2026-11-03 (UTC), while it reconciled the fuzz setup page with
what is built. [Milestone 1012 (a short sharded in-guest fuzz pass in CI)](1012-a-short-sharded-fuzz-pass-in-ci.md) names this work as a
follow-on.

Reuse: the window logic of `helpers/continuous-fuzz.sh`, whose `--ci-range SHARD` gives a shard's
driver and first seed; the follower's fetch-free reads and green rule in `helpers/fuzz-follow-main.py`;
the held-findings store; and the filer in nifeos/fuzz-host, which files every held log as a private
advisory. Nothing here writes a new transport, and the filer needs no change.

## Background

The `fuzz-guests` job runs one short in-guest pass per shard on every merge group, on a hosted runner,
as a non-required check. When a shard goes red, the job prints a pointer: the commit, the shard number
and the line "the fuzz host replays this shard". It prints no seed and no guest text, because a hosted
log is public.

## What it builds

The host side of that pointer, in three parts, each the one milestone 1012 lists. The note
[`continuous-host.md`](../../notes/fuzzing/continuous-host.md#replaying-a-red-ci-shard) describes
them for an operator.

1. The host learns that a shard went red. `helpers/fuzz-red-shards.py look`, run by the supervisor at
   most every 15 minutes, lists the failed merge-queue runs of `ci.yml` from the public API with no
   credential. It keeps a run once the clone's HEAD contains its commit, and reads that run's jobs.
   A job named `fuzz-guests (shard N, ...)` that ended `failure` or `timed_out` is a red shard. The
   commit must also be green by the follower's rule. It never reads the job's log.
2. The host replays the shard. The supervisor stops its slots, re-executes itself and replays the
   queue before any slot starts. `helpers/continuous-fuzz.sh --replay COMMIT SHARD RED_AT` checks the
   commit out in `work/replay/`, takes the driver and first seed from that commit's own
   `--ci-range SHARD`, and sweeps the range once with the window's own build.
3. A reproduction becomes a held finding. Its log moves to the held directory under the name the
   filer reads, and its `windows.tsv` row carries the held class and the slot `replay`. The filer
   files a private advisory from them unchanged. The journal line holds a finding id and an age
   alone. A clean replay keeps no log and writes a `replay-clean` row.

## The three questions, answered

Each answer is a two-way choice, decided by the maintainer, provisional;
reversing costs one pull request. Each adds no credential, token, secret or permission anywhere.

- **Poll, not push.** A push needs a credential in the workflow and an inbound path to the host;
  1012 refused the first, and the host has neither. The poll reads public data only. Measured:
  the unauthenticated run listing answers with `x-ratelimit-limit: 60`. A look
  costs one request, plus one per failed run that main holds, plus three per red shard. Of the last
  100 failed merge-queue runs, 98 had readable jobs and none had a red `fuzz-guests` shard, and most
  had commits that never reached main, which cost nothing past the listing. At one look every
  15 minutes the poll adds about 4 requests an hour to the follower's.
- **The commit comes from main, through the clone.** GitHub deletes a `gh-readonly-queue/` branch
  when its entry finishes. An anonymous fetch by commit id still worked after the deletion (tested
  against a deleted entry's commit), but that is undocumented, and the commit of an
  entry that never landed has not passed main's gates. Running it would break the self-updating
  host's third limit. The required checks of main do not include `fuzz-guests`, and the queue merges
  with `MERGE` under `ALLGREEN`, so an entry whose only red is a shard lands and its commit reaches
  main unchanged: each of the last eight green merge-queue runs had its commit on main. The replay
  therefore takes only a commit that the clone's HEAD contains, and the follower moves the clone
  only under the four limits. It fetches nothing.
- **A clean replay is counted.** Its `replay-clean` row appears under "Windows by status" in the
  daily report, a count with no seed, and the report needs no change. A clean replay of a red shard
  is a false red, or a finding this host does not reproduce, so the count bounds the false-red rate
  that 1012 must measure before `fuzz-guests` becomes required.

## How it is checked

`helpers/fuzz-replay-selftest.sh` and `python3 -I helpers/fuzz-red-shards.py --selftest`, run by
`script/lint`. A replay against a stand-in guest that prints a `replay:` line leaves one held log
and one held row, and prints a finding id and an age alone. A clean guest leaves no log. A commit the
clone does not contain is refused before anything builds. Stubs of the run and job listings show
that green, skipped and cancelled shards, a red job of another name, a red shard on another commit
and a red shard on a commit main never took start no replay. End to end, the supervisor replays the
queue before its first slot.

## BUGS

- A red shard on an entry that never lands is never replayed, and the entry's next run sweeps other
  seeds. [`continuous-host.md`](../../notes/fuzzing/continuous-host.md#bugs) lists this with the
  replay's other limits.
- Nobody has run a replay on the host. The first red shard after the host's clone follows main past
  this change is the first.

## Status

Built. The host runs it once its clone follows main past it. A held reproduction waits for the
filer, which is built and not yet installed
([`finishing-the-setup.md`](../../notes/fuzzing/finishing-the-setup.md)).

## Index row

The fuzz host polls the public run listings, replays each red `fuzz-guests` shard from main with the shard's own seed range, and holds any reproduction for the filer.
