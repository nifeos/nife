# A merge-queue ejection, caught before the queue and recovered after it

Milestone 630 (a merge-queue ejection is caught before the queue, and recovered after it),
2026-10-03 UTC, and its recovery half rebuilt the same day as milestone 727 (a queue eviction goes
to a maintainer session), provisional. The name of this note, the `needs-maintainer` label and the
`ready status` workflow are provisional.

Every merge-group attempt costs 20 to 38 minutes since swish-check joined CI. So an ejection costs
twice: once for the group that failed, and again for every retry that fails the same way. The
ejection also cancels auto-merge and tells nobody, which is the sixth class in
[the follow-through COE](coes/2026-09-30-lane-follow-through.md). Pull request #1473 met both at 01:45 UTC.
Its own roadmap block still read IN-PROGRESS, and `script/roadmap --check` can only see that
inside a merge group, because only there does the branch's merge commit exist.

## Before the queue: the ready check

`script/roadmap --ready-branch <branch>` reads frontmatter and nothing else. It fails when a block
still reads IN-PROGRESS on that branch, and names the block and the fix. `script/roadmap
--selftest` proves it against fixtures under `script/lint`.

With `--against <ref>` it also judges a `milestone/N-*` branch's own block against the base: the
branch must change milestone N's block (or the block a renumbered lane moved to), and the block must
not read IN-PROGRESS or NOT-STARTED, except that a branch which adds the block may leave it
NOT-STARTED. The workflow passes the base of the merge commit under test, `HEAD^1`. This check
exists because `script/lint-fleet` 4b skips on the detached HEAD of every CI run, so nothing else
required a milestone branch to move its block.

`.github/workflows/ready-status.yml` runs it on `pull_request` (opened, reopened, synchronize,
ready_for_review, converted_to_draft) and on `merge_group`. It reads the live draft state, and a
draft passes, because IN-PROGRESS is a draft's honest status. The pre-push hook asks the same
question when the pushed branch's pull request is ready, and skips silently without `gh`, a
network or a pull request.

Falsified live with #1491, a throwaway whose block read IN-PROGRESS on its own branch:

| Step | Run | Result |
| --- | --- | --- |
| Opened as a draft | 37088183446 | success |
| `gh pr ready` | 37088221768 | failure, 11 s after the event, fix in the log |
| `gh pr ready --undo` | 37088256676 | success |

It was closed and its branch deleted the same minute.

## The pre-push hook runs what fits in seconds

The hook grew one gate per lost queue cycle: clippy, two prose ratchets, then the baselines check.
Milestone 630 (a merge-queue ejection is caught before the queue, and recovered after it) made it
all of `script/lint`. Measured on patagonia with a warm target directory, three runs each, wall
clock, beside two lanes' QEMU:

| Command | Runs (s) | Median |
| --- | --- | --- |
| `script/lint --clippy` | 27.36, 32.90, 26.71 | 27.4 s |
| `script/lint` | 56.48, 55.20, 56.65 | 56.5 s |

That was a quiet moment. Later the same day, with the load average at 11 to 19, `script/lint` took
70.3, 110.7 and 119.4 s warm and 201.9 s with a cold target directory. A claim push sat in it for a
whole lane and lanes took to `--no-verify`. The hook also refused a claim push five times for a
roadmap block that cannot exist before the claim.

calef approved the narrower hook on #1564 (2026-10-03 UTC). It runs `script/fmt --check`, then
`script/lint --no-cargo`, then `--ready-branch`, and skips all three for a push whose commits
change no files. Clippy stays in CI. Measured on patagonia at load 15 to 17, wall clock:

| Step | Runs (s) |
| --- | --- |
| `script/fmt --check` | 1.8 |
| `script/lint --no-cargo`, warm tree | 41.6, 47.2, 44.7 |
| `script/fmt --check` and `script/lint --no-cargo`, no `target/` directory | 43.0 |

The proposal estimated 33 s from one loaded run. The measurement is 42 to 47 s, under a load average
of 15 to 17. The mode is CPU bound (about 46 s of CPU in those runs) and never touches `target/`, so
a cold target directory costs nothing extra. An empty push costs the
hook's own startup, under a second.

**The partition is read out of `script/lint`, not listed beside it.** `helpers/lint-no-cargo.awk`
splits the script at each `echo "==> ` header and drops every section with a line that invokes
cargo (a command whose first word is `cargo`, a Python `["cargo"` argv, or `command -v cargo`).
Prose that mentions cargo does not count. A new check therefore lands in exactly one bucket when it
is written, and a section that runs cargo is dropped unless it carries a `# no-cargo-ok:` marker
and reads `$LINT_NO_CARGO` to skip its cargo part. A marker on a section with no cargo, a marker
with no guard, and cargo before the first header each fail the mode. Of 65 sections, 46 run and 19
are skipped (12 clippy, rustdoc, and 6 that call `cargo metadata` or `cargo machete`;
`script/lint --no-cargo --list` prints them).

The earlier run found one defect too. git exports GIT_DIR to a hook, and
`helpers/scope-merge-base-selftest.sh` builds a throwaway repository with it still set. Its
`git init` and `git config` wrote `core.bare = true` and a fake identity into the shared
`.git/config` before it failed. Both were restored by hand, no commit carried the identity, and
the hook and the selftest now clear git's environment first.

## After the queue: a label for a maintainer session

Milestone 630 first built this as a hold: the drain commented on an ejection, labeled
`queue-ejected`, and declined to re-arm a failed head, re-arming everything else. calef's rulings on
#1564 (2026-10-03) removed every re-arm and re-queue, because the automation fought his own
dequeues and still left him as the only detector
([the correction](coes/2026-10-03-the-queue-judged-one-pull-request-at-a-time.md)). What replaced it
labels and never acts on the queue, with one marked exception (`stuck-head`, below).

`helpers/merge-drain.sh` asks GraphQL once per pass for every open pull request (its last
`RemovedFromMergeQueueEvent` and `AddedToMergeQueueEvent`, `mergeable`, auto-merge, and when it was
last unarmed), the queue's entries, and the closed pull requests still wearing the label.
`helpers/needs-maintainer.jq` decides. Each cause adds `needs-maintainer` and posts one comment per
episode, deduplicated by a marker:

| Cause | When | Evidence in the comment | Cleared when |
| --- | --- | --- | --- |
| ejected | last removal neither `merged` nor `manual`, nothing re-added it, same head | reason, time, head, the group's failed runs | back in the queue, or the head moves |
| conflict | ready and `mergeable` CONFLICTING | the conflicting files, from `git merge-tree` | the conflict is gone |
| stale | a queue entry whose pull request is merged or closed | entry state, enqueue and merge times, the dequeue command | the entry is gone |
| unarmed | ready, not armed, not queued, 30 minutes since it was last unarmed | since when, and any resolved `Blocked-by:` | armed, queued, or a draft again |
| off-main | ready, not armed, on a base other than `main`, 30 minutes since it was last unarmed | the base, its pull request, the ways out | armed, merged, an open `Blocked-by:`, or a draft |
| red | wearing `ci-failing` for 30 minutes | when that label went on, the head, whether armed | `ci-failing` comes off |
| stale-draft | a draft whose head commit is 6 hours old by committer date | the date, the branch, the four ways out | a commit, an open `Blocked-by:`, `parked`, or closed |
| stuck-head | a queue entry still `AWAITING_CHECKS` 10 minutes after its merge group's last run completed | the group's runs with conclusions, the dequeue command | the entry leaves `AWAITING_CHECKS` or its group changes |
| orphan | a branch with commits `main` lacks and no open pull request, its tip 2 hours old | the head, how many commits, the three ways out | landed, deleted, a pull request opened, or `parked` |
| unmergeable | an open pull request's queue entry reads `UNMERGEABLE` | its position, the entries ahead, which of them its head conflicts with, the dequeue command | the entry no longer reads `UNMERGEABLE` |
| budget | an open issue wearing `near-budget` | the issue's title and when it opened, the two ways out | the issue closes |

Every cause but `stale`, `orphan` and `unmergeable` needs a pull request from this repository, without `needs-architect` or
`held-for-red-trunk`. All but `stale-draft` need it ready, and `ejected`, `conflict` and `unarmed`
need it to be into `main`. A draft labeled `parked` (provisional name, 2026-10-06) is
exempt from `stale-draft` and `orphan`: it is held on purpose for work outside the lane system, such as
calef's GLM runs, and must carry a comment giving the reason. A `merge_conflict` ejection names no group commit, so it
cannot say which head was ejected and is cleared by the conflict going instead. `manual` is not an
ejection, because a person or `dequeue_held` meant it; if nobody follows up, it is `unarmed` 30
minutes later. `unarmed` and `off-main` are not raised beside `ejected`, `conflict` or `red`, which
already say what is wrong.

The last two arrived on 2026-10-04, from pull requests nobody owned until calef noticed them.
Pull request #1640 was stacked on #1630's branch and sat green, mergeable and unlabeled for about
three hours. `eligible` admits only a pull request into `main`, so every cause skipped it, and calef
merged it into its base by hand. Then #1617 and #1653 sat red under `ci-failing`, a label posted
once that no session reads, while their lanes had ended `WAITING`. `red` routes that label into this one
rather than teaching every session a second label, so `gh pr list --label needs-maintainer` stays
the whole queue. Its 30 minutes start when `ci-failing` went on, which a push resets.

The seventh, `stale-draft`, came on 2026-10-05. Draft #1644 was stacked on #1640 and #1630, its
lane's session ended, and it had no commit after 22:11 UTC the day before. Every other cause skips
a draft, because a draft is its lane's, and that only holds while the lane is alive. The age is the
head commit's committer date, since `updatedAt` moves with every bot comment and every retarget.
A draft holding only its claim commit is not exempt, as it is the clearest sign of a dead lane.

The eighth, `orphan`, exists because a worktree with no pull request is, in calef's words
(2026-10-06), "a problem that leads to lack of visibility and progress on that work." It reads remote branches, the
one ledger every session sees, and labels the pull request the branch last had: a merged one that
gained commits after it merged, or a closed one still holding work. A branch that never had a pull
request has nothing to label, so the drain opens a draft for it, marked with the head it adopted.
Every branch prefix is in scope: the survey in milestone 580 (nobody reads branches) found every
prefix leaking, and so did this one. `parked` on the last pull request exempts it, which is how
argon's two closed branches wait for their board (#1738, #1732). Work that never left a laptop is not on GitHub at all;
`helpers/at-risk-check.sh` lists it at prune time.

The ninth, `unmergeable`, came on 2026-10-07. #1795 sat in the queue behind #1745 with its entry
reading `UNMERGEABLE`. The two conflicted in `notes/package-boundaries.md` while each merged cleanly
with `main`, so `conflict` never fired. The queue does not eject such an entry, so `ejected` never
did either. calef found it on the queue page. The cause reads the entry's state from the
query the drain already ran, and the comment compares the head with each entry ahead using `git
merge-tree`, pair by pair. Its limits (no grace period, pairs rather than the whole group) are in
the BUGS in `helpers/merge-drain.sh`.

The tenth, `budget`, came with milestone 808 (every gate accounts for its time). It is the first
cause on an issue rather than a pull request. `helpers/ci_job_times.py sync` opens a `near-budget`
issue when a CI job's merge-group runs pass about 85% of its budget. §254 (a gate prints what each
item cost) ruled that it must reach a session through this listing. The drain finds open ones
through a search of their own, so an old one is not lost below the fifty newest issues. The
session's command became the REST issues endpoint, which returns pull requests and issues together.

The event's `beforeCommit` is the group's merge commit, not the head, which was a surprise. Its
second parent is the head that was enqueued, and the group's runs are the `merge_group` runs at
`beforeCommit`.

The split milestone 630 measured still matters to whoever reads the comment. Of the 18
`failed_checks` ejections by 2026-10-03, 5 had a CI run conclude `failure` and 13 had CI
`cancelled` with every other workflow green, and #1454 merged on its fifth attempt at an unchanged
head. So the comment lists the group's runs by conclusion: a cancellation is usually runner supply,
and re-arming is the session's call to make, not the drain's.

`helpers/needs-maintainer-selftest.sh` feeds the decision a response recorded live at
2026-10-03T23:54:15Z, when #1569 had just been ejected on a `merge_conflict`, and one case per
cause and per clearing. A `--dry-run` pass against the live repository the same minute labeled
#1569 and nothing else. The scheduled workflow runs only from `main`, so the first real label comes
after merge.

The log's event lines:

```console
$ gh run view <run id> --log | grep -E 'LABELLED|CLEARED'
```

## A finished group the queue never acted on

`stuck-head` (lane/drain-stuck-queue-head, 2026-10-07 UTC; name provisional) is the case where
nothing failed visibly and nothing moved. #1824 was enqueued at 16:38 and sat at position 1 in
`AWAITING_CHECKS` until the maintainer dequeued it at 19:42, blocking every entry behind it. Its
group's runs (CI 37653600611, verify 37653600565) were `completed` with conclusion `failure` by
17:01, yet every job listed in them was green. A required check never posted, so GitHub neither
merged nor ejected it. `gh run rerun --failed` refused. #1829 (`missing-check`) covers the
armed, not-yet-queued half of the same family; this covers the queue entry.

The rule: an open pull request's entry reads `AWAITING_CHECKS`, its group has at least one
`merge_group` workflow run, every one is `completed`, and the latest completed 10 minutes ago or
more. `helpers/stuck-head.jq` summarizes the runs; `nm_stuck_head` in
`helpers/needs-maintainer.jq` applies the age.

**The threshold runs from the last completion, not from the group's start.** Group durations are
the wrong clock: 13 finished groups on 2026-10-07 took 4.6 to 29.3 minutes. The latency that
matters is from the last run completing to GitHub acting. For four merges (#1812, #1816, #1818,
#1821) it was 0.2, 0.2, 0.4 and 0.6 minutes (the group's latest `updated_at` against the
`RemovedFromMergeQueueEvent`). #1824 was 161 minutes. Ten minutes is about 17 times the slowest
healthy latency and loses ten minutes, not three hours. Pick it again if a slower healthy
latency turns up.

**This is the one exception, and a foot gun to extend** (calef, #1833, 2026-10-07 UTC). The first
stall of a pull request makes the drain dequeue the entry and enqueue it again at the back
(`requeue_stuck_head` in `helpers/merge-drain.sh`), then comment with the group's run ids. A second
stall, found from the drain's own earlier `needs-maintainer:stuck-head` comment, gets
`needs-maintainer` and no re-queue. The #1564 rulings still hold for everything else: selftest
section 5 cuts that one function out before it greps for arming and enqueueing, requires exactly
one caller, and plants a re-arm in `dequeue_held` to prove the gate still bites. A second cause
that re-queues would be a new ruling, not an extension of this one.

The drain's token can do it. `dequeue_held` already dequeues under the App's `pull-requests:
write`, and `enqueuePullRequest` reached GitHub's eligibility logic under that token on
2026-09-24, as the workflow's header records. No live re-queue has run yet; the workflow only
runs from `main`.

## BUGS

- A labeled pull request waits for a maintainer session. With none running, it waits for the
  next one, which is slower than the drain's old re-arm and is not calef's job.
- An ejection is labeled at the drain's next pass, which follows the group's CI completion through
  `workflow_run` but can trail it under load. The first pass after this lands also labels any open
  pull request whose last removal was a current ejection.
- An armed pull request whose required checks never report, or that auto-merge never enqueues, is
  none of the causes. One whose checks fail is `red`. The drain used to comment on the first and enqueue the second.
- `--ready-branch` matches by branch name. A block naming the wrong branch passes it and still
  fails in the group.
- `notes/check-inventory.md` does not list `--ready-branch` or the hook's wider lint. That table is
  a 2026-09-03 snapshot already over its word budget, and a row costs words it does not have.
- `stuck-head` does not see a group whose runs never finished (a run stuck `in_progress` or
  `queued`), nor one with no runs at all. Those wait on GitHub's own timeout. Recorded response:
  `helpers/needs-maintainer-fixtures/stuck-head-runs-1824.json`.
