# The ACPI walk that refused every table: an appendix to milestone 165

An appendix to [milestone 165 (x86_64 PCI enumeration)](../0165-x86-64-pci-acpi-mcfg.md). It holds
the bug, why no gate caught it, the fix, and the gate that now watches for it.

## The ACPI walk refused every table on a machine with real RAM

`arch::x86_64::machine::reachable` bounded the walk at 1 GiB, on a comment saying the boot map
"aliases only the low gigabyte". `boot.s` has never done that. It builds 2048 page-directory entries
of 2 MiB covering the low 4 GiB, and points both the identity entry and the direct map at them. It
says so in its own comment, because the APICs and the ECAM window are all above 1 GiB.

Nothing caught the disagreement, and the reason is the interesting part. Firmware places its ACPI
tables just under the top of RAM. Both QEMU runners pass `-m 256M`, so the tables landed at
`0x0fb7e014` and fit under a bound four times too small. Booted under OVMF with `-m 2048` they land
at `0x7fb7e014`. There the same kernel came up with no RSDP, no MADT, no MCFG and no DMAR. That
meant no APIC, no timer, no PCI and no VT-d, on a machine that had described all four. Every real
x86 machine has more than 1 GiB, so this was unconditional on xenon and invisible in every gate.

With the bound at the boot map's real extent, the same 2 GiB boot reads the XSDT, the MADT and the
MCFG. It enables the ECAM window at `0xe0000000`, brings up both APICs and the timer, and completes.

The gate moved to where the bug was, rather than a test being added beside it. The UEFI runner now
boots at 2 GiB by default (`NIFE_MEM` sets it back for the memory-map comparison in
notes/x86-uefi-boot.md). `cargo xtask uefi-boot` asserts the end of the chain, a PCIe window enabled
from an MCFG entry read at a high physical address. Falsified rather than assumed: with the bound
put back to 1 GiB, that gate fails on all three of its assertions. It also fails on the "outside the
boot map" line the walk now prints. That line matters because this is the one failure that looks
like a healthy boot from outside, a tour that completes on a machine that simply has no devices.
