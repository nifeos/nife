#!/bin/sh
# helpers/fuzz-daily-publish-selftest.sh: the daily publish runs before the supervisor's first window.
#
# A dry run against an empty NIFE_FUZZ_HOME in a scratch directory must exit 0, print the release tag
# and both asset paths, and leave an empty windows.tsv. It fails if the script touches state/windows.tsv
# without creating state/ first, the way the first install behaves when the timer fires before the
# supervisor has written anything. Nothing here runs gh or reads a real home.
#
# Name: provisional, minted 2026-11-01 (UTC) by lane/fuzz-publish-creates-state.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/fuzz-daily-publish-selftest.XXXXXX")"
trap 'rm -rf "$work"' EXIT INT TERM

NIFE_FUZZ_HOME="$work/home"
export NIFE_FUZZ_HOME
mkdir -p "$NIFE_FUZZ_HOME"

fail() { echo "fuzz-daily-publish-selftest: $1" >&2; exit 1; }

"$here/fuzz-daily-publish.sh" --dry-run >"$work/stdout" 2>"$work/stderr" \
    || fail "dry run on an empty home exited non-zero: $(cat "$work/stderr")"

today="$(date -u +%Y-%m-%d)"
grep -q "release fuzz-$today on nifeos/nife" "$work/stdout" || fail "the dry run did not print the tag"
grep -q "$NIFE_FUZZ_HOME/out/$today/report.md" "$work/stdout" || fail "the dry run did not list report.md"
grep -q "$NIFE_FUZZ_HOME/out/$today/clean-ranges.tsv" "$work/stdout" || fail "the dry run did not list clean-ranges.tsv"
[ -f "$NIFE_FUZZ_HOME/state/windows.tsv" ] || fail "state/windows.tsv was not created"
[ ! -s "$NIFE_FUZZ_HOME/state/windows.tsv" ] || fail "state/windows.tsv is not empty"
[ ! -e "$NIFE_FUZZ_HOME/state/held" ] || fail "the script created state/held"
echo "fuzz-daily-publish-selftest: ok"
