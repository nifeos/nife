#!/bin/sh
#
# Selftest for script/crate-review-packet: builds the packet for one small crate with the grep
# fallback (no compiler needed) and checks that every section a reviewer relies on is present.
# Run by script/lint-fleet.
#
# Name: provisional, 2026-11-01 (UTC), after the other helpers/*-selftest.sh.

set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
"$root/script/crate-review-packet" --no-rustdoc --out "$work" block_roster >/dev/null
packet="$work/block_roster.md"
for heading in '## The crate' '## Source size' '## Public API' '## `unsafe`' \
    '## Dependencies, and where each points' '## References back into nife' \
    '## Kani harnesses and falsification patches' '## Crates in nife that depend on it' \
    '## Identity values' '## README' '## Reviewer checklist'; do
    grep -qF "$heading" "$packet" || { echo "crate-review-packet-selftest: missing section $heading" >&2; exit 1; }
done
grep -q 'grep-based fallback' "$packet" || { echo "crate-review-packet-selftest: fallback not announced" >&2; exit 1; }
grep -q 'blkrosv1' "$packet" || { echo "crate-review-packet-selftest: MAGIC not reported" >&2; exit 1; }
if "$root/script/crate-review-packet" --out "$work" no_such_crate >/dev/null 2>&1; then
    echo "crate-review-packet-selftest: an unknown crate was accepted" >&2; exit 1
fi
echo "crate-review-packet-selftest: ok"
