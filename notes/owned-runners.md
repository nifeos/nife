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

Stage 1 moves the twelve x86_64 jobs that run on `merge_group` or `push` and need `/dev/kvm`.
Stage 2, later, moves the rest. Until a workflow switches its
`runs-on`, nothing but the canary uses the pool.

## The switch: `NIFE_RUNNER_OWNED`

The repository variable `NIFE_RUNNER_OWNED` chooses between the two pools. The value `true` selects
owned; anything else, including unset, selects hosted. A job that takes part in the switch writes:

```yaml
runs-on: ${{ vars.NIFE_RUNNER_OWNED == 'true' && 'nife-owned-x86_64' || 'ubuntu-26.04' }}
```

GitHub accepts an expression for a `runs-on` string, and `vars` is available there. The job also
carries `if: github.event_name == 'merge_group' || github.event_name == 'push'`, because the owned
side of the expression can never serve a pull request.

Set it with `gh variable set NIFE_RUNNER_OWNED --repo nifeos/nife --body true`.

## The canary

`.github/workflows/owned-runner-canary.yml` runs on `merge_group` and on `push` to `main` only.
It holds two jobs with identical steps:

- `canary-owned` on `[nife-owned-x86_64]`, with `continue-on-error: true`, so a failure cannot
  block the queue.
- `canary-hosted` on `ubuntu-26.04`, the reference.

Each prints the runner name, hostname and `id`, tests that `/dev/kvm` is writable, prints the
`qemu-system-x86_64` and `cargo` versions when present, fetches `https://index.crates.io/config.json`
and `https://api.github.com` with the status code and time of each, and times the whole probe. The
output goes to the job summary. The workflow holds no secrets and `permissions: contents: read`.
Neither job name is a required check; do not add either to the ruleset.

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
issue labelled `ci`. The maintainers who operate the pool add the host to the allowlist. Do not work
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

Its `--selftest` plants a pull request workflow, the switch under `pull_request_target`, a job with
no guard and a guard that names `pull_request`, and requires each to fail. Pointing the helper at a
copy of the canary workflow with `pull_request:` added to `on:`, or with the `if:` lines removed,
exits 1 and names the job.

## BUGS

- The lint reads the `if:` text and does not evaluate it; `merge_group || true` passes.
- A `runs-on` that comes from a matrix value or a reusable workflow's input is not resolved.
- The canary compares conclusions and times by a person's reading of the summaries. Nothing totals a
  week automatically.
