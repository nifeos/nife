---
status: BUILT
built: 2026-11-19
promoted_from: the-merge-queue-is-sized-to-the-runner-pool
raised: 2026-11-04
milestone_dependencies: 965
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1042. The merge queue is sized to the runner pool

COE action item: notes/coes/2026-11-03-merge-queue-backlog.md, action 5.

Raised by lane/coe-merge-queue-2026-11-03 on 2026-11-04 (UTC), from the correction of error for the
Pacific day of 2026-11-03 (`notes/coes/2026-11-03-merge-queue-backlog.md`). This milestone changes no setting: the ruleset and the repository variable
are the architect's.

Reuse: the ruleset's merge queue parameters, the `NIFE_RUNNER_OWNED` switch of `notes/owned-runners.md`,
and milestone 965 (gating CI rents more concurrency before it owns a runner). No new tool: the wait measure extends `helpers/ci_job_times.py` and `script/metrics`.

## Background

Ruleset 19596094 builds up to 5 queue entries at once, merges up to 5 together, requires every
entry green (`ALLGREEN`), and waits one minute before it builds a group of one. Each entry runs its own
group, so the ceiling is about 5 merges per group wall time.

On 2026-11-03 a group took a median 74.5 minutes while the hosted pool was full, and 18.2 once the
heavy gates left the pull request (#3176). A group spends about 200 job minutes, roughly 11 jobs at
once, so 5 groups need about 55 of the 60 hosted slots. The build window and the pool are one
budget: a wider window with the same pool makes every group slower.

## The ruling (2026-11-19, UTC)

The queue builds up to 5 entries at once and groups with `ALLGREEN`, one entry to a group. Each
trigger below moves one setting, and nothing else moves it:

- `NIFE_RUNNER_OWNED` becomes `true` once the owned-runner canary evidence is in. It moves the stage
  1 x86_64 jobs, about a fifth of a group's job minutes, to the 5 owned slots. The architect sets the
  variable.
- `max_entries_to_build` rises to 8 only after a measured day shows the median hosted job waiting
  under one minute with 5 groups building. `ci-job-waits.svg` in `notes/project-metrics.md` is that
  measure.
- The group size stays one entry until `fuzz-guests` stops failing inside merge groups. Then a larger
  group (`HEADGREEN`, or a larger `min_entries_to_merge` with a longer wait) gets a new measurement.
- Milestone 965 (gating CI rents more concurrency before it owns a runner) decides the plan upgrade
  on its own.

`notes/merge-queue.md`'s "The queue's own settings" and `notes/merge-queue/queue-sizing.md` hold the
settings, their triggers and the measurement of 2026-11-19.

## The week the ruling rests on

The 7 days before 2026-11-11 (UTC):

- 396 merge-group builds, 25% of them red.
- The queue removed 28% of its entries before they merged: 54 for a merge conflict, 44 for a failed
  check and 25 for an invalid merge commit.
- The `lint` job failed in 53 merge-group builds, although `lint` gates every pull request before
  it enters the queue. The proposal
  [lint fails in the queue after passing on the pull request](proposals/lint-fails-in-the-queue-after-passing-on-the-pull-request.md)
  has the causes and the fixes.

On 2026-11-19 the queue took 28 entries, removed 1 and merged 27, with a median of 1.1 hours from
entry to merge. Recent merge-group runs took 10 to 15 minutes, and the weekly CI build had a median
of 19.0 minutes and a p90 of 34.0 in 2026W47.

## Refused

- Raising `max_entries_to_build` to 10: the pool cannot run 10 groups, so each would take longer and
  the ceiling would not move.
- Grouping several entries into one build now: one red entry fails the group and every entry in it
  rebuilds, and the week measured had 7 red group runs in 39.
- Moving more of the heavy set off the group, to run on `push` to `main` only: it lets a regression
  reach `main`, which is the queue's whole purpose.

## Done when

- The settings chosen are in `notes/merge-queue.md`'s "The queue's own settings", with the
  measurement that justified them.
- The record keeps the median hosted job wait and the share of hosted jobs that waited over 5
  minutes, per week (`notes/project-metrics/ci-job-wait-days.csv`, which `helpers/ci_job_times.py`
  writes daily), and `ci-job-waits.svg` charts the median.

## Follow-on

- **Done.** `notes/merge-queue.md`'s "The queue's own settings" and `notes/merge-queue/queue-sizing.md`
  hold the settings and their triggers, with the measurement of 2026-11-19 (UTC).
- **Done.** `helpers/ci_job_times.py` records the hosted jobs' wait for a runner each day, and
  `script/metrics` charts the weekly median against the one-minute line as `ci-job-waits.svg`, with
  the share over 5 minutes in the `ci-job-waits` measure file.
- **Recorded.** The lint failures inside merge groups and the canary trigger the rule above. They
  are not work of this milestone, because the proposal about lint in the queue and
  `notes/owned-runners.md` own them. The two settings they gate are `NIFE_RUNNER_OWNED` and then the
  window of 8.

## Index row

The merge queue builds up to five groups on a pool that fits about five. Proposed: choose the build window, the group size and the owned-runner switch together, against a measured day.
