---
name: worktree-commits
description: >-
  Committing in nife: load before committing, pushing or saving work aside in a lane worktree.
  Never use git stash (the stash stack is shared machine-wide by every lane), and never rewrite a
  commit once it is pushed: no reset, rebase, amend or force-push; bring main in with a merge. One
  purpose per commit, the message says why, commit early and push.
---

# Commits, and git in a worktree

Where this says "this file", it means `AGENTS.md`. Its name was ratified by calef on 2026-10-06
(UTC); notes/skills/README.md records it.

## Commits

One purpose per commit. The message explains why, not what (the diff shows what). If a commit
records a correction or a surprise, say so in the message. The message is also where history goes:
what a document said before, which ruling a new one replaces, which lane did what. The document
states only what is now true (§298 (a document states what is in force now)). Why these rules read as opposites and are
not, with the failure behind each, is in
[design/tenets/git-in-a-worktree.md](../../../design/tenets/git-in-a-worktree.md).

Commit early and push. `git blame` is what a commit is for: a reader tracing why a line looks the
way it does must land on a commit that explains it. Every commit a lane pushes reaches main as
written, so each one has to stand on its own.

- While working, commit whenever a piece works and push whenever a commit exists. A pushed branch
  survives a dead session, a killed process and a laptop that will not wake, and nothing else does.
  Uncommitted work in a lane worktree is the one thing no part of this system protects.
- **Push with `script/push`, not `git push`.** git opens the connection before the pre-push hook
  runs, and GitHub drops it after five to eight idle minutes; the gates pass and the branch does not
  move. `script/push` keeps the connection alive, then reads origin's branch back and ends with
  `NOT PUSHED` and exit 1 when it is not HEAD's commit. Believe that line, not `git push`'s status.
- **A pushed commit is never rewritten** (calef, 2026-10-18 UTC). No `git reset`, `git rebase`,
  `git commit --amend` or force-push touches a commit that is on the remote. Bring main in with `git
  merge origin/main`. On a fleet clone the pre-push hook refuses a push that would rewrite one
  (`helpers/push-rewrite-check.sh`). A fix to a pushed commit is a new commit whose message names the commit it
  corrects, and that message is what `git blame` lands on. Before the first push, amend freely.
- Every commit an agent writes ends with a `Co-Authored-By:` trailer naming its model, such as
  `Co-Authored-By: GLM 5.3 <noreply@z.ai>`, whatever the vendor or tool; `script/metrics` reads it.
- In a worktree, `origin/*` is not a fixed point. Agent worktrees share one `.git`, so
  `origin/main` moves under a lane while it works; read it when you merge it, never cache it.
- `git stash` is unsafe in these worktrees, for the same reason one level over: the stash stack is
  per-`.git`, so it is shared machine-wide across every lane, and on a fleet clone it fails
  ("Cannot save the current status") because `helpers/stash-block.sh` holds a ref that blocks it.
  Use a patch file (`git diff >
  /tmp/<lane>-<what>.patch`, later `git apply`) instead, and name it what no other lane would.
- Squash-*merging* is impossible (`allow_squash_merge` is `false` on this repository), so two
  purposes in one pull request stay two commits, and a commit whose separateness is itself the
  argument keeps it.
