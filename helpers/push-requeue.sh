#!/bin/sh
#
# helpers/push-requeue.sh: the two halves of `script/push --requeue`, kept out of script/push so the
# push itself stays small. Provisional name (lane/push-requeue, 2026-11-04 UTC); COE action 7
# (notes/coes/2026-11-03-merge-queue-backlog.md).
#
#     helpers/push-requeue.sh before <branch>   # dequeue if queued; print "<number> <armed 0|1>"
#     helpers/push-requeue.sh after <number>    # arm it again, through helpers/arm.sh
#
# `before` prints "none 0" for a branch with no open pull request. `armed` is 1 when the pull request
# was in the merge queue or had auto-merge on, and only then does script/push call `after`: a pull
# request that was deliberately unarmed stays unarmed. A queued pull request is dequeued with
# `dequeuePullRequest` and read back; still queued is exit 1, because the pre-push hook would refuse
# the push and a push that slipped through would drop the entry silently.
#
# The arming rules, including the hold labels, are helpers/arm.sh's; this file holds none.

set -u
here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
. "$here/pr-queue-state.sh"

case "${1:-}" in
before)
	branch="${2:?usage: push-requeue.sh before <branch>}"
	if ! view="$(pr_state_by_branch "$branch")"; then
		echo "push-requeue: could not read the pull request for $branch from GitHub; nothing was pushed." >&2
		exit 1
	fi
	if [ -z "$view" ] || [ "$view" = null ]; then
		echo "none 0"
		exit 0
	fi
	number="$(printf '%s' "$view" | jq -r '.number')"
	queued="$(printf '%s' "$view" | jq -r '.isInMergeQueue')"
	auto="$(printf '%s' "$view" | jq -r '.autoMergeRequest != null')"
	if [ "$queued" = true ]; then
		id="$(printf '%s' "$view" | jq -r '.id')"
		if ! gh api graphql -f query="mutation{dequeuePullRequest(input:{id:\"$id\"}){clientMutationId}}" >/dev/null 2>&1; then
			echo "push-requeue: could not dequeue #$number; nothing was pushed." >&2
			exit 1
		fi
		if [ "$(pr_state_by_number "$number" | jq -r '.isInMergeQueue')" != false ]; then
			echo "push-requeue: #$number is still in the merge queue after the dequeue; nothing was pushed." >&2
			exit 1
		fi
		echo "push-requeue: dequeued #$number." >&2
	fi
	if [ "$queued" = true ] || [ "$auto" = true ]; then
		echo "$number 1"
	else
		echo "$number 0"
	fi
	;;
after)
	number="${2:?usage: push-requeue.sh after <number>}"
	exec "$here/arm.sh" "$number"
	;;
*)
	echo "usage: push-requeue.sh before <branch> | after <number>" >&2
	exit 2
	;;
esac
