#!/bin/sh
#
# script/lint-fleet's check 4b: a `milestone/N-*` branch must name a milestone that has a roadmap
# block in the design record, per §90 (the claim is a draft pull request; the status flip is a gate).
#
#     helpers/milestone-branch-block.sh <repo-root> <branch>
#
# Exits 1 when the branch names a milestone that has no block and the lane was not renumbered.
# Exits 0 otherwise.
#
# **The record is in nifeos/record** (milestone 1018 (the design record moves to nifeos/record and
# is published as a website)). The blocks are read through helpers/record_root.py: this tree's own
# while it holds the record, else the record at the commit basalt pins. A lane's status flip lands
# in its pull request in nifeos/record, which this tree's diff cannot see, so the warning for a
# branch that changes nothing in its block is that repository's to give. A block raised after the
# pin is not seen here until basalt's pin moves past it. script/lint-fleet has the history of the check; this file holds the logic so that
# helpers/milestone-branch-block-selftest.sh can plant cases against it.
#
# **A renumbered lane keeps its branch** (2026-10-10 UTC, #1901). A lane's milestone number is
# provisional until merge, so a collision moves its block, file, commits and pull request title to a
# new number. Its branch cannot follow: renaming the head branch of an open pull request closes the
# pull request. #1901 was cut as `milestone/868-...`, renumbered to 870 and then 871, and kept the
# 868 branch. So a branch whose number has no block passes when a block records where the lane
# went: one whose slug equals the branch's slug, or whose text names the branch verbatim
# (notes/skills/developer-lane/SKILL.md, shared state).
#
# Name: provisional, 2026-10-10 (UTC), describing what it checks. calef has not ruled on it.

set -eu
repo=$1
branch=$2

case "$branch" in
milestone/*) ;;
*) exit 0 ;;
esac
rest="${branch#milestone/}"
milestone_n="${rest%%-*}"
slug="${rest#*-}"
case "$milestone_n" in
'' | *[!0-9]*) exit 0 ;; # no leading number: check 4's shape rule owns that
esac

# The block's filename pads the number to four digits (calef, 2026-10-07 UTC); the branch name does
# not, so `milestone/7-x` and `milestone/0007-x` both find 0007-*.md.
file_n="$(printf '%04d' "$(expr "$milestone_n" + 0)")"
record="$(python3 "$(dirname "$0")/record_root.py" --top "$repo")"
roadmap_block="$(ls "$record/design/roadmap/$file_n-"*.md 2>/dev/null | head -1 || true)"
[ -n "$roadmap_block" ] && exit 0

# The renumbered lane's block, if one records it.
for f in "$record"/design/roadmap/[0-9]*-"$slug".md; do
    if [ -f "$f" ]; then
        echo "==> 4b: branch '$branch' was renumbered; its block is ${f#"$record"/design/}"
        exit 0
    fi
done
moved_to="$(grep -lF -- "$branch" "$record"/design/roadmap/[0-9]*.md 2>/dev/null | head -1 || true)"
if [ -n "$moved_to" ]; then
    echo "==> 4b: branch '$branch' was renumbered; its block is ${moved_to#"$record"/design/}"
    exit 0
fi

echo "lint: branch '$branch' names milestone $milestone_n, which has no roadmap block." >&2
echo "  The blocks are read from the design record at the commit basalt pins; a block raised" >&2
echo "  after that pin is seen once basalt's pin moves past it." >&2
echo "  If this lane was renumbered, keep the branch name (renaming the branch closes its pull" >&2
echo "  request) and let the moved block record it: the same slug as the branch, or the" >&2
echo "  branch name written in the block (notes/skills/developer-lane/SKILL.md, shared state)." >&2
exit 1
