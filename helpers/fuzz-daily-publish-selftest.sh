#!/bin/sh
# helpers/fuzz-daily-publish-selftest.sh: the daily publish runs before the supervisor's first window.
#
# A dry run against an empty NIFE_FUZZ_HOME in a scratch directory must exit 0, print the release tag
# and both asset paths, and leave an empty windows.tsv. It fails if the script touches state/windows.tsv
# without creating state/ first, the way the first install behaves when the timer fires before the
# supervisor has written anything. Nothing here runs gh or reads a real home.
#
# A second dry run plants a windows.tsv. Slot 0 holds a replay finding whose held seed is 7777777,
# then runs clean windows from one seed past it, the way the supervisor does. Slot 2 holds a hang,
# then runs clean. Slot 1 runs clean throughout, with one infrastructure window. The table must name
# no range whose first seed less one, or whose last seed, is the held seed; it must leave out every
# range of slots 0 and 2 from the held window on, and keep their earlier ranges and all of slot 1's.
# The seeds are synthetic.
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

NIFE_FUZZ_HOME="$work/planted"
export NIFE_FUZZ_HOME
mkdir -p "$NIFE_FUZZ_HOME/state"
held_seed=7777777
# utc, driver, slot, first, ran, ms, status, commit. Slot 0's held window starts at 7777000 and ran
# 777 seeds before its replay, so the supervisor resumes the slot at 7777778.
{
    row() { printf '2000-01-01T00:00:%02dZ\t%s\t%s\t%s\t%s\t60000\t%s\taaaaaaa\n' "$@"; }
    row 1 kernel-thread 0 7776000 1000 ok
    row 2 confined 1 1000000 500 ok
    row 3 kernel-thread 2 3000000 400 ok
    row 4 kernel-thread 0 7777000 777 held-finding
    row 5 confined 1 1000500 0 infrastructure
    row 6 kernel-thread 2 3000400 0 held-hang
    row 7 kernel-thread 0 7777778 1000 ok
    row 8 confined 1 1000501 500 ok
    row 9 kernel-thread 2 3000401 400 ok
    row 10 kernel-thread 0 7778778 1000 ok
    row 11 confined 1 1001001 500 ok
} > "$NIFE_FUZZ_HOME/state/windows.tsv"

"$here/fuzz-daily-publish.sh" --dry-run >"$work/stdout" 2>"$work/stderr" \
    || fail "dry run on a planted home exited non-zero: $(cat "$work/stderr")"
ranges="$NIFE_FUZZ_HOME/out/$today/clean-ranges.tsv"

# Columns: driver, first, ran, commit.
located="$(awk -F'\t' -v h="$held_seed" '$2 - 1 == h || $2 + $3 - 1 == h' "$ranges")"
[ -z "$located" ] || fail "a published range locates the held seed: $located"

firsts="$(cut -f2 "$ranges" | sort -n | tr '\n' ' ')"
[ "$firsts" = "1000000 1000501 1001001 3000000 7776000 " ] \
    || fail "published first seeds are '$firsts', want all of slot 1's and the ranges of slots 0 and 2 before their held windows"

echo "fuzz-daily-publish-selftest: ok"
