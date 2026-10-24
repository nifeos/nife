# Booting x86_64 from firmware: the appendices to notes/x86-uefi-boot.md

[`notes/x86-uefi-boot.md`](../x86-uefi-boot.md) is the page to read: the decision, the design, the
commands and the stick procedure for the bench. The files here hold the evidence and the history
behind each summary on it.

- [`the-fork.md`](the-fork.md): UEFI against GRUB, priced.
- [`loader-design.md`](loader-design.md): leaving long mode step by step, and the ELF reading.
- [`measured-under-ovmf.md`](measured-under-ovmf.md): the PVH-against-UEFI table, and the RAM
  reclaimed.
- [`running-under-firmware.md`](running-under-firmware.md): the two gates, what the suite cost, and
  SMP.
- [`first-light-2026-09-05.md`](first-light-2026-09-05.md): first light on xenon, the panic and its
  fix, and what xenon still has to establish.
- [`usb-stick-firmware-steps-xenon.md`](usb-stick-firmware-steps-xenon.md): the four firmware steps as checked against
  the machine.
- [`network-boot-xenon.md`](network-boot-xenon.md): the network-boot procedure in full.
- [`limitations.md`](limitations.md): the BUGS list in full.

Name: provisional, for the directory and every stem in it. The directory follows the `notes/<stem>/`
appendix convention §212 (a prose budget) set, so its name is the parent's stem. Each file is named
for its content. calef names directories and files, and `script/names --unratified` lists each stem.
