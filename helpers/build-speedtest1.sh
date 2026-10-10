#!/usr/bin/env bash
# Build **SQLite's `speedtest1`, unmodified**, for the three nife targets (milestone 835 (a C
# library, stage 1: files, clock and memory), the first consumer of milestone 831 (SQLite's
# speedtest1 on nife and Linux)).
#
# This is an experiment's apparatus, as `helpers/build-ripgrep.sh` is: nothing in `script/test`
# runs it. `xtask`'s archive step packs `target/speedtest1/<triple>/speedtest1` when it is on disk,
# and `system_tests/src/user/speedtest1_tests.rs` skips when it is not. Fetching SQLite from
# sqlite.org in a gate is a dependency decision (§46 (thin primitives or whole subsystems)), so
# the pull request that added this asks calef for the ruling.
#
# The source is SQLite's and is untouched. Two files, both pinned by SHA-256:
#   - the 3.50.4 amalgamation (`sqlite3.c`, `sqlite3.h`), sqlite.org's release zip;
#   - `test/speedtest1.c` at the `version-3.50.4` tag of SQLite's GitHub mirror, because the
#     amalgamation zip does not carry it. Everything nife-specific is on the command line below: the
# target flags, nife's C library headers (`vendor/relibc/include`), `-D__nife__`, and two SQLite
# compile-time options that SQLite documents for exactly this case (`SQLITE_THREADSAFE=0`: stage
# 1 has no threads; `SQLITE_OMIT_LOAD_EXTENSION`: nife links statically and has no `dlopen`).
# The same flags build the Linux comparison row in milestone 831, so the two programs differ
# only in their C library.
#
# Linking is `helpers/build-c-program.sh`'s, which says what it does to the objects.
#
# Usage: helpers/build-speedtest1.sh
#        NIFE_SPEEDTEST1_TRIPLES="x86_64-unknown-nife" helpers/build-speedtest1.sh   (one target)
#        NIFE_CC=/path/to/clang helpers/build-speedtest1.sh
#
# See c_library/README.md for what the C library does and does not do.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION=3.50.4
AMALGAMATION=sqlite-amalgamation-3500400
# Read 2026-10-10 (UTC) from the downloads themselves; the zip's is also on sqlite.org's download
# page as SHA3-256, which is why it is recorded here as SHA-256 of what was fetched.
SHA256_ZIP=1d3049dd0f830a025a53105fc79fd2ab9431aea99e137809d064d8ee8356b032
SHA256_SPEEDTEST1=f495cd1c3f727ebf6270d967b43f11a14304053ae4532d6338dbfea65c1a5a78

BUILD="${TMPDIR:-/tmp}/nife-speedtest1-$(printf %s "$ROOT" | shasum -a 256 | cut -c1-12)"
mkdir -p "$BUILD"

fetch() { # url file sha256
  if [ ! -f "$BUILD/$2" ]; then
    echo "build-speedtest1: fetching $1"
    curl -sSfL --retry 3 --max-time 120 -o "$BUILD/$2.part" "$1"
    mv "$BUILD/$2.part" "$BUILD/$2"
  fi
  got="$(shasum -a 256 "$BUILD/$2" | cut -d' ' -f1)"
  if [ "$got" != "$3" ]; then
    echo "build-speedtest1: $2 has SHA-256 $got, not $3; refusing it" >&2
    rm -f "$BUILD/$2"
    exit 1
  fi
}
fetch "https://www.sqlite.org/2025/$AMALGAMATION.zip" "$AMALGAMATION.zip" "$SHA256_ZIP"
fetch "https://raw.githubusercontent.com/sqlite/sqlite/version-$VERSION/test/speedtest1.c" \
  speedtest1.c "$SHA256_SPEEDTEST1"
if [ ! -f "$BUILD/$AMALGAMATION/sqlite3.c" ]; then
  (cd "$BUILD" && unzip -q -o "$AMALGAMATION.zip")
fi

CC="$("$ROOT/helpers/c-library-cc.sh")"

for TRIPLE in ${NIFE_SPEEDTEST1_TRIPLES:-aarch64-unknown-nife riscv64-unknown-nife x86_64-unknown-nife}; do
  OBJ="$BUILD/obj/$TRIPLE"
  mkdir -p "$OBJ"
  read -r -a CFLAGS <<< "$("$ROOT/helpers/c-library-cflags.sh" "$TRIPLE")"
  SQLITE_OPTS=(-O2 -DSQLITE_THREADSAFE=0 -DSQLITE_OMIT_LOAD_EXTENSION)
  echo "build-speedtest1: compiling SQLite $VERSION for $TRIPLE"
  "$CC" "${CFLAGS[@]}" "${SQLITE_OPTS[@]}" -c "$BUILD/$AMALGAMATION/sqlite3.c" -o "$OBJ/sqlite3.o"
  "$CC" "${CFLAGS[@]}" "${SQLITE_OPTS[@]}" -I"$BUILD/$AMALGAMATION" \
    -c "$BUILD/speedtest1.c" -o "$OBJ/speedtest1.o"
  "$ROOT/helpers/build-c-program.sh" speedtest1 "$TRIPLE" "$OBJ/sqlite3.o" "$OBJ/speedtest1.o"
done
