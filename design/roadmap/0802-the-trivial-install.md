---
status: NOT-STARTED
raised: 2026-08-30
milestone_dependencies: 242, 400, 515, 801, 809
decision_dependencies: none
machine_requirements: two x86_64 UEFI PCs, each with a USB keyboard and a monitor
specific_machine: none
needs_person: yes
---
# 802. The trivial install that makes a second customer possible: a web page a stranger follows

Rungs 1d and 4 of milestone 198 (a package manager), the "trivial install" half of 198's title, as
a milestone of its own. calef, 2026-10-06 (UTC): *"Don't fix the tooling to name a
rung. We should make our milestones finer grained if we're going to express dependencies on a
fraction of them."* The title and slug are drafts. `raised` is 198's, because this is the half of calef's 2026-08-30 sentence that is
still open: *"I don't think we expose nife to third parties (aka other customers) until we have a
package manager and a trivial install process."*

## What "trivial install" has to mean

A package manager without an install story is a mechanism nobody reaches. The constraint: a person
with hardware and no prior knowledge of this project reaches a running system. That is principle
3's test applied to running rather than to building. Today the answer is a `cargo xtask` invocation
on a development machine, which is not an install.

calef defined it on 2026-09-19 in
[§157 (a trivial install is a web page, a USB drive, and packages over the internet)](../decisions/0157-a-trivial-install-is-a-web-page-a-usb-drive-and-packages.md).
It is a web page from which one downloads a minimal system, writes it to a USB drive and installs
it. The
system then grows by installing packages over the internet.

## The rungs that are this milestone

[Milestone 198's rung table](0198-package-manager.md#the-rungs) maps every rung to its milestone.
Two are here:

| Rung | Ships | Exit criterion a stranger could check |
|---|---|---|
| 1d. A PC that is not xenon | Nothing new if 1a to 1c hold | The same stick on one fleet machine reaches `$` at its own keyboard and monitor |
| 4. The web page | A published release and a page | A stranger with a PC, a USB stick and no prior knowledge follows the page to rung 3c's result; the stranger harness (`notes/stranger-test.md`) runs against the download, not the build |

The others it waits on:

- rung 1b, milestone 400 (the shell on the firmware's screen)
- rung 1c, milestone 242 (USB host and HID)
- rung 2, milestone 515 (the installer)
- rung 3b, milestone 494 (a driver for the network card a PC actually has)
- rung 3c, milestone 801 (packages over the internet)
 Milestone 243 (a machine with no serial port has no way to say anything)'s fleet supplies
the second PC.

## What must be true before the page goes up

§157 makes publishing calef's act. What a lane can make true first, so the act is only a decision:

- Rungs 1 to 3 pass on a machine that is not xenon, with a USB keyboard, so milestone 242 is
  built.
- A release exists. `gh release list` returns nothing today (§157's measurement); the download
  carries a checksum, and a signature if the trust ruling's key signs it. The Secure Boot ruling
  creates no key.
- The page's first instruction is milestone 500's: turn Secure Boot off, after writing down the
  BitLocker recovery key.
- Something checks DECISIONS §135's requirement 1 ("no conveyed artifact carries copyleft") for
  the image the page conveys; §135's own `BUGS` says nothing enforces it.
- The stranger harness passes against the download, and records the time from page to prompt.
- calef holds `nifeos.org`. Whether the page lives there is calef's (§157).

## Which rulings it needs

| Ruling | The question | Where it stands |
|---|---|---|
| Secure Boot | Does a stranger turn Secure Boot off, or do we sign? | Ruled K1, turn it off, so nife holds no boot key. Milestone 500 (a stick that boots with Secure Boot on, or a page that says how to turn it off) is the record |
| Publication | Is it time to put the page up? | §157, step 1: calef's act |

Publication is `needs_person`.

Reuse: `stick_maker`, the installer and the package client are the tree's own and are built;
the release and the page are not surveyed yet, and the lane that builds them owes that survey under
§46 (thin primitives or whole subsystems).

The package client the stranger meets at rung 4 is `jig`, milestone 809 (the package client
becomes a program), not today's shell builtin, so this milestone depends on 809 (added 2026-10-07
(UTC), when 809 was promoted).

## Scope note (architectural parity)

The x86_64 UEFI PC in `machine_requirements` is §157's reference machine, the one a stranger most
plausibly owns. It is not a limit on the install, and the other two architectures are a gap with a
plan. Booting is not the gap: milestone 441 (the program that makes the stick) already writes
`BOOTAA64.EFI` and `BOOTRISCV64.EFI` beside `BOOTX64.EFI`, and each boots under QEMU's EDK2.
Installing is.

- aarch64. A UEFI machine boots `\EFI\BOOT\BOOTAA64.EFI` from the same kind of stick, but nothing
  offers the install there. A device-tree handoff has no slot for the boot file (milestone 568 (the
  boot file has nowhere to go on a device-tree machine)). An aarch64 server describes itself with
  ACPI instead (milestone 578 (an ACPI discovery path for aarch64)). The kernel is also still
  linked at QEMU `virt`'s RAM (milestone 441's note on milestone 127 (the seL4 machine)).
- riscv64. A board like radon (VisionFive 2) boots through U-Boot from microSD, so its procedure
  differs. radon runs `script/board-image`'s card today, and whether its U-Boot's `bootefi` runs
  the stick's `BOOTRISCV64.EFI` is unmeasured (notes/boot-stick.md). The installer's FAT writer
  writes that eleven-character name with long-name entries (milestone 560 (a long file name, or
  riscv64 cannot be installed)), and milestone 568 applies here too.

The plan: the x86_64 path ships first. Each other path is a follow-on, proposed with its own
procedure on the page when a machine a stranger plausibly owns exists for it.

## BUGS

- The rungs are sequenced on one Dell. Rung 1d is the second-machine criterion, and until it
  passes, every claim here is about xenon. A stranger's laptop may have no Ethernet (milestone 788
  (Wi-Fi on a PC that has no Ethernet)), a SATA disk (no AHCI driver; the installer's `BUGS`), or
  its NVMe behind Intel RST or VMD.
- Rung 1c is milestone 242, which milestone 192 (a keyboard on real silicon) prices at *"months rather than weeks"* and 242
  itself declines to price. This milestone is on its critical path.

## Index row

The door principle 1 names: calef, 2026-08-30, no third party sees nife until there is a package
manager and a trivial install. Milestone 198 keeps the package manager. A stranger follows a web
page from a download to a stick to an installed system that grows by packages over the internet
(§157). It holds rungs 1d (a second PC) and 4 (the page), and waits on the screen (400), the USB
keyboard (242), the installer (515), packages over the internet (801) and calef's act of
publication. Its first instruction is milestone 500's: turn Secure Boot off.
