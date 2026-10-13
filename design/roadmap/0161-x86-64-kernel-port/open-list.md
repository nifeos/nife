# Milestone 161 appendix: the open list's items 0 to 2, and the port-I/O question

An appendix to [milestone 161 (the x86_64 kernel port)](../0161-x86-64-kernel-port.md), moved out
of that page on 2026-10-12 (UTC) under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md).
The main page is complete without it. The block's open list ran in the order the work should be
done, because each item was a prerequisite for the next. Items 3 and 4 are in [ring-3.md](ring-3.md)
and [processes-and-userspace.md](processes-and-userspace.md), and items 5 and 6 in
[smp-and-vt-d.md](smp-and-vt-d.md). This file has the rest as the block recorded them in late
August 2026. Long sentences were split to meet
[§213 (writing standards)](../../decisions/0213-writing-standards.md); no fact changed.

## Item 0: the device-discovery seam

The device-discovery seam is the other abstraction
[milestone 20 (a portable HAL, proven on a second architecture)](../0020-portable-hal.md) promised.
Its narrow half is built (`memory::bring_up_page_frames`). Without it the frame allocator could not
come up on a machine with no device tree, and the port stopped there. `memory::init` is now
explicitly a device-tree front end, and `arch::x86_64::machine::bring_up_memory` is the x86 one.

The wide half was split into its own milestone,
[milestone 176 (the x86_64 discovery seam's wide half)](../0176-x86-64-discovery-seam-wide-half.md). Two of the four places this item named are
built:

- The interrupt controller is built, by item 2. The boot tour hands the MADT's answers to
  `arch::x86_64::irq` directly. That path goes around `memory.rs`'s statics rather than through them.
- PCI is wired, and has been since the ACPI/MCFG work of
  [milestone 165 (x86_64 PCI enumeration)](../0165-x86-64-pci-acpi-mcfg.md). `main.rs` calls `memory::record_pci_regions` from
  the ACPI `ecam` path, so `memory::pci_regions()` returns `Some` on x86_64.

Milestone 176 builds the other two windows. Piece 1 routes COM1's ISA IRQ into `memory::UART_IRQ`
(`Acpi::isa_irqs[4]`, via `memory::record_uart_irq`). Piece 2 is a CMOS RTC seam. The PC-compatible
CMOS RTC is two fixed I/O ports, not a page, so the "map the device, let userspace drive it" pattern
the other two architectures' RTCs use has no CMOS equivalent.
[DECISIONS §130 (how the kernel-resident CMOS RTC reaches the userspace clock service)](../../decisions/0130-cmos-rtc-delegation.md)
rules it (2026-08-26, "Ratify option 3"). The kernel reads CMOS once at boot and hands the seed to
the clock service as a `Spawn` argument, the same way `kind` already crosses that boundary.

notes/x86-port/acpi-and-pci.md has the table of which fact has which source, and milestone 176 has
both pieces.

## Item 1: fine-grained page tables, and the address-space layout they force

BUILT; [early-boot.md](early-boot.md) step 9 has what landed and what it cost. The number stays,
because the items cite each other by it.

The decision this item existed to record (two bases, Linux's) was taken as recorded. The hazard it
flagged was removed rather than sequenced around: `boot.s` installs the direct map's PML4 entry
itself, so `phys_to_virt` never changes meaning. `mmu::device_va` is gone.

What was still open was not the map but its leaves. `crates/paging` mapped 4 KiB pages and nothing
else, so the direct map cost 0.2% of RAM in page tables. 2 MiB and 1 GiB leaves were a change to the
shared format trait that all three architectures would want. They wanted their own milestone rather
than an x86 patch. In the end the 2026-09-19 lane built them here; see [closing.md](closing.md).

## Item 2: the IO APIC

BUILT 2026-08-24; [early-boot.md](early-boot.md) step 10 has it. The number was kept in place for
the same reason.

The trap this item existed to flag turned out to be exactly as stated, and is now proven rather than
asserted. q35's MADT says ISA IRQ 0 arrives as GSI 2. The resolution lives in
`machine_discovery::acpi::isa_irq_table`, where a host test holds it.

What was still open was PCI interrupt routing, which is blocked on AML rather than on tables. Also
MSI, which bypasses the redirection table entirely by writing a vector straight to the local APIC.
MSI is the path worth building for that reason. Neither was this item, and neither ever was. Both
were settled later by
[milestone 215 (a PCI function's interrupt reaches nothing on x86_64)](../0215-x86-64-pci-interrupt-routing.md)
and [milestone 459 (legacy INTx interrupt routing on x86_64)](../0459-legacy-intx-interrupt-routing.md).

## A capability shape for port I/O: not a step, and resolved

On the other two architectures a device is a page. So a device capability is a mapping, and the MMU
enforces it. x86's legacy devices, the console UART included, are in an I/O space with no page
tables. The only mechanism with the right granularity is the TSS I/O permission bitmap, which is
per-task rather than per-page. `user::UART_PHYS` is zero on this architecture, and that zero is the
marker.

[DECISIONS §121 (what a device capability is when the device has no page)](../../decisions/0121-port-io-capability.md)
answered it: DECIDED 2026-08-25, "Ratify option 2, permanently". x86's legacy port-I/O devices, the
console included, stay kernel-resident forever. No new capability object gets added. Only
memory-mapped devices get a userspace driver on this architecture. That is a permanent, recorded
parity gap rather than an interim stance.

The measured cost of the alternative supported closing it rather than leaving it open. The TSS
I/O-bitmap write costs ~2,682 ns per context switch, 423% overhead on the naive always-write
implementation. Nothing on the customer path is affected. Every device the Time Machine thesis
touches (network, disk, everything SMB needs) is already memory-mapped PCI/PCIe. Each already gets
the ordinary capability treatment identically on all three architectures. There is nothing further
to build here.

## Two arch-contract names, renamed

The two arch-contract names that did not stretch to a third architecture were `arch::psci_cpu_on`
and `kernel_main`'s `dtb` argument. They were renamed `arch::cpu_start` and `boot_info_pointer`; see
`notes/x86-port.md` and `arch/x86_64/mod.rs`.
