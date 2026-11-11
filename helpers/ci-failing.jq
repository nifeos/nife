# helpers/ci-failing.jq: which REQUIRED checks are failing on a pull request's head commit.
#
# Provisional name, minted with helpers/ci-failing.sh (see that file and notes/merge-queue.md).
#
# Input: an array of check-run objects for one commit, as `gh api repos/R/commits/SHA/check-runs
# --paginate --slurp` returns them once the pages are flattened (each has `id`, `name`,
# `conclusion`, `status`, `html_url`). Argument `$required`: the array of required check names.
#
# Output: one object per failing required check, `{name, url}`, sorted by name.
#
# **A check name is failing only if its most recent run failed.** Runs of one name accumulate on a
# commit (a rerun, a cancelled duplicate, a rerun after a flake), and a failure superseded by a
# later success is not a failure; #1592 had stale failures sitting beside newer successes. "Most
# recent" is the highest check-run id, because ids only grow and `started_at` is null on a run that
# has not started. A newer run that is queued, in progress or cancelled therefore hides an older
# failure, deliberately: a rerun is underway, or a newer push owns the result, and either way the
# old verdict is no longer the head's. Only `failure` and `timed_out` count, the same pair
# notes/merge-queue.md says to trust over `statusCheckRollup`.
#
# **The `architect hold` check is ignored by name.** It goes red on purpose while `needs-architect`
# is on a pull request, which is a person's queue and not a failing build; flagging it would label
# every pull request waiting on an architect.
#
# A required check with no run at all is not failing; it is missing, which is a different fact and
# is `ready status` and the queue's business.

def failing($required):
  group_by(.name)
  | map(sort_by(.id) | last)
  | map(select(.name as $n | $required | index($n)))
  | map(select(.name | startswith("architect hold") | not))
  | map(select(.conclusion == "failure" or .conclusion == "timed_out"))
  | map({name: .name, url: .html_url})
  | sort_by(.name);

# **A run that finished before the pull request was last marked ready decides nothing.** A draft's
# `lint` fails on purpose ("draft: lint not run", the architect's ruling of 2026-11-11 (UTC) on
# question B of the proposal "lint fails in the queue after passing on the pull request"), and in the
# seconds after `ready_for_review` and before the ready run's own `lint` exists, that red is the
# newest. `$ready_at` is the last ReadyForReviewEvent's time, "" for a pull request never a draft.
# A run with no `completed_at` has not finished and is kept, so it still hides an older failure.
def failing($required; $ready_at):
  map(select($ready_at == "" or .completed_at == null or .completed_at >= $ready_at))
  | failing($required);
