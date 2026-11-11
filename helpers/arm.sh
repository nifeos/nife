#!/bin/sh
#
# helpers/arm.sh: the one way to arm a pull request. Provisional name (lane/push-requeue,
# 2026-11-04 UTC); COE action 7 (notes/coes/2026-11-03-merge-queue-backlog.md).
#
#     helpers/arm.sh <pull request number>
#     helpers/arm.sh <pull request number> --despite-conflict "<why>"
#     helpers/arm.sh <pull request number> --despite-failed-checks "<why>"
#
# Refuses a draft, a pull request that is not open, and one wearing a hold label
# (helpers/hold-labels.sh: `needs-architect`, `held-by-lane`, `held-for-red-trunk`), naming the label.
# Then it runs `script/queue-conflicts --pr <n>` (COE action 6, milestone 1054 (an entry that
# conflicts with the queue is caught before it)) and refuses a pull request that conflicts with
# `main` or an entry ahead of it, printing the check's report: each conflicting file and the entries
# ahead that changed it. Otherwise it runs `gh pr merge <n> --auto --merge` and then reads GitHub
# back: armed means a merge queue entry exists or `autoMergeRequest` is not null. Neither is exit 1
# and a line saying so.
#
# It also refuses a head whose newest `lint` check run is skipped, failed or cancelled, or is still
# running beside a skipped one, since GitHub counts a draft run's skip as passed. And it refuses a
# head that the merge queue last removed for failed checks, which `--despite-failed-checks "<why>"`
# overrides after posting the reason. Both rules and their cases are at the checks, below.
#
# `--despite-conflict "<why>"` is the override: it arms over a reported conflict, after posting the
# reason as a comment on the pull request, since notes/merge-queue/conflict-precheck.md asks a lane
# that arms over a conflict to say why there. It refuses an empty reason, and a comment that does not
# post leaves the pull request unarmed. When the check could not tell (its exit 2: no network, no
# queue) this arms and says the check did not run, which is how arming behaved before the check
# existed. Provisional flag name.
#
# ARM_QUEUE_CHECK replaces the check's command, for helpers/push-requeue-selftest.sh only.
#
# # Why
#
# On 2026-11-03 the session account enqueued #3186 three times while it carried `needs-architect`,
# and the drain dequeued it each time, discarding three group builds. At 06:00 UTC on 11-04 two clean
# pull requests sat with auto-merge off after a push. A bare `gh pr merge --auto` checks no label and
# prints success whether or not anything was armed. This checks the label first and the result after.
#
# On a CLEAN pull request `gh pr merge --auto` enqueues directly, and `autoMergeRequest` reads null
# while queued; that is why the queue entry is read too (helpers/pr-queue-state.sh; `gh pr view
# --json` has no queue field). Arming a pull request by hand with
# `gh pr merge --auto` stays possible; it is only no longer what the briefs name.
#
# ARM_VERIFY_TRIES (default 5) and ARM_VERIFY_SLEEP (default 2 seconds) bound the read-back, because
# GitHub can take a moment to show an entry.

set -u
here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
. "$here/hold-labels.sh"
. "$here/pr-queue-state.sh"

me="arm"
usage='usage: helpers/arm.sh <pull request number> [--despite-conflict "<why>"] [--despite-failed-checks "<why>"]'
case "${1:-}" in
'' | *[!0-9]* | -*)
	echo "$usage" >&2
	exit 2
	;;
esac
pr="$1"
shift
despite=
despite_failed=
while [ "$#" -gt 0 ]; do
	case "$1" in
	--despite-conflict | --despite-failed-checks)
		if [ "$#" -lt 2 ] || [ -z "$(printf '%s' "$2" | tr -d '[:space:]')" ]; then
			echo "$me: NOT ARMED. $1 needs the reason, which goes on the pull request." >&2
			exit 2
		fi
		if [ "$1" = --despite-conflict ]; then despite="$2"; else despite_failed="$2"; fi
		shift 2
		;;
	*)
		echo "$usage" >&2
		exit 2
		;;
	esac
done

if ! view="$(pr_state_by_number "$pr")" || [ -z "$view" ] || [ "$view" = null ]; then
	echo "$me: NOT ARMED. could not read #$pr from GitHub." >&2
	exit 1
fi

state="$(printf '%s' "$view" | jq -r '.state')"
if [ "$state" != OPEN ]; then
	echo "$me: NOT ARMED. #$pr is $state, not OPEN." >&2
	exit 1
fi
if [ "$(printf '%s' "$view" | jq -r '.isDraft')" = true ]; then
	echo "$me: NOT ARMED. #$pr is a draft; mark it ready first (gh pr ready $pr)." >&2
	exit 1
fi
if hold="$(printf '%s' "$view" | jq -r '.labels[]' | hold_label_of)"; then
	echo "$me: NOT ARMED. #$pr carries \`$hold\`, which holds it out of the queue; the drain would dequeue it. Clear the hold first (script/record-ruling for an architect hold)." >&2
	exit 1
fi

armed() { # reads helpers/pr-queue-state.sh's JSON on stdin; exit 0 when queued or auto-merge is set
	jq -e '(.isInMergeQueue == true) or (.autoMergeRequest != null)' >/dev/null
}

if printf '%s' "$view" | armed; then
	echo "$me: #$pr already armed."
	exit 0
fi

head="$(printf '%s' "$view" | jq -r '.headRefOid // empty')"
if [ -z "$head" ]; then
	echo "$me: NOT ARMED. could not read #$pr's head commit from GitHub." >&2
	exit 1
fi

# **A skipped lint is not a passed lint.** A draft's CI run skips `lint`, and GitHub counts a
# skipped required check as passed, so a pull request armed the moment it is marked ready enters the
# queue on the draft's skip, before the ready run's `lint` has started (#3320 and #3427, 2026-11).
# The newest `lint` check run on the head decides: success arms; skipped, failed or cancelled
# refuses; unfinished refuses only when a skipped one also sits on this head, because then GitHub
# may already count the check as passed. A head with no `lint` yet (just pushed, not a draft) arms:
# GitHub waits for a required check that has not posted, and that is what lets `script/push
# --requeue` re-arm right after its push.
if ! runs="$(gh api "repos/{owner}/{repo}/commits/$head/check-runs?check_name=lint&filter=all&per_page=100" \
	--jq '[.check_runs[] | {id, status, conclusion}]' 2>/dev/null)"; then
	echo "$me: NOT ARMED. could not read the lint check on #$pr's head $head." >&2
	exit 1
fi
lint_verdict="$(printf '%s' "$runs" | jq -r '
	(any(.[]; .conclusion == "skipped")) as $skip
	| (sort_by(.id) | last) as $new
	| if $new == null then "none"
	  elif $new.status != "completed" then (if $skip then "unfinished-after-skip" else "unfinished" end)
	  else $new.conclusion end')"
case "$lint_verdict" in
success | none | unfinished) ;;
unfinished-after-skip)
	echo "$me: NOT ARMED. #$pr's lint is still running on $head, and a skipped lint from its draft run sits on the same head, which GitHub would count as passed. Arm it once lint has passed." >&2
	exit 1
	;;
*)
	echo "$me: NOT ARMED. the newest lint check on #$pr's head $head is $lint_verdict. Arm a head whose lint passed." >&2
	exit 1
	;;
esac

# **A failure does not clear by waiting.** The queue removes an entry for `failed_checks`, and an
# unchanged head usually fails the same way again: the entry ahead that it broke against has merged,
# and its pull-request checks, which never saw that entry, still read green. On 2026-11-07 and 08
# #3314 and #3336 went back three times each at one head and failed each time. The last removal
# decides; its merge-group commit's last parent is the head it built. `--despite-failed-checks
# "<why>"` arms anyway (a flake, say), after posting the reason on the pull request.
if ! removal="$(gh api graphql -F owner='{owner}' -F name='{repo}' -F n="$pr" \
	-f query="query(\$owner:String!,\$name:String!,\$n:Int!){repository(owner:\$owner,name:\$name){pullRequest(number:\$n){timelineItems(last:1,itemTypes:[REMOVED_FROM_MERGE_QUEUE_EVENT]){nodes{... on RemovedFromMergeQueueEvent{reason beforeCommit{parents(last:1){nodes{oid}}}}}}}}}" \
	--jq '.data.repository.pullRequest.timelineItems.nodes[0] // {} | "\(.reason // "") \(.beforeCommit.parents.nodes[0].oid // "")"' 2>/dev/null)"; then
	echo "$me: NOT ARMED. could not read #$pr's last merge queue removal." >&2
	exit 1
fi
reason="$(printf '%s' "${removal% *}" | tr '[:upper:]' '[:lower:]')"
if [ "$reason" = failed_checks ] && [ "${removal##* }" = "$head" ]; then
	if [ -z "$despite_failed" ]; then
		echo "$me: NOT ARMED. the merge queue last removed #$pr for failed checks at this same head, $head, so it would fail again. Read the merge-group log, push a fix and arm again; or, for a flake, arm with --despite-failed-checks \"<why>\"." >&2
		exit 1
	fi
	if ! gh pr comment "$pr" --body "Armed again at \`$head\`, which the merge queue last removed for failed checks, with \`helpers/arm.sh --despite-failed-checks\`. Why: $despite_failed" >/dev/null 2>&1; then
		echo "$me: NOT ARMED. could not post the reason for arming over the failed checks on #$pr." >&2
		exit 1
	fi
	echo "$me: arming #$pr over its last failed checks; the reason is on the pull request."
fi

# The conflict check. Exit 0 clean, 1 conflict, 2 could not tell (script/queue-conflicts).
check="${ARM_QUEUE_CHECK:-$here/../script/queue-conflicts}"
report="$("$check" --pr "$pr" 2>&1)"
verdict=$?
case "$verdict" in
0) ;;
1)
	printf '%s\n' "$report" >&2
	if [ -z "$despite" ]; then
		echo "$me: NOT ARMED. #$pr conflicts with the merge queue (above), and the queue would remove it. Merge main in, resolve, push and arm again; or arm over it with --despite-conflict \"<why>\"." >&2
		exit 1
	fi
	if ! gh pr comment "$pr" --body "Armed over a conflict that \`script/queue-conflicts\` reported, with \`helpers/arm.sh --despite-conflict\`. Why: $despite

\`\`\`
$report
\`\`\`" >/dev/null 2>&1; then
		echo "$me: NOT ARMED. could not post the reason for arming over the conflict on #$pr." >&2
		exit 1
	fi
	echo "$me: arming #$pr over the conflict; the reason is on the pull request."
	;;
*)
	printf '%s\n' "$report" >&2
	echo "$me: the queue conflict check could not run (exit $verdict), so #$pr is armed unchecked." >&2
	;;
esac

if ! out="$(gh pr merge "$pr" --auto --merge 2>&1)"; then
	echo "$me: NOT ARMED. gh pr merge $pr --auto --merge failed: $out" >&2
	exit 1
fi

tries="${ARM_VERIFY_TRIES:-5}"
while :; do
	if after="$(pr_state_by_number "$pr")" && printf '%s' "$after" | armed; then
		echo "$me: #$pr armed ($(printf '%s' "$after" | jq -r 'if .isInMergeQueue then "in the merge queue" else "auto-merge enabled" end'))."
		exit 0
	fi
	tries=$((tries - 1))
	[ "$tries" -gt 0 ] || break
	sleep "${ARM_VERIFY_SLEEP:-2}"
done
echo "$me: NOT ARMED. gh pr merge $pr --auto said nothing was wrong, but GitHub shows #$pr neither in the merge queue nor with auto-merge on. Arm it by hand and read the page." >&2
exit 1
