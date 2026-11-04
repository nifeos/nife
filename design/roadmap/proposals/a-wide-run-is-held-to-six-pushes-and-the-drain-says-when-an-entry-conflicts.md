---
status: PROPOSED
raised: 2026-11-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# A wide run is held to six pushes and the drain says when an entry conflicts

Raised by lane/queue-conflict-precheck on 2026-11-04 (UTC), while building the pre-arm check of
[an entry that conflicts with the queue is caught before it](an-entry-that-conflicts-with-the-queue-is-caught-before-it.md)
(COE action 6, `notes/coes/2026-11-03-merge-queue-backlog.md`). That lane built the pre-arm check,
`script/queue-conflicts`, and the launch check for shared scope files. Two parts of the original
proposal were left, and they need a design the first two do not.

## Background

- The drain does not yet use the check. On 2026-11-03 GitHub kept 13 entries it knew were UNMERGEABLE
  in the queue until a merge moved `main`. The check can find them: a drain pass that runs
  `helpers/queue_conflicts.py check --pr N --advisory` on each queued entry could comment once on
  one that conflicts with the entries ahead, naming the files, so its lane learns without waiting
  for the removal. What is open is where the pass runs (the drain is a hosted workflow with a
  checkout, and each check fetches heads) and how it avoids a second comment.
- Nothing caps how many pull requests one wide run pushes at once. On 2026-11-03, 25 naming pull
  requests were marked ready within seventy minutes and filled the hosted pool with their own
  checks. The original proposal recommends a cap of 6. It needs a counter that lanes of one run can
  all see: the run's open non-draft pull requests that have not entered the queue, read from
  GitHub, refused at `gh pr ready` time by a helper the wide-run brief names.

## What it builds

1. The drain comments once on a queued entry that conflicts with the entries ahead of it, using
   `helpers/queue_conflicts.py`.
2. A helper that a lane in a wide run calls before `gh pr ready`, which counts the run's ready pull
   requests not yet in the queue (the run is named by a label or branch prefix) and refuses the
   seventh.

## Done when

- A drain fixture with a conflicting queued entry produces one comment, and a second pass produces
  none.
- A fixture run with six ready pull requests refuses a seventh and allows it once one has entered the
  queue.
