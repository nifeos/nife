#!/bin/sh
#
# helpers/push-queued-check.sh: refuse a push to a branch whose pull request is in the merge queue.
# Provisional name; milestone: refuse a push to a queued branch (lane/push-while-queued, 2026-10-05 UTC).
#
# Why. A push to a queued branch removes the pull request from the queue (the timeline says
# `RemovedFromMergeQueueEvent`, reason `manual`) and takes its auto-merge with it. The group build
# is thrown away and every group behind it rebuilds. Measured 2026-10-05: 20 of 40 non-merge
# removals in about 28 hours were "manual", and two pull requests lost three entries each in two
# minutes to repeated pushes. See notes/merge-queue.md, "A push to a queued branch".
#
# Usage: the pre-push hook feeds git's ref lines on stdin
# (`<local ref> <local sha> <remote ref> <remote sha>`). Exit 0 lets the push go, exit 1 refuses it.
#
# Override: NIFE_PUSH_QUEUED=1 (provisional name) when the push is meant. `git push --no-verify`
# also works, but skips the other gates too.
#
# Fails open, by design and loudly: no `gh`, no auth, no network, or a blocked GraphQL endpoint (a
# cloud session) prints one warning line and lets the push go. It is one API call per pushed
# branch. Works under sh, bash and zsh (POSIX sh, run by path).

if [ -n "${NIFE_PUSH_QUEUED:-}" ]; then
	exit 0
fi
if ! command -v gh >/dev/null 2>&1; then
	echo "push-queued-check: gh is not installed, so I cannot tell whether this branch is queued; pushing anyway." >&2
	exit 0
fi

query='query($owner:String!,$name:String!,$b:String!){repository(owner:$owner,name:$name){pullRequests(first:1,states:OPEN,headRefName:$b){nodes{number id mergeQueueEntry{state}}}}}'
refused=0
while read -r _ _ remote_ref _; do
	case "$remote_ref" in refs/heads/*) ;; *) continue ;; esac
	branch="${remote_ref#refs/heads/}"
	if ! out="$(gh api graphql -F owner='{owner}' -F name='{repo}' -F b="$branch" -f query="$query" \
		-q '.data.repository.pullRequests.nodes[0] | if . == null then "none" elif .mergeQueueEntry == null then "idle" else "queued \(.number) \(.id)" end' 2>/dev/null)"; then
		echo "push-queued-check: gh could not ask GitHub (offline, unauthenticated or GraphQL blocked); pushing $branch anyway." >&2
		exit 0
	fi
	case "$out" in
	"queued "*)
		set -- $out
		cat >&2 <<MSG
push-queued-check: pull request #$2 for $branch is in the merge queue, and a push removes it.
  The group build is thrown away, auto-merge is cancelled, and every group behind it rebuilds.
  Push with script/push --requeue instead: it dequeues #$2, pushes, reads the branch back,
  and arms it again through helpers/arm.sh, which refuses a hold or a queue conflict.
  If a push that drops the entry is intended: NIFE_PUSH_QUEUED=1 git push ...
MSG
		refused=1
		;;
	none | idle) ;;
	*)
		echo "push-queued-check: unexpected answer '$out'; pushing $branch anyway." >&2
		;;
	esac
done
exit "$refused"
