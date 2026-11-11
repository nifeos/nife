#!/bin/sh
#
# Selftest for the toolchain step of helpers/continuous-fuzz.sh, run by script/lint. A stub `rustup`
# in a throwaway $HOME/.cargo/bin (the supervisor puts that directory first on PATH) records its
# calls, so no download happens and no toolchain is needed. It proves three things: the supervisor
# installs the pinned toolchain exactly once before it looks at KVM or starts a slot; a failed
# install stops it with its own message; and a failed install writes no window row. Without the
# step, the failing-install case below reaches the KVM refusal instead and the check fails.

set -eu
here="$(cd "$(dirname "$0")" && pwd)"
sup="$here/continuous-fuzz.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fail() { echo "continuous-fuzz-selftest: $*" >&2; exit 1; }

mkdir -p "$work/home/.cargo/bin"
cat > "$work/home/.cargo/bin/rustup" <<'STUB'
#!/bin/sh
echo "$*" >> "$STUB_CALLS"
[ "${STUB_FAIL:-0}" = 1 ] || exit 0
echo "error: stub toolchain install failed" >&2
exit 1
STUB
chmod +x "$work/home/.cargo/bin/rustup"

# run FAIL: run the supervisor against a stub rustup and a KVM device that does not exist, so that
# a supervisor which got past the toolchain step stops at the KVM refusal and starts no guest.
run() {
    rm -f "$work/calls"
    : > "$work/calls"
    set +e
    out=$(HOME="$work/home" NIFE_FUZZ_HOME="$work/fuzz" NIFE_FUZZ_KVM_DEVICE="$work/no-such-device" \
        STUB_CALLS="$work/calls" STUB_FAIL="$1" NIFE_RUSTUP_ATTEMPTS=1 NIFE_RUSTUP_BACKOFF=0 \
        "$sup" 2>&1)
    rc=$?
    set -e
    calls=$(wc -l < "$work/calls" | tr -d ' ')
}

run 0
[ "$calls" -eq 1 ] || fail "install ok: $calls rustup calls, wanted exactly 1 ($out)"
grep -q '^show active-toolchain$' "$work/calls" || fail "install ok: wanted \`rustup show active-toolchain\`, got $(cat "$work/calls")"
[ "$rc" -ne 0 ] || fail "install ok: exit 0 with no KVM"
printf '%s' "$out" | grep -q 'KVM is not usable' || fail "install ok: did not go on to the KVM check ($out)"

run 1
[ "$rc" -ne 0 ] || fail "install fails: exit 0"
[ "$calls" -eq 1 ] || fail "install fails: $calls rustup calls, wanted 1"
printf '%s' "$out" | grep -q 'could not install the toolchain' || fail "install fails: no clear message ($out)"
printf '%s' "$out" | grep -q 'KVM is not usable' && fail "install fails: went on to the KVM check ($out)"
[ ! -e "$work/fuzz/state/windows.tsv" ] || fail "install fails: a window row was written"

# The class of a window. A wait that ran out its tick budget is held-budget-expired whether or not
# its message carries a replay line; a mismatch, a crash and a hang keep their own classes; a clean
# window stays ok. The marker is the one the guest prints (kernel/src/testing.rs).
marker=$(sed -n 's/^pub const BUDGET_EXPIRED_MARKER: &str = "\(.*\)";$/\1/p' "$here/../kernel/src/testing.rs")
[ "$marker" = "budget-expired:" ] || fail "the guest's budget marker is '$marker', the supervisor greps 'budget-expired:'"

verdict() { # LOG-TEXT RC FIRST
    printf '%s\n' "$1" > "$work/log"
    "$sup" --classify "$work/log" "$2" "$3"
}
want() { # NAME GOT WANT
    [ "$2" = "$3" ] || fail "$1: got '$2', wanted '$3'"
}

want "clean" "$(verdict 'syscall fuzzer: 50 seeds, 900 ms (x)' 0 100)" "ok 50 150"
want "mismatch" "$(verdict 'syscall fuzzer: seed 0x6f step 3: a slot differs (replay: NIFE_SYSCALL_FUZZ_SEEDS=111:1)' 1 100)" "held-finding 11 112"
want "teardown budget" "$(verdict "syscall fuzzer: seed 0x6f step 3: $marker teardown should have woken actor 1 (replay: NIFE_SYSCALL_FUZZ_SEEDS=111:1)" 1 100)" "held-budget-expired 11 112"
want "confined budget" "$(verdict "confined fuzzer: seed 0x6f step 3: $marker fuzzer 0 never came back (replay: NIFE_CONFINED_FUZZ_SEEDS=111:1)" 1 100)" "held-budget-expired 11 112"
want "budget panic" "$(verdict "panicked at x: $marker actor 2 neither finished nor parked" 1 100)" "held-budget-expired 0 101"
want "crash" "$(verdict 'panicked at x: index out of bounds' 1 100)" "held-crash 0 101"
want "hang" "$(verdict 'booting' 143 100)" "held-hang 0 101"
# A guest a signal ended before its bound did not hang: sweep says so, and the window is
# infrastructure. The line is the supervisor's own (`early_stop`), so the two cannot drift.
early=$(sed -n 's/^early_stop="\(.*\)"$/\1/p' "$sup")
[ -n "$early" ] || fail "the supervisor defines no early_stop line"
want "stopped early" "$(verdict "booting
continuous-fuzz: $early after 41 s, short of its 300 s bound" 143 100)" "infrastructure 0 101"
want "killed early" "$(verdict "booting
continuous-fuzz: $early after 3 s, short of its 300 s bound" 137 100)" "infrastructure 0 101"
# A guest that ran out its bound hung, whatever its exit status: the runner hands the bound's
# TERM to QEMU, and QEMU then exits 0.
ran_out=$(sed -n 's/^bound_ran_out="\(.*\)"$/\1/p' "$sup")
[ -n "$ran_out" ] || fail "the supervisor defines no bound_ran_out line"
want "bound ran out" "$(verdict "booting
continuous-fuzz: $ran_out after 300 s" 0 100)" "held-hang 0 101"
want "infrastructure" "$(verdict 'booting' 125 100)" "infrastructure 0 101"

echo "continuous-fuzz-selftest: ok"
