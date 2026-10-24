# The two loader-to-kernel tokens on the command line

A page beside [`notes/boot-slots.md`](../boot-slots.md). It holds the design behind
`boot_slot::cmdline` and `boot_slot::medium`, the two words `uefi_loader` writes on the kernel's
command line for the running system to read.

## Why the slot travels as a word on the command line

A running system cannot work out which slot started it. The parent note's "What marks a trial boot
successful" gives the reason: the chooser spends a try before handing off, so the slot a
confirmation is for may no longer be the one `select` names.

`uefi_loader`'s `hvm_start_info` already carries a NUL-terminated command line. It already holds
more than one word: the screen, and `machine_discovery::framebuffer::SCREEN_HOLD` under the gate's
feature. The kernel already reads it before `mmu::init`. So `boot-slot=N` is a third word on a line
that exists, not a new agreement. The other shapes were priced against it:

| | cost |
|---|---|
| a field in `machine_discovery`'s handoff | a new field in a structure the PVH specification fixes, or a nife-private one beside it, and a second thing for a loader on each of three architectures to fill in |
| a module in the `hvm_start_info` module list | the list has two entries, and a third would be a page of memory and an entry format, to carry one nibble |
| a fixed physical page | a number two programs agree on with nothing checking it, and the one shape this tree has no precedent for |
| a word on the command line | one token, one parser, and a host test that round-trips it against the kernel's own reader |

The value is one decimal digit. `select_excluding` cannot exclude past slot 63, and
`system_installer` lays out two slots. A slot number that did not fit a digit would be a different
disk layout, so `encode` refuses rather than truncating. It returns 0 rather than panicking,
because its caller is a bootloader with no console left. A line it could not write is a boot that
does not confirm, which is the direction the whole feature fails in anyway.

## Why the boot medium is a token too

`boot-medium=nvme` is written by `uefi_loader` when the file it was started from is on an NVMe disk.
The kernel mounts the internal NVMe filesystem only when the word is there. It exists because
RedoxFS writes on mount: a stick that mounted the installed disk changed it with nobody asking.
`cargo xtask install-boot` measured that on its third boot, as three blocks written. Absence is the
safe answer, so a stick, a `-kernel` boot and any loader without the token all leave the internal
disk alone. It is G2 of the live stick proposal, and calef ruled the token on PR #1652 (2026-10-04
UTC).
