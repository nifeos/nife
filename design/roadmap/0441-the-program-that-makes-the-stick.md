---
status: BUILT
raised: 2026-09-19
built: 2026-09-19
promoted_from: a-program-that-makes-the-stick
---
# 441. The program that makes the stick: one download per host, a boot for every architecture

Built 2026-09-19, on `milestone/the-program-that-makes-the-stick`. *(Number provisional
until the merge queue lands it.)* Promoted from the proposal *A program that makes the stick*, which
this block replaces and which was deleted in the same change; DECISIONS §157 cites that path,
which is milestone 436's class and is left to it. Built and proved under QEMU and on file-backed
disks; no physical stick has been written and no board has booted one. Those are the bench's, and
the steps are in notes/boot-stick.md.

## What calef decided, and what this built

DECISIONS §157, amended twice on 2026-09-19: a trivial install starts from one downloaded program
per host operating system (macOS, Linux, Windows) with the boot payload inside it, which writes a
bootable stick when run. It is **not signed or notarized for now**. From any machine it builds a
boot for any other machine. The stick is for customers and for the lab alike.

What exists now:

| Piece | Where | Proved by |
|---|---|---|
| `stick_maker` (provisional), the host program | `crates/stick_maker` | host tests on captured `diskutil` documents and on the whole conversation against a fake host; `helpers/stick-maker-proof.sh` on macOS; `.github/workflows/stick-maker-hosts.yml` on Linux and Windows runners |
| `BOOTAA64.EFI` and `BOOTRISCV64.EFI` beside `BOOTX64.EFI` | `uefi_loader/src/arch/` | `cargo xtask stick-boot` |
| The universal stick (U1) | `cargo xtask stick` stages `target/stick` | all three firmwares boot the one directory, and the stick the program writes, to the progenitor |
| An ELF-to-PE converter, since rustc has no riscv64 UEFI target | `crates/portable_executable` (provisional) | host tests, and EDK2 loading its output |
| The seal, made structural | `uefi_loader/build.rs` refuses an archive its kernel does not vouch for | a mismatched pair refused, a matched one built |

## The universal stick, U1, and why it won

UEFI fixes a removable-media file name per architecture (`\EFI\BOOT\BOOTX64.EFI`, `BOOTAA64.EFI`,
`BOOTRISCV64.EFI`) and each firmware looks only for its own, so one FAT32 stick carrying all three
boots every UEFI machine and nobody picks a target. The proposal recommended it over U2 (the person
picks a target) on elegance, at equal cost: one layout, no per-board scripts. Built, it cost a loader
port per architecture, which is the same work U2 would have needed for any board that boots through
UEFI.

**U1's premise on the boards is still recalled, not read**: that radon's and argon's U-Boot builds
implement `bootefi` and hand over a device tree. The five-command check at each prompt answers it,
and notes/boot-stick.md has the table of what each answer means. **argon cannot run the aarch64
payload yet**, because the aarch64 kernel is linked at QEMU `virt`'s RAM
(`0x4008_0000`) and argon's starts at `0x8000_0000`; that is milestone 127's port, for `booti` as
much as for this. radon's RAM does contain the riscv64 kernel's `0x8020_0000`.

## The five questions the proposal left to a lane, answered

1. Finding the stick safely. One rule, the removable-media bit, read three ways (macOS
   `RemovableMedia`, Linux `/sys/block/*/removable` or an SD `device/type`, Windows
   `DRIVE_REMOVABLE`). Chosen against "external, on USB" because a USB hard disk reports USB and not
   internal while clearing the removable-media bit, and is where backups live: only removable media
   is ever offered, and a USB hard disk never is. Captured `diskutil` documents for USB hard disks are
   the test fixtures. Confirmation is by name and size, and erasing asks for the disk's identifier
   typed back.
2. Formatting: the OS's tools, `diskutil eraseDisk` and `sfdisk` plus `mkfs.vfat`, one FAT32
   partition on MBR. Not an in-tree FAT32 writer, and the reason is not effort: a tree-written
   formatter means this program writing a raw block device, the most dangerous act available to it
   and one the OS tools guard. MBR over GPT was measured: `diskutil`'s GPT layout puts a 200 MB `EFI`
   partition ahead of the data one. Windows prints the `diskpart` steps and does not run them, until
   it can be run against a VHD on Windows.
3. Building the host binaries: `cargo xtask stick --host <triple>...` from the Mac for macOS
   arm64 and x86_64 (joined by `lipo` into one universal binary) and static Linux x86_64 and arm64
   (musl, linked by `rust-lld`). **Windows does not link on the Mac**: Rust's Windows targets need a
   MinGW runtime or the Windows SDK. A Windows CI runner builds x86_64 and arm64.
4. Testing without a real disk: hdiutil-attached raw files on macOS, a loop device on Linux (in
   CI, as root). Windows is exercised through discovery only.
5. What it says when it finishes: ejects the stick and says how to boot it per architecture,
   Secure Boot off; and it leaves `NIFE.TXT` on the stick naming the build and each file's digest.

## What ran where

| Host | Built | Executed |
|---|---|---|
| macOS arm64 | yes | discovery on the real machine; erase and copy paths on file-backed disks; the written images booted on all three firmwares |
| macOS x86_64 | yes (cross) | discovery, under Rosetta 2 on the development Mac |
| Linux x86_64 | yes (cross, static) | CI run 35462277634: erase and copy paths on a loop device; the runner's disk refused as internal |
| Linux arm64 | yes (cross, static) | no |
| Windows x86_64 | CI only | CI run 35462277634: discovery; `C:` refused as the system, `D:` as fixed media |
| Windows arm64 | CI only | no (built, not run) |

## Follow-on

- **Recorded.** Windows does not erase; it prints the `diskpart` steps. What would promote it (a
  VHD on a Windows host to prove it against) is in `crates/stick_maker/src/windows.rs`.
- **Milestone 127.** argon cannot take the aarch64 payload because the kernel is linked at QEMU
  `virt`'s RAM; relinking or relocating it for argon's `0x8000_0000` is that port's work, and the
  limit is also recorded in `uefi_loader/src/arch/aarch64/mod.rs`.
- **Recorded.** The bench checks for radon and argon, each with a table of what the answer means,
  are in `notes/boot-stick.md`, beside `notes/visionfive2.md`'s item 10.
- **Recorded.** QEMU proofs run to their time bound because nife never exits (six minutes for
  `stick-boot`, twelve for the proof script); stopping at the progenitor line is in
  `notes/boot-stick.md`.
- **Recorded.** The universal macOS binary carries the payload twice (82.9 MB against 41.6 MB), in
  `notes/boot-stick.md`.
- **Recorded.** The flash-stick fixture is synthesized and says so:
  `crates/stick_maker/tests/fixtures/macos/usb-flash-stick-info.plist`.
- **Milestone 436.** Deleting the proposal leaves DECISIONS §157 citing a path that resolves to
  nothing, which is exactly the class that milestone collects and fixes; a lane may not edit
  `design/decisions/` in any case.
- **Decision.** Which profile the download is built in (41.6 MB debug, 20.0 MB release, both proved
  on all three firmwares), signing, and the web page are rung 4 of
  `design/decisions/0157-a-trivial-install-is-a-web-page-a-usb-drive-and-packages.md`, calef's.

## BUGS

- Nothing here has touched a physical stick or a board. Every write was to a file-backed disk and
  every boot was QEMU.
- The flash-stick fixture is synthesized and says so in its first line; capture a real one.
- Some flash sticks clear the removable-media bit and are not offered, with no override, on
  purpose.
- **Names are provisional**: `stick_maker`, `portable_executable`, `device_tree_patch`, `cargo xtask
  stick` and `stick-boot`, `helpers/qemu-stick.sh`, `helpers/stick-maker-proof.sh`, the download
  names `stick_maker-<os>-<cpu>`, and `NIFE.TXT`.

## Index row

The first rung of §157's trivial install: `stick_maker`, one program per host with every
architecture's boot file inside it, which finds removable disks by the removable-media bit (a
USB hard disk reports USB too, so the bus is never the rule). It copies onto a FAT32 stick without
administrator rights or erases with the OS's own tool, and it leaves a universal stick: `BOOTX64.EFI`,
`BOOTAA64.EFI` and `BOOTRISCV64.EFI` side by side, each firmware booting its own. The aarch64 and
riscv64 loaders are new (riscv64 through an ELF-to-PE converter, since rustc has no riscv64 UEFI
target), and the loader's build now refuses a kernel beside an archive it does not vouch for.
Proved under QEMU on all three firmwares from the stick the program wrote; no physical stick or board
yet.
