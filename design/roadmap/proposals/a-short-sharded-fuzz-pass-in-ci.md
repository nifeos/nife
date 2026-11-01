---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A short sharded in-guest fuzz pass in CI

Raised by lane/a-short-sharded-fuzz-pass-in-ci on 2026-11-01 (UTC). The integrator mints the number
at promotion.

Reuse: the job calls the window logic of `helpers/continuous-fuzz.sh` (build the x86_64 archive and
the system test image with a baked seed range, then run one KVM guest through the PVH runner). It
also uses the KVM recipe `swish-check-x86_64` already uses on hosted runners. It writes no new harness.

## What is in force

The maintainer decided these on 2026-11-01 (UTC), provisional; reversing each costs one pull request.

- The job runs on `merge_group` and `workflow_dispatch`, never on `pull_request`, on a hosted
  runner, never a self-hosted one.
- The job lands as a non-required check. Making it required is a later pull request, once the
  false-red rate at four parallel shards is measured.
- The job deletes the guest's log unseen. There is no encrypted artifact and no repository secret.
- A finding found in CI is private until fixed. The log carries a count, the shard, the commit and
  the line "the fuzz host replays this shard", and nothing else the guest said.

## Why

A pull request gets the 60-second cargo-fuzz targets over the parsers. Nothing in CI sweeps the two
in-guest drivers (milestone 752 (a seeded syscall driver with a shadow model) and milestone 779 (fuzz
the surface a confined process can reach)). The host finds a regression within hours; a change that
breaks a driver invariant merges and waits for the host.

## What to build

- `helpers/continuous-fuzz.sh --ci SHARD` runs one window on the checkout it stands in. Even shards
  run the `kernel-thread` driver and odd shards the `confined` driver. It exits 0 for a clean
  window, 1 for a finding and 3 for infrastructure (no KVM, a failed build, a guest that ended with
  no verdict).
- Four shards, two per driver, each a job of a matrix in `ci.yml` named `fuzz-guests`.
- The first seed of a shard derives from the commit and the shard number alone, from a block below
  the host's own range, so a CI shard never repeats a seed the host sweeps.
  `helpers/continuous-fuzz.sh --ci-range SHARD` prints the driver and first seed for the commit at
  HEAD.
- Time bound: 60 seconds of guest time. Two builds get 10 minutes each and the run gets 5, inside the 20-minute job budget (milestone 721 (each merge-group CI job has a
  20-minute budget)). The four shards run in parallel.
- Failure: a `replay:` line, a panic, or a guest that outruns its run bound. A build failure or a
  missing KVM device fails with a different message and the exit code 3.
- The guest's output goes to a mode-700 directory that an exit trap removes. The job does not use
  `script/ci-build`, `helpers/hang_evidence.py` or a time record, because each keeps or uploads step
  output.
- A check that fails without the change: `helpers/fuzz-ci-selftest.sh`, run by `script/lint`. It
  asserts that `ci.yml` has the job, that the job runs `--ci`, runs on `merge_group` and
  `workflow_dispatch` and never names `pull_request` or a self-hosted runner, and has no upload,
  `tee`, `cat` or step summary that could carry guest output. It then runs the shard against a
  stand-in guest that prints a `replay:` line and a panic. The output must hold exit 1, the count,
  the shard, the commit and the replay line, with no guest text and no seed, and no private
  directory may remain. It also asserts that a clean guest exits 0, a missing KVM device exits 3, and the
  same commit and shard give the same seed range.

## What the fuzz host must do (a follow-on, not built here)

The host side is a milestone of its own, written by another lane. This job requires three things of
it, and builds none of them.

1. The host learns that a shard found something. The trigger is out of scope here: it needs a
   network path and a credential, which this job does not add. The trigger carries a commit and a
   shard number and nothing else.
2. The host replays the shard from the commit and the shard number. `--ci-range SHARD` on that
   checkout gives the driver and the first seed, so the replay is deterministic.
3. Any reproduction becomes a held finding that enters the existing automatic path: the host files a
   private advisory and a fix lane reads it. Nothing that reproduces reaches a public log, release
   or artifact.

Until the host side lands, a red `fuzz-guests` job is a pointer that the host will replay, and
nobody replays it.

## BUGS

- The commit of a merge-group run is the merge commit on a `gh-readonly-queue/` branch. GitHub
  deletes that branch after the merge, so the host may fetch the commit only while GitHub still
  holds it. The follow-on milestone settles how the host gets the tree.
- Nobody has measured the false-red rate at four parallel shards. The job stays non-required until
  somebody does.
