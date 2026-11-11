#!/bin/sh
#
# Hold the merge queue while `main` is red, and give it back afterwards.
#
#     helpers/queue-hold.sh hold 1168        # hold everything except #1168, which is the fix
#     helpers/queue-hold.sh hold 1168 -n     # say what that would do, change nothing
#     helpers/queue-hold.sh status           # what is held right now
#     helpers/queue-hold.sh release          # re-arm auto-merge on everything held
#
# PROVISIONAL NAME. Minted 2026-09-23 by this lane; not put to calef. See the `Name:` block below.
#
# # Why this exists
#
# `helpers/trunk-health.sh` says `main` is red. `helpers/merge-drain.sh` landed what did not need
# calef (until 2026-10-03; it now only labels and dequeues). Nothing implemented the response in between, and calef named the gap on 2026-09-23: *"Is
# there a missing mechanism for fixing when main goes red? It seems like the fix is enqueuing the one
# fix and holding everything else until that lands, then re-enabling everything else."*
#
# He is right, and the same evening a maintainer performed that procedure by hand, got it wrong
# twice, and left the held set in a chat message. The judgement (is this a real trunk failure, which
# pull request is the fix) is in briefs/main-is-red.md and stays a person's; this script is the
# mechanical half, which is the half that was got wrong.
#
# # The record is a label, not this script's memory
#
# The held set has to survive the session that made it, so it lives where `needs-architect` already
# lives: on the pull requests. The held queue is one command,
#
#     gh pr list --label held-for-red-trunk
#
# rather than something somebody has to have read. That is rung three of AGENTS.md's ladder (a
# written record at the thing itself), and rungs one and two are deliberately not attempted: no gate
# can tell a pull request that should be held from one that should not, and a watcher that acted on
# its own would be resolving without judgement, which notes/merge-queue.md argues against on purpose.
#
# **Nothing else is remembered, and that is a design choice rather than an omission.** The obvious
# alternative is to record each pull request's auto-merge state at hold time and restore exactly
# that. It cannot be done: GitHub clears `autoMergeRequest` the moment a pull request enters the
# queue (notes/merge-queue.md records that trap under #965), so "armed and queued" and "never armed"
# read identically. Release therefore re-arms every held pull request under helpers/queue-eligible.jq
# and not `needs-architect`, which is what the drain applied every five minutes until it stopped
# arming on 2026-10-03. That is why `needs-architect` pull requests are skipped by `hold` as well as
# by `release`: holding one would be a no-op that release could only undo by arming something no
# eligible policy ever would.
#
# # The four steps, each one here because the obvious version of it failed (2026-09-23)
#
#   1. **Disable auto-merge first.** Dequeuing alone does not stick: with auto-merge still armed the
#      pull request re-enqueues itself within minutes. The maintainer drained the queue that evening,
#      reported it done, and found all seven back.
#   2. **Then dequeue.** `gh pr merge --disable-auto` does not remove an entry that is already in the
#      queue; only the GraphQL mutation does, and its input field is `id`, not `pullRequestId`.
#   3. **Then cancel the orphaned group builds.** GitHub leaves `gh-readonly-queue/...` runs
#      executing for entries that no longer exist. Their results cannot be consumed by anything and
#      they starve the fix of runners; 55 were cancelled by hand that evening.
#   4. **Land the fix alone**, then `release`.
#
# # BUGS
#
#   - **It cannot tell a group build that contains the fix from one that does not.** A merge-queue
#     branch is named `gh-readonly-queue/main/pr-<N>-<sha>` after the *last* entry in the group, so a
#     group holding the fix behind two other entries looks like any other orphan. The run whose name
#     carries the fix's own number is skipped; the rest are cancelled. **So hold before the fix is
#     enqueued**, which is the order briefs/main-is-red.md gives anyway. If the fix is evicted this
#     way, re-enqueue it; nothing is lost but a CI round trip.
#   - **`release` re-arms rather than restores.** See above: the pre-hold state is not recoverable
#     from the API, so a pull request that was deliberately left unarmed before the hold comes back
#     armed. Until 2026-10-03 the drain would have armed it on its next pass regardless; since then
#     a lane that left its pull request unarmed on purpose should make it a draft before a hold.
#   - **It does not check whether `main` is actually red.** Deliberate: the judgement is the brief's,
#     the exempt pull request is an argument, and a script that second-guessed either would be
#     resolving. `status` prints `helpers/trunk-health.sh --once` beside the held set so a reader
#     sees both facts together, and that is as far as it goes.
#   - **Nothing expires a hold.** A session that dies mid-hold leaves labelled pull requests that
#     nothing will release; the recovery list is in briefs/main-is-red.md, and it is one `release`.
#     An unreleased hold is visible (the label) but nothing announces it.
#   - **The drain used to undo a hold.** Until 2026-09-23 it re-enqueued everything held here, three
#     times in one evening, invisibly (a dequeue leaves no trace of why an entry returned). Since
#     2026-10-03 it arms nothing, and its `dequeue_held` takes a held pull request back out of the
#     queue, so it now enforces the hold rather than fighting it.
#   - **`gh pr list --label` reads a search index that lags the label write.** In the 2026-09-23
#     rehearsal a `release --dry-run` run seconds after a label was added listed one held pull
#     request where the real `release` a moment later found two. So `status` immediately after
#     `hold` can under-report; give it a few seconds, and trust the labels on the pull requests
#     themselves over this listing when the two disagree.
#   - **`status` always reports the real repository's trunk**, even under `QUEUE_HOLD_REPO`:
#     `helpers/trunk-health.sh` takes no such override. That is harmless (a rehearsal repository has
#     no trunk anybody cares about) and confusing enough to be worth saying once.
#   - **It holds what is open when it runs.** A pull request opened or marked ready *during* the hold
#     is not labelled, and a lane that arms it puts it into a red trunk. Re-running `hold`
#     is idempotent and sweeps the new arrivals; nothing does that automatically.
#
# Name: unrecorded. Provisional, minted 2026-09-23 by this lane. `queue-hold` for what it does to the
# queue rather than for the mechanism, the family `merge-drain.sh` named itself into. It lives in
# `helpers/` rather than `script/` for the reason `merge-drain.sh` gives for itself: a maintainer's
# tool, not a front door a contributor types, so it carries no notes/scripts.md row (`script/lint`'s
# script-docs check only walks `script/`). The label `held-for-red-trunk` is provisional too, and
# says `trunk` rather than `main` for `trunk-health.sh`'s reason: the branch could be renamed and the
# concept could not. See notes/main-is-red.md.

set -e
cd "$(dirname "$0")/.."

# Overridable for one reason, and it is the reason a drill exists: the mutating path cannot be
# rehearsed against this repository without touching real pull requests and real group builds.
# `QUEUE_HOLD_REPO=<scratch repo> helpers/queue-hold.sh hold 1` runs the whole thing end to end
# somewhere nothing is lost. Not a knob for ordinary use; see notes/main-is-red.md for the rehearsal.
REPO="${QUEUE_HOLD_REPO:-nifeos/nife}"
HELD_LABEL="held-for-red-trunk"
ARCHITECT_LABEL="needs-architect"
LABEL_COLOR="b60205"
# GitHub caps a label description at 100 characters and answers a longer one with an HTTP 422. The
# first rehearsal of this script sent 110, the create failed, and because the failure was swallowed
# every `--add-label` after it failed too and the hold recorded nothing while reporting success.
# That is why `ensure_label` below aborts rather than continuing: without the label there is no
# record, and a hold with no record is the chat message this script exists to replace.
LABEL_DESC="Held while \`main\` is red; do not enqueue. helpers/queue-hold.sh release clears it."

usage() {
	echo "usage: $(basename "$0") hold <fix-pr-number> [-n|--dry-run]" >&2
	echo "       $(basename "$0") release [-n|--dry-run]" >&2
	echo "       $(basename "$0") status" >&2
	exit 2
}

dry=""
cmd="${1:-}"
[ -n "$cmd" ] || usage
shift || true

fix=""
for arg in "$@"; do
	case "$arg" in
	-n | --dry-run) dry=1 ;;
	[0-9]*) fix="$arg" ;;
	*) usage ;;
	esac
done

say() { printf '%s: %s\n' "queue-hold" "$1"; }

# Every mutation goes through this, so `--dry-run` cannot miss one by being remembered at each call
# site. It names what it is about to change whether or not it changes it, which is the reporting this
# script owes: a hold that says "7 held" and not which seven is the chat message this replaces.
#
# **The command is silenced HERE rather than at the call sites**, and the first dry run of this
# script is why: `do_or_say "..." gh pr merge ... >/dev/null 2>&1` sends the *narration* to
# /dev/null along with the command, so a dry run that decided to change seven pull requests printed
# three lines and named none of them. A wrapper that reports has to own the redirection.
do_or_say() {
	what="$1"
	shift
	if [ -n "$dry" ]; then
		say "would $what"
		return 0
	fi
	say "$what"
	"$@" >/dev/null 2>&1 || say "  ...that call did not take ($what); carrying on"
}

# The label is created with `--force` rather than created-if-missing so that editing the description
# in this file is enough to update it on GitHub. Creating is idempotent that way, which is most of
# what makes `hold` safe to run twice.
ensure_label() {
	if [ -n "$dry" ]; then
		say "would ensure the $HELD_LABEL label exists"
		return 0
	fi
	if ! err=$(gh label create "$HELD_LABEL" --repo "$REPO" --color "$LABEL_COLOR" \
		--description "$LABEL_DESC" --force 2>&1); then
		say "cannot create or update the $HELD_LABEL label, so nothing would be recorded:"
		printf '%s\n' "$err" >&2
		exit 1
	fi
}

# Open, ready, into `main`, not already needing an architect. The same admission set
# `merge-drain.sh`
# computes, and for the same reasons: a draft is not asking to be merged, a stacked pull request
# targets a branch with no queue, and `needs-architect` is a hold this one must not overwrite.
#
# The first three of those tests are helpers/queue-eligible.jq, shared with merge-drain.sh since
# the 2026-09-24 security audit found both copies admitting a fork's pull request; see that file.
ELIGIBLE_JQ="$(dirname "$0")/queue-eligible.jq"
holdable() {
	gh pr list --repo "$REPO" --state open --limit 200 \
		--json number,isDraft,baseRefName,labels,title,isCrossRepository 2>/dev/null |
		jq -r --arg A "$ARCHITECT_LABEL" --arg H "$HELD_LABEL" --arg F "${fix:-0}" "$(cat "$ELIGIBLE_JQ")"'
			[ .[]
			  | eligible
			  | select((.number | tostring) != $F)
			  | select((.labels | map(.name) | index($A)) | not)
			  | select((.labels | map(.name) | index($H)) | not) ]
			| sort_by(.number)[] | "\(.number)\t\(.title)"' 2>/dev/null
}

held() {
	gh pr list --repo "$REPO" --state open --limit 200 --label "$HELD_LABEL" \
		--json number,title,isDraft 2>/dev/null |
		jq -r 'sort_by(.number)[] | "\(.number)\t\(.title)"' 2>/dev/null
}

# Prints "<node id> <count of removed_from_merge_queue events>" for a pull request, by GraphQL.
removal_state() {
	gh api graphql -f query='query($o:String!,$n:String!,$num:Int!){repository(owner:$o,name:$n){pullRequest(number:$num){id timelineItems(last:100,itemTypes:[REMOVED_FROM_MERGE_QUEUE_EVENT]){nodes{__typename}}}}}' \
		-f o="${REPO%%/*}" -f n="${REPO#*/}" -F num="$1" \
		--jq '.data.repository.pullRequest | "\(.id) \(.timelineItems.nodes | length)"' 2>/dev/null || echo " 0"
}

# Borrowed whole from `merge-drain.sh`'s `dequeue_held`, including the before/after timeline count:
# `dequeuePullRequest` is a no-op on a pull request that is not queued and reports success either
# way, so counting removed-from-merge-queue timeline events (GraphQL) is the only way to say truthfully whether
# anything moved. The input field is `id`; `pullRequestId` is rejected, which cost an attempt.
dequeue() {
	num="$1"
	st=$(removal_state "$num") || return 0
	id=${st% *} before=${st#* }
	[ -n "$id" ] || return 0
	gh api graphql -f query="mutation{dequeuePullRequest(input:{id:\"$id\"}){clientMutationId}}" \
		>/dev/null 2>&1 || return 0
	st=$(removal_state "$num") && after=${st#* } || after=$before
	[ "$after" -gt "$before" ] && say "dequeued #$num"
	return 0
}

# Step 3. See BUGS on why the fix's own number is the only exemption available here.
cancel_orphaned_group_builds() {
	runs=$(gh run list --repo "$REPO" --limit 100 \
		--json databaseId,headBranch,status 2>/dev/null |
		jq -r --arg F "${fix:-0}" '
			.[]
			| select(.headBranch | startswith("gh-readonly-queue/"))
			| select(.status != "completed")
			| select((.headBranch | contains("pr-" + $F + "-")) | not)
			| "\(.databaseId)\t\(.headBranch)"' 2>/dev/null)
	[ -n "$runs" ] || { say "no orphaned group builds running"; return 0; }
	printf '%s\n' "$runs" | while IFS="$(printf '\t')" read -r id branch; do
		[ -n "$id" ] || continue
		do_or_say "cancel orphaned group build $id ($branch)" \
			gh run cancel "$id" --repo "$REPO"
	done
}

case "$cmd" in
hold)
	[ -n "$fix" ] || usage
	ensure_label
	say "holding everything except #$fix, which is the fix"
	list=$(holdable)
	if [ -z "$list" ]; then
		say "nothing left to hold (already held, or nothing open that the drain would arm)"
	else
		printf '%s\n' "$list" | while IFS="$(printf '\t')" read -r num title; do
			[ -n "$num" ] || continue
			# Order matters and is the first failure this script exists to prevent: an armed pull
			# request re-enqueues itself within minutes of being dequeued.
			do_or_say "disable auto-merge on #$num: $title" \
				gh pr merge "$num" --repo "$REPO" --disable-auto
			[ -z "$dry" ] && dequeue "$num"
			do_or_say "label #$num $HELD_LABEL" \
				gh pr edit "$num" --repo "$REPO" --add-label "$HELD_LABEL"
		done
	fi
	cancel_orphaned_group_builds
	say "held set is now: gh pr list --repo $REPO --label $HELD_LABEL"
	say "now land #$fix alone, then: $(basename "$0") release"
	;;
release)
	list=$(held)
	if [ -z "$list" ]; then
		say "nothing is held"
		exit 0
	fi
	# Failures are collected and reported at the end rather than aborting the pass: one conflicted
	# pull request must not leave the other six held, which is the shape `merge-drain.sh` already
	# uses ("stops rather than guessing, per pull request rather than per pass").
	printf '%s\n' "$list" | while IFS="$(printf '\t')" read -r num title; do
		[ -n "$num" ] || continue
		if [ -n "$dry" ]; then
			say "would re-arm auto-merge on #$num and unlabel it: $title"
			continue
		fi
		if gh pr merge "$num" --repo "$REPO" --auto --merge >/dev/null 2>&1; then
			say "re-armed #$num: $title"
			gh pr edit "$num" --repo "$REPO" --remove-label "$HELD_LABEL" >/dev/null 2>&1 || true
		else
			# Left labelled on purpose. The label is the record, and a pull request this could not
			# re-arm is exactly the one a person must still look at; unlabelling it would hide it.
			say "COULD NOT re-arm #$num (conflict, failing check, or closed): $title"
			say "  it keeps the $HELD_LABEL label until a person clears it"
		fi
	done
	say "release pass done; anything still labelled needs a person: gh pr list --label $HELD_LABEL"
	;;
status)
	helpers/trunk-health.sh --once || true
	list=$(held)
	if [ -z "$list" ]; then
		say "nothing is held"
	else
		printf '%s\n' "$list" | while IFS="$(printf '\t')" read -r num title; do
			say "held: #$num $title"
		done
		say "release with: $(basename "$0") release"
	fi
	;;
*) usage ;;
esac
