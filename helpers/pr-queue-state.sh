# shellcheck shell=sh
# helpers/pr-queue-state.sh: read one pull request's arming state. Sourced by helpers/arm.sh and
# helpers/push-requeue.sh. Provisional name (lane/push-requeue, 2026-11-04 UTC).
#
# `gh pr view --json` has no queue field (checked on the gh this repository runs: no
# `isInMergeQueue`), so this asks GraphQL for `mergeQueueEntry`, the way helpers/push-queued-check.sh
# does. On a CLEAN pull request `gh pr merge --auto` enqueues directly and `autoMergeRequest` reads
# null, so armed means a queue entry OR a non-null `autoMergeRequest`.
#
#     pr_state_by_number <n>        pr_state_by_branch <branch>
#
# Each prints one JSON object on stdout and returns 0:
#     { number, id, state, isDraft, headRefOid, labels: [name...], isInMergeQueue, autoMergeRequest }
# A branch with no open pull request prints `null`. A failed or malformed answer returns 1.

PR_STATE_FIELDS='number id state isDraft headRefOid labels(first:30){nodes{name}} autoMergeRequest{enabledAt} mergeQueueEntry{state}'
PR_STATE_JQ='if .data.repository == null then error("GitHub gave no repository") else . end
	| (.data.repository.pullRequest // .data.repository.pullRequests.nodes[0])
	| if . == null then null else
	  {number, id, state, isDraft, headRefOid, labels: [.labels.nodes[].name],
	   isInMergeQueue: (.mergeQueueEntry != null), autoMergeRequest} end'

pr_state_by_number() {
	psn_out="$(gh api graphql -F owner='{owner}' -F name='{repo}' -F n="$1" \
		-f query="query(\$owner:String!,\$name:String!,\$n:Int!){repository(owner:\$owner,name:\$name){pullRequest(number:\$n){$PR_STATE_FIELDS}}}" \
		2>/dev/null)" || return 1
	printf '%s' "$psn_out" | jq -c "$PR_STATE_JQ"
}

pr_state_by_branch() {
	psb_out="$(gh api graphql -F owner='{owner}' -F name='{repo}' -F b="$1" \
		-f query="query(\$owner:String!,\$name:String!,\$b:String!){repository(owner:\$owner,name:\$name){pullRequests(first:1,states:OPEN,headRefName:\$b){nodes{$PR_STATE_FIELDS}}}}" \
		2>/dev/null)" || return 1
	printf '%s' "$psb_out" | jq -c "$PR_STATE_JQ"
}
