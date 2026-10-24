# Merge methods and holds: no squash at the repository, and `Blocked-by:`

*Name: provisional.*

An appendix to [notes/merge-queue.md](../merge-queue.md), which keeps the rules. This file holds
why squash and rebase merging were disabled at the repository, and why a hold names its own release
condition.

## Squash and rebase merging are disabled at the repository, not only in the queue

Decided by calef 2026-08-18, closing a gap between a rule and its enforcement. `AGENTS.md` has said
*never squash-merge a branch* since the convention was written, and the reason is `git blame`. The
lane of milestone 96 (one init: the spawn service written twice) put the loader unification in its
own commit *ahead of* the migration, so that a boot failure could not be ambiguous between two
changes. A squash-merge would have destroyed exactly that. The merge commit already carries the pull
request's title, so `git log --first-parent` reads as one entry per piece of work, while the detail
stays reachable underneath. The clean log costs nothing.

What was actually enforcing it until then was the merge queue's `merge_method: MERGE`, plus people
having read `AGENTS.md`. The repository still had `allow_squash_merge` and `allow_rebase_merge` set.
So the buttons were there, and the rule held by configuration coincidence and memory. That is rung two
propped on rung four. The same day measured what unenforced policy is worth here. A CI gate that could
not block a merge let `main` go red for hours, which was the advisory-checks decision, waiting on its
own branch as this was written.

Now `allow_squash_merge=false`, `allow_rebase_merge=false`, `allow_merge_commit=true`. It is
reversible in one API call, and the previous settings are recoverable from any repository snapshot.

This does not change how a lane works, and the distinction is the one worth keeping straight.
Squashing *within* a branch is still the rule. Commit early and push often, because a pushed branch
survives a dead session and nothing else does. Then squash the checkpoints into the purposes before
reporting. A checkpoint has no reader; a purpose commit has one. What is now impossible is collapsing
those purposes into one at merge, which is a different act on a different object.

Two exceptions stay unsquashed inside a branch, and they are why this matters: a commit that records
a correction or a surprise, and a commit whose separateness is itself the argument.

## `Blocked-by: #N`: a hold that releases itself

The problem it solves is ordering, not attention. `needs-architect` means a person must decide
something, and it was the only lever the drain had. On 2026-08-18 that lever was reached for twice
where no decision was owed. Both times it put a false entry on the one queue in this project that
must not accumulate noise.

The live case: #329 and #324 each carried a file named `97-*.md` under `design/decisions/`. Both were
green alone. A merge-queue group containing both fails the decisions gate, because two sections
cannot share a number. So #329 was evicted as `UNMERGEABLE`, while its own page reported `CLEAN`.

Green alone, green alone, red together is not a state any per-branch check can see. #274 is the
expensive version of the same shape. It was enqueued and evicted 29 times, 26 of them inside a
3.5-hour loop. #271 had landed a doctest calling a method whose arity #274 was changing, and git
merged both with no conflict marker.

A generic hold label was considered and refused, on the failure mode rather than on tidiness. A
manual label must be removed by whoever remembers, and this tree has the receipts. `needs-architect`
was left on both #320 and #329 after each had been answered, on one day, and calef found both. A hold
that outlives its reason is a false blocker. A false blocker is worse than none, because it is
believed.

So the hold names its own release condition, `Blocked-by: #324`, in the pull request body. Until
2026-10-03 the drain skipped that pull request while #324 was open, and armed it on the first pass
after #324 merged. Since the drain stopped arming, the line keeps a ready pull request's `unarmed`
cause off while #324 is open. The pull request is labeled `needs-maintainer` once #324 has merged or
closed, which says "arm this now" to a session rather than doing it. On a draft it earns the
`unblocked` label instead.

Use it for a mechanical constraint and nothing else. If a person must decide, the label is still the
right answer, and the two must not be conflated. One is a queue for an architect's attention; the
other is a fact about two branches.
