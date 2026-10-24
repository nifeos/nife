# Measurement: hosted CI during the milestone-929 run

An appendix to
[gating-ci-rents-more-concurrency-before-it-owns-a-runner.md](../gating-ci-rents-more-concurrency-before-it-owns-a-runner.md).
Every figure was read from the GitHub API and `git` on 2026-10-24 between 03:35 and 03:50 UTC. All
times are UTC.

## Method

- Runs: `gh api repos/nifeos/nife/actions/runs?created=<hour>` for each hour from 2026-10-23 00:00
  to 2026-10-24 03:59, 4,106 runs.
- Jobs: `gh api repos/nifeos/nife/actions/runs/<id>/jobs?filter=all` for the 1,393 runs of `CI`,
  `verify`, `codeql` and `mutation gate`, 14,803 jobs. Skipped jobs carry a runner label but never
  take a slot, so they are dropped.
- Wait is a job's `started_at` minus its `created_at`. A job is created when its `needs:` finish,
  so a run's waits add up across its stages.
- A full `CI` run is one that executed more than 15 jobs. The prose-only and draft paths run fewer.
- Merges: `git log --first-parent origin/main --since=2026-10-23T00:00:00Z`.

## Queue snapshots

| time | queued runs | in-progress runs | source |
|---|---|---|---|
| 01:25 | 124 | 4 | maintainer snapshot |
| 03:15 | 151 | 6 | maintainer snapshot |
| 03:32 | 249 | 3 | maintainer snapshot |
| 03:35:31 | 234 | 4 | this lane |

At 03:36:13, the jobs of every queued or in-progress run:

| label | in progress | queued |
|---|---|---|
| `ubuntu-24.04` | 20 | 257 |
| `ubuntu-24.04-arm` | 40 | 173 |
| total | 60 | 430 |

One in-progress run, 39698476042, dates from 2026-10-22 and is a stale entry, not a running job.

## Concurrency, 00:00 to 03:30 on 10-24

Heavy-workflow jobs running, sampled each minute:

| label | median | p90 | max |
|---|---|---|---|
| both | 59 | 71 | 92 |
| `ubuntu-24.04` | 29 | 36 | 54 |
| `ubuntu-24.04-arm` | 32 | 37 | 42 |

The excursions above 60 are unexplained. Job timestamps are coarse, and GitHub's cap may not be a
hard ceiling; the per-minute median and the 03:36 snapshot both sit at 60.

Job-minutes in the window, 12,335 of 12,600 possible at 60 slots:

| workflow and event | job-minutes |
|---|---|
| `CI`, pull request | 7,358 |
| `CI`, merge group | 2,084 |
| `verify`, pull request | 1,513 |
| `codeql`, pull request | 784 |
| `codeql`, push | 210 |
| `verify`, merge group | 204 |
| `mutation gate`, pull request | 160 |
| `mutation gate`, merge group | 15 |

## Wait for a runner, by hour

Executed jobs only, minutes from created to started:

| hour | event | jobs | median | p90 | max |
|---|---|---|---|---|---|
| 10-23 08 | pull request | 1,258 | 0.6 | 8.6 | 17.4 |
| 10-23 09 | pull request | 1,780 | 6.1 | 20.2 | 39.2 |
| 10-23 09 | merge group | 71 | 13.7 | 17.9 | 20.1 |
| 10-23 14 | merge group | 229 | 0.1 | 0.8 | 2.2 |
| 10-23 22 | merge group | 32 | 0.0 | 0.1 | 0.1 |
| 10-24 00 | pull request | 1,313 | 12.8 | 23.3 | 34.8 |
| 10-24 01 | pull request | 1,111 | 5.1 | 13.6 | 24.2 |
| 10-24 01 | merge group | 297 | 9.1 | 15.2 | 21.7 |
| 10-24 02 | pull request | 760 | 10.3 | 19.1 | 27.1 |
| 10-24 02 | merge group | 206 | 10.3 | 20.1 | 28.2 |
| 10-24 03 | pull request | 533 | 0.0 | 18.1 | 24.5 |
| 10-24 03 | merge group | 219 | 10.2 | 21.4 | 29.3 |

The oldest queued `CI` run at 03:35, 40054840698, was created 02:36:00. Its draft gate started at
02:47:13 and its suite at 02:47:29.

## What one run costs

| run | n | jobs | job-minutes | wall, median | wall, worst |
|---|---|---|---|---|---|
| full `CI`, merge group, 10-23 13:00 to 23:59 | 6 | 26 | 130 | 14 | 15 |
| full `CI`, merge group, 10-24 00:00 to 04:00 | 12 | 26 | 128 | 38 | 53 |
| full `CI`, pull request, 10-24 00:00 to 04:00 | 47 | 26 | 127 | 46 | 69 |
| `verify` with proofs, either event | 34 | over 4 | 54 | 51 | |

A full `CI` run by label: 17 jobs and 44 job-minutes on `ubuntu-24.04`, 9 jobs and 84 job-minutes
on `ubuntu-24.04-arm`. `verify`'s proving runs spend 49 job-minutes on `ubuntu-24.04` and 3 on
arm64.

The longest jobs of a full `CI` run, median minutes:

| job | label | minutes |
|---|---|---|
| cpu matrix shard (rv64 sifive-u54 rva22s64) | arm64 | 13.7 |
| test shard (host crates, aarch64 and riscv64) | arm64 | 12.5 |
| test shard (x86_64) | arm64 | 12.3 |
| fuzz | x86_64 | 11.9 |
| bench (icount regression tripwire) | x86_64 | 10.8 |
| cpu matrix shard (rva23s64 thead-c906) | arm64 | 10.6 |
| swish-check (three architectures) | arm64 | 10.6 |

## Repeat cost

- Full `CI` pull-request runs since 10-23 00:00: 149, on 53 branches, a median 3 per branch.
  Outcomes: 54 success, 53 cancelled by a later push, 31 failure, 11 still queued.
- Merge-group `CI` runs: 46 for 30 pull requests. Nine pull requests had more than one. Five of
  those were rebuilt between 08:20 and 14:29 on 10-23, while a new advisory held the supply-chain
  check red on every branch (#2614). Four were rebuilt at 03:08 on 10-24, when the queue re-formed
  its groups.

## Merges on `main`

24 first-parent merges from 2026-10-23 00:00 to 2026-10-24 03:36. On 10-24: #2600 at 00:46:11 and
#2621 at 00:52:26, then twelve from #2592 at 01:06:33 to #2644 at 01:55:25, then none through
03:36.

The queue at 03:36: 13 entries, five awaiting checks (#2643, enqueued 01:59:29; #2597; #2641;
#2640; #2595) and eight queued behind them. Its ruleset builds at most five entries at once, with a
240-minute check timeout.

## Repository settings read

- `gh api repos/nifeos/nife/actions/permissions/fork-pr-contributor-approval`:
  `all_external_contributors`.
- `gh api repos/nifeos/nife/actions/permissions/workflow`: default permissions `read`, cannot
  approve reviews.
- `gh api repos/nifeos/nife/actions/permissions`: `sha_pinning_required: true`.
- `gh pr list --state all --limit 1000`: no cross-repository pull request.
- The org plan reads `team`.

## BUGS

- One night of one milestone, whose diffs were comment-only. A code milestone runs `verify`'s
  proofs far more often, which moves more of the load onto x86_64.
- The 85% idle figure for the owned host is the brief's, from the same run, not re-measured here.
- Job waits understate a run's wait, because a run's stages wait in turn.
