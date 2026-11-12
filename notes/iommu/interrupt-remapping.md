# VT-d interrupt remapping

*Name: provisional, minted 2026-11-11 (UTC) by milestone/1068-vtd-interrupt-remapping.*

An appendix to [notes/iommu.md](../iommu.md), which links it from its VT-d section. The driver is
`kernel/src/arch/x86_64/iommu.rs`. Its limits stay in that note's BUGS.

## Why DMA remapping is not enough

An MSI is a memory write to the interrupt address window, so DMA remapping does not confine it. A
component that writes a device's MSI-X table could aim it at any vector.

## Turning it on

Remapping needs the unit's `ECAP.IR` and the DMAR's `INTR_REMAP` both; Linux needs the flag too.
Then `initialize` installs an all-absent table of 256 entries and invalidates the interrupt entry
cache. It clears `GCMD.CFI`, which blocks compatibility-format messages, and sets `GCMD.IRE`.

The interrupt entry cache has no register-based invalidation. Section 6.5.2 forbids register-based
invalidation once the queue is on. So every unit with `ECAP.QI` invalidates through its queue.

## One entry per grant

`alloc_msi_vector(requester_id)` writes one entry per grant: the vector, this core, and the
requester id. The entry sets `SVT` = 01 and `SQ` = 00, so all sixteen bits must match. The device
gets a remappable-format message that names the entry and no vector (`remap_message`). The bus
server can choose only which entry to name.

The unit refuses two forgeries. A forger that copies another device's message faults with reason
0x26: the entry's requester id is not the forger's. A compatibility-format message faults with
reason 0x25.

## The IO APIC

With `CFI` clear, the IO APIC's own messages need entries too. Each line keeps one for good. It
names the requester id from the DMAR's IO APIC scope. `route_gsi` writes the line in remappable
format (`remap_io_apic_line`). Where no DMAR scope names the IO APIC, remapping stays off, as in
Linux. Rewriting an unchanged entry writes nothing. A moved line changes only the low half.

## What checks it

Two Kani harnesses hold the encodings. The test
`a_message_forged_from_another_devices_grant_is_refused` is step 5 of milestone 1068 (the kernel
turns interrupt remapping on where an IOMMU offers it).

QEMU 10.2's `vtd_interrupt_remap_msi` forwards a message without the format bit whatever `CFI`
says. So the test asserts the compatibility half only on bare metal, where `CPUID.1:ECX[31]` is
clear. Under a hypervisor it prints a line instead.
