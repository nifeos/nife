# helpers/hold-labels.sh: the labels that hold a pull request out of the merge queue. Sourced, not
# run. POSIX sh. Provisional name (lane/push-requeue, 2026-11-04 UTC).
#
# One definition for the two things that must agree: helpers/merge-drain.sh dequeues a pull request
# wearing one of these, and helpers/arm.sh refuses to arm one. Before this file each held its own
# copy of the strings, so a fourth hold added to one would still be armed by the other. The jq side
# (`nm_held` in helpers/needs-maintainer.jq) cannot source a shell file; helpers/arm-selftest.sh
# checks that its list names the same three.
#
#     HELD_LABEL        needs-architect: a person must decide something.
#     LANE_HELD_LABEL   held-by-lane: calef has ruled and the lane owes a change. Its own label, so
#                       `needs-architect` stays calef's worklist; architect-hold.yml fails on it.
#     RED_TRUNK_LABEL   held-for-red-trunk: a hold on the whole queue while `main` is broken, placed
#                       by helpers/queue-hold.sh (notes/main-is-red.md).
#
# `hold_label_of` reads label names on stdin, one per line, and prints the first hold among them (in
# the order above) or nothing. Exit 0 when it printed one.

HELD_LABEL="needs-architect"
LANE_HELD_LABEL="held-by-lane"
RED_TRUNK_LABEL="held-for-red-trunk"

hold_label_of() {
	names="$(cat)"
	for hold_candidate in "$HELD_LABEL" "$LANE_HELD_LABEL" "$RED_TRUNK_LABEL"; do
		if printf '%s\n' "$names" | grep -qxF -- "$hold_candidate"; then
			printf '%s\n' "$hold_candidate"
			return 0
		fi
	done
	return 1
}
