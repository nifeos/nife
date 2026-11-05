#!/bin/sh
#
# helpers/patch-section-check.sh: refuse a `[patch]` section that points anywhere but this tree's
# own vendor/ directory.
#
#     helpers/patch-section-check.sh [<root>]
#
# Reads every tracked Cargo.toml and .cargo/config.toml under <root> (default: the repository),
# outside vendor/. A `[patch.crates-io]` or `[patch."<git url>"]` table is a local development aid
# that redirects a dependency for the whole build; on main it hides which source a crate really
# builds from. A crate that has moved to its own repository is pinned as
# `{ version, git, rev }` at the tag's commit, never patched (milestone 857, fork 2;
# notes/crate-move.md). The one allowed entry is a `path` into vendor/, which is code this tree
# carries and owns (talc, with an upstream fix). Any other patch entry, a `git` or `registry`
# source or a `path` outside vendor/, fails and names the file and line.
#
# Outside a git checkout (the selftest's fixtures) it reads files with find. Selftest:
# helpers/patch-section-check-selftest.sh. Name: provisional.

set -eu
root=${1:-$(cd "$(dirname "$0")/.." && pwd)}
cd "$root"

if git rev-parse --show-toplevel >/dev/null 2>&1 && [ "$(git rev-parse --show-toplevel)" = "$(pwd)" ]; then
    files=$(git ls-files -- 'Cargo.toml' '**/Cargo.toml' '.cargo/config.toml' '**/.cargo/config.toml' ':!vendor')
else
    files=$(find . \( -name Cargo.toml -o -path '*/.cargo/config.toml' \) -not -path './vendor/*' -not -path './target/*' | sed 's|^\./||')
fi
[ -n "$files" ] || exit 0

status=0
for f in $files; do
    [ -f "$f" ] || continue
    awk -v file="$f" '
        function bad(why) { printf "lint: %s:%d: %s\n", file, NR, why > "/dev/stderr"; failed = 1 }
        { line = $0; sub(/^[ \t]+/, "", line) }
        line ~ /^#/ || line == "" { next }
        line ~ /^\[/ {
            inpatch = (line ~ /^\[[ \t]*patch([ \t.\]]|$)/)
            if (inpatch && line ~ /^\[[ \t]*patch[ \t]*\]/) bad("a bare [patch] table")
            next
        }
        line ~ /^patch[ \t]*\./ { if (line !~ /path[ \t]*=[ \t]*"[^"]*vendor\// || line ~ /(git|registry)[ \t]*=/) bad("a dotted patch key outside vendor/"); next }
        inpatch {
            if (line !~ /path[ \t]*=[ \t]*"([^"]*\/)?vendor\// || line ~ /(git|registry)[ \t]*=/)
                bad("a [patch] entry that is not a path into vendor/")
        }
        END { exit failed }
    ' "$f" || status=1
done
if [ "$status" -ne 0 ]; then
    echo "lint: a [patch] section may not reach main. Pin a moved crate as { version, git, rev } at its tag's commit (notes/crate-move.md)." >&2
fi
exit "$status"
