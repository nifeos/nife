---
status: NOT-STARTED
promoted_from: the-merge-queue-is-sized-to-the-runner-pool
raised: 2026-11-04
milestone_dependencies: 965
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1042. The merge queue is sized to the runner pool

COE action item: notes/coes/2026-11-03-merge-queue-backlog.md, action 5.

Raised by lane/coe-merge-queue-2026-11-03 on 2026-11-04 (UTC), from the correction of error for the
Pacific day of 2026-11-03 (`notes/coes/2026-11-03-merge-queue-backlog.md`). This milestone changes no setting: the ruleset and the repository variable
are the architect's.

Reuse: the ruleset's merge queue parameters, the `NIFE_RUNNER_OWNED` switch of `notes/owned-runners.md`,
and milestone 965 (gating CI rents more concurrency before it owns a runner). No new tool.

## Background

Ruleset 19596094 builds up to 5 queue entries at once, merges up to 5 together, requires every
entry green (`ALLGREEN`), and waits one minute before it builds a group of one. Each entry runs its own
group, so the ceiling is about 5 merges per group wall time.

On 2026-11-03 a group took a median 74.5 minutes while the hosted pool was full, and 18.2 once the
heavy gates left the pull request (#3176). A group spends about 200 job minutes, roughly 11 jobs at
once, so 5 groups need about 55 of the 60 hosted slots. The build window and the pool are one
budget: a wider window with the same pool makes every group slower.

## The question

Which settings, in which order? Recommendation first.

- (a) Recommended. Leave `max_entries_to_build` at 5 and the grouping at `ALLGREEN`. Set
  `NIFE_RUNNER_OWNED` to `true` once the canary has passed its acceptance items, which moves the
  stage 1 x86_64 jobs (about a fifth of a group's job minutes) to the 5 owned slots. Then decide
  milestone 965 (gating CI rents more concurrency before it owns a runner). Raise the build window to
  8 only after a measured day shows hosted jobs waiting under a minute at the median with 5 groups
  building.
- (b) Raise `max_entries_to_build` to 10 now. Refused for now: the pool cannot run 10 groups, so each
  would take longer and the ceiling would not move.
- (c) Group several entries into one build (`HEADGREEN`, or a larger `min_entries_to_merge` with a
  longer wait). A green group of 5 then costs one build, not 5. Refused for now: one red entry
  fails the group and every entry in it rebuilds, and the day had 7 red group runs in 39. Worth
  measuring again once `fuzz-guests` stops failing in groups.
- (d) Move more of the heavy set off the group (run some gates on `push` to `main` only). Refused:
  it lets a regression reach `main`, which is the queue's whole purpose.

## Done when

- The settings chosen are in `notes/merge-queue.md`'s "The queue's own settings", with the day's
  measurement that justified them.
- A week of queue data shows the median group wall time and the share of hosted jobs that waited
  over 5 minutes, before and after.

## Index row

The merge queue builds up to five groups on a pool that fits about five. Proposed: choose the build window, the group size and the owned-runner switch together, against a measured day.
