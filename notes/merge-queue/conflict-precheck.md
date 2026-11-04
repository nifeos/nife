# Catching a conflict with the queue before arming

Provisional names: `script/queue-conflicts` and `helpers/queue_conflicts.py`.

## Why

On 2026-11-03 the queue removed 24 entries for merge conflicts, 14 of them in one minute. Each
removal cost the pull request its place and a push, and the push cost a full run of its checks. GitHub
had kept 13 entries it already knew were UNMERGEABLE until a merge moved `main`
([the correction](../coes/2026-11-03-merge-queue-backlog.md), action 6). A conflict between two
entries is knowable before either is armed, since `git merge-tree` answers in milliseconds.

## What it does

`script/queue-conflicts` merges a pull request's head against `main` plus every entry ahead of it in
the queue, and names each conflicting file with the entries ahead that changed it.

- The queue is read with the GraphQL `mergeQueue(branch: "main").entries` query, the same one
  `helpers/merge-drain.sh` reads. Each entry's `pullRequest.headRefOid` is its head. Entries are folded
  in by position, each with `git merge-tree --write-tree` against the running tip, and committed
  with two parents so the next merge sees them.
- An entry that does not merge into the tip is reported and left out, as the queue itself would
  remove it. The pull request is merged against what remains.
- The merges run in a scratch bare repository that borrows the caller's objects. No worktree, branch
  or ref of the caller's repository is written, and the scratch directory is removed afterwards.
- Exit 0 is clean, 1 is a conflict, 2 is "could not tell" (no `gh`, no network). A check that could
  not run never claims a pass.
- A pull request not yet in the queue is checked against the whole queue. One already queued is
  checked against the entries ahead of it only.

Run it before `gh pr merge --auto`. `script/push` runs it in `--advisory` mode after a confirmed push of a
branch whose pull request is open and ready: it prints a conflict and nothing else, and never changes
`script/push`'s exit.

## The escape hatch

Nothing blocks arming. `gh pr merge --auto` works without the check, and `NIFE_SKIP_QUEUE_CHECK=1`
skips the advisory run in `script/push`. A lane that arms over a reported conflict says why in the
pull request.

## The launch check

`script/queue-conflicts shared SCOPE...` takes one file per planned lane of a wide run
([briefs/run-a-milestone-wide.md](../../briefs/run-a-milestone-wide.md)), one path per line, a
trailing `/` for a directory. It lists every path in more than one scope and exits 1. The
maintainer runs it before the first claim; generated records (a names note, a baseline) go into the
scope files like any other path.

## Tests

`python3 helpers/queue_conflicts.py --selftest`, run by `script/lint-fleet`, builds local
repositories with no network: a conflict with an entry ahead, a clean pull request, an entry
behind (not ahead), a conflict with `main` alone, an entry that conflicts with the ones before it,
the advisory mode, and the scope check. One test replaces `merge_tree` with a no-op that reports no
conflict and requires the conflict case to fail under it.

## BUGS

- It does not yet run in the drain, which would comment once on a queued entry that has gone
  UNMERGEABLE, and nothing caps concurrent pushes from one wide run. Both are in the proposal
  [a wide run is held to six pushes and the drain says when an entry conflicts](../../design/roadmap/proposals/a-wide-run-is-held-to-six-pushes-and-the-drain-says-when-an-entry-conflicts.md).
- It judges the entries' heads as they are now. An entry pushed after the check changes the answer;
  the queue itself discards a push after enqueue, so the window is small.
- The scope check compares paths and directory prefixes, not globs.
