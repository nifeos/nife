# What to do when `main` goes red

The trunk breaking is the one failure in this repository that stops everyone at once: every open pull
request is now measured against a base that is wrong, and the merge queue will happily grind through
groups that cannot pass. Three scripts watched the queue before 2026-09-23 and all three reported;
none responded. This note is the response, what it is made of, and what it got wrong first.

The procedure itself is in `nifeos/nife:briefs/main-is-red.md`; the mechanics are in
`helpers/queue-hold.sh`'s header. Neither is repeated here. This is the why, and the evidence.

## The gap calef named

2026-09-23: *"Is there a missing mechanism for fixing when main goes red? It seems like the fix is
enqueuing the one fix and holding everything else until that lands, then re-enabling everything
else."*

That is the whole procedure, correctly stated, and nothing implemented it. `helpers/trunk-health.sh`
says the trunk is red. `helpers/merge-drain.sh` arms everything that does not need an architect,
which during a red trunk is precisely the wrong thing and it will keep doing it every five minutes.
`helpers/lane-claim-check.sh` and `helpers/at-risk-check.sh` watch one step earlier still. The
response lived in whoever happened to be at the keyboard, which is rung zero of AGENTS.md's ladder
and is not a mechanism.

The same evening a maintainer ran the procedure by hand, got it wrong twice, and left the held set in
a chat message. All three of those are the failure this note records.

## The four things that fail, with dates

(There is a fifth below, found the same evening and ours rather than GitHub's.)

Each of these is why a step of the procedure exists.

1. Dequeuing does not stick while auto-merge is armed. A pull request removed from the queue with
   its auto-merge request still live re-enqueues itself within minutes, so a drained queue fills
   again. Disable auto-merge first, then dequeue.
2. `gh pr merge --disable-auto` does not remove an entry already in the queue. The two operations
   are independent, and only the GraphQL mutation dequeues. Its input field is `id`, not
   `pullRequestId`:

       gh api graphql -f query='mutation($pr:ID!){dequeuePullRequest(input:{id:$pr}){clientMutationId}}' -f pr="$node_id"

   `helpers/merge-drain.sh` learned the same call on 2026-09-18 for a different reason (a
   `needs-architect` label arriving 73 seconds after an enqueue). `queue-hold.sh` borrows its
   before/after timeline count. The mutation is a silent no-op on a pull request that is not queued,
   so counting `removed_from_merge_queue` events is the only honest way to say whether anything
   moved.
3. GitHub leaves orphaned group builds running. Dequeuing an entry does not cancel the
   `gh-readonly-queue/...` workflow runs already executing for the group it was in. Their results
   cannot be consumed by anything, and on a repository with this one's runner concurrency they starve
   the fix of the machines it needs. 55 were cancelled by hand that evening.
4. A green CI conclusion does not mean a healthy tree. This is the one worth carrying furthest,
   because it falsifies a claim this tree has been making since 2026-08-04. `ci.yml` skips the steps
   of `build + test` when every changed path matches `notes/`, `design/` or a root `*.md`, and the
   job still posts a green required status. It has to: a job that never runs never posts, and the
   check is required, so the merge queue would wait forever. That rule is sound for a kernel and
   false for `crates/documentation`, whose tests render those exact files. So a documentation-only
   commit broke `every_character_survives`, merged, and left `main` red for hours while
   `helpers/trunk-health.sh`, which reads the conclusion, said green. Pull request #1168 fixes the
   skip. The durable lesson is the watcher's, and is now in its `BUGS`: a conclusion is a claim
   about what ran, not about the tree.

## The fifth thing that fails, and it was ours

A watcher undid the hold three times, and the hold reported success each time. Found 2026-09-23
by watching the live queue refill. `helpers/merge-drain.sh` runs under `launchd` with
`StartInterval 300`, and its admission policy excluded exactly two things, drafts and
`needs-architect`. It knew nothing about `held-for-red-trunk`, so every hold survived at most five
minutes and then quietly came apart.

Two things make this worse than an ordinary bug and worth the space:

- The evidence points at the wrong thing. Dequeuing leaves no record of why an entry returned, so
  a refilled queue reads as the operator's own dequeue having failed. It was misdiagnosed twice
  before anyone read the drain.
- **It is the same class as the defect this note already records against `helpers/trunk-health.sh`:**
  a mechanism whose assumption about its environment quietly stopped being true. The drain's
  assumption was that the only reason to keep a pull request out of the queue is a label about that
  pull request. A red trunk is a reason about the queue.

The fix is in the drain rather than beside it, and it lands in both halves of the logic that file
already has: the enqueue filter, and the 2026-09-18 re-check that dequeues what became held *after*
admission. That re-check exists because admission was checked once and never re-checked, which is
this failure one label earlier, so the new label belongs in the same argument.

**The sequencing consequence, until that change is on `main`:** holding the queue requires stopping
the drain by hand (`launchctl unload ~/Library/LaunchAgents/com.nife.merge-drain.plist`), and
whoever does that owes the reload afterwards. The brief carries both commands, the reload as part of
release rather than as a reminder, because a drain left dead is a queue that lands nothing and
announces nothing.

Since 2026-10-03 the drain arms nothing at all (milestone 727 (a queue eviction goes to a maintainer session), provisional), so it can no longer
undo a hold; it only dequeues what is held. What can still undo one is a lane arming a new pull
request into a red trunk, which the BUGS below records.

## Why the record is a label

The held set must survive the session that made it, and on 2026-09-23 it did not: it was a list in a
chat message, which is the medium milestone 94 (the untracked-work sweep) exists to abolish.

So it is a label, `held-for-red-trunk`, on the pull requests themselves. That is rung three of
AGENTS.md's ladder, the same rung and the same mechanism `needs-architect` already proves. The held
queue is `gh pr list --label held-for-red-trunk` rather than something somebody has to have read. A
session that dies leaves a visible set rather than an invisible one.

**Rungs one and two are deliberately not attempted.** No gate can tell a pull request that should be
held from one that should not, because that is a judgment about whether a given failure is a real
trunk failure. And a watcher that held the queue on its own would be resolving rather than reporting,
which `notes/merge-queue.md` argues against on purpose and which this note has no evidence to
overturn.

**Nothing else is remembered, and that is forced rather than chosen.** The obvious design records
each pull request's auto-merge state at hold time and restores exactly that. It cannot be built:
GitHub clears `autoMergeRequest` the moment a pull request enters the queue, so "armed and queued"
and "never armed" are indistinguishable through the API. Release therefore re-arms every held pull
request under `helpers/queue-eligible.jq`'s admission policy.

## How it was rehearsed, since the live queue was not available

The script was written while seven pull requests were genuinely held for a red trunk, so testing the
mutating path against `nifeos/nife` was out: `hold` would have labeled other lanes' work and
cancelled live group builds. Two things made it testable anyway.

- `--dry-run` against the real repository, which is read-only and proved the selection: it named
  fifteen pull requests, excluded the fix given as an argument, the drafts, and anything carrying
  `needs-architect`.
- **`QUEUE_HOLD_REPO` against a scratch repository** (`calef/queue-hold-drill`, created, exercised and
  archived the same hour), with four pull requests standing in for a fix, an ordinary lane, a draft
  and an architect hold. `hold` labeled exactly the ordinary lane; a second `hold` changed nothing
  and said so; `release` re-armed and unlabeled it, and correctly refused a draft that had been
  labeled by hand, leaving the label on it and naming it.

**The rehearsal is the only reason two bugs are not in the tree**, and both are the same shape, which
is a failure that reports success:

- A wrapper that narrates has to own its redirection. `do_or_say "..." gh pr merge ... >/dev/null
  2>&1` sends the narration to `/dev/null` along with the command, so the first dry run decided to
  change fifteen pull requests and named none of them.
- GitHub caps a label description at 100 characters and answers 110 with an HTTP 422. Swallowed by a
  `|| true`, that made every subsequent `--add-label` fail against a label that did not exist, while
  the hold reported each one as done. `ensure_label` now aborts instead: without the label there is
  no record, and a hold with no record is the chat message this replaces.

## BUGS

- Nothing releases a hold, and nothing expires one. A session that dies mid-hold leaves the queue
  stopped until a person notices. The tell is `gh pr list --label held-for-red-trunk` non-empty
  while `main` is green, and the recovery list is in the brief. This is an accepted
  gap of the same family as the watchers not reporting their own death.
- A pull request opened or marked ready during a hold is not held. `hold` labels what is open
  when it runs, and a lane that arms a new arrival puts it into a red trunk.
  Re-running `hold` sweeps them; nothing does that on a timer.
- The orphan-cancellation step can evict the fix. A merge-queue branch is named after the last
  entry in its group, so a group holding the fix behind other entries is indistinguishable from an
  orphan. Holding before enqueueing the fix avoids it entirely, which is what the brief says to do,
  and the cost if it happens is one CI round trip.
- Nothing measures whether this is faster than doing it by hand. The claim is that the failures
  above are typed once rather than rediscovered, and the sample is one incident, which is the same
  honest caveat `briefs/README.md` carries about delegation.
- `gh pr list --label` lags. It reads a search index, and in the rehearsal a listing seconds after
  a label write was short by one. Trust the labels on the pull requests over the listing when they
  disagree.
- **The four failure modes above are recorded from one evening.** They are what happened, not a
  survey of what can happen; a queue behavior GitHub changes tomorrow will not announce itself here.
