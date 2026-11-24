Report the state of the merge queue. You are in the repository's git worktree. **Change no files.**
This is read-only and its whole output is your final message.

    git fetch origin
    gh pr list --json number,isDraft,mergeStateStatus,title,statusCheckRollup,labels
    gh api --paginate 'repos/partsfoundry/nife/issues?labels=needs-maintainer&state=all' \
        --jq '.[] | [.number, (if .pull_request then "pr" else "issue" end), .state, .title] | @tsv'

## What to work out for each open pull request

- Does it carry `needs-maintainer`. The merge drain put it there for one of seven causes and said
  which in a comment: ejected, conflicting, a stale queue entry, or 30 minutes ready and unarmed,
  ready on a base other than `main`, or wearing `ci-failing`. The seventh is a draft whose head
  has had no commit for 6 hours, which usually means its lane has ended.
  List these first, with the cause, whatever else the survey finds. The listing above holds
  issues as well: an issue there is a CI job near its budget (the `budget` cause, milestone 808
  (every gate accounts for its time)). Report it with its job and share of budget.

- Is it ready or draft, and is auto-merge already enabled.
- Is it `DIRTY` or `CONFLICTING`, which means it needs a rebase before anything else can happen.
- Does it have a failing check, and which one by name.
- Is it `BLOCKED` only by the `needs-architect` label, which is a deliberate hold rather than a
  problem, and must be reported as such rather than as a failure.

- Is the session's own ledger clean: every dispatched lane has either a DONE marker or a live
  process, and no PENDING file sits unexecuted. A claim commit with no lane behind it is a stalled
  claim, and this survey names it rather than passing it.

## The traps

- `BLOCKED` with no failing check usually means checks are still running. Count the checks with
  no conclusion yet and say so, rather than reporting it as stuck.
- `auto=false` does not mean unqueued. GitHub clears the auto-merge request once a pull request
  enters the merge queue, so a clean pull request showing `auto=false` may already be queued. Check
  `gh run list --event merge_group` before claiming anything is idle.
- A pull request absent from the default listing may have merged, not vanished. Confirm with
  `--state all` before reporting it as gone.
- A lane log that ends in a connection error or a permission rejection is a dead dispatch, not
  a running lane. The dispatch looked successful and produced nothing. Redispatch it and count
  the death, or hand it to whoever owns the dispatch
  ([notes/coes/2026-09-30-lane-follow-through.md](https://github.com/partsfoundry/record/blob/main/notes/coes/2026-09-30-lane-follow-through.md)).

## What to report

A table: number, ready or draft, state, any `needs-maintainer` cause, and either the failing
check's name or the reason it is held. Then one short list of what needs a human: rebases needed, failures to investigate, and
anything held on `needs-architect`.

Do not rebase anything. Do not enqueue anything. Do not edit or commit.
