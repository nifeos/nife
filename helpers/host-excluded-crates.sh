#!/bin/sh
# helpers/host-excluded-crates.sh: which workspace crates cannot compile for the host, derived
# from cargo's own dependency graph rather than listed by hand.
#
# A crate that reaches `crates/user_mode_runtime` cannot compile for the host: `svc` and `ecall`
# there are EL0 syscalls, and with no nife kernel under them they fault. `cargo tree -i` answers
# exactly that question of the graph itself, so the answer moves when a dependency moves and
# cannot lag behind one. This file is the tree's single derivation of that set (roadmap 393). Its
# consumers run it instead of carrying the list: script/lint's two clippy passes, `cargo xtask
# test`'s host and Miri passes (xtask/src/suite.rs), script/coverage's run and its floor, and
# script/mutation's cargo-mutants call. script/lint's "the host pass excludes exactly the
# bare-metal crates" gate derives the set a second way, from `cargo metadata`, and fails when the
# two disagree or when a consumer stops calling this file; that gate keeps this derivation honest,
# and this derivation keeps the consumers in step.
#
# Hand lists were the record until 2026-10-15, and that is how the exclusion set went wrong on
# 2026-08-03 and stayed wrong for eleven days, invisible because CI had moved to an aarch64 runner
# the same day, where the EL0 assembly compiles by accident. script/lint's gate tells that story
# whole.
#
#   helpers/host-excluded-crates.sh          crate names, one per line
#   helpers/host-excluded-crates.sh paths    the same crates as paths relative to the root
#
# Names provisional (lane milestone/393), like every other name a lane mints.
set -e
cd "$(dirname "$0")/.."

# Default edges, normal and dev and build alike: a dev-dependency on `user_mode_runtime` also
# cannot build under `cargo test`, so it belongs in the set, and this is the same question the
# lint gate's `cargo metadata` derivation asks of the resolve graph.
tree=$(cargo tree -i user_mode_runtime --prefix none)
[ -n "$tree" ] || {
    echo "host-excluded-crates: cargo tree named nothing for user_mode_runtime; refusing to" >&2
    echo "  hand the host passes an empty exclusion set" >&2
    exit 1
}

case "${1:-names}" in
names)
    # The first line of an inverted tree is its root, `user_mode_runtime` itself, and the set
    # keeps it: no syscall may run on the host, so the crate stays out of the workspace passes. Its
    # own tests run in a pass of their own (xtask's host tests and script/lint's host clippy name it).
    printf '%s\n' "$tree" | awk '{print $1}' | sort -u
    ;;
paths)
    root=$(pwd -P)
    printf '%s\n' "$tree" | awk -v root="$root" '
        {
            sub(/ \(\*\)$/, "")         # cargo tree marks a repeated package after the path
            path = $0
            sub(/^[^(]*\(/, "", path)   # through the first "(": the path itself, ")" left on
            sub(/\)$/, "", path)
            sub("^" root "/", "", path)
            print path
        }' | sort -u
    ;;
*)
    echo "usage: helpers/host-excluded-crates.sh [names|paths]" >&2
    exit 2
    ;;
esac
