---
status: BUILT
raised: 2026-09-21
built: 2026-10-22
promoted_from: a-long-file-name-or-riscv64-cannot-be-installed
---
# 560. A long file name, or riscv64 cannot be installed

Stage 1 of milestone 813 (a proven FAT crate, released on its own), by calef's ruling on #2580
(2026-10-22 UTC), which 813's block quotes. Raised by the rung 2a lane of milestone 198 (a package
manager, and the trivial install that makes a second customer possible), which wrote
`crates/file_allocation_table` as an 8.3-only `mkfs`.

## The gap, in one line

The three removable-media boot file names, and whether each is an 8.3 short name:

| architecture | file | 8.3? |
|---|---|---|
| `x86_64` | `BOOTX64.EFI` | yes, 7 + 3 |
| aarch64 | `BOOTAA64.EFI` | yes, 8 + 3 |
| riscv64 | `BOOTRISCV64.EFI` | **no**, 11 + 3 |

An installer writes the EFI system partition with `crates/file_allocation_table`, so a writer with
short names only cannot put the third on a disk. `components/src/system_installer.rs` is the one
caller on the machine being installed; `xtask/src/cloud_image.rs` is the host-side one.

## What the writer does

A name that is 8.3 is written as a short entry alone. Any other name the specification admits gets
VFAT long-name entries (`crates/file_allocation_table/src/long_name.rs`):

1. A short alias by the specification's basis-name steps with a `~1` tail, so `BOOTRISCV64.EFI` is
   `BOOTRI~1.EFI`. The tail never needs a second number because the volume holds one file.
2. `ChkSum` over the alias's eleven bytes, stamped into every long-name entry. This is the part
   that fails silently: a driver that rejects the run shows the alias and boots nothing.
3. `ceil(len / 13)` entries in reverse order, the first on disk carrying `LAST_LONG_ENTRY`, each
   holding thirteen UCS-2 units with one `0x0000` after the name and `0xFFFF` to the end.
4. The `EFI\BOOT` cluster rendered slot by slot across its sectors, so a 255-unit name (twenty
   entries, 23 slots with the dot entries and the alias) reaches its second sector.

A name that is empty, over 255 units, outside the Basic Multilingual Plane, holding a control
character or one of `" * / : < > ? \ |`, or ending in a space or a period is refused with
`Error::NameRefused` rather than mangled.

## Done means

- Kani proves the checksum equals the specification's shift-and-add loop for every short name,
  that every run of every length up to 255 has the right ordinals, flag, attribute, type, cluster,
  checksum and unit packing with its terminator and padding, and that every alias of a name up to
  14 units is a legal short name. Each harness has a replayable falsification patch.
- `tests/a_third_party_driver_reads_it.rs` writes `\EFI\BOOT\BOOTRISCV64.EFI`; on macOS
  `fsck_msdos -n` exits 0, the host's `msdos` driver lists `BOOTRISCV64.EFI` and not the alias,
  and the file reads back byte for byte. A flipped checksum fails it.
- `cargo xtask stick-boot` writes the riscv64 payload onto a GPT disk with this crate and boots it
  under EDK2 riscv64 to the progenitor, so a firmware reads the run on every run of the gate.

## Built, 2026-10-22 (UTC)

All three hold. The `stick-boot` transcript line is
`stick-boot: riscv64: booted from BOOTRISCV64.EFI on target/cloud-image/nife-riscv64.img to the
progenitor`. EDK2 finds the file only through the long-name run: the same disk with one bit of
each entry's checksum flipped, booted by `helpers/qemu-stick.sh riscv64`, never starts the loader
and stops at the UEFI Shell.

## BUGS

- **It does not by itself make riscv64 installable.** The loader hands its own file over as a PVH
  module, which the device-tree architectures have no slot for: milestone 568 (the boot file has
  nowhere to go on a device-tree machine).
- The alias is always `~1`. Right for a volume created holding one file; the crate's own `BUGS`
  says so, and milestone 813's step 7 numbers aliases when the crate writes a second file.
- Nothing here reads a long name. Reading them, with the checksum checked, is milestone 813's
  step 4.

## Follow-on

- **Milestone 568.** The device-tree handoff slot, the other half of a riscv64 install.
- **Milestone 813.** Steps 2 to 9 of the full crate: the sector interface and BPB parse, bounded
  chain walks, directories and long-name read, file read and the MBR, allocation and write, rename
  and alias numbering, the general format, then release.

## Index row

`crates/file_allocation_table` writes VFAT long-name entries ahead of a `~1` alias whenever a name is not 8.3, so the installer's FAT writer can put `\EFI\BOOT\BOOTRISCV64.EFI` on a disk. Kani proves the checksum and the run's layout for every name, the host's driver reads the long name back, and EDK2 riscv64 boots the file from a volume this crate wrote.
