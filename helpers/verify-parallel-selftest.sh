#!/bin/sh
# helpers/verify-parallel-selftest.sh: `script/verify --parallel` in milliseconds, over a stub
# `cargo` and a private slot directory, so no Kani runs and no real heavy-job slot is taken.
#
# Asserts the four things the mode exists to hold:
#   1. N computation: the default is the smaller of CI's shards and the fixture slot count, and a
#      memory figure too small for the ask lowers an explicit count, out loud.
#   2. Partition coverage: the N children prove every row of the crate table exactly once, and the
#      parent says its shards partition the table before it forks.
#   3. The single slot claim: the parent holds the one slot; the children run inside it and write no
#      slot record of their own.
#   4. Exit aggregation: one child failing fails the parent, which names that child and claims no
#      suite verdict.
#
# Run by script/lint.

set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/nife-verify-parallel-selftest.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

fail() { echo "verify-parallel-selftest: $*" >&2; exit 1; }
# While debugging: keep the workspace on failure instead of deleting it.
if [ -n "${NIFE_SELFTEST_KEEP:-}" ]; then
	fail() { echo "verify-parallel-selftest: $* (workspace kept at $tmp)" >&2; echo "$tmp"; exit 1; }
	trap - EXIT HUP INT TERM
fi

# The stub `cargo`: a version line for the pin check, one log line per call, and a failure on
# demand. `script/kani-install` reads that version, finds the pin, and exits 0, so the parent's
# install is a no-op and so is each child's, which is what nifeos/nife#2882 makes of them.
mkdir -p "$tmp/bin"
cat >"$tmp/bin/cargo" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >>"${NIFE_SELFTEST_LOG:?}"
case "$1 $2" in
  "kani --version") echo "Kani Rust Verifier 0.68.0 (cargo plugin)"; exit 0 ;;
esac
if [ -n "${NIFE_SELFTEST_FAIL_CRATE:-}" ] &&
   printf '%s\n' "$*" | grep -q -- "-p ${NIFE_SELFTEST_FAIL_CRATE} "; then
  echo "stub cargo: failing $NIFE_SELFTEST_FAIL_CRATE on purpose" >&2
  exit 1
fi
exit 0
STUB
chmod +x "$tmp/bin/cargo"
export PATH="$tmp/bin:$PATH"

# A private slot directory: the test never waits on a real job and never collides with one.
NIFE_HEAVY_SLOT_DIR="$tmp/slots"
export NIFE_HEAVY_SLOT_DIR
mkdir -p "$NIFE_HEAVY_SLOT_DIR"

# The hook's --no-cargo mode tells heavy scripts to take no slot, because a real wait there is idle
# time on the push connection git holds open. This test's parent takes a slot in the private
# directory above and never waits on a real job, so the mode is cleared here and the single-slot
# claim stays asserted on every path that runs this file.
unset LINT_NO_CARGO

log="$tmp/cargo.log"
export NIFE_SELFTEST_LOG="$log"

run() { "$root/script/verify" "$@"; }

# --- 1. N computation ----------------------------------------------------------------------
# CI runs 4 prove shards, read from verify.yml. A two-slot fixture caps the default at two children.
out="$tmp/auto.out"
: >"$log"
NIFE_HEAVY_SLOTS=2 NIFE_VERIFY_MEMORY_BYTES=$((128 * 1024 * 1024 * 1024)) \
	run --parallel >"$out" 2>&1 || fail "the default-count run exited non-zero"
grep -q "the smaller of CI's 4 shards and this machine's 2 heavy-job slots" "$out" ||
	fail "the default count did not read the fixture slot count" 
[ "$(grep -c 'started shard' "$out")" -eq 2 ] ||
	fail "a two-slot fixture did not bound the default to two children" "$out"

# A memory figure that cannot carry four children lowers an explicit four to one, and says so.
out="$tmp/lowered.out"
: >"$log"
NIFE_HEAVY_SLOTS=8 NIFE_VERIFY_MEMORY_BYTES=$((16 * 1024 * 1024 * 1024)) \
	run --parallel 4 >"$out" 2>&1 || fail "the lowered run exited non-zero"
grep -q "lowered from 4 to 1" "$out" ||
	fail "the memory guard lowered the count without saying so" "$out"
[ "$(grep -c 'started shard' "$out")" -eq 1 ] ||
	fail "the lowered run did not start exactly one child" "$out"

# --- 2 and 3. Partition coverage, and the single slot claim ---------------------------------
out="$tmp/pass.out"
: >"$log"
NIFE_HEAVY_SLOTS=4 NIFE_VERIFY_MEMORY_BYTES=$((128 * 1024 * 1024 * 1024)) \
	run --parallel 4 >"$out" 2>&1 || fail "the four-child run exited non-zero"
grep -q "4 shards partition the table" "$out" ||
	fail "the parent did not say its four shards partition the table" "$out"
[ "$(grep -c 'started shard' "$out")" -eq 4 ] ||
	fail "the parent did not start four children" "$out"
grep -q "all 4 shards passed" "$out" ||
	fail "a passing four-child run printed no suite line" "$out"

# Every row of the table, across the children's own logs, exactly once: the union is the table and
# never a subset, read from what ran rather than from what was planned.
logdir="$(sed -n 's|^parallel: logs in ||p' "$out")"
[ -n "$logdir" ] || fail "the parent printed no log directory"
proved="$(grep -h -o '^==> kani: [^ ]*' "$logdir"/shard-*-of-4.log | sed 's/^==> kani: //' | sort)"
expected="$(sed -n '/^CRATES="/,/^TABLE$/p' "$root/script/verify" | sed '1d;$d' | cut -f1 | sort)"
if [ "$proved" != "$expected" ]; then
	fail "the children did not prove every row of the crate table exactly once"
fi

# The parent holds the one slot; a child that took its own would have written a second record.
holders=0
for f in "$NIFE_HEAVY_SLOT_DIR"/slot-*; do
	[ -s "$f" ] && holders=$((holders + 1))
done
[ "$holders" -eq 1 ] ||
	fail "expected exactly one slot record (the parent's), found $holders"

# --- 4. Exit aggregation ---------------------------------------------------------------------
out="$tmp/fail.out"
: >"$log"
NIFE_SELFTEST_FAIL_CRATE=glob
export NIFE_SELFTEST_FAIL_CRATE
if run --parallel 4 >"$out" 2>&1; then
	fail "a run whose child failed exited zero" "$out"
fi
unset NIFE_SELFTEST_FAIL_CRATE
grep -q "NO SUITE VERDICT" "$out" ||
	fail "a run with a failed child printed no refusal" "$out"
grep -qE "^shard [0-9]+/4: FAILED" "$out" ||
	fail "the summary does not name the child that failed" "$out"

echo "verify-parallel selftest: N computation, the partition, one slot, and a named failed child"
