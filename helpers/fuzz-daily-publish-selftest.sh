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
grep -q "release fuzz-$today on partsfoundry/nife" "$work/stdout" || fail "the dry run did not print the tag"
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

# The Filer line: host-local counts and times, read from a stub filer, a stub systemctl and a stub
# ledger. Only the filer's closing tally is kept: the cause lines it prints must not reach the report.
cat > "$work/bin-filer" <<'STUB'
#!/bin/sh
echo "fuzz-filer: cause abcdef123456 (budget): 9000 held logs, parked: a budget is filed only when a rule says file"
echo "fuzz-filer: $FILER_TALLY"
STUB
cat > "$work/bin-systemctl" <<'STUB'
#!/bin/sh
case "$1" in
    is-enabled) [ -n "$TIMER_STATE" ] && echo "$TIMER_STATE"; [ "$TIMER_STATE" = enabled ] ;;
    show) echo "LastTriggerUSec=$TIMER_LAST" ;;
esac
STUB
chmod +x "$work/bin-filer" "$work/bin-systemctl"
printf '# comment\n2026-11-07T12:00:00Z\tabcdef0123456789\tfiled\tGHSA-xxxx-xxxx-xxxx\n2026-11-07T12:05:00Z\tfedcba\tcause\tGHSA-xxxx\t3\tabc\n' > "$work/ledger.tsv"

filer_line() { # name, want (the whole line), then environment assignments for env(1)
    name="$1"; want="$2"; shift 2
    rm -rf "$work/fl"; mkdir -p "$work/fl"
    env NIFE_FUZZ_HOME="$work/fl" NIFE_FUZZ_FILER="$work/bin-filer" NIFE_FUZZ_SYSTEMCTL="$work/bin-systemctl" \
        NIFE_FUZZ_FILER_LEDGER="$work/ledger.tsv" "$@" "$here/fuzz-daily-publish.sh" --dry-run >/dev/null 2>&1 \
        || fail "$name: the dry run exited non-zero"
    got="$(grep '^Filer:' "$work/fl/out/$today/report.md")"
    [ "$got" = "Filer: $want" ] || fail "$name: line is '$got', want 'Filer: $want'"
    [ "$(grep -c '^Filer:' "$work/fl/out/$today/report.md")" = 1 ] || fail "$name: not exactly one Filer line"
    ! grep -q 'abcdef123456\|9000' "$work/fl/out/$today/report.md" || fail "$name: a cause line leaked into the report"
}

ok_tally='17 held logs, 17 causes, 1 filed before, 16 parked, 0 suppressed, 0 refused, none to file'
filer_line "healthy" \
    "held=17 causes=17 filed=1 parked=16 unfiled=0 ledger-newest=2026-11-07T12:05:00Z timer-enabled=yes timer-last-run=2026-11-17T09:30:00Z" \
    FILER_TALLY="$ok_tally" TIMER_STATE=enabled TIMER_LAST="Tue 2026-11-17 09:30:00 UTC"
filer_line "causes to file" \
    "held=20 causes=19 filed=1 parked=16 unfiled=2 ledger-newest=2026-11-07T12:05:00Z timer-enabled=yes timer-last-run=2026-11-17T09:30:00Z" \
    FILER_TALLY='20 held logs, 19 causes, 1 filed before, 16 parked, 0 suppressed, 0 refused, 2 to file; the dry run sent nothing and wrote nothing' \
    TIMER_STATE=enabled TIMER_LAST="Tue 2026-11-17 09:30:00 UTC"
filer_line "timer disabled and never run" \
    "held=17 causes=17 filed=1 parked=16 unfiled=0 ledger-newest=2026-11-07T12:05:00Z timer-enabled=no timer-last-run=never" \
    FILER_TALLY="$ok_tally" TIMER_STATE=disabled TIMER_LAST=n/a
filer_line "systemctl silent" \
    "held=17 causes=17 filed=1 parked=16 unfiled=0 ledger-newest=2026-11-07T12:05:00Z timer-enabled=unknown timer-last-run=unknown" \
    FILER_TALLY="$ok_tally" TIMER_STATE= TIMER_LAST=
filer_line "no tally" \
    "held=unknown causes=unknown filed=unknown parked=unknown unfiled=unknown ledger-newest=2026-11-07T12:05:00Z timer-enabled=yes timer-last-run=2026-11-17T09:30:00Z" \
    FILER_TALLY='the token is refused' TIMER_STATE=enabled TIMER_LAST="Tue 2026-11-17 09:30:00 UTC"
filer_line "no ledger" \
    "held=17 causes=17 filed=1 parked=16 unfiled=0 ledger-newest=none timer-enabled=yes timer-last-run=2026-11-17T09:30:00Z" \
    FILER_TALLY="$ok_tally" TIMER_STATE=enabled TIMER_LAST="Tue 2026-11-17 09:30:00 UTC" \
    NIFE_FUZZ_FILER_LEDGER="$work/absent.tsv"
filer_line "no filer installed" \
    "held=unknown causes=unknown filed=unknown parked=unknown unfiled=unknown ledger-newest=2026-11-07T12:05:00Z timer-enabled=yes timer-last-run=2026-11-17T09:30:00Z" \
    NIFE_FUZZ_FILER="$work/absent-filer" TIMER_STATE=enabled TIMER_LAST="Tue 2026-11-17 09:30:00 UTC"

# The publish path refuses a missing or stale token by name, before any gh call. A stub gh on PATH
# records each call; every refusal must exit non-zero, name its cause in one line, print no token,
# and call nothing. The last case, a token with an hour left, must reach the stub exactly once.
mkdir -p "$work/bin"
# GH_STUB_RELEASE sets what `gh release view` finds: unset, no release; `same`, the two ruled
# assets; `partial`, report.md alone; `extra`, the two plus one more; `error`, a network failure.
cat > "$work/bin/gh" <<STUB
#!/bin/sh
echo called >> "$work/gh-calls"
echo "\$*" >> "$work/gh-args"
if [ "\$1 \$2" = "release view" ]; then
    case "\${GH_STUB_RELEASE:-}" in
        same) printf 'report.md\\nclean-ranges.tsv\\n' ;;
        partial) echo report.md ;;
        extra) printf 'clean-ranges.tsv\\nreport.md\\nnotes.txt\\n' ;;
        error) echo 'error connecting to api.github.com' >&2; exit 1 ;;
        *) echo 'release not found' >&2; exit 1 ;;
    esac
fi
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

# A fresh token reaches gh: one view that finds no release, then one create.
rm -f "$work/gh-calls" "$work/gh-args"
PATH="$work/bin:$PATH" NIFE_FUZZ_PUBLISH=1 GH_TOKEN="$secret" GH_TOKEN_EXPIRES_AT="$(offset_time 3600)" \
    "$here/fuzz-daily-publish.sh" --publish >/dev/null 2>"$work/stderr" \
    || fail "a fresh token was refused: $(cat "$work/stderr")"
[ "$(wc -l < "$work/gh-calls" | tr -d ' ')" = 2 ] || fail "a fresh token did not reach gh exactly twice"
case "$(head -1 "$work/gh-args")" in
    "release view fuzz-$today --repo partsfoundry/nife "*) ;;
    *) fail "the first gh call is not 'release view fuzz-$today --repo partsfoundry/nife': $(head -1 "$work/gh-args")" ;;
esac

# The create is the ruled release (partsfoundry/record:design/roadmap/1026): `gh release create fuzz-<utc date>` on
# partsfoundry/nife, with report.md and clean-ranges.tsv as its only assets, and the script never pushes.
args="$(tail -1 "$work/gh-args")"
case "$args" in
    "release create fuzz-$today --repo partsfoundry/nife "*) ;;
    *) fail "gh was not called as 'release create fuzz-$today --repo partsfoundry/nife': $args" ;;
esac
assets="$(printf '%s\n' "$args" | tr ' ' '\n' | grep '^/' | sed 's|.*/||' | sort -u | tr '\n' ' ')"
[ "$assets" = "clean-ranges.tsv report.md " ] || fail "release assets are '$assets', want report.md and clean-ranges.tsv"
! grep -v '^ *#' "$here/fuzz-daily-publish.sh" | grep -q 'git push' || fail "the publish script calls git push"

# A second run on the same day. The day's release with both assets is success with no create; any
# other set of assets, or a view that fails for another reason, is a loud refusal with no create.
second_run() { # name, want status (0 or 1), want (text in stdout or stderr), GH_STUB_RELEASE value
    name="$1"; want_status="$2"; want="$3"
    rm -f "$work/gh-calls" "$work/gh-args"
    set +e
    env PATH="$work/bin:$PATH" NIFE_FUZZ_PUBLISH=1 GH_TOKEN="$secret" GH_TOKEN_EXPIRES_AT="$(offset_time 3600)" \
        GH_STUB_RELEASE="$4" "$here/fuzz-daily-publish.sh" --publish >"$work/stdout" 2>"$work/stderr"
    status=$?
    set -e
    [ "$status" = "$want_status" ] || fail "$name: exited $status, want $want_status: $(cat "$work/stderr")"
    grep -q -- "$want" "$work/stdout" "$work/stderr" || fail "$name: output lacks '$want'"
    ! grep -q 'release create' "$work/gh-args" || fail "$name: gh release create was called"
    ! grep -q -E 'release (delete|upload|edit)' "$work/gh-args" || fail "$name: the existing release was changed"
    ! grep -q "$secret" "$work/stdout" "$work/stderr" || fail "$name: the token was printed"
}
second_run "release complete" 0 "already exists on partsfoundry/nife with report.md and clean-ranges.tsv" same
[ ! -s "$work/stderr" ] || fail "release complete: wrote to stderr: $(cat "$work/stderr")"
second_run "release missing an asset" 1 "assets 'report.md', want 'clean-ranges.tsv report.md'" partial
second_run "release with an extra asset" 1 "assets 'clean-ranges.tsv notes.txt report.md'" extra
second_run "view fails" 1 "cannot tell whether release fuzz-$today exists" error

# No held finding appears in either asset: the held seed is absent from the report and the table.
! grep -q "$held_seed" "$NIFE_FUZZ_HOME/out/$today/report.md" "$ranges" \
    || fail "the held seed appears in a published asset"
grep -q 'Held findings: 0, oldest 0 days' "$NIFE_FUZZ_HOME/out/$today/report.md" \
    || fail "the report does not state the held count and age"

echo "fuzz-daily-publish-selftest: ok"
