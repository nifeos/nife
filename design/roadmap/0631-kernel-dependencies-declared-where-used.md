---
status: BUILT
raised: 2026-10-02
built: 2026-10-03
promoted_from: kernel-dependencies-declared-where-used
---
# 631. The kernel declares each dependency where it is used

Built 2026-10-03.

**calef ruled on 2026-10-03 (UTC), on PR #1477: `tock-registers` moves into the aarch64 target
table as proposed, and option B for `memory_corruption_canary_gate`.** No new feature, no default
feature; the warning stays and is recorded as an exception where a reader meets it.

## Evidence

`nightly-2026-10-02`'s cargo adds `cargo::unused_dependencies`, warning-only and on by default, and
PR #1474 (the toolchain bump) found it firing. A dependency in the unconditional `[dependencies]`
table is compiled for every target, so any target whose source never names it warns.

| Dependency | Used by | Warned on |
|---|---|---|
| `tock-registers` | aarch64 only: `drivers/{pl011,gic,gicv3}.rs`, `arch/aarch64/*`, and three `cfg(target_arch = "aarch64")` blocks in `lib.rs`. `ns16550.rs` names it only in a comment saying why it does not use it. | riscv64 and x86_64 |
| `memory_corruption_canary_gate` | All three architectures, in `sched.rs`'s `canary` module, which is `cfg(not(feature = "bench"))`. The bench build swaps in a no-op twin. | Any `--features bench` build, on every architecture |

## What was built

1. `tock-registers = "0.10"` moved into `[target.'cfg(target_arch = "aarch64")'.dependencies]`,
   beside `aarch64-cpu`, with a comment saying why it is there. This is the pattern PR #1370 used
   for `calendar`.
2. `memory_corruption_canary_gate` stays where it was, and a `BUGS` entry beside it in
   `kernel/Cargo.toml` records the warning on every bench build, why Cargo cannot remove it, the
   refused fix, and that the exception is a foot gun. No `allow` either: it would hide a second,
   real unused dependency.

## Refused: a default `corruption_canary` feature

The proposal's shape for the canary was an optional dependency enabled by a new
`corruption_canary` feature in `default`, with the bench build turning it off. Refused because it
would be the kernel's first default feature, every bench and icount invocation would have to
remember `--no-default-features`, and forgetting that would silently add the canary's tick load to
the bench floors. A wrong floor nobody notices costs more than a warning everybody can read.

## Proof

Measured on `nightly-2026-10-02`, rebased on `d6a902a9b`.

- `cargo tree -p kernel --target <triple> -e normal`, before and after, for
  `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`. The
  only difference is the `tock-registers` edge leaving riscv64 and x86_64. aarch64's tree is
  identical, since `aarch64-cpu` also pulls it.
- `Cargo.lock` is byte-identical.
- After `cargo clean -p kernel`, `cargo build -p kernel` on each architecture emits no
  `unused_dependencies` warning. Before the move, the same riscv64 build warned
  ``unused dependency `tock-registers` ``.
- `cargo build -p kernel --features bench` on each architecture emits exactly one, for
  `memory_corruption_canary_gate`: the recorded exception.

Reversible by moving one line back; nothing else depends on which table it is in.

## Follow-on

- **Recorded.** The bench build's `memory_corruption_canary_gate` warning, as a `BUGS` entry beside
  the dependency in `kernel/Cargo.toml`, marked as a deliberate exception.
- **Refused.** A default `corruption_canary` feature, for the reason above.
- **Milestone 713.** Milestone 713 (the rest of the tree declares its dependencies where they are used). The same lint on `user_mode_runtime` and `redoxfs_server`, outside this
  milestone's scope, in `design/roadmap/0713-the-rest-of-the-tree-declares-dependencies-where-used.md`.

## Index row

`nightly-2026-10-02` made cargo warn about a dependency a target never names. `tock-registers` moved
into the aarch64 target table, so riscv64 and x86_64 stop compiling it. The one remaining kernel
warning, the canary gate on `--features bench`, is kept on purpose and recorded beside the
dependency. The only fix would have been the kernel's first default feature and a silent way to
skew the bench floors.
