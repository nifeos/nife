#!/bin/sh
# helpers/fuzz-ci-selftest.sh: the CI fuzz job keeps a finding private, and fails the lint when it
# stops doing so.
#
# Two checks, each of which fails without the change that introduced the job:
#
#   1. The wiring. `ci.yml` has a `fuzz-guests` job that runs `helpers/continuous-fuzz.sh --ci`,
#      runs on `merge_group` and `workflow_dispatch` and not on `pull_request`, never names a
#      self-hosted runner, and has no step that uploads, prints or copies the guest's output.
#   2. The behavior. `helpers/continuous-fuzz.sh --ci` runs in a scratch tree against a stand-in
#      guest. A guest that prints a `replay:` line and a panic exits 1, and nothing it printed, no
#      seed and no panic text, reaches standard output or standard error; the private directory is
#      gone afterwards. A clean guest exits 0, a missing KVM device exits 3, and the same commit and
#      shard give the same seed range (the fuzz host replays a shard from those two alone).
#
# Name: provisional, minted 2026-11-01 (UTC) by lane/a-short-sharded-fuzz-pass-in-ci.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
fail() { echo "fuzz-ci-selftest: $*" >&2; exit 1; }

# 1. The wiring. The job's block runs from its id to the next two-space-indented key.
job="$(awk '/^  fuzz-guests:/{on=1; print; next} on && /^  [A-Za-z0-9_-]+:/{exit} on{print}' "$root/.github/workflows/ci.yml")"
[ -n "$job" ] || fail ".github/workflows/ci.yml has no fuzz-guests job"
code="$(printf '%s\n' "$job" | grep -v '^ *#')"
printf '%s\n' "$code" | grep -q 'helpers/continuous-fuzz.sh --ci ' || fail "fuzz-guests does not run helpers/continuous-fuzz.sh --ci"
printf '%s\n' "$code" | grep -q "merge_group" || fail "fuzz-guests does not run on merge_group"
printf '%s\n' "$code" | grep -q "workflow_dispatch" || fail "fuzz-guests does not run on workflow_dispatch"
printf '%s\n' "$code" | grep -q "pull_request" && fail "fuzz-guests names pull_request; a fork's log is public"
printf '%s\n' "$code" | grep -qE "self-hosted|runs-on:.*(patagonia|galapagos|xenon|argon|radon)" && fail "fuzz-guests names a self-hosted runner"
printf '%s\n' "$code" | grep -q "runs-on: ubuntu-24.04" || fail "fuzz-guests does not run on a hosted ubuntu-24.04 runner"
# The udev rule that opens /dev/kvm uses `tee` on a file that is not the guest's.
printf '%s\n' "$code" | grep -v 'udev/rules.d' | grep -qE "upload-artifact|hang_evidence|ci-build|\btee\b|\bcat\b|guest\.log|GITHUB_STEP_SUMMARY" \
    && fail "fuzz-guests has a step that could carry the guest's output into a log or artifact"

# 2. The behavior, in a scratch tree with stand-ins for the tools the shard calls.
work="$(mktemp -d "${TMPDIR:-/tmp}/fuzz-ci-selftest.XXXXXX")"
trap 'rm -rf "$work"' EXIT INT TERM
tree="$work/tree"
mkdir -p "$tree/helpers" "$work/bin" "$work/tmp"
cp "$here/continuous-fuzz.sh" "$tree/helpers/"
: > "$tree/helpers/qemu-path.sh"
cat > "$tree/helpers/qemu-bounded.sh" <<'STUB'
#!/bin/sh
shift
exec "$@"
STUB
cat > "$tree/helpers/qemu-runner-x86_64.sh" <<'STUB'
#!/bin/sh
cat "$STUB_GUEST"
exit "${STUB_GUEST_RC:-0}"
STUB
cat > "$work/bin/cargo" <<'STUB'
#!/bin/sh
case "$1" in
    xtask) mkdir -p target ;;
    test) echo '{"reason":"compiler-artifact","target":{"name":"x"},"package_id":"x","name":"system_tests","executable":"/stub/elf"}' ;;
esac
STUB
cat > "$work/bin/qemu-system-x86_64" <<'STUB'
#!/bin/sh
exit 0
STUB
chmod +x "$tree"/helpers/*.sh "$work"/bin/*
git -C "$tree" init -q
git -C "$tree" -c user.name=t -c user.email=t@example.com commit -q --allow-empty -m stand-in
commit="$(git -C "$tree" rev-parse HEAD)"

run() { # run SHARD KVM_DEVICE; output in $work/out, status in $work/rc
    set +e
    (cd "$tree" && HOME="$work/home" PATH="$work/bin:$PATH" TMPDIR="$work/tmp" NIFE_FUZZ_KVM_DEVICE="$2" \
        helpers/continuous-fuzz.sh --ci "$1") > "$work/out" 2>&1
    echo $? > "$work/rc"
    set -e
}

first="$(cd "$tree" && helpers/continuous-fuzz.sh --ci-range 0)"
[ "$first" = "$(cd "$tree" && helpers/continuous-fuzz.sh --ci-range 0)" ] || fail "the same commit and shard gave two seed ranges"
[ "$first" != "$(cd "$tree" && helpers/continuous-fuzz.sh --ci-range 1)" ] || fail "two shards share a seed range"

# A finding: a replay line and a panic. Exit 1, and nothing of either reaches the output.
{
    echo "boot banner SECRETPANICTEXT"
    echo "panicked at kernel/src/secret.rs: SECRETPANICTEXT"
    echo "replay: NIFE_SYSCALL_FUZZ_SEEDS=7785200123:1"
} > "$work/guest-finding"
STUB_GUEST="$work/guest-finding" STUB_GUEST_RC=1 run 0 /dev/null
[ "$(cat "$work/rc")" = 1 ] || fail "a finding did not exit 1 (exit $(cat "$work/rc")): $(cat "$work/out")"
grep -q "the fuzz host replays this shard" "$work/out" || fail "a finding did not say the fuzz host replays the shard"
grep -q "1 finding in shard 0 at commit $commit" "$work/out" || fail "a finding did not print the count, shard and commit"
grep -q "SECRETPANICTEXT" "$work/out" && fail "a finding leaked the guest's output"
sed "s/$commit//g" "$work/out" | grep -qE '[0-9]{9,}' && fail "a finding printed a seed"
[ -z "$(ls -A "$work/tmp")" ] || fail "the private directory outlived the shard"

# A clean window.
echo "syscall fuzzer: 500 seeds, 1000 ms (all clean)" > "$work/guest-clean"
STUB_GUEST="$work/guest-clean" run 0 /dev/null
[ "$(cat "$work/rc")" = 0 ] || fail "a clean window did not exit 0: $(cat "$work/out")"
grep -q "shard 0 (kernel-thread) clean" "$work/out" || fail "a clean window did not say so"

# No KVM is infrastructure, never a pass and never a finding.
STUB_GUEST="$work/guest-clean" run 0 "$work/no-such-device"
[ "$(cat "$work/rc")" = 3 ] || fail "a missing KVM device did not exit 3"

echo "fuzz-ci-selftest: the job is wired for merge_group only, a finding prints no guest output, a clean shard and a missing KVM device exit 0 and 3"
