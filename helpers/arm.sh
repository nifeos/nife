#!/bin/sh
#
# helpers/arm.sh: the one way to arm a pull request. Provisional name (lane/push-requeue,
# 2026-11-04 UTC); COE action 7 (notes/coes/2026-11-03-merge-queue-backlog.md).
#
#     helpers/arm.sh <pull request number>
#
# Refuses a draft, a pull request that is not open, and one wearing a hold label
# (helpers/hold-labels.sh: `needs-architect`, `held-by-lane`, `held-for-red-trunk`), naming the label.
# Otherwise it runs `gh pr merge <n> --auto --merge` and then reads GitHub back: armed means
# a merge queue entry exists or `autoMergeRequest` is not null. Neither is exit 1 and a line saying so.
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
case "${1:-}" in
'' | *[!0-9]* | -*)
	echo "usage: helpers/arm.sh <pull request number>" >&2
	exit 2
	;;
esac
pr="$1"

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
