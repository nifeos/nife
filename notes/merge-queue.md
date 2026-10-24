# The merge queue, and the three things that watch it

Three scripts, all maintainer tools rather than front doors. Two were born on 2026-08-04 out of the
same evening's failures: `helpers/merge-drain.sh`, which landed what did not need an architect
and since 2026-10-03 labels what a maintainer session must pick up, and `helpers/trunk-health.sh`,
which says when `main` is red. `helpers/lane-claim-check.sh` joined them on
2026-08-31 and watches one step earlier, for work that has not reached the queue at all. A fourth,
`helpers/at-risk-check.sh`, joined on 2026-09-23 and watches earlier still, for uncommitted work
sitting in a lane worktree. It was first called from `helpers/trunk-health.sh`'s loop, so "three
things that watch" named the count of things that run unattended in this repository's automation.
Since 2026-09-24 it runs from `launchd` on each developer's own machine instead (its section says
why). Names are provisional.

The evidence and history behind each section are in [`merge-queue/`](merge-queue/README.md), and
each section links the appendix it summarizes.

## Why they exist rather than being someone's job

The roles in CLAUDE.md are Maintainer, Developer, Steward. On 2026-08-04 three things went wrong in
one evening and all three were the same shape: a duty that belonged to whoever happened to notice.

- Two green pull requests sat unmerged for hours because nobody armed auto-merge on them. Not a
  judgment call, not a policy: they were opened and forgotten.
- `main` went red and nobody owned it. A developer cannot see `main` by design. The steward
  watched pull request checks and never the trunk. The maintainer's hygiene list is prune the
  worktree, delete the branch, relink `nife-dev`, leave no QEMU, and does not mention it.
- Merging one pull request staled the other eight under the new up-to-date rule, and nothing
  picked them back up until calef asked.

The pattern is the one milestone 92 (security audits as a mechanism) argues about audits. A practice
that lives in memory gets skipped exactly when it matters, and the maintainer is structurally worst at this particular duty because
merging happens *between* conversations rather than during them.

A steward that reports and never acts does not cover it. "The queue is stalled" arriving in a
message is only useful if someone reads the message and does something. These two scripts act. (The
drain acts by labeling rather than arming: see its section.)

## `helpers/merge-drain.sh`

```console
$ helpers/merge-drain.sh --dry-run
merge-drain[patagonia]: (dry run) would run: gh pr edit 1569 --repo nifeos/nife --add-label needs-maintainer
merge-drain[patagonia]: LABELLED #1569 (conflict, ejected)
merge-drain[patagonia]: (dry run) would run: gh pr comment 1569 --repo nifeos/nife --body merge-drain[patagonia]: needs-maintainer. CONFLICTS with `main` ...
```

That is a real pass, against the live repository at 2026-10-03 23:54 UTC, minutes after #1569 was
ejected on a `merge_conflict`. Without `--dry-run` the same pass makes the writes.

**Since 2026-10-03 it arms nothing** (milestone 727 (a queue eviction goes to a maintainer session), provisional). calef ruled on #1564: *"I don't
want the job of watching the queue."* That day the drain armed every eligible pull request on every
pass, re-armed ejected ones, enqueued what auto-merge had stranded and commented on stalls, and
calef was still the only thing that noticed an ejection. It re-queued pull requests the calef
account had just taken out six or seven times, and its 478 stall comments moved nobody. The record
is [the 2026-10-03 queue correction](coes/2026-10-03-the-queue-judged-one-pull-request-at-a-time.md).
A lane now arms its own pull request in the same command as `gh pr ready`.

What a pass does, in order:

- runs `helpers/lane-claim-check.sh` (below);
- dequeues a pull request that picked up `needs-architect` or `held-for-red-trunk` after it was
  enqueued, the one case the queue cannot see because it does not read labels;
- labels a paused draft `unblocked` once its `Blocked-by:` pull requests resolve
  ([blocked-by-drafts.md](blocked-by-drafts.md));
- reruns, once, a CI run a concurrency group cancelled as a same-second duplicate (BUGS, below);
- labels `needs-maintainer` (name provisional) on a pull request a maintainer session must pick
  up, comments once per cause with the evidence, and takes the label off when the cause goes.

The causes and their comments are in [queue-ejection.md](queue-ejection.md). The decision is
`helpers/needs-maintainer.jq`, checked by `helpers/needs-maintainer-selftest.sh` against a recorded
response, and that selftest also fails if the drain arms or enqueues again.

The `unarmed` cause waits out CI. A ready pull request whose head has a `CI` or `verify` run queued
or in progress is not counted, because the gate-in-CI flow (`briefs/gate-in-ci.md`) leaves it ready
and unarmed for the whole run by design. The 30 minutes restart when the last of them finishes.

The session-side half is rung three. `briefs/session-start.md` reads
the `needs-maintainer` listing before anything else, and `helpers/nanny.py` wakes a running
session when the label lands.

Its event lines (`DEQUEUED`, `RERAN`, `UNBLOCKED`, `LABELLED`, `CLEARED`) each print once per
transition, so they can be counted across passes. A snapshot read late is merely stale; a flow read
late lands in the wrong bucket, which is the correction in `script/metrics` and
[project-metrics.md](project-metrics.md).


## `helpers/lane-claim-check.sh`

```console
$ helpers/lane-claim-check.sh
lane-claim-check: LEFTOVER. milestone/121-ripgrep's #600 is MERGED; delete the branch
lane-claim-check: UNCLAIMED. milestone/194-falsification-roadmap-status has no pull request after 16 minutes. AGENTS.md §90: gh pr create --draft
```

It watches for a lane that pushed a branch and opened no pull request, which is invisible to
everything that starts from `gh pr list`. The draft is the claim (AGENTS.md §90 (the claim is a
draft pull request; the status flip is a gate)), so a branch with none is a milestone two lanes could
take at once. It runs first in the drain's pass. A 15-minute grace from the branch's birth, a
separate `LEFTOVER` line, and counting any open pull request keep it from crying wolf. It is a report,
not a gate. [`merge-queue/lane-claim-check.md`](merge-queue/lane-claim-check.md) has why each choice
was made.

## What the merge queue took over

GitHub's merge queue serializes candidates, tests each against the tip and ejects what fails, which
the drain had been reconstructing from outside. On 2026-08-16 the drain lost about 150 lines to it.
Turn the queue off and the four failures the drain went through before it return, because under the
up-to-date rule a merge stales every other branch. The queue is also the prevention half: it tests a
candidate against the tip, which catches two branches green alone and red together, and that is the
better half. These scripts only detect. [`merge-queue/what-the-merge-queue-took-over.md`](merge-queue/what-the-merge-queue-took-over.md)
has the four shapes and the evening that motivated it.

## How they run: in Actions, as `nife-smelter[bot]` (2026-09-24)

Scheduled workflows:

| Workflow | Runs | Identity |
| --- | --- | --- |
| `.github/workflows/merge-drain.yml` | `helpers/merge-drain.sh`, which calls `helpers/lane-claim-check.sh` inside its own pass | `nife-smelter[bot]` |
| `.github/workflows/trunk-health.yml` | `helpers/trunk-health.sh --once`, and fails the run when `main` is red or a cadence is dead | `nife-smelter[bot]` |
| `.github/workflows/ci-failing.yml` | `helpers/ci-failing.sh` ([ci-failing.md](ci-failing.md)) | `nife-smelter[bot]` |
| `launchd`, per developer | `helpers/at-risk-check.sh`, which reads that machine's own worktrees | nobody: it needs no credential |

Each workflow mints a one-hour installation token from the App, so no key is at rest on anybody's
machine. The bot identity means what is still attributed to calef is genuinely calef. Before the move,
two `launchd` jobs on patagonia had to be retired by hand, and the at-risk check now has its own
per-developer `launchd` job. Scheduled runs lose a little cadence (five minutes, delayed under load)
and the loop's red-once reporting.

When reading the drain's output, ask the queue itself, not the pull request.
`autoMergeRequest` goes null on enqueue, and `statusCheckRollup` keeps cancelled runs.
`gh api graphql` for `pullRequest(number: N) { mergeQueueEntry { state position } }` is the authority.
[`merge-queue/actions-migration.md`](merge-queue/actions-migration.md) has the premise test, the
`launchctl` commands and the plist, and what was lost.

## `helpers/trunk-health.sh`

```console
$ helpers/trunk-health.sh --once
main is green at 5a09f754

$ helpers/trunk-health.sh
MAIN IS RED at d1e6b1e9 -- failing: CI -- nobody is assigned to this
main recovered at 38dc6473
```

It reports the *transition* to red and the transition back, never every red poll: a trunk broken for
an hour is one fact, not twenty-four. It reports recovery deliberately, because a watcher that only
speaks on failure teaches its reader that silence means health, and silence is also what a dead
watcher produces.

The phrase "nobody is assigned to this" is not filler. A red trunk with an owner is a task; a red
trunk without one is the failure being surfaced.

What to do once it speaks is [notes/main-is-red.md](main-is-red.md), added 2026-09-23 because
calef asked whether the response existed and it did not: this watcher reported a red trunk and
`helpers/merge-drain.sh` carried on arming pull requests into it every five minutes. The response is
`helpers/queue-hold.sh` (hold the queue, land one fix alone, release) with the judgment in
[briefs/main-is-red.md](../briefs/main-is-red.md). It stays a person's to run, for the reason this
note gives throughout: a queue reports, it does not resolve.

This watcher reads CI's *conclusion*, and a skipped required check posts `success` (its `BUGS`).
On 2026-09-23 a documentation-only commit broke a `crates/documentation` test `ci.yml` had skipped.
A prose-only change now runs that test, among others: [prose-only.md](merge-queue/prose-only.md).

## `helpers/at-risk-check.sh`, the one watch that stays on your own machine

```console
$ helpers/at-risk-check.sh
at-risk-check: UNCOMMITTED. /path/to/nife-worktrees/atrisk (maintainer/work-one-prune-from-gone) has 2 changed file(s), newest touched 41 minutes ago. One prune away from gone; commit and push.
```

Uncommitted work in a lane worktree is the only failure here that destroys rather than delays. This
reads every lane worktree's
`git status --porcelain` and reports any whose newest changed file is older than `AT_RISK_MINUTES`
(default 30). It reports and never acts, and never uses `git stash`, whose stack is shared across
worktrees. It runs from `launchd` on each developer's machine, because it reads that machine's
worktrees and a runner has none. [`merge-queue/at-risk-check.md`](merge-queue/at-risk-check.md) has
the measured cost, the fold and unfold, and the live-tree check.

## The queue's own settings

A push to `main` cites the merge group instead of repeating it (A′, approved by calef 2026-09-24).
When a successful `merge_group` run exists at exactly the pushed SHA, the push run skips its suite and
logs the run it cites; otherwise it runs everything. The queue's check timeout is 240 minutes, not
60, because at 60 it evicted a green pull request for waiting on runners.
[`merge-queue/main-push-and-check-timeout.md`](merge-queue/main-push-and-check-timeout.md) has the
premise checked against history and the eviction timeline.

## `script/preflight-queue`: the group build, run here first

The queue's prevention has a price, and a red member is where it is paid. A group of up to five
is built as one, and a member that is red on top of the entries ahead of it fails the whole group,
which is then rebuilt without it. Every other member's build is thrown away. Per-pull-request CI
cannot see this case at all, because the failing input is the stack, not the branch. On 2026-09-24 a
lane replayed the queue by hand (23 entries, about 40 minutes) and found three pull requests, #1194,
#1222 and #1182, green alone and red on top of what was ahead of them. Each would have cost a group
roughly an hour of runner time per member.

`script/preflight-queue` makes that replay a command. It walks the queue in order, then the pull
requests armed but not yet queued. It merges each onto the green ones ahead in a scratch worktree, and
runs the cheap end of `script/ci-build` plus one aarch64 suite for anything that touches code. Its
header has the ladder, what it skips and why, and its `BUGS`.

When to run it:

- Before enqueueing a batch. When a session is about to arm several pull requests at once, and
  especially when two of them touch the test-wiring hotspot, run it with the batch armed but before
  the queue has formed groups. A dry run costs this machine minutes per code entry and seconds per
  documentation entry.
- When a group has just failed and the queue is deep. The failure evicted one member; the rest
  are rebuilt, and a second red member behind it costs another group. A dry run says whether there is
  one.
- Not beside a `script/verify` or a mutation sweep, which is AGENTS.md's memory ceiling; the
  script already skips its own falsification rung when a solver is running, but its aarch64 suite
  still competes for cores.

It defaults to `--dry-run`. `--act` comments on each red entry, with the failing command, an excerpt
and the stack it was tested on, and dequeues it (or disables auto-merge if it was not yet queued).
If `main` itself is red at the baseline, it acts on nothing and exits 3, because every entry would
inherit that failure.

## What the queue bought, measured

Measured on 2026-08-16 from the GitHub API, 40 hours before the queue against 10 hours after. Merges
per elapsed hour went from 0.97 to 1.76, and enqueue-to-merged from 17.0 to 12.3 minutes. No run on
`main` went red, against 2 of 120 before. Five pull requests enqueued together landed in 20.6
minutes. The queue amortizes wall clock, not CI cost: it builds one candidate per entry, so job-minutes
per landing stayed flat. The prover decides only the tail, and that tail was mostly changes the
`--affected-since` predicate could not attribute. The samples are small, one afternoon each.
[`merge-queue/what-the-queue-bought.md`](merge-queue/what-the-queue-bought.md) has the table, the five
caveats and the levers.

## Squash and rebase merging are disabled at the repository, not only in the queue

calef decided on 2026-08-18: `allow_squash_merge=false`, `allow_rebase_merge=false`,
`allow_merge_commit=true`. A squash-merge destroys the commit separation `git blame` depends on, and
until then only the queue's `merge_method` and memory enforced the rule. Squashing checkpoints into
purposes *within* a branch is still the rule; collapsing the purposes at merge is what is now
impossible. [`merge-queue/merge-methods-and-holds.md`](merge-queue/merge-methods-and-holds.md) has
the reasoning.

## `Blocked-by: #N`: a hold that releases itself

Put `Blocked-by: #324` in a pull request body when it must not land before #324: green alone, green
alone, red together. While #324 is open the drain keeps the pull request's `unarmed` cause off. Once
#324 merges or closes, it labels the pull request `needs-maintainer`, or a draft `unblocked`. Use it
for a mechanical constraint only; if a person must decide, the label is still the answer. A generic
hold label was refused, because a hold that outlives its reason is a false blocker, and a false
blocker is believed. [`merge-queue/merge-methods-and-holds.md`](merge-queue/merge-methods-and-holds.md)
has the cases that motivated it.

## A push to a queued branch

Re-arm after the last push: [push-while-queued.md](push-while-queued.md).

## BUGS

The full list, every entry as written, is [`merge-queue/limitations.md`](merge-queue/limitations.md).
The ones a maintainer most needs:

- The queued-push refusal misses cloud lanes: [push-while-queued.md](push-while-queued.md).
- A push after enqueue is silently discarded. The queue merges the SHA it enqueued, and the last
  commit exists only locally. Treat marking a pull request ready as the end of pushing, and confirm
  landed work by content (`git merge-base --is-ancestor <sha> origin/main`).
- A branch in the merge queue cannot be pushed to. Make the last push, then arm; CI need not be
  green yet (`briefs/gate-in-ci.md`, Step 4).
- A branch stacked on another pull request and then merged with `main` has two merge bases, and the
  queue evicts it as a conflict git does not see. Rebase onto `main` instead.
- One push can raise two `synchronize` events and strand a green pull request outside the queue. The
  drain reruns the cancelled duplicate once.
- A′ lets `main`'s caches go stale and re-runs the whole workflow when a non-required job fails; the
  240-minute timeout lets a hung group block for four hours.
- The drain's log says what the machinery did, never what a person did, and its event counts begin on
  2026-09-23.
- The drain's surface pass judges a brand-new pull request against its own checkout's `main`, which
  can be one merge behind: an empty claim draft picked up the merge before it and took a
  `needs-architect` it did not earn (#2202, 2026-10-15). Release is one `record-ruling`; the fix
  wants the pull request's real base ([limitations.md](merge-queue/limitations.md)).
- `Blocked-by:` holds the drain and not the queue, reads only the first line, and matches inside code
  spans.
- The 2026-08-16 measurement is a snapshot nothing re-derives.
