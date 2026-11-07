#!/bin/sh
# Prove helpers/open-kvm.sh ends at once, without sudo, when the device is writable; reaches for the
# udev recipe when it is not; and that every workflow step which opens KVM runs the script and no
# copy of the recipe.
#
# Name: provisional, minted by lane/owned-runner-flip-safe.

set -u
cd "$(dirname "$0")/.." || exit 2

dir=$(mktemp -d) || exit 2
trap 'rm -rf "$dir"' EXIT
failed=0

fail() {
    echo "open-kvm-selftest: $*" >&2
    failed=1
}

mkdir "$dir/bin"
cat >"$dir/bin/sudo" <<'STUB'
#!/bin/sh
echo "sudo $*" >>"$SUDO_LOG"
cat >/dev/null
exit 1
STUB
chmod +x "$dir/bin/sudo"
: >"$dir/writable"
chmod 600 "$dir/writable"

SUDO_LOG="$dir/sudo.log" PATH="$dir/bin:$PATH" KVM_DEVICE="$dir/writable" helpers/open-kvm.sh >/dev/null 2>&1 ||
    fail "a writable device must pass"
test -e "$dir/sudo.log" && fail "a writable device must not reach for sudo"

if [ "$(id -u)" != 0 ]; then
    : >"$dir/readonly"
    chmod 400 "$dir/readonly"
    SUDO_LOG="$dir/sudo.log" PATH="$dir/bin:$PATH" KVM_DEVICE="$dir/readonly" helpers/open-kvm.sh >/dev/null 2>&1 &&
        fail "a device that is not writable must take the udev recipe, which the stub sudo fails"
    grep -q 'sudo tee /etc/udev/rules.d/99-kvm4all.rules' "$dir/sudo.log" 2>/dev/null ||
        fail "a device that is not writable must try the udev recipe"
fi

for file in .github/workflows/*.yml; do
    if grep -n '99-kvm4all' "$file" >/dev/null; then
        fail "$file holds a copy of the udev recipe: run helpers/open-kvm.sh instead"
    fi
done
grep -q 'helpers/open-kvm.sh' .github/workflows/owned-runner-canary.yml ||
    fail "the owned canary's dry run must run helpers/open-kvm.sh"

exit "$failed"
