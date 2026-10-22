# Installing nife onto a disk

Milestone 198 (a package manager, and the trivial install that makes a second customer possible),
rung 2a, built 2026-09-21. [Milestone 515 (a stick that puts itself on the machine's
disk)](../design/roadmap/0515-the-installer-a-stick-runs-to-put-itself-on-the-disk.md) is the
proposal this follows, and where it was priced; what is here is what was built, what it measured,
and the three things the building changed.

The exit criterion, from milestone 198's own rungs table, is a sentence a stranger could check:

> OVMF boots the stick image with an empty NVMe attached; the installer names the disk, asks,
> partitions, formats and copies; the machine reboots with the stick detached, reaches `$`, and
> reads back a file written before the reboot. One `cargo xtask` gate.

`cargo xtask install-boot` is that gate and it passes.

## Running it

```console
$ cargo xtask install-boot
--- boot 1 of 2: the stick installs itself onto an empty NVMe disk ---
  install     : this system was booted from a file and can install itself.
  install     :   TARGET: the NVMe disk attached to this machine, 1073741824 bytes.
  install     :   EVERYTHING ON THAT DISK WILL BE DESTROYED.
  install     :   Type INSTALL and press return to proceed; anything else continues the boot.
  install     : > INSTALL
  install     : partitioning and copying the boot file...
  install     : installed. nife data at LBA 2048, EFI system at LBA 1046528.
  install     : filesystem created.
  install     : DONE. Remove the installation medium and reboot.

--- boot 2 of 2: the same machine with the stick detached ---
BdsDxe: loading Boot0001 "UEFI QEMU NVMe Ctrl nife-nvme 1" from PciRoot(0x0)/Pci(0x3,0x0)/NVMe(...)
nife uefi_loader: milestone 87
...
$ ls
  made-on-target
$ wc made-on-target
  1 10 57
install-boot: PASS
```

The disk it wrote is left behind at `target/nife-install.img`, deliberately, because it is the
evidence. On macOS the EFI system partition inside it can be read by somebody else's FAT driver:

```console
$ dd if=target/nife-install.img of=/tmp/esp.img bs=512 skip=1046528 count=1048576
$ hdiutil attach -imagekey diskimage-class=CRawDiskImage -nobrowse /tmp/esp.img
/dev/disk7                                     /Volumes/NIFE
$ diskutil info /dev/disk7 | grep Personality
   File System Personality:   MS-DOS FAT32
$ cmp /Volumes/NIFE/EFI/BOOT/BOOTX64.EFI target/esp-install/EFI/BOOT/BOOTX64.EFI && echo same
same
$ hdiutil detach /Volumes/NIFE
```

## The pieces, and who may destroy what

| | holds | may destroy |
|---|---|---|
| `kernel/src/user/install_service.rs` | the boot path and the console | nothing: it has no block endpoint of its own to spend |
| `components/src/system_installer.rs`, `ROLE_SURVEY` | the disk | nothing it could write would be read back: no entropy endpoint, so no unique ids |
| `components/src/system_installer.rs`, `ROLE_INSTALL` | the disk, an entropy endpoint, a read-only copy of the boot file | that disk |
| `redoxfs_server`'s `mkfs` (unmodified) | the same disk, the same entropy endpoint | that disk |

**The question is asked by the service and not by the installer**, and that is the design rather
than an accident of where the console is. A confirmation a program prints to itself is a
confirmation that program could also decide to skip; asking before the capability is granted means
the authority to wipe the disk does not exist until the answer does.

**The survey role is what keeps that honest a second time.** An installed machine boots from a file
too, so an offer that did not look at the disk first would ask every installed machine, once per
boot, whether to wipe itself. Looking means reading a partition table, and the kernel does not parse
partition tables (notes/block-devices.md). So the looking is done by a process holding the disk and
no entropy endpoint: the `disk_partitioner` verify role of milestone 57 (partitioning and formatting
a real drive), one milestone along. It is the same demonstration: a program that cannot draw a
unique id cannot write a table anything reads
back.

## The three things the building changed

### 1. The running system did not have its own file, and now it does

This is milestone 515's central finding and it is worth restating because it is easy to meet twice.
`uefi_loader` places the kernel's segments, hands the archive over as a PVH module, and then the PE
file that contained both is gone. An installer must write that file to the new disk's EFI system
partition, and it cannot be carried inside the archive, because the file contains the archive.

So the loader reads it back off the volume it was started from, through `HandleProtocol` on its own
image handle and the boot volume's `SimpleFileSystem`, while the firmware is still up and can read a
FAT volume in one call. It hands it over as module 1; module 0 stays the archive, and the order
is a contract stated at both ends.

**The kernel and the archive therefore move as a set for free**, which was the failure this had to
make unexpressible. A disk carrying a new kernel beside an archive it does not vouch for halts at
`MEASURED BOOT REFUSED`, and that cost a boot on 2026-09-01. They are sealed together inside the one
file at build time (`uefi_loader/build.rs`'s `refuse_an_unsealed_pair`), and the installer copies the
file.

Two candidates were priced in milestone 515 and lost. Reading the stick over USB mass storage waits
on milestone 242 (USB host and HID), which milestone 192 (a keyboard on real silicon) prices at
*"months rather than weeks"*. Writing the installer as a UEFI application discards milestone 57's
confined
partitioner entirely.

It is x86_64 only. A device-tree handoff has one initrd slot in `/chosen` and no second one, so
`hand_over`'s boot-file argument is threaded through the other two architectures as `None` with the
gap recorded at the signature. The trivial install of §157 (a trivial install is a web page, a USB
drive, and packages) is a PC, which is why this is where it was needed first.

### 2. The data partition goes first on the disk, and that deletes three pieces of work

Every installer a reader has met puts the EFI system partition first. This one does not.

`redoxfs`'s own `FileSystem::open` scans blocks `0..65536` of whatever disk it is handed for its
header, and adopts the block it finds it at as the filesystem's origin. So **a RedoxFS that begins
inside the first 256 MiB of a disk is mounted correctly by a server that was given the whole disk.**
With the data partition at LBA 2048 it begins at block 256.

What that deletes from this rung: a partition-aware mount in `redoxfs_server`, and a base-block
field on the `blk` wire (a wire value two programs agree on, and therefore calef's). Also a
caretaker
process in the middle of every filesystem block. `kernel/src/user/fs_service.rs` grew an NVMe arm and
nothing else.

What it costs is real and is recorded in `system_installer`'s `BUGS`: the filesystem server on an
installed machine can address the EFI system partition and the partition table. What keeps it inside
the partition today is that `mkfs` created it bounded by a `PartitionDisk`, so its allocator never
learns about the blocks past the end. That is a property of the filesystem rather than a capability,
which is the wrong rung of AGENTS.md's ladder. Closing it is a lane of its own and it is the first
thing a
reader of this page should consider taking.

### 3. FAT32 had to be written, and the alignment is the part that bites

Nothing in this tree could write a FAT volume. Every path around it (QEMU's `vvfat`, macOS's
`diskutil`, Linux's `mkfs.vfat`) is a host tool the target cannot reach. The FAT32 stratum of
milestone 140 (mount a drive this system did not create) is read-first and unbuilt. `crates/file_allocation_table` is the smallest thing that closes
it: it creates one volume holding one file at one path, as pure computation with no I/O, and is then
finished.

Two numbers in it are load-bearing and neither is obvious.

**The cluster count decides the FAT type**, whatever the boot sector claims. Microsoft's own
specification defines it that way and drivers in the field follow; below 65525 data clusters the
volume *is* FAT16. At 4096-byte clusters that floor is a little over 256 MiB, which is why the EFI
system partition the installer lays out is 512 MiB and not the 100 MiB a reader expects. The
crate refuses to build a smaller one rather than handing firmware a lie about itself.

**The data area has to be cluster-aligned, and FAT32's own sizing arithmetic does not align it.**
The specification's formula produced 1023 sectors per table on a 512 MiB volume, and
`32 + 2 * 1023` is not a multiple of eight, so cluster 5 began at partition sector 2102. The
installer writes whole 4096-byte blocks through `filesystem_contract::blk`, so
`2102 / 8` truncated and ten megabytes landed six sectors early on a real disk. The file was
there, complete, and shifted; the firmware would not start it.

Three things came out of that one bug, in the order AGENTS.md's ladder asks for them. The crate now
rounds the table up until the data area starts on a cluster boundary, which is what every modern
`mkfs.vfat` does. A host test checks the alignment across seven volume sizes. And the installer
refuses a volume whose file does not land on a transfer block, rather than dividing and writing
somewhere else.

## What was measured, and by whom

The claims here are worth separating by who is doing the checking, because the weak ones look like
the strong ones from a distance.

| claim | checked by | strength |
|---|---|---|
| the FAT volume's bytes match the specification | this tree's own unit tests | weakest: a writer checked against its own reading of the spec |
| the volume is FAT32 and the file reads back byte-exact | macOS's `msdos` driver, 2026-09-21 | strong: somebody else's driver, and it names the FAT type independently |
| the installed disk boots | OVMF, with its variable store deleted first | strong: the firmware found `\EFI\BOOT\BOOTX64.EFI` with no `Boot####` variable and no `bootindex` |
| the filesystem survived the reboot | the installed system's own shell: `ls`, then `wc made-on-target` → `1 10 57` | strong: the file was written by `mkfs` on the previous boot |
| a stick booted on the installed machine boots the stick, not the disk's slot | OVMF, boot 3 of `install-boot`, the disk a second boot option | strong for G1 of the live stick proposal; the loader says `the boot slots here are on another disk` |
| that stick boot leaves the disk unchanged | the same boot, comparing the image byte for byte | strong: byte-identical since the boot medium token, below |
| any of this works on real firmware | nothing | rung 2b, on xenon, and it is somebody's hands rather than a lane's |

The third row is milestone 515's B1, which that proposal recommended and called unmeasured on
anything but OVMF. It is now measured on OVMF and on nothing else. The deletion of the variable store
is what makes it a measurement at all. The default store persists across runs in a checkout, so
without it the second boot would have been riding a boot option the first one left behind.

## What this does not do

**Whole disk only**, and that one has no proposal. Installing beside another operating system means
resizing a filesystem nife cannot read. That is the territory of milestone 140 (mount a drive this
system did not create) and a different order of risk to somebody's data.

Everything else this rung found that it did not do is written where the next person meets it. That
is the `BUGS` section beside the feature and, for the five that want work rather than only a record,
as a
proposal in `design/roadmap/proposals/`. The five, and what each is actually about:

| proposal | the thing it is about |
|---|---|
| `the-disk-an-installer-names-has-no-model.md` | the offer says "the NVMe disk attached to this machine", which is a guess on the first machine with two. The single most load-bearing sentence a stranger reads |
| `the-install-offer-should-say-what-is-already-on-the-disk.md` | the survey asks only whether nife is there, so a disk holding Windows is described as an unqualified target |
| `there-is-no-way-back-from-the-stick.md` | an installed disk is never offered an install again, which is right, and turns a power cut between the installer and `mkfs` into an unrecoverable state |
| `a-long-file-name-or-riscv64-cannot-be-installed.md` | `BOOTRISCV64.EFI` is not 8.3; built as milestone 560 (a long file name, or riscv64 cannot be installed), so the FAT writer gives it long-name entries |
| `the-boot-file-has-nowhere-to-go-on-a-device-tree-machine.md` | `/chosen` has one initrd slot and no second one, which is why this rung is an `x86_64` claim |

A stick boot used to write the installed disk (the live stick proposal's G2, measured 2026-10-04).
With no virtio disk the kernel mounted the NVMe filesystem whatever the boot medium was, and RedoxFS
writes on mount. Boot 3 of `install-boot` changed 3 blocks of the nife data partition with nobody
typing anything. calef ruled the fix on PR #1652, 2026-10-04 (UTC): the loader writes
`boot-medium=nvme` (provisional, `boot_slot::medium`) when its file is on an NVMe disk, or when a
chooser started it from a slot. The kernel mounts the NVMe only when that token is there. A
stick, a `-kernel` boot and the device-tree architectures leave the internal disk alone. Boot 3 now
finds the disk byte-identical.

The sixth, the partition-bounded mount, is a `BUGS` entry in `system_installer` rather than a proposal,
because what closes it is a wire value two programs agree on and that is an architect's to name
rather than a lane's to propose.

## See also

- [Booting x86_64 from real firmware](x86-uefi-boot.md): the loader this builds on, and the bench
  procedure.
- [The boot stick, and the program that makes it](boot-stick.md): how the file gets onto a stick in
  the first place, which is rung 1.
- [Block devices: what is attached, and what holding one means](block-devices.md): milestone 57's
  partitioner and surveyor, whose authority split this inherits.
- [NVMe: the first non-virtio disk](non-volatile-memory-express.md): the disk this installs onto and
  the confined server that serves it.
