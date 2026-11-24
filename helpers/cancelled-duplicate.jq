# helpers/cancelled-duplicate.jq: the CI run a concurrency group cancelled as a duplicate of its
# own same-second sibling, and which GitHub then reads as the pull request's newest, empty suite.
#
# The cause is in nifeos/record:notes/merge-queue.md's BUGS (#1203, 2026-09-24): one push raised two
# `synchronize` events, the group cancelled the newer copy before any job existed, and the merge
# queue read the newest run per workflow and answered "11 of 13 required status checks are
# expected". No workflow-level fix is sound (the note says why), so the detection is the drain's,
# and this file is that detection, lifted from the note's query so the two cannot drift apart.
#
# Input: the `actions/runs?head_sha=<sha>&event=pull_request` response. Output: one object per
# workflow whose newest run is `cancelled` **and** has a sibling created in the same second that
# succeeded or is still running. The same-second condition is what separates this from an
# ordinary supersede: a draft marked ready has a cancelled run followed twenty to sixty seconds
# later by a successful one, and that is not stranded.
#
# **Once, and the run itself is the record.** A rerun bumps the run's `run_attempt`, so a cancelled
# duplicate whose attempt is already above one has been rerun before, by the drain or by a person,
# and is not emitted again, so nothing reruns it forever. No file, label or comment holds the state: `gh run rerun` is what writes it.
#
# Consumers splice this file in front of their program (jq cannot compose `-f` with inline text);
# helpers/cancelled-duplicate-selftest.sh checks it against fixtures.
def cancelled_duplicates:
  .workflow_runs
  | group_by(.name)[]
  | sort_by(.id) as $r
  | ($r | last) as $n
  | select($n.conclusion == "cancelled")
  | select([$r[] | select(.id != $n.id and .created_at == $n.created_at
                          and (.conclusion == "success" or .status != "completed"))] | length > 0)
  | {id: $n.id, name: $n.name, run_attempt: ($n.run_attempt // 1)};

# The ones still owed a rerun.
def rerunnable: cancelled_duplicates | select(.run_attempt == 1);
