# Catching a conflict with the queue before arming

Provisional names: `script/queue-conflicts` and `helpers/queue_conflicts.py`.

## Why

On 2026-11-03 the queue removed 24 entries for merge conflicts, 14 of them in one minute. Each
removal cost the pull request its place and a push, and the push cost a full run of its checks.
GitHub kept 13 entries it already knew were UNMERGEABLE until a merge moved `main`
([the correction](../coes/2026-11-03-merge-queue-backlog.md), action 6). `git merge-tree` finds
such a conflict in milliseconds, before anyone arms either entry.

## What it does

`script/queue-conflicts` merges a pull request's head against `main` plus every entry ahead of it in
the queue, and names each conflicting file with the entries ahead that changed it.

- It reads the queue with the GraphQL `mergeQueue(branch: "main").entries` query, the same one
  `helpers/merge-drain.sh` uses. Each entry's `pullRequest.headRefOid` is its head. It folds the
  entries in by position, merging each into the running tip with `git merge-tree --write-tree` and
  committing the result with two parents so the next merge sees it.
- It reports an entry that does not merge into the tip and leaves it out, as the queue itself
  would remove it. The pull request merges against what remains.
- The merges run in a scratch bare repository that borrows the caller's objects. The check writes no
  worktree, branch or ref of the caller's repository, and removes the scratch directory afterwards.
- Exit 0 means clean, 1 a conflict, 2 "could not tell" (no `gh`, no network). A check that could
  not run never claims a pass.
- A pull request outside the queue meets the whole queue. A queued one meets the entries ahead of
  it only.

`helpers/arm.sh`, the one arming entry point, runs it and refuses a pull request it reports a
conflict for, printing the report. When the check could not tell, `helpers/arm.sh` arms and says the
check did not run. After a confirmed push of a branch whose pull request is open
and ready, `script/push` runs it in `--advisory` mode: it prints a conflict and nothing else, and
never changes `script/push`'s exit.

## The escape hatch

`helpers/arm.sh <n> --despite-conflict "<why>"` arms over a reported conflict. It posts the reason
on the pull request first, with the check's report, and arms nothing when the comment does not post
or the reason is empty. `NIFE_SKIP_QUEUE_CHECK=1` skips the advisory run in `script/push`.

## The launch check

`script/queue-conflicts shared SCOPE...` takes one file per planned lane of a wide run
([briefs/run-a-milestone-wide.md](../../briefs/run-a-milestone-wide.md)), one path per line, a
trailing `/` for a directory. It lists every path in more than one scope and exits 1. The
maintainer runs it before the first claim, with the generated records (a names note, a baseline) in
the scope files like any other path.

## Tests

`python3 helpers/queue_conflicts.py --selftest`, run by `script/lint-fleet`, builds local
repositories with no network. It covers a conflict with an entry ahead, a clean pull request, an
entry behind (not ahead), a conflict with `main` alone, an entry that conflicts with the ones before
it, the advisory mode, and the scope check. One test replaces `merge_tree` with a no-op that
reports no conflict and requires the conflict case to fail under it.

`helpers/push-requeue-selftest.sh`, run by `script/lint`, covers the refusal in `helpers/arm.sh`
(case 10). It runs this check with the real `git` against a local fixture whose queue holds one
entry ahead. `helpers/arm.sh` refuses the conflicting head and names the file and the entry, and
arms the clean head. Under `--despite-conflict` it comments and then arms, and `script/push
--requeue` over the conflict leaves the pull request unarmed.

## BUGS

- The drain does not run it yet, so nothing comments on a queued entry that has gone UNMERGEABLE,
  and nothing caps concurrent pushes from one wide run. The proposal
  [the drain names a conflicting entry and a wide run stops at six pushes](../../design/roadmap/1041-the-drain-names-a-conflicting-entry-and-a-wide-run-stops-at-six-pushes.md)
  holds both.
- It judges the entries' heads as they are now. A push to an entry after the check changes the
  answer, although the queue discards a push after enqueue, so the window is small.
- The scope check compares paths and directory prefixes, not globs.
