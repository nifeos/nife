#!/bin/sh
# helpers/kani-install-selftest.sh: the guards in script/kani-install and the Kani pin in
# helpers/prune_toolchains.py, proved in a temporary HOME over a stub `cargo` and a stub `rustup`.
# Nothing here touches the real ~/.kani or ~/.rustup, and `cargo kani setup` is only ever the stub.
#
#   1. a matching version with no toolchain link runs setup again and says so.
#   2. a matching version whose link dangles (its target deleted) does the same, and the link that
#      is repaired by the stub's setup is then accepted without another setup.
#   3. a matching version with a link to a directory that has a rustc runs nothing but the version
#      query.
#   4. RUSTUP_HOME set to a private directory is refused with exit 1 before any setup, for both the
#      repair path and the install path; the default $HOME/.rustup is allowed, and
#      NIFE_KANI_SETUP_WITH_PRIVATE_RUSTUP=1 overrides.
#   5. helpers/prune_toolchains.py --remove keeps the toolchain named in
#      ~/.kani/kani-*/rust-toolchain-version and removes an unpinned dated nightly, and refuses
#      --remove when ~/.kani holds a version directory with no readable pin.
#
# Run by script/lint.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/nife-kani-install-selftest.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
fail() { echo "kani-install-selftest: $*" >&2; exit 1; }

pin="$(tr -d '[:space:]' <"$root/.kani-version")"
mkdir -p "$tmp/tree/script" "$tmp/bin"
cp "$root/script/kani-install" "$tmp/tree/script/kani-install"
cp "$root/.kani-version" "$tmp/tree/.kani-version"
cat >"$tmp/bin/cargo" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >>"$KANI_STUB_LOG"
if [ "$1" = kani ] && [ "$2" = --version ]; then
    printf 'Kani Rust Verifier %s (cargo plugin)\nCBMC 6.11.0\n' "$KANI_STUB_VERSION"
fi
if [ "$1" = kani ] && [ "$2" = setup ]; then
    mkdir -p "$HOME/.rustup/toolchains/nightly-fixed/bin"
    : >"$HOME/.rustup/toolchains/nightly-fixed/bin/rustc"
    chmod +x "$HOME/.rustup/toolchains/nightly-fixed/bin/rustc"
    mkdir -p "$HOME/.kani/kani-$KANI_STUB_VERSION"
    ln -sfn "$HOME/.rustup/toolchains/nightly-fixed" "$HOME/.kani/kani-$KANI_STUB_VERSION/toolchain"
fi
exit 0
STUB
chmod +x "$tmp/bin/cargo"

# run <name> [VAR=value ...]: kani-install at the pin in HOME=$tmp/home, RUSTUP_HOME unset unless given.
run() {
	name="$1"
	shift
	: >"$tmp/$name.log"
	set +e
	env -u RUSTUP_HOME -u NIFE_KANI_SETUP_WITH_PRIVATE_RUSTUP HOME="$tmp/home" PATH="$tmp/bin:$PATH" \
		KANI_STUB_LOG="$tmp/$name.log" KANI_STUB_VERSION="$pin" "$@" \
		sh "$tmp/tree/script/kani-install" >"$tmp/$name.out" 2>&1
	rc=$?
	set -e
}
setups() { grep -c '^kani setup$' "$tmp/$1.log" || true; }

# 1. no link at all
rm -rf "$tmp/home"
mkdir -p "$tmp/home"
run none
[ "$rc" = 0 ] || fail "no link: exit $rc"
[ "$(setups none)" = 1 ] || fail "no link: setup did not run once"
grep -q "does not resolve" "$tmp/none.out" || fail "no link: the rerun was not announced"

# 2. a dangling link, then the repaired link
rm -rf "$tmp/home"
mkdir -p "$tmp/home/.kani/kani-$pin"
ln -s "$tmp/home/gone/toolchain" "$tmp/home/.kani/kani-$pin/toolchain"
run dangling
[ "$rc" = 0 ] || fail "dangling: exit $rc"
[ "$(setups dangling)" = 1 ] || fail "dangling: setup did not run once"
grep -q "does not resolve" "$tmp/dangling.out" || fail "dangling: the rerun was not announced"
run repaired
[ "$rc" = 0 ] || fail "repaired: exit $rc"
[ "$(setups repaired)" = 0 ] || fail "repaired: setup ran with a good link"

# 3. a good link is left alone: only the version query reaches cargo
grep -v '^kani --version$' "$tmp/repaired.log" | grep -q . && fail "good link: cargo ran more than the version query"

# 4. a private RUSTUP_HOME
rm -rf "$tmp/home"
mkdir -p "$tmp/home/.kani/kani-$pin"
ln -s "$tmp/home/gone/toolchain" "$tmp/home/.kani/kani-$pin/toolchain"
run private RUSTUP_HOME="$tmp/private-rustup"
[ "$rc" = 1 ] || fail "private rustup (repair): exit $rc, wanted 1"
[ "$(setups private)" = 0 ] || fail "private rustup (repair): setup ran"
grep -q "NIFE_KANI_SETUP_WITH_PRIVATE_RUSTUP" "$tmp/private.out" || fail "private rustup: the message does not name the override"
run privinstall RUSTUP_HOME="$tmp/private-rustup" KANI_STUB_VERSION=0.0.1
[ "$rc" = 1 ] || fail "private rustup (install): exit $rc, wanted 1"
grep -q '^install ' "$tmp/privinstall.log" && fail "private rustup (install): cargo install ran"
run default RUSTUP_HOME="$tmp/home/.rustup"
[ "$rc" = 0 ] || fail "default RUSTUP_HOME refused: exit $rc"
rm "$tmp/home/.kani/kani-$pin/toolchain"
ln -s "$tmp/home/gone/toolchain" "$tmp/home/.kani/kani-$pin/toolchain"
run override RUSTUP_HOME="$tmp/private-rustup" NIFE_KANI_SETUP_WITH_PRIVATE_RUSTUP=1
[ "$rc" = 0 ] || fail "override: exit $rc"
[ "$(setups override)" = 1 ] || fail "override: setup did not run"

# 5. the prune keeps Kani's toolchain
rm -rf "$tmp/home" "$tmp/repo"
mkdir -p "$tmp/home/.kani/kani-$pin" "$tmp/repo"
echo nightly-2020-01-01 >"$tmp/home/.kani/kani-$pin/rust-toolchain-version"
cat >"$tmp/bin/rustup" <<'STUB'
#!/bin/sh
if [ "$1 $2" = "toolchain list" ]; then
    printf '%s\n' nightly-2020-01-01-x86_64-unknown-linux-gnu nightly-2020-02-02-x86_64-unknown-linux-gnu stable-x86_64-unknown-linux-gnu
elif [ "$1 $2" = "toolchain uninstall" ]; then
    printf '%s\n' "$3" >>"$RUSTUP_STUB_LOG"
fi
STUB
chmod +x "$tmp/bin/rustup"
: >"$tmp/rustup.log"
(cd "$tmp/repo" && HOME="$tmp/home" PATH="$tmp/bin:$PATH" RUSTUP_STUB_LOG="$tmp/rustup.log" \
	python3 -I "$root/helpers/prune_toolchains.py" --remove >"$tmp/prune.out" 2>&1) || fail "prune: failed: $(cat "$tmp/prune.out")"
grep -q "KEEP nightly-2020-01-01" "$tmp/prune.out" || fail "prune: did not keep Kani's toolchain"
grep -q '^nightly-2020-01-01' "$tmp/rustup.log" && fail "prune: uninstalled Kani's toolchain"
grep -q '^nightly-2020-02-02' "$tmp/rustup.log" || fail "prune: did not remove the unpinned nightly"
rm "$tmp/home/.kani/kani-$pin/rust-toolchain-version"
if (cd "$tmp/repo" && HOME="$tmp/home" PATH="$tmp/bin:$PATH" RUSTUP_STUB_LOG="$tmp/rustup.log" \
	python3 -I "$root/helpers/prune_toolchains.py" --remove >"$tmp/prune2.out" 2>&1); then
	fail "prune: --remove went ahead with an unreadable Kani pin"
fi

echo "kani-install-selftest: ok"
