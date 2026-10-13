# `main` is red

Hold the merge queue, land one fix alone, give the queue back. This brief holds the judgement;
`helpers/queue-hold.sh` does the mechanical half. Provisional names, both of them.

You are the maintainer session that noticed, or the one `helpers/trunk-health.sh` shouted at. Either
way the trunk is now yours: "we are all owners here, so if there isn't an owner then own it."

## First, decide whether it is real, because three things look like a red trunk and are not

Do this before touching anything. A hold costs everybody's lane a cycle, and holding for a phantom
teaches the next session to ignore the procedure.

- A stale base. A pull request red against a base it was cut from says nothing about `main`. Read
  the run whose `headSha` is `main`'s current tip, which is what `helpers/trunk-health.sh --once`
  already filters for. If you are reading a pull request's checks, you are reading the wrong thing.
- A cancelled shard. A cancelled run reads as a failure in most listings and is not one. Ask what
  cancelled it: a newer push to the same concurrency group, an eviction, or somebody's `gh run
  cancel` (including this procedure's own step 3). Nothing was learned by a cancelled run, in either
  direction.
- A dead cadence, not a broken trunk. `script/cadence-check` reports a weekly job that has
  produced no result, and `trunk-health.sh` relays it. That is a real problem and it is not this
  one: a cadence has been dead for weeks by the time it is noticed, so it never justifies holding a
  queue.

And the fourth case, which is the opposite mistake: green does not mean healthy. `ci.yml` skips
the build jobs' steps (`test`, `swish-check`, `boot-check`) for a commit that touches only
`notes/`, `design/` or a root `*.md`, and the checks still post `success`. That can hide a broken
`crates/documentation` test for hours. So if a lane reports a failure that CI says did not happen,
believe the lane and reproduce locally (`cargo test -p documentation`) before concluding the trunk
is fine.

## The drain is on your side now

`helpers/merge-drain.sh` runs unattended as the `merge drain` Actions workflow every five minutes.
It arms nothing (milestone 727 (a queue eviction goes to a maintainer session), provisional), and
it dequeues anything labelled `held-for-red-trunk` that is still in the queue, so
leave it running. What can still undo a hold is a lane arming a new pull request; re-run `hold` to
sweep new arrivals.

## Then hold, and hold before you enqueue the fix

    helpers/queue-hold.sh hold <fix-pr> --dry-run     # read it first; it names every pull request
    helpers/queue-hold.sh hold <fix-pr>

Order matters. Hold first, enqueue the fix second. The script cancels in-flight group builds and
cannot tell one that contains the fix from one that does not (its own `BUGS` says why: the branch
name carries only the group's last entry). Holding first makes that impossible rather than unlikely.

The four steps it performs are in its header with the failure behind each one. The two worth knowing
by heart, because they are what a person doing this by hand gets wrong:

- Disabling auto-merge and dequeuing are both required, in that order. Dequeuing an armed pull
  request re-enqueues it within minutes. This is how a queue reported drained comes back with all
  seven entries.
- Orphaned `gh-readonly-queue/...` runs keep executing for entries that no longer exist. Their
  results cannot be consumed and they starve the fix of runners.

## Land the fix alone

One pull request, nothing else enqueued. That is the point of the hold: a group build that mixes the
fix with four other entries tells you nothing about the fix if it fails, and the tree is already in
the state where you cannot afford an ambiguous result.

If the fix itself fails in the queue, **do not release**. Fix forward on the same pull request, or
revert the merge that broke the trunk and treat the revert as the fix. A revert is the cheaper answer
more often than it feels like it is.

## Release, and read what it could not restore

    helpers/queue-hold.sh status       # trunk state and the held set, together
    helpers/queue-hold.sh release

Release re-arms auto-merge and removes the label. **Anything it could not re-arm keeps the label on
purpose** and is named in the output: a conflict, a failing check, or a pull request that was closed
while held. That list is the handoff, and it is the one part of this that a person must read rather
than skim.

`release` does not restore the pre-hold state, because GitHub does not expose it (`autoMergeRequest`
is cleared at enqueue). It applies `helpers/queue-eligible.jq`'s admission policy instead.

## If a session died mid-hold

The label is the whole recovery record; nothing else was kept, and nothing expires it.

1. `gh pr list --repo nifeos/nife --label held-for-red-trunk` is the held set, whoever made it.
2. `helpers/trunk-health.sh --once`. If `main` is green, the fix landed and the hold was simply never
   given back: run `helpers/queue-hold.sh release`.
3. Check the drain is enabled: `gh workflow view "merge drain"` reads `active`. If a session
   disabled it and died, re-enable it.
4. If `main` is still red and no fix pull request is open, the previous session died before it wrote
   one. You are now the session that noticed; start at the top of this brief.
5. If a pull request carries both `held-for-red-trunk` and `needs-architect`, leave it held. The
   architect hold outranks this one and `release` will refuse to re-arm it anyway.

The tell that this happened at all: `gh pr list --label held-for-red-trunk` is not empty while
`main` is green.

## What this brief deliberately does not do

**It does not become a watcher.** `notes/merge-queue.md` argues that a queue reports and does not
resolve, and deciding that a given failure is a real trunk failure, and which pull request is the
fix, is exactly the judgement a watcher does not have. Every step above is a person's, and the script
exists only so that the steps a person gets wrong are typed once.

Names minted here are provisional: `helpers/queue-hold.sh`, and the `held-for-red-trunk` label.
