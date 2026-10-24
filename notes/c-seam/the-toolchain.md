# The toolchain: bare-metal clang, one compiler for two ISAs

*Name: provisional. After the parent's own heading.*

How the C component in [`notes/c-seam.md`](../c-seam.md) is compiled: how clang is found, why one compiler must have both
backends, and the flags, one of which is load-bearing.

`fixtures/build.rs` compiles `fixtures/c/c_seam.c` and hands the object to the linker for the `c_shim`
binary only (`cargo::rustc-link-arg-bin=c_shim=...`). No archive and no `ar`: one translation unit,
one object, straight onto the linker's command line. Every other program in the `user` package links
exactly as before, which keeps the foreign component from becoming everyone's problem.

How clang is found, and why the check is a capability check. The same discipline `xtask`'s
`llvm_tool` uses for `llvm-objcopy`: resolve the tool from a known list rather than hoping it is on
`PATH` under the right name, and fail loudly with what to install.

1. `$CRICKER_CC`, the escape hatch.
2. `/opt/homebrew/opt/llvm/bin/clang`, then `/usr/local/opt/llvm/bin/clang`. Homebrew's llvm keg is
   deliberately not linked onto `PATH` (it would shadow Apple's clang), so it is named directly.
3. `clang` on `PATH`. Debian and Ubuntu build every LLVM backend into their packages, so this is the
   usual CI answer.

Each candidate must have both the AArch64 and RISC-V backends, checked with `clang -print-targets`:

```
$ /usr/bin/clang -print-targets | grep -c riscv        # Apple clang, Xcode CLT
0
$ /opt/homebrew/opt/llvm/bin/clang -print-targets | grep -c riscv
4
```

Apple's clang is rejected on purpose, and it is worth being clear that this is not an oversight:
it compiles the aarch64 side perfectly well. Requiring both backends from *one* compiler even when
only one ISA is being built is DECISIONS §19 (parity is a gate, not an aspiration) applied to the
toolchain. A machine where the two architectures are compiled by two different clangs is a machine
where "it works on aarch64" has stopped predicting anything about riscv64, and the failure would
surface halfway through a build instead of at the front door.

Cost to a fresh clone: one dependency. `script/bootstrap` installs it (`brew install llvm` on macOS,
`apt-get install clang` on Debian), and from this milestone on `cargo build -p user` needs a
cross-capable clang. Without one, `fixtures/build.rs` panics with installation instructions rather than
leaving the link to fail with a bare "undefined symbol: c_seam_transform".

## The flags, and the one that is load-bearing

```
aarch64:  --target=aarch64-unknown-none-elf  -mgeneral-regs-only
riscv64:  --target=riscv64-unknown-none-elf  -march=rv64imac -mabi=lp64
both:     -ffreestanding -fno-pic -fno-stack-protector -Os -std=c11 -Wall -Wextra -Werror
```

- `-mgeneral-regs-only` is not a size optimization, it is a correctness requirement. Without it,
  clang happily vectorizes the component's byte loops: 53 vector-register operands appear in the
  object. Three independent reasons that breaks here, and any one is enough. The Rust target is
  `aarch64-unknown-none-softfloat`, which uses no FP/SIMD registers in its ABI. The kernel never
  touches `CPACR_EL1`, so FP/SIMD traps at EL0. And the context switch saves no FP state, so even if
  it did not trap, the registers would be corrupted across a preemption. Whichever of those bit first,
  the bug would be a confined component that fails for a reason having nothing to do with its logic.
- `-mabi=lp64`, not `lp64d`, because the Rust target is `riscv64imac` (no F or D extension). An ABI
  mismatch here would either be refused by lld or, worse, linked with arguments passed in registers the
  other side never reads.
- `-fno-pic` to match the bare targets' `static` relocation model.
- `-ffreestanding` so no hosted libc is implied. clang still supplies its own `stddef.h` and
  `stdint.h` from its resource directory, which is all the component includes.


## The x86_64 flags, and the third backend

```
x86_64:   --target=x86_64-unknown-none-elf  -mno-sse -mno-mmx -mno-red-zone
```

- `-mno-sse` and `-mno-mmx` match the Rust target's `features = "-mmx,-sse,+soft-float"`. The
  kernel does not enable the FP unit for ring 3, so a vectorized byte loop would take `#UD` on its
  first `xmm` instruction (`CR4.OSFXSR` clear). It is the aarch64 argument for
  `-mgeneral-regs-only`, one ISA over.
- `-mno-red-zone` has no counterpart on the other two architectures. The red zone is the 128 bytes
  below `rsp` that a leaf function may use without moving the pointer, and it is safe only if
  nothing can push there behind the function's back. An interrupt in ring 3 switches to a different
  stack, so a user program could keep it. It is off because the Rust target sets
  `disable_redzone = true`, and two sides of one call that disagree about 128 bytes of stack do not
  share an ABI.
- `fixtures/build.rs` requires the X86 backend too, from the same clang. Every clang that has the
  AArch64 and RISC-V backends (Homebrew's keg, a distro build) also has X86, and Apple's clang still
  fails on RISC-V, so the third backend rejects no compiler the other two accept.
