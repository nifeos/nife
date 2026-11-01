---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# A short sharded in-guest fuzz pass in CI

Raised by lane/a-short-sharded-fuzz-pass-in-ci on 2026-11-01 (UTC), asked to add a bounded fuzz job
to CI that runs the harness the continuous fuzzing host runs. No roadmap block describes that job:
milestone 42 (supply chain and fuzzing) and milestone 779 (fuzz the surface a confined process can
reach) cover the cargo-fuzz `fuzz` job, which does not run the in-guest drivers, and
`notes/fuzzing/continuous-host.md` covers the host only. This proposal states the smallest useful
design and waits for a decision before any workflow is written.

Reuse: the job calls the window logic of `helpers/continuous-fuzz.sh` (build the x86_64 archive and
the system test image with a baked seed range, run one KVM guest through the PVH runner). It writes
no new harness.

## What is missing

A pull request gets the 60-second cargo-fuzz targets over the parsers. Nothing in CI sweeps the two
in-guest drivers (milestone 752 (a seeded syscall driver with a shadow model) and milestone 779).
The host finds a regression within hours; a pull request that breaks a driver invariant merges and
waits for the host.

## Proposed job

- What runs: `kernel-thread` and `confined`, the two drivers, each as the host's one window runs
  it. The script gains a `--ci DRIVER FIRST_SEED` mode that reuses `window()`'s build and run steps
  and skips the state directory, the held directory and the worktrees.
- Shards: 4, two per driver, each with its own first seed. CI seed ranges sit below the host's
  base (10000000000), so a CI shard never repeats a seed the host sweeps. The first seed derives
  from the shard and the run number, so successive runs cover new seeds.
- Time bound: the driver's own 60 seconds of guest time, a build bound of 15 minutes and a run
  bound of 5 minutes, inside the 20-minute job budget (milestone 721 (each merge-group CI job has a
  20-minute budget)). Shards run in parallel, so wall time is one shard's build plus about 2 minutes.
- Where: a GitHub-hosted runner with `/dev/kvm`, never a self-hosted one. The job refuses, by name,
  when KVM is absent (`--check-kvm`) and never falls back to TCG, which sweeps a tenth of the seeds.
  A fork pull request therefore never reaches owned hardware. Whether hosted Linux runners expose
  KVM to this repository is unverified and is the first thing the build lane measures.
- When: `merge_group` and `workflow_dispatch`. Running on every `pull_request` push is a cost
  question this proposal does not settle (see below).
- What counts as a failure: a `replay:` line, a panic, or a guest that runs out its run bound. A
  build failure or missing KVM is infrastructure and fails the job with a different message.
- How a finding is handled: the shard's log goes to a runner temp file, mode 600, and is deleted by
  the final step. Nothing from it reaches the job log, a step summary or an artifact. The job prints
  only `N of 4 shards reported a finding; a person must look; rerun the shard privately`, and
  names the shard, the commit and the driver, never a seed. Because a seed replays the same calls,
  a person with the commit and the shard reproduces it off the runner. The finding follows the
  existing path: private until its fix merges, a private security advisory first.
- Check that fails without the change: a lint section asserts that `ci.yml` has a job running
  `helpers/continuous-fuzz.sh --ci`, that no step in it echoes, `cat`s, tees or uploads the shard
  log, and that its `runs-on` is not self-hosted. A second test runs `--ci` against a stub guest
  that prints a `replay:` line and asserts that the output holds the count and no seed.

## What I need from you

1. Do the hosted runners need to run it on `pull_request`, or is `merge_group` enough? The
   recommendation is `merge_group` only: a fork pull request on a public repository writes its log
   where anyone can read it, and the private-finding rule is easier to hold when the job runs only
   on the queue entry of a pull request a maintainer already admitted. Loosening later is compatible.
2. May a finding found at queue time fail the entry and block the merge? The harness false reds
   (guests that fail on a deadline under load, where the seed replays green) are not yet fixed, per
   the continuous fuzzing proposal. The recommendation is to land the job as non-required until
   that fix lands, then make it required.
3. Is a log that is deleted without being shown acceptable, given that a person must rerun to see
   the finding? The alternative, an encrypted artifact, needs a key held in repository secrets, which
   is an external action and a trust-root question.

## Unmeasured

Build time of the two images on a cold hosted runner, KVM availability there, and the false-red rate
at four parallel shards. The build lane measures all three before the job gates anything.
