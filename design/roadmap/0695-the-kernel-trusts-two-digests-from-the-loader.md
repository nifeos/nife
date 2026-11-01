---
status: PARTIAL
raised: 2026-09-27
promoted_from: the-kernel-trusts-two-digests-from-the-loader
milestone_dependencies: 976
decision_dependencies: 235
machine_requirements: none
specific_machine: none
needs_person: no
---
# 695. The kernel trusts two digests handed to it by the loader

*Title and slug are drafts.* Branch: `milestone/695-the-kernel-trusts-two-digests-from-the-loader`.

Item 6 of milestone 607 (nife is built and updated from packages), with milestone 697 (the trust
root leaves the kernel binary). §235 (the OS is built and updated from packages) ruled the shape on
2026-09-27 (UTC), T4 with T2. This block is the boot-path half; 697 is the build half and holds the
measurements.

Reuse: `crates/measured_boot`'s digest text, `crates/device_tree_blob`'s `/chosen` walk,
`crates/device_tree_patch`'s rewrite, `uefi_loader`'s command-line writer and build script, and the
three QEMU runners. No new crate and no new dependency.

## What changes

The loader hands the kernel two digests on all three architectures, as two words on the kernel's
command line: `nife.progenitor=sha256:<hex>`, the progenitor's, and `nife.base=sha256:<hex>`, the
digest of the archive's `program_measurements` table. The architect ruled on both words and their
spelling on 2026-11-01 (UTC), and §235's "manifest digest" reads as this table digest. On a device tree they sit in `/chosen/bootargs`; on x86_64 in
the PVH command line. `measured_boot::loader_digests` is the one grammar every writer and the
kernel use.

The kernel records the words while its front end still maps the line, and `trust::require` holds
the progenitor and the table to them before it enters userspace. A boot whose loader handed no
words, a repeated word or a malformed digest halts with `MEASURED BOOT REFUSED`, naming which.
`TRUST_ROOT` still holds both entries too, and both sources must agree until 697's part 2 removes
it. Only Secure Boot (milestone 500 (a stick that boots with Secure Boot on)) or the slot header's
CRC-32 protects the loader, and §235 accepted that cost. Signatures stay where §220 (signed builds)
put them, at install and update time.

Four writers copy the words; none computes them at boot. `cargo xtask` writes
`<archive>.loader-digests` beside every archive it packs, read back from the packed bytes. The QEMU
runners pass it as `-append`. `uefi_loader/build.rs` embeds it beside the archive, after holding it
to the archive's two entries. The board's U-Boot scripts set `bootargs` to the card's copy, and the
TH1520 FIT's environment carries it as `usr_bootargs`.

Comparable system: Fuchsia checks the `system_image` Merkle root, carried in its boot arguments, in
userspace rather than in the kernel.

## What is calef's

Both questions of
[`how-the-loader-hands-the-kernel-two-digests.md`](proposals/how-the-loader-hands-the-kernel-two-digests.md)
have answers from the architect, given 2026-11-01 (UTC). Question 1, the bytes of the handoff: option A, the two
command-line words `nife.progenitor=` and `nife.base=`, which the code here builds. Question 2:
option A, the composer records the digests and every loader copies them.

A third question follows and stays separate. Once the kernel can update apart from the base, the
syscall surface becomes a versioned ABI that installed programs rely on.

## The consumer, and the condition the code waits on

The first caller is milestone 976 (an OS update is a set of packages that lands through a slot),
part 4, the updater. The architect directed on 2026-11-01 (UTC) that this lane and 697's launch
before part 4 shows what a kernel download costs per update; that direction lifts the earlier hold.
Nobody has taken that measurement, so nothing here depends on it, and each piece reverts on its own. A
milestone names its consumer. It is not BUILT until a record cites its first use.

## The plan in parts

| part | what | needs a ruling? | size |
|---|---|---|---|
| 1 | A contract crate for how the kernel hands the base digest to the progenitor, and the progenitor's check of the table against it, host-tested. The two release together, so the lane picks the form. | no | about 200 lines and host tests in `crates/` and `crates/system_initializer` |
| 2 | The parser and writer for the handoff's two words, `measured_boot::loader_digests`. | question 1, decided | about 150 lines and host tests |
| 3 | The kernel reads the two words on each path and holds the progenitor and the table to them. | question 1, decided | about 150 lines, one QEMU suite per architecture |
| 4 | The writers: `xtask` and the QEMU runners, `uefi_loader`, the board's U-Boot scripts, and 976's slot writer. | question 2, decided | about 300 lines; the board half needs a bench boot |

## Follow-on

- **Done.** Part 2, on this block's branch: `measured_boot::loader_digests`, with host tests for
  every refusal and a doc test, and `measured_boot::PROGENITOR`, the entry name three programs now
  share.
- **Done.** Part 3, on this block's branch: `device_tree_blob`'s `boot_arguments` reads the root's
  own `/chosen/bootargs`; `trust::record_loader_digests` and `trust::require` hold the progenitor
  and the table to the words. Checked 2026-11-01 (UTC): `script/test --no-host` passed on
  riscv64, on x86_64 under both PVH direct boot and OVMF, and on aarch64. One earlier aarch64 run
  failed `a_memory_grant_holding_a_running_process_is_refused_once_then_reclaimed`, which passed
  alone and in the full rerun; it touches no code here.
- **Done.** Part 4's first three writers, on this block's branch. `device_tree_patch` gains
  `with_initial_ramdisk_and_boot_arguments`, and host tests read every writer's output back with
  the kernel's parser.
- **Outstanding.** Part 1: the kernel still checks the table itself, against both the words and
  `TRUST_ROOT`. Moving the check into the progenitor needs the contract crate part 1 names; checked
  against `kernel/src/trust.rs`, which still calls `require_program_measurements`.
- **Outstanding.** Part 4's fourth writer, 976's version 2 slot, which 976's part 4c lane owns.
  Ruled 2026-11-01 (UTC): the header grows from 188 to 252 bytes, with the progenitor digest and
  the base digest after the three member entries and the CRC-32 last. The updater writes the two
  digests and the chooser copies them into the two words. Checked against draft PR #3032, which
  records the ruling; `crates/boot_slot` does not hold the fields yet.
- **Outstanding.** A bench boot of the board scripts. No bench boot ran; the scripts use only
  `setenv`, which the proven vocabulary already holds. They carry the card's words, so a network
  boot of a rebuilt archive needs the card's script rewritten with it (`script/board-image --tftp
  --card`).
- **Outstanding.** The composed image becomes the only image when this milestone and 697 land
  (ruled 2026-11-01 UTC), and the boot gates then run on it; 697's block has the end state.
  Checked: the tree still builds its own image (`cargo xtask uefi-image`).

## Index row

The loader hands the kernel the progenitor's digest and the `program_measurements` table's digest, on all three
architectures, and the progenitor checks the base set (§235, T4 with T2). The boot-path half of
607's item 6; its first consumer is 976 part 4. The architect answered both handoff questions on 2026-11-01.
