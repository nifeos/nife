#!/bin/sh
#
# helpers/catch-up-selftest.sh: prove script/catch-up reads the milestone blocks and fails loud.
#
#     helpers/catch-up-selftest.sh
#
# Until this existed, script/catch-up read design/roadmap/README.md first, found no table row in
# what is now prose, and printed zero changes with exit 0 for every window since the index was
# retired. A scratch repository with three commits stands in for the history: an index era (a
# README table), a block era (the table gone, blocks present) and a broken era (blocks present with
# no status). Each of these must hold:
#   1. block era against index era: the counts match the fixture (1 changed, 1 minted, none
#      reported as disappeared, since the table's retirement is not a removal);
#   2. block era against an earlier block era: the same, read from blocks at both ends;
#   3. a tree whose blocks yield no status exits non-zero with a message and prints no zero;
#   4. a tree with a README that holds no row and no blocks exits non-zero.
# No network, no real history; it needs git and python3.

set -eu
cd "$(dirname "$0")/.."
repo="$(pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/catch-up-selftest.XXXXXX")"
trap 'rm -rf "$work"' EXIT
fail() { echo "catch-up-selftest: $1" >&2; exit 1; }

mkdir -p "$work/script" "$work/helpers"
cp "$repo/script/catch-up" "$work/script/catch-up"
cp "$repo/helpers/roadmap_block.py" "$work/helpers/"
cd "$work"
git init -q .
git config user.email selftest@example.invalid
git config user.name selftest
git config commit.gpgsign false
mkdir -p design/roadmap

block() { # number slug status
    printf -- '---\nstatus: %s\nraised: 2026-11-01\n---\n# %s. Title %s\n\nbody\n' "$3" "$1" "$1" >"design/roadmap/$1-$2.md"
}
commit() { git add -A && git commit -q -m "$1" --date "$2 12:00:00 +0000"; }
export GIT_COMMITTER_DATE

# Index era: a README table naming milestones 1 and 2.
cat >design/roadmap/README.md <<'R'
| 1 | PROPOSED | [One](0001-one.md) | why | |
| 2 | BUILT | [Two](0002-two.md) | why | 2026-08-01 |
R
GIT_COMMITTER_DATE="2026-09-01T12:00:00" commit index 2026-09-01
index_era=$(git rev-parse HEAD)

# Block era: the table is prose, milestone 1 is now BUILT, 2 unchanged, 3 is new.
printf 'Prose only, no table.\n' >design/roadmap/README.md
block 0001 one BUILT; block 0002 two BUILT; block 0003 three PROPOSED
GIT_COMMITTER_DATE="2026-10-01T12:00:00" commit blocks 2026-10-01
block_era=$(git rev-parse HEAD)

# A later block era: 4 is new, 3 changed.
block 0003 three BUILT; block 0004 four PROPOSED
GIT_COMMITTER_DATE="2026-11-01T12:00:00" commit later 2026-11-01
later=$(git rev-parse HEAD)

count() { # label section-title output
    printf '%s\n' "$3" | sed -n "s/^$2 (\([0-9]*\))\$/\1/p"
}

git checkout -q "$block_era"
out=$(script/catch-up "$index_era" 2>&1) || fail "block era against index era failed: $out"
[ "$(count x 'MILESTONES THAT CHANGED STATUS' "$out")" = 1 ] || fail "index to blocks: wrong changed count: $out"
[ "$(count x 'MILESTONES NEWLY MINTED' "$out")" = 1 ] || fail "index to blocks: wrong minted count: $out"
printf '%s\n' "$out" | grep -q 'MILESTONE ROWS THAT DISAPPEARED' && fail "index to blocks reads the retired table as removals: $out"

git checkout -q "$later"
out=$(script/catch-up "$block_era" 2>&1) || fail "block era against block era failed: $out"
[ "$(count x 'MILESTONES THAT CHANGED STATUS' "$out")" = 1 ] || fail "blocks to blocks: wrong changed count: $out"
[ "$(count x 'MILESTONES NEWLY MINTED' "$out")" = 1 ] || fail "blocks to blocks: wrong minted count: $out"

# Blocks that yield no status: the reader is broken, so refuse instead of printing zero.
printf '# 5. No frontmatter\n\nbody\n' >design/roadmap/0005-five.md
for n in 0001-one 0002-two 0003-three 0004-four; do git rm -q "design/roadmap/$n.md"; done
GIT_COMMITTER_DATE="2026-11-02T12:00:00" commit broken 2026-11-02
if out=$(script/catch-up "$block_era" 2>&1); then fail "statusless blocks exited 0: $out"; fi
printf '%s\n' "$out" | grep -q 'refusing to report zero' || fail "statusless blocks gave no message: $out"
printf '%s\n' "$out" | grep -q 'MILESTONES NEWLY MINTED (0)' && fail "statusless blocks printed a zero: $out"

# No block and no table at all.
git rm -q design/roadmap/0005-five.md
GIT_COMMITTER_DATE="2026-11-03T12:00:00" commit empty 2026-11-03
if out=$(script/catch-up "$block_era" 2>&1); then fail "an empty roadmap exited 0: $out"; fi
printf '%s\n' "$out" | grep -q 'no roadmap at HEAD' || fail "an empty roadmap gave no message: $out"

echo "catch-up-selftest: ok"
