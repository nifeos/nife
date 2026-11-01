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
grep -q '^toolchain install$' "$work/calls" || fail "install ok: wanted \`rustup toolchain install\`, got $(cat "$work/calls")"
[ "$rc" -ne 0 ] || fail "install ok: exit 0 with no KVM"
printf '%s' "$out" | grep -q 'KVM is not usable' || fail "install ok: did not go on to the KVM check ($out)"

run 1
[ "$rc" -ne 0 ] || fail "install fails: exit 0"
[ "$calls" -eq 1 ] || fail "install fails: $calls rustup calls, wanted 1"
printf '%s' "$out" | grep -q 'could not install the toolchain' || fail "install fails: no clear message ($out)"
printf '%s' "$out" | grep -q 'KVM is not usable' && fail "install fails: went on to the KVM check ($out)"
[ ! -e "$work/fuzz/state/windows.tsv" ] || fail "install fails: a window row was written"

echo "continuous-fuzz-selftest: ok"
