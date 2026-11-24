# helpers/stuck-head.jq: have a merge group's workflow runs all finished? lane/drain-stuck-queue-head,
# 2026-10-07 UTC; the name is provisional.
#
# Input: the body of `GET /repos/{repo}/actions/runs?head_sha=<group head>&event=merge_group`.
# Output: nothing while any run is unfinished or there are no runs at all (a group whose workflows
# have not been created yet is not stuck), else
#   { completedAt: <latest updated_at>, runs: [{ id, name, conclusion, attempt, url }...] }
# The age test and the cause are helpers/needs-maintainer.jq's (`nm_stuck_head`); this file must not
# call `gh`, so the selftest can feed it the recorded response for #1824's group.
#
# The latest `updated_at` of a completed run is when it completed, to the second, which is the
# clock the threshold runs from. The group's start is the wrong clock: a healthy group takes 5 to
# 29 minutes to finish, but GitHub merges or ejects it within 0.2 to 0.6 minutes of the last run
# completing (four merges measured 2026-10-07, see nifeos/record:notes/queue-ejection.md).

.workflow_runs
| select(length > 0 and all(.status == "completed"))
| { completedAt: (map(.updated_at) | max),
    runs: map({ id, name, conclusion, attempt: .run_attempt, url: .html_url }) }
