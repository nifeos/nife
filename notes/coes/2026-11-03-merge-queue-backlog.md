# A correction of error: the merge queue drained slowly, and 33 pull requests were left waiting

*Recorded 2026-11-04 (UTC) by lane/coe-merge-queue-2026-11-03, commissioned the same day. Title
provisional. The window is the Pacific day of 2026-11-03 (08:00 UTC on 11-03 to 08:00 UTC on 11-04).
This lane read the data at 05:58 UTC, so it lacks the last two hours.*

## Summary

1. The tree merged 32 pull requests in the day, and 33 ready pull requests were still open at the end of it.
2. From 15:30 UTC the hosted runner pool sat at its cap of about 60 jobs, with up to 670 jobs waiting,
   because each push to a pull request ran the full heavy gate set.
3. So a pull request waited a median 100 minutes for its own checks, and a merge group took 74.5
   minutes instead of 18. #3176 moved the heavy gates into the queue at 23:56 UTC.
4. Meanwhile every naming pull request edited the same names note, so each merge made the others
   conflict: the queue removed 24 entries for merge conflicts, 14 of them in one minute.
5. After #3176 the queue could take about 16 merges an hour, but 31 of the open pull requests were
   in conflict and the queue sat nearly empty.

## Impact

| measure | value |
|---|---|
| merged in the window | 32 (29 merge commits; #3123 and #3124 landed inside #3117's) |
| open at 05:58 UTC, not draft | 33: 31 in conflict (26 naming, 5 line-citation), 2 clean and held by `needs-architect` |
| queue entries | 84, from 57 pull requests |
| queue removals other than a merge | 55: 24 `merge_conflict`, 24 `manual`, 5 `invalid_merge_commit`, 2 `failed_checks` |
| time from first arming to merge | median 4.3 h, p90 9.8 h, longest 23.6 h (#3117) |
| time from last queue entry to merge | median 62 min, p90 181 min |
| workflow runs created | 3,842: 2,829 `pull_request`, 646 `workflow_run`, 253 `merge_group`, 88 `push` |
| runs cancelled | 1,142 (687 `pull_request`, 445 bot `workflow_run`, 1 `merge_group`) |
| job minutes in the heavy workflows | 45,793, of which `pull_request` 35,541 (78%) and `merge_group` 7,937 (17%) |
| job minutes in cancelled jobs | 6,148, all `pull_request` |

By hour. "Waiting" counts hosted jobs created and not yet started, at the top of the hour. From
08:00 to 12:59 UTC (00:00 to 04:59 PT) nothing entered the queue.

| UTC | PT | merged | entered queue | removed, not merged | depth at end | jobs waiting |
|---|---|---|---|---|---|---|
| 13 | 05 | 1 | 1 | 0 | 0 | 0 |
| 14 | 06 | 1 | 1 | 0 | 0 | 0 |
| 15 | 07 | 0 | 4 | 0 | 4 | 0 |
| 16 | 08 | 1 | 1 | 4 | 0 | 278 |
| 17 | 09 | 0 | 0 | 0 | 0 | 670 |
| 18 | 10 | 1 | 8 | 0 | 7 | 330 |
| 19 | 11 | 0 | 8 | 3 | 12 | 590 |
| 20 | 12 | 1 | 15 | 8 | 18 | 465 |
| 21 | 13 | 4 | 14 | 1 | 27 (28 at peak) | 451 |
| 22 | 14 | 4 | 7 | 23 | 8 | 225 |
| 23 | 15 | 5 | 2 | 4 | 1 | 260 |
| 00 | 16 | 1 | 8 | 3 | 5 | 279 |
| 01 | 17 | 6 | 4 | 2 | 1 | 27 |
| 02 | 18 | 1 | 1 | 1 | 0 | 0 |
| 03 | 19 | 4 | 6 | 4 | 0 | 0 |
| 04 | 20 | 2 | 3 | 1 | 0 | 0 |
| 05 | 21 | 0 | 1 | 1 | 0 | 0 |

From 16:00 to 01:00, 58 to 72 hosted jobs ran at most samples. Runs with a waiting job numbered 82
to 124 from 16:00 to 21:00, 52 to 60 until 00:00, and 2 at 01:00.

Before and after #3176 (merged 23:56 UTC):

| | before | after |
|---|---|---|
| `merge_group` CI run, wall time | median 74.5 min, p90 143.7 (16 runs) | median 18.2 min, p90 22.7 (14 runs) |
| `merge_group` job, wait for a runner | median 26.0 min, p90 56.2 | median 0.1 min, p90 4.0 |
| `pull_request` CI run, wall time | median 100 min, p90 136 | median 5 min |
| `pull_request` verify run, wall time | median 115 min | under 2 min |
| arming to entering the queue | median 159 min, p90 365 (54 entries) | median 68 min (18 entries) |

## Timeline

| UTC | PT | what | source |
|---|---|---|---|
| 11-03 15:00 to 16:10 | 07:00 to 08:10 | 25 naming pull requests (#3133 to #3156) are opened and marked ready | pull requests |
| 16:10:04 | 08:10 | #3132 merges; #3133, #3134, #3135 and #3137 are removed for conflicts within 68 s | queue events |
| 16:10 to 18:21 | 08:10 to 10:21 | the queue is empty while armed pull requests wait for their own checks | queue events |
| 18:31 to 18:55 | 10:31 to 10:55 | four groups start; they take 118, 144, 146 and 157 min | run list |
| about 21:54 | 13:54 | a local development host restarts and every in-process lane stops; 13 of 29 entries read UNMERGEABLE | maintainer handoff |
| 22:05 | 14:05 | #3176 (the heavy gates only in the queue) is marked ready, built on #3175 | pull requests |
| 22:57:52 to 22:58:45 | 14:57 | #3160 and #3168 merge; 14 entries are removed for conflicts | queue events |
| 23:25 | 15:25 | #3181 (no totals in the names note, blank-line baselines) is opened | pull requests |
| 23:56 | 15:56 | #3176 merges | pull requests |
| 11-04 00:39 to 00:56 | 16:39 to 16:56 | #3175, now an empty diff, builds a group, fails `empty diff` (run 41748506618) and is closed | run list |
| 01:34 | 17:34 | #3181 merges | pull requests |
| 03:04 to 05:55 | 19:04 to 21:55 | the session account enqueues #3186, held since 02:27, three times; the drain dequeues it each time | queue events |
| 03:40 | 19:40 | #3117 merges with #3123 and #3124; #3118, #3120 and #3122 are removed for conflicts | queue events |
| 04:54 | 20:54 | #3180 (stage 1 of the owned runners) merges; `NIFE_RUNNER_OWNED` stays unset | pull requests, jobs |

## The bound on throughput

Ruleset 19596094 builds up to 5 entries at once, merges up to 5 together, requires each green
(`ALLGREEN`), and waits 1 minute for a group of one. Each entry ran its own group (39 group CI runs),
so the ceiling is about 5 merges per group wall time.

- Before #3176: 5 per 74.5 min is 4.0 merges an hour. From 18:00 to 23:56 the tree merged 15, or 2.5
  an hour, with 3 to 5 groups building. The rest of the gap is the removals, which discard a build.
- After #3176: 5 per 18.2 min is 16.5 an hour. From 00:00 to 05:58 the tree merged 14, or 2.3 an
  hour, with the queue at depth 0 to 5. The queue was not the bound; conflicts were.
- Capacity: a group spends about 200 job minutes (170 in CI, 32 in verify) over 18 minutes, about
  11 jobs at once. Five such groups need about 55 of the 60 hosted slots, so a larger build window
  buys nothing without more runners.

Loss by cause, in PR-hours armed or queued and not merging: 299 before 23:56 and 41 after.

| rank | cause | measured or estimated loss |
|---|---|---|
| 1 | heavy gates on every push saturated the hosted pool | about 150 PR-hours: 54 entries, each paying about 110 min of extra check time and 56 min of extra group time |
| 2 | the shared names note made merges conflict | 24 removals; the 14 of 22:58 left the queue for good (13 still out at 05:58, about 91 PR-hours), and 31 pull requests were in conflict at the end |
| 3 | a push to a queued pull request means a dequeue | 21 dequeues by the session account, 13 with a push within 5 min; 14 came back after a median 75 min, 32 PR-hours in all |
| 4 | five `invalid_merge_commit` removals | #3135 alone spent 3.6 hours between its first conflict and its last re-entry; the cause is not known |
| 5 | automation working against itself | #3186 cycled three times through the queue and the drain; #3175 spent one 23-minute group on an empty diff |

## Root causes

### 1. Every push ran the heavy gate set, against a 60-job pool

1. Why did the queue drain slowly? A merge group took a median 74.5 minutes, and an armed pull
   request waited a median 159 minutes to enter the queue.
2. Why so long? Merge-group jobs waited a median 26 minutes for a runner, and pull-request runs waited
   on the same pool: hosted jobs ran at about 60 with 225 to 670 waiting from 16:00 to 00:00.
3. Why was the pool full? Pull-request runs took 78% of all job minutes. 39 pull requests opened within
   two bursts (15:00 to 16:10 and 18:10 to 18:21) pushed 206 distinct heads, and every push ran the
   full CI and verify sets, kernel mutants and CodeQL.
4. Why did every push run them? A pull request ran the same jobs as a merge group until #3176. The
   ruling came on 2026-11-03, and #3176 merged at 23:56 because it too waited behind the same pool.
5. Why was the pool still 60? Milestone 965 (gating CI rents more concurrency before it owns a runner)
   measured the same cap binding on 2026-10-24, and its decision is unwritten. The owned pool
   (#3180) merged at 04:54 behind a variable that is still unset.

### 2. Every naming pull request rewrote the same lines of one note

1. Why were 31 pull requests in conflict at the end? Each merge changed lines the others had changed.
2. Which lines? Against the main of 22:57:52, 11 of the 21 removed entries this lane re-merged
   (`git merge-tree`) conflict on `notes/unratified-names-journey-5.md`, three on
   `helpers/line_citations.py` and two on `designware_mobile_storage`'s `internal_direct_memory_access.rs`.
   The other six conflict with entries ahead of them in the queue.
3. Why did they share lines? The note carried totals and count tables that every group's pull request
   updated, and the two baselines were dense sorted lists. #3181 removed the totals and spaced the
   baselines at 01:34, after the cascade.
4. Why did the fix come after the lanes? The ruling of 2026-11-03 put the rework after the queue
   drained, and the maintainer role launched about 25 naming lanes at once before it.
5. Why did no check stop that? The rule that bounds lane count by the collision surface lives in the
   maintainer skill and in `briefs/run-a-milestone-wide.md` as prose. Nothing reads a planned set of
   lanes for shared files, and nothing refuses an entry that conflicts with the queue ahead of it.
   GitHub held 13 UNMERGEABLE entries in the queue at 21:54 and removed them only at a merge.

### 3. A push to a queued pull request costs its place, and re-arming is by hand

1. Why did entries leave the queue by hand 21 times? Lanes and the maintainer session pushed catch-up
   merges to queued branches, and a push requires a dequeue first (`helpers/push-queued-check.sh`).
2. Why did that cost 75 minutes each? A pushed head must pass its pull-request checks again, behind the
   same full pool, and enters the queue at the back.
3. Why was the push needed? Cause 2: the entry already conflicted.
4. Why does the push not re-arm? The hook prints three steps for a person to run. Re-arming came
   within 23 minutes in all but one case, so idle unarmed time is not a measured loss; but at 06:00
   #3120 and #3122 sat clean with auto-merge off.
5. Why is it by hand? The rulings of 2026-10-03 took re-arming away from the drain, because it fought
   the account's own dequeues, and nothing took its place on the push side.

### 4. The session enqueued what the drain was holding

#3186 took `needs-architect` at 02:27. The drain dequeues a held pull request (`dequeue_held`), and
the session account enqueued it again at 03:04, 04:52 and 05:54, and the drain discarded each build.
Why: arming does not read the hold labels, and the session did not either. #3175 is the same shape:
it stayed armed after #3176 absorbed its diff, and one group spent 23 minutes proving it empty.

## What went well

- #3176 cut the merge-group wall time from 74.5 to 18.2 minutes and a pull request's own checks from
  100 to 5, on the day the cause surfaced.
- #3181 removed the shared lines, so catch-up merges now apply cleanly.
- The drain held `needs-architect` correctly every time, and no held pull request merged.

## What went badly

- The maintainer role launched about 25 lanes into one note within 70 minutes, and 14 more at 18:10,
  while the hosted pool was already full. It applied neither the collision-surface rule nor the
  runner ceiling the maintainer skill names.
- The maintainer role put the structural fix (#3181) after the drain, which needed it.
- The fix to the pool came as two pull requests, #3175 and #3176 on top of it, and #3175 stayed
  armed after #3176 merged.
- The maintainer's read at 21:54 took a queued entry's empty auto-merge field (GitHub's normal
  state) for a disarmed one, so its re-arm list was partly wrong.

## Where we were lucky

- The queue's check timeout is 240 minutes. Ten groups took more than 60 and the longest 157; at the
  old 60-minute timeout the queue would have evicted every one.
- `fuzz-guests` failed in 6 of the 39 group runs, and it is not a required check, so the queue
  merged those groups anyway.

## Action items

Each proposal and milestone below names this file and its row, under its title.

| # | what | status | record |
|---|---|---|---|
| 1 | The heavy gates run only in the merge queue, and the `full-ci` label runs them on demand. Priority 1. | delivered | **Done.** #3176, merged 2026-11-03 23:56 UTC |
| 2 | The names note carries no totals, and each baseline row stands alone between blank lines. Priority 1. | delivered | **Done.** #3181, merged 2026-11-04 01:34 UTC |
| 3 | Stage 1 of the owned runners, behind `NIFE_RUNNER_OWNED`; setting the variable is the architect's. Priority 1. | delivered | **Done.** #3180, merged 2026-11-04 04:54 UTC |
| 4 | Rent more hosted concurrency, or not: the same cap bound again. Priority 1, owner the architect. | proposed | **Milestone 965.** Milestone 965 (gating CI rents more concurrency before it owns a runner) |
| 5 | Choose the build window, the group size and stage 1's switch together, against measured capacity. Priority 1, owner the architect and a lane. | proposed | **Milestone 1042.** Milestone 1042 (the merge queue is sized to the runner pool) |
| 6 | Merge a pull request against the queue's tip before arming it, and name lanes that share a file before a wide run launches. Priority 2, owner the maintainer role and a lane. | proposed | **Milestone 1054.** Milestone 1054 (conflicts) |
| 7 | `script/push` dequeues, pushes and re-arms in one step, and arming refuses a held pull request. Priority 2, owner a lane. | proposed | **Milestone 1052.** Milestone 1052 (re-arms) |
| 8 | The unexplained `invalid_merge_commit` removals and the `fuzz-guests` failures in group runs. Priority 3. | delivered | **Recorded.** BUGS entries in `notes/merge-queue/limitations.md`, "Removals the queue gives no reason for" |
| 9 | A correction's action items name their records, and those records name the correction, checked by `script/roadmap --check`. Priority 2. | delivered | **Done.** #3188: the table form and the back-reference check in `helpers/coe_actions.py` |

## How the numbers were made

```
# pull requests updated since 11-02 20:00: queue, auto-merge, ready, label and commit events
gh api graphql   # pullRequests(orderBy: UPDATED_AT) { timelineItems(itemTypes: [...]) }
# runs in windows under the API's 1,000 cap, then jobs of the heavy workflows (1,337 runs)
gh api --paginate "repos/nifeos/nife/actions/runs?created=<from>..<to>&per_page=100"
gh api "repos/nifeos/nife/actions/runs/<id>/jobs?filter=all&per_page=100"
git merge-tree --write-tree --name-only <main at removal> <head then>
gh api repos/nifeos/nife/rulesets/19596094
```

A job waits from `created_at` to `started_at`; one cancelled before it started never waited. A PR-hour is one pull request armed or queued for one hour, cut at 05:58 UTC.

## BUGS

- Cause 1's loss charges every entry the median excess; some paid less.
- The workflow-scope gap (lane tokens cannot arm a pull request that changes a workflow) left no
  measurable mark: every ready-to-armed gap was under 7 seconds.
- The cause of the `invalid_merge_commit` removals, and how long the host restart kept lanes from
  pushing; the maintainers keep unit-specific details of the restart off-tree.
- GitHub keeps no history of a pull request's mergeable state, so the PR-hours above mix time in
  conflict while armed with time waiting for checks.
- Samples above 60 running jobs (93 at 15:30, 91 at 19:00) may count jobs that ended within the
  sample, and the API does not report the cap.
