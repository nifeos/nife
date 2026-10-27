---
status: PROPOSED
raised: 2026-10-27
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The boot stick's shared names are code

Raised by the 2026-10-27 documentation sweep
([its report](../../audit-reports/2026-10-27-the-fat-crate-read-from-the-outside.md), finding 8).
Name: provisional, for this file and for every constant it sketches.

Reuse: `crates/boot_ladder`, which already holds the boot lines the kernel prints and the gates
match (`BANNER`, `SELF_TEST`), and which `xtask` already depends on.

## The failure

Five names cross between the programs that build, write and boot the stick, and every side spells
each one itself.

| agreement | its spellings |
|---|---|
| the kernel's handover line, which every boot gate waits for | `kernel/src/boot.rs` prints it; `xtask/src/stick.rs` (`BOOTED`), `install.rs`, `uefi.rs`, `swish_check/verdict.rs` and `helpers/stick-maker-proof.sh` match it |
| the loader's first line, which names milestone 87 (the x86_64 bare-metal machine) | `uefi_loader/src/loading.rs` prints it; `xtask/src/stick.rs` and `install.rs` match it |
| each architecture's removable-media file name | `xtask/src/stick.rs` (`PAYLOADS`), `crates/stick_maker/src/payload.rs` (`architecture`), `uefi_loader/src/placement.rs` (`BOOT_FILE_PATH`), `components/src/system_installer.rs` (`BOOT_FILE_NAME`) |
| the payload directory xtask hands the stick program's build | `NIFE_STICK_PAYLOADS` and `NIFE_STICK_BUILD`, set in `xtask/src/stick.rs` and read in `crates/stick_maker/build.rs` |
| the downloads' names and directory | `xtask::stick::download_name` and `target/stick-maker`; `helpers/stick-maker-proof.sh` spells `stick_maker-macos-$(uname -m)` again |

A changed handover line passes every unit test and fails every boot gate with a timeout, which reads
as a hang rather than a rename. A changed file name on one side boots nothing from a stick the other
side wrote. The method ruled 2026-10-26 UTC (a contract is code, not prose) and rule 7 of
`AGENTS.md` both make these code.

## The fix

```rust
// boot_ladder
/// The kernel's last line before the progenitor runs; every boot gate waits for it.
pub const PROGENITOR: &str = "nife: handing the system to the userspace progenitor.";
/// The prefix of the loader's first line.
pub const LOADER: &str = "nife uefi_loader: ";

/// The UEFI removable-media path's file name for each architecture.
pub const REMOVABLE_MEDIA: [(&str, &str); 3] = [
    ("x86_64", "BOOTX64.EFI"),
    ("aarch64", "BOOTAA64.EFI"),
    ("riscv64", "BOOTRISCV64.EFI"),
];
```

The kernel, the loader, `xtask`, `stick_maker` and `system_installer` take those names from it.
`helpers/stick-maker-proof.sh` asks `cargo xtask` for the handover line and the download's path
rather than spelling either.
The two environment variable names go in a constant `stick_maker`'s build script and `xtask` both
import.

## Exit test

`git grep -n "handing the system to the userspace progenitor" -- ':!bench' ':!*.md'` finds only
`crates/boot_ladder`, the removable-media names appear outside it only in tests and docs, and
`cargo xtask stick-boot` and `helpers/stick-maker-proof.sh` pass.
