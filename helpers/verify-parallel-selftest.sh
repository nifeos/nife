#!/bin/sh
#
# Selftest: script/verify --parallel forks the shards of one table, runs them inside the one heavy
# slot the parent holds, and refuses a suite verdict from anything but every child passing. Run by
# script/lint, and it needs no cargo and no Kani: a scratch tree holds a copy of script/verify, the
# three helpers it calls, a stub script/kani-install and a stub `cargo`, so the shard children are
# the real orchestration over stubbed proving. Milliseconds, no network, no slot of the machine's.
#
# What is proved, and what breaks each:
#
#   1. The partition: four children prove every crate of the table exactly once, and the parent says
#      so before it forks. Breaks on a packer change that lets a crate fall between two shards, or
#      on a fork loop that starts the wrong slice.
#   2. The install is ordered: script/kani-install runs first, before any child starts, and every
#      child calls it too (a no-op once Kani is at the pin, nifeos/nife#2882). Breaks if the parent
#      forks before installing, which is the race this design exists to remove.
#   3. Exit aggregation: one child failing exits the parent non-zero, names that shard, and prints no
#      pass line. Breaks on `wait` results being ignored, or a summary read as a verdict.
#   4. The one slot: with a one-slot fixture and four children, the run finishes. Breaks if the
#      children each try to take a slot: the parent holds the only one, so they would wait forever.
#   5. N from fixture counts: the default is the smaller of CI's shards and the fixture slot count,
#      and a fixture memory figure lowers it out loud. Breaks on a sizing rule that ignores either,
#      or that lowers a count in silence.
#   6. The refusals: --shard, --only, a stray argument and more children than crates are refused
#      before any fork. Each of those would make the run something other than the suite.
#
# The arithmetic itself is helpers/verify_parallel.py --selftest, run here against fixture memory
# and slot figures so the sizing rule is proved on any machine, whatever its own memory.

set -eu
here="$(cd "$(dirname "$0")" && pwd)"
me="verify-parallel-selftest"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_CONFIG_PARAMETERS

tmp="$(mktemp -d "${TMPDIR:-/tmp}/nife-verify-parallel-selftest.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
tmp="$(cd "$tmp" && pwd -P)"
tree="$tmp/tree"
log="$tmp/cargo.log"
mkdir -p "$tree/script" "$tree/helpers" "$tree/bin" "$tree/components/src"

cp "$here/../script/verify" "$tree/script/verify"
cp "$here/verify_times.py" "$here/verify_parallel.py" "$here/heavy_slot.py" "$tree/helpers/"
cp "$here/heavy-slot.sh" "$tree/helpers/heavy-slot.sh"

# The two stubs. kani-install says so on stdout, which only the parent's call reaches (the children's
# stdout goes to their logs), and in the shared log, which every call reaches. That is what lets the
# test see the parent install before it forks rather than after. `cargo` reports a version, records
# every proving call by its arguments, and fails when the test names a crate to fail.
printf '#!/bin/sh\nprintf "install\\n"\nprintf "install\\n" >>"$NIFE_SELFTEST_LOG"\n' \
	>"$tree/script/kani-install"
chmod +x "$tree/script/kani-install"
cat >"$tree/bin/cargo" <<'STUB'
#!/bin/sh
[ "$1" = kani ] && [ "$2" = "--version" ] && { echo "cargo-kani 0.68.0"; exit 0; }
printf '%s\n' "$*" >>"$NIFE_SELFTEST_LOG"
crate=""
prev=""
for a in "$@"; do
	[ "$prev" = "-p" ] && crate="$a"
	prev="$a"
done
if [ -n "${NIFE_SELFTEST_FAIL_CRATE:-}" ] && [ "$crate" = "$NIFE_SELFTEST_FAIL_CRATE" ]; then
	echo "stub cargo: failing $crate on purpose" >&2
	exit 1
fi
exit 0
STUB
chmod +x "$tree/bin/cargo"

# The fixture workflow, which is where the default child count reads CI's shard count. Four, to
# match the expectations below; the live file is read by helpers/verify_times.py --selftest.
mkdir -p "$tree/.github/workflows"
cat >"$tree/.github/workflows/verify.yml" <<'FIXTURE'
      - name: script/verify --shard ${{ matrix.shard }}/4
        run: script/verify --shard ${{ matrix.shard }}/4
FIXTURE

# `components` is selected by `git grep` for its harnesses, so the tree is a repository with one
# fixture program carrying a harness. Every other row is proved by the stub with no selection.
g() { git -C "$tree" -c core.hooksPath=/dev/null "$@"; }
g init -q
g config user.name selftest
g config user.email selftest@invalid
printf '// a fixture program with one harness\n#[kani::proof]\nfn proof() {}\n' \
	>"$tree/components/src/probe.rs"
g add -A

export NIFE_SELFTEST_LOG="$log"
export PATH="$tree/bin:$PATH"
NIFE_HEAVY_SLOT_DIR="$tmp/slots"
NIFE_HEAVY_SLOTS=8
NIFE_VERIFY_MEMORY_BYTES=$((128 * 1024 * 1024 * 1024))
export NIFE_HEAVY_SLOT_DIR NIFE_HEAVY_SLOTS NIFE_VERIFY_MEMORY_BYTES

fail() {
	echo "$me: $1" >&2
	shift
	for f in "$@"; do
		[ -f "$f" ] && sed 's/^/  | /' "$f" >&2
	done
	exit 1
}

run_verify() {
	(cd "$tree" && exec "$tree/script/verify" "$@")
}

# A deadline, because the failure this proves against is a deadlock (a child waiting for the slot its
# parent holds) and a deadlock never returns. macOS has no timeout(1). $1 is seconds.
run_bounded() {
	limit=$1
	shift
	"$@" &
	bg=$!
	n=0
	while kill -0 "$bg" 2>/dev/null; do
		if [ "$n" -ge $((limit * 5)) ]; then
			kill "$bg" 2>/dev/null || true
			wait "$bg" 2>/dev/null || true
			return 124
		fi
		n=$((n + 1))
		sleep 0.2
	done
	wait "$bg"
}

# The copy's crate table, as the TSV the packer eats, read through the helper that owns the format.
table_rows() {
	(cd "$tree" && python3 -c '
import sys
sys.path.insert(0, "helpers")
import verify_times
for name, secs, desc in verify_times.read_table(open("script/verify").read()):
    print("%s\t%d\t%s" % (name, secs, desc))
')
}

# The crates one shard of the copy's table holds, straight from script/verify's packer.
shard_crates() {
	(cd "$tree" && table_rows | python3 helpers/verify_times.py shard "$1/$2" 2>/dev/null)
}

# Every crate the children proved, one per line, sorted. Each child's log carries the `==> kani:`
# lines script/verify prints, so this reads what actually ran rather than what was planned.
proved_crates() {
	grep -h -o '^==> kani: [^ ]*' "$1"/shard-*-of-*.log | sed 's/^==> kani: //' | sort
}

latest_logdir() {
	ls -d "$tree"/target/verify-parallel/*/ 2>/dev/null | tail -n 1
}

# 0. The arithmetic, against fixture counts.
"$here/verify_parallel.py" --selftest >/dev/null ||
	fail "helpers/verify_parallel.py --selftest failed (the sizing rule, above)"

# 1 and 2. Four children, all passing: the partition, the install order, the summary.
: >"$log"
out="$tmp/pass.out"
run_verify --parallel 4 >"$out" 2>&1 ||
	fail "a four-child run over stubbed proving exited non-zero" "$out"
grep -q '^parallel: 4 shards partition the table' "$out" ||
	fail "the parent did not say its four shards partition the table" "$out"
[ "$(grep -c '^parallel: started shard ' "$out")" -eq 4 ] ||
	fail "the parent did not start four children" "$out"
grep -q '^==> verify --parallel: all 4 shards passed' "$out" ||
	fail "a passing four-child run printed no suite line" "$out"
dir="$(latest_logdir)"
[ -n "$dir" ] || fail "no log directory was written under target/verify-parallel"
[ "$(ls "$dir"/shard-*-of-4.log | wc -l | tr -d ' ')" -eq 4 ] ||
	fail "the four per-shard logs were not written" "$dir/summary.txt"
[ "$(grep -c -E '^shard [0-9]+/[0-9]+: (passed|FAILED)' "$dir/summary.txt")" -eq 4 ] ||
	fail "the summary does not hold one verdict line per child" "$dir/summary.txt"
if [ "$(proved_crates "$dir")" != "$(table_rows | cut -f1 | sort)" ]; then
	fail "the children did not prove every crate of the table exactly once" "$dir/summary.txt"
fi
# The parent's install is the first thing this run printed, and the children's are not in this file
# at all: their stdout went to their logs.
[ "$(head -n 1 "$out")" = install ] ||
	fail "the parent did not run script/kani-install before it forked (its first line is not an install)" "$out"
[ "$(grep -c '^install$' "$out")" -eq 1 ] ||
	fail "expected exactly one install on the parent's own output, found $(grep -c '^install$' "$out")" "$out"
# One install from the parent and one from each child, which is a no-op once Kani is at the pin.
[ "$(grep -c '^install$' "$log")" -eq 5 ] ||
	fail "expected 5 install calls (the parent, then each child), found $(grep -c '^install$' "$log")" "$log"

# 3. One child failing: the parent fails, names that shard, and claims no verdict.
fail_crate=glob
fail_shard=""
k=1
while [ "$k" -le 4 ]; do
	case " $(shard_crates "$k" 4) " in
	*" $fail_crate "*) fail_shard=$k ;;
	esac
	k=$((k + 1))
done
[ -n "$fail_shard" ] || fail "the packer put no shard of the table on $fail_crate, so the failure cannot be aimed"
: >"$log"
NIFE_SELFTEST_FAIL_CRATE="$fail_crate"
export NIFE_SELFTEST_FAIL_CRATE
out="$tmp/fail.out"
if run_verify --parallel 4 >"$out" 2>&1; then
	fail "a run whose shard $fail_shard/4 failed exited zero" "$out"
fi
unset NIFE_SELFTEST_FAIL_CRATE
grep -q "^parallel: NO SUITE VERDICT: shard(s) $fail_shard/4 failed" "$out" ||
	fail "the failure did not name shard $fail_shard/4 as the child that failed" "$out"
grep -q "^shard $fail_shard/4: FAILED (exit 1)" "$out" ||
	fail "the summary does not record shard $fail_shard/4 as failed (exit 1)" "$out"
if grep -q '^==> verify --parallel: all ' "$out"; then
	fail "a run with a failed child still printed a suite verdict" "$out"
fi

# 4. One heavy slot, four children: the run finishes, and the machine sees exactly one slot taken.
slot_run() {
	(
		unset LINT_NO_CARGO
		export NIFE_HEAVY_SLOTS=1 NIFE_HEAVY_SLOT_DIR="$tmp/slots-one"
		run_verify --parallel 4
	)
}
: >"$log"
out="$tmp/one-slot.out"
if ! run_bounded 120 slot_run >"$out" 2>&1; then
	fail "a four-child run under a one-slot fixture did not finish inside 120 s: the children waited for a slot their parent holds" "$out"
fi
grep -q '^==> verify --parallel: all 4 shards passed' "$out" ||
	fail "the one-slot run did not pass" "$out"
[ "$(ls "$tmp/slots-one" | wc -l | tr -d ' ')" -eq 1 ] ||
	fail "the one-slot run touched more than one slot file: $(ls "$tmp/slots-one" | tr '\n' ' ')"
grep -q 'job=verify' "$tmp/slots-one/slot-0" ||
	fail "the slot holder was not script/verify (slot record: $(cat "$tmp/slots-one/slot-0"))"

# 5. Default N from the fixture counts: two slots cap the default to two children, and a small
#    memory figure lowers an explicit count out loud.
: >"$log"
out="$tmp/two-slots.out"
NIFE_HEAVY_SLOTS=2
export NIFE_HEAVY_SLOTS
run_verify --parallel >"$out" 2>&1 ||
	fail "a default-count run with a two-slot fixture exited non-zero" "$out"
grep -q "4 shards and this machine's 2 heavy-job slots" "$out" ||
	fail "the default count did not read the fixture slot count" "$out"
[ "$(grep -c '^parallel: started shard ' "$out")" -eq 2 ] ||
	fail "a two-slot fixture did not bound the default to two children" "$out"
: >"$log"
out="$tmp/lowered.out"
NIFE_VERIFY_MEMORY_BYTES=$((16 * 1024 * 1024 * 1024))
export NIFE_VERIFY_MEMORY_BYTES
run_verify --parallel 4 >"$out" 2>&1 ||
	fail "a run lowered by the memory guard exited non-zero" "$out"
grep -q '^parallel: lowered from 4 to 1 children' "$out" ||
	fail "the memory guard lowered the count without saying so" "$out"
[ "$(grep -c '^parallel: started shard ' "$out")" -eq 1 ] ||
	fail "the lowered run did not start exactly one child" "$out"
NIFE_HEAVY_SLOTS=8
NIFE_VERIFY_MEMORY_BYTES=$((128 * 1024 * 1024 * 1024))
export NIFE_HEAVY_SLOTS NIFE_VERIFY_MEMORY_BYTES

# 6. The refusals: each exits non-zero before any fork, whatever it did before that.
refused() {
	label=$1
	shift
	: >"$log"
	if run_verify "$@" >"$tmp/refused.out" 2>&1; then
		fail "$label was accepted; the run is something other than the suite" "$tmp/refused.out"
	fi
	if grep -q '^parallel: started shard ' "$tmp/refused.out" || grep -q 'kani -p' "$log"; then
		fail "$label forked a child before refusing" "$tmp/refused.out"
	fi
}
refused "--parallel beside --shard" --shard 1/4 --parallel
refused "--parallel beside --only" --only capability --parallel
refused "--parallel with a stray argument" --parallel 4 --harness some_harness
refused "--parallel 0" --parallel 0
refused "a non-numeric count" --parallel two
# More children than crates is a packer refusal, and it is the parent's to make: a run of empty
# shards would prove nothing and report it as the suite.
NIFE_VERIFY_MEMORY_BYTES=$((1024 * 1024 * 1024 * 1024))
export NIFE_VERIFY_MEMORY_BYTES
refused "more children than crates" --parallel 99
NIFE_VERIFY_MEMORY_BYTES=$((128 * 1024 * 1024 * 1024))
export NIFE_VERIFY_MEMORY_BYTES

echo "verify-parallel selftest: the shards partition the table, run in one slot, and a failed child refuses the suite"
