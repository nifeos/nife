#!/bin/sh
#
# helpers/ci-log-baseline-selftest.sh: script/ci-log-baseline keeps backslashes in log text as typed.
#
# script/ci-log-baseline runs under /bin/sh, which is dash on Ubuntu, and dash's `echo` turns `\n`,
# `\t` and `\c` in its argument into a line break, a tab and a cut. The text the script passes
# around is CI log text (a failing check's message, an error body), which holds backslashes:
# Windows paths, escaped strings, rustc's `\n` inside a quoted literal. The same defect crashed
# helpers/ci-failing.sh on 2026-10-31 (issue 2960). The attribution line is one CSV field, so a
# line break in it is a corrupt row. This feeds format_attribution, lifted out of the script, a
# detail full of backslashes and requires it to come back byte for byte. Run it under dash to
# see it fail on `echo`.

set -e
here="$(cd "$(dirname "$0")" && pwd)"
me="$(basename "$0")"
script="$here/../script/ci-log-baseline"

fn="$(awk '/^format_attribution\(\) \{/ {on=1} on {print} on && /^\}/ {exit}' "$script")"
[ -n "$fn" ] || { echo "$me: format_attribution is gone from script/ci-log-baseline; update this test" >&2; exit 1; }
eval "$fn"

detail='error: C:\new\tmp says "a\nb\tc" and ends with \c'
for status in MARKER PREFLIGHT MODELS TRIPWIRE UNATTRIBUTABLE OTHER; do
	got="$(format_attribution job "$status" "$detail")"
	case "$got" in
	*"$detail"*) ;;
	*)
		echo "$me: $status changed the backslashes in its detail:" >&2
		printf '  want a line holding: %s\n' "$detail" >&2
		printf '  got: %s\n' "$got" >&2
		exit 1
		;;
	esac
	[ "$(printf '%s\n' "$got" | wc -l)" -eq 1 ] || { echo "$me: $status produced more than one line" >&2; exit 1; }
done
echo "$me: ok"
