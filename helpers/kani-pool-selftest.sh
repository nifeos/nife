#!/bin/sh
# helpers/kani-pool-selftest.sh: the machine's Kani memory is bounded however many `script/verify`
# runs start at once, proved over a stub `cargo` that holds each "proof" for a moment.
#
# A fake memory size (NIFE_VERIFY_MEMORY_BYTES) fixes the Kani pool (helpers/heavy_slot.py, "The
# Kani pool"): the reserve plus a whole number of 2 GiB units. Stages 1 to 4 weigh a harness at one
# unit (NIFE_KANI_HARNESS_GIB=2) so the arithmetic reads plainly; stage 5 uses the default weight.
#
#   1. one unit per run, a two-unit pool: three concurrent runs never overlap more than two, and the
#      third prints that it waits. Fails if a run can start without its units.
#   2. weight: VERIFY_JOBS=2 on a four-unit pool admits two runs, not three. Fails if a run takes
#      one unit however many harnesses it runs at once.
#   3. clamp: VERIFY_JOBS=8 on a two-unit pool runs `cargo kani -j 2` and says it lowered the count.
#      Fails if the pool bounds the waiting but not the width.
#   4. GITHUB_ACTIONS: the pool is off, VERIFY_JOBS passes through and nothing waits (CI is
#      unchanged).
#   5. the default weight (10 GiB a harness): on a 34 GiB machine two runs fit and the third waits,
#      so the pool is sized for proofs far past the 3.5 GiB glob figure.
#   6. --status names the holder's lane, worktree and command line.
#
# Run by script/lint.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/nife-kani-pool-selftest.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
fail() { echo "kani-pool-selftest: $*" >&2; exit 1; }

mkdir -p "$tmp/bin"
cat >"$tmp/bin/cargo" <<'STUB'
#!/bin/sh
case "$1 $2" in
  "kani --version") echo "Kani Rust Verifier 0.68.0 (cargo plugin)"; exit 0 ;;
esac
printf '%s\n' "$*" >>"${NIFE_SELFTEST_ARGS:?}"
printf 'start %s\n' "$$" >>"${NIFE_SELFTEST_EVENTS:?}"
sleep 1
printf 'end %s\n' "$$" >>"${NIFE_SELFTEST_EVENTS:?}"
exit 0
STUB
chmod +x "$tmp/bin/cargo"
export PATH="$tmp/bin:$PATH"
unset LINT_NO_CARGO GITHUB_ACTIONS NIFE_KANI_UNITS NIFE_KANI_POOL VERIFY_JOBS
NIFE_HEAVY_SLOT_DIR="$tmp/slots"
NIFE_HEAVY_SLOTS=16
export NIFE_HEAVY_SLOT_DIR NIFE_HEAVY_SLOTS
mkdir -p "$tmp/slots"
export NIFE_SELFTEST_ARGS="$tmp/args" NIFE_SELFTEST_EVENTS="$tmp/events"
GIB=$((1024 * 1024 * 1024))
# The reserve is 12 GiB and a unit 2 GiB, so these sizes give pools of two and four units.
POOL2=$((16 * GIB))
POOL4=$((20 * GIB))
POOL11=$((34 * GIB))
export NIFE_KANI_HARNESS_GIB=2

# peak <events file>: the most proofs ever in flight at once.
peak() { awk '/^start/ { n++; if (n > m) m = n } /^end/ { n-- } END { print m + 0 }' "$1"; }

three() {
	: >"$tmp/events"
	for i in 1 2 3; do
		"$root/script/verify" --only glob >"$tmp/run$i.out" 2>&1 &
		pids="$pids $!"
		sleep 0.3
	done
	for p in $pids; do wait "$p" || fail "a run failed: $(cat "$tmp"/run*.out)"; done
}

# 1.
pids=""
NIFE_VERIFY_MEMORY_BYTES=$POOL2 VERIFY_JOBS=1 three
[ "$(peak "$tmp/events")" -eq 2 ] || fail "stage 1: peak $(peak "$tmp/events"), want 2 on a two-unit pool"
grep -q "waiting for 1 of 2 Kani units" "$tmp/run3.out" || fail "stage 1: the third run did not say it waits"
[ "$(grep -c start "$tmp/events")" -eq 3 ] || fail "stage 1: not all three runs proved"

# 2.
pids=""
NIFE_VERIFY_MEMORY_BYTES=$POOL4 VERIFY_JOBS=2 three
[ "$(peak "$tmp/events")" -eq 2 ] || fail "stage 2: peak $(peak "$tmp/events"), want 2 runs on four units at two each"
grep -q "waiting for 2 of 4 Kani units" "$tmp/run3.out" || fail "stage 2: the third run did not say it waits"

# 3.
: >"$tmp/args"
NIFE_VERIFY_MEMORY_BYTES=$POOL2 VERIFY_JOBS=8 "$root/script/verify" --only glob >"$tmp/clamp.out" 2>&1 ||
	fail "stage 3: the clamped run failed"
grep -q "VERIFY_JOBS=8 lowered to 2" "$tmp/clamp.out" || fail "stage 3: the lowering was silent"
grep -q -- "-j 2 " "$tmp/args" || fail "stage 3: cargo kani was not run at -j 2: $(cat "$tmp/args")"

# 4.
pids=""
GITHUB_ACTIONS=true NIFE_VERIFY_MEMORY_BYTES=$POOL2 VERIFY_JOBS=1 three
[ "$(peak "$tmp/events")" -eq 3 ] || fail "stage 4: the pool bounded a CI run"
! grep -q "Kani units" "$tmp"/run*.out || fail "stage 4: a CI run mentioned the pool"

# 5.
pids=""
NIFE_KANI_HARNESS_GIB=10 NIFE_VERIFY_MEMORY_BYTES=$POOL11 VERIFY_JOBS=1 three
[ "$(peak "$tmp/events")" -eq 2 ] || fail "stage 5: peak $(peak "$tmp/events"), want 2 at the default weight"
grep -q "waiting for 5 of 11 Kani units" "$tmp/run3.out" || fail "stage 5: the third run did not say it waits: $(cat "$tmp/run3.out")"

# 6. A holder in flight is named by lane, worktree and command.
: >"$tmp/events"
NIFE_VERIFY_MEMORY_BYTES=$POOL2 VERIFY_JOBS=1 "$root/script/verify" --only glob >/dev/null 2>&1 &
holder=$!
sleep 0.5
st="$(NIFE_VERIFY_MEMORY_BYTES=$POOL2 python3 "$root/helpers/heavy_slot.py" --status)"
wait "$holder"
printf '%s\n' "$st" | grep -q "lane " || fail "stage 6: --status names no lane: $st"
printf '%s\n' "$st" | grep -q "worktree $root" || fail "stage 6: --status names no worktree: $st"
printf '%s\n' "$st" | grep -q "command .*verify" || fail "stage 6: --status names no command: $st"

echo "kani-pool selftest: the third run waits, a run weighs its harnesses, the width is clamped, CI is untouched, the holder is named"
