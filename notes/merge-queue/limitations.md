# The merge queue's limitations, in full

*Name: provisional.*

An appendix to [notes/merge-queue.md](../merge-queue.md), whose BUGS section keeps the short list.
This is every entry as it was written, sentence-split for the prose limits.

## The log

The queued-push refusal misses cloud lanes: [push-while-queued.md](../push-while-queued.md).

The event lines start from the day they landed, and the 3,355 passes before it cannot be backfilled.
The old log holds snapshots only. So the drain's action count begins on 2026-09-23, and any
comparison across that boundary is between two different measurements. The same holds at 2026-10-03,
when `ARMED`, `ENQUEUED`, `EJECTED` and `RELEASED` stopped and `LABELLED` and `CLEARED` began.

And this log can only ever say what the machinery did, never what a person did. An action absent from
it is ambiguous between "the drain did not do this" and "somebody did it by hand". The ambiguity is
worst exactly when it matters, which is when something unexpected happened. Closing that needs
distinct GitHub identities rather than a better log. The proposal is
[design/roadmap/0642-who-took-the-step.md](../../design/roadmap/0642-who-took-the-step.md) (name
provisional), and it is an architect's call.

## A′ and the timeout

- A′ lets `main`'s Actions caches go stale. A pull request restores only its own ref's caches and
  `main`'s, and most pushes to `main` now skip every cache step. rust-cache was measured in
  notes/actions-cache-budget-2026-10-07.md and ruled on in #1814. QEMU and the patched Kani are saved
  on every push by ci.yml's `main-caches` job (milestone 870 (`main` keeps its CI caches warm),
  provisional). That came after #1899's `.qemu-version` change cost every QEMU job about 270 s.
- A′ trusts a merge-group run's overall conclusion, which is stricter than what the queue requires.
  The queue lands on the required checks alone. A merge-group run can conclude `cancelled` or
  `failure` because of a non-required job, and still land. `0b72f673` (#1156) did, when verify's
  falsify job hit its 45-minute timeout. That push then re-runs the whole workflow, and it did (verify
  36029132635, cancelled by the same timeout). A flaky non-required job costs a full re-run on
  `main`.
- A 240-minute check timeout is four hours in which a genuinely hung group blocks everything behind
  it. The honest fix is fewer runs competing for runners, and A′ is the first of those. When the queue
  stops starving, lower the timeout again, because the cost of a long timeout only shows up when a
  group is actually stuck.

## One push can raise two `synchronize` events

When the cancelled copy is the newer run, the pull request is stranded, green and armed, outside the
queue (#1203, 2026-09-24). Every workflow on `e0875d56` ran twice at 15:43:24. That included
`coe architect label`, which listens only for `opened`, `reopened` and `synchronize`, so it was not
`ready_for_review` (that fired at 15:30:28). The concurrency group cancelled one copy of each within a
second, and before any job existed. For verify the cancelled copy was the older id (36022255755), and
nothing broke. For CI it was the newer id (36022256151, over 36022255835's success). A forced enqueue
answered "11 of 13 required status checks are expected": CI's eleven. So GitHub reads the newest
suite per workflow, and an empty cancelled suite there hides a green one. Rerunning the cancelled run
fixed it, at the cost of a whole second suite (attempt 2 ran 16:41 to 17:37).

No workflow-level fix is sound. The duplicate event is GitHub's. A pending run is always cancelled
when a newer one joins its group, whatever `cancel-in-progress` says. Putting the SHA in the group key
would stop the duplicates cancelling each other, but a new push would then stop replacing the old
one. Skipping when a same-SHA run already succeeded would also skip after a draft run that concluded
`success` with every job skipped, which is #567 again. The detection belongs in the drain, which can
rerun the run it finds:

  ```sh
  sha=$(gh pr view "$pr" --json headRefOid --jq .headRefOid)
  gh api "repos/nifeos/nife/actions/runs?head_sha=$sha&event=pull_request&per_page=100" --jq '
    .workflow_runs | group_by(.name)[] | sort_by(.id) as $r | ($r | last) as $n
    | select($n.conclusion == "cancelled")
    | select([$r[] | select(.id != $n.id and .created_at == $n.created_at
                            and (.conclusion == "success" or .status != "completed"))] | length > 0)
    | $n.id'          # each id printed: gh run rerun <id>
  ```

The same-second condition is what separates this from an ordinary replaced run. #1207, #1209 and #1211
each have a cancelled CI run followed 20 to 66 seconds later by a successful one. That is a draft
marked ready, and none of them was stranded.

The drain does this now (#1252, 2026-09-24). The query above is `helpers/cancelled-duplicate.jq`,
spliced into `merge-drain.sh`. A pull request in this shape gets its cancelled duplicate rerun once,
logged as `RERAN #N run <id>`. Once is decided by the run's own `run_attempt`, so no file or label
holds the state. A duplicate already at attempt 2 is a `STALLED.` line for a person. The rerun uses
the workflow's own token with `actions: write`, because the App's token cannot rerun a workflow.
Adding `Actions: read/write` to the App is calef's call, and would let the rerun carry the App's
identity.

## A stacked branch merged with `main` has two merge bases

GitHub calls that a conflict that git does not see (#1220, 2026-09-24). #1220 was cut from #1213's
branch. After #1213 landed and `main` was merged back in, `git merge-base --all` gave both
`47c3a3c9` and #1213's own commit. `git merge-tree` merged cleanly against every entry ahead of it in
the queue. But the queue marked it `UNMERGEABLE`, built no group for it, and evicted it with
`merge_conflict` at 20:00:51. Its page said `CONFLICTING`. Once the base pull request lands, rebase
the stacked commits onto `main` rather than merging `main` in. The tell is
`git merge-base --all origin/main HEAD` printing more than one line.

## A branch in the merge queue cannot be pushed to

The error names the fix without naming the cost. `git push` is rejected with `GH006: Protected branch
update failed ... Branches that are queued for merging cannot be updated. To modify this branch,
dequeue the associated pull request.` It was met on 2026-09-20 by a maintainer who armed auto-merge,
then found a defect in the committed tip. The fixed commit could not be pushed.
`gh pr merge --disable-auto` did not release it, and the `merge-queue` REST endpoint answered
`Not Found`. What worked was waiting for the group build to evict the branch, then pushing and
re-arming.

The order that avoids it: gate, then arm. An armed pull request is one whose content you have stopped
editing. That sounds obvious, and is exactly what a maintainer fixing a gloss at the last moment
forgets.

## `Blocked-by:`

- `Blocked-by:` is matched anywhere in the body, including inside a code span or a quotation. A pull
  request that *discusses* the convention, as opposed to using it, will be held. The counted-claims
  check in `script/lint` solved the same problem by blanking code spans first. This does not, and the
  cheap fix is available if it ever bites.
- It holds the drain, not the queue. Anyone who enqueues by hand, or arms with `gh pr merge --auto`
  directly, bypasses it entirely. The drain is the normal path, and this covers the normal path; it
  is not an interlock.
- Only the first `Blocked-by:` is read. A pull request sequenced behind two others can only say so
  once, and the honest workaround is to name the later one.

## A push after enqueue is silently discarded

The pull request keeps reporting the newer commit as its head (found 2026-09-16, milestone 304
(`cargo kani -p kernel` only ever compiled one architecture)). The queue merges the SHA it enqueued.
Push again before the group build lands, and GitHub updates `headRefOid` to the new commit, shows the
PR as merged, and then deletes the branch. So the merge commit's second parent is the *old* SHA, and
the last commit exists nowhere but a local object store. Milestone 304 lost a documentation commit
this way. It found it only because the lane happened to diff `origin/main` for its own content
afterwards. It was recovered by cherry-picking out of the pruned worktree's objects, and landed as its
own pull request.

The tell is that `gh pr view --json headRefOid` disagrees with
`git log -1 --format=%P <merge commit>`. Nothing reports it, and no check fails. Both halves look
correct in isolation: the PR says merged, `main` is green, and the diff a reader compares against is
the one that was enqueued.

Two habits cover it, and the first is nearly free. After a merge, confirm the work is on `main` by
content rather than by the PR's state (`git merge-base --is-ancestor <your last SHA> origin/main`).
And treat the moment a pull request is marked ready as the end of pushing. A lane that wants one more
commit should expect to open a second pull request for it. That is cheaper than the recovery above,
and is what happened here anyway.

## The reduced drain enqueued nothing for three hours

The reduced `merge-drain.sh` had not been run against a live queue, and the first time it was, it
enqueued nothing for three hours. Fixed 2026-08-17. The entry is kept because the prediction that
preceded it was right, and was not acted on. It said every claim about the script was read from the
source rather than observed, because it was written and shellchecked in a container with no `gh` at
all.

What the source could not show: `gh pr merge --auto --merge --delete-branch` fails outright when a
merge queue is enabled (`Cannot use -d or --delete-branch when merge queue enabled`). The call site
sent its output to `/dev/null` under `|| true`. So every pass printed `9 armed, 0 stalled` while the
queue sat empty and nothing merged. The flag was redundant as well as fatal. This repository sets
`delete_branch_on_merge`, so the platform deletes the head branch itself.

Two lessons, and the second is the reusable one. A count of *attempts* was being printed as a count
of *results*, which is the shape AGENTS.md's ladder calls rung zero wearing a uniform. And nothing on
a pull request object says it is in a merge queue. A queued pull request reports
`mergeStateStatus: CLEAN` with a null `autoMergeRequest`, because arming became membership.
`mergeQueue.entries` is the only authority. The obvious field looking authoritative while being wrong
is what cost the three hours. The verification now asks the queue.

## The drain judges a new pull request against a stale `main`

`merge-drain.sh`'s surface pass diffs each unlabeled pull request head against
`git merge-base origin/main <head>` in its own checkout, and runs
`helpers/architect-label-rules.py` on the result. That `main` is whatever the pass fetched, which
can be one merge behind the base a just-opened pull request really branched from. Observed
2026-10-15 (UTC) on #2202, an empty claim draft opened seconds after #2193 merged. The drain's
`main` predated the merge, so the empty diff contained #2193's edits to `design/decisions/0203` and
`0299`. The decisions rule fired, and the drain added `needs-architect` by its #1792 policy. The
add cost calef the glance the policy prices it at.

`architect-label.yml` is not exposed the same way: it resolves the pull request's own base at run
time and fired nothing on the same pull request. Its header records the same exposure for the
payload's cached `base.sha` (#1416); the drain's line carries it through its checkout rather than
the payload. A second, cosmetic defect sat on the same run: the `HELD #N for an architect (...)`
log line prints the previous record's causes, because `$causes` is assigned after the hold block
uses it, so #2202's hold logged `(orphan)` from an unrelated adoption. The fix, when a lane takes
it: diff against the pull request's real base fetched at pass time, or re-check a firing against a
fresh fetch before holding, and assign `$causes` before the hold block prints it.

## The watchers

- `lane-claim-check.sh` is only as alive as the drain is. It runs from the drain's pass and has no
  schedule of its own. It also reports to stdout only, because a branch with no pull request has
  nowhere to be commented on. So its findings reach whoever reads the drain's log and nobody else. Its
  own header carries the rest: `milestone/*` only, one page of activity feed, and that it sees a
  missing claim rather than the duplicate claim §90 (the claim is a draft pull request) actually
  fears.
- `at-risk-check.sh` was only as alive as `trunk-health.sh` while it was folded into it, the same
  dependency `lane-claim-check.sh` has on the drain, for the same reason: no schedule of its own. It
  also only sees the worktrees on the machine it runs from. So patagonia asleep means a worktree on a
  laptop taken elsewhere is unwatched regardless, which the recorded gap already names.
- `trunk-health.sh` polls at 90 seconds and reads only `main`. A release branch, if this tree ever
  grows one, is invisible to it.
- Neither reported its own death, and on 2026-08-18 that cost hours of red trunk. A killed watcher
  stops saying anything, which looks exactly like a healthy quiet queue. Milestone 723 (a stopped
  merge watcher is reported within three of its own intervals) now opens an issue when either stops. A
  dead drain still labels nothing until that issue is read.

## The measurement

- The measurement is a snapshot, and nothing re-derives it. Every number in it was taken by hand from
  the API on 2026-08-16 and pasted into prose. That is precisely the class milestone 125 (a number in
  the prose is a claim) exists to fix. Re-take them rather than trusting them, once the windows are
  wider than a day. The scripts that produced them were a lane's scratch files and were not kept,
  deliberately, because a throwaway analysis committed as a tool is a tool nobody maintains.
- "Opened to merged" is mostly a measurement of people. It is in the table because leaving it out
  would be picking the flattering metric. But it is dominated by how long a pull request waited for a
  human to enqueue it, and no arrangement of CI moves it. Read the enqueue-to-merged rows for anything
  about the queue.
- The eviction and the re-enqueue are the same timeline event pair. `added_to_merge_queue` and
  `removed_from_merge_queue` do not distinguish a candidate GitHub ejected from one a person removed
  and re-added. So the eleven re-enqueues counted cannot be attributed. The honest reading is an upper
  bound on the queue's own churn.
