#!/usr/bin/env bash
# Link a C program's objects into a nife program (milestone 835 (a C library, stage 1: files,
# clock and memory)). The shared second half of `helpers/build-speedtest1.sh` and
# `helpers/build-ioping.sh`, which fetch and compile; this renames, archives and links.
#
#     helpers/build-c-program.sh <name> <triple> <object>...
#
# writes `target/<name>/<triple>/<name>`. Each object must have been compiled with
# `helpers/c-library-cflags.sh <triple>`. The one holding `main` has it renamed `nife_c_main`
# (`llvm-objcopy --redefine-sym`, on every object, which leaves the others unchanged), because the
# `std` program the C is linked into has a `main` of its own (c_library/src/start.rs). The program
# also carries the unvouched-`std` manifest note `cargo xtask foreign-note` writes, as `rg` does.
#
# It uses this checkout's own `std` farm by path, as helpers/build-ripgrep.sh explains, and the
# pinned toolchain's `llvm-tools` for `llvm-objcopy` and `llvm-ar`.
set -euo pipefail
[ $# -ge 3 ] || { echo "usage: helpers/build-c-program.sh <name> <triple> <object>..." >&2; exit 2; }
NAME="$1"; TRIPLE="$2"; shift 2
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/target/$NAME/$TRIPLE"
WORK="$OUT/link"
mkdir -p "$WORK"
LLVM_BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin"
# This checkout's patched `std` farm, built if it is not (idempotent and quick when it is).
(cd "$ROOT" && cargo xtask std-src)

objs=()
for o in "$@"; do
  copy="$WORK/$(basename "$o")"
  cp "$o" "$copy"
  "$LLVM_BIN/llvm-objcopy" --redefine-sym main=nife_c_main "$copy"
  objs+=("$copy")
done
rm -f "$WORK/lib$NAME.a"
"$LLVM_BIN/llvm-ar" rcs "$WORK/lib$NAME.a" "${objs[@]}"
(cd "$ROOT" && cargo xtask foreign-note "$TRIPLE" "$WORK/note.o")
(
  cd "$ROOT/c_library"
  NIFE_C_ARCHIVE="$WORK/lib$NAME.a" NIFE_C_NOTE="$WORK/note.o" \
  RUSTUP_TOOLCHAIN="$ROOT/target/nife-farm" \
    cargo build --release --bin c_program \
      -Zjson-target-spec \
      -Zbuild-std=core,alloc,std,panic_abort \
      -Zbuild-std-features=compiler-builtins-mem \
      --target "$ROOT/targets/$TRIPLE.json"
)
cp "$ROOT/c_library/target/$TRIPLE/release/c_program" "$OUT/$NAME"
echo "build-c-program: $OUT/$NAME ($(wc -c < "$OUT/$NAME") bytes)"
