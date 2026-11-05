#!/bin/sh
#
# Selftest for the host following main (helpers/fuzz-follow-main.py and its use in
# helpers/continuous-fuzz.sh), run by script/lint. No network, no KVM, no toolchain.
#
# A scratch upstream repository stands in for nifeos/nife, and a clone of it stands in for the host's
# clone. Stubs first on PATH (the supervisor puts `$HOME/.cargo/bin` there) replace four programs: a
# `git` that records each fetch and points the one upstream address at the scratch repository, a
# `curl` that serves fixture API answers, and `rustup`, `cargo` and `qemu-system-x86_64` stubs that
# make a window take a moment and end clean. It proves, against the four ruled limits:
#
#   - the fetch names only main of the public address, over https (limit 2);
#   - main at the clone's own commit, and a main that does not contain it, each stay put and make no
#     API request (limit 1);
#   - an unreadable API and a red required check each stay put; a green one names the commit
#     (limit 3; the green rule's own cases are `fuzz-follow-main.py --selftest`);
#   - `apply` refuses a commit the last fetch did not bring, and a clone with local changes;
#   - end to end, the supervisor drains its slots, moves the clone, re-executes itself (the toolchain
#     step runs twice), moves every slot worktree, records windows at the new commit, and prints one
#     line that the unit file changed while installing nothing (limit 4).

set -eu
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/fuzz-follow-main-selftest.XXXXXX")"
sup_pid=""
trap '[ -z "$sup_pid" ] || kill "$sup_pid" 2>/dev/null; rm -rf "$work"' EXIT

fail() { echo "fuzz-follow-main-selftest: $*" >&2; exit 1; }

GIT_CONFIG_NOSYSTEM=1
GIT_CONFIG_GLOBAL=/dev/null
STUB_REAL_GIT="$(command -v git)"
STUB_UPSTREAM="$work/up"
STUB_FETCHES="$work/fetches"
STUB_CURLS="$work/curls"
STUB_CALLS="$work/rustup-calls"
STUB_API="$work/api"
export GIT_CONFIG_NOSYSTEM GIT_CONFIG_GLOBAL STUB_REAL_GIT STUB_UPSTREAM STUB_FETCHES STUB_CURLS \
    STUB_CALLS STUB_API

bin="$work/home/.cargo/bin"
mkdir -p "$bin" "$STUB_API"
cat > "$bin/git" <<'STUB'
#!/bin/sh
case " $* " in
*" fetch "*)
    printf '%s\n' "$*" >> "$STUB_FETCHES"
    for a; do
        shift
        if [ "$a" = https://github.com/nifeos/nife.git ]; then set -- "$@" "$STUB_UPSTREAM"; else set -- "$@" "$a"; fi
    done
    exec "$STUB_REAL_GIT" -c protocol.file.allow=always "$@"
    ;;
esac
exec "$STUB_REAL_GIT" "$@"
STUB
cat > "$bin/curl" <<'STUB'
#!/bin/sh
for a; do url="$a"; done
printf '%s\n' "$url" >> "$STUB_CURLS"
[ "${STUB_API_FAIL:-0}" = 0 ] || { echo "curl: (22) The requested URL returned error: 403" >&2; exit 22; }
case "$url" in
*/rules/branches/main) cat "$STUB_API/rules.json" ;;
*/commits/*/check-runs*)
    sha="${url#*/commits/}"; sha="${sha%%/*}"
    sed -e "s/@SHA@/$sha/g" -e "s/@CONCLUSION@/${STUB_CONCLUSION:-success}/g" "$STUB_API/runs.json" ;;
*) exit 22 ;;
esac
STUB
cat > "$bin/rustup" <<'STUB'
#!/bin/sh
echo "$*" >> "$STUB_CALLS"
STUB
cat > "$bin/cargo" <<'STUB'
#!/bin/sh
case "$*" in
"xtask initial-ramdisk-x86") mkdir -p target && : > target/initial-ramdisk-x86_64.img ;;
test*) echo '{"reason":"compiler-artifact","target":{"name":"system_tests"},"executable":"/stub/system_tests"}' ;;
esac
STUB
cat > "$bin/qemu-system-x86_64" <<'STUB'
#!/bin/sh
case " $* " in *" -S "*) exit 0 ;; esac
sleep 1
echo "syscall fuzzer: 5 seeds, 1 ms (stub)"
echo "confined fuzzer: 5 seeds, 1 ms (stub)"
exit 3
STUB
chmod +x "$bin"/*

cat > "$STUB_API/rules.json" <<'JSON'
[{"type":"pull_request"},{"type":"required_status_checks","parameters":{"required_status_checks":[{"context":"lint","integration_id":null},{"context":"test","integration_id":null}]}}]
JSON
cat > "$STUB_API/runs.json" <<'JSON'
{"total_count":3,"check_runs":[
 {"name":"lint","status":"completed","conclusion":"success","head_sha":"@SHA@","app":{"slug":"github-actions","id":15368}},
 {"name":"test","status":"completed","conclusion":"@CONCLUSION@","head_sha":"@SHA@","app":{"slug":"github-actions","id":15368}},
 {"name":"test","status":"completed","conclusion":"skipped","head_sha":"@SHA@","app":{"slug":"github-actions","id":15368}}]}
JSON

g() { git -c user.name=selftest -c user.email=selftest@invalid -c commit.gpgsign=false "$@"; }

# The upstream: commit A holds the files the supervisor needs, B changes the unit and adds a file,
# and C shares no history with A, as a rewritten main would not.
up="$STUB_UPSTREAM"
mkdir -p "$up/helpers/systemd"
for f in continuous-fuzz.sh fuzz-follow-main.py qemu-bounded.sh qemu-path.sh rustup-retry.sh \
    qemu-runner-x86_64.sh; do
    cp -p "$here/$f" "$up/helpers/$f"
done
cp "$here/systemd/nife-fuzz.service" "$up/helpers/systemd/"
g -C "$up" init --quiet --initial-branch=main
g -C "$up" add -A
g -C "$up" commit --quiet -m A
a="$(git -C "$up" rev-parse HEAD)"
echo "# changed" >> "$up/helpers/systemd/nife-fuzz.service"
echo marker > "$up/marker"
g -C "$up" add -A
g -C "$up" commit --quiet -m B
b="$(git -C "$up" rev-parse HEAD)"
g -C "$up" checkout --quiet --orphan side
echo side > "$up/side"
g -C "$up" add -A
g -C "$up" commit --quiet -m C
c="$(git -C "$up" rev-parse HEAD)"
g -C "$up" checkout --quiet main

clone="$work/clone"
git clone --quiet "$up" "$clone"
git -C "$clone" checkout --quiet --detach "$a"

PATH="$bin:$PATH"
export PATH
follow() { python3 -I "$clone/helpers/fuzz-follow-main.py" "$@"; }
look() {
    : > "$STUB_CURLS"
    set +e
    out="$(follow look 2>&1)"
    rc=$?
    set -e
    curls="$(wc -l < "$STUB_CURLS" | tr -d ' ')"
}

# Limit 1 and 2: main where the clone is.
git -C "$up" update-ref refs/heads/main "$a"
look
[ "$rc" -eq 1 ] || fail "main at the clone's commit: exit $rc ($out)"
[ "$curls" -eq 0 ] || fail "main at the clone's commit: $curls API requests, wanted none"
grep -q ' fetch .* https://github.com/nifeos/nife.git +refs/heads/main:refs/fuzz-follow/main$' "$STUB_FETCHES" \
    || fail "the fetch did not name main of the public address alone: $(cat "$STUB_FETCHES")"
grep -q 'protocol.allow=never' "$STUB_FETCHES" || fail "the fetch does not refuse other protocols"

# Limit 1: main that does not contain the clone's commit.
git -C "$up" update-ref refs/heads/main "$c"
look
[ "$rc" -eq 1 ] && [ "$curls" -eq 0 ] || fail "non-fast-forward: exit $rc, $curls API requests ($out)"
printf '%s' "$out" | grep -q 'not a fast-forward' || fail "non-fast-forward: no reason ($out)"

# Limit 3: an unreadable API, then a red check, then green.
git -C "$up" update-ref refs/heads/main "$b"
STUB_API_FAIL=1 look
[ "$rc" -eq 1 ] || fail "unreadable API: exit $rc ($out)"
STUB_CONCLUSION=failure look
[ "$rc" -eq 1 ] || fail "red check: exit $rc ($out)"
printf '%s' "$out" | grep -q "'test' concluded failure" || fail "red check: no reason ($out)"
look
[ "$rc" -eq 0 ] && [ "$out" = "$b" ] || fail "green: exit $rc, printed '$out', wanted $b"
[ "$curls" -eq 2 ] || fail "green: $curls API requests, wanted 2"
[ "$(git -C "$clone" rev-parse HEAD)" = "$a" ] || fail "look moved the clone"

# apply refuses what the last fetch did not bring, and a clone with local changes.
follow apply "$c" 2>/dev/null && fail "apply took a commit the fetch did not bring"
echo dirty >> "$clone/helpers/qemu-path.sh"
follow apply "$b" 2>/dev/null && fail "apply moved a clone with local changes"
git -C "$clone" checkout --quiet -- helpers/qemu-path.sh

# End to end: the supervisor at A, main at B and green.
sup_out="$work/supervisor.out"
HOME="$work/home" NIFE_FUZZ_HOME="$work/fuzz" NIFE_FUZZ_KVM_DEVICE=/dev/null NIFE_FUZZ_GUESTS=2 \
    NIFE_FUZZ_FOLLOW_EVERY=1 NIFE_RUSTUP_ATTEMPTS=1 NIFE_RUSTUP_BACKOFF=0 \
    "$clone/helpers/continuous-fuzz.sh" > "$sup_out" 2>&1 &
sup_pid=$!
short_b="$(git -C "$clone" rev-parse --short "$b")"
deadline=$(($(date +%s) + 90))
moved=0
while [ "$(date +%s)" -lt "$deadline" ]; do
    if [ -f "$work/fuzz/state/windows.tsv" ] \
        && [ "$(cut -f8 "$work/fuzz/state/windows.tsv" | grep -c "^$short_b$")" -ge 2 ]; then
        moved=1
        break
    fi
    kill -0 "$sup_pid" 2>/dev/null || break
    sleep 1
done
kill "$sup_pid" 2>/dev/null || true
wait "$sup_pid" 2>/dev/null || true
sup_pid=""
[ "$moved" -eq 1 ] || fail "end to end: no windows at the new commit within 90 s: $(tail -20 "$sup_out")"
[ "$(git -C "$clone" rev-parse HEAD)" = "$b" ] || fail "end to end: the clone is not at main"
for n in 0 1; do
    [ "$(git -C "$work/fuzz/work/$n" rev-parse HEAD)" = "$b" ] || fail "end to end: slot $n is not at main"
done
[ "$(grep -c 'show active-toolchain' "$STUB_CALLS")" -ge 2 ] \
    || fail "end to end: the toolchain step ran once, so the supervisor did not re-execute"
[ "$(grep -c 'the unit files changed (helpers/systemd/nife-fuzz.service); nothing was installed' "$sup_out")" -eq 1 ] \
    || fail "end to end: wanted one unit-file line: $(grep -i unit "$sup_out")"

# Structural, not behavioral: the race between two `git worktree add` runs in one clone did not
# reproduce locally in 20 tries, so this pins the order that removes it. The supervisor makes every
# slot's worktree in its own loop before the line that starts the first worker.
cf="$(dirname "$0")/continuous-fuzz.sh"
prep_line="$(grep -n '^    prepare_slot "\$slot" || exit 1$' "$cf" | head -1 | cut -d: -f1)"
start_line="$(grep -n '^    worker "\$slot" &$' "$cf" | head -1 | cut -d: -f1)"
[ -n "$prep_line" ] && [ -n "$start_line" ] && [ "$prep_line" -lt "$start_line" ] \
    || fail "the supervisor does not make the slot worktrees before it starts the workers"

echo "fuzz-follow-main-selftest: ok"
