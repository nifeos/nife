#!/bin/sh
#
# Prove that `helpers/qemu-bounded.sh` still does the four things it exists to do.
#
#     helpers/qemu-bounded-selftest.sh                 test the sibling script
#     helpers/qemu-bounded-selftest.sh path/to/other   test a specific copy (e.g. an old one)
#
# **The name is provisional** (the lane of milestone 226 (`qemu-bounded.sh` leaves an emulator
# behind);
# naming is an architect's call).
#
# # Why a script rather than a paragraph
#
# Every property here is one somebody already lost time to, and every one of them is invisible
# to a reader of the diff. That a bound fires is easy to believe and easy to check. That a
# detached killer survives a `head` which exited, that an orphaned `sleep` does not hold a pipe
# open to the end of the bound, that `perl -e 'alarm'` still cannot bound QEMU: each of those
# was found by a lane losing an afternoon, and each would come back silently under a plausible
# simplification of the script. This is the difference between the properties being tested and
# the properties being remembered.
#
# It is **not wired into any gate**, and that is deliberate rather than an omission. It starts
# real emulators and spends about a minute doing it, so putting it in `script/lint` would tax
# every lane for a script that changes twice a year. Run it when you touch `qemu-bounded.sh`.
#
# # What each case is
#
# 1. **status and promptness.** A child that exits on its own returns its own status, at once.
# 2. **the bound fires on a real QEMU.** The original job.
# 3. **`perl -e 'alarm'` still does not.** The constraint the whole script exists for, checked
#    against the QEMU actually installed rather than against the note that records it.
# 4. **an orphan is not left when the wrapper is killed** (milestone 226). Both the signalled
#    case, where the killer's trap fires, and the SIGKILLed-wrapper case, where only the poll
#    can save it.
# 5. **a pipeline whose reader exits early still gets its emulator killed.** The reason the
#    killer is detached at all.
# 6. **a fast child in a pipeline does not block to the end of the bound** (milestone 38).
# 7. **a failure names who holds the disk image** (milestone 226).
# 8. **and does not name a mere reader**, which is what made the first version of that diagnostic
#    fire on a green `script/test`.
# 9. **the bound reaches an emulator that is not the child** (2026-09-24). `cargo xtask shell` and
#    the x86_64 runner both put QEMU below the process this script starts, and signalling only
#    that process left QEMU under pid 1 every time.
#
# # BUGS
#
# **It cannot test the case that has no fix.** A SIGKILL delivered to the killer as well as the
# wrapper orphans the emulator, and this script does not assert on that because asserting on it
# would only pin a known defect in place. `qemu-bounded.sh`'s own BUGS section records it.
#
# **It finds processes by a `-name` marker containing its own pid**, which is unique in practice
# and not by construction. It kills only by pid, and never anything it did not start, so a
# collision costs a wrong verdict rather than another lane's emulator.
#
# **aarch64 only.** The script under test is architecture-blind, so this checks the property on
# the emulator the dev machine runs. A riscv64 or x86_64 QEMU would exercise the same signal
# handling.

set -e

BOUNDED="${1:-$(dirname "$0")/qemu-bounded.sh}"
QEMU=qemu-system-aarch64
FAILURES=0
TMP="${TMPDIR:-/tmp}/qemu-bounded-selftest.$$"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"' EXIT

if ! command -v "$QEMU" >/dev/null 2>&1; then
    echo "selftest: $QEMU not installed; nothing to test." >&2
    exit 0
fi

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; FAILURES=$((FAILURES + 1)); }

# The pid of the emulator carrying this marker, and only that: `pgrep -f` also matches the
# wrapper and the killer, whose argv contains the marker too.
emulator_pid() {
    pids="$(pgrep -f "$1" 2>/dev/null | tr '\n' ' ')"
    [ -n "$pids" ] || return 0
    ps -o pid=,comm= -p $pids 2>/dev/null | grep qemu-system | head -1 | awk '{print $1}'
}

# A QEMU that runs forever and never exits on its own, which is what every case here needs.
# With no `-kernel` and no firmware it simply idles.
idle_qemu() { echo "$QEMU -M virt -nographic -name $1"; }

echo "==> 1. a child that exits on its own keeps its status, and does not wait out the bound"
started="$(date +%s)"
set +e
"$BOUNDED" 30 sh -c 'exit 7' >/dev/null 2>&1
status=$?
set -e
elapsed=$(( $(date +%s) - started ))
if [ "$status" -eq 7 ] && [ "$elapsed" -lt 5 ]; then
    pass "status $status after ${elapsed}s"
else
    fail "expected status 7 in under 5s, got $status in ${elapsed}s"
fi

echo "==> 2. the bound fires on a real QEMU"
mark="qbst2-$$"
started="$(date +%s)"
# shellcheck disable=SC2046
"$BOUNDED" 5 $(idle_qemu "$mark") >/dev/null 2>&1 || true
elapsed=$(( $(date +%s) - started ))
sleep 1
if [ -z "$(emulator_pid "$mark")" ] && [ "$elapsed" -ge 4 ] && [ "$elapsed" -lt 15 ]; then
    pass "emulator gone, ${elapsed}s for a 5s bound"
else
    fail "emulator survived a 5s bound, or the bound took ${elapsed}s"
    pkill -f "$mark" 2>/dev/null || true
fi

echo "==> 3. perl's alarm still cannot bound QEMU (why this script exists)"
mark="qbst3-$$"
# shellcheck disable=SC2046
perl -e 'alarm 3; exec @ARGV' $(idle_qemu "$mark") >/dev/null 2>&1 &
perlpid=$!
sleep 8
if kill -0 "$perlpid" 2>/dev/null; then
    pass "a 3s alarm did not stop it after 8s; QEMU still swallows SIGALRM"
    kill -TERM "$perlpid" 2>/dev/null || true
else
    fail "the alarm worked, which contradicts this script's whole premise; re-read nifeos/record:notes/qemu.md"
fi
sleep 1
pkill -f "$mark" 2>/dev/null || true

# Kill the wrapper and see whether the emulator outlives it. `signal` goes to the wrapper;
# `kill_killer` says whether the detached killer is signalled too, which is what `pkill -f`
# does and what a dying process group does.
orphan_case() {
    signal="$1"
    kill_killer="$2"
    label="$3"
    mark="qbst4-$signal$kill_killer-$$"
    # shellcheck disable=SC2046
    "$BOUNDED" 120 $(idle_qemu "$mark") >/dev/null 2>&1 &
    wrapper=$!
    sleep 3
    qpid="$(emulator_pid "$mark")"
    if [ -z "$qpid" ]; then
        fail "$label: the emulator never started"
        return 0
    fi
    if [ "$kill_killer" = yes ]; then
        for kid in $(pgrep -P "$wrapper" 2>/dev/null); do
            [ "$kid" = "$qpid" ] && continue
            kill -"$signal" "$kid" 2>/dev/null || true
        done
    fi
    kill -"$signal" "$wrapper" 2>/dev/null || true
    n=0
    while [ "$n" -lt 15 ]; do
        kill -0 "$qpid" 2>/dev/null || { pass "$label: reaped ${n}s after the wrapper died"; return 0; }
        sleep 1
        n=$((n + 1))
    done
    fail "$label: emulator $qpid alive 15s later, ppid=$(ps -o ppid= -p "$qpid" | tr -d ' ')"
    kill -KILL "$qpid" 2>/dev/null || true
}

echo "==> 4. an emulator does not outlive the wrapper that started it"
orphan_case TERM yes "wrapper and killer both signalled (pkill -f)"
orphan_case KILL no "wrapper SIGKILLed, only the poll can notice"

echo "==> 5. a pipeline whose reader exits early still gets its emulator killed"
mark="qbst5-$$"
started="$(date +%s)"
# `-d exec -D /dev/stdout` over a zeroed BIOS is a real QEMU that writes continuously and
# never exits, which is what makes `head` exit early and the emulator keep going.
"$BOUNDED" 8 "$QEMU" -M virt -nographic -bios /dev/zero -name "$mark" -d exec -D /dev/stdout 2>/dev/null | head -5 >/dev/null || true
elapsed=$(( $(date +%s) - started ))
sleep 2
if [ -z "$(emulator_pid "$mark")" ]; then
    pass "emulator reaped after the reader exited (${elapsed}s for an 8s bound)"
else
    fail "emulator survived the reader exiting"
    pkill -f "$mark" 2>/dev/null || true
fi

echo "==> 6. a fast child in a pipeline does not hold the pipe to the end of the bound"
started="$(date +%s)"
"$BOUNDED" 30 sh -c 'echo fast' 2>/dev/null | cat >/dev/null
elapsed=$(( $(date +%s) - started ))
if [ "$elapsed" -lt 5 ]; then
    pass "pipeline returned in ${elapsed}s against a 30s bound"
else
    fail "pipeline took ${elapsed}s against a 30s bound; something is holding the pipe"
fi

echo "==> 7. a failure says who has the disk image open"
mark="qbst7-$$"
img="$TMP/held.img"
dd if=/dev/zero of="$img" bs=1048576 count=4 2>/dev/null
"$BOUNDED" 25 "$QEMU" -M virt -nographic -name "$mark" \
    -drive "if=none,file=$img,format=raw,id=d0" -device virtio-blk-device,drive=d0 >/dev/null 2>&1 &
holder=$!
sleep 3
diag="$("$BOUNDED" 10 "$QEMU" -M virt -nographic -name "$mark-second" \
    -drive "if=none,file=$img,format=raw,id=d1" -device virtio-blk-device,drive=d1 2>&1 >/dev/null || true)"
if echo "$diag" | grep -q "is still open by another process"; then
    pass "the lock failure named the holder"
else
    fail "a lock failure named no holder; got: $diag"
fi
kill -TERM "$holder" 2>/dev/null || true
sleep 3
pkill -f "$mark" 2>/dev/null || true

echo "==> 8. a reader is not reported, because the lock is about writing"
img="$TMP/read-only.img"
dd if=/dev/zero of="$img" bs=1048576 count=1 2>/dev/null
# A plain reader, which is what `script/test` has: its three legs share one read-only OVMF
# firmware file, so before this filter every green run that overlapped another lane reported a
# holder and was wrong to. The command is a failing `sh` rather than a QEMU because the argument
# scan does not care what ran, only that it failed and named a path.
sh -c 'exec 3<"$1"; sleep 20' sh "$img" &
reader=$!
sleep 1
diag="$("$BOUNDED" 5 sh -c 'exit 1' sh "$img" 2>&1 >/dev/null || true)"
if echo "$diag" | grep -q "is still open by another process"; then
    fail "a read-only holder was reported: $diag"
else
    pass "a read-only holder is not reported"
fi
kill -TERM "$reader" 2>/dev/null || true

echo "==> 9. the bound reaches an emulator the child started rather than exec'd"
mark="qbst9-$$"
# `; true` keeps `sh -c` from exec'ing QEMU, so the emulator is a grandchild of the wrapper.
"$BOUNDED" 5 sh -c "$(idle_qemu "$mark"); true" >/dev/null 2>&1 || true
sleep 3
if [ -z "$(emulator_pid "$mark")" ]; then
    pass "a grandchild emulator was reaped with the bound"
else
    fail "a grandchild emulator outlived the bound, ppid=$(ps -o ppid= -p "$(emulator_pid "$mark")" | tr -d ' ')"
    pkill -f "$mark" 2>/dev/null || true
fi

echo
if [ "$FAILURES" -eq 0 ]; then
    echo "qemu-bounded-selftest: all cases pass ($BOUNDED)"
else
    echo "qemu-bounded-selftest: $FAILURES case(s) failed ($BOUNDED)" >&2
fi
exit "$FAILURES"
