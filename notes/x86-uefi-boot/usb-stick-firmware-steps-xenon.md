# The stick procedure's firmware steps, as checked against the machine

*Name: ratified 2026-10-18 (calef, "Yes") Refused `stick-firmware-steps`.*

An appendix to [notes/x86-uefi-boot.md](../x86-uefi-boot.md), whose bench procedure keeps the four
steps in short form. This file holds each step as first written, what the machine's own menus said
about it, and the POST hazard the steps do not name.

Enter setup with F2 at the Dell splash; F12 is the one-time boot menu.

These four steps have now been checked against the machine's own menus rather than against Dell's
documentation, from the 70 photographs calef took on 2026-09-04. Three were right as written, and one
was worded differently. Both facts are recorded below. Every value each step asks for is already set
on xenon, page by page, in `notes/xenon-firmware.md`.

1. Secure Boot: off. This image is unsigned, and nothing in this tree signs it. On a 7050 that is
   *Secure Boot → Secure Boot Enable → Disabled*, and it may require *Boot List Option* to be UEFI
   first. Expect to have to do this. A Secure Boot machine will refuse the stick with a
   security-violation message and no other explanation. Right as written: the panel is
   `Secure Boot Enable`, with a Disabled/Enabled radio pair, and its own help text states the UEFI
   precondition. It is already Disabled.
2. Boot List Option: UEFI, not Legacy. Legacy/CSM boot would look for an MBR boot sector, which this
   stick does not have. Right as written, with one refinement. `Boot List Option` is not a page of its
   own; it is the lower half of *General → Boot Sequence*. On xenon the Legacy radio is grayed out,
   because `Enable Legacy Option ROMs` is off. Already UEFI.
3. Serial port: COM1. The C4PDJ module presents COM1 at I/O port `0x3f8`, which is where the kernel's
   console driver looks (`arch::x86_64::port`). This is the one that was worded differently. The old
   wording said "Serial port: enabled", and guessed that the firmware might offer an address or IRQ
   choice. It does not. *System Configuration → Serial Port* is a single five-way radio, `Disabled` /
   `COM1` / `COM2` / `COM3` / `COM4`, with no separate enable. The page's own help says
   `COM1 = Port is configured at 3F8h with IRQ 4`. So there is nothing to enable and nothing to enter;
   pick COM1, and the address and IRQ follow. Already COM1.
4. Leave the rest alone on the first attempt. In particular, do not disable the integrated NIC or
   change the SATA mode. Nothing here needs them, and a changed setting is one more variable in a
   bring-up that already has enough. Right as written: both settings exist under
   *System Configuration* (`Integrated NIC`, `SATA Operation`), and on xenon they are `Enabled` and
   `AHCI`. AHCI rather than `RAID On` is the reason the NVMe appears as an ordinary PCIe function.

## The hazard these four steps do not name

Only the transcription found it. xenon's `POST Behavior → Warnings and Errors` is set to
`Prompt on Warnings and Errors`, and `Keyboard Errors → Enable Keyboard Error Detection` is ticked.
So the machine stops at POST and waits for a keypress when it finds no keyboard. Its own event log
shows it doing that eight times over the past year. That did not bite during first light, because a
keyboard was attached. It will bite the first time anybody tries to power-cycle this machine and let
it boot unattended, which is what `notes/bench-runbook.md` and `notes/serial-less-output.md` both want
next. Changing it is two settings, and it is calef's call, because it changes the machine's behavior
for everything else it is used for.
