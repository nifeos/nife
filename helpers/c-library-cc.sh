#!/bin/sh
# Print the clang that compiles C for nife's C library (milestone 835 (a C library, stage 1: files,
# clock and memory)): `$NIFE_CC` if set, else the first clang with the AArch64, RISC-V and X86
# backends, found the way fixtures/build.rs finds one (Apple's clang has no RISC-V backend).
set -eu
if [ -n "${NIFE_CC:-}" ]; then echo "$NIFE_CC"; exit 0; fi
for c in /opt/homebrew/opt/llvm/bin/clang /usr/local/opt/llvm/bin/clang clang; do
  if command -v "$c" >/dev/null 2>&1 && "$c" -print-targets 2>/dev/null | grep -q riscv64; then
    echo "$c"; exit 0
  fi
done
echo "c-library-cc: no clang with the AArch64, RISC-V and X86 backends; set NIFE_CC" >&2
exit 1
