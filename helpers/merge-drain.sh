#!/bin/sh
#
# The merge drain, after 2026-10-03: it watches the queue and says who must act. It never arms,
# enqueues or re-queues a pull request.
#
#     helpers/merge-drain.sh              # one pass
#     helpers/merge-drain.sh --dry-run    # one pass, printing every write instead of making it
#
# PROVISIONAL NAME. Minted 2026-08-04 for a script that drained the queue by arming it, and kept
# when that stopped, because a rename is calef's call. See notes/merge-queue.md.
#
# # What it does
#
#   - Labels `needs-maintainer` (name provisional) on a pull request a maintainer session must pick
#     up, comments once per cause with the evidence, and takes the label off when the cause is
#     gone (`needs_maintainer`, below; the decision is helpers/needs-maintainer.jq).
#   - Dequeues a pull request that picked up `needs-architect`, `held-by-lane` or
#     `held-for-red-trunk` after it was enqueued (`dequeue_held`).
#   - Labels a paused draft `unblocked` once its `Blocked-by:` pull requests have resolved.
#   - Reruns, once, a CI run a concurrency group cancelled as a same-second duplicate.
#   - Opens a draft for a branch holding work that never had a pull request, so the label has
#     somewhere to go (the `orphan` cause's `adopt`; it never touches the branch).
#   - Reruns, once, the workflow owning a required check that never reported on an armed head whose
#     runs have all finished, and flags `missing-check` when that did not help (`missing_check_scan`).
#   - Recovers a queue entry whose merge group's runs all finished but which still reads
#     AWAITING_CHECKS (the `stuck-head` cause): the first time, `requeue_stuck_head` dequeues it
#     and enqueues it again at the back; the second time it only labels. The one exception to
#     "it never acts on the queue" below (calef, #1833).
#   - Runs helpers/lane-claim-check.sh, which reports a pushed lane branch with no pull request.
#
# # What it stopped doing, and why (milestone 727 (a queue eviction goes to a maintainer session), provisional; calef's rulings on #1564)
#
# On 2026-10-03 calef was the queue's only working detector: he spotted each conflicting or
# failing pull request, took it out, and told the maintainer session. Meanwhile this script armed
# every eligible pull request on every pass, re-armed ejected ones, enqueued what auto-merge had
# left behind, and commented on stalls. It re-queued pull requests the calef account had just
# taken out six or seven times that day, gave the already-merged #1555 a stale queue entry, and its
# 478 stall comments since 2026-08-26 moved nobody (55% of pull requests saw an action in the hour
# after one, 50% in the hour before). The record is
# notes/coes/2026-10-03-the-queue-judged-one-pull-request-at-a-time.md.
#
# calef ruled: "I don't want the job of watching the queue." So the arming, the re-arming, the
# enqueue of a stranded pull request, the stall comments, the stale-draft and stuck-check reports
# and milestone 630 (a merge-queue ejection is caught before the queue, and recovered after it)'s ejection hold all went. A lane arms its own pull request in the same command
# as `gh pr ready`. What none of them did was hand a problem to someone whose job it is, and that
# is the one thing built in their place.
#
# # Where it runs
#
# `.github/workflows/merge-drain.yml`, as `nife-smelter[bot]`, every five minutes and whenever CI
# completes. A pass from a checkout is the same pass, as whatever `gh` is logged in as. The old
# watching form is gone: the schedule is the loop, and a loop on a laptop was a second actor.

set -e
cd "$(dirname "$0")/.."

REPO="nifeos/nife"
# The three hold labels (HELD_LABEL, LANE_HELD_LABEL, RED_TRUNK_LABEL) are defined once, in
# helpers/hold-labels.sh, which helpers/arm.sh also reads to refuse arming a held pull request.
. "$(dirname "$0")/hold-labels.sh"

# **Which drain spoke.** A GitHub App installation token carries the App and not the caller, so
# GitHub cannot tell a reader whether the workflow or a laptop acted; the tag lives in the content.
# The workflow sets `actions:<run id>`, a run somebody can open; a laptop tags itself with its
# hostname. `$ME` prefixes every line this script prints and every comment it posts. The comment
# markers are deliberately not tagged: they are the dedupe key, and a key that changed with the
# instance would let two drains each post the same comment once.
INSTANCE="${MERGE_DRAIN_INSTANCE:-$(hostname -s 2>/dev/null || echo unknown)}"
ME="merge-drain[$INSTANCE]"


dry=""
case "$1" in
--dry-run) dry=1 ;;
# Accepted and ignored: every run is one pass now, and the workflow and older notes say `--once`.
--once | "") ;;
*)
	echo "usage: $(basename "$0") [--dry-run]" >&2
	exit 2
	;;
esac

# Every write goes through here, so `--dry-run` can exercise a pass against the live repository and
# change nothing. That is how a change to this script is tried before it merges, since the
# scheduled workflow runs only from `main`.
w() {
	if [ -n "$dry" ]; then
		echo "$ME: (dry run) would run: $(printf '%s ' "$@" | tr '\n' ' ' | cut -c1-160)"
	else
		"$@" >/dev/null 2>&1
	fi
}

# Prints "<node id> <count of removed_from_merge_queue events>" for a pull request, by GraphQL.
removal_state() {
	gh api graphql -f query='query($o:String!,$n:String!,$num:Int!){repository(owner:$o,name:$n){pullRequest(number:$num){id timelineItems(last:100,itemTypes:[REMOVED_FROM_MERGE_QUEUE_EVENT]){nodes{__typename}}}}}' \
		-f o="${REPO%%/*}" -f n="${REPO#*/}" -F num="$1" \
		--jq '.data.repository.pullRequest | "\(.id) \(.timelineItems.nodes | length)"' 2>/dev/null || echo " 0"
}

# **Dequeue anything now held that is already in the merge queue.** Admission is checked once, at
# enqueue time, and until 2026-09-18 nothing ever re-checked it, so a label arriving *after* the
# enqueue was ignored by everything: the `architect hold` check on the pull request went red, and the
# queue carried on regardless because the queue does not read labels.
#
# **This is not hypothetical and the window is small enough to lose a race in.** On 2026-09-18 this
# drain enqueued #923 at 03:53:36; its lane, which had discovered mid-flight that its work disproved
# a DECISIONS premise and therefore needed calef, labelled it `needs-architect` at 03:54:49. Seventy
# three seconds. calef noticed it merging and it was dequeued by hand. The lane had already dequeued
# itself once at 03:08:05, and this drain simply put it back, which is the part a lane cannot defend
# against on its own.
#
# The shape is AGENTS.md's ladder: the label was rung two (a gate that fires without being
# remembered) for everything *except* the queue, where it was rung zero. This closes that, on the
# side that can see both facts. A lane discovering late that it needs an architect is the normal
# case rather than the exceptional one, because finding the thing that needs deciding is usually the
# work.
dequeue_held() {
	gh pr list --repo "$REPO" --state open --json number,labels,title 2>/dev/null |
		jq -r --arg L "$HELD_LABEL" --arg B "$LANE_HELD_LABEL" --arg R "$RED_TRUNK_LABEL" '
			.[]
			| (.labels | map(.name)) as $names
			| ([$L, $B, $R] | map(select(. as $l | $names | index($l))) | first) as $why
			| select($why != null)
			| "\(.number)\t\($why)\t\(.title)"' 2>/dev/null |
		while IFS="$(printf '\t')" read -r num why title; do
			[ -n "$num" ] || continue
			# `dequeuePullRequest` is a no-op on a pull request that is not queued, so this needs no
			# membership test: asking is cheaper than checking, and the check would race anyway.
			# GraphQL for the lookup and the before/after count: the REST budget is shared by every agent
			# on the account (notes/skills/maintainer/SKILL.md). One call returns the node id and the count of
			# removed-from-merge-queue events (the last hundred; a count that cannot grow reads as no move).
			st=$(removal_state "$num") || continue
			id=${st% *} before=${st#* }
			[ -n "$id" ] || continue
			w gh api graphql -f query="mutation{dequeuePullRequest(input:{id:\"$id\"}){clientMutationId}}" || continue
			st=$(removal_state "$num") && after=${st#* } || after=$before
			# Only speak when something actually moved. A held pull request that was never queued is
			# the common case and saying so every five minutes is how a watcher gets muted.
			if [ "$after" -gt "$before" ]; then
				echo "$ME: DEQUEUED #$num ($why arrived after it was enqueued): $title"
			fi
		done
}

# What "eligible for the merge queue" means: not a draft, into `main`, and from this repository
# rather than a fork. One file, helpers/queue-eligible.jq, shared with queue-hold.sh and the
# detector's decision; helpers/queue-eligible-selftest.sh checks it under script/lint.
ELIGIBLE_JQ="$(dirname "$0")/queue-eligible.jq"

# Whether pull request $1 already carries a comment with marker $2.
marked() {
	count=$(gh "${NM_KIND:-pr}" view "$1" --repo "$REPO" --json comments 2>/dev/null |
		jq -r --arg m "$2" '[.comments[] | select(.body | contains($m))] | length' 2>/dev/null) || true
	[ "${count:-0}" != "0" ]
}

# Post `$3` on pull request `$1` once, ever, keyed by the marker `$2` (an HTML comment, invisible
# when rendered). A condition that persists is re-detected every pass, and a comment every pass
# buries the one useful comment within the hour.
notify() {
	marked "$1" "$2" && return 0
	w gh "${NM_KIND:-pr}" comment "$1" --repo "$REPO" --body "$3

<!-- $2 -->"
}

# `Blocked-by: #N[, #M ...]` in a pull request body: a hold that names its own release condition,
# for a mechanical ordering constraint (two branches that are green alone and red together), as
# opposed to `needs-architect`, which means a person must decide something. A manual hold label
# was refused because a label has to be removed by whoever remembers, and this tree has watched
# that fail (notes/merge-queue.md has the history). On a draft it earns the `unblocked` label
# below; on a ready pull request it keeps the detector's `unarmed` cause off until every pull
# request it names has merged or closed. helpers/blocked-by.sh is the parser.
. "$(dirname "$0")/blocked-by.sh"

UNBLOCKED_LABEL="unblocked"
BLOCKED_BY_RESOLUTION_JQ="$(dirname "$0")/blocked-by-resolution.jq"

# A paused DRAFT whose `Blocked-by:` line has resolved. A draft that is genuinely waiting on
# another pull request looks exactly like a finished lane that went quiet, so the waiting has to be
# written where a machine can read it.
#
# # Why this exists
#
# Draft #1289 was paused on 2026-09-25 waiting on #1288. #1288 merged an hour later. Nobody resumed
# #1289 for two days, because the pause was recorded only in prose (a comment, a report). A mechanism that fires once and a fact that lives only in prose is exactly the shape
# AGENTS.md's ladder warns about: "somebody will notice" is rung zero.
#
# So this is rung two instead: a draft that names `Blocked-by: #N[, #M ...]` in its own body gets a
# label, `unblocked`, and one comment, the moment every listed pull request has merged or been
# reported closed. Both are visible without opening the pull request: `gh pr list --label unblocked`
# is the whole board, the same shape as `gh pr list --draft` and `gh pr list --label
# needs-architect` already are.
#
# The decision itself (merged, still open, closed unmerged, or already labelled) is
# helpers/blocked-by-resolution.jq, spliced in below the same way helpers/queue-eligible.jq is
# elsewhere in this script; helpers/blocked-by-resolution-selftest.sh checks it against fixtures
# without a `gh` call, and script/lint runs that selftest. The label and the marker comment are
# this function's own job, because they need `gh` and the jq file must not.
#
# **A blocker CLOSED without merging is reported as such rather than folded into a plain
# "unblocked"**: it usually means the plan changed, and that is
# a fact for a person to read rather than release silently.
unblocked_drafts() {
	gh pr list --repo "$REPO" --state open --json number,isDraft,title,body,labels 2>/dev/null |
		jq -c --arg L "$UNBLOCKED_LABEL" '
			.[]
			| select(.isDraft == true)
			| select(.body | test("(?i)blocked-by:"))
			| {number, title, body, has_label: ((.labels | map(.name) | index($L)) != null)}
		' 2>/dev/null |
		while IFS= read -r pr; do
			[ -n "$pr" ] || continue
			num=$(printf '%s' "$pr" | jq -r '.number')
			title=$(printf '%s' "$pr" | jq -r '.title')
			body=$(printf '%s' "$pr" | jq -r '.body')
			has_label=$(printf '%s' "$pr" | jq -r '.has_label')

			blockers=$(nife_blocked_by "$body")
			[ -n "$blockers" ] || continue

			# One `gh pr view` per named blocker, building the fixture `blocked-by-resolution.jq`
			# reads and the human-readable lines (with timestamps) the jq file deliberately does
			# not carry, because its own selftest fixtures are state-only.
			blockers_json='[]'
			merged_line=""
			closed_line=""
			for b in $blockers; do
				bjson=$(gh pr view "$b" --repo "$REPO" --json state,mergedAt 2>/dev/null)
				bstate=$(printf '%s' "$bjson" | jq -r '.state // "OPEN"' 2>/dev/null)
				[ -n "$bstate" ] || bstate="OPEN"
				blockers_json=$(printf '%s' "$blockers_json" |
					jq -c --argjson n "$b" --arg s "$bstate" '. + [{number: $n, state: $s}]')
				case "$bstate" in
				MERGED)
					mergedat=$(printf '%s' "$bjson" | jq -r '.mergedAt // "an unknown time"' 2>/dev/null)
					merged_line="${merged_line}#$b merged at $mergedat; "
					;;
				CLOSED)
					closed_line="${closed_line}#$b "
					;;
				esac
			done

			result=$(jq -cn --argjson hl "$has_label" --argjson bl "$blockers_json" \
					'{has_label: $hl, blockers: $bl}' |
				jq -c "$(cat "$BLOCKED_BY_RESOLUTION_JQ")"'resolution' 2>/dev/null)
			status=$(printf '%s' "$result" | jq -r '.status' 2>/dev/null)

			case "$status" in
			already-labelled | waiting | "")
				continue
				;;
			closed-unmerged)
				msg="merge-drain: UNBLOCKED. ${closed_line}closed unmerged; resume this lane and check whether the plan changed."
				;;
			unblocked)
				msg="merge-drain: UNBLOCKED. ${merged_line}resume this lane."
				;;
			esac

			echo "$ME: UNBLOCKED #$num ($title)"
			notify "$num" "merge-drain:unblocked" "$msg"
			w gh pr edit "$num" --repo "$REPO" --add-label "$UNBLOCKED_LABEL" || true
		done
}

# The `unblocked` label removes itself the moment its own reason stops applying, the same shape as
# `Blocked-by:` itself: nothing to remember, because a manual label that outlives its reason is a
# false signal and this project has the receipts for that (see the comment above `blocked_by`,
# now `nife_blocked_by` in helpers/blocked-by.sh). Its reason stops applying in exactly two ways:
# a new commit lands (the lane resumed on its own before anyone read the comment), or the draft is
# marked ready (it is not paused any more, it is asking to merge). Checked every pass rather than
# left to the marker comment, because leaving a stale label costs nothing to detect and everything
# to trust if it lingers on a draft that has clearly moved on.
release_unblocked_labels() {
	gh pr list --repo "$REPO" --state open --json number,isDraft,labels,commits 2>/dev/null |
		jq -r --arg L "$UNBLOCKED_LABEL" '
			.[]
			| select((.labels | map(.name) | index($L)) != null)
			| [.number, .isDraft, (.commits[-1].committedDate // "")]
			| @tsv
		' 2>/dev/null |
		while IFS="$(printf '\t')" read -r num isdraft lastcommit; do
			[ -n "$num" ] || continue
			if [ "$isdraft" = "false" ]; then
				w gh pr edit "$num" --repo "$REPO" --remove-label "$UNBLOCKED_LABEL" || true
				continue
			fi
			[ -n "$lastcommit" ] || continue
			labeled_at=$(gh api graphql -f query='query($o:String!,$n:String!,$num:Int!){repository(owner:$o,name:$n){pullRequest(number:$num){timelineItems(last:100,itemTypes:[LABELED_EVENT]){nodes{... on LabeledEvent{createdAt label{name}}}}}}}' \
				-f o="${REPO%%/*}" -f n="${REPO#*/}" -F num="$num" 2>/dev/null |
				jq -r --arg L "$UNBLOCKED_LABEL" '
					[.data.repository.pullRequest.timelineItems.nodes[] | select(.label.name == $L)] | last | .createdAt // empty
				' 2>/dev/null)
			[ -n "$labeled_at" ] || continue
			newer=$(jq -n --arg a "$lastcommit" --arg b "$labeled_at" \
				'(($a | fromdateiso8601) > ($b | fromdateiso8601))' 2>/dev/null)
			if [ "$newer" = "true" ]; then
				w gh pr edit "$num" --repo "$REPO" --remove-label "$UNBLOCKED_LABEL" || true
			fi
		done
}

# **Rerun the CI run a concurrency group cancelled as a duplicate, once** (2026-09-24, #1203's
# cause, found by the A′ lane and written up in notes/merge-queue.md's BUGS). One push can raise two
# `synchronize` events; the group cancels the newer copy before any job exists; GitHub reads the
# newest run per workflow, so the empty cancelled suite hides the green one and the queue answers
# "11 of 13 required status checks are expected". No workflow-level fix is sound, so the drain
# reruns the run it finds. It is not arming: it makes a check report, and the pull request's own
# auto-merge, or its absence, decides the rest. The detection is helpers/cancelled-duplicate.jq, the note's own query,
# and `rerunnable` is what decides: only a run at `run_attempt` 1, so the same run is never rerun
# twice and the run itself is the record. The rerun needs `actions: write`, which the App's token
# does not carry, since milestone 128 (the automation gets its own identity) minted it with
# Contents and Pull requests; `merge-drain.yml`
# passes the workflow's own token as MERGE_DRAIN_RERUN_TOKEN for this one call, and a laptop run
# uses whatever `gh` is logged in as.
CANCELLED_JQ="$(dirname "$0")/cancelled-duplicate.jq"
# The runs still owed a rerun at `$1` (a head SHA): "<id> <workflow name>" per line.
cancelled_duplicate_runs() {
	gh api "repos/$REPO/actions/runs?head_sha=$1&event=pull_request&per_page=100" 2>/dev/null |
		jq -r "$(cat "$CANCELLED_JQ")"'rerunnable | "\(.id) \(.name)"' 2>/dev/null
}
# `gh run rerun` with the token that may do it: the workflow's own under Actions, `gh`'s login on a
# laptop. Exported only for this call, and only when set, so a laptop run with no GH_TOKEN keeps
# its keyring login rather than an empty variable.
rerun_run() {
	if [ -n "$dry" ]; then
		echo "$ME: (dry run) would rerun run $1"
	elif [ -n "$MERGE_DRAIN_RERUN_TOKEN" ]; then
		GH_TOKEN="$MERGE_DRAIN_RERUN_TOKEN" gh run rerun "$1" --repo "$REPO" >/dev/null 2>&1
	else
		gh run rerun "$1" --repo "$REPO" >/dev/null 2>&1
	fi
}

# The candidates: eligible, BLOCKED, nothing still running, and a CANCELLED check at the head. The
# rollup is the cheap filter, so the runs API is asked only for a pull request in this shape.
rerun_cancelled_duplicates() {
	tab="$(printf '\t')"
	gh pr list --repo "$REPO" --state open \
		--json number,title,isDraft,baseRefName,isCrossRepository,mergeStateStatus,headRefOid,statusCheckRollup 2>/dev/null |
		jq -r "$(cat "$ELIGIBLE_JQ")"'
			.[] | eligible
			| select(.mergeStateStatus == "BLOCKED")
			| select(.statusCheckRollup
				| (map(select(.status == "QUEUED" or .status == "IN_PROGRESS" or .status == "PENDING")) | length) == 0
				  and (map(select(.conclusion == "CANCELLED")) | length) > 0)
			| "\(.number)\t\(.headRefOid)\t\(.title)"' 2>/dev/null |
		while IFS="$tab" read -r num sha title; do
			[ -n "$num" ] || continue
			cancelled_duplicate_runs "$sha" | while IFS=' ' read -r run_id run_name; do
				[ -n "$run_id" ] || continue
				if rerun_run "$run_id"; then
					echo "$ME: RERAN #$num run $run_id ($run_name was cancelled as a same-second duplicate and hid the green one) ($title)"
				else
					echo "$ME: #$num run $run_id ($run_name) is a cancelled duplicate and the rerun was refused; the token may lack actions:write ($title)"
				fi
			done
		done
}

# **A required check that never reported** (lane/drain-missing-check, 2026-10-07 UTC; names
# provisional). For each armed, ready pull request whose pull-request workflow runs have all
# finished, ask whether every context the `main` ruleset requires has a check run at the head
# (helpers/missing-check.jq decides). If one is absent, rerun, once, the workflow that owns it:
# `run_attempt` above 1 is the record, so the next pass finds the rerun already tried and the
# `missing-check` cause (helpers/needs-maintainer.jq) names the absent checks for a maintainer.
# Like the cancelled-duplicate rerun this makes a check report and arms nothing. The verdicts are
# left in MC_MAP, { "<number>": { absent, tried } }, for `needs_maintainer` to splice in.
#
# BUGS: the owner of a context is found by a job `name:` at four-space indent in a workflow file,
# so a job whose name is an expression (a matrix shard) has no owner and is flagged without a
# rerun. A check that comes from a commit status rather than a check run is read from the status
# API. The ruleset is read with the token the pass has; if that read fails the scan does nothing.
MISSING_JQ="$(dirname "$0")/missing-check.jq"
# "<context><TAB><workflow name>" for every job name in .github/workflows.
workflow_owners() {
	for wf in .github/workflows/*.yml; do
		[ -f "$wf" ] || continue
		awk -v tab="$(printf '\t')" '
			/^name:/ && !top { top = $0; sub(/^name:[ ]*/, "", top); gsub(/^["\x27]|["\x27]$/, "", top) }
			/^    name:/ { n = $0; sub(/^    name:[ ]*/, "", n); gsub(/^["\x27]|["\x27]$/, "", n); print n tab top }
		' "$wf"
	done
}
MC_MAP='{}'
missing_check_scan() {
	MC_MAP='{}'
	required=$(gh api "repos/$REPO/rules/branches/main" 2>/dev/null |
		jq -c '[ .[] | select(.type == "required_status_checks") | .parameters.required_status_checks[].context ] | unique' 2>/dev/null) || required=""
	[ -n "$required" ] && [ "$required" != "[]" ] || return 0
	owners=$(workflow_owners | jq -R -s -c 'split("\n") | map(select(. != "") | split("\t")) | map({(.[0]): .[1]}) | add // {}')
	for row in $(printf '%s' "$1" | jq -r '.data.repository.pullRequests.nodes[]
			| select(.isDraft == false and .isCrossRepository == false and .autoMergeRequest != null)
			| "\(.number):\(.headRefOid)"'); do
		num=${row%%:*}
		sha=${row#*:}
		runs=$(gh api "repos/$REPO/actions/runs?head_sha=$sha&per_page=100" --jq '[.workflow_runs[] | {id, name, status, conclusion, run_attempt, event}]' 2>/dev/null) || continue
		present=$( { gh api --paginate "repos/$REPO/commits/$sha/check-runs?per_page=100" --jq '.check_runs[].name' &&
			gh api "repos/$REPO/commits/$sha/status" --jq '.statuses[].context'; } 2>/dev/null | jq -R -s -c 'split("\n") | map(select(. != ""))') || continue
		verdict=$(jq -n -c --argjson required "$required" --argjson present "$present" --argjson runs "$runs" --argjson owners "$owners" \
			"{required: \$required, present: \$present, runs: \$runs, owners: \$owners} | $(cat "$MISSING_JQ")"'[ missing_checks ] | .[0] // empty' 2>/dev/null) || continue
		[ -n "$verdict" ] || continue
		for rid in $(printf '%s' "$verdict" | jq -r '.rerun[].id'); do
			rname=$(printf '%s' "$verdict" | jq -r --argjson i "$rid" '.rerun[] | select(.id == $i) | .name')
			if rerun_run "$rid"; then
				echo "$ME: RERAN #$num run $rid ($rname finished with a required check never created: $(printf '%s' "$verdict" | jq -r '.absent | join("; ")'))"
			else
				echo "$ME: #$num run $rid ($rname) lacks a required check and the rerun was refused; the token may lack actions:write"
			fi
		done
		MC_MAP=$(printf '%s' "$MC_MAP" | jq -c --arg n "$num" --argjson v "$verdict" '. + {($n): {absent: $v.absent, tried: $v.tried}}')
	done
}

# The merge group's runs that did not succeed, as markdown list lines: "- <name>: <conclusion>, <url>".
group_runs() {
	[ "$1" = "null" ] && return 0
	gh api "repos/$REPO/actions/runs?head_sha=$1&event=merge_group&per_page=100" \
		--jq '.workflow_runs[] | select(.conclusion != "success" and .conclusion != "skipped")
			| "- \(.name): \(.conclusion // .status), \(.html_url)"' 2>/dev/null || true
}

# **The one place the drain acts on the queue, a marked exception** (calef, #1833, 2026-10-07 UTC).
# The #1564 rulings (2026-10-03) took every arm, re-arm and enqueue away from this script, because
# it fought calef's own dequeues. #1824 then sat at the head from 16:38 to 19:42 UTC with every
# merge-group run completed and a required check never posted, and the maintainer had to dequeue
# and re-enqueue it by hand. calef ruled that the drain does that itself, for that case only, and
# only the first time: a second stall of the same pull request gets `needs-maintainer` and no
# re-queue (the decision in helpers/needs-maintainer.jq reads its own earlier marker comment).
# THIS IS A FOOT GUN TO EXTEND. Do not add a second caller or a second cause that re-queues; the
# selftest (section 5) cuts this function out before it forbids every other arm and enqueue, and
# requires exactly one caller. The App's token can do it: `dequeue_held` already dequeues under
# the same `pull-requests: write`, and `enqueuePullRequest` reached GitHub's eligibility logic
# under it on 2026-09-24 (see .github/workflows/merge-drain.yml).
# $1 = pull request number, $2 = its GraphQL node id. Returns nonzero if nothing was re-queued.
requeue_stuck_head() {
	w gh api graphql -f query="mutation{dequeuePullRequest(input:{id:\"$2\"}){clientMutationId}}" || return 1
	w gh api graphql -f query="mutation{enqueuePullRequest(input:{pullRequestId:\"$2\"}){clientMutationId}}" && return 0
	w gh pr merge "$1" --repo "$REPO" --auto --merge
}

# **A pull request a maintainer session must pick up wears `needs-maintainer`** (milestone 727,
# provisional; the label's name is provisional too, and calef's call). The causes, and why
# each is one, are in helpers/needs-maintainer.jq, which decides; this carries the decision out:
#
#   label   add the label, print `LABELLED #N`, and comment once per cause with the evidence
#   keep    comment on any cause whose episode has not been commented on yet
#   clear   the cause is gone (back in the queue, head moved, conflict fixed, armed, merged,
#           a draft again): take the label off and print `CLEARED #N`. A session never has to.
#   adopt   a branch holding work has never had a pull request (the `orphan` cause), so there is
#           nothing to label: open a draft for it, marked with the head it adopted, label that,
#           and print `ADOPTED <branch> as #N`. This is the drain's one write that creates a pull
#           request. It is a draft, so it can never merge, and a draft is the claim §90 (the claim
#           is a draft pull request) asks every lane to open first; the drain opens the one its
#           lane did not. Closing it is the undo.
#
# The label is created idempotently by the workflow, with the workflow's own token, because the
# App's token may not create labels. A laptop pass assumes it exists.
#
# The session-side half is rung three, a written record: briefs/session-start.md runs
# the `needs-maintainer` listing first (the REST issues endpoint, pull requests and issues in one
# call, since milestone 808 (every gate accounts for its time) put a cause on issues) and fixes
# those first, and helpers/nanny.py wakes a running session when the label lands on a pull request.
# `state=all` because a stale queue entry belongs to a pull request that is no longer open.
#
# BUGS, the `orphan` cause's (lane/orphan-work, 2026-10-06 UTC):
#   - Its clock is the branch tip's committer date, not the branch's birth, so a lane that keeps
#     pushing without a pull request is never called an orphan while it pushes. The birth clock
#     `helpers/lane-claim-check.sh` reads is the repository activity feed, one page deep; a tip
#     clock needs no second call and fires two hours after a lane stops, which is the case that
#     went unseen.
#   - It reads the first 100 branches. The repository had 23 on 2026-10-06; past 100, a branch
#     beyond the page is not seen.
#   - It cannot see work that never left a laptop: a detached HEAD or an unpushed commit is
#     invisible to GitHub. helpers/at-risk-check.sh lists those, and briefs/merge-and-cleanup.md
#     runs it before a prune.
#   - helpers/lane-claim-check.sh still prints the same branches to this log, on its own clocks.
#     Folding it into this cause, or keeping it, is calef's call (#1787).
#
# BUGS, the `unmergeable` cause's (lane/queue-unmergeable-report, 2026-10-07 UTC):
#   - No grace period. Whether GitHub ever shows `UNMERGEABLE` for a moment and then clears it on
#     its own is unmeasured; if it does, this labels and comments on noise, and the label comes off
#     a pass later. If the log shows the label going on and coming off one pass apart, key a grace
#     on `enqueuedAt` the way `unarmed` uses `$minutes`.
#   - The comment compares the head with each entry ahead one pair at a time. An entry can be
#     unmergeable only against two of them together, or for a reason that is not a textual
#     conflict; then every line reads "merges cleanly", and the verdict rests on GitHub's word.
#   - The dedupe key is the head. A head dequeued and enqueued again that goes `UNMERGEABLE` a
#     second time is labeled again but not commented again; the first comment still applies.
#   - It reads the first 100 entries, and it sees the queue only when a pass runs: every five
#     minutes when GitHub's scheduler keeps up, and whenever a CI run completes, which includes the
#     merge-group runs of the entries ahead (merge-drain.yml's BUGS has why the schedule is a floor).
NM_LABEL="needs-maintainer"
NM_MINUTES=${NM_MINUTES:-30}
NM_JQ="$(dirname "$0")/needs-maintainer.jq"
# What makes a question for calef open, shared with script/architect-queue (lane/architect-queue).
OQ_JQ="$(dirname "$0")/open-question.jq"
STUCK_JQ="$(dirname "$0")/stuck-head.jq"
# The diff rules architect-label.yml runs on each push, run here again on every head that carries no
# hold or ruling label, because a push event is not guaranteed: GitHub runs no `pull_request`
# workflow while a pull request conflicts with its base, which is how #1745's syscall change went
# unread (2026-10-06).
RULES_PY="$(dirname "$0")/architect-label-rules.py"
# Every field helpers/needs-maintainer.jq reads, in one call. helpers/needs-maintainer-selftest.sh
# checks this text names each of them, and its fixtures are this query's recorded responses.
NM_QUERY='query($owner: String!, $name: String!, $labelled: String!, $budget: String!) {
  repository(owner: $owner, name: $name) {
    mergeQueue(branch: "main") { entries(first: 100) { nodes { position enqueuedAt state headCommit { oid } pullRequest { id number state mergedAt headRefOid } } } }
    pullRequests(states: OPEN, first: 100, orderBy: {field: CREATED_AT, direction: DESC}) { nodes {
      number isDraft baseRefName isCrossRepository headRefName headRefOid createdAt mergeable body url
      labels(first: 30) { nodes { name } }
      comments(last: 30) { nodes { createdAt url body } }
      autoMergeRequest { enabledAt }
      removed: timelineItems(last: 1, itemTypes: [REMOVED_FROM_MERGE_QUEUE_EVENT]) { nodes { ... on RemovedFromMergeQueueEvent { createdAt reason beforeCommit { oid parents(first: 2) { nodes { oid } } } } } }
      added: timelineItems(last: 1, itemTypes: [ADDED_TO_MERGE_QUEUE_EVENT]) { nodes { ... on AddedToMergeQueueEvent { createdAt } } }
      unarmed: timelineItems(last: 1, itemTypes: [READY_FOR_REVIEW_EVENT, AUTO_MERGE_DISABLED_EVENT, REMOVED_FROM_MERGE_QUEUE_EVENT]) { nodes { __typename ... on ReadyForReviewEvent { createdAt } ... on AutoMergeDisabledEvent { createdAt } ... on RemovedFromMergeQueueEvent { createdAt } } }
      labelEvents: timelineItems(last: 20, itemTypes: [LABELED_EVENT]) { nodes { ... on LabeledEvent { createdAt label { name } } } }
      commits(last: 1) { nodes { commit { committedDate statusCheckRollup { contexts(first: 100) { nodes { __typename ... on CheckRun { status completedAt checkSuite { workflowRun { workflow { name } } } } } } } } } }
    } }
    issues(states: OPEN, first: 50, orderBy: {field: CREATED_AT, direction: DESC}) { nodes {
      number createdAt url body
      labels(first: 30) { nodes { name } }
      comments(last: 30) { nodes { createdAt url body } }
      labelEvents: timelineItems(last: 20, itemTypes: [LABELED_EVENT]) { nodes { ... on LabeledEvent { createdAt label { name } } } }
    } }
    refs(refPrefix: "refs/heads/", first: 100) { nodes {
      name
      target { oid ... on Commit { committedDate } }
      compare(headRef: "main") { behindBy }
      associatedPullRequests(last: 5, orderBy: {field: CREATED_AT, direction: ASC}) { nodes { number state labels(first: 30) { nodes { name } } } }
    } }
  }
  search(query: $labelled, type: ISSUE, first: 50) { nodes { __typename ... on PullRequest { number state labels(first: 30) { nodes { name } } } ... on Issue { number state labels(first: 30) { nodes { name } } } } }
  budget: search(query: $budget, type: ISSUE, first: 50) { nodes { ... on Issue { number state createdAt url title labels(first: 30) { nodes { name } } } } }
}'
# The label on a CI job's near-budget tracking issue (milestone 808 (every gate accounts for its
# time); helpers/ci_job_times.py opens the issue, helpers/needs-maintainer.jq names the cause).
NM_BUDGET_LABEL="near-budget"

# The files a head conflicts on against `main`, as markdown list lines. `git merge-tree
# --write-tree` needs no worktree; it exits 1 on a conflict and prints the tree, then one
# conflicted path per line. The workflow's checkout has every branch, so a same-repository head is
# local. GitHub's `mergeable` is computed lazily and can trail `main` (on 2026-10-03 it called #1569
# CONFLICTING while git merged it cleanly), so a clean merge here is said rather than hidden.
conflict_files() {
	if ! git cat-file -e "$1^{commit}" 2>/dev/null; then
		echo "- (the head is not in this checkout, so the conflicting files are not listed)"
		return 0
	fi
	if out=$(git merge-tree --write-tree --name-only --no-messages origin/main "$1" 2>/dev/null); then
		echo "- (git merges this head cleanly against \`main\` at $(git rev-parse --short origin/main 2>/dev/null), so GitHub's verdict may be stale; re-check it before rebasing)"
	else
		printf '%s\n' "$out" | tail -n +2 | head -20 | sed 's/^/- `/; s/$/`/'
	fi
}

# What head $1 does against each entry ahead of it ($2, the cause's `ahead`), as markdown list
# lines. Pairwise rather than against the group GitHub builds, because the group's commit is not
# fetched and a pair is what a lane can act on: "wait for #1745, then rebase". Each pair merges at
# its own merge base, so a conflict here is a conflict between the two changes, whatever `main` did.
ahead_conflicts() {
	printf '%s' "$2" | jq -r '.[] | "\(.number) \(.head)"' | while read -r n h; do
		if ! git cat-file -e "$1^{commit}" 2>/dev/null || ! git cat-file -e "$h^{commit}" 2>/dev/null; then
			echo "- #$n: (a head is not in this checkout, so it was not compared)"
		elif out=$(git merge-tree --write-tree --name-only --no-messages "$h" "$1" 2>/dev/null); then
			echo "- #$n: merges cleanly with this head"
		else
			echo "- #$n: conflicts in $(printf '%s\n' "$out" | tail -n +2 | head -10 | sed 's/^/`/; s/$/`/' | paste -sd ' ' - | sed 's/` `/`, `/g')"
		fi
	done
}

# The comment for one cause ($2, a JSON object) on pull request $1.
nm_comment() {
	c="$2"
	cause=$(printf '%s' "$c" | jq -r '.cause')
	case "$cause" in
	ejected)
		at=$(printf '%s' "$c" | jq -r '.at')
		reason=$(printf '%s' "$c" | jq -r '.reason')
		group=$(printf '%s' "$c" | jq -r '.group')
		head=$(printf '%s' "$c" | jq -r '.head')
		runs=$(group_runs "$group")
		[ -n "$runs" ] || runs="- no merge-group run to name (a \`merge_conflict\` builds none)"
		what="EJECTED from the merge queue at $at, reason \`$reason\`, at head \`$head\`. The ejection cancelled auto-merge, and nothing re-queues it.

$runs

Hand it to its lane: a pushed fix, or the conflict resolved, takes this label off. If the failure is a known flake, re-arm it (\`helpers/arm.sh $1\`), which refuses this head unless a failed job's log matches \`helpers/known-flakes.tsv\`; back in the queue takes the label off too."
		;;
	conflict)
		head=$(printf '%s' "$c" | jq -r '.head')
		files=$(conflict_files "$head") || files=""
		what="CONFLICTS with \`main\` at head \`$head\`.

$files

Rebase or merge \`main\` (briefs/rebase-onto-main.md), push, and arm it. The label comes off when the conflict is gone."
		;;
	unarmed)
		since=$(printf '%s' "$c" | jq -r '.since')
		blockers=$(printf '%s' "$c" | jq -r 'if (.blockers | length) > 0 then " Its `Blocked-by:` pull requests have all resolved (" + (.blockers | join(", ")) + ")." else "" end')
		what="READY AND UNARMED since $since, over $NM_MINUTES minutes, and not in the merge queue.$blockers Nothing arms a pull request but its lane or a maintainer session, and a lane that has ended its turn is not running: the maintainer session owns this.

If it is done, arm it: \`helpers/arm.sh $1\`. If it is not, make it a draft again: \`gh pr ready $1 --undo\`. Either takes the label off."
		;;
	off-main)
		since=$(printf '%s' "$c" | jq -r '.since')
		base=$(printf '%s' "$c" | jq -r '.base')
		basepr=$(gh pr list --repo "$REPO" --state open --head "$base" --json number -q '.[0].number // empty' 2>/dev/null) || basepr=""
		[ -n "$basepr" ] && basepr=" (#$basepr's head)" || basepr=""
		what="READY SINCE $since, over $NM_MINUTES minutes, and based on \`$base\`$basepr rather than \`main\`. The merge queue drains \`main\` only, so nothing will merge this, and nothing else watching the queue can see it. The maintainer session owns the next step, one of:

- merge it into its base, so it lands with that pull request: \`gh pr merge $1 --merge\`
- retarget it once its base has landed, then arm it: \`gh pr edit $1 --base main\`
- if it is waiting on its base on purpose, say so with a \`Blocked-by:\` line naming that pull request, or make it a draft: \`gh pr ready $1 --undo\`

Any of these takes the label off."
		;;
	red)
		since=$(printf '%s' "$c" | jq -r '.since')
		head=$(printf '%s' "$c" | jq -r '.head')
		branch=$(printf '%s' "$c" | jq -r '.branch')
		armed=$(printf '%s' "$c" | jq -r 'if .armed then "It is armed, and an armed pull request whose required checks fail never enters the queue, so it will sit here." else "It is not armed." end')
		what="RED: \`ci-failing\` has been on this pull request since $since, over $NM_MINUTES minutes, at head \`$head\`. The \`ci-failing\` comment names each failing check and its job. $armed

A lane that ended its turn \`WAITING\` is not running and will not see this. The maintainer session owns it: resume the lane that pushed \`$branch\` with the failing job's log, or fix it in place. A push whose checks pass takes \`ci-failing\` off, and this label with it."
		;;
	stale-draft)
		since=$(printf '%s' "$c" | jq -r '.since')
		branch=$(printf '%s' "$c" | jq -r '.branch')
		what="A DRAFT WITH NO COMMIT SINCE $since, by its head commit's committer date, on \`$branch\`. Its lane may have ended: a lane that ended its turn is not running, and nothing else will move this. The maintainer session owns it, by one of:

- resume or adopt the lane that pushed \`$branch\`
- mark it ready if the work is done: \`gh pr ready $1\`
- if it is waiting on purpose, say so with a \`Blocked-by:\` line naming what it waits on
- if it is held for work outside the lane system (calef's GLM runs, for one), label it \`parked\` and comment why
- close it, after recording anything it knows in \`notes/\` (CLAUDE.md: an unmerged branch is not the record)

A new commit, a \`Blocked-by:\` on an open pull request, \`parked\`, or closing it takes the label off."
		;;
	orphan)
		branch=$(printf '%s' "$c" | jq -r '.branch')
		head=$(printf '%s' "$c" | jq -r '.head')
		since=$(printf '%s' "$c" | jq -r '.since')
		ahead=$(printf '%s' "$c" | jq -r '.ahead // "some"')
		case "$(printf '%s' "$c" | jq -r '.shape')" in
		MERGED) shape="This pull request merged, and \`$branch\` has $ahead commit(s) since that \`main\` does not, the newest from $since." ;;
		CLOSED) shape="This pull request closed without merging, and \`$branch\` still holds $ahead commit(s) \`main\` does not, the newest from $since." ;;
		*) shape="\`$branch\` held commits that \`main\` does not and had no pull request, so the drain opened this draft for it. Its newest commit is from $since." ;;
		esac
		what="ORPHAN WORK at \`$head\`. $shape No open pull request carries it, so nothing shows it to anyone. The maintainer session owns the next step, one of:

- land it: open a pull request from \`$branch\` (or, for this draft, push to it or mark it ready)
- record what it found in \`notes/\` and delete the branch: \`git push origin --delete $branch\`
- keep it on purpose: label this pull request \`parked\` and comment why

Any of these takes the label off."
		;;
	hold-no-ask)
		since=$(printf '%s' "$c" | jq -r '.since')
		ruled=$(printf '%s' "$c" | jq -r 'if .ruled then "A ruling was recorded at \(.ruled) and nothing has asked since." else "Nothing on it asks a question under a \"What I need from you\" heading." end')
		what="IN CALEF'S QUEUE WITH NOTHING TO ANSWER: \`needs-architect\` has been on since $since, and there is no open question. $ruled The label is his work queue, so this costs him a look for nothing (#1783). The maintainer session owns the next step, one of:

- if calef has ruled, record it: \`script/record-ruling $1 --text '<his words>'\` (add \`--held-by-lane\` if the lane owes a change); it swaps the labels
- if there is a question, post it as a comment under a \`## What I need from you\` heading
- if nothing was ever owed to an architect, remove \`needs-architect\`

Any of these takes the label off."
		;;
	ask-no-hold)
		at=$(printf '%s' "$c" | jq -r '.at')
		url=$(printf '%s' "$c" | jq -r '.url')
		line=$(printf '%s' "$c" | jq -r '.line')
		what="A QUESTION FOR CALEF THAT HIS QUEUE COULD NOT SHOW: asked at $at ($url), with no ruling recorded since, and no \`needs-architect\` on this, so the drain added it (calef's ruling on #1792: the bot adds, and only flags removals). The question begins: \"$line\". If \`architect-ruled\` is on and this asks something the old ruling did not cover, remove \`architect-ruled\` so the hold blocks the merge. If calef has already answered, record it: \`script/record-ruling $1 --text '<his words>'\`. A section that asks for something to happen once this has merged is not a question: it belongs under \`## After this merges\`, which the drain does not read."
		;;
	surface-no-hold)
		head=$(printf '%s' "$c" | jq -r '.head')
		# Each finding as the question calef answers (the rules file's `--ask`; calef on #1838,
		# 2026-10-08 UTC: "The label doesn't make sense on its own."), the raw line if that fails.
		rules=$(printf '%s' "$c" | jq -r '.rules[0:10][]' | python3 "$RULES_PY" --ask 2>/dev/null) ||
			rules=""
		[ -n "$rules" ] || rules=$(printf '%s' "$c" | jq -r '.rules[0:10][] | "- `" + . + "`"')
		what="AN ARCHITECT'S SURFACE MOVED, UNLABELED: \`helpers/architect-label-rules.py\` fires on this diff at head \`$head\`, and it carries none of \`needs-architect\`, \`architect-ruled\` or \`held-by-lane\`. The labeler that should have caught it runs on push, and GitHub runs no \`pull_request\` workflow while a pull request conflicts with its base (#1745). What calef is asked, one question per finding:

$rules

So the drain added \`needs-architect\` (calef's ruling on #1792: the bot adds, and only flags removals). An answer is the ruling (\"nothing like that moved\" on a false positive included), and so is one calef already gave on this surface: record it with \`script/record-ruling $1\`. A lane that knows more than the rule does posts its own ask under a \`## What I need from you\` heading; a hold with no ask is flagged \`hold-no-ask\` after $NM_MINUTES minutes."
		;;
	missing-check)
		head=$(printf '%s' "$c" | jq -r '.head')
		absent=$(printf '%s' "$c" | jq -r '.absent | map("- `" + . + "`") | join("\n")')
		what="REQUIRED CHECK NEVER REPORTED at head \`$head\`. Every pull-request workflow run there has finished, and the \`main\` ruleset requires these, none of which has a check run at all:

$absent

The drain already reran the workflow that owns them once, or none owns them, so a rerun will not fix it and nothing is red for helpers/ci-failing.sh to report: an armed pull request with a check that does not exist waits forever (#1814, #1816 and #1812 on 2026-10-07). The maintainer session owns the next step: rerun that run once more (the Actions page, or \`gh run\` with its \`rerun\` subcommand), or push an empty commit to raise a fresh run, and if it recurs the job's \`if:\` or the workflow's event filter is the suspect. A new head, or the check reporting, takes the label off."
		;;
	stuck-head-requeue)
		head=$(printf '%s' "$c" | jq -r '.head')
		group=$(printf '%s' "$c" | jq -r '.group')
		position=$(printf '%s' "$c" | jq -r '.position')
		enqueued=$(printf '%s' "$c" | jq -r '.enqueued')
		completed=$(printf '%s' "$c" | jq -r '.completed')
		runlist=$(printf '%s' "$c" | jq -r '.runs | map("- \(.name) (run \(.id)): \(.conclusion), attempt \(.attempt), \(.url)") | join("\n")')
		what="STUCK AT POSITION $position OF THE MERGE QUEUE, AND RE-QUEUED. The entry was enqueued at $enqueued and still read \`AWAITING_CHECKS\`, yet every workflow run of its merge group (\`$group\`, head \`$head\`) had finished by $completed, more than 10 minutes before. GitHub merges or ejects a finished group within a minute, so a required check never posted and nothing would move it, with everything behind it waiting (#1824, 2026-10-07). The group's runs:

$runlist

${REQUEUE_RESULT:-The drain dequeued it and enqueued it again at the back, which builds a fresh group.} This is the drain's one action on the queue, a narrow exception to #1564 that calef ruled on #1833. If this pull request stalls again the drain does not re-queue it: it adds \`needs-maintainer\`."
		;;
	stuck-head)
		head=$(printf '%s' "$c" | jq -r '.head')
		group=$(printf '%s' "$c" | jq -r '.group')
		position=$(printf '%s' "$c" | jq -r '.position')
		enqueued=$(printf '%s' "$c" | jq -r '.enqueued')
		completed=$(printf '%s' "$c" | jq -r '.completed')
		id=$(printf '%s' "$c" | jq -r '.id')
		runlist=$(printf '%s' "$c" | jq -r '.runs | map("- \(.name) (run \(.id)): \(.conclusion), attempt \(.attempt), \(.url)") | join("\n")')
		what="STUCK AT POSITION $position OF THE MERGE QUEUE, A SECOND TIME. The entry was enqueued at $enqueued and still reads \`AWAITING_CHECKS\`, yet every workflow run of its merge group (\`$group\`, head \`$head\`) finished by $completed. The drain already re-queued this pull request once and does not do it twice, so a repeat is not a flake: suspect the pull request or a required check. The group's runs:

$runlist

The maintainer session owns the next step. To free the place now:

    gh api graphql -f query='mutation{dequeuePullRequest(input:{id:\"$id\"}){clientMutationId}}'

The label comes off when the entry leaves \`AWAITING_CHECKS\` or its group changes."
		;;
	unmergeable)
		head=$(printf '%s' "$c" | jq -r '.head')
		position=$(printf '%s' "$c" | jq -r '.position')
		enqueued=$(printf '%s' "$c" | jq -r '.enqueued')
		id=$(printf '%s' "$c" | jq -r '.id')
		aheadjson=$(printf '%s' "$c" | jq -c '.ahead')
		aheadlist=$(printf '%s' "$c" | jq -r 'if (.ahead | length) == 0 then "nothing" else (.ahead | map("#\(.number)") | join(", ")) end')
		pairs=$(ahead_conflicts "$head" "$aheadjson") || pairs=""
		[ -n "$pairs" ] || pairs="- (no entry ahead of it, so GitHub's verdict is against \`main\` alone; see the \`conflict\` cause)"
		what="UNMERGEABLE IN THE MERGE QUEUE at position $position, enqueued at $enqueued, at head \`$head\`. Ahead of it: $aheadlist. GitHub cannot build it on top of those entries, usually because it conflicts with one of them rather than with \`main\`, and it does not eject it: the entry waits, and nothing else reports it (#1795 behind #1745, 2026-10-07).

$pairs

The maintainer session owns the next step: hand it to its lane to merge \`main\` once the conflicting pull request ahead has landed (briefs/rebase-onto-main.md), push, and arm it again. To free its place now, take it out of the queue:

    gh api graphql -f query='mutation{dequeuePullRequest(input:{id:\"$id\"}){clientMutationId}}'

The label comes off when the entry no longer reads \`UNMERGEABLE\`."
		;;
	budget)
		opened=$(printf '%s' "$c" | jq -r '.opened')
		title=$(printf '%s' "$c" | jq -r '.title // "the tracking issue"')
		what="A CI JOB IS NEAR ITS BUDGET: \"$title\", opened $opened by \`helpers/ci_job_times.py sync\` because the job's merge-group runs passed about 85% of its budget in \`.github/ci-job-budgets\` (§254 (a gate prints what each item cost)). Nothing has failed yet; this is the weeks of warning a drift gives before milestone 721 (each merge-group CI job has a 20-minute budget)'s hard limit ejects pull requests. The issue body has the job's recent runs, its slowest steps and where to find its per-item record. The maintainer session owns the decision, one of:

- make something faster, or split the job, and let the issue close itself when the runs drop back under the line
- raise the budget with a reason line in \`.github/ci-job-budgets\`, which moves the line with it

The label comes off when the issue closes."
		;;
	stale)
		state=$(printf '%s' "$c" | jq -r '.state')
		merged=$(printf '%s' "$c" | jq -r '.merged // "an unrecorded time"')
		enqueued=$(printf '%s' "$c" | jq -r '.enqueued')
		entry=$(printf '%s' "$c" | jq -r '.entry')
		id=$(printf '%s' "$c" | jq -r '.id')
		what="STILL IN THE MERGE QUEUE, though it is \`$state\` (merged at $merged). The entry was enqueued at $enqueued and reads \`$entry\`. Remove it:

    gh api graphql -f query='mutation{dequeuePullRequest(input:{id:\"$id\"}){clientMutationId}}'

The label comes off when the entry is gone."
		;;
	*)
		what="needs a maintainer for a cause this script does not describe: \`$cause\`."
		;;
	esac
	printf '%s' "$ME: needs-maintainer. $what

The label is how a maintainer session finds this without anyone watching the queue (milestone 727, provisional; helpers/needs-maintainer.jq has the causes)."
}

needs_maintainer() {
	resp=$(gh api graphql -F owner="${REPO%/*}" -F name="${REPO#*/}" \
		-f labelled="repo:$REPO is:closed label:$NM_LABEL" \
		-f budget="repo:$REPO is:issue is:open label:$NM_BUDGET_LABEL" -f query="$NM_QUERY" 2>/dev/null) || resp=""
	if [ -z "$resp" ] || [ "$(printf '%s' "$resp" | jq -r '.data.repository != null' 2>/dev/null)" != "true" ]; then
		# Loud, because a detector that fails quietly is the drain's old shape. The run stays green
		# so a GitHub outage does not page anyone twice; the next pass asks again.
		echo "$ME: the needs-maintainer query failed; nothing was labelled or cleared this pass"
		return 0
	fi

	# A pull request's `Blocked-by:` pull requests, draft or ready (`stale-draft` honours them too),
	# resolved here because the jq file must not call `gh`: { "<number>": ["OPEN", "MERGED", ...] }.
	blockers='{}'
	for num in $(printf '%s' "$resp" | jq -r '.data.repository.pullRequests.nodes[]
			| select(.body // "" | test("(?i)blocked-by:")) | .number'); do
		body=$(printf '%s' "$resp" | jq -r --argjson n "$num" '.data.repository.pullRequests.nodes[] | select(.number == $n) | .body')
		states='[]'
		for b in $(nife_blocked_by "$body"); do
			s=$(gh pr view "$b" --repo "$REPO" --json state -q .state 2>/dev/null) || s=""
			# Unknown reads as OPEN: a pull request is then not called unarmed this pass, and the
			# next pass asks again.
			states=$(printf '%s' "$states" | jq -c --arg s "${s:-OPEN}" '. + [$s]')
		done
		blockers=$(printf '%s' "$blockers" | jq -c --arg n "$num" --argjson s "$states" '. + {($n): $s}')
	done

	# The architect-label rules on each head with no hold or ruling label, spliced into its node as
	# `architectSurface` (the `surface-no-hold` cause). A head this checkout lacks (a fork's), or a
	# diff the rules cannot read, is skipped here: architect-label.yml labels the unexamined case on
	# push, and this pass is its backstop, not its replacement.
	#
	# **`origin/main` is fetched here, after the query, never trusted from the checkout.** In the
	# workflow it is GITHUB_SHA, main as of the trigger: actions/checkout fetches every branch, then
	# force-rewinds `origin/main` to that sha, and a run queued behind the `merge-drain` group starts
	# minutes later. A head cut from a newer main then has the stale tip as its merge base, so main's
	# own commits read as the pull request's: #2334's empty claim commit was held for
	# `crates/capability_manifest_note`, which #2316 had changed on main (run 39110360811). Fetched
	# after the query, so every head it lists was cut from a main no newer than this one. A failed
	# fetch skips the scan, since diffing against a tip that may be stale is the bug itself.
	surface='{}'
	rows=''
	if git fetch --quiet --no-tags origin +refs/heads/main:refs/remotes/origin/main 2>/dev/null; then
		rows=$(printf '%s' "$resp" | jq -r '.data.repository.pullRequests.nodes[]
			| select([.labels.nodes[].name] | (index("needs-architect") or index("architect-ruled") or index("held-by-lane")) | not)
			| "\(.number):\(.headRefOid)"')
	else
		echo "$ME: could not refresh origin/main, so the architect-surface scan is skipped this pass"
	fi
	# The open pull requests whose ruling stands (`architect-ruled`, no `held-by-lane`): a file a
	# head carries byte for byte from one of them is not asked again (#3450, which carried #3434's
	# ruled §130 (how the userspace clock service reads the x86 CMOS RTC) edit).
	# Every merge base is read, not one (`--against`; #3416 and #3167 had two).
	ruled=$(printf '%s' "$resp" | jq -r '.data.repository.pullRequests.nodes[]
		| select([.labels.nodes[].name] | (index("architect-ruled") and (index("held-by-lane") | not)))
		| "\(.number):\(.headRefOid)"')
	for row in $rows; do
		num=${row%%:*}
		head=${row#*:}
		git cat-file -e "$head^{commit}" 2>/dev/null || continue
		set --
		for pair in $ruled; do
			set -- "$@" --ruled "$pair"
		done
		rc=0
		out=$(python3 "$RULES_PY" --against origin/main "$head" "$@" 2>/dev/null) || rc=$?
		[ "$rc" = 0 ] && [ -n "$out" ] || continue
		surface=$(printf '%s' "$surface" | jq -c --arg n "$num" --arg o "$out" '. + {($n): ($o | split("\n") | map(select(. != "")))}')
	done
	resp=$(printf '%s' "$resp" | jq -c --argjson s "$surface" \
		'.data.repository.pullRequests.nodes |= map(. + (if $s[.number | tostring] then {architectSurface: $s[.number | tostring]} else {} end))')

	# Required checks that never reported on an armed head (the `missing-check` cause): rerun the
	# owning workflow once, and splice each verdict into its node as `missingChecks`.
	missing_check_scan "$resp"
	resp=$(printf '%s' "$resp" | jq -c --argjson m "$MC_MAP" \
		'.data.repository.pullRequests.nodes |= map(. + (if $m[.number | tostring] then {missingChecks: $m[.number | tostring]} else {} end))')
	# Each AWAITING_CHECKS entry's merge-group runs, summarized by helpers/stuck-head.jq and spliced
	# into the entry as `stuckHead` (the `stuck-head` cause). One API call per such entry; the age
	# test is the decision's.
	stuck='{}'
	for row in $(printf '%s' "$resp" | jq -r '.data.repository.mergeQueue.entries.nodes[]?
			| select(.state == "AWAITING_CHECKS" and .headCommit.oid != null) | "\(.position):\(.headCommit.oid)"'); do
		pos=${row%%:*}
		group=${row#*:}
		sh_=$(gh api "repos/$REPO/actions/runs?head_sha=$group&event=merge_group&per_page=100" 2>/dev/null |
			jq -c -f "$STUCK_JQ" 2>/dev/null) || sh_=""
		[ -n "$sh_" ] || continue
		stuck=$(printf '%s' "$stuck" | jq -c --arg p "$pos" --argjson v "$sh_" '. + {($p): $v}')
	done
	resp=$(printf '%s' "$resp" | jq -c --argjson s "$stuck" \
		'.data.repository.mergeQueue.entries.nodes |= map(. + (if $s[.position | tostring] then {stuckHead: $s[.position | tostring]} else {} end))')

	printf '%s' "$resp" |
		jq -c --arg l "$NM_LABEL" --argjson now "$(date +%s)" --argjson m "$NM_MINUTES" --argjson b "$blockers" \
			"$(cat "$ELIGIBLE_JQ" "$OQ_JQ" "$NM_JQ")"'nm_decide($l; $now; $m; $b)' |
		while IFS= read -r rec; do
			num=$(printf '%s' "$rec" | jq -r '.number')
			# `gh pr` or `gh issue`: the two architect causes also apply to issues.
			NM_KIND=$(printf '%s' "$rec" | jq -r '.kind // "pr"')
			action=$(printf '%s' "$rec" | jq -r '.action')
			causes=$(printf '%s' "$rec" | jq -r '.causes | map(.cause) | join(", ")')
			# calef, #1792, 2026-10-06 (UTC): the drain adds `needs-architect` itself for an open
			# question or a moved surface, and only flags the reverse (hold-no-ask), because a wrong
			# removal could merge a pull request without his ruling. Never remove it here.
			if [ "$(printf '%s' "$rec" | jq -r '.hold')" = true ]; then
				if w gh "$NM_KIND" edit "$num" --repo "$REPO" --add-label "$HELD_LABEL"; then
					echo "$ME: HELD #$num for an architect ($causes)"
				else
					echo "$ME: #$num needs $HELD_LABEL ($causes) and could not be labeled"
				fi
			fi
			case "$action" in
			hold) ;;
			requeue)
				# calef's #1833 exception: see requeue_stuck_head. The comment follows below.
				qid=$(printf '%s' "$rec" | jq -r '.causes[] | select(.cause == "stuck-head-requeue") | .id')
				if requeue_stuck_head "$num" "$qid"; then
					REQUEUE_RESULT=""
					echo "$ME: REQUEUED #$num (its merge group finished and the queue never acted)"
				else
					REQUEUE_RESULT="The drain tried to dequeue and enqueue it again and could not, so a maintainer must."
					echo "$ME: #$num is stuck at the head and could not be re-queued"
				fi
				;;
			adopt)
				branch=$(printf '%s' "$rec" | jq -r '.branch')
				head=$(printf '%s' "$rec" | jq -r '.causes[0].head')
				if [ -n "$dry" ]; then
					echo "$ME: (dry run) would open a draft for orphan branch $branch at $head and label it $NM_LABEL"
					continue
				fi
				url=$(gh pr create --repo "$REPO" --draft --base main --head "$branch" \
					--title "orphan: $branch" --body "$ME: opened as a draft because \`$branch\` holds work that never had a pull request (the \`orphan\` cause, helpers/needs-maintainer.jq). Nothing on it was changed.

<!-- needs-maintainer:orphan adopted $head -->" 2>/dev/null) || url=""
				num=${url##*/}
				case "$num" in
				''|*[!0-9]*)
					echo "$ME: $branch is orphan work and a draft could not be opened for it"
					continue
					;;
				esac
				w gh pr edit "$num" --repo "$REPO" --add-label "$NM_LABEL" || true
				echo "$ME: ADOPTED $branch as #$num (orphan)"
				;;
			clear)
				if w gh "$NM_KIND" edit "$num" --repo "$REPO" --remove-label "$NM_LABEL"; then
					echo "$ME: CLEARED #$num (its cause is gone)"
				fi
				continue
				;;
			label)
				if w gh "$NM_KIND" edit "$num" --repo "$REPO" --add-label "$NM_LABEL"; then
					echo "$ME: LABELLED #$num ($causes)"
				else
					echo "$ME: #$num needs a maintainer ($causes) and could not be labelled $NM_LABEL; does the label exist?"
				fi
				;;
			esac
			printf '%s' "$rec" | jq -c '.causes[]' | while IFS= read -r c; do
				marker="needs-maintainer:$(printf '%s' "$c" | jq -r '"\(.cause):\(.key)"')"
				notify "$num" "$marker" "$(nm_comment "$num" "$c")" || true
			done
		done
}

# # The log's event lines
#
# A snapshot answers "what is true now" and an event answers "what happened", and only events can
# be counted across passes (the correction is in `script/metrics` and notes/project-metrics.md).
# Each of these prints once per transition, never once per pass:
#
#     merge-drain[...]: DEQUEUED #N ...    a held pull request was taken out of the queue
#     merge-drain[...]: RERAN #N run <id>  a cancelled same-second duplicate was rerun, once
#     merge-drain[...]: REQUEUED #N ...    a stuck queue head was dequeued and enqueued again, once
#     merge-drain[...]: RERAN #N run <id>  (again) a workflow was rerun, once, for a required check never created
#     merge-drain[...]: UNBLOCKED #N ...   a paused draft's blockers resolved
#     merge-drain[...]: LABELLED #N ...    needs-maintainer added, with its causes
#     merge-drain[...]: CLEARED #N ...     needs-maintainer removed, its cause gone
#     merge-drain[...]: ADOPTED <branch> as #N   a draft opened for an orphan branch
pass() {
	# A pushed lane branch with no pull request is invisible to everything that starts from
	# `gh pr list`. Milestone 204 (a pushed lane branch with no draft pull request is a claim nobody
	# can see); the script owns its own grace period.
	sh helpers/lane-claim-check.sh || true
	dequeue_held || true
	unblocked_drafts || true
	release_unblocked_labels || true
	rerun_cancelled_duplicates || true
	needs_maintainer || true
}

pass
