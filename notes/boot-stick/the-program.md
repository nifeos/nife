# The program that makes the stick, file by file

An appendix to [`notes/boot-stick.md`](../boot-stick.md). It holds the design detail behind the doc
comments in `crates/stick_maker`, which link here.

## The halves

- `disk.rs` is the host-independent model. It holds the one rule for what may be written
  (`disk::refusal`) and the choice between copying and erasing (`disk::plan`).
- `macos.rs`, `linux.rs` and `windows.rs` each turn their host's description of its disks into
  `disk::Disk` values. Each is compiled only where it runs, and in tests. They also hold the few
  commands that change a disk.
- `payload.rs` is what is carried, and `write.rs` puts it on a mounted volume safely.
- `cli.rs` is the whole conversation with the person, tested against a fake host.
- `host/` is the thin half that runs each operating system's own tools. It is proved on each host
  instead of by the coverage run.

## The payload is embedded at build time

`build.rs` reads `NIFE_STICK_PAYLOADS`, a directory laid out the way the stick will be
(`EFI/BOOT/*.EFI`). `cargo xtask stick` builds that directory and then points the variable at it.
Every file under `EFI/BOOT/` is embedded at its relative path. With the variable unset, the program
builds with no boot files and refuses to write a stick, saying how to build one that has them. That
keeps `cargo build --workspace`, `cargo test` and `script/lint` working with no environment. The
loader's own `build.rs` uses the same arrangement.

## Why a kernel and its archive cannot arrive mismatched

Each boot file is one UEFI application. It already contains its kernel and the userspace archive
that kernel measures, and `uefi_loader/build.rs` refuses a pair whose archive the kernel does not
vouch for. The only unit this program handles is that sealed file, so it cannot pair a kernel with
the wrong archive. radon and xenon have both halted at `MEASURED BOOT REFUSED` on a mismatched pair
that a person copied by hand. This program has no way to express that copy.

## No stick shares a GUID with another

A prebuilt image written to every stick would give them all one disk GUID and one set of partition
GUIDs. Firmware boot entries, `PARTUUID` lookups and installs would then be ambiguous when two
copies meet one machine. Measured on 2026-10-06 (UTC), it does not happen here, for structural
reasons:

- There is no image. `cargo xtask stick` stages a directory of three `.EFI` files
  (`target/stick/EFI/BOOT/`), and the program copies files onto a FAT volume. Nothing in the build
  writes a partition table, so there is nothing to share.
- The copy path leaves the stick's own table alone, whatever its maker wrote.
- The erase path writes MBR, never GPT, with the host's own tool (`host/macos.rs`'s `erase` says
  why). So a stick has no disk GUID and no partition unique GUID at all.
- A disk installed from a stick is a GPT that `components/src/system_installer.rs` writes. It draws
  a fresh version-4 GUID for the disk and for each partition from the entropy service.

The evidence is the erase path, run twice on two 64 MiB file-backed disks (`hdiutil attach`, dummy
payloads, `--include-disk-images --erase`). Both came out MBR with LBA 1 all zeroes, so no
`EFI PART` header. Each held one type `0x0b` partition at sector 63. The FAT32 volume ids were
`0x89f41902` and `0x3d471902`. They differ, and that id is what Linux's `/dev/disk/by-uuid` and
macOS name a FAT volume by.

Both MBR disk signatures were `0x00000000`. So Linux gives every such stick's partition the same
`PARTUUID`, `00000000-01`, and a firmware boot entry that names the partition by MBR signature alone
cannot tell two apart. `diskutil` has no option to set the signature. Writing one ourselves means
opening the raw device, which needs root for a real stick, and the macOS path is built to need none.
Linux's `sfdisk` draws a random signature for a new `label: dos`; that is libfdisk's behavior, read
rather than run here.

Nothing in nife looks a disk up by GUID or MBR signature. `uefi_loader`'s chooser finds its own disk
by device-path prefix (`uefi_loader/src/device_path.rs`) and reads a table by type GUID.

## macOS

`diskutil` is in every macOS install and is Apple's supported interface to Disk Arbitration from a
script. The program shells out to it and reads its `-plist` output. The table `diskutil` prints by
default is for people: its column widths move with the longest name, and its wording is not a
contract.

`plist.rs` reads the XML itself rather than taking the `plist` crate. calef ruled it on 2026-10-20
(UTC), "Keep ours.", on milestone 784 (reuse what the tree wrote)'s item (f), under §46 (thin
primitives or whole subsystems). The `plist` crate (1.10.1) would bring nine crates new to the tree,
`serde` among them, to read a trusted local tool's output in a program that runs only on the host.
The grammar `diskutil` emits is eight element types and five character entities, fully specified by
Apple's DTD. Correctness is whether it reads what `diskutil` wrote, and the captured fixtures
answer that.

The two USB hard disks and the disk image in the fixtures are real captures, with identifiers,
names and sizes made neutral. The flash stick is synthesized, because no stick was attached when
they were captured. `diskutil eraseDisk` has been run by this program only on file-backed disks
(`hdiutil attach`), never on a physical stick; Apple documents the same command for both.

## Linux

Every fact the offer rule needs is a file under `/sys/block`, readable without root:

- `removable` is the SCSI removable-media bit.
- `device/type` says `SD` for an SD card on the MMC bus, whose `removable` is 0 even though the
  card comes out.
- The canonical path of the device says whether it hangs off a USB controller.

Mounts come from `/proc/self/mounts`. A filesystem that is not mounted is named by udev's database
in `/run/udev/data`, which is also readable without root. So listing needs no privileges and no
tool; only erasing does. `linux.rs` takes the three roots as arguments, so its tests build a small
fake sysfs rather than needing a Linux machine.

What has not been checked on Linux:

- The pure half is tested against a synthetic sysfs, not a live kernel. The layouts are the
  kernel's documented ABI (`Documentation/ABI/stable/sysfs-block`) and udev's database format. The
  udev format is recalled rather than read against a live system.
- Free space is not known before copying, because there is no `statvfs` without a dependency. A FAT
  volume that is too full is found by the copy failing, which leaves the previous files in place.
- Erasing needs root, `sfdisk` (util-linux, on every distribution) and `mkfs.vfat` (dosfstools,
  absent from some minimal installs). The program says which package to install.
- Without udev (some containers, Alpine with mdev) an unmounted partition's filesystem has no name.
  An unmounted FAT stick there is offered for erasing rather than copying. Mounting it first avoids
  that.

## Windows

Windows names volumes by drive letter. `GetDriveTypeW` answers `DRIVE_REMOVABLE` from the same
removable-media bit the other two hosts read. A USB hard disk answers `DRIVE_FIXED`, so the one rule
in `disk.rs` refuses it with nothing Windows-specific. The four functions used are in `kernel32`,
which every Windows process already links, so they are declared by hand rather than through a
bindings crate (§46).

Erasing is not automated on Windows, by choice. A stick that is not FAT gets the `diskpart` steps
printed, and the person confirms the disk to select by its size in `list disk`. Automating it needs
the drive-letter-to-disk-number mapping (`IOCTL_STORAGE_GET_DEVICE_NUMBER`). Destructive code that
has never run once on the host it targets is the one thing this program should not ship. Promote it
when a Windows machine can run it against a VHD (`diskpart`'s `create vdisk`), the host's
file-backed disk.

FAT32 through `format` stops at 32 GB, which is Windows' own limit and not FAT32's. The printed
steps therefore create a 1 GB partition, which the boot files fit in many times over.
