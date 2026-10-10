#!/usr/bin/env bash
# Build **ioping, unmodified**, for the three nife targets (milestone 835 (a C library, stage 1:
# files, clock and memory), its second consumer, and the program milestone 834 (ioping on nife and
# Linux) runs on silicon).
#
# An experiment's apparatus on `helpers/build-speedtest1.sh`'s terms: nothing in `script/test`
# runs it, the archive packs `target/ioping/<triple>/ioping` when it is on disk, and
# `system_tests/src/user/ioping_tests.rs` skips when it is not. Fetching it in a gate is §46 (thin
# primitives or whole subsystems)'s decision.
#
# The source is ioping 1.3's one C file, `ioping.c` at the `v1.3` tag of its GitHub repository,
# pinned by SHA-256 and compiled untouched. ioping is GPL-3.0-or-later; the binary this makes is
# therefore GPL, which §135 (running GPL software is aggregation) already covers for a program nife
# runs. Nothing nife-specific is in its source: it takes its generic path on a platform that is
# neither Linux nor a BSD (`gettimeofday` rather than `clock_gettime`, its own `err` and `errx`),
# which is what the same file does on any POSIX system it does not know.
#
# Usage: helpers/build-ioping.sh
#        NIFE_IOPING_TRIPLES="x86_64-unknown-nife" helpers/build-ioping.sh   (one target)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION=1.3
# Read 2026-10-10 (UTC) from the download, and equal to the `ioping.c` in the v1.3 release tarball.
SHA256_IOPING_C=4336abbf0b71df3e006769b7913879a2fcdb59531a9e773fc5af74c08fb408e0

BUILD="${TMPDIR:-/tmp}/nife-ioping-$(printf %s "$ROOT" | shasum -a 256 | cut -c1-12)"
mkdir -p "$BUILD"
if [ ! -f "$BUILD/ioping.c" ]; then
  echo "build-ioping: fetching ioping $VERSION"
  curl -sSfL --retry 3 --max-time 120 -o "$BUILD/ioping.c.part" \
    "https://raw.githubusercontent.com/koct9i/ioping/v$VERSION/ioping.c"
  mv "$BUILD/ioping.c.part" "$BUILD/ioping.c"
fi
got="$(shasum -a 256 "$BUILD/ioping.c" | cut -d' ' -f1)"
if [ "$got" != "$SHA256_IOPING_C" ]; then
  echo "build-ioping: ioping.c has SHA-256 $got, not $SHA256_IOPING_C; refusing it" >&2
  rm -f "$BUILD/ioping.c"
  exit 1
fi

CC="$("$ROOT/helpers/c-library-cc.sh")"

for TRIPLE in ${NIFE_IOPING_TRIPLES:-aarch64-unknown-nife riscv64-unknown-nife x86_64-unknown-nife}; do
  OBJ="$BUILD/obj/$TRIPLE"
  mkdir -p "$OBJ"
  read -r -a CFLAGS <<< "$("$ROOT/helpers/c-library-cflags.sh" "$TRIPLE")"
  echo "build-ioping: compiling ioping $VERSION for $TRIPLE"
  # `-std=gnu99` and an empty `EXTRA_VERSION` are what ioping's own Makefile passes.
  "$CC" "${CFLAGS[@]}" -O2 -std=gnu99 -DEXTRA_VERSION=\"\" -c "$BUILD/ioping.c" -o "$OBJ/ioping.o"
  "$ROOT/helpers/build-c-program.sh" ioping "$TRIPLE" "$OBJ/ioping.o"
done
