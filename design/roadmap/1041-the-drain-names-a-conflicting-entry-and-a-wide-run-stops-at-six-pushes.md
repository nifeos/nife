---
status: NOT-STARTED
promoted_from: the-drain-names-a-conflicting-entry-and-a-wide-run-stops-at-six-pushes
raised: 2026-11-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1041. The drain names a conflicting entry and a wide run stops at six pushes

Raised by lane/queue-conflict-precheck on 2026-11-04 (UTC), while it built the pre-arm check for
[the pre-arm check proposal](proposals/an-entry-that-conflicts-with-the-queue-is-caught-before-it.md)
(COE action 6, `notes/coes/2026-11-03-merge-queue-backlog.md`). That lane built
`script/queue-conflicts` and the launch check for shared scope files. Two parts of the original
proposal remain, and each needs a design the first two did not.

Reuse: `helpers/queue_conflicts.py` for the drain's check, the drain's GraphQL query for the
queue, and `helpers/heavy_slot.py` as the model for a counted slot.

## Background

- The drain does not use the check yet. On 2026-11-03 GitHub kept 13 entries it knew were
  UNMERGEABLE in the queue until a merge moved `main`. A drain pass could run
  `helpers/queue_conflicts.py check --pr N --advisory` on each queued entry and comment once on a
  conflicting one, naming the files. Open: where the pass runs, since each check fetches heads, and
  how it avoids a second comment.
- Nothing caps how many pull requests one wide run pushes at once. On 2026-11-03 a run marked 25
  naming pull requests ready within seventy minutes, and their checks filled the hosted pool. The
  original proposal recommends a cap of 6. It needs a counter every lane of the run can see: the
  run's open, ready pull requests that have not entered the queue, read from GitHub.

## What it builds

1. A drain pass that comments once on a queued entry conflicting with the entries ahead of it.
2. A helper that a wide-run lane calls before `gh pr ready`. It counts the run's ready pull requests
   not yet in the queue (a label or branch prefix names the run) and refuses the seventh.

## Done when

- A drain fixture with a conflicting queued entry produces one comment, and a second pass produces
  none.
- A fixture run with six ready pull requests refuses a seventh, and allows it once one has entered
  the queue.

## Index row

The merge drain does not use the pre-arm conflict check, and nothing caps how many pull requests one wide run readies at once. Proposed: a drain pass that comments once on a conflicting entry, and a counted limit of six.
