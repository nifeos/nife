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
#   7. a measured row (helpers/kani_peaks.py) weighs its peak rounded up to whole units, in a scratch
#      git repository: a row never measured, a crate edited and not committed, a crate committed
#      since, an untracked file in it and a new Kani version each weigh the default instead. Fails if
#      a weight outlives the code it measured.
#   8. script/verify --only weighs its own row: a fixture table gives `glob` one unit, so
#      VERIFY_JOBS=2 on a four-unit pool admits two runs and the third waits for two units. The
#      row's path is .kani-version, a file no lane edits in passing. Fails if the measurement does
#      not reach the run.
#   9. a run waiting for the pool holds no heavy slot: with two slots and a pool that fits one run, a
#      holder and two waiting runs leave a slot free, and a job that needs only a slot starts while
#      they wait. Fails if a waiting run holds a slot it does nothing with, which on 2026-11-14 held
#      13 of a host's 15 slots while lint and test jobs queued behind them.
#  10. a run waiting for a slot holds no Kani units: with the one slot taken, a run that could have
#      its units waits for the slot with none held. Fails if a run holds units while it waits for a
#      slot, which would deadlock against a job that holds a slot and waits for units.
#
# No stage times a race. The stub `cargo` holds each "proof" until the selftest creates a release
# file, and the selftest creates it only after the observation it is about to assert has happened:
# the third run printed that it waits (stages 1, 2, 5 and 8), all three runs are in flight (stage 4)
# or the holder is in flight (stage 6). A fixed hold time instead failed about 1 run in 12 with eight
# copies and eight busy loops on one machine, when the third run started its pool request after an
# earlier proof had already ended.
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
# Hold the proof, and so its units, until the selftest releases it. The bound keeps a selftest that
# died from leaving the stub behind; it is a cleanup limit, never a pause the assertions rely on.
waited=0
while [ ! -e "${NIFE_SELFTEST_GO:?}" ] && [ "$waited" -lt 1200 ]; do
	sleep 0.1
	waited=$((waited + 1))
done
printf 'end %s\n' "$$" >>"${NIFE_SELFTEST_EVENTS:?}"
exit 0
STUB
chmod +x "$tmp/bin/cargo"
export PATH="$tmp/bin:$PATH"
# script/kani-install, which script/verify runs, repairs a Kani toolchain link that does not resolve
# by running `cargo kani setup`, which the stub would count as a proof. A temporary HOME whose link
# resolves keeps the runs to the proofs, on a machine that has no Kani installed as on one that has.
mkdir -p "$tmp/home/tc/bin"
: >"$tmp/home/tc/bin/rustc"
chmod +x "$tmp/home/tc/bin/rustc"
mkdir -p "$tmp/home/.kani/kani-$(tr -d '[:space:]' <"$root/.kani-version")"
ln -s "$tmp/home/tc" "$tmp/home/.kani/kani-$(tr -d '[:space:]' <"$root/.kani-version")/toolchain"
export HOME="$tmp/home"
unset LINT_NO_CARGO GITHUB_ACTIONS NIFE_KANI_UNITS NIFE_KANI_POOL VERIFY_JOBS
NIFE_HEAVY_SLOT_DIR="$tmp/slots"
NIFE_HEAVY_SLOTS=16
export NIFE_HEAVY_SLOT_DIR NIFE_HEAVY_SLOTS
mkdir -p "$tmp/slots"
export NIFE_SELFTEST_ARGS="$tmp/args" NIFE_SELFTEST_EVENTS="$tmp/events" NIFE_SELFTEST_GO="$tmp/go"
GIB=$((1024 * 1024 * 1024))
# The reserve is 12 GiB and a unit 2 GiB, so these sizes give pools of two and four units.
POOL2=$((16 * GIB))
POOL4=$((20 * GIB))
POOL11=$((34 * GIB))
export NIFE_KANI_HARNESS_GIB=2

# peak <events file>: the most proofs ever in flight at once.
peak() { awk '/^start/ { n++; if (n > m) m = n } /^end/ { n-- } END { print m + 0 }' "$1"; }

# until_true <what> <command...>: poll for up to 2 minutes until the command succeeds, and fail naming
# <what> when it never does.
until_true() {
	what="$1"
	shift
	n=0
	until "$@"; do
		n=$((n + 1))
		[ "$n" -lt 1200 ] || fail "timed out waiting for $what"
		sleep 0.1
	done
}
starts() { [ "$(grep -c '^start' "$tmp/events")" -ge "$1" ]; }
# run3_decided: the third run either printed that it waits, or got its units and started a proof.
run3_decided() { grep -q "waiting for" "$tmp/run3.out" || starts 3; }

# three: start three `script/verify --only glob` runs one at a time, each only once the one before
# it holds a proof, hold all of them until the third has either said it waits or started, then
# release them and join. A third run that starts without waiting still ends the stage, and the
# stage's own assertion then fails on what it saw.
three() {
	rm -f "$tmp/go"
	: >"$tmp/events"
	for i in 1 2 3; do
		"$root/script/verify" --only byte_glob >"$tmp/run$i.out" 2>&1 &
		pids="$pids $!"
		if [ "$i" -lt 3 ]; then until_true "run $i to start its proof" starts "$i"; fi
	done
	if [ "${GITHUB_ACTIONS:-}" = true ]; then
		until_true "all three runs to start" starts 3
	else
		until_true "the third run to wait or start" run3_decided
	fi
	: >"$tmp/go"
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
: >"$tmp/go"
NIFE_VERIFY_MEMORY_BYTES=$POOL2 VERIFY_JOBS=8 "$root/script/verify" --only byte_glob >"$tmp/clamp.out" 2>&1 ||
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
rm -f "$tmp/go"
: >"$tmp/events"
NIFE_VERIFY_MEMORY_BYTES=$POOL2 VERIFY_JOBS=1 "$root/script/verify" --only byte_glob >/dev/null 2>&1 &
holder=$!
until_true "the holder to start its proof" starts 1
st="$(NIFE_VERIFY_MEMORY_BYTES=$POOL2 python3 "$root/helpers/heavy_slot.py" --status)"
: >"$tmp/go"
wait "$holder"
printf '%s\n' "$st" | grep -q "lane " || fail "stage 6: --status names no lane: $st"
printf '%s\n' "$st" | grep -q "worktree $root" || fail "stage 6: --status names no worktree: $st"
printf '%s\n' "$st" | grep -q "command .*verify" || fail "stage 6: --status names no command: $st"

# 7.
python3 - "$root/helpers" "$tmp/repo" <<'PY' || fail "stage 7: a measured weight did not follow its code"
import os, subprocess, sys
sys.path.insert(0, sys.argv[1])
import kani_peaks
repo = sys.argv[2]
os.makedirs(os.path.join(repo, "crates/a/src"))


def sh(*args):
    subprocess.run(["git", "-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", *args],
                   check=True, capture_output=True)


def write(path, text):
    with open(os.path.join(repo, path), "w") as f:
        f.write(text)


def record():
    tree = kani_peaks.trees(repo, ["crates/a"])["crates/a"]
    kani_peaks.save({"a": {"row": "a", "path": "crates/a", "tree": tree, "kani": "0.68.0",
                           "peak_mib": 3000}})


def check(stage, want, why):
    got = kani_peaks.weights(["a", "b"], 2, 5, repo)
    assert got["b"] == (5, "not measured"), (stage, got)
    assert got["a"][0] == want and why in got["a"][1], (stage, got["a"])


write("crates/a/src/lib.rs", "pub fn a() {}\n")
write(".kani-version", "0.68.0\n")
sh("init", "-q")
sh("add", "-A")
sh("commit", "-q", "-m", "a")
os.environ["NIFE_KANI_PEAKS"] = os.path.join(repo, "peaks.tsv")
record()
check("fresh", 2, "measured")
write("crates/a/src/lib.rs", "pub fn a() { loop {} }\n")
check("edited", 5, "local changes")
sh("commit", "-qam", "edit")
check("committed", 5, "changed since")
record()
check("measured again", 2, "measured")
write("crates/a/src/new.rs", "\n")
check("untracked", 5, "local changes")
os.remove(os.path.join(repo, "crates/a/src/new.rs"))
write(".kani-version", "0.69.0\n")
check("new Kani", 5, "Kani 0.68.0")
PY

# 8.
[ -z "$(git -C "$root" status --porcelain -- .kani-version)" ] ||
	fail "stage 8: .kani-version has local changes, so the fixture row cannot be fresh; commit it first"
printf 'row\tpath\ttree\tkani\tpeak_mib\nbyte_glob\t.kani-version\t%s\t%s\t1500\n' \
	"$(git -C "$root" rev-parse HEAD:.kani-version)" "$(tr -d '[:space:]' <"$root/.kani-version")" >"$tmp/peaks.tsv"
pids=""
unset NIFE_KANI_HARNESS_GIB
NIFE_KANI_PEAKS="$tmp/peaks.tsv" NIFE_VERIFY_MEMORY_BYTES=$POOL4 VERIFY_JOBS=2 three
[ "$(peak "$tmp/events")" -eq 2 ] || fail "stage 8: peak $(peak "$tmp/events"), want 2 runs of two one-unit harnesses on four units"
grep -q "waiting for 2 of 4 Kani units" "$tmp/run3.out" ||
	fail "stage 8: the third run did not wait for its measured two units: $(cat "$tmp/run3.out")"

# 9.
rm -f "$tmp/go"
: >"$tmp/events"
small() { NIFE_HEAVY_SLOTS="$1" NIFE_KANI_HARNESS_GIB=2 NIFE_VERIFY_MEMORY_BYTES=$POOL2 VERIFY_JOBS=2 "$root/script/verify" --only glob; }
small 2 >"$tmp/s9-holder.out" 2>&1 &
pids="$!"
until_true "the stage 9 holder to start its proof" starts 1
for i in 1 2; do
	small 2 >"$tmp/s9-wait$i.out" 2>&1 &
	pids="$pids $!"
	until_true "stage 9 waiter $i to wait" grep -q "waiting for" "$tmp/s9-wait$i.out"
done
NIFE_HEAVY_SLOTS=2 python3 "$root/helpers/heavy_slot.py" lint -- sh -c ": >'$tmp/s9-lint'" >"$tmp/s9-lint.out" 2>&1 &
pids="$pids $!"
# Bounded so a failure still releases the runs below; a free slot is taken within one 2 s poll.
n=0
while [ ! -e "$tmp/s9-lint" ] && [ "$n" -lt 300 ]; do sleep 0.1; n=$((n + 1)); done
st9="$(NIFE_HEAVY_SLOTS=2 NIFE_VERIFY_MEMORY_BYTES=$POOL2 python3 "$root/helpers/heavy_slot.py" --status)"
ended9="$(grep -c '^end' "$tmp/events" || true)"
lint9=no
if [ -e "$tmp/s9-lint" ]; then lint9=yes; fi
: >"$tmp/go"
for p in $pids; do wait "$p" || fail "stage 9: a job failed: $(cat "$tmp"/s9-*.out)"; done
[ "$lint9" = yes ] || fail "stage 9: a job needing only a heavy slot waited behind runs waiting for the Kani pool: $st9"
[ "$ended9" -eq 0 ] || fail "stage 9: the holder finished before the lint started, so the stage proved nothing"
grep -q "waiting for 2 of 2 Kani units" "$tmp/s9-wait1.out" "$tmp/s9-wait2.out" ||
	fail "stage 9: the waiting runs did not wait for the pool"
printf '%s\n' "$st9" | grep -q "^2 heavy-job slots in .*, 1 held$" ||
	fail "stage 9: runs waiting for the pool hold heavy slots: $st9"
[ "$(grep -c '^start' "$tmp/events")" -eq 3 ] || fail "stage 9: not all three runs proved"

# 10.
rm -f "$tmp/go"
: >"$tmp/events"
NIFE_HEAVY_SLOTS=1 python3 "$root/helpers/heavy_slot.py" lint -- sh -c \
	": >'$tmp/s10-lint'; n=0; while [ ! -e '$tmp/go' ] && [ \$n -lt 1200 ]; do sleep 0.1; n=\$((n + 1)); done" \
	>"$tmp/s10-lint.out" 2>&1 &
pids="$!"
until_true "the stage 10 slot holder to start" test -e "$tmp/s10-lint"
small 1 >"$tmp/s10-run.out" 2>&1 &
pids="$pids $!"
until_true "the stage 10 run to wait for a slot" grep -q "waiting for one of 1 heavy-job slots" "$tmp/s10-run.out"
st10="$(NIFE_HEAVY_SLOTS=1 NIFE_VERIFY_MEMORY_BYTES=$POOL2 python3 "$root/helpers/heavy_slot.py" --status)"
: >"$tmp/go"
for p in $pids; do wait "$p" || fail "stage 10: a job failed: $(cat "$tmp"/s10-*.out)"; done
printf '%s\n' "$st10" | grep -q "^2 Kani units of 2 GiB in .*, 0 held;" ||
	fail "stage 10: a run waiting for a heavy slot holds Kani units: $st10"
[ "$(grep -c '^start' "$tmp/events")" -eq 1 ] || fail "stage 10: the run did not prove once the slot was free"

echo "kani-pool selftest: the third run waits, a run weighs its harnesses, the width is clamped, CI is untouched, the holder is named, a measured row weighs its peak while its code is unchanged, and no run holds a slot or units it waits beside"
