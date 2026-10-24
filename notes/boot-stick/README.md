# The boot stick: the appendices to notes/boot-stick.md

[`notes/boot-stick.md`](../boot-stick.md) carries how to make a stick, which disks the program
will touch, the three boot files, the seal, what was proved, the bench checks for radon and argon,
and BUGS. This directory carries the detail its prose budget has no room for.

- [`device-tree-handover.md`](device-tree-handover.md): the four steps that hand the kernel a device
  tree on aarch64 and riscv64, the EDK2 transcripts, and the ACPI case, measured on 2026-09-19 and
  fixed on 2026-09-23.
- [`argon.md`](argon.md): the argon bench check in full, its three U-Boot commands and what each
  answer means.
- [`the-program.md`](the-program.md): the design behind `crates/stick_maker`, file by file, with the
  GUID measurement and what each host's half reads.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

- `device-tree-handover` Name: provisional, for the handover the appendix measures. calef names
  files.
- `argon` Name: provisional, for the parent's argon bench check. calef names files.
- `the-program` Name: provisional, for the program the appendix describes. calef names files.
