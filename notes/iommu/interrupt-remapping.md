# VT-d interrupt remapping

*Name: provisional, minted 2026-11-11 (UTC) by milestone/1068-vtd-interrupt-remapping.*

An appendix to [notes/iommu.md](../iommu.md), which links it from its VT-d section; the driver is
`kernel/src/arch/x86_64/iommu.rs`, and the driver's limits stay in that note's BUGS.

An MSI is a memory write to the interrupt address window, so DMA remapping does not confine it: a
component that writes a device's MSI-X table could aim it at any vector. Where the unit's `ECAP.IR`
and the DMAR's `INTR_REMAP` both offer remapping (Linux needs the flag too), `initialize` installs
an all-absent table of 256 entries, invalidates the interrupt entry cache, clears `GCMD.CFI` to
block compatibility-format messages, and sets `GCMD.IRE`. The interrupt entry cache has no
register-based invalidation, and section 6.5.2 forbids register-based invalidation once the queue
is on, so every unit with `ECAP.QI` invalidates through its queue.

`alloc_msi_vector(requester_id)` then writes one entry per grant: the vector, this core, and the
requester id with `SVT` = 01 and `SQ` = 00, so all sixteen bits must match. The device gets a
remappable-format message that names the entry and no vector (`remap_message`). The unit refuses
a forger that copies another device's message (fault reason 0x26, the entry's requester id is not
the forger's) and one that writes a compatibility-format message (reason 0x25). The bus server can
choose only which entry to name.

With `CFI` clear the IO APIC's own messages need entries too. Each line keeps one for good, naming
the requester id from the DMAR's IO APIC scope, and `route_gsi` writes the line in remappable
format (`remap_io_apic_line`). Where no DMAR scope names the IO APIC, remapping stays off, as in
Linux. Rewriting an unchanged entry writes nothing, and a moved line changes only the low half.

Two Kani harnesses hold the encodings. The test
`a_message_forged_from_another_devices_grant_is_refused` is step 5 of milestone 1068 (the kernel
turns interrupt remapping on where an IOMMU offers it).

QEMU 10.2's `vtd_interrupt_remap_msi` forwards a message without the format bit whatever `CFI`
says, so the test asserts the compatibility half only on bare metal (`CPUID.1:ECX[31]` clear) and
prints a line under a hypervisor.
