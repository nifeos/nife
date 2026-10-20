#!/bin/sh
# helpers/heavy-slot-selftest.sh: the heavy-job slots hold a second job back, free a dead holder's
# slot, and let a nested job through without waiting on its own parent.
#
# A scratch tree carries copies of helpers/heavy-slot.sh and helpers/heavy_slot.py and two stand-in
# heavy scripts that source the former exactly as script/lint does, so the re-exec path is the one
# under test. Its slot directory is private, so this never contends with real jobs on the machine,
# and it runs with one slot. Each stage fails on its own way of breaking the lock:
#
#   1. the second job waits: fails if a slot can be taken while held (the lock does not exclude);
#   2. it says once who it waits on: fails if the holder's record is missing or repeated per poll;
#   3. SIGKILL of the holder frees the slot: fails if the lock outlives its process (a lock file or
#      directory with no cleanup, the shape a killed lane would otherwise leave stuck);
#   4. a nested job, with its environment scrubbed, runs at once and in place: fails if it takes or
#      waits for a second slot, which with one slot is a deadlock, or if it re-executes "$0";
#   5. under LINT_NO_CARGO a job runs while the slot is held: fails if `script/lint --no-cargo`'s
#      sections can wait on a slot again, which loses a push after its gates pass (#2478).
#
# Name: provisional, minted 2026-10-19 (UTC) by lane/heavy-job-slots.
set -eu
# This runs inside `script/lint --no-cargo`, whose LINT_NO_CARGO would let every stage's job through.
unset LINT_NO_CARGO

here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/heavy-slot-selftest.XXXXXX")"
holder=
cleanup() {
    [ -z "$holder" ] || kill -9 "$holder" 2>/dev/null || true
    pkill -9 -f "$work/" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT INT TERM

mkdir -p "$work/helpers" "$work/script" "$work/slots"
cp "$here/heavy-slot.sh" "$here/heavy_slot.py" "$work/helpers/"
NIFE_HEAVY_SLOT_DIR="$work/slots"
NIFE_HEAVY_SLOTS=1
export NIFE_HEAVY_SLOT_DIR NIFE_HEAVY_SLOTS

# $1 marks it started; then it sleeps $2 seconds as the same pid, so SIGKILL of $! kills the holder
# and nothing it started survives to keep the slot.
cat >"$work/script/hold" <<'EOF'
#!/bin/sh
. "$(dirname "$0")/../helpers/heavy-slot.sh"
: >"$1"
exec sleep "$2"
EOF
# Records that it ran, then (with a second argument) sources the slot again in a child shell whose
# "$0" names no file, the shape `script/lint --no-cargo` has, and under `env -i`, the way a tool
# that scrubs its children's environment would. Re-executing "$0" there would fail.
cat >"$work/script/run" <<'EOF'
#!/bin/sh
. "$(dirname "$0")/../helpers/heavy-slot.sh"
: >"$1"
if [ $# -gt 1 ]; then
    env -i PATH="$PATH" NIFE_HEAVY_SLOT_DIR="$NIFE_HEAVY_SLOT_DIR" NIFE_HEAVY_SLOTS=1 \
        sh -c '. "$(dirname "$0")/../helpers/heavy-slot.sh"; : >"$1"' "$(dirname "$0")/absent" "$2"
fi
EOF
chmod +x "$work/script/hold" "$work/script/run"

fail() {
    echo "heavy-slot-selftest: $*" >&2
    exit 1
}

# Polls for a file for up to $2 seconds.
appears() {
    i=0
    while [ ! -e "$1" ]; do
        i=$((i + 1))
        [ "$i" -le $(($2 * 10)) ] || return 1
        sleep 0.1
    done
}

"$work/script/hold" "$work/held" 60 &
holder=$!
appears "$work/held" 10 || fail "the holder never started"

"$work/script/run" "$work/second-ran" 2>"$work/second.err" &
second=$!
# Three polls' worth: long enough that a second job not held back would have run, and that a
# waiting message printed per poll would have printed more than once.
sleep 4.5
[ ! -e "$work/second-ran" ] || fail "a second job ran while the only slot was held"
grep -q "pid=$holder job=hold" "$work/second.err" \
    || fail "the waiting job did not name the holder; it printed: $(cat "$work/second.err")"
[ "$(grep -c 'is waiting' "$work/second.err")" -eq 1 ] \
    || fail "the waiting message was not printed exactly once: $(cat "$work/second.err")"

LINT_NO_CARGO=1 "$work/script/run" "$work/no-cargo-ran" 2>"$work/no-cargo.err" &
appears "$work/no-cargo-ran" 10 \
    || fail "a job under LINT_NO_CARGO waited for the held slot: $(cat "$work/no-cargo.err")"

kill -9 "$holder"
wait "$holder" 2>/dev/null || true
holder=
appears "$work/second-ran" 10 || fail "the slot stayed held after its holder was killed"
wait "$second" || fail "the second job failed after taking the freed slot"

"$work/script/run" "$work/outer-ran" "$work/inner-ran" 2>"$work/nested.err" &
outer=$!
appears "$work/inner-ran" 10 \
    || fail "a nested job did not run inside its parent's slot: $(cat "$work/nested.err")"
wait "$outer" || fail "the nested run failed: $(cat "$work/nested.err")"
! grep -q 'is waiting' "$work/nested.err" || fail "a nested job waited: $(cat "$work/nested.err")"

echo "heavy-slot-selftest: ok"
