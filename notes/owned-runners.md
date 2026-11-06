# Owned runners: what they are, who can reach them, and how to flip them

*Name: provisional, minted 2026-11-03 (UTC) by lane/owned-runners-canary.*

COE action item: notes/coes/2026-11-03-merge-queue-backlog.md, action 3.

The variable `NIFE_RUNNER_OWNED`, `helpers/owned_runner_check.py` and the canary workflow's job names are provisional too.

The organization runs a small pool of its own CI runners next to GitHub's hosted ones. This page is
the runbook: what the pool is, what may run on it, who can put code on it, how a repository
maintainer switches work between owned and hosted, and what to do when it misbehaves. The maintainers keep
machine-local facts (which hosts, where they sit) off-tree, and this page does not need them.

## What the owned runners are

- Five runners in the runner group `nife-owned` of the `nifeos` organization, all carrying the label
  `nife-owned-x86_64`.
- Each job gets a fresh rootless container on an owned host, then the runner exits: just-in-time and
  ephemeral, so no job sees another's files. Inside it the job runs as `runner` with passwordless
  `sudo`, so it is root in the container and nowhere else.
- Each container has 4 vCPUs, 16 GiB of memory, `/dev/kvm`, a 50 GB volume and a 90-minute limit.
- The image is Ubuntu 26.04 with runner 2.338.2 and `qemu-system-x86` installed.
- Egress goes through an allowlist proxy. The proxy refuses a host not on the list and reports it to the
  maintainers, who add it (see "A blocked host" below).

## The admission rule

A host-side hook admits exactly two kinds of job and turns the rest away:

- `merge_group` on a ref under `gh-readonly-queue/main/`
- `push` to `main`

The hook turns away a `pull_request` job at the door, and a `workflow_dispatch` job too, so a
fork's pull request run never reaches owned hardware. A merge group that holds a fork's pull
request is another matter: "Who can reach the pool" below. A turned-away job does not fail; it waits for a runner that
will never take it and blocks whatever needs it. For that reason `script/lint` rejects the tree that would send one there
(see "The lint" below).

## Who can reach the pool

Code reaches an owned runner only through a merge group or a push to `main`, and both need write
access to nifeos/nife:

- A merge group holds pull requests that an account with write access armed or enqueued. It runs
  each pull request's own code, its workflow files included, so a pull request that edits a
  workflow runs its edit on the pool before any check on it can fail.
- A push to `main` comes only from the merge queue: the `main` ruleset refuses a direct push, and
  has no bypass actor.
- A push to any other branch runs no owned job, because every owned job's workflow limits `push`
  to `main` (the lint below), and the host refuses one that arrives anyway.

So the pool runs code from the people and Apps with write access, and from any pull request they
put in the queue. A pull request from a fork is code its author controls, and it reaches the pool
the moment it enters the queue, whatever `NIFE_RUNNER_OWNED` says. **Do not arm or enqueue a
pull request from a fork** (GitHub shows it as from another repository). Re-push its reviewed
commits to a branch here and queue that, which pins what was read, as
[milestone 965 (gating CI rents more concurrency before it owns a runner)](../design/roadmap/0965-gating-ci-rents-more-concurrency-before-it-owns-a-runner.md)
describes.

## What runs on them

Stage 1 moves the x86_64 jobs that run only on `merge_group` or `push` and fit one slot (4 vCPU,
16 GiB, 50 GB disk, 90 minutes): the longest took 15 minutes on recent queue runs. They take the
switch below with the variable unset, so they stay hosted until a maintainer sets the variable to `true`.

- ci.yml: `swish-check-x86_64`, `bench`, `crate-probes`.
- verify.yml: the four `prove` shards.

These stay hosted on purpose:

- `fuzz-guests` (the rule in `helpers/fuzz-ci-selftest.sh`: a fuzz finding stays private) and the two
  `fuzz-shard` jobs (the same disclosure rule may cover them; they wait for a ruling).
- Every aarch64 and riscv64 guest job, and every job on an arm runner.
- The required summary and gate jobs, which last seconds, and every job that runs on a pull request.

Stage 2, later, moves the rest.

## The switch: `NIFE_RUNNER_OWNED`

The variable stays `false` until the strict canary (`canary-owned`) and the dry run
(`canary-owned-dry-run`) are both green on the pool. They must be green in one merge group or push
run, with no `FINDING` line in either summary. The proxy's refusal log for that run must show no
refusal except the runner's own background requests to two named hosts (below). "The canary"
below describes both. Flipping needs no review beyond that evidence. It is a reversible decision;
Rollback below undoes it.

The two named hosts are `pkg.github.com` and `copilot-proxy.githubusercontent.com`. The runner's
worker asks for both together, a fraction of a second apart, during the "Set up job" step of some
jobs. When GitHub's job message carries a list of connectivity-check URLs, the worker sends a
`GET` to each through the proxy and reports the result to GitHub as telemetry. The code is
`InitializeJob` in the runner's `src/Runner.Worker/JobExtension.cs`. A refusal costs the job
nothing, since the worker records the error and goes on. No runner setting turns the check off, and
no job needs either host, so both stay off the allowlist and their refusals are expected. Any other
refusal is "A blocked host" below.

The repository variable `NIFE_RUNNER_OWNED` chooses between the two pools. The value `true` selects
owned; anything else, including unset, selects hosted. A job that takes part in the switch writes:

```yaml
runs-on: ${{ vars.NIFE_RUNNER_OWNED == 'true' && 'nife-owned-x86_64' || 'ubuntu-26.04' }}
```

GitHub accepts an expression for a `runs-on` string, and `vars` is available there. The stage 1
jobs also run on a pull request that carries `full-ci` and on the dispatch that label starts. The
owned hosts refuse both. These jobs therefore use the event-aware form, which picks hosted on every
event but a merge group and a push to `main`:

```yaml
runs-on: ${{ vars.NIFE_RUNNER_OWNED == 'true' && (github.event_name == 'merge_group' || github.event_name == 'push') && 'nife-owned-x86_64' || 'ubuntu-26.04' }}
```

A job that has no pull request path, such as the canary, may instead carry
`if: github.event_name == 'merge_group' || github.event_name == 'push'` with the plain form.

Set it with `gh variable set NIFE_RUNNER_OWNED --repo nifeos/nife --body true`.

## The canary

The canary is monitoring, not a gate: nothing waits for it. A maintainer reads its summaries before
the flip to see whether the image is ready, and after it to see the pool's health. `.github/workflows/owned-runner-canary.yml` runs on
`merge_group` and on `push` to `main` only. It holds two probe jobs, the strict image checks
and the dry run on the owned side, then an isolation test:

- `canary-owned` on `[nife-owned-x86_64]`, with no `continue-on-error`, so a FINDING shows the job
  red. It is not a required check, so a failure cannot block the queue.
- `canary-hosted` on `ubuntu-26.04`, the reference.

Both probe jobs print the runner name, hostname and `id`, tests that `/dev/kvm` is writable, prints the
`qemu-system-x86_64` and `cargo` versions when present, and fetches `https://index.crates.io/config.json`
and `https://api.github.com` with the status code and time of each. It times the whole probe. The owned job
then checks the container's walls: a fetch with the proxy bypassed must fail, and no Docker, Podman
or containerd socket may exist. Each open wall prints a `FINDING` line and fails the job. The output
goes to the job summary. The workflow holds no secrets and `permissions: contents: read`.
Neither job name is a required check; do not add either to the ruleset.

### The strict image checks

`canary-owned` also checks out the repository, installs the pinned toolchain with
`helpers/rustup-retry.sh toolchain install`, and then runs `helpers/owned-image-probe.sh` for each
of four checks, through `helpers/owned-canary-image.sh`, which exits 1 for a failing probe or any
`FINDING` line (`helpers/owned-canary-image-selftest.sh` proves both ways). Any `FINDING` line fails the job, so a job image that the stage 1 jobs cannot use
shows red where an earlier canary showed green. The checks are:

- `tools`: `rustup` and `cargo` are on `PATH`.
- `toolchain`: the channel `rust-toolchain.toml` pins appears in `rustup toolchain list`, and
  `rustc -vV` and `cargo --version` run in the repository.
- `apt-proxy`: apt has an `Acquire::http::Proxy` or `Acquire::https::Proxy` setting that is not
  `DIRECT`, and `sudo` keeps the runner's proxy variables when the runner sets them.
- `pip`: a virtual environment's pip downloads `wheel` and `qemu.qmp` (the two packages QEMU's
  `configure` fetches) through the proxy.

### The dry run

`canary-owned-dry-run` runs on the owned label, `continue-on-error`, on the same two events as the
canary. It runs the setup steps of each job that the event-aware `runs-on` can place on the pool,
without the real work, and keeps going after a failed step so one run names everything missing:

- `crate-probes`: the pinned toolchain install.
- the `prove` shards: `script/kani-install`.
- `swish-check-x86_64` and `bench`: the QEMU cache restore and the decision `script/ci-qemu` would
  take, which is to skip on a hit and build on a miss. The workspace must be
  `/home/runner/work/nife/nife`, the layout where a cache saved by a hosted runner restores under
  `$HOME`. The dry run also fetches what a cold build fetches: `apt-get update`, the QEMU source
  tarball and pip's build dependencies.

It does not build QEMU or run a proof. A pass says the image can start each job's setup; the first
real queue runs after the flip are the evidence for the rest. When a job's setup steps change, the
dry run changes with them.

Every canary job ends with a `Resources` step that runs even after a failure and appends to the job
summary:

- wall time and `free -m`;
- the cgroup's peak memory (`memory.peak`, or `memory.current` and `memory.max` when the peak file
  is absent);
- `df -h` for `$HOME`, `/tmp` and the volume that holds the workspace (the slot volume);
- each fetch that did not return 200, by host.

The same text goes to the job log as well, because the job summary is not in the log API and a script
can read only the log.

### What the isolation test proves

An owned job starts from a clean container, so it sees none of an earlier job's files. Three jobs
test that claim. All three are `continue-on-error`, and none is a required check.

- `canary-owned-write` writes a marker file, `nife-canary-marker-<run_id>`, to `$HOME`, `/tmp` and
  `/etc`. The container is rootless, so it tries `sudo -n` when plain writing fails. A path it
  cannot write prints a `FINDING` line. That records a fact about the container and does not fail the
  job.
- `canary-owned-check` needs the writer and runs five instances in parallel (`fail-fast: false`) on
  the same label. Each prints its runner name, confirms that none of the three markers exists, and
  confirms that `$HOME` holds no `nife-canary-marker-*` file. A `LEAK` line fails that instance.
- `canary-owned-isolation` runs on a hosted runner, reads the run's jobs from the API and states
  whether any check instance ran on the writer's slot. Runner names end in `-sNN-<hex>`. The hex is new for every
  job, so the slot is the name without it, and equal slots mean one slot. The job reports how many
  of the check instances ran on the writer's slot.

Reading the summary: a `LEAK` line in any instance means a slot kept a file between jobs. A leak
can only show where the writer ran, so a clean run says something only when the isolation job says
`writer slot seen again: yes`; a `no` says nothing about slot reuse.

## A blocked host

If a step fails to reach a host, the proxy has refused it. Name the host, the step
that wanted it and the run URL, in a comment on the pull request that introduced the step or in an
issue labeled `ci`. The maintainers who operate the pool add the host to the allowlist. Do not work
around the proxy and do not route a pull request job to a hosted runner to avoid it. The hosted
twin is unaffected, so the rollback below keeps the queue moving until the maintainers add the host.

## A pool that is down

A job sent to the owned label waits for a runner, and `timeout-minutes` does not start until one
takes it. Two required checks, `bench` and `swish-check-x86_64`, and the `prove` shards behind
`verify (Kani proofs)`, sit on the switch, so with the pool down and the variable `true` every
merge group waits. The merge queue's 240-minute check timeout then removes the entry, and GitHub
cancels the waiting job after 24 hours. Nothing alarms sooner (see BUGS). A queue with entries
in `AWAITING_CHECKS` and owned jobs in `queued` is the sign; roll back.

The canary's owned jobs and the dry run name the label directly, so they wait on a down pool even with the
variable `false`. They are not required checks and do not hold the queue.

## Rollback

Set the variable to `false` (`gh variable set NIFE_RUNNER_OWNED --repo nifeos/nife --body false`).
It needs no pull request and no review, and works with the pool down. Jobs that start after it use
`ubuntu-26.04` for every job on the switch. A job already waiting on the owned label keeps
waiting: dequeue the pull request and enqueue it again, which builds a new merge group whose run
reads the new value. Say why in the pull
request or issue that explains the rollback.

## The lint

`helpers/owned_runner_check.py`, run by `script/lint`, judges every job whose `runs-on`, matrix
or reusable-workflow input names `nife-owned-x86_64` or reads `NIFE_RUNNER_OWNED`, and fails the
tree when:

- the job's workflow lists any trigger but `merge_group` and `push` (a pull request, a
  `pull_request_target`, `workflow_run`, `workflow_dispatch`, `workflow_call`, `issue_comment`,
  `schedule` or any other);
- the job lacks a job-level `if:` that names only `github.event_name == 'merge_group'` or
  `'push'`;
- the workflow's `push` is not limited to `branches: [main]`, or takes tags; or
- the job sets no `timeout-minutes`.

The event-aware `runs-on` above, character for character with a hosted label of the shape
`ubuntu-NN.NN`, is exempt from the first two, since it resolves hosted on every other event. Any
near miss is judged by all four: a clause that names `pull_request`, no clause, no parentheses, or
an owned fallback.

The same helper fails any job in a `pull_request_target` or `workflow_run` workflow that names the
pull request's head (`github.event.pull_request.head.sha`, `github.head_ref`,
`github.event.workflow_run.head_sha`, `refs/pull/`, `gh pr checkout`), on any runner: those events
hold the base repository's token while the head is the fork's code. No workflow here uses
`pull_request_target` today.

Its `--selftest` plants each of those failures and each near miss, requires each to fail, and
requires the good shapes to pass.

## BUGS

- The lint reads the `if:` text and does not evaluate it; `merge_group || true` passes.
- A `runs-on` built from a matrix or an input is read as text, not resolved.
- The lint cannot bind a merge group, which runs the pull request's own workflow files. Keeping a
  fork's pull request out of the queue is a rule for whoever arms it, in "Who can reach the pool".
- Nothing alarms on a down pool before the queue's 240-minute timeout. `helpers/merge-drain.sh`'s
  stuck-head check waits for every run of the group to finish, and a run with a job waiting on a
  runner never finishes.
- The canary's totals are a person's reading of the summaries; nothing totals them.
