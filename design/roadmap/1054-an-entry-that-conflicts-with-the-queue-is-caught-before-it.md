---
status: BUILT
promoted_from: an-entry-that-conflicts-with-the-queue-is-caught-before-it
raised: 2026-11-04
built: 2026-11-10
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

## What exists

1. The pre-arm check. `script/queue-conflicts [--pr N]` (`cmd_check` and `evaluate` in
   `helpers/queue_conflicts.py`) merges the pull request's head with `git merge-tree` against `main`
   plus every entry ahead of it in the queue. It works in a scratch repository that borrows the
   caller's objects, and names each conflicting file and the entries ahead that changed it. Exit 0
   clean, 1 conflict, 2 could not tell. `script/push` runs it in `--advisory` mode after a confirmed
   push. The notes are `notes/merge-queue/conflict-precheck.md`.
2. The launch check for wide runs. `script/queue-conflicts shared SCOPE...` (`cmd_shared` and
   `shared_paths`) lists every path two planned lanes' scope files both name, directory prefixes
   included, and exits 1 when there is one. Step 2 of the partition in
   `briefs/run-a-milestone-wide.md` runs it before the first claim, with the generated records in
   the scope files.
3. Arming refuses a conflict. `helpers/arm.sh`, the one arming entry point of milestone 1052 (a
   push to a queued pull request re-arms it), runs `script/queue-conflicts --pr <n>` after its draft
   and hold checks. On exit 1 it prints the check's report, which names each conflicting file and
   the entries ahead that changed it, and arms nothing. `--despite-conflict "<why>"` is the
   override: it posts the reason and the report on the pull request, then arms, and arms nothing
   when the reason is empty or the comment does not post. On exit 2 (the check could not tell) it
   arms and says the check did not run. `script/push --requeue` arms through it, so a re-arm over a
   conflict is refused the same way. Provisional flag name.

`python3 helpers/queue_conflicts.py --selftest`, run by `script/lint-fleet` (which the `lint` job of
`.github/workflows/ci.yml` runs), builds local repositories with no network. It covers a conflict
with an entry ahead, a clean pull request, an entry behind, a conflict with `main` alone, an entry
that conflicts with those before it, the advisory mode and the scope check. Under a no-op
`merge_tree` the conflict case must fail.

Case 10 of `helpers/push-requeue-selftest.sh`, run by `script/lint`, runs the real
`script/queue-conflicts` with the real `git` against a local fixture whose queue holds one entry
ahead. `helpers/arm.sh` refuses the conflicting head and names the file and the entry, arms the
clean head, comments and then arms under `--despite-conflict`, and arms nothing when that comment
fails; `script/push --requeue` over the conflict is not re-armed.

The drain's comment on a queued entry gone UNMERGEABLE, and the cap on ready pull requests from one
wide run, are milestone 1041 (the drain names a conflicting entry and a wide run stops at six pushes).

## Options considered

- Rely on #3181's layout alone. It removes the names note's shared lines, but not shared source
  files: two mobile-storage groups still conflict on one module.
- Serialize all naming lanes. Refused: most groups touch disjoint files, and the check finds the ones
  that do not.

## Done when

- The pre-arm check refuses a fixture pull request that conflicts with a queued entry, and passes one
  that does not, under the lint. Met: `helpers/queue_conflicts.py --selftest` in `script/lint-fleet`.
- A wide run's launch names its shared files before the first claim. Met: step 2 of the partition in
  `briefs/run-a-milestone-wide.md`.
- `helpers/arm.sh` refuses a fixture pull request that conflicts with an entry ahead, names the
  files, and arms it under the override. Met: case 10 of `helpers/push-requeue-selftest.sh`.

## Follow-on

- **Milestone 1041.** The drain pass that comments once on a conflicting queued entry, and the cap
  of six ready pull requests per wide run.

## Index row

`script/queue-conflicts` catches a pull request that conflicts with the queue, and a wide run's shared scope files, before either costs a run, and `helpers/arm.sh` refuses to arm a conflicting pull request unless `--despite-conflict` gives the reason on it.
