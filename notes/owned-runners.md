# Owned runners: what they are, how to flip them, and how the canary judges them

*Name: provisional, minted 2026-11-03 (UTC) by lane/owned-runners-canary.*

The variable `NIFE_RUNNER_OWNED`, `helpers/owned_runner_check.py` and the canary workflow's job names are provisional too.

The organization runs a small pool of its own CI runners next to GitHub's hosted ones. This page is
the runbook: what the pool is, what may run on it, how a repository maintainer switches work between
owned and hosted, how the pool earns its place, and what to do when it misbehaves. The maintainers keep
machine-local facts (which hosts, where they sit) off-tree, and this page does not need them.

## What the owned runners are

- Five runners in the runner group `nife-owned` of the `nifeos` organization, all carrying the label
  `nife-owned-x86_64`.
- Each job gets a fresh rootless container on an owned host, then the runner exits: just-in-time and
  ephemeral, so no job sees another's files.
- Each container has 4 vCPUs, 16 GiB of memory, `/dev/kvm`, a 50 GB volume and a 90-minute limit.
- The image is Ubuntu 26.04 with runner 2.338.2 and `qemu-system-x86` installed.
- Egress goes through an allowlist proxy. The proxy refuses a host not on the list and reports it to the
  maintainers, who add it (see "A blocked host" below).

## The admission rule

A host-side hook admits exactly two kinds of job and turns the rest away:

- `merge_group` on a ref under `gh-readonly-queue/main/`
- `push` to `main`

The hook turns away a `pull_request` job at the door, and a `workflow_dispatch` job too. A fork's
pull request therefore never runs on owned hardware: its code belongs to the author, and the hook
says no before a container starts. A turned-away job does not fail; it waits for a runner that
will never take it and blocks whatever needs it. For that reason `script/lint` rejects the tree that would send one there
(see "The lint" below).

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

`.github/workflows/owned-runner-canary.yml` runs on `merge_group` and on `push` to `main` only.
It holds two jobs with identical steps, then an isolation test:

- `canary-owned` on `[nife-owned-x86_64]`, with `continue-on-error: true`, so a failure cannot
  block the queue.
- `canary-hosted` on `ubuntu-26.04`, the reference.

Each prints the runner name, hostname and `id`, tests that `/dev/kvm` is writable, prints the
`qemu-system-x86_64` and `cargo` versions when present, fetches `https://index.crates.io/config.json`
and `https://api.github.com` with the status code and time of each, and times the whole probe. The
output goes to the job summary. The workflow holds no secrets and `permissions: contents: read`.
Neither job name is a required check; do not add either to the ruleset.

Every canary job ends with a `Resources` step that runs even after a failure and appends to the job
summary:

- wall time and `free -m`;
- the cgroup's peak memory (`memory.peak`, or `memory.current` and `memory.max` when the peak file
  is absent);
- `df -h` for `$HOME`, `/tmp` and the volume that holds the workspace (the slot volume);
- each fetch that did not return 200, by host.

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
  whether any check instance ran on the writer's slot. Runner names end in `-sNN-<hex>`, and equal
  names mean one slot.

Reading the summary: no `LEAK` line in any instance is the pass. A leak can only show where the
writer ran, so the test counts only when the isolation job says `writer slot seen again: yes`.
A `no` means this run proved nothing about slot reuse; read the next run. A `FINDING` for `/etc`
is the expected result of a rootless container.

### What counts as a pass

A canary week passes when all of these hold across every queue run in seven days (UTC):

1. `canary-owned` concludes the same as `canary-hosted` on every run. A run where the owned job
   fails and the hosted one passes is a miss, whatever the cause.
2. The two curl lines return a success status on both sides on every run.
3. `/dev/kvm` is writable on the owned side on every run.
4. The owned job's total time is no more than the hosted job's on the median run. The comparison is
   a reading of the summaries; queue time before the job starts is a separate number and does not
   count against the pool.

Fewer than twenty runs in the week extends the week; a week with no run proves nothing.

### The first run

After the workflow lands, its first `merge_group` or `push` run is the evidence the pool was
configured as described. Open the run, read the "Set up job" step of `canary-owned`, and record its
runner name and machine name lines with the run's URL for the maintainers who operate the pool.

## A blocked host

If a step fails to reach a host, the proxy has refused it. Name the host, the step
that wanted it and the run URL, in a comment on the pull request that introduced the step or in an
issue labeled `ci`. The maintainers who operate the pool add the host to the allowlist. Do not work
around the proxy and do not route a pull request job to a hosted runner to avoid it. The hosted
twin is unaffected, so the rollback below keeps the queue moving until the maintainers add the host.

## Rollback

Set the variable to `false` (`gh variable set NIFE_RUNNER_OWNED --repo nifeos/nife --body false`).
The next queue run uses `ubuntu-26.04` for every job on the switch. Nothing else changes, and
re-running a queued job picks up the new value. A repository maintainer may do this without a
review; say so in the pull request or issue that explains why.

## The lint

`helpers/owned_runner_check.py`, run by `script/lint`, fails the tree when:

- a workflow whose `on:` lists `pull_request` or `pull_request_target` has any job whose `runs-on`
  names `nife-owned-x86_64` or reads `NIFE_RUNNER_OWNED`; or
- any job with such a `runs-on` lacks a job-level `if:` that names only `github.event_name ==
  'merge_group'` or `'push'`.

The event-aware `runs-on` above passes both rules, character for character, with a hosted label of
the shape `ubuntu-NN.NN`. Its event clause sits inside `runs-on`, so the job needs no `if:` and its
workflow may take pull requests. Any near miss fails: a clause that names `pull_request`, no clause,
no parentheses, or an owned fallback.

Its `--selftest` plants a pull request workflow, the switch under `pull_request_target`, a job with
no guard and a guard that names `pull_request`, and requires each to fail. It also plants the
event-aware form under `pull_request` and requires it to pass. Four near misses of it must each fail. Pointing the helper at a
copy of the canary workflow with `pull_request:` added to `on:`, or with the `if:` lines removed,
exits 1 and names the job.

## BUGS

- The lint reads the `if:` text and does not evaluate it; `merge_group || true` passes.
- A `runs-on` that comes from a matrix value or a reusable workflow's input is not resolved.
- The canary compares conclusions and times by a person's reading of the summaries. Nothing totals a
  week automatically.
