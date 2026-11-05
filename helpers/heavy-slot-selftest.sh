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
#   6. a holder that is pid 1 is found among its descendants: fails if the ancestor walk stops short
#      of pid 1. When `script/test` is a container's first process the slot records pid=1, and a
#      walk that never reached pid 1 left the script waiting for its own lock for ever (found
#      collecting the tenth pass). Hermetic: a real pid 1 needs a pid namespace, which an
#      unprivileged host cannot make, so the process table is the one thing stood in for.
#   7. waiters start in the order they began waiting, and a waiter killed while it waits holds
#      nobody up: fails if a freed slot goes to whichever waiter polls first (the ledger's order,
#      helpers/heavy_ledger.py) or if a dead waiter's ticket blocks the queue.
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
cp "$here/heavy-slot.sh" "$here/heavy_ledger.py" "$work/helpers/"
# The scratch copy polls five times faster than the real one (POLL_SECONDS is 2), so the stages that
# wait for a poll to notice something take a fraction of a second. Nothing but the interval differs;
# a source that no longer spells the constant this way fails here rather than testing at 2 s.
sed 's/^POLL_SECONDS = 2$/POLL_SECONDS = 0.2/' "$here/heavy_slot.py" >"$work/helpers/heavy_slot.py"
grep -q '^POLL_SECONDS = 0.2$' "$work/helpers/heavy_slot.py" \
    || { echo "heavy-slot-selftest: helpers/heavy_slot.py no longer spells POLL_SECONDS = 2" >&2; exit 1; }
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
# The message appears at the first poll. Then four more polls (0.2 s each): long enough that a
# second job not held back would have run, and that a waiting message printed per poll would have
# printed more than once.
i=0
until grep -q 'is waiting' "$work/second.err" 2>/dev/null; do
    i=$((i + 1))
    [ "$i" -le 100 ] || fail "the second job never said it was waiting"
    sleep 0.1
done
sleep 0.8
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

# Stage 6. The slot is locked through one descriptor and records pid=1; this process asks, through
# another descriptor, whether an ancestor holds a slot, with a process table in which its parent is
# a child of pid 1. It must say yes, and must still say no for a holder outside its family.
mkdir -p "$work/pid1-slots"
NIFE_HEAVY_SLOT_DIR="$work/pid1-slots" python3 - "$work/helpers" <<'PY' || fail "the pid 1 stage failed"
import fcntl, os, sys
sys.path.insert(0, sys.argv[1])
import heavy_slot
directory = os.environ["NIFE_HEAVY_SLOT_DIR"]
fd = os.open(os.path.join(directory, "slot-0"), os.O_RDWR | os.O_CREAT, 0o644)
fcntl.flock(fd, fcntl.LOCK_EX)
def write(pid):
    os.ftruncate(fd, 0)
    os.lseek(fd, 0, os.SEEK_SET)
    os.write(fd, ("pid=%d job=test worktree=/x started=now\n" % pid).encode())
heavy_slot.parent_table = lambda: {os.getppid(): 1, 1: 0}
write(1)
assert 1 in heavy_slot.ancestors(), "the ancestor walk stopped before pid 1"
assert heavy_slot.held_by_ancestor(1, directory), "a pid 1 holder was not found in the family"
write(4194000)
assert not heavy_slot.held_by_ancestor(1, directory), "a holder outside the family passed"
PY

# Stage 7. With the one slot held, a first waiter dies in line, then a second (which holds the slot
# once it has it) and a third (which runs and exits) join. When the holder dies, the second must
# start and the third must not: polling alone hands the slot to whichever asks first.
waiting_in() {
    i=0
    until grep -q 'is waiting' "$1" 2>/dev/null; do
        i=$((i + 1))
        [ "$i" -le 100 ] || fail "$2 never said it was waiting"
        sleep 0.1
    done
}
"$work/script/hold" "$work/order-holder" 60 &
holder=$!
appears "$work/order-holder" 10 || fail "the order stage's holder never started"
"$work/script/hold" "$work/order-dead" 60 2>"$work/order-dead.err" &
dead=$!
waiting_in "$work/order-dead.err" "the waiter that dies in line"
"$work/script/hold" "$work/order-first" 60 2>"$work/order-first.err" &
first=$!
waiting_in "$work/order-first.err" "the first live waiter"
kill -9 "$dead"
wait "$dead" 2>/dev/null || true
"$work/script/run" "$work/order-second" 2>"$work/order-second.err" &
second=$!
waiting_in "$work/order-second.err" "the second live waiter"
kill -9 "$holder"
wait "$holder" 2>/dev/null || true
holder=
appears "$work/order-first" 10 \
    || fail "the oldest live waiter did not start when the slot freed: $(cat "$work/order-first.err")"
sleep 0.8
[ ! -e "$work/order-second" ] || fail "a later waiter started while the slot was held"
kill -9 "$first"
wait "$first" 2>/dev/null || true
appears "$work/order-second" 10 || fail "the last waiter never started: $(cat "$work/order-second.err")"
wait "$second" || fail "the last waiter failed after taking the slot"
python3 "$work/helpers/heavy_ledger.py" --selftest >/dev/null || fail "the ledger's decision selftest failed"

# Stage 8. The per-machine count file sets the slots; the environment beats it; the file sits beside
# the slot directory (so a private slot directory reads a private file); a bad value is refused. A
# stuck-holder hint names a quiet old holder and not a busy or young one, and a slot whose worktree
# is gone is marked orphaned.
mkdir -p "$work/count/slots"
env -u NIFE_HEAVY_SLOTS NIFE_HEAVY_SLOT_DIR="$work/count/slots" python3 - "$work/helpers" "$work/count" <<'PY' || fail "the count file stage failed"
import os, sys
sys.path.insert(0, sys.argv[1])
import heavy_slot
beside = sys.argv[2]
assert heavy_slot.count_file() == os.path.join(beside, "heavy-slot-count"), heavy_slot.count_file()
default = heavy_slot.slot_count()
open(heavy_slot.count_file(), "w").write("10\n")
assert heavy_slot.slot_count() == 10, "the count file was not honored"
os.environ["NIFE_HEAVY_SLOTS"] = "3"
assert heavy_slot.slot_count() == 3, "the environment did not win over the file"
del os.environ["NIFE_HEAVY_SLOTS"]
os.remove(heavy_slot.count_file())
assert heavy_slot.slot_count() == default, "removing the file did not restore the default"
open(heavy_slot.count_file(), "w").write("many\n")
try:
    heavy_slot.slot_count()
    raise AssertionError("a bad count file was accepted")
except SystemExit:
    pass
quiet = {10: (1, 5.0, 7200.0), 11: (10, 0.0, 7000.0), 20: (1, 3000.0, 7200.0), 30: (1, 0.0, 60.0)}
rec = lambda pid: "pid=%d job=lint units=1 worktree=/x started=now" % pid
hint = heavy_slot.stuck_hint({0: rec(10), 1: rec(20), 2: rec(30)}, quiet)
assert "slot 0 (pid 10)" in hint and "slot 1" not in hint and "slot 2" not in hint, hint
assert heavy_slot.to_seconds("1-02:03:04") == 86400 + 7384 and heavy_slot.to_seconds("0:05.50") == 5.5
text = heavy_slot.describe({0: "pid=%d job=lint units=1 worktree=%s/gone started=now cmd=x" % (os.getpid(), beside)})
assert "ORPHANED" in text, text
PY

echo "heavy-slot-selftest: ok"
