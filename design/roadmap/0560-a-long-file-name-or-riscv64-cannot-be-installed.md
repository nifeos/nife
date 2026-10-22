---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: a-long-file-name-or-riscv64-cannot-be-installed
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 560. A long file name, or riscv64 cannot be installed

The number is **provisional**: the integrator mints it at merge. Promoted from the proposal `a-long-file-name-or-riscv64-cannot-be-installed` on 2026-09-22, filed 2026-09-21. Raised by the rung 2a lane of milestone 198 (a package manager, and
the trivial install that makes a second customer possible), which wrote
`crates/file_allocation_table` and refused the name rather than mangling it.

*Covered by milestone 813 (a proven FAT crate, released on its own), noted 2026-10-07 (UTC). calef
ruled on #1806 that `crates/file_allocation_table` grows into a FAT12/16/32 read-write crate with
long names, and 813's Exit formats a volume holding `BOOTRISCV64.EFI`. The status stays NOT-STARTED
because this piece can still land first and on its own. The last BUGS line below, that the crate
should never read, predates that ruling.*

*`fatfs` and `embedded-sdmmc` were measured against this milestone on 2026-10-22 (UTC), in
[784's FAT appendix](0784-reuse-what-the-tree-wrote/fat-crates-measured.md). Whether to adopt one or
build a full crate of our own, with this piece first, waits on calef
([the options](0784-reuse-what-the-tree-wrote/fat-adopt-or-build.md)).*

Nothing has to be decided, bought or ruled on. It is a self-contained piece of
`crates/file_allocation_table` with host tests and one third-party reader already wired up.

## The gap, in one line

`crates/file_allocation_table` writes 8.3 short names only, and the three removable-media boot file
names are:

| architecture | file | 8.3? |
|---|---|---|
| `x86_64` | `BOOTX64.EFI` | yes, 7 + 3 |
| aarch64 | `BOOTAA64.EFI` | yes, 8 + 3 |
| riscv64 | `BOOTRISCV64.EFI` | **no**, 11 + 3 |

The crate refuses the third (`Error::NameNotShort`) rather than mangling it, deliberately: a mangled
name is a file the firmware will not find and nothing will say why. The refusal is tested.

## What it forecloses today, and what it does not

It forecloses an installer on riscv64, and nothing else. The one caller is
`components/src/system_installer.rs`, which writes a disk's EFI system partition on the machine being
installed. Nothing else in this tree writes FAT at all. `crates/stick_maker` runs on a host and
delegates formatting to that host's own tools (`diskutil`, `mkfs.vfat`). QEMU's `vvfat`
synthesizes a volume from a directory. So both reach `BOOTRISCV64.EFI` without going near this
crate.

So the gap is narrow and it is also on the path. DECISIONS §157 (a trivial install is a web
page, a USB drive, and packages) makes the install a PC. Rung 2a is an `x86_64` claim for a
second reason as well (see the separate proposal on the device-tree architectures' module slot). But
radon is a riscv64 board this project owns, and the first time somebody wants nife on its disk
rather than on its card, this is one of the two things in the way.

## What it costs, and the part that bites

A long-name entry is not one field. The specification puts a run of `LDIR` entries ahead of the
short entry, in reverse order, each carrying thirteen UTF-16 units, each stamped with a checksum of
the short name that the short entry still has to carry. So the work is:

1. Generate a short alias (`BOOTRI~1.EFI`), because the short entry does not go away.
2. Compute the `ChkSum` over those eleven bytes, which is the one-byte rotate-and-add the
   specification defines. This is the part that fails silently. A driver that does not like the
   checksum ignores the long entries and uses the alias. So a wrong checksum produces a volume that
   mounts, lists a file with the wrong name, and boots nothing.
3. Write `ceil(len / 13)` entries with the ordinal and `LAST_LONG_ENTRY` bit, plus the `0xFFFF`
   padding the specification requires after the terminating NUL.
4. Widen the directory cluster's capacity accounting: three directories currently fit in one cluster
   each with room to spare, and a fifteen-character name is two more entries in `EFI\BOOT`.

Roughly a hundred lines and a handful of host tests. The cheap part is that it is already
falsifiable by somebody else's driver: `tests/a_third_party_driver_reads_it.rs` writes an image
the host mounts, and a wrong checksum shows up as the alias appearing in `ls` instead of the real
name. That test is the whole reason this is a small piece of work rather than a risky one.

## BUGS

- **It does not by itself make riscv64 installable.** The second thing in the way is that the loader
  hands its own file over as a PVH module, which the device-tree architectures have no slot for; that
  is its own proposal, and this one is useless without it.
- The short alias is a policy nobody has chosen. `~1` is what every implementation writes and
  nothing here has to collide with an existing name, since the volume is created empty, so the
  numbering question that makes aliases hard elsewhere does not arise. If this crate ever grows a
  second file, it does.
- **Nothing here reads a long name**, and nothing should: this crate has no reader at all, and
  milestone 140 (mount a drive this system did not create) is the read half.

## Index row

`crates/file_allocation_table` writes 8.3 short names only, so it refuses `BOOTRISCV64.EFI` at eleven characters rather than mangling it into a file the firmware will not find. That refusal is correct and it forecloses exactly one thing: an installer on riscv64, since `install_service` is the only caller that writes FAT on the machine being installed. Long-name support in that crate, with host tests and the third-party reader already wired up, is what makes the install a three-architecture claim instead of an x86_64 one.
