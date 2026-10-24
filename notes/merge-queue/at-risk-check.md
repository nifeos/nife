# `helpers/at-risk-check.sh`, in full

*Name: provisional. After the script it documents.*

An appendix to [notes/merge-queue.md](../merge-queue.md). It holds why the at-risk watch was built,
how it reads a worktree, why it never acts, and why it was folded into the trunk watcher and then
unfolded.

## Why it exists

AGENTS.md gives the steward a second watch, named beside the idle-lane one and called the more
valuable of the two: "a lane worktree with modifications and no commit in half an hour is uncommitted
work one prune away from gone, which is the only failure in this system that destroys rather than
delays." Nothing built it. `launchctl list` showed `com.nife.merge-drain` and `com.nife.trunk-health`
on patagonia and nothing else. So the duty AGENTS.md assigns by name had no mechanism behind it.

The cost was measured, not hypothetical. In one session on 2026-09-23, three pieces of work were
found only by luck rather than by anything watching:

- A fix to `helpers/open-lane.sh` (later #1097) surfaced while pruning merged worktrees.
- A fix to `system_tests/src/user/live_swap_tests.rs` (later #1101) survived two prunes uncommitted,
  and had to be recovered twice.
- Sixty-two lines of a decisions amendment sat unsaved on `maintainer/202-four-tiers` for hours after
  the conversation had moved on.

The maintainer pruned worktrees twice that same session. Any of the three could have been destroyed
outright.

```console
$ helpers/at-risk-check.sh
at-risk-check: UNCOMMITTED. /path/to/nife-worktrees/atrisk (maintainer/work-one-prune-from-gone) has 2 changed file(s), newest touched 41 minutes ago. One prune away from gone; commit and push.
```

## How it reads a worktree

It reads every worktree but the main checkout (`git worktree list --porcelain`). It skips any already
`prunable`, since nothing is left in them to lose. For the rest it reads `git status --porcelain`.

The clock is the newest modification time among the changed files, tracked or untracked, not the
branch's last commit date. A worktree can carry a commit from hours ago and be mid-edit again a moment
later. The fact that matters is how long the current uncommitted state has sat, not when it was last
saved. `AT_RISK_MINUTES` (default 30, AGENTS.md's own "half an hour") overrides it. That is the same
convention `GRACE_MINUTES` already sets in `helpers/lane-claim-check.sh`.

## It reports and never acts

That is the same boundary `helpers/lane-claim-check.sh` holds. It does not commit on a lane's behalf,
and it does not use `git stash`. The stash stack is per-`.git`, shared across every worktree of this
repository rather than scoped to one. So one worktree's `git stash` can be popped by a session working
in a completely different worktree. That is action at a distance, of exactly the kind this script
exists to warn about rather than to commit. AGENTS.md's own note, "`git stash` is unsafe in these
worktrees, for the same reason one level over," is the same finding from the other side.

## Folded, then unfolded

It was folded into `helpers/trunk-health.sh`'s loop on 2026-09-23, and unfolded on 2026-09-24. The
reasoning is worth keeping, because it was right both times. The fold avoided a third watcher that
could die silently, reusing a job already firing on the right interval. That holds only while both
halves run on the same machine. When the trunk half moved to Actions they stopped doing so.
Everything else in `trunk-health.sh` reads GitHub, and this reads this machine's worktrees. So
carrying the fold into a runner would have produced a check reporting nothing forever, while the
hazard sat on a laptop unwatched.

It now has the third `launchd` job the fold avoided, with the cost that decision was avoiding
accepted explicitly: nothing reports its death either. The plist and the commands are in
[actions-migration.md](actions-migration.md).

Unlike RED/GREEN, and unlike the cadence check beside it, this does not dedupe by transition. A
worktree still at risk on the next poll is still exactly as at risk. Distinguishing "still true" from
"newly true" would need a second piece of state this script does not otherwise keep. The cost of not
deduping is log lines, rather than anything a reader has to act on twice.

## Verified against the live tree, 2026-09-23

Of 42 lane worktrees, only this lane's own carried uncommitted work, and it was minutes old, inside
the grace window. Nothing else was flagged. No false positive against a lane legitimately mid-work
was observed, because nothing was over the threshold to test that against. `AT_RISK_MINUTES=0`
against the same tree correctly flagged this lane's own worktree, confirming the mechanism fires.
