---
status: PARTIAL
raised: 2026-09-27
promoted_from: the-trust-root-leaves-the-kernel-binary
milestone_dependencies: 695, 976
decision_dependencies: 235
machine_requirements: none
specific_machine: none
needs_person: no
---
# 697. The trust root leaves the kernel binary

*Title and slug are drafts.* Branch: `milestone/697-the-trust-root-leaves-the-kernel-binary`; part
1 is on `milestone/697-part-1-the-kernel-keeps-only-the-progenitor`.

Item 6 of milestone 607 (nife is built and updated from packages), with milestone 695 (the kernel
trusts two digests handed to it by the loader). §235 (the OS is built and updated from packages)
ruled the shape on 2026-09-27 (UTC): T4 with T2. The loader hands the kernel the progenitor's digest
and the digest of the archive's `program_measurements` table. The kernel checks only the progenitor, and the progenitor checks
the base set. This block is the build half: the kernel binary stops depending on the base set. 695
is the boot-path half.

Reuse: `crates/measured_boot`, `crates/sealed_pair`, `xtask`'s measurement step and `uefi_loader`'s
build script, each changed in place. No new dependency.

## What ties the kernel to the base set, measured

Measured 2026-10-31 (UTC) at `origin/main` 25e17a56e. A stripped release kernel is 860,624 bytes on
aarch64, 684,536 on riscv64 and 877,528 on x86_64 (about 230 KB each under `gzip -9`).

- `kernel/build.rs` compiles two digests from `target/init-measure-<arch>.txt` into `TRUST_ROOT`:
  the progenitor's (`sealed_pair::BOOT_PROGRAMS`) and the archive's `program_measurements`
  table's. That is 64 bytes of `.rodata`. The six test child builders that
  `spawn_elf_child_builder` enters are measured through the table (`trust::require_program_in`),
  and the system test `the_trust_root_names_only_the_progenitor_and_the_table` holds the root to
  those two names.
- A one-byte change to one digest changes one byte of the stripped kernel on all three
  architectures, and nothing else. So a base update changes at most 64 bytes of the kernel: the
  progenitor's digest and the table's.
- `xtask` packs the archive before it builds the kernel (`uefi::uefi_kernel`, `base_image`'s kernel
  step), and `uefi_loader/build.rs` refuses a kernel and archive that `sealed_pair` finds unsealed.

The proposal
[`how-the-loader-hands-the-kernel-two-digests.md`](proposals/how-the-loader-hands-the-kernel-two-digests.md)
has the table of what this costs an update under 976's option B.

## The consumer, and the condition the code waits on

The first caller is milestone 976 (an OS update is a set of packages that lands through a slot),
part 4: the updater that writes the inactive slot. calef ruled on 2026-10-31 (UTC) that 976 part 4
takes option B, a version 2 slot of named members. While the kernel vouches for the base, basalt
publishes an interim kernel package, rebuilt on every base change; this block deletes it.

The wait on that measurement, ruled 2026-10-31 (UTC), was lifted on 2026-11-01 (UTC): the
architect directed that the lanes for this block and 695 start before 976 part 4 shows what a
kernel download costs per update. The measurement is still not taken. Part 1 does not depend on it;
parts 2 to 4 do, in the sense that the measurement is what justifies them, so each stays reversible
until it is in 976's block. A milestone names its consumer, and is not BUILT until a record cites
its first use.

## The plan in parts

| part | what | needs a ruling? | size |
|---|---|---|---|
| 1 | **Done.** The system-test kernel measures the six child builders through the table, as `trust::require_program` does for the benchmark boots; `BOOT_PROGRAMS` is `progenitor` alone. | ruled 2026-11-01 | `kernel/src/trust.rs`, `kernel/src/user/entry.rs`, `sealed_pair`, `xtask/src/measure.rs`, one system test |
| 2 | `kernel/build.rs` stops generating `TRUST_ROOT`; `kernel/src/trust.rs` checks against the digests 695 delivers. `xtask` drops the archive-before-kernel order. | question 1 of the proposal | about 200 lines removed, 100 added; lands with 695's part 3 |
| 3 | `sealed_pair` and `uefi_loader/build.rs` check the loader's copy of the digests against the archive instead of searching the kernel for them. | question 2 | about 250 lines, host tests |
| 4 | Basalt stops rebuilding the kernel package on a base change, and the interim kernel package's per-base rebuild goes. | no, once 2 and 3 land | a basalt pull request, bumped after the nife commit lands |

Part 1's ruling (2026-11-01 UTC): the table measures the child builders, the production kernel
carries no test-program digests, and tests have no second trust mechanism. On the same day the
architect ruled that `nife.base` is the second digest the loader hands the kernel and names the
table's digest, and that the composed image is the one image: the tree's own image goes away when
695 and this block land, and the boot gates (`confirm-boot`, and the interim `update-boot`, which
then folds into `confirm-boot`) run on the composed image. Part 4 needs no ruling of its own.

## Index row

A kernel binary carries 64 bytes of base-set digests compiled in, the progenitor's and the
`program_measurements` table's; test programs are measured through the table. The build half of
607's item 6; its first consumer is 976 part 4, whose measured kernel download cost is still owed.
