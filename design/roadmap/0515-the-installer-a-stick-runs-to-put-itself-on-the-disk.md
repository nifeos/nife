---
status: PARTIAL
raised: 2026-09-19
promoted_from: the-installer-a-stick-runs-to-put-itself-on-the-disk
milestone_dependencies: 261
decision_dependencies: 244
machine_requirements: x86_64 UEFI silicon with an NVMe drive
specific_machine: none
needs_person: yes
---
# 515. The installer: a stick that puts itself on the machine's disk and is then not needed

*(Number provisional until the merge queue lands it.)* Promoted from the
proposal `the-installer-a-stick-runs-to-put-itself-on-the-disk`, filed 2026-09-19, on calef's
instruction of 2026-09-20 to give every proposal on `main` a number. The text below is the
proposal's own, unedited except for this paragraph: the argument is its author's and promotion is
not the moment to improve it. Written by milestone 198 (a package manager, and the trivial install)'s rungs lane (`milestone/198-rungs-to-a-
trivial-install`) for rung 2 of the trivial install DECISIONS §157 (a trivial install is a web page,) defines. §157's table names the
installer as owned by no milestone; this is the proposal for it.

The on-disk layout of an installed system is read by every later nife that boots
from that disk, so once a stranger has installed it is a format (the layout fork below, options
only). The mechanism is reversible and is recommended. The bench half also needs milestone 261's
disk wipe, which is calef's act.

## Status, 2026-10-03: the QEMU half is built, and this block said NOT-STARTED for twelve days

Exit criterion 1 was built on 2026-09-21 in #1056 (`cargo xtask install-boot`, account in
`notes/installing.md`). The slots followed in milestone 525 (a bad upgrade cannot brick the
machine: two boot slots, tries and priority) and milestone 554 (a good upgrade sticks: what marks a
trial boot successful). This block was never moved, so it read NOT-STARTED while its own text
below said "Rung 2a was built". Re-run on `main` at `e613c520` on 2026-10-03:

| gate | result | wall time, warm, on patagonia |
|---|---|---|
| `cargo xtask install-boot` | PASS: offer, `INSTALL`, partition, `mkfs`, then the stick-less boot reads `made-on-target` (`1 10 57`) | 42.5 s |
| `cargo xtask rollback-boot` | PASS: a doomed slot 1 is abandoned, slot 0 boots by itself | not timed |
| `cargo xtask confirm-boot` | PASS: a good trial boot marks itself successful | not timed |

All three are x86_64 under OVMF. aarch64 and riscv64 have no installer. The gap is milestone 568
(the boot file has nowhere to go on a device-tree machine); the riscv64 file name it also needed
is milestone 560 (a long file name, or riscv64 cannot be installed), built. Criteria 2 and 3 are
hardware; criterion 2 was met on 2026-10-04 (see Follow-on), and criterion 3 is what keeps this PARTIAL.

## The layout ruling

Milestone 198's rulings table lists install layout as open, and milestone 525's `BUGS` calls the
on-disk format provisional. One layout is now built (data first, two 64 MiB raw boot slots, a
512 MiB FAT32 ESP holding the chooser), so the ruling is "ratify this, and pick a slot size". The
seven questions are answered in [the layout ruling](0515-the-installer-a-stick-runs-to-put-itself-on-the-disk/the-layout-ruling.md).
The slot size has the least room: the boot file grew from about 10 MiB to 15.6 MB in twelve days,
and an installed disk cannot grow its slots.

## What an install has to do, and how little of it is new

The finding that sizes this proposal: an installed system can boot the same single file the stick
boots. `uefi_loader` carries the kernel and the userspace archive inside `BOOTX64.EFI`
(`uefi_loader/src/main.rs`, "The kernel is embedded rather than loaded from the filesystem"), and a
boot with no filesystem still reaches the prompt (`crates/system_initializer`, the "4 without a disk,
5 with one" slot comments). So an install is four steps, and the tree already has two of them:

| Step | What exists | What is missing |
|---|---|---|
| 1. Write a partition table | `components/src/disk_partitioner.rs` (milestone 57 (partitioning and formatting a real drive, and extended attributes)): writes a GPT with both copies, draws GUIDs from an entropy endpoint, holds one disk and nothing else | It writes **one hard-coded layout** for a 64 MiB disk (`filesystem_contract::fixture::blank`: ESP, a Linux partition, nife data). A layout computed from the disk's size |
| 2. Make the data filesystem | `redoxfs_server/src/bin/mkfs.rs` (milestone 57): finds the nife data partition by §45's type GUID and creates RedoxFS inside it, bounded by a `PartitionDisk` | Nothing for step 2 itself |
| 3. Put `BOOTX64.EFI` on the EFI system partition | Nothing: there is no FAT writer (milestone 140's FAT32 stratum is `NOT-STARTED` and read-first) | Both the bytes of the file and a way to write FAT |
| 4. Make the firmware boot the disk | Nothing | A boot entry, or reliance on the firmware's fallback path |

And two pieces the installed system needs on its next boot, found by reading the boot path rather
than assumed:

- The boot mount is a whole virtio disk. `PartitionDisk` exists only inside `mkfs`; the FS
  service the boot builds mounts a whole block device and has no NVMe arm
  (`kernel/src/user/fs_service.rs` names neither). An installed system has to mount the nife data
  partition off the NVMe server's endpoint, or it boots to a prompt with no disk.
- The installer has to find the disk. ~~`block_roster` cannot name an NVMe disk, so this
  proposal depends on `a-block-roster-that-can-name-an-nvme-disk.md`.~~ It did not. Rung 2a was
  built on 2026-09-21 without it: the kernel hands the installer the NVMe endpoint directly and
  never consults the roster. The roster proposal stands on its own merits and is unaffected; what
  was wrong was this line, which assumed the only way to reach a disk was to look it up by name.

## The problem nobody had written down: the running system does not have its own file

Step 3 needs the bytes of `BOOTX64.EFI`, and the running system does not hold them. The loader
places the kernel's segments and hands over the archive as a module, then exits boot services; the
PE file that contained both is gone. And the file cannot be put inside the archive, because the file
*contains* the archive. So the installer needs a way to read the stick, and the options are:

| | Shape | Cost | Kept or lost |
|---|---|---|---|
| **R1. The loader hands its own file over** | Before `ExitBootServices`, `uefi_loader` opens its own file through the firmware (the loaded-image protocol names the device it came from, and the simple file system protocol reads it; both UEFI-standard, from the specification as recalled, not re-read) and passes the ~10 MB of bytes as a second module beside the archive | Two protocols added to a loader that uses seven function pointers today (`uefi_loader/src/efi.rs`); one more handoff field (`machine_discovery`, rule 7); the kernel hands the bytes to the installer as a read-only blob, which is milestone 233's precedent for `login` | **Recommended.** Smallest, and nife still does the install |
| R2. nife reads the stick itself | USB mass storage over milestone 242's host controller, then milestone 140's FAT32 reader | 242 is "months" by its own account and declines USB storage explicitly; 140's FAT32 is unbuilt | Lost for this rung: it makes rung 2 wait on the largest driver in the project for a file the firmware can read in one call |
| R3. The installer is a UEFI application | Runs before nife, writing the disk through the firmware's block I/O | Every step above rewritten against firmware protocols; nothing of milestone 57's confined partitioner and `mkfs` is used | Lost: it discards the one part of this that demonstrates anything (a partitioner holding one disk and no path to type), and §157 says the minimal system installs itself |

The equal-cost test, from §92 (a caretaker is supervised by the client it serves). R1 would still be chosen at equal cost: it keeps the authority story (the installer
holds one disk, one entropy endpoint and one read-only blob) and adds no driver whose only job is to
re-read what the firmware already read.

## The layout, which is the irreversible half (options, no winner)

| | Layout | What it buys | What it costs |
|---|---|---|---|
| **L1. ESP plus data** | GPT: an EFI system partition holding `BOOTX64.EFI`, and one nife data partition (§45) for everything else | The fewest moving parts. The same shape as milestone 57's fixture less its Linux partition | An upgrade rewrites the one file in place, so a bad upgrade has no previous file to fall back to |
| **L2. ESP with two slots plus data** | As L1, but the ESP holds the current and the previous `BOOTX64.EFI` (two names, and a boot entry or a small chooser for each) | A bad upgrade can be undone by booting the other file, which is what the activation fork's A2 wants at package level, one level down (Fuchsia's A/B slots and OSTree's previous deployment are the prior art the activation proposal already read) | Twice the ESP space, and a rule for which slot is current, which is itself a format |
| **L3. ESP with a small loader, system on a raw partition** | The ESP carries a loader that reads the kernel and archive from a raw nife boot partition | The ESP stops changing on upgrade; slots move into a partition nife owns | A second loader, and a raw-partition reader in it; the most new code of the three |

Two constraints hold for all three and are recorded rather than decided:

- The ESP must be FAT. Whether it is FAT32 or FAT16 at the sizes involved, and the minimum size
  firmware accepts, was not checked and is a first task for the lane.
- The logical block size. `disk_partitioner` and `disk_surveyor` both assume 512 bytes because
  nothing on the `blk` wire reports it (their `BUGS`). An NVMe namespace can be formatted with
  4096-byte blocks; the Micron 2450's format is not recorded in the tree. A table written in the
  wrong unit is unreadable by every other OS, so the lane reads the namespace's format from
  IDENTIFY before writing anything.

## The ESP's contents, and the boot entry (reversible, recommended)

Writing FAT without a FAT writer. A host tool builds an ESP image with the file's clusters
preallocated and contiguous and its bytes left zero. The archive carries that image (it contains no
copy of the file, so it does not contain itself). The installer writes the image into the ESP, then
writes R1's bytes at the one offset the host tool recorded. That is a layout two programs agree on,
both in this tree, so it is reversible. And it is fragile in the way it looks: it is a stand-in for
milestone 140's FAT32 write half and should say so at the call site.

The boot entry. Two ways, and the cheap one is unmeasured:

- B1. The fallback path on the internal disk. Put the file at `\EFI\BOOT\BOOTX64.EFI` on the new
  ESP and remove the stick. Whether xenon's firmware (Dell BIOS 1.27.0, `notes/xenon-firmware.md`)
  boots a fixed disk's fallback path without a `Boot####` variable is not known; many firmwares
  do (recalled, not read). One bench boot answers it.
- B2. Write a boot variable. `SetVariable` is a runtime service, and nife maps none: the loader
  never calls `SetVirtualAddressMap` (`uefi_loader/src/efi.rs`) and nothing in the kernel calls a
  runtime service (`git grep` for them finds only those two comments). So B2 is either a runtime
  services mapping in the kernel, which is new surface, or the loader writing the variable on some
  boot before `ExitBootServices`.

Recommendation: B1 first and measure it, on xenon and on one machine from milestone 243's fleet;
B2 only if a firmware refuses. Would B1 still be chosen at equal cost? Yes: a system that needs no
firmware variable survives a firmware reset and a disk moved to another machine.

## What a stranger's machine most plausibly has

A judgment, not a measurement. An NVMe disk, on anything sold in the last several years; xenon's
is a `Micron 2450 NVMe 256GB` on M.2 with SATA in AHCI rather than RAID (`notes/xenon-firmware.md`).
Two cases this proposal does not cover, recorded in `BUGS` below: SATA disks, which need an AHCI
driver nothing owns. And NVMe hidden behind Intel RST or VMD ("RAID On" in many laptops'
firmware), which milestone 261 already names as the reason xenon's AHCI setting mattered.

## Exit criteria a stranger could check

1. Under QEMU: OVMF boots the stick image with an empty NVMe disk attached. The installer runs,
   asks for confirmation naming the disk, partitions, formats and copies. The machine reboots with
   the stick detached and reaches the prompt. A file written before the reboot reads back after it.
   One `cargo xtask` gate, in the shape of `uefi-boot`.
2. On xenon, the same with a photograph, after milestone 261's wipe and bench boot.
3. On one machine that is not xenon, from milestone 243's fleet, which is what makes it a claim
   about PCs rather than about one Dell.

## Size, honestly

One milestone for the QEMU half: a computed layout in place of the fixture, R1's handoff and blob
grant, the ESP template tool, the installer program, the boot mount of a partition off NVMe, and the
gate. The bench half is milestone 261's remaining step plus one boot. It depends on
milestone 261 (the NVMe driver leaves the kernel)'s bench step, and no longer on `a-block-roster-that-can-name-an-nvme-disk.md`, which
rung 2a proved unnecessary by handing the endpoint over directly. It does not depend
on the network, on packages, or on milestone 242. An installer that asks for confirmation over the
serial console is rung 2 on xenon, and the same installer at a USB keyboard is rung 2 on a stranger's
PC once rung 1 has one.

## BUGS

- Whole disk only. The first installer replaces the disk's table. Installing beside Windows means
  resizing a filesystem nife cannot read, which is milestone 140's territory and a different order of
  risk to someone's data.
- An installer is the most destructive program this tree would ship, and the confirmation step
  is the whole of what stands between a stranger and a wiped disk. It must name the disk by model and
  size, not by an ordinal.
- SATA (AHCI) and RST/VMD disks are not covered. Nothing owns an AHCI driver; this is the home
  for that finding until a lane proposes one.
- The ESP template is a stand-in for writing FAT, and a file that outgrows its preallocated
  clusters breaks the scheme silently unless the host tool refuses it. Milestone 140's FAT32 write
  half is what retires it.
- Nothing here is crash-atomic, inheriting `disk_partitioner`'s own `BUGS` entry: a power cut
  mid-install leaves a disk that boots nothing. Real installers share the property; nobody has
  measured this one.
- B1 is unmeasured on any firmware but OVMF.
- The installed image has no USB keyboard until milestone 242 (USB host and HID) lands, so serial is
  the only input on xenon today.

## Follow-on

- **Done.** Met 2026-10-04 (UTC). Exit criterion 2, xenon: calef installed nife onto xenon's internal
  NVMe from a USB stick and booted it from the disk. Transcript (credentials redacted):
  [`bench/xenon-2026-10-04/install-and-disk-boot.log`](../../bench/xenon-2026-10-04/install-and-disk-boot.log).
  Boot 1 (stick) offered `TARGET: the NVMe disk attached to this machine, 256060514304 bytes`,
  the size risk 6's bench boots read that night, then `> INSTALL`, `installed. nife data at LBA
  2048, EFI system at LBA 499068928.`, `filesystem created.`, `DONE. Remove the installation
  medium and reboot.` Boot 2, after calef removed the stick and power-cycled, printed `boot file
  : none (this boot did not come from a file)`, offered no install, and reached the `$` prompt.
  - **The keystroke was a script's.** calef said "Lets do it" and asked the maintainer session to
    watch the serial output; a watcher on patagonia's serial port sent `INSTALL` after an exact
    size match. The person at the bench authorized the install; a script sent the keystroke.
  - **It depends on #1636.** The image was built from `lane/xenon-nvme-diag` (PR #1636, the VT-d
    `clflush` fix) at `0c16dcaac` with `cargo xtask install-boot` (QEMU gate PASS), sha256
    starting `69cc518b`. An install image from before that fix hits the NVMe CompletionTimeout
    on xenon, so this criterion holds only with #1636's fix merged.
  - **Read-back passed.** About 22:25 UTC the same day, xenon booted from its NVMe with no stick
    and reached `$`. Typed over serial (no USB keyboard yet): `ls` printed `made-on-target` and
    `operator/`; `wc made-on-target` printed `1 10 57`, the expected value. Transcript:
    [`bench/xenon-2026-10-04/readback.log`](../../bench/xenon-2026-10-04/readback.log).
  - **Photographs: owed.** calef's, not committed.
  Milestone stays PARTIAL on criterion 3.
- **Outstanding.** Exit criterion 3, a fleet machine that is not xenon, from milestone 243 (a machine with no serial port has no way to say anything)'s fleet.
- **Decision.** Ruled 2026-10-03 in `design/decisions/0244-the-installed-disk-has-four-partitions.md`: the layout above stands, and slots stay at 64 MiB. Two conditions remain with another lane: the installer installs a release image, and CI gates the release image at 16 MiB.
- **Milestone 560.** `BOOTRISCV64.EFI` is not an 8.3 name; the FAT writer's long-name entries carry it.
- **Milestone 568.** `/chosen` has one initrd slot, so aarch64 and riscv64
  have no installer.
- **Milestone 569.** Open as milestone 569 (the disk an installer names has no model, only a size).
- **Milestone 570.** Open as milestone 570 (the install offer should say what is already on the disk).
- **Milestone 572.** Open as milestone 572 (there is no way back from the stick: an installed disk is
  never offered an install again).
- **Milestone 712.** Milestone 712 (the install gates run nowhere, so rung 2a can rot without anybody hearing). Nothing runs these three gates on any schedule:
  `design/roadmap/0712-the-install-gates-run-nowhere.md`.

## Index row

The finding that sizes this proposal: an installed system can boot the same single file the stick
boots.
