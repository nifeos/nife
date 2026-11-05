#!/bin/sh
#
# helpers/patch-section-check-selftest.sh: prove helpers/patch-section-check.sh fails on each planted
# `[patch]` form and passes a clean tree, a vendor/ path patch and a pinned git dependency.
#
#     helpers/patch-section-check-selftest.sh

set -e
cd "$(dirname "$0")/.."
check="$(pwd)/helpers/patch-section-check.sh"
dir="$(mktemp -d "${TMPDIR:-/tmp}/patch-section-check-selftest.XXXXXX")"
trap 'rm -rf "$dir"' EXIT
fail() { echo "patch-section-check-selftest: $1" >&2; exit 1; }

# expect <pass|refuse> <label> <file> <text>
expect() {
    rm -rf "$dir/t"; mkdir -p "$dir/t/$(dirname "$3")"
    printf '%s\n' "$4" >"$dir/t/$3"
    sh "$check" "$dir/t" >"$dir/out" 2>"$dir/err" && got=pass || got=refuse
    [ "$got" = "$1" ] || fail "$2: wanted $1, got $got"
    if [ "$1" = refuse ]; then grep -q "$3:[0-9]" "$dir/err" || fail "$2: the refusal does not name file and line"; fi
}

expect pass "no patch" Cargo.toml '[package]
name = "a"
[dependencies]
b = { version = "1", git = "https://example.invalid/b", rev = "0123456789abcdef" }'
expect pass "vendor path patch" jig/Cargo.toml '[patch.crates-io]
talc = { path = "../vendor/talc" }'
expect pass "patch named only in a comment" Cargo.toml '# the [patch.crates-io] below is gone
[package]
name = "a"'
expect refuse "crates-io path patch" Cargo.toml '[patch.crates-io]
b = { path = "../b" }'
expect refuse "crates-io git patch" Cargo.toml '[patch.crates-io]
b = { git = "https://example.invalid/b", rev = "0123456789abcdef" }'
expect refuse "git url patch" Cargo.toml '[patch."https://example.invalid/b"]
b = { path = "../b" }'
expect refuse "bare patch" Cargo.toml '[patch]'
expect refuse "dotted key" Cargo.toml 'patch.crates-io.b = { path = "../b" }'
expect refuse "vendor path plus git" Cargo.toml '[patch.crates-io]
b = { path = "vendor/b", git = "https://example.invalid/b" }'
expect refuse "in cargo config" sub/.cargo/config.toml '[patch.crates-io]
b = { path = "../b" }'
expect refuse "sub-table form" Cargo.toml '[patch.crates-io.b]
path = "../b"'
echo "patch-section-check-selftest: ok"
