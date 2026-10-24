# Recorded gaps, legitimate absences, and the lists that stayed complete

*Name: provisional.*

An appendix to [`notes/architecture-list-sweep.md`](../architecture-list-sweep.md), which states the
result.

## Recorded gaps, which are fine

Listed so the next reader does not count them twice.

| Where | Missing | Where the reason is written |
|---|---|---|
| ~~`xtask/src/main.rs` `STD_TARGETS: [&str; 2]`, and `targets/` holding two `.json` specs~~ | ~~`x86_64-unknown-nife`~~ | **Closed** by milestone 184 (extend the `std` port to x86_64): `STD_TARGETS` has three entries and `targets/` three specs |
| `xtask/src/suite.rs`, the vendored RedoxFS build loop `for target in [TARGET, RISCV_TARGET]` | x86_64 | milestone 164 (x86_64 userspace can't build `aes`). The reason is written at `initial_ramdisk_x86` and at `fs_service.rs`'s `NO_FS_SERVER`, **not at the loop itself**, which is the one weakness in an otherwise clean record |
| ~~`xtask/src/swish_check.rs` `swish_check`, refusing `--arch x86_64`~~ | ~~an x86_64 leg~~ | **Closed**: the leg runs (`swish_check::swish_check`'s `--arch` match), milestone 182 (x86-64's own interactive-boot entry point) built it, and milestone 177 (wire the graphical terminal stack into the real interactive boot) is BUILT |
| `xtask/src/icount.rs` `icount`, which refuses `--arch x86_64` | an x86_64 icount leg | The real reason is at `bench_x86`'s doc comment (the LAPIC timer is a periodic hardware reload with no re-armed deadline to compare against). **The inline comment at the match arm gives a different and now-stale reason** ("needs a userspace this port cannot build"), which stopped being true when milestone 161 item 4 landed. Worth a one-line correction by whoever next touches it |
| `script/lint`, the boot-mode feature loops `for tgt in aarch64... riscv64...` | x86_64 | Thirty lines above the loop, naming each narrowing, why it is a scope gap rather than an oversight, what is still covered, and **the trigger that retires the suppression**. This is the model |
| `kernel/src/user/fs_service.rs` `NO_FS_SERVER`, `NO_MKFS`; `clock_service.rs` `NO_RTC`; `user/console_device.rs` `NO_UART_PAGE`; `std_service.rs` `NO_STD_EXERCISER` | per-test x86_64 coverage | The `skip!()` mechanism, whose whole design is that a skipped fixture prints why |

## Legitimately architecture-specific, which are not findings

`script/board-image` (riscv64 only; it builds a U-Boot `booti` image for a StarFive board).
`script/cpu-matrix` (riscv64 only, and its first line says so; it sweeps QEMU's riscv CPU models).
`script/ci-build`'s `hvf` check (aarch64 only by construction: Hypervisor.framework runs the host's own
ISA). `crates/device_tree_blob/tests/qemu_aarch64_virt.rs` and `qemu_riscv64_virt.rs` with their `.dtb` fixtures
(q35 has ACPI and no device tree). `notes/riscv-tlb-shootdown.md` and `notes/x86-tlb-shootdown.md`
with no aarch64 twin (aarch64 broadcasts `TLBI` in hardware, so there is no software shootdown to
document). `notes/riscv-port.md` and `notes/x86-port.md` with no aarch64 twin (aarch64 is the
original, not a port). The per-architecture assembly whose names differ because the hardware does
(`vectors.s` against `trap.s`, `image_header.s`, `segments.rs`, `port.rs`, `rtc.rs`, `ap_boot.rs`,
`machine.rs`). `script/qemu-check`'s device probes, which ask each emulator for the device that
emulator needs.

## The things that are complete, because they are the answer to the design question

These are worth more than the findings, because each one is a shape that did not go stale.

- `kernel/build.rs`: `match arch` over three arms with `other => panic!("nife has no linker
  script for target arch {other}")`. A fourth architecture fails the build with a sentence naming
  the file to edit.
- `kernel/src/arch/mod.rs`: three `#[cfg]` module arms and three flat re-exports, and the
  module's own comment says a new ISA is a new directory rather than a diff.
- `crates/elf`'s `EXPECTED_MACHINE`: three explicit arms because it was two and the default
  arm was a bug. It read `#[cfg(not(target_arch = "riscv64"))] EM_AARCH64`, so the x86_64 kernel
  was compiled to accept aarch64 binaries and refuse its own. Its comment states the general lesson
  in one sentence: "A default arm that names one architecture is a trap the moment a third exists."
- `xtask`'s `ArchLegs`: an enum whose doc records the same correction. It was `Both`, its two
  predicates were written as `self != the_other_one`, and that answers `true` for every leg the
  moment a third variant exists. Now `All` with explicit `matches!`.
- `rust-toolchain.toml`'s `targets`: three entries, and the only architecture list in the tree
  that a tool reads rather than a human copies.
- `.cargo/config.toml`: three `[target.*] runner` blocks. `crates/paging/src/`: `aarch64.rs`,
  `sv39.rs`, `x86_64.rs`. `crates/machine_discovery/src/`, `bench/baseline-<arch>.txt`,
  `helpers/qemu-runner-<arch>.sh`, `kernel/link-<arch>.ld`, and twelve files under
  `kernel/src/arch/<arch>/`: all complete at three.

The pattern. Everything that stayed complete is either a Rust `match` the compiler pushed on,
or a per-architecture file whose absence a build notices. Everything that went stale is a
space-separated string in a shell script, a YAML step, a TOML array, or a sentence in a note.
Ten of the eleven silent gaps are in the second group and the eleventh is a `#[cfg]` pair with no `else`.

## One hazard that is not a finding today

Ten sites spell `#[cfg(not(target_arch = "x86_64"))]` or `#[cfg_attr(not(target_arch = "x86_64"),
...)]`, in `kernel/src/console.rs`, `memory.rs`, `pci.rs`, `smp.rs`, `user/clock_service.rs`, and
`user/supervision_tests.rs`. Every one is correct today, because "not x86_64" is exactly
"aarch64 or riscv64" while there are three architectures. Every one is the shape that made
`EXPECTED_MACHINE` wrong, and each will silently include a fourth architecture on the day one
arrives. Named here rather than filed as ten bugs, because rewriting correct code on a hypothetical
is the gold-plating the tenets refuse. What it argues for is that a fourth port's first task is a
grep for `not(target_arch`, which is the sort of thing a note is for.
