# Git in a shared worktree: why the commit rules read as opposites

*Appendix to `nifeos/nife:AGENTS.md`, which carries the rules: one purpose per commit, commit
early and push, never rewrite a pushed commit, never `git stash` in a worktree. This file carries why
each rule exists and the failure that produced it, all of them from days when a lane lost work, took
another lane's, or stopped.*


One purpose per commit. The message explains why, not what (the diff shows what). If a commit
records a correction or a surprise, say so in the message. See the history of milestone 1 (boot to
Rust on QEMU `virt`) for the shape.

Commit early and push, and never rewrite what was pushed. `git blame` is what a commit is for: a
reader tracing why a line looks the way it does must land on a commit that explains it. A pushed
commit reaches main as written, so a correction is its own commit, and its message is the
explanation the reader lands on.

So while working, commit whenever a piece works and push whenever a commit exists, because a pushed
branch survives a dead session, a killed process and a laptop that will not wake, and nothing else
does. On 2026-08-04 a lane sat on seven modified files with zero commits for hours; had that
worktree been pruned the work was gone, and it was caught by inspection rather than by any
mechanism. Uncommitted work in a lane worktree is the one thing no part of this system protects.

**A pushed commit is never rewritten** (calef, 2026-10-18 UTC): no `git reset`, `git rebase`,
`git commit --amend` or force-push on a commit that is on the remote, and main comes in with `git
merge origin/main`. Rewriting history fails here in two ways:

- It stops the lane. Agents run in Claude Code's auto mode, which classes `git reset` and a
  force-push as destructive and refuses them, though not every time. On 2026-10-18 a lane's
  `git reset --soft` was refused twice, and its next call waited seven hours for an approval in a
  window nobody was watching. A rule that a lane can follow only some of the time is not a rule.
- It touches shared state. Agent worktrees share one `.git`, so `origin/main` moves under a lane
  while it works. On 2026-08-04 a developer that ran `git reset --soft origin/main` to squash
  silently staged four other lanes' files as its own, including a deletion, and caught it only by
  reading `git status` before committing. In a worktree, `origin/*` is not a fixed point.

The cost is a branch that carries its fix commits. That is paid once per pull request and read
through `git log --first-parent`, where the merge commit is one entry per piece of work.

`git stash` is unsafe in these worktrees, for the same reason one level over. The stash stack is
per-`.git`, not per-worktree, so it is shared machine-wide across every lane. Found 2026-08-26 by
the two-core-crash lane of milestone 161 (the x86_64 kernel port): another session pushed a stash
between this lane's `git stash` and its `git stash pop`, so the pop popped *someone else's* entry
into this lane's tree and conflicted. Nothing was lost that time (the conflict preserved the other
entry, and it was restored and re-popped by name). But the failure mode is the same family as the
`git reset --soft origin/main` trap above: a command that reads as lane-local silently touches
shared state. **Use a patch file (`git diff > /tmp/<lane>-<what>.patch`, later `git apply`) instead of `git
stash` in a worktree**, the same way `origin/*` is not a fixed point once more than one lane can
move it. Name it what no other lane would: this said `/tmp/x.patch` until two lanes both staged a
body at `/tmp/pr-body.md` on 2026-09-21 and one pushed the other's text.

**Two purposes stay two commits.** Squash-*merging* is impossible: `allow_squash_merge` is `false`
on this repository, so the platform refuses it. The lane of milestone 96 (one init: the spawn service written twice) put the
loader unification in its own commit *ahead of* the migration. That way a boot failure could not be
ambiguous between two changes, which is the whole reason that structure exists. A squash-merge would
have destroyed it. The merge commit carries the pull request's title, so `git log --first-parent`
already reads as one entry per piece of work while the detail stays reachable underneath.
