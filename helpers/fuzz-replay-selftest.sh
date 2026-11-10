#!/bin/sh
# helpers/fuzz-replay-selftest.sh: the fuzz host replays a red CI shard and holds what reproduces,
# and fails the lint when it stops doing so. No network, no KVM, no toolchain.
#
# A scratch repository stands in for the host's clone, and stubs stand in for cargo, QEMU, rustup and
# the public API (curl). Each check fails without milestone 1038
# (design/roadmap/1038-the-fuzz-host-replays-a-red-ci-shard.md):
#
#   1. A replay against a stand-in guest that prints a `replay:` line and a panic leaves one held
#      log, named as the filer reads it, and one held row in windows.tsv, and prints one line: a
#      finding id (the log's SHA-256, as the filer names it) and an age. No guest text and no seed.
#   2. A replay against a clean guest leaves no log, held or not, and one `replay-clean` row.
#   3. A commit the clone's HEAD does not contain is refused before anything builds.
#   4. A stub of the run and check-run listings: a failed run whose shards are green, one whose
#      shards were skipped, one whose red job is not a shard, and one whose commit main never took
#      queue nothing; one red shard on main queues exactly that shard.
#   5. End to end, the supervisor replays the queued shard before any slot starts, and the queue
#      empties.
#
# The red-shard rule's own cases are `fuzz-red-shards.py --selftest`.
#
# Name: provisional, minted 2026-11-10 (UTC) by lane milestone/1038-the-fuzz-host-replays-a-red-ci-shard.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
fail() { echo "fuzz-replay-selftest: $*" >&2; exit 1; }
work="$(mktemp -d "${TMPDIR:-/tmp}/fuzz-replay-selftest.XXXXXX")"
sup_pid=""
trap '[ -z "$sup_pid" ] || kill "$sup_pid" 2>/dev/null; rm -rf "$work"' EXIT INT TERM

GIT_CONFIG_NOSYSTEM=1
GIT_CONFIG_GLOBAL=/dev/null
STUB_API="$work/api"
STUB_CARGO="$work/cargo-calls"
export GIT_CONFIG_NOSYSTEM GIT_CONFIG_GLOBAL STUB_API STUB_CARGO
g() { git -c user.name=selftest -c user.email=selftest@invalid -c commit.gpgsign=false "$@"; }

# The clone: commit A carries the scripts, and HEAD is A. Commit S is on a side branch, which main
# never took.
tree="$work/clone"
bin="$work/home/.cargo/bin"
mkdir -p "$tree/helpers" "$bin" "$STUB_API"
for f in continuous-fuzz.sh fuzz-red-shards.py fuzz-follow-main.py rustup-retry.sh; do
    cp -p "$here/$f" "$tree/helpers/$f"
done
: > "$tree/helpers/qemu-path.sh"
cat > "$tree/helpers/qemu-bounded.sh" <<'STUB'
#!/bin/sh
shift
exec "$@"
STUB
# The stand-in guest: a CI range (below the supervisor's base) prints $STUB_GUEST, a slot's range
# prints a clean end line.
cat > "$tree/helpers/qemu-runner-x86_64.sh" <<'STUB'
#!/bin/sh
range="${NIFE_SYSCALL_FUZZ_SEEDS:-${NIFE_CONFINED_FUZZ_SEEDS:-}}"
if [ "${range%%:*}" -lt 10000000000 ]; then
    cat "$STUB_GUEST"
    exit "${STUB_GUEST_RC:-0}"
fi
sleep 1
echo "syscall fuzzer: 5 seeds, 1 ms (stub)"
echo "confined fuzzer: 5 seeds, 1 ms (stub)"
STUB
printf 'target/\n' > "$tree/.gitignore"
cat > "$bin/cargo" <<'STUB'
#!/bin/sh
echo "$*" >> "$STUB_CARGO"
case "$1" in
    xtask) mkdir -p target ;;
    test) echo '{"reason":"compiler-artifact","target":{"name":"system_tests"},"name":"system_tests","executable":"/stub/elf"}' ;;
esac
STUB
cat > "$bin/qemu-system-x86_64" <<'STUB'
#!/bin/sh
exit 0
STUB
cat > "$bin/rustup" <<'STUB'
#!/bin/sh
exit 0
STUB
cat > "$bin/curl" <<'STUB'
#!/bin/sh
for a; do url="$a"; done
case "$url" in
*/actions/workflows/ci.yml/runs*) cat "$STUB_API/runs.json" ;;
*/actions/runs/*/jobs*) id="${url#*/actions/runs/}"; cat "$STUB_API/jobs-${id%%/*}.json" ;;
*/rules/branches/main) cat "$STUB_API/rules.json" ;;
*/commits/*/check-runs*) sha="${url#*/commits/}"; sed "s/@SHA@/${sha%%/*}/g" "$STUB_API/check-runs.json" ;;
*) exit 22 ;;
esac
STUB
chmod +x "$tree"/helpers/*.sh "$tree"/helpers/*.py "$bin"/*
g -C "$tree" init --quiet --initial-branch=main
g -C "$tree" add -A
g -C "$tree" commit --quiet -m A
a="$(git -C "$tree" rev-parse HEAD)"
g -C "$tree" checkout --quiet -b side
g -C "$tree" commit --quiet --allow-empty -m S
s="$(git -C "$tree" rev-parse HEAD)"
g -C "$tree" checkout --quiet --detach "$a"

PATH="$bin:$PATH"
export PATH
fuzz="$work/fuzz"
replay() { # replay HOME COMMIT SHARD; output in $work/out
    : > "$STUB_CARGO"
    HOME="$work/home" NIFE_FUZZ_HOME="$1" NIFE_FUZZ_KVM_DEVICE=/dev/null \
        "$tree/helpers/continuous-fuzz.sh" --replay "$2" "$3" "$(($(date +%s) - 600))" > "$work/out" 2>&1 \
        || fail "--replay exited non-zero: $(cat "$work/out")"
}
# shellcheck disable=SC2046  # two words, split on purpose
set -- $(cd "$tree" && helpers/continuous-fuzz.sh --ci-range 0)
first0="$2"

# 1. A finding.
{
    echo "boot banner SECRETGUESTTEXT"
    echo "panicked at kernel/src/secret.rs:1:1: SECRETGUESTTEXT"
    echo "replay: NIFE_SYSCALL_FUZZ_SEEDS=$((first0 + 41)):1"
} > "$work/guest-finding"
STUB_GUEST="$work/guest-finding" STUB_GUEST_RC=1
export STUB_GUEST STUB_GUEST_RC
replay "$fuzz/held" "$a" 0
[ "$(wc -l < "$work/out" | tr -d ' ')" = 1 ] || fail "a finding printed more than one line: $(cat "$work/out")"
grep -qE '^continuous-fuzz: replay held finding [0-9a-f]{16} \(red 10m ago\)$' "$work/out" \
    || fail "a finding did not print a finding id and an age alone: $(cat "$work/out")"
grep -qE "SECRETGUESTTEXT|$first0|$((first0 + 41))|$a" "$work/out" && fail "a finding printed guest text, a seed or the commit"
held="$(ls "$fuzz/held/state/held")"
[ "$(echo "$held" | wc -l | tr -d ' ')" = 1 ] || fail "a finding left $(echo "$held" | wc -l) held logs"
echo "$held" | grep -qE "^[0-9]{8}T[0-9]{6}Z-$first0\.log$" || fail "the held log is not named as the filer reads it: $held"
id="$(sha256sum "$fuzz/held/state/held/$held" 2>/dev/null || shasum -a 256 "$fuzz/held/state/held/$held")"
grep -q "finding $(echo "$id" | cut -c1-16) " "$work/out" || fail "the finding id is not the held log's SHA-256"
row="$(cat "$fuzz/held/state/windows.tsv")"
[ "$(printf '%s\n' "$row" | awk -F'\t' 'NF == 8 && $2 == "kernel-thread" && $3 == "replay" && $7 == "held-finding"' | wc -l | tr -d ' ')" = 1 ] \
    || fail "a finding did not leave one held row: $row"
[ "$(printf '%s\n' "$row" | cut -f4)" = "$first0" ] || fail "the held row's first seed is not the shard's"
[ -z "$(ls -A "$fuzz/held/logs/replay")" ] || fail "a finding left a log outside the held directory"

# 2. A clean replay.
echo "syscall fuzzer: 500 seeds, 1000 ms (all clean)" > "$work/guest-clean"
STUB_GUEST="$work/guest-clean" STUB_GUEST_RC=0
replay "$fuzz/clean" "$a" 0
[ "$(cat "$work/out")" = "continuous-fuzz: replay of a red CI shard came back clean" ] || fail "a clean replay printed: $(cat "$work/out")"
[ -z "$(ls -A "$fuzz/clean/state/held")" ] || fail "a clean replay left a held log"
[ -z "$(ls -A "$fuzz/clean/logs/replay")" ] || fail "a clean replay left a log"
cut -f3,7 "$fuzz/clean/state/windows.tsv" | grep -qx "replay	replay-clean" || fail "a clean replay left no replay-clean row"

# 3. A commit main never took.
replay "$fuzz/side" "$s" 1
grep -q "replay refused" "$work/out" || fail "a commit off main was not refused: $(cat "$work/out")"
[ ! -s "$STUB_CARGO" ] || fail "a commit off main was built"
[ ! -e "$fuzz/side/state/windows.tsv" ] || fail "a commit off main left a row"

# 4. The listings. Run 11: shards green, a red job elsewhere. 12: shards skipped. 13: a red job named
# like no shard. 14: a red shard on a commit main never took. 15: a red shard 3 on main.
now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
runs() { for r; do
    # shellcheck disable=SC2086  # two words, split on purpose
    set -- $r
    printf '{"id":%s,"head_sha":"%s","event":"merge_group","status":"completed","head_branch":"gh-readonly-queue/main/pr-%s-x","created_at":"%s"},' "$1" "$2" "$1" "$now"
done; }
printf '{"total_count":5,"workflow_runs":[%s]}' "$(runs "11 $a" "12 $a" "13 $a" "14 $s" "15 $a" | sed 's/,$//')" > "$STUB_API/runs.json"
job() { printf '{"name":"%s","status":"completed","conclusion":"%s","head_sha":"%s","completed_at":"%s"}' "$1" "$2" "$3" "$now"; }
shard() { echo "fuzz-guests (shard $1, the in-guest drivers under KVM)"; }
printf '{"total_count":2,"jobs":[%s,%s]}' "$(job "$(shard 0)" success "$a")" "$(job "coverage (host crates)" failure "$a")" > "$STUB_API/jobs-11.json"
printf '{"total_count":1,"jobs":[%s]}' "$(job "$(shard 1)" skipped "$a")" > "$STUB_API/jobs-12.json"
printf '{"total_count":1,"jobs":[%s]}' "$(job "fuzz (parsers, 60s per target)" failure "$a")" > "$STUB_API/jobs-13.json"
printf '{"total_count":1,"jobs":[%s]}' "$(job "$(shard 2)" failure "$s")" > "$STUB_API/jobs-14.json"
printf '{"total_count":2,"jobs":[%s,%s]}' "$(job "$(shard 3)" failure "$a")" "$(job "$(shard 1)" success "$a")" > "$STUB_API/jobs-15.json"
cat > "$STUB_API/rules.json" <<'JSON'
[{"type":"required_status_checks","parameters":{"required_status_checks":[{"context":"lint","integration_id":null}]}}]
JSON
cat > "$STUB_API/check-runs.json" <<'JSON'
{"total_count":1,"check_runs":[{"name":"lint","status":"completed","conclusion":"success","head_sha":"@SHA@","app":{"slug":"github-actions","id":15368}}]}
JSON
e2e="$fuzz/e2e"
NIFE_FUZZ_HOME="$e2e" python3 -I "$tree/helpers/fuzz-red-shards.py" look > "$work/out" 2>&1 || fail "look queued nothing: $(cat "$work/out")"
[ "$(cut -d' ' -f1,2 "$e2e/state/replay-queue")" = "$a 3" ] || fail "the queue is not the one red shard on main: $(cat "$e2e/state/replay-queue")"
NIFE_FUZZ_HOME="$e2e" python3 -I "$tree/helpers/fuzz-red-shards.py" look > "$work/out" 2>&1 && fail "a second look queued again"
[ "$(wc -l < "$e2e/state/replay-queue" | tr -d ' ')" = 1 ] || fail "a second look grew the queue"

# 5. End to end: the supervisor replays the queue before any slot starts.
STUB_GUEST="$work/guest-finding" STUB_GUEST_RC=1
HOME="$work/home" NIFE_FUZZ_HOME="$e2e" NIFE_FUZZ_KVM_DEVICE=/dev/null NIFE_FUZZ_GUESTS=1 \
    NIFE_FUZZ_FOLLOW_EVERY=0 NIFE_FUZZ_REPLAY_EVERY=0 NIFE_RUSTUP_ATTEMPTS=1 NIFE_RUSTUP_BACKOFF=0 \
    "$tree/helpers/continuous-fuzz.sh" > "$work/supervisor.out" 2>&1 &
sup_pid=$!
deadline=$(($(date +%s) + 60))
while [ "$(date +%s)" -lt "$deadline" ]; do
    [ -f "$e2e/state/windows.tsv" ] && [ "$(wc -l < "$e2e/state/windows.tsv")" -ge 2 ] && break
    kill -0 "$sup_pid" 2>/dev/null || break
    sleep 1
done
kill "$sup_pid" 2>/dev/null || true
wait "$sup_pid" 2>/dev/null || true
sup_pid=""
[ "$(head -1 "$e2e/state/windows.tsv" | cut -f3,7)" = "replay	held-finding" ] \
    || fail "end to end: the first window is not the replay: $(cat "$e2e/state/windows.tsv" 2>/dev/null) $(tail -5 "$work/supervisor.out")"
sed -n 2p "$e2e/state/windows.tsv" | cut -f3 | grep -qx 0 || fail "end to end: no slot ran after the replay"
[ ! -s "$e2e/state/replay-queue" ] || fail "end to end: the queue did not empty"
grep -q "SECRETGUESTTEXT" "$work/supervisor.out" && fail "end to end: the supervisor printed guest text"

echo "fuzz-replay-selftest: ok"
