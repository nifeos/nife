#!/bin/sh
#
# helpers/arm.sh: the one way to arm a pull request. Provisional name (lane/push-requeue,
# 2026-11-04 UTC); COE action 7 (notes/coes/2026-11-03-merge-queue-backlog.md).
#
#     helpers/arm.sh <pull request number>
#     helpers/arm.sh <pull request number> --despite-conflict "<why>"
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
# A pull request whose last removal from the queue was `failed_checks`, at the head it still has, is
# refused unless the failure is a known flake: a failed merge-group job's log
# (`gh run view <run> --log-failed`) matches a signature in helpers/known-flakes.tsv. A push lifts the
# refusal, since the ejected head is then no longer the head. Other removals (`manual`, `merged`, a
# conflict) are not refused here. ARM_KNOWN_FLAKES replaces the list, for the selftest only.
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
# On 2026-11-10 #3436 was ejected at 22:49 UTC because swish-check failed with "the progenitor is
# out of memory". It was re-armed at 00:18 at the same head with nothing fixed, and failed the same
# way. A real failure fails again at the same
# head, so re-arming one only spends a group build; the refusal leaves it for its lane.
#
# ARM_VERIFY_TRIES (default 5) and ARM_VERIFY_SLEEP (default 2 seconds) bound the read-back, because
# GitHub can take a moment to show an entry.

set -u
here="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
. "$here/hold-labels.sh"
. "$here/pr-queue-state.sh"

me="arm"
usage='usage: helpers/arm.sh <pull request number> [--despite-conflict "<why>"]'
case "${1:-}" in
'' | *[!0-9]* | -*)
	echo "$usage" >&2
	exit 2
	;;
esac
pr="$1"
shift
despite=
case "$#:${1:-}" in
0:) ;;
2:--despite-conflict)
	despite="$2"
	if [ -z "$(printf '%s' "$despite" | tr -d '[:space:]')" ]; then
		echo "$me: NOT ARMED. --despite-conflict needs the reason, which goes on the pull request." >&2
		exit 2
	fi
	;;
*)
	echo "$usage" >&2
	exit 2
	;;
esac

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

# The ejection check. The last removal, its group commit and the group's second parent, which is the
# pull request's head when the group was built (helpers/needs-maintainer.jq's nm_ejected reads the
# same fields). A removal with no group commit cannot say which head it was, so it is not refused.
ejection_jq='.data.repository.pullRequest as $p | ($p.removed.nodes[-1] // null) as $r
	| if $r != null and $r.reason == "failed_checks"
	     and ($r.beforeCommit.parents.nodes[1].oid // null) == $p.headRefOid
	  then "\($r.createdAt) \($r.beforeCommit.oid) \($p.headRefOid)" else "" end'
if ! ejection="$(gh api graphql -F owner='{owner}' -F name='{repo}' -F n="$pr" \
	-f query='query($owner:String!,$name:String!,$n:Int!){repository(owner:$owner,name:$name){pullRequest(number:$n){headRefOid removed:timelineItems(last:1,itemTypes:[REMOVED_FROM_MERGE_QUEUE_EVENT]){nodes{... on RemovedFromMergeQueueEvent{createdAt reason beforeCommit{oid parents(first:2){nodes{oid}}}}}}}}}' \
	2>/dev/null)" || ! ejection="$(printf '%s' "$ejection" | jq -r "$ejection_jq" 2>/dev/null)"; then
	echo "$me: NOT ARMED. could not read #$pr's last removal from the merge queue." >&2
	exit 1
fi
if [ -n "$ejection" ]; then
	read -r ejected_at group head <<ENDEJECTION
$ejection
ENDEJECTION
	flakes="${ARM_KNOWN_FLAKES:-$here/known-flakes.tsv}"
	matched=
	if grep -q '^[^#]' "$flakes" 2>/dev/null; then
		logs="$(gh api "repos/{owner}/{repo}/actions/runs?head_sha=$group&event=merge_group&per_page=100" 2>/dev/null |
			jq -r '.workflow_runs[] | select(.conclusion == "failure" or .conclusion == "timed_out") | .id' 2>/dev/null |
			while read -r run; do gh run view "$run" --log-failed 2>/dev/null; done)"
		matched="$(grep '^[^#]' "$flakes" | while IFS='	' read -r name signature record; do
			[ -n "$signature" ] || continue
			if printf '%s\n' "$logs" | grep -qE -- "$signature"; then
				printf '%s (%s)\n' "$name" "$record"
				break
			fi
		done)"
	fi
	if [ -z "$matched" ]; then
		echo "$me: NOT ARMED. #$pr was ejected from the merge queue at $ejected_at for failed_checks, at head $head, which is still its head, and no failed merge-group job of group $group matches a known flake in helpers/known-flakes.tsv. The same head fails the same way: hand it to its lane, whose pushed fix lifts this. If the failure is a flake, record it in a note and add its signature to the list first." >&2
		exit 1
	fi
	echo "$me: #$pr was ejected for failed_checks at this head, and the failure matches the known flake $matched; re-arming."
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
