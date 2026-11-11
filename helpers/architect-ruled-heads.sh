#!/bin/sh
# shellcheck shell=sh
#
# helpers/architect-ruled-heads.sh: print `<number>:<head sha>`, one per line, for each open pull
# request that carries `architect-ruled` and not `held-by-lane`, except the one named.
#
#     helpers/architect-ruled-heads.sh <owner/repo> [<pull request to leave out>]
#
# helpers/architect-label-rules.py takes each line as `--ruled N:SHA`: a file another branch
# carries byte for byte from one of these pull requests, at the same base, is a change calef
# already ruled on, so the labeler does not ask him again (#3450 carried #3434's ruled §130 (how
# the kernel-resident CMOS RTC reaches the userspace clock service) edit). `held-by-lane` is left
# out because a send-back means the lane still owes a change to what was ruled. A failed listing
# prints nothing, which leaves every finding standing: the safe side.
#
# Name: provisional. Hyphenated per design/naming.md's `helpers/` entry-point rule; calef has not
# ratified it.

set -eu

repo="$1"
self="${2:-}"

gh pr list --repo "$repo" --state open --label architect-ruled --limit 200 \
	--json number,headRefOid,labels 2>/dev/null |
	jq -r --arg self "$self" '.[]
		| select((.number | tostring) != $self)
		| select([.labels[].name] | index("held-by-lane") | not)
		| "\(.number):\(.headRefOid)"' 2>/dev/null || true
