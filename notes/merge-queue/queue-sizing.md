# The queue's build window, grouping and runner switch

[Milestone 1042](../../design/roadmap/1042-the-merge-queue-is-sized-to-the-runner-pool.md) answers COE action item 5 of
[the merge queue backlog](../coes/2026-11-03-merge-queue-backlog.md). The rule in force is below.
[`notes/merge-queue.md`](../merge-queue.md) holds the summary under "The queue's own settings".

A group runs about 11 jobs at once, so 5 groups need about 55 of the 60 hosted slots. The build
window and the pool are one budget: a wider window on the same pool makes every group slower.

| setting | value | what moves it |
|---|---|---|
| `max_entries_to_build` | 5 | It rises to 8 only after a measured day shows the median hosted job waiting under one minute with 5 groups building. |
| grouping | `ALLGREEN`, one entry to a group | A larger group waits until `fuzz-guests` stops failing inside merge groups, since one red entry fails every entry in its group. |
| `NIFE_RUNNER_OWNED` | `true` once the owned-runner canary evidence is in | The architect sets the variable; [owned-runners.md](../owned-runners.md) has the canary. |

Milestone 965 (gating CI rents more concurrency before it owns a runner) decides the plan upgrade
separately.

## The measure the window reads

`ci-job-waits.svg` in [project-metrics.md](../project-metrics.md) charts the weekly median wait of a
hosted merge-group job, creation to start, against the one-minute line. The share of jobs that
waited over five minutes sits beside it in `notes/project-metrics/ci-job-waits.csv`.

## The measurement of 2026-11-19 (UTC)

The queue took 28 entries: it removed 1 and merged 27, with a median of 1.1 hours from entry to
merge. Recent merge-group runs took 10 to 15 minutes. The weekly CI build, first job start to last
job end, had a median of 19.0 minutes and a p90 of 34.0 in 2026W47.
