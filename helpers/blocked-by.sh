# helpers/blocked-by.sh: parse a pull request body's `Blocked-by:` line into pull request numbers.
#
# There is no shebang because this file is sourced, not run, the same reason
# helpers/branch-name-check.sh has none: it exists to be read into another script's shell rather
# than spawn its own.
# shellcheck shell=sh
#
# The convention: a pull request body carries one line naming what it is sequenced behind,
# `Blocked-by: #N` or `Blocked-by: #N, #M, ...`. Two consumers read it. `helpers/merge-drain.sh`'s
# admission hold (partsfoundry/record:notes/merge-queue.md, "`Blocked-by: #N`: a hold that releases itself",
# 2026-08-18) holds a non-draft pull request out of the queue until every listed number merges or
# is reported closed. The draft-unblock pass (2026-09-27, calef's fix for #1289: it was paused
# waiting on #1288, #1288 merged an hour later, and nobody resumed #1289 for two days because a
# pause recorded only in prose has no mechanism behind it) labels a paused DRAFT `unblocked` and
# comments once the same condition holds.
#
# Both need every number on the line, not only the first: a pull request sequenced behind two
# others could previously say so only once (partsfoundry/record:notes/merge-queue.md's BUGS section recorded this as an
# accepted limitation). Pulling the parser into its own file, sourced by both consumers and by a
# selftest that touches no `gh` call, is what makes fixing that a one-place change instead of two.
#
# Name: provisional, minted 2026-09-27 pulling `blocked_by` out of merge-drain.sh. `nife_` prefixed
# in the family of `nife_check_branch_name_shape`, the other sourced-not-run helper whose function
# needed a namespace because it is read into a caller's own shell rather than called through a
# subprocess boundary.

# nife_blocked_by <body>
#
# Prints one pull request number per line, in the order written, read from the first line
# containing "Blocked-by:" (case-insensitive on the key, matching a person's typing rather than a
# regex's idea of capitalisation). Prints nothing if no such line exists.
#
# Only the first matching LINE is read. A second "Blocked-by:" lower in the body is not
# accumulated, so every number a pull request is sequenced behind belongs on the one line:
# `Blocked-by: #1347, #1354`, or `Blocked-by: #1347 #1354`, either separator works because the
# numbers are found with `grep -o` rather than split on a comma.
nife_blocked_by() {
	line=$(printf '%s' "$1" | grep -i -m1 'blocked-by:') || return 0
	[ -n "$line" ] || return 0
	printf '%s\n' "$line" | grep -o '#[0-9][0-9]*' | tr -d '#'
}
