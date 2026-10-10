#!/usr/bin/env bash
# Build `cryptography_exerciser` for the three nife custom targets, for milestone 442 (a crypto provider `rustls` can use on all three bare-metal targets).

#
# **Part of the gated build since milestone 855 (the TLS graph enters the gated build).** `cargo
# xtask test` runs this for the legs it boots (`xtask::farm::tls_graph`), so `script/test` and CI's
# kernel legs build the program and the suite runs it. calef launched 855 on 2026-10-10 (UTC),
# option 1 of its block: fetch from crates.io as the rest of the workspace does. The dependency
# question this header used to defer was ruled by §196 (nife carries TLS: `rustls` for the protocol)
# and §198 (the glue is ours, the primitives are not). It is its own workspace still, so its crates
# stay out of the main `Cargo.lock`; `script/supply-chain` scans its graph.
#
# The one thing it does that `build-ripgrep.sh` does not: the package is **ours**, so it carries its
# own `.cargo/config.toml` with the `getrandom` backend selector and the four soft-implementation
# cfgs, and this script adds nothing to the command line beyond the target, build-std and the two pins
# below. If a
# build fails, the configuration is in the package where a reader will find it.
#
# Usage: helpers/build-cryptography-exerciser.sh
#        NIFE_CRYPTO_TRIPLES="x86_64-unknown-nife" helpers/build-cryptography-exerciser.sh
#
# See notes/cryptography-provider.md for the measurement that chose these crates and what is still open.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/cryptography_exerciser"
OUT="$ROOT/target/cryptography-exerciser"

# The patched std lives in the `nife-dev` toolchain, which `xtask std-src` builds and links.
# `RUSTUP_TOOLCHAIN` rather than `+nife-dev` for the reason `xtask::std_exerciser` records: the
# cargo proxy exports `RUSTUP_TOOLCHAIN=nightly`, which would override a `+` selector. A
# `rust-toolchain.toml` naming `nife-dev` is NOT an alternative here: milestone 442's lane measured
# it getting `aarch64-unknown-nife` wrong on an aarch64 host while the other two stayed right, and
# `script/crypto-probes`' header records the whole trap. And by path rather than by name
# (2026-09-30): the name is account-wide and a concurrent lane steals it mid-run; `std-src` above
# has just built "$ROOT/target/nife-farm", so the path is this checkout's own farm.
(cd "$ROOT" && cargo xtask std-src)

for TRIPLE in ${NIFE_CRYPTO_TRIPLES:-aarch64-unknown-nife riscv64-unknown-nife x86_64-unknown-nife}; do
  (
    cd "$SRC"
    # `CARGO_TARGET_DIR` is pinned because the copy below reads `$SRC/target`, and an exported
    # one would put the build elsewhere (xtask::farm::exerciser_target_dir has the 2026-09-30
    # story). `--locked` because a gate must build the graph the lockfile names, not a newer one.
    CARGO_TARGET_DIR="$SRC/target" RUSTUP_TOOLCHAIN="$ROOT/target/nife-farm" cargo build --release --locked \
      -Zjson-target-spec \
      -Zbuild-std=core,alloc,std,panic_abort \
      -Zbuild-std-features=compiler-builtins-mem \
      --target "$ROOT/targets/$TRIPLE.json"
  )
  mkdir -p "$OUT/$TRIPLE"
  cp "$SRC/target/$TRIPLE/release/cryptography_exerciser" "$OUT/$TRIPLE/cryptography_exerciser"
  echo "build-cryptography-exerciser: $OUT/$TRIPLE/cryptography_exerciser ($(wc -c <"$OUT/$TRIPLE/cryptography_exerciser") bytes)"
done
