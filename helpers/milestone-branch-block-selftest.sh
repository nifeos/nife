#!/bin/sh
#
# Selftest for helpers/milestone-branch-block.sh (script/lint-fleet's check 4b), run by script/lint-fleet.
# Builds a throwaway repository that holds a record of its own, so it needs no tree and no network.
# The cases are #1901's shape (cut with one number, renumbered, branch kept) beside the case the
# check exists to catch. The failing case is the point: a check that never fails looks the same as
# one that cannot (milestone 640 (a gate is not evidence until somebody has watched it fail)).
#
# Name: provisional, 2026-10-10 (UTC), after the other helpers/*-selftest.sh.

set -eu
check="$(cd "$(dirname "$0")" && pwd)/milestone-branch-block.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null

fail() { echo "milestone-branch-block-selftest: $*" >&2; exit 1; }
g() { git -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@"; }

# expect NAME WANT_EXIT WANT_TEXT|"" [NOT_TEXT]: run the check on the current branch.
expect() {
    name=$1 want=$2 text=$3 not=${4:-}
    set +e
    out=$(sh "$check" "$work" "$(g symbolic-ref --short HEAD)" 2>&1)
    got=$?
    set -e
    [ "$got" -eq "$want" ] || fail "$name: exit $got, wanted $want ($out)"
    if [ -n "$text" ]; then
        printf '%s' "$out" | grep -qF -- "$text" || fail "$name: output lacks '$text' ($out)"
    fi
    if [ -n "$not" ]; then
        printf '%s' "$out" | grep -qF -- "$not" && fail "$name: output has '$not' ($out)"
    fi
    return 0
}

# lane BRANCH: a fresh branch off main with one claim commit.
lane() {
    g checkout -q -b "$1" main
    g commit -q --allow-empty -m "claim"
}

g init -q -b main .
# The scratch tree holds a record of its own (design/roadmap/README.md is the marker
# helpers/record_root.py looks for), so the check reads these blocks and not the pinned record.
mkdir -p design/roadmap
echo "the roadmap" > design/roadmap/README.md
echo "relibc's seed" > design/roadmap/0868-relibcs-seed.md
echo "caches" > design/roadmap/0870-caches.md
echo "a sixth pass" > design/roadmap/0871-a-sixth-pass.md
printf 'Branch milestone/869-old-title, renumbered from 869.\n' > design/roadmap/0872-new-title.md
g add -A; g commit -q -m base
g update-ref refs/remotes/origin/main main

# 1. A branch whose milestone has a block passes quietly.
lane milestone/870-caches-again
expect "own block" 0 "" "lint:"

# 2. #1901: cut as 873, renumbered to 871, branch kept, block slug matches the branch.
lane milestone/873-a-sixth-pass
expect "renumbered, slug" 0 "renumbered; its block is roadmap/0871-a-sixth-pass.md"

# 3. Renumbered and retitled: the block names the branch, so it is still found.
lane milestone/869-old-title
expect "renumbered, named" 0 "its block is roadmap/0872-new-title.md"

# 4. A number with no block and no renumber record fails, and says where the blocks are read.
lane milestone/990-nowhere
expect "no block at all" 1 "which has no roadmap block"
expect "no block, the pin" 1 "the commit basalt pins"

# 5. Not a milestone branch: nothing to say.
lane maintainer/whatever
expect "not a milestone" 0 "" "lint:"

echo "milestone-branch-block-selftest: ok"
