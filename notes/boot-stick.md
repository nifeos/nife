# The boot stick, and the program that makes it

DECISIONS §157, as amended on 2026-09-19: a trivial install starts from one downloaded program per
host operating system, with the boot payload inside it, which writes a bootable stick when run. It
is not signed, and from any machine it builds a boot for any other machine. This note is how that
was built, what it was proved on, and what it was not. The measured device-tree handover is in
[`boot-stick/`](boot-stick/README.md). The roadmap block is
`design/roadmap/0441-the-program-that-makes-the-stick.md` (number provisional).

## In one paragraph

`cargo xtask stick` builds three UEFI boot files, one per architecture, and a program called
`stick_maker` (name provisional) that carries all three. Run on a Mac, a Linux PC or a Windows PC,
the program lists the USB sticks and SD cards on the machine, asks which, and writes the three
files to `\EFI\BOOT\` on it. Each machine's firmware finds the file with its own architecture's name
and ignores the other two, so the one stick boots an x86_64 PC, an aarch64 board and a riscv64
board, and nobody picks a target. Under QEMU, the stick the program writes boots all three
architectures to the userspace progenitor. On real hardware it has booted nothing yet.

## Using it

```console
$ cargo xtask stick                        # the three boot files, then stick_maker for this Mac
$ target/stick-maker/stick_maker-macos-arm64
nife stick maker, build 0d4399c98566
  aarch64  EFI/BOOT/BOOTAA64.EFI    16.0 MB
  riscv64  EFI/BOOT/BOOTRISCV64.EFI 14.4 MB
  x86_64   EFI/BOOT/BOOTX64.EFI     10.2 MB

Removable disks:
  1) disk6    Generic USB Flash              32.0 GB  USB            FAT "UNTITLED"
Which one? (1-1, or Enter to stop) 1
Will copy 40.6 MB of boot files to /Volumes/UNTITLED on disk6 (Generic USB Flash). Nothing on it is erased.
Type y to copy: y
  wrote /Volumes/UNTITLED/EFI/BOOT/BOOTAA64.EFI (16.0 MB)
  ...
Ejected disk6; it can be pulled out.
```

(The listing above is the program's real output shape; the disk in it is the synthesized stick of
the test fixtures, because no flash stick was attached while this was written. See BUGS.)

| Flag | What it does |
|---|---|
| `--list [--all]` | Show what would be offered; `--all` adds every disk that is not, with the reason |
| `--disk ID --yes` | No questions: copy onto that disk's FAT volume, never erasing |
| `--disk ID --erase` | No questions: erase that disk if it is not FAT, then copy |
| `--keep-mounted` | Do not eject afterwards |
| `--include-disk-images` | Also offer file-backed disks, which is how every test here runs |

What it leaves on the stick besides the boot files is `NIFE.TXT`: which build, and the SHA-256 of
each file. That retires, in part, `notes/x86-uefi-boot.md`'s BUGS entry *a stale `.efi` on a stick is
silent*: the stick now says what it carries, in a file any computer can open.

## Which disks it will touch, and the measurement that decided it

**A disk is offered only when its media is removable**: the SCSI removable-media bit that a USB
flash stick or an SD card reader sets. macOS reports it as `RemovableMedia`, Linux as
`/sys/block/<dev>/removable` (and an SD card's `device/type` of `SD`), Windows as `DRIVE_REMOVABLE`.
One rule, `crates/stick_maker/src/disk.rs`'s `refusal`, tested once.

**How the program is laid out, and why it matters for trusting it.** Every decision is in a file
the tests reach: the rule (`disk.rs`), what each host's tools say (`macos.rs`, `linux.rs`,
`windows.rs`, read from captured output), and the whole conversation with the person (`cli.rs`, run
against a fake host). `src/host/` only runs the tools, and is exempt from `script/coverage`'s floor
with the reason recorded there: the coverage run cannot execute `diskutil` or erase a disk. It is
run for real on each host instead, by `helpers/stick-maker-proof.sh` and the CI workflow.

The obvious rule was "external, on USB", and a USB hard disk refutes it. A removable USB flash
stick is offered; USB hard disks, which is where backups usually live, are refused:

```console
$ target/stick-maker/stick_maker-macos-arm64 --list --all
  disk2    Generic External USB Disk       1.0 TB  USB            FAT "EFI", unrecognized
           not offered: not removable media (a USB hard disk or SSD, which is where backups live)
  disk4    Generic Desktop USB Disk        2.0 TB  USB            FAT "EFI", Journaled HFS+ "..."
           not offered: not removable media (a USB hard disk or SSD, which is where backups live)
```

A USB hard disk reports `Internal = false`, `BusProtocol = USB` and `RemovableMedia = false`, so a
rule keyed on the bus would offer a backup disk for erasing. The test fixtures
(`crates/stick_maker/tests/fixtures/macos/`) are what `diskutil` printed for two such disks, with
names and sizes made neutral and every flag as captured. The listing above uses their names.

Three more refusals stand behind that one: a disk the host calls internal, a disk with a volume
mounted where the running system lives, and a file-backed disk unless asked for.

## Copy first, erase only when needed

| Path | When | What happens | Administrator rights |
|---|---|---|---|
| Copy | A mounted FAT volume with room | Files written under `EFI/BOOT/`, nothing erased | No |
| Erase | Anything else (exFAT, NTFS, APFS, no volume) | One FAT32 partition, MBR, then the copy | macOS no; Linux `sudo`; Windows not automated |

Each file is written to a temporary name, flushed, renamed over the old one and read back, so a stick
pulled out mid-write holds last time's file or this time's, never half of one.

**Erasing uses each host's own tool**, not a FAT32 writer in this tree: `diskutil eraseDisk` on
macOS, `sfdisk` and `mkfs.vfat` on Linux. The reason is not effort. A tree-written formatter would
mean this program opening a raw block device and writing to it, which is the most dangerous thing it
could do and the thing the OS tools already refuse to do to a mounted system disk. The cost is
stated: Linux needs `dosfstools`, which some minimal installs lack, and the program says which
package to install.

MBR, not GPT, measured rather than preferred: `diskutil eraseDisk ... GPT` writes a 200 MB `EFI`
partition ahead of the data one (both USB hard disks above have that layout). So the stick would
carry two FAT volumes, and a firmware that looks only at the first finds nothing. MBR gives one, and
U-Boot's distro boot and every UEFI implementation read it.

**Windows prints the `diskpart` steps instead of running them.** Automating it means mapping a drive
letter to a disk number and running destructive code on a host where it has never run once; that is
recorded in `crates/stick_maker/src/windows.rs`'s BUGS with what would promote it.

## The three boot files

| File | Built for | rustc target tier (read 2026-09-19) | Kernel entry contract |
|---|---|---|---|
| `BOOTX64.EFI` | `x86_64-unknown-uefi` | Tier 2, no host tools | PVH (`notes/x86-uefi-boot.md`) |
| `BOOTAA64.EFI` | `aarch64-unknown-uefi` | Tier 2, no host tools | arm64 `booti`: MMU off, `x0` = device tree |
| `BOOTRISCV64.EFI` | `riscv64imac-unknown-none-elf`, converted | **no riscv64 UEFI target exists** | riscv `booti`: paging off, `a0` = hart, `a1` = device tree |

The tiers are from `rustc --print target-list` on the pinned nightly, which lists `aarch64-`,
`i686-` and `x86_64-unknown-uefi` and nothing for RISC-V, and from the platform-support page. So the
riscv64 loader is linked as a static position-independent ELF and `cargo xtask stick` turns it into
PE/COFF through `object`: one section per segment, the ELF's 241 `R_RISCV_RELATIVE`
relocations turned into `IMAGE_REL_BASED_DIR64` fixups. It booted on its first run under EDK2.

All three are one loader source, `uefi_loader`, with the architecture-specific arms under
`uefi_loader/src/arch/`. The kernel is not modified on any architecture and cannot tell which loader
started it.

### How the device tree reaches the kernel on aarch64 and riscv64

The loader finds the tree in the UEFI configuration table under `gFdtTableGuid`, and copies it below
what the kernel's boot page tables reach, adding the initrd's bounds under `/chosen`. On riscv64 it
also asks which hart it is, through `RISCV_EFI_BOOT_PROTOCOL` or `/chosen/boot-hartid`, and prints
which answered. It then exits boot services and enters the kernel with the tree's address in `x0`
or `a1`, the MMU or paging off.

EDK2 on aarch64 hides the device tree when it presents ACPI, measured 2026-09-19. That was fixed on
2026-09-23: the loader now writes the tree from the ACPI tables
(`uefi_loader/src/device_tree_from_acpi.rs`). So `NIFE_ACPI=on helpers/qemu-stick.sh` reaches the
same prompt `acpi=off` does, with no PCI bus, since that needs an AML interpreter. Every aarch64
cloud machine is the `acpi=on` case, so this is the half of `notes/rented-metal.md`'s finding that
was ours to fix. [`device-tree-handover.md`](boot-stick/device-tree-handover.md) has the four steps,
the transcripts under EDK2, and what the two descriptions differ by.

### The seal, which is structural now

Each boot file is a loader carrying its kernel and the archive that kernel measures. The loader's
build refuses a pair that does not match (`uefi_loader/build.rs`, `refuse_an_unsealed_pair`). It
hashes the archive's `progenitor`, the six ELF child builders and
`program_measurements` and requires each digest to occur in the kernel image. Proved both ways on
2026-09-19: the current pair builds, and this tree's kernel beside the main checkout's two-day-old
archive fails with `NOT SEALED: the kernel ... does not vouch for the archive entry 'progenitor'`.
`stick_maker` embeds only finished boot files, so nothing downstream can pair a kernel with the
wrong archive: radon's boot 12 and xenon's 2026-09-17 refusal have no way to happen through it.

## Proved, and how

| Claim | How | Result, 2026-09-19 |
|---|---|---|
| One directory boots on all three firmwares | `cargo xtask stick-boot`: `target/stick` as a USB stick under OVMF, EDK2 aarch64 and EDK2 riscv64 | All three to the progenitor |
| The aarch64 stick boots from ACPI alone, as a cloud machine offers it | `cargo xtask acpi-boot`: `BOOTAA64.EFI` under EDK2 with `acpi=on`, once on QEMU's default GICv2 and once with `gic-version=3`, a CI gate in the `boot-check` job (2026-10-19) | Both to the progenitor, with the loader's "describing this machine from ACPI" line, and on the GICv3 its GICv3 and ITS lines |
| The program writes a stick that boots on all three | `helpers/stick-maker-proof.sh`: a blank file erased, a FAT32 file copied onto, each booted under all three | Six of six to the progenitor; a file already on the FAT32 stick survived |
| Nothing is erased without `--erase` or the disk's name typed back; a refused disk stays refused when named; a host that cannot erase says so before asking | `crates/stick_maker/src/cli.rs`'s tests, which run the whole conversation against a fake host | All pass |
| A backup disk is never offered | `crates/stick_maker` host tests on the captured `diskutil` documents, and `--list --all` on the development Mac | Every USB hard disk refused |
| A mismatched kernel and archive cannot be embedded | The loader's build against a two-day-old archive | Build refused, entry named |
| The x86_64 loader still boots after moving to `arch/x86_64` | `helpers/qemu-uefi-x86_64.sh target/esp` | Unchanged transcript |

## The host axis: what ran, what was only built, what was not built

| Host | Built | Executed |
|---|---|---|
| macOS arm64 | Yes, on the development Mac | **Yes**: discovery against the real disks, both paths against file-backed disks, the written images booted |
| macOS x86_64 | Yes, cross-built (`x86_64-apple-darwin`), and `lipo` joins the two when both are asked for | No (no Intel Mac here) |
| Linux x86_64 | Yes, cross-built static (`x86_64-unknown-linux-musl`, linked by `rust-lld`) | By CI, on a loop device: `.github/workflows/stick-maker-hosts.yml` |
| Linux arm64 | Yes, cross-built static (`aarch64-unknown-linux-musl`) | No |
| Windows x86_64 and arm64 | **Not from the Mac**: Rust's Windows targets need a MinGW runtime or the Windows SDK to link, and this Mac has neither. Type-checked and linted here for both | By CI on a Windows runner: built for both, discovery run |

```console
$ cargo xtask stick --host aarch64-apple-darwin --host x86_64-apple-darwin \
                    --host x86_64-unknown-linux-musl --host aarch64-unknown-linux-musl
```

Nothing is published. The downloads land in `target/stick-maker/`; putting them on a web page is
rung 4 of §157 and calef's act.

## At the bench: radon and argon

Nothing below has been run. Both boards reach the stick through U-Boot, and the five-command check
comes first (radon: notes/visionfive2.md, "To measure at the bench", item 10; argon: milestone
127's block): `usb start`, `usb storage`, `fatls usb 0:1 /`, `help bootefi`, `printenv boot_targets`.

### radon (VisionFive 2)

A microSD card is a stick too, and on radon it removes USB from the question entirely: U-Boot
reads the card already, every boot. So write the card with `stick_maker` (it offers SD cards; a card
that is already FAT32 is copied onto, and the existing `nife-vf2.img`, `nife-initial-ramdisk.img` and
`boot.scr.uimg` stay beside the new files), put it in, interrupt autoboot, and:

```
StarFive # load mmc 1:1 ${kernel_addr_r} /EFI/BOOT/BOOTRISCV64.EFI
StarFive # bootefi ${kernel_addr_r} ${fdtcontroladdr}
```

If `usb storage` found the stick in the five-command check, the same two lines with `usb 0:1` in
place of `mmc 1:1` boot it from USB.

| What radon prints | What it means |
|---|---|
| `nife uefi_loader: milestone 87`, then `boot hart from ...: N`, then the kernel's `hart N booted` | The universal stick works on radon; record which source gave the hart |
| `Unknown command 'bootefi'` | This U-Boot has no UEFI; radon stays on `script/board-image`'s card (U2) |
| The loader's line, then `uefi_loader: the firmware says neither ... which hart this is` | This U-Boot offers neither the RISC-V boot protocol nor `/chosen/boot-hartid`; the loader needs a third source, which is a finding to bring back |
| `uefi_loader: wanted 0x0000000080200000..` and a list of what is in the way | U-Boot's EFI memory map holds the kernel's fixed load address; the listed ranges are the answer |
| The loader's lines and then silence on the console | The handover or the kernel; the same triage as `script/board-image`'s boots, since from `hart N booted` on it is the same kernel |

The riscv64 boot file carries the `board` kernel, the one radon runs, and it was proved under
QEMU's EDK2 in that build. Whether radon's U-Boot 2021.10 offers `RISCV_EFI_BOOT_PROTOCOL` or
`/chosen/boot-hartid` is recalled rather than read: both are said to postdate or coincide with that
release, which is exactly why the loader prints which one answered.

### argon (Jetson TX1)

The aarch64 boot file cannot run nife on argon yet: its kernel is linked at `0x4008_0000`, QEMU
`virt`'s RAM, and argon's starts at `0x8000_0000`, which is the port milestone 127 (the seL4
machine: a Jetson TX1) owes. The check is
still worth making, because it shows whether L4T's U-Boot runs our loader and hands it a device
tree (U1's premise). The three commands and what each answer means are in
[`argon.md`](boot-stick/argon.md).

## EXAMPLES

Make a stick on this Mac, from nothing, and prove it before touching a real stick:

```console
$ cargo xtask stick
$ cargo xtask stick-boot
stick-boot: x86_64: booted from BOOTX64.EFI to the progenitor
stick-boot: aarch64: booted from BOOTAA64.EFI to the progenitor
stick-boot: riscv64: booted from BOOTRISCV64.EFI to the progenitor
$ helpers/stick-maker-proof.sh        # about twelve minutes: six boots, each held to its bound
```

Boot any stick image or directory under one firmware by hand:

```console
$ helpers/qemu-stick.sh riscv64 target/stick
$ NIFE_SMP=4 helpers/qemu-stick.sh aarch64 /path/to/written.img
```

Check what a stick carries, on any computer:

```console
$ cat /Volumes/NIFE/NIFE.TXT
$ shasum -a 256 /Volumes/NIFE/EFI/BOOT/*
```

## BUGS

- No real stick, card or board has been written or booted by any of this. Every write was to a
  file-backed disk, every boot was QEMU. The first physical stick is calef's, and so is the first
  board boot.
- The flash-stick fixture is synthesized, because no stick was attached when the fixtures were
  captured; its first line says so. Capture a real one (`diskutil info -plist diskN`) the first time
  one is at hand and replace it.
- Some flash sticks clear the removable-media bit (models sold as "fixed disks", for Windows To
  Go) and are not offered. There is no override flag, deliberately: the bit is what keeps backup
  disks out, and an override is the flag a stranger reaches for first.
- **Windows cannot erase**, and prints the `diskpart` steps; see `crates/stick_maker/src/windows.rs`.
- **Windows cannot be built on the development Mac.** CI builds it; a release process that wants all
  six downloads from one machine needs a MinGW or the Windows SDK there, or a Windows runner.
- The program is not signed, by decision (§157). macOS Gatekeeper refuses it on first open and
  Windows SmartScreen warns; the steps past each are rung 4's web page to write, and they move
  between OS releases.
- The downloads are large, and the profile is not chosen yet. Measured 2026-09-19 for macOS
  arm64: 41.6 MB from `cargo xtask stick` (debug payloads: 10.2, 16.0 and 14.4 MB), 20.0 MB
  from `cargo xtask stick --release` (2.6, 8.3 and 8.2 MB). Both sets boot on all three firmwares.
  The numbers move with every build, as `BOOTX64.EFI`'s already do in notes/x86-uefi-boot.md: the
  debug download was 45.1 MB the same evening, after rebasing onto that day's `main`.
  `cargo xtask stick` defaults to debug like every other xtask build, which is what the bench has
  used; which one a customer downloads is rung 4's choice.
- A universal macOS binary carries the payload twice, once per slice: 82.9 MB against 41.6 MB
  for the debug build. Carrying the payload outside the Mach-O slices would halve it, at the cost
  of the program being two files or reading itself.
- **The aarch64 payload runs on QEMU `virt` and nowhere else yet**, for the load-address reason under
  argon. The riscv64 payload is linked for `0x8020_0000`, which radon's RAM contains.
- Each QEMU proof boot runs to its time bound, because nife hands over to a shell and never
  exits; `stick-boot` takes about six minutes and the proof script about twelve. Watching the
  transcript and stopping at the progenitor line would cut both to seconds.
