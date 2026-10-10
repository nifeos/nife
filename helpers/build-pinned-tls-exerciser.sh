#!/usr/bin/env bash
# Build pinned_tls_exerciser for the three nife targets, where xtask's archive build looks for it:
# target/pinned-tls-exerciser/<triple>/pinned_tls_exerciser. Milestone 501 (a TLS client that
# speaks to one pinned peer).
#
# helpers/build-cryptography-exerciser.sh's shape. Part of the gated build since milestone 855 (the
# TLS graph enters the gated build): `cargo xtask test` runs this for the legs it boots, so
# system_tests/src/user/pinned_tls_tests.rs and milestone 801's package_index_tests.rs run in
# `script/test` and CI. `cargo xtask std-src` first, because it builds the `std` farm `-Zbuild-std`
# compiles against; that also relinks the machine-wide `nife-dev` toolchain (notes/std.md).
#
#     helpers/build-pinned-tls-exerciser.sh
#     NIFE_CRYPTO_TRIPLES=x86_64-unknown-nife helpers/build-pinned-tls-exerciser.sh
#
# Name: provisional 2026-10-06 (UTC), the sibling of build-cryptography-exerciser.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/pinned_tls_exerciser"
OUT="$ROOT/target/pinned-tls-exerciser"

(cd "$ROOT" && cargo xtask std-src)

for TRIPLE in ${NIFE_CRYPTO_TRIPLES:-aarch64-unknown-nife riscv64-unknown-nife x86_64-unknown-nife}; do
  (
    cd "$SRC"
    # Pinned and locked for build-cryptography-exerciser.sh's reasons.
    CARGO_TARGET_DIR="$SRC/target" RUSTUP_TOOLCHAIN="$ROOT/target/nife-farm" cargo build --release --locked \
      -Zjson-target-spec \
      -Zbuild-std=core,alloc,std,panic_abort \
      -Zbuild-std-features=compiler-builtins-mem \
      --target "$ROOT/targets/$TRIPLE.json"
  )
  mkdir -p "$OUT/$TRIPLE"
  # Both binaries of the workspace: the TLS client's own test, and milestone 801 (packages over the
  # internet)'s index client, which links the same provider and so shares this build's flags.
  for BIN in pinned_tls_exerciser package_fetch_exerciser; do
    cp "$SRC/target/$TRIPLE/release/$BIN" "$OUT/$TRIPLE/$BIN"
    echo "build-pinned-tls-exerciser: $OUT/$TRIPLE/$BIN ($(wc -c <"$OUT/$TRIPLE/$BIN") bytes)"
  done
done
