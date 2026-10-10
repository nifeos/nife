#!/bin/sh
# The clang flags that compile a C program for nife's C library (milestone 835 (a C library, stage
# 1: files, clock and memory)), one triple at a time: `helpers/c-library-cflags.sh
# aarch64-unknown-nife`. Printed one per word, for a caller's command line.
#
# Each target half matches the Rust target spec in `targets/<triple>.json`, because the C objects
# and the Rust `std` they link with must agree on the ABI:
#   - aarch64: `-mgeneral-regs-only -mabi=aapcs-soft` is the target's `"abi": "softfloat"` (doubles
#     passed in general registers, arithmetic by compiler-builtins), and `-mno-unaligned-access` its
#     `+strict-align`.
#   - riscv64: `rv64imac` and `lp64` (no F or D), with the `medany` code model, which is what the
#     target's `"code-model": "medium"` means to clang.
#   - x86_64: no SSE, no MMX, no x87 and soft float, which is the target's `+soft-float` with SSE
#     off: a `double` travels in general registers (`rdi`, `rsi`, `rax`) and its arithmetic is a
#     compiler-builtins call, as in Rust. `-mno-sse` alone is not enough, because clang then refuses
#     to return a `double` at all ("SSE register return with SSE disabled"); turning off SSE2 and
#     the x87 as well is what makes it choose the soft-float convention. With no x87, clang
#     refuses `long double` outright on this target (`-mlong-double-64` does not change that), so
#     a C program for x86_64 nife cannot use the type; c_library/README.md's BUGS says so. No red
#     zone, as fixtures/build.rs explains.
# The rest says there is no hosted libc but nife's (`-ffreestanding -nostdlibinc`, then
# `vendor/relibc/include`), that the code is static (`-fno-pic`), that nife's C library has no stack
# protector runtime, and that the platform is nife (`-D__nife__`, which the generated headers'
# `target_os = "nife"` arms test; no compiler defines it, because no compiler has a nife target).
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
case "${1:-}" in
  aarch64-unknown-nife)
    echo --target=aarch64-unknown-none-elf -mgeneral-regs-only -mabi=aapcs-soft -mno-unaligned-access ;;
  riscv64-unknown-nife)
    echo --target=riscv64-unknown-none-elf -march=rv64imac -mabi=lp64 -mcmodel=medany ;;
  x86_64-unknown-nife)
    echo --target=x86_64-unknown-none-elf -mno-sse -mno-sse2 -mno-mmx -mno-80387 -msoft-float -mno-red-zone ;;
  *)
    echo "usage: helpers/c-library-cflags.sh <aarch64|riscv64|x86_64>-unknown-nife" >&2; exit 2 ;;
esac
echo -ffreestanding -nostdlibinc -fno-pic -fno-stack-protector -D__nife__ -isystem "$ROOT/vendor/relibc/include"
