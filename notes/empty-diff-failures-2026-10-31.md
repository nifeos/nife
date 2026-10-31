# Why the `empty diff` job fails: 609 failures in 14 days, sorted

*Name: provisional, minted 2026-10-31 (UTC) by lane maintainer/empty-diff-failures-sorted.*

Part of [how the tree is run](README/how-the-tree-is-run.md). This note answers the question in the
proposal `find-why-the-empty-diff-job-fails-first` (pull request #2968). It changes no workflow. The
job is `.github/workflows/empty-diff.yml` and the script it runs is `helpers/empty-diff-check.sh`,
both from milestone 627 (a pull request that changes nothing does not merge).

## The answer

The hypothesis holds. Almost every failure is a draft that carries only its `claim:` commit, and the
job fails it as designed. Of 609 failed runs, 577 (95%) are that case on a draft. Nobody waits on
one. A person or a lane paid time on about 30 runs (5%), and most of those cost two to three minutes.

## What was collected

Every run of the `empty diff` workflow on the `pull_request` event from 2026-10-17 through
2026-10-31 (UTC). The API stops at 1,000 results per query, so the list came one day at a time.
There are 2,552 runs on 697 pull requests. Of these, 1,939 succeeded, 609 failed, 2 were cancelled
and 2 still ran when the list was taken. For each failed run the collection fetched its job, its
pull request's commits, and the pull request's draft and ready events.

- Every failed job failed at the one step that compares trees, after 6 to 53 seconds (median 9).
  None failed at checkout, so none is infrastructure.
- Every failed run was attempt 1. Nobody re-ran a failure.
- All 574 pull requests that ever failed failed on their first run. The other 123 pull requests
  opened with a real commit and passed first time.
- The 609 failures cost 5,722 runner-seconds in all, about 95 minutes.
- Draft state at the time of a run comes from replaying the pull request's ready and
  convert-to-draft events. A pull request with no event keeps its current state.
- A head counts as claim-only when every non-merge commit up to it has a `claim:` subject. Eleven
  runs had heads that a lane had rewritten away from the pull request, so the commit list could not
  show them. For these the head commit itself shows one parent, no files and a `claim:` or `Claim:`
  subject.

## The classes

| class | runs | pull requests |
|---|---|---|
| fails by design: claim-only head, pull request was a draft | 577 | 573 |
| claim-only head, pull request was already marked ready | 28 | 26 |
| real empty diff | 4 | 3 |
| cancelled by a label change | 0 | 0 |
| infrastructure | 0 | 0 |
| other | 0 | 0 |

The four real empty diffs are the falsification draft #2420 (an empty commit without a `claim:`
subject, kept to show the check fires), a draft whose merge of `main` left its tree equal to its
base (#2719), and two runs on #2618, a ready pull request whose work had already landed on `main`.
#2618 closed unmerged.

No failure was a label-change cancellation. The workflow starts on opened, reopened, synchronize
and ready for review, so a label never starts it, and its concurrency group never cancels a run in
progress. The two cancelled runs were on branches `comments/929-kernel-syscall-selftest` and
`comments/929-crates-09`. They ran 52 seconds and 5 minutes, so neither is a zero-second
cancellation. They are not failures and are left out of the 609.

## What cost anyone time

A draft nobody was waiting on: 577 runs, 95% of failures. The median wait from that failure to the
next passing run was 27 minutes, which is the time the lane spent writing the first real commit.
That is the draft being a draft. It is not a hold. Of those 573 pull requests, 539 merged, 23 closed
unmerged and 11 never had a later run.

A pull request that was ready: 30 runs (5%) on 27 pull requests.

- 28 runs on claim-only heads of ready pull requests. Each run started about two seconds after the
  ready event, so the lane marked the pull request ready before its real commits reached GitHub.
  The next push passed. Eighteen of the 28 passed within 4 minutes of the failure, a median of 2.6
  across all 28. Six took 10 to 20 minutes. Four took 40, 44, 154 and 359 minutes. Those four are
  upper bounds on a hold, since the lane may have kept working on the branch. All 28 later passed,
  and 25 of the 26 pull requests merged.
- 2 runs on #2618, a real empty diff. The check was right: there was nothing to merge.

So the check held a ready pull request for at most a few minutes in about two thirds of its ready
failures, and for longer in four. Summed, the class comes to about 12 hours at its widest
reading, and the four long cases supply most of that.

## What this says about the bottleneck

The 53-of-83 figure in the proposal holds up: here 574 of 697 pull requests (82%) failed `empty
diff` first. It overstates a problem, because those are claim drafts, and the check is right to
fail them. They sit before the first real commit, not before the merge queue. A ready pull
request was held in 28 of 2,552 runs (1%) by a claim-only head and in 2 by an empty diff. This job
is not where the 25-minute pre-queue median goes. Measure the pre-queue median from the
first ready event, which sets these drafts aside.

## Smallest change that removes the cost

Skip the job on drafts, and only there. In `.github/workflows/empty-diff.yml`, give the job
`if: github.event_name != 'pull_request' || !github.event.pull_request.draft`. A job that its
own `if` skips reports as skipped, and GitHub counts a skipped required check as satisfied.

This does not hide what milestone 627 exists to show. The check still runs on `ready_for_review`,
when a pull request stops being a draft, and on `merge_group`, the authoritative half. A claim-only
pull request can only merge after both. The merged-with-nothing incident behind 627 was a claim pull
request that went ready and merged, which both runs still catch. The proposal refused
skipping drafts on the worry that it hides the failure. These two runs keep it visible where it matters.

Open questions before a lane makes it:

- Confirm that a skipped job satisfies the required context in the ruleset. The ruleset requires
  the job name, so test it on a draft before relying on it.
- `helpers/empty-diff-selftest.sh` fails four shapes and passes one. It tests the script, not the
  workflow condition, so #2420, the live falsification, needs a ready counterpart.
- Removing the red mark on drafts also removes the claim-time hint, which tells a lane that its
  claim pull request has no work yet. The hint still prints at the first ready flip.

Leave the 28 ready-before-push cases alone. They cost a median of 2.6 minutes, and the check is
doing its job. If they matter, the fix is in the lane's order: push, then `gh pr ready`.
