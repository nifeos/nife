# A push to `main` cites the merge group, and the queue's 240-minute timeout

*Name: provisional.*

An appendix to [notes/merge-queue.md](../merge-queue.md). It holds two changes of 2026-09-24 that
were both answers to starved runners: A′, which stops a push to `main` repeating the merge group's
suite, and the check timeout raised from 60 minutes to 240.

**Status of the first half, 2026-10-16 (UTC): the `push` to `main` can no longer fire.** `main` now
sits behind a repository rule, and a push is declined outright (`push declined due to repository
rule violations`); everything lands through the merge queue, including the maintainer's own filings.
The A′ branch remains in `ci.yml` and `verify.yml` and still works as designed should the rule ever
change, but no current flow reaches it. `helpers/trunk-health.sh` is unaffected where it matters:
its health lookup names merge-group runs by exact SHA, which is still how every commit arrives; only
its A′ branch (reading a skipped push run as green) has nothing left to read. The second half below,
the ruleset's `check_response_timeout_minutes`, is the
ruleset's own setting, still operative, and is why the queue's green entries stopped being evicted
for runner supply.

## A push to `main` cites the merge group instead of repeating it (A′)

calef approved this on 2026-09-24 ("Proceed with A′"). On a `push` to `main`, the `draft gate` job in
`ci.yml` and `verify.yml` asks the API whether a `merge_group` run of the same workflow concluded
`success` at exactly `github.sha`. If one did, it sets `run=false`, prints that run's ID and URL, and
every gated job skips. If there is no such run, the run did not succeed, or the API errors, it runs
everything. A commit that reaches `main` outside the queue has no merge-group run, and still gets the
full suite.

Why: the runners were starved. 31 runs were queued at 16:40 UTC that day. Every landed batch paid for
its suite twice: once on the `gh-readonly-queue/` ref, then again, identically, on the push to
`main`. A full verify is the project's long pole on its own.

The premise was checked against history first. For each of the twelve first-parent commits on `main`
from `aee5b141` to `47c3a3c9`, a successful `merge_group` CI run has `head_sha` equal to that commit.
The queue builds each entry's merge commit on its own ref, and the commit that lands is that very
object. Inside a batch only the tip gets a `push` run. `ac04fb01` (#1179) and `b9b0d4bb` (#1202) got
none, while the batch tips `0b72f673` and `47c3a3c9` did. The tip's merge-group run tested the tip
with every earlier entry of its batch already beneath it. So an exact-SHA lookup answers "was this
tree tested" without inferring anything about neighbors.

```console
$ gh api "repos/nifeos/nife/actions/workflows/ci.yml/runs?event=merge_group&head_sha=47c3a3c9d01e96c8d007a4b41ff9d2adc8858f10&status=success&per_page=1" \
    --jq '.workflow_runs[0].id'
36025842548
```

A push that finds one logs `==> <sha> was tested by merge group run <id>; skipping the suite` in its
`draft gate` step, with the run's URL on the next line.

`helpers/trunk-health.sh` reads a skipped push run as green, since the run concludes `success`, and
names the merge-group runs behind it.

## The queue's check timeout is 240 minutes, not 60

The maintainer raised the ruleset's `check_response_timeout_minutes` from 60 to 240, after #1213 was
evicted with reason `checks_timed_out`. Measured from the timeline and the run data:

| event | UTC |
|---|---|
| #1213 added to the queue | 16:53:43 |
| merge-group CI 36030550892 and verify 36030550968 created | 16:54:01 |
| first verify job past the gate starts (a runner freed) | 17:13:05 |
| last prove shard finishes | 17:48:23 |
| evicted, `checks_timed_out` | 17:54:14 |
| required `verify (Kani proofs)` aggregate starts, then reports `success` | 17:58:53 |

The build did run. What timed out was the wait. There were about nineteen minutes queued before any
real job. Then a five-second aggregate job sat ten minutes for a runner after the last shard, and
reported four minutes past the deadline. CI had finished green at 17:45:20. At 60 minutes the timeout
measured runner supply rather than the change. So it evicted a green pull request, and made it queue
again.
