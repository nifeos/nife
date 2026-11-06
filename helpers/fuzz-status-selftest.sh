#!/bin/sh
# helpers/fuzz-status-selftest.sh: script/fuzz-status reports PASS, FAIL and UNKNOWN for each check.
#
# Stub `gh`, `systemctl` and `journalctl` on PATH answer from fixture files, a scratch fuzz home holds
# the state, and a scratch clone holds the follower's ref, so nothing here reaches GitHub or systemd.
# Time is fixed (NIFE_FUZZ_STATUS_NOW). Each scenario changes a few fixtures and names the status it
# expects on a check's line; the first scenario is a healthy system and must exit 0, and a scenario
# with a FAIL must exit 1 while one with only UNKNOWN must exit 0. Scenarios run in order:
#
#   default mode: heartbeat (PASS, FAIL failed, FAIL stale, UNKNOWN no runs), release exists (PASS,
#   FAIL), publish on (PASS, FAIL old), loop issues (PASS, FAIL), advisories (PASS, UNKNOWN), host
#   commit (PASS green, FAIL red, UNKNOWN unfinished)
#   host mode: service (PASS, FAIL), timer (PASS, FAIL), iteration (PASS, FAIL old, FAIL absent,
#   UNKNOWN unreadable), follow-main (PASS, FAIL too far behind, UNKNOWN no ref), output directory
#   (PASS, FAIL), worktrees (PASS, FAIL missing, FAIL commondir), corpus (PASS, FAIL stale, FAIL no
#   growth with --wait), disk (PASS, FAIL)
#
# Name: provisional, minted 2026-11-05 (UTC) by maintainer/fuzz-status.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
tool="$here/../script/fuzz-status"
work="$(mktemp -d "${TMPDIR:-/tmp}/fuzz-status-selftest.XXXXXX")"
trap 'rm -rf "$work"' EXIT INT TERM
fail() { echo "fuzz-status-selftest: $1" >&2; exit 1; }

NOW=1793000000
day() { python3 -c "import datetime,sys;print((datetime.datetime.fromtimestamp($NOW+int(sys.argv[1])*86400,datetime.timezone.utc)).strftime('%Y-%m-%d'))" "$1"; }
stamp() { python3 -c "import datetime,sys;print(datetime.datetime.fromtimestamp($NOW+int(sys.argv[1]),datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'))" "$1"; }
TODAY="$(day 0)"; YESTERDAY="$(day -1)"; OLD="$(day -5)"

bin="$work/bin"; fx="$work/fx"; home="$work/home"; clone="$work/clone"
mkdir -p "$bin" "$fx" "$home"
cat > "$bin/gh" <<'STUB'
#!/bin/sh
f="$FX/$1_$2"
[ "$1" = api ] && f="$FX/api_$(printf %s "$2" | tr '/?&=' '____')"
[ -f "$f" ] || { echo "stub gh: no fixture $f" >&2; exit 1; }
cat "$f"
STUB
cat > "$bin/systemctl" <<'STUB'
#!/bin/sh
case "$1" in
    is-active) cat "$FX/is_active" ;;
    show) [ -f "$FX/show_timer" ] || exit 1; cat "$FX/show_timer" ;;
esac
STUB
cat > "$bin/journalctl" <<'STUB'
#!/bin/sh
[ -f "$FX/journal" ] || exit 1
cat "$FX/journal"
STUB
chmod +x "$bin"/*
SHA=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa

healthy() {
    rm -rf "$fx" "$home"; mkdir -p "$fx" "$home/out/$TODAY" "$home/state"
    printf '[{"status":"completed","conclusion":"success","createdAt":"%s"}]\n' "$(stamp -3600)" > "$fx/run_list"
    printf '[{"tagName":"fuzz-%s"}]\n' "$TODAY" > "$fx/release_list"
    echo '[]' > "$fx/issue_list"
    echo '[]' > "$fx/api_repos_nifeos_nife_security-advisories"
    printf '{"sha":"%s"}\n' "$SHA" > "$fx/api_repos_nifeos_nife_commits_main"
    echo '[{"type":"required_status_checks","parameters":{"required_status_checks":[{"context":"lint"}]}}]' \
        > "$fx/api_repos_nifeos_nife_rules_branches_main"
    checks green
    echo active > "$fx/is_active"
    printf 'UnitFileState=enabled\nActiveState=active\nNextElapseUSecRealtime=Tue 14:00:00 UTC\n' > "$fx/show_timer"
    printf '%s host nife-fuzz[1]: continuous-fuzz: slot 1 confined: ok\n' "$((NOW - 120))" > "$fx/journal"
    for n in 0 1 2 3; do mkdir -p "$home/work/$n"; git -C "$home/work/$n" init -q; git -C "$home/work/$n" -c user.name=t -c user.email=t@example.org commit -q --allow-empty -m w; done
    printf '%s\tconfined\t1\t100\t5000\t60000\tok\tabc\n%s\tconfined\t1\t200\t5000\t60000\tok\tabc\n' \
        "$(stamp -600)" "$(stamp -300)" > "$home/state/windows.tsv"
    # The clone: HEAD one commit behind the followed main.
    rm -rf "$clone"; git init -q "$clone"
    git -C "$clone" -c user.name=t -c user.email=t@example.org commit -q --allow-empty -m one
    git -C "$clone" -c user.name=t -c user.email=t@example.org commit -q --allow-empty -m two
    git -C "$clone" update-ref refs/fuzz-follow/main HEAD
    git -C "$clone" checkout -q --detach HEAD~1
}
checks() {
    state="$1"; concl=success; status=completed
    [ "$state" = red ] && concl=failure
    [ "$state" = pending ] && { concl=null; status=in_progress; }
    printf '{"total_count":1,"check_runs":[{"name":"lint","head_sha":"%s","status":"%s","conclusion":%s,"app":{"slug":"github-actions","id":15368}}]}\n' \
        "$SHA" "$status" "$( [ "$concl" = null ] && echo null || echo "\"$concl\"")" \
        > "$fx/api_repos_nifeos_nife_commits_${SHA}_check-runs_per_page_100_page_1"
}

# run MODE-ARGS...: sets rc and out.
run() {
    rc=0
    out="$(PATH="$bin:$PATH" FX="$fx" NIFE_FUZZ_STATUS_NOW="$NOW" NIFE_FUZZ_STATUS_CLONE="$clone" \
        NIFE_FUZZ_HOME="$home" "$tool" "$@" 2>&1)" || rc=$?
}
# expect CHECK STATUS: the line for CHECK starts with STATUS.
expect() {
    line="$(printf '%s\n' "$out" | grep -E "^(PASS|FAIL|UNKNOWN) +$1 " || true)"
    case "$line" in
        "$2 "*) ;;
        *) fail "check '$1' wanted $2, got: ${line:-<no line>}
$out" ;;
    esac
}
exit_is() { [ "$rc" -eq "$1" ] || fail "exit status $rc, wanted $1
$out"; }

# ---- default mode
healthy; run
for c in "heartbeat runs" "fuzz release exists" "daily publish is on" "loop-broken issues" advisories "host commit"; do
    expect "$c" PASS
done
exit_is 0

healthy; printf '[{"status":"completed","conclusion":"failure","createdAt":"%s"}]\n' "$(stamp -3600)" > "$fx/run_list"
run; expect "heartbeat runs" FAIL; exit_is 1
healthy; printf '[{"status":"completed","conclusion":"success","createdAt":"%s"}]\n' "$(stamp -400000)" > "$fx/run_list"
run; expect "heartbeat runs" FAIL
healthy; echo '[]' > "$fx/run_list"
run; expect "heartbeat runs" UNKNOWN; exit_is 0
healthy; echo '[]' > "$fx/release_list"
run; expect "fuzz release exists" FAIL; expect "daily publish is on" FAIL; exit_is 1
healthy; printf '[{"tagName":"fuzz-%s"}]\n' "$YESTERDAY" > "$fx/release_list"
run; expect "daily publish is on" PASS
healthy; printf '[{"tagName":"fuzz-%s"}]\n' "$OLD" > "$fx/release_list"
run; expect "fuzz release exists" PASS; expect "daily publish is on" FAIL
healthy; echo '[{"number":7},{"number":9}]' > "$fx/issue_list"
run; expect "loop-broken issues" FAIL; printf '%s\n' "$out" | grep -q '2 open issue' || fail "issue count not printed"
healthy; echo '[{"ghsa_id":"GHSA-xxxx-xxxx-xxxx","summary":"SECRET-DETAIL"}]' > "$fx/api_repos_nifeos_nife_security-advisories"
run; expect advisories PASS
printf '%s\n' "$out" | grep -q 'SECRET-DETAIL\|GHSA' && fail "an advisory detail was printed"
healthy; rm "$fx/api_repos_nifeos_nife_security-advisories"
run; expect advisories UNKNOWN; exit_is 0
healthy; checks red; run; expect "host commit" FAIL
healthy; checks pending; run; expect "host commit" UNKNOWN; exit_is 0

# ---- host mode
healthy; run --host
for c in "service active" "publish timer" "slot iteration" follow-main "output directory" "slot worktrees" "corpus growth" "disk free"; do
    expect "$c" PASS
done
exit_is 0
printf '%s\n' "$out" | grep -q 'next run Tue 14:00' || fail "the timer's next run was not printed"
healthy; echo inactive > "$fx/is_active"; run --host; expect "service active" FAIL; exit_is 1
healthy; printf 'UnitFileState=disabled\nActiveState=inactive\n' > "$fx/show_timer"; run --host; expect "publish timer" FAIL
healthy; printf '%s host nife-fuzz[1]: continuous-fuzz: slot 1 confined: ok\n' "$((NOW - 7200))" > "$fx/journal"
run --host; expect "slot iteration" FAIL
healthy; printf '%s host nife-fuzz[1]: continuous-fuzz: main is at x\n' "$((NOW - 60))" > "$fx/journal"
run --host; expect "slot iteration" FAIL
healthy; rm "$fx/journal"; run --host; expect "slot iteration" UNKNOWN; expect "slot worktrees" UNKNOWN; exit_is 0
healthy; run --host --max-behind 0; expect follow-main FAIL
healthy; git -C "$clone" update-ref -d refs/fuzz-follow/main; run --host; expect follow-main UNKNOWN
healthy; rm -rf "$home/out/$TODAY"; run --host; expect "output directory" FAIL
healthy; rm -rf "$home/out/$TODAY"; mkdir "$home/out/$YESTERDAY"; run --host; expect "output directory" PASS
healthy; rm -rf "$home/work/2"; run --host; expect "slot worktrees" FAIL
healthy; printf '%s host nife-fuzz[1]: continuous-fuzz: slot 1 confined: ok\n%s host git: fatal: failed to read .git/worktrees/2/commondir\n' \
    "$((NOW - 60))" "$((NOW - 50))" > "$fx/journal"
run --host; expect "slot worktrees" FAIL
healthy; : > "$home/state/windows.tsv"; run --host; expect "corpus growth" FAIL
healthy; printf '%s\tconfined\t1\t100\t5000\t60000\tok\tabc\n' "$(stamp -9000)" > "$home/state/windows.tsv"
run --host; expect "corpus growth" FAIL
healthy; printf '%s\tconfined\t1\t100\t5000\t60000\theld-finding\tabc\n' "$(stamp -60)" > "$home/state/windows.tsv"
run --host; expect "corpus growth" FAIL
printf '%s\n' "$out" | grep -q 'held-finding' && fail "a held row was printed"
healthy; run --host --wait 1; expect "corpus growth" FAIL
healthy; (sleep 0.3; printf '%s\tconfined\t1\t300\t5000\t60000\tok\tabc\n' "$(stamp -1)" >> "$home/state/windows.tsv") &
run --host --wait 1; wait; expect "corpus growth" PASS
healthy; run --host --disk-floor-gib 999999999; expect "disk free" FAIL
healthy; rm -rf "$home"; run --host; expect "disk free" UNKNOWN; expect "output directory" UNKNOWN

echo "fuzz-status-selftest: ok"
