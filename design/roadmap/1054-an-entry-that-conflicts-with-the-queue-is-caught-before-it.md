---
status: NOT-STARTED
promoted_from: an-entry-that-conflicts-with-the-queue-is-caught-before-it
raised: 2026-11-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1054. An entry that conflicts with the queue is caught before it

COE action item: notes/coes/2026-11-03-merge-queue-backlog.md, action 6.

Raised by lane/coe-merge-queue-2026-11-03 on 2026-11-04 (UTC), from the correction of error for the
Pacific day of 2026-11-03 (`notes/coes/2026-11-03-merge-queue-backlog.md`). 
Reuse: `git merge-tree --write-tree`, the GraphQL `mergeQueue` entries the drain already reads in
`helpers/merge-drain.sh`, and the scope-file partition of `briefs/run-a-milestone-wide.md`.

## Background

On 2026-11-03 the queue removed 24 entries for merge conflicts, 14 of them in one minute at 22:58
UTC. Re-made with `git merge-tree`, 11 of the 21 tested conflict on one shared note. GitHub kept 13
entries it already knew were UNMERGEABLE in the queue at 21:54 and removed them only when a merge
moved `main`. Each removal costs the pull request its place and a push, and the push costs a full
run of its checks.

The rule that should have prevented it exists as prose: the maintainer skill bounds lane count by
the collision surface, and the wide-run brief partitions lanes by scope file before the first
claim. Nothing reads a planned set of lanes, and nothing reads the queue before an arm.

## What it builds

1. A pre-arm check. Before `gh pr merge --auto`, merge the pull request's head with `git merge-tree`
   against the tip a group would build on. That tip is `main` plus every entry ahead in the queue. On a
   conflict, refuse to arm and name the files and the entries ahead that touch them. The same check
   runs in the drain, which comments once on a queued entry that has already gone UNMERGEABLE.
2. A launch check for wide runs. Given the scope files of a planned set of lanes, list every file
   two lanes would both write, generated records included (a names note, a baseline). Refuse the
   launch until each shared file has one owner or a layout that gives each lane its own lines.
3. A cap on concurrent pushes from one wide run (recommended: 6), so that a burst of ready pull
   requests does not fill the hosted pool with their own checks.

## Options considered

- Rely on #3181's layout alone. It removes the names note's shared lines, but not shared source
  files: two mobile-storage groups still conflict on one module.
- Serialize all naming lanes. Refused: most groups touch disjoint files, and the check finds the ones
  that do not.

## Done when

- The pre-arm check refuses a fixture pull request that conflicts with a queued entry, and passes one
  that does not, under `script/lint`.
- A wide run's launch names its shared files before the first claim.

## Index row

A pull request that conflicts with an entry ahead of it is armed anyway and removed later at the cost of a full run. This checks the conflict before arming and before a wide run launches.
