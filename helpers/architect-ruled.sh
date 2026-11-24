#!/bin/sh
# shellcheck shell=sh
#
# helpers/architect-ruled.sh: exit 0 if a pull request carries `architect-ruled` or `held-by-lane`
# RIGHT NOW, printing which one, 1 if it carries neither, 2 if the labels could not be read.
#
#     helpers/architect-ruled.sh <pr-number> <owner/repo>
#
# Reads the LIVE label list over the API, never the event payload. The payload is a snapshot taken
# when the event fired; a run that queues behind a push sees the labels as they were then. #1712,
# 2026-10-05: calef applied `architect-ruled` at 19:24:14Z, a run queued by the earlier push read
# its payload (no `architect-ruled`) and added `needs-architect` at 19:24:35Z, and the armed pull
# request stalled on the hold check with both labels on it.
#
# Scope of the label, as recorded (nothing older records one): it sticks until a person removes it.
# A later push that adds a decision the ruling did not cover is NOT re-flagged by the labeler. The
# maintainer who sees such a push removes `architect-ruled`. Foot gun, chosen on purpose: the
# alternative (clearing on every synchronize) is the loop #1378 and #1400 showed, since a rebase
# re-presents the whole diff.
#
# Why three readers: the labelers skip adding (live read, two asks in architect-label.yml because the
# diff takes time); architect-hold.yml passes on the label even with needs-architect also on, since a
# read-then-write race cannot be fully closed and a person can add needs-architect by hand.
# `needs-architect` means "waiting on calef" and nothing else: applying `architect-ruled` removes it
# (architect-ruled-clears-hold.yml, 2026-10-06), so the label pair never shows on the queue.
# `held-by-lane` counts too (2026-10-06): it marks a send-back, a ruling that asked the lane for a
# change, and the labelers must not put `needs-architect` back on while the lane does that work.
# Not in partsfoundry/record:notes/merge-queue.md: that note is over its §212 (a prose budget) cap and may not grow.
# The writer is script/record-ruling, the one way to record a ruling (lane/architect-queue,
# 2026-10-06): it posts the ruling and makes this swap in one edit.
# Callers: architect-label.yml and coe-architect-label.yml (do not add), architect-hold.yml (pass).
# Selftest: helpers/architect-ruled-selftest.sh. Name: provisional, 2026-10-05.

set -u
pr="$1"
repo="$2"
labels="$(gh api "repos/$repo/issues/$pr" --jq '.labels[].name' 2>/dev/null)" || {
	echo "architect-ruled: could not read the labels of PR #$pr" >&2
	exit 2
}
for l in architect-ruled held-by-lane; do
	if printf '%s\n' "$labels" | grep -qx "$l"; then
		echo "$l"
		exit 0
	fi
done
exit 1
