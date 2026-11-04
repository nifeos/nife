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
    row 12 confined 3 5000000 10 held-budget-expired
    row 13 confined 3 5000011 500 ok
} > "$NIFE_FUZZ_HOME/state/windows.tsv"

"$here/fuzz-daily-publish.sh" --dry-run >"$work/stdout" 2>"$work/stderr" \
    || fail "dry run on a planted home exited non-zero: $(cat "$work/stderr")"
ranges="$NIFE_FUZZ_HOME/out/$today/clean-ranges.tsv"

# Columns: driver, first, ran, commit.
located="$(awk -F'\t' -v h="$held_seed" '$2 - 1 == h || $2 + $3 - 1 == h' "$ranges")"
[ -z "$located" ] || fail "a published range locates the held seed: $located"

firsts="$(cut -f2 "$ranges" | sort -n | tr '\n' ' ')"
[ "$firsts" = "1000000 1000501 1001001 3000000 7776000 " ] \
    || fail "published first seeds are '$firsts', want all of slot 1's and the ranges of slots 0 and 2 before their held windows, and none of slot 3 (held-budget-expired is held like the rest)"

# The publish path refuses a missing or stale token by name, before any gh call. A stub gh on PATH
# records each call; every refusal must exit non-zero, name its cause in one line, print no token,
# and call nothing. The last case, a token with an hour left, must reach the stub exactly once.
mkdir -p "$work/bin"
cat > "$work/bin/gh" <<STUB
#!/bin/sh
echo called >> "$work/gh-calls"
STUB
chmod +x "$work/bin/gh"

secret=ghs_selftestsecret0123456789
offset_time() { # seconds from now, as the mint helper writes it
    date -u -d "@$(($(date +%s) + $1))" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null \
        || date -u -r "$(($(date +%s) + $1))" +%Y-%m-%dT%H:%M:%SZ
}

refuses() { # name, want (text in the message), then environment assignments for env(1)
    name="$1"; want="$2"; shift 2
    rm -f "$work/gh-calls"
    set +e
    env -u GH_TOKEN -u GH_TOKEN_EXPIRES_AT PATH="$work/bin:$PATH" NIFE_FUZZ_PUBLISH=1 "$@" \
        "$here/fuzz-daily-publish.sh" --publish >"$work/stdout" 2>"$work/stderr"
    status=$?
    set -e
    [ "$status" -ne 0 ] || fail "$name: exited 0"
    grep -q -- "$want" "$work/stderr" || fail "$name: message lacks '$want': $(cat "$work/stderr")"
    [ "$(wc -l < "$work/stderr" | tr -d ' ')" = 1 ] || fail "$name: message is not one line"
    ! grep -q "$secret" "$work/stdout" "$work/stderr" || fail "$name: the token was printed"
    [ ! -e "$work/gh-calls" ] || fail "$name: gh was called"
}

refuses "token unset" "GH_TOKEN is not set; see fuzz-token" \
    GH_TOKEN_EXPIRES_AT="$(offset_time 3600)"
refuses "token empty" "GH_TOKEN is not set; see fuzz-token" \
    GH_TOKEN= GH_TOKEN_EXPIRES_AT="$(offset_time 3600)"
refuses "expiry unset" "GH_TOKEN_EXPIRES_AT is not set" GH_TOKEN="$secret"
refuses "expiry garbage" "GH_TOKEN_EXPIRES_AT is not YYYY" GH_TOKEN="$secret" GH_TOKEN_EXPIRES_AT=tomorrow
refuses "expiry wrong shape" "GH_TOKEN_EXPIRES_AT is not YYYY" GH_TOKEN="$secret" \
    GH_TOKEN_EXPIRES_AT="2026-11-03 14:00:00"
refuses "expiry impossible" "GH_TOKEN_EXPIRES_AT is not YYYY" GH_TOKEN="$secret" \
    GH_TOKEN_EXPIRES_AT=2026-13-45T99:99:99Z
refuses "expired" "expired at" GH_TOKEN="$secret" GH_TOKEN_EXPIRES_AT="$(offset_time -60)"
refuses "inside the margin" "safety margin" GH_TOKEN="$secret" GH_TOKEN_EXPIRES_AT="$(offset_time 120)"

# The NIFE_FUZZ_PUBLISH guard still comes first.
rm -f "$work/gh-calls"
set +e
env -u NIFE_FUZZ_PUBLISH PATH="$work/bin:$PATH" GH_TOKEN="$secret" GH_TOKEN_EXPIRES_AT="$(offset_time 3600)" \
    "$here/fuzz-daily-publish.sh" --publish >/dev/null 2>"$work/stderr"
status=$?
set -e
[ "$status" -ne 0 ] || fail "publish without NIFE_FUZZ_PUBLISH exited 0"
grep -q "NIFE_FUZZ_PUBLISH=1 is not set" "$work/stderr" || fail "the NIFE_FUZZ_PUBLISH guard did not speak"
[ ! -e "$work/gh-calls" ] || fail "gh was called without NIFE_FUZZ_PUBLISH"

# A fresh token reaches gh.
rm -f "$work/gh-calls"
PATH="$work/bin:$PATH" NIFE_FUZZ_PUBLISH=1 GH_TOKEN="$secret" GH_TOKEN_EXPIRES_AT="$(offset_time 3600)" \
    "$here/fuzz-daily-publish.sh" --publish >/dev/null 2>"$work/stderr" \
    || fail "a fresh token was refused: $(cat "$work/stderr")"
[ "$(wc -l < "$work/gh-calls" | tr -d ' ')" = 1 ] || fail "a fresh token did not reach gh exactly once"

echo "fuzz-daily-publish-selftest: ok"
