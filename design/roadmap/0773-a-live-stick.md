---
status: NOT-STARTED
promoted_from: a-live-stick
raised: 2026-10-04
milestone_dependencies: 242, 400
decision_dependencies: none
machine_requirements: x86_64 UEFI silicon with a USB keyboard and a monitor
specific_machine: none
needs_person: yes
---
# 773. A live stick: boot a PC into nife without touching its disk

Asked for by calef (2026-10-04 UTC). Title, slug and every new name here are provisional.

## What "live stick" means

A stick that boots a PC such as xenon into a usable nife, runs from memory, and leaves the PC's own
disk exactly as it found it.

Exit criteria, each visible to a person at xenon:

1. With Secure Boot off, xenon starts the stick from the firmware's one-time boot menu.
2. The boot tour and then `$` appear on the monitor.
3. `echo hello` typed at a USB keyboard prints `hello`. No serial cable is attached.
4. A file written in the session reads back in the same session (`ls`, `wc`).
5. With nife installed on xenon's disk, the stick still boots the stick's own system. The loader
   says so on screen, and the installed system's files and boot-slot tries are unchanged when it
   next boots.
6. With the stick removed, the next boot is whatever xenon booted before.
7. Nothing is written to the internal disk unless the person types the install confirmation.

Under QEMU, a gate checks criterion 5 by hashing the NVMe image around a stick boot.

## What is already built

- The one-file boot. `\EFI\BOOT\BOOTX64.EFI` carries the kernel and the archive, sealed together
  (milestone 87 (the x86_64 bare-metal machine), BUILT). `stick_maker` writes it from macOS, Linux
  or Windows without erasing the stick (milestone 441 (the program that makes the stick), BUILT).
  It writes `BOOTX64.EFI`, `BOOTAA64.EFI` and `BOOTRISCV64.EFI` side by side, so one stick boots
  all three architectures. That is proved under QEMU only: no physical stick has booted argon or
  radon yet (`notes/boot-stick.md`).
- It runs from memory. The loader hands the archive over as a PVH module and reads its own file back
  as a second module while the firmware is still up (milestone 515 (the installer), R1;
  `notes/installing.md`). Nothing reads the stick after `ExitBootServices`. The stick can be pulled
  at the prompt.
- A boot with no disk reaches the prompt (`crates/system_initializer`).
- The installer stick is already a live stick when nobody types `INSTALL`. The offer waits 30
  seconds, then "anything else continues the boot" (`kernel/src/user/install_service.rs`).
- The prompt on the screen: milestone 400 (the shell on the firmware screen) is PARTIAL, built and
  gated under OVMF, xenon outstanding.
- The keyboard: milestone 242 (USB host and HID) is in the merge queue as PR #1629. Its BUGS say
  root ports only, one controller, one keyboard.

## The gaps, and who owns each

| Gap | What happens today | Owner |
|---|---|---|
| G1. A stick booted on an installed machine starts the installed system | The stick's loader is the same binary as the disk's. Its chooser finds boot slots on the NVMe, may spend a try (a GPT write), and chain-loads the slot (`uefi_loader/src/chooser.rs`, `choose`) | PR #1652 (open) |
| G2. A stick boot mounts the internal disk | With no virtio disk, `fs_service::wire_servers` falls through to `nvme_disk()`. If nife is installed there, `redoxfs_server` opens it with `cleanup: true`, which commits on mount (`redoxfs_server/src/lib.rs`, `open`) | PR #1652 (open) |
| G3. A boot with no disk has no filesystem at all | The shell gets no directory capability, so criterion 4 fails | Nobody |
| G4. The install offer cannot be answered at a USB keyboard | The kernel reads the UART before the progenitor exists (`install_service.rs` BUGS; `notes/xenon-bench-2026-10.md`) | Nobody |
| G5. The prompt on xenon's monitor | Built under OVMF only | Milestone 400, bench step |
| G6. A USB keyboard | In the queue | Milestone 242, PR #1629 |
| G7. Secure Boot on | Must be off | Milestone 500 (a stick that boots with Secure Boot on), NOT-STARTED |
| G8. Network | virtio-net only | Milestone 494 (the network card a PC has), PR #1632; Wi-Fi is a proposal in progress on PR #1650 (`wifi-on-a-pc-that-has-no-ethernet.md`) |
| G9. Saving files back to the stick | No USB mass storage, no writable FAT | Nobody; see Persistence |
| G10. The install path on aarch64 and riscv64 | No install offer: a device-tree handoff has no slot for the boot file. The FAT writer writes `BOOTRISCV64.EFI` with long-name entries (milestone 560 (a long file name, or riscv64 cannot be installed), built). Both run live meanwhile | Milestone 568 (the boot file has nowhere to go on a device-tree machine) |
| G11. Each board's install-target disk driver | Unsurveyed. Nobody has listed which disk argon and radon would install onto, or whether nife has a driver for it | Nobody |

G1 and G2 are this proposal's findings, read in the code (see BUGS).

G1 means criterion 5 fails today. It also corrects milestone 572 (there is no way back from the
stick), which blames the survey. The survey never runs: the chooser starts the disk's slot, which
"has no boot file" (`chooser.rs` BUGS), so the offer is skipped and 572's REPLACE is never reached.

On a disk carrying Windows, G2 is probably harmless: `FileSystem::open` should find no RedoxFS
header and fail before it writes.

### The fix for G1 and G2, recommended

One fact closes both: which disk this file came from. The loader already holds it, as its
`LoadedImage::device_handle` and that handle's device path.

- The chooser boots slots only from the disk its own file lives on. That is exactly what the
  chooser's first BUGS entry already asks for, for a different reason ("boot the slots on *this*
  disk"). A stick is never that disk, so a stick never chain-loads.
- The loader tells the kernel whether the boot medium is the NVMe disk, as one more field in the
  `machine_discovery` handoff. The kernel mounts the NVMe filesystem only when it is. Loader and
  kernel are sealed into one file (`refuse_an_unsealed_pair`), so nobody outside this tree reads
  the field, and it can change freely.

Lost: the firmware's `RemovableMedia` flag (a USB SSD often reports a fixed disk, recalled, not
measured); and a build-time "live" flag, which the installed copy would inherit.

### G3: a filesystem in memory, recommended

A block server over RAM, serving the `filesystem_contract::blk` wire, with `mkfs` run on it at boot. `redoxfs_server` and `mkfs` are reused unchanged. The live session then has a writable
filesystem that is gone at power-off. A provisional name for the server: `memory_disk`.

Size: 256 MiB, provisional; the lane sizes it from the memory map. Prior art, read: Redox's
`lived` (`storage/lived/src/main.rs`, MIT) serves the live image from memory with an overlay of
written blocks. nife needs no overlay, because its programs come from the archive.

## Persistence: ruled

**Ruled by calef on 2026-10-04 (UTC), late evening, on PR #1651: no persistence in v1.** The live
system's files live in memory only. His reason, in his words: *"Persistence seems neat. However it
seems only useful if programs carry over. Without the programs, the files aren't typically very
useful."*

### What a persistent stick is gated on

A persistent stick boots on any architecture and carries work between machines of any
architecture. By the ruling, that needs programs to carry over, not only files.
So it depends on milestone 198's packages being usable on every architecture from one stick. Three
options were recorded; two remain, and neither is chosen:

- (a) One package holds all three architectures' builds.
- (b) The stick records which packages are installed, and each architecture fetches its own build.
- (c) Programs in an architecture-neutral form, such as WebAssembly. Refused by calef on
  2026-10-05 (UTC): *"Wasm sounds like a horrible performance trade off for a neat feature. No
  thanks."*

#### A leading candidate, not a decision

Recorded at calef's request on 2026-10-05 (UTC). A hybrid of (a) and
(b): thin at the source, fat on the stick. The package server keeps one build per architecture,
and a normal install fetches only its own. Installing onto a persistent stick fetches all three,
because the stick may boot offline on any machine and has no distribution layer behind it. That is
`design/fat-binaries.md`'s conclusion applied: fat binaries are right "exactly when you cannot
interpose an intelligent distribution layer".

The evidence is n=3, a sample and not a law. The maintainer measured three programs built together
on 2026-09-29, in `target/<arch>-unknown-nife/release`, on 2026-10-05 (bytes):

| program | x86_64 | aarch64 | riscv64 |
|---|---|---|---|
| std_exerciser | 299592 | 412488 | 519352 |
| std_grep | 134168 | 209352 | 193032 |
| std_echo | 91976 | 170312 | 140208 |
| total | 525736 | 792152 | 852592 |

The fat sum is 2,170,480 bytes: 2.55x the largest build and 4.13x x86_64. Gzipped, the totals are
183068, 207023 and 242190, so fat is 3.45x x86_64: the slices share almost nothing. A package's
non-code content (documentation, data, manifest) is stored once, which lowers the ratio. Why the
aarch64 and riscv64 builds are larger than x86_64 is unexplained.

A refinement, recorded at calef's request on 2026-10-05 (UTC), also not a decision. He
suggested four versions per package: universal, aarch64, riscv64 and x86_64. The maintainer's
refinement is that universal is an index, not a fourth built artifact: a few hundred bytes naming
the three builds by digest, like a Docker manifest list. A built universal artifact would store
every program twice on the server, add a fourth digest to sign and trust under milestone 198's
run-by-digest rule, and could drift from the three it claims to hold. A normal
install reads the index and fetches its own build. An install onto a persistent stick fetches all
three, checks each against its own digest, and stores them side by side under one package name.

These are fat packages, not fat binaries. A fat binary is one executable with slices, like Mach-O
universal, and needs a new loader format; ELF has none (FatELF was proposed and rejected in 2009,
from memory). A fat package is three ordinary ELFs plus the index, chosen by the package manager
at install or activation, with no loader change.

#### What else it needs

A USB mass storage driver, a stick layout with a persistent area, and encryption at rest. A lost
stick holding someone's work is exposed, which is why Tails encrypts its area.

The stick layout v1 writes is provisional. Today that is `stick_maker`'s files in `\EFI\BOOT\` on
whatever FAT volume the stick already has, plus `NIFE.TXT`. v1 must not grow anything a persistent
area would have to work around.

#### The analysis put to calef

| | Shape | What it needs | Verdict |
|---|---|---|---|
| P0 | None. Everything is RAM; the stick is read once, by the firmware | G3 only | Ruled for v1 |
| P1 | A nife data partition on the stick, RedoxFS. Ubuntu's `writable` partition, Tails' Persistent Storage | USB mass storage (bulk-only transport and SCSI); 242's xHCI driver split into a host driver and class drivers, since keyboard and stick share one controller; hubs; `stick_maker` partitioning the stick | The named second rung |
| P2 | A filesystem image in a file on the stick's FAT volume. Fedora's old `--overlay-size-mb`, Ubuntu's `casper-rw` file | All of P1, plus a FAT32 writer for a foreign volume (milestone 140 (mount a drive this system did not create)) | Lost: P1's cost plus a second filesystem underneath |
| P3 | The loader reads a saved state file before `ExitBootServices` | No way to write back once the firmware is gone | Lost: read-only persistence is not persistence |

P1's xHCI split is Redox's shape (`xhcid` serves `usbscsid`), and it needs a destructive mode in
`stick_maker`, which today erases nothing. Fedora Media Writer writes no persistence ("not yet",
per Fedora Magazine), and Redox's live ISO runs from RAM. Tails never mounts the internal disk
unless the person does it by hand (`tails.net`), which is criterion 7's rule. At equal cost P1
would win, so the P0 recommendation was about effort.

## One image or two: ruled

**Ruled by calef on 2026-10-04 (UTC), late evening, on PR #1651: O1 on the universal stick.** One
image for all three architectures, built on milestone 441's `stick_maker`. It boots live by
default and offers the install at boot. On aarch64 and riscv64 there is no install offer until
milestones 568 and 560 land (G10); those two architectures run live meanwhile.

The options as put to him:

| | Shape | For | Against |
|---|---|---|---|
| O1 | One image. It boots live; the install offer is a timed question at boot, as today | Already built; one download, as §157 (a trivial install is a web page, a USB drive, and packages) wants; Haiku's shape | A 30-second pause on live boots with an NVMe disk; G4 must move the question to userspace |
| O2 | One image. Installing is a program typed at the prompt (`install`, provisional) | Fedora's and Redox's shape. No pause | The disk grant moves to the progenitor, and the confirm-before-grant property must be rebuilt there |
| O3 | Two images, live and installer | A live stick no keystroke can make wipe a disk | Two artifacts; installing from the installer image installs the installer |

The recommendation was O1, with O2 as later polish: G4 is needed by both, and once the question is
asked in userspace they differ only in when.

## The seven questions

1. Considered: P1 to P3 and O2, O3 above; for G1 and G2, `RemovableMedia` and a build flag.
2. The tree already boots one file from memory, and its installer stick already falls through to a
   live boot. The chooser BUGS already ask for the device-path match G1 needs.
3. Prior art, read on 2026-10-04: Tails (`tails.net`), Redox (`doc.redox-os.org/book/installing.html`;
   `lived` and `usbscsid` on `gitlab.redox-os.org`), Haiku (`haiku-os.org/get-haiku/installation-guide/`),
   Fedora (Fedora Magazine). Ubuntu only through secondary sources, as `help.ubuntu.com` returned
   503: a `writable` partition (`casper-rw` before 20.04) and a `persistent` boot parameter.
4. The premise "a stick boot never writes the internal disk" is false on an installed machine.
5. Cost was not measured, and no code written. G1 and G2 are one device-path walk and one handoff
   field, G3 one small block server, G4 one moved question. P1 is a driver family.
6. All of v1 is code in one sealed file. Publication (§157 rung 4) is the irreversible part, and
   calef's.
7. Answered per fork above.

## Reuse

Under §46 (thin primitives or whole subsystems) as amended 2026-10-04 (PR #1637, in flight):
take or adapt by default outside the kernel and the Kani-proved crates.

- `memory_disk` (G3): Reuse: adapted from Redox `lived`'s shape. It speaks nife's own `blk` wire,
  which nothing upstream reads, and it is small. `redoxfs_server` and `mkfs`: taken, unchanged.
- G1, G2: Reuse: write. A device-path walk in `uefi_loader` and a handoff field; loader-to-kernel
  wire (rule 7).
- USB mass storage (not v1): Reuse: adapt Redox `usbscsid` (MIT). `crab-usb` 0.12.1 (Apache-2.0)
  was read and has no mass-storage class.
- FAT (P2, lost): for milestone 140's lane, `embedded-sdmmc` 0.10.0 (MIT or Apache-2.0, `no_std`,
  read and write, released 2026-08) beats `fatfs` 0.3.6 (needs `core_io`, last released 2023-01).

## Ranking

The live stick is milestone 198 (a package manager, and the trivial install)'s rung 1 plus G1 to
G4. §157 called it "right destination, wrong first rung" for want of input drivers, and milestone
242 is now in the queue.

On principle 1 it sits on the customer path, ahead of rung 2b. A stranger with Windows on a PC
will try a stick that leaves the disk alone long before one that offers to wipe it (the installer is
whole-disk only, 515 BUGS). G1 and G2 rank first: they are small defects in what
already ships. Persistence ranks after rung 3c.

Proposed order: G1 and G2 (PR #1652), G3, G4 after 242 lands, then the xenon bench run of the
exit criteria.

## BUGS

- G1 and G2 are read from code, not seen on silicon. One bench boot of a stick on an installed
  xenon confirms them: the loader would print `boot slots on one disk`.
- G1 can write a stranger's disk today. Spending a try rewrites the GPT on a machine whose slot is
  in trial, from a stick the person thought was read-only.
- The live session loses everything at power-off under P0. The prompt should say so on boot.
- A disk carrying Windows is assumed safe under G2 from reading `FileSystem::open`. Nobody has run
  a stick boot against a non-nife NVMe image and compared hashes. Criterion 5's gate should include
  that case.
- The 256 MiB RAM disk size is a guess.
- G1 to G4 were read on the x86_64 path only. PR #1652 (open, calef's option K2) never mounts the
  internal NVMe on aarch64 or riscv64: `memory::booted_from_nvme` is set only by
  `arch::x86_64::machine`, and `fs_service::wire_servers` needs it. Checked against the diff, not
  run. The install offer is x86_64 only until G10 closes.
- SATA and RST/VMD disks are safe only by accident: nife has no driver for them (515 BUGS).
- A keyboard behind a hub is not found (242 BUGS). Many PC keyboards are behind one.

## Names (provisional)

"live stick", `memory_disk`, the `install` program in O2, and the handoff field for the boot medium.
None is ratified.

## Index row

A USB stick that boots a PC such as xenon into a usable nife from memory, leaving the PC's own disk exactly as found. It matters because it is the cheapest way for a stranger to try nife on a machine they own, which the claim that nobody needs nife (a non-fatal note in fatal risk 8's appendix) cannot be tested without.
