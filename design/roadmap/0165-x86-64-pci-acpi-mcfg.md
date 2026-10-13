---
status: BUILT
raised: 2026-08-24
built: 2026-09-02
---
# 165. x86_64 PCI enumeration: wire `kernel/src/pci.rs` to ACPI's MCFG

BUILT. Of the three things it owed, two are built elsewhere (PCI interrupt routing by milestone 215
(a PCI function's interrupt reaches nothing on x86_64), VT-d by milestone 161's item 6) and the
third, a real BAR placement, is closed below, measured under real firmware.

It checks whether x86_64 can reach a real-hardware data point for [DECISIONS §86 (whether an NVMe
driver can leave the kernel)](../decisions/0086-el0-nvme-driver.md) faster and cheaper than a
from-scratch JH7110 driver would. The reason to expect so: x86's PCIe discovery is ACPI-described
and architecturally identical between QEMU's `q35` and real x86 hardware (unlike RISC-V's QEMU-only
fake ECAM device).

It needed no syscall surface, new dependency or decision section of its own. What it unblocks (real
NVMe-behind-IOMMU on x86) is gated on VT-d, item 6 of milestone 161, which is built.

## The gap, as `notes/x86-port.md` already named it

Milestone 161 (the x86_64 kernel port) landed real ACPI table parsing (the RSDP scan, the root
table walk, the MADT, and the MCFG) against real hardware evidence (the Dell OptiPlex 7050 Micro,
milestone 87 (the x86_64 bare-metal machine)).
`notes/x86-port/acpi-and-pci.md`'s "discovery seam" section recorded
the gap this milestone closes: "The PCIe ECAM window the MCFG describes is exactly the constant
`arch::mmu::PCI_ECAM_PHYS` hardcodes... The constant should come from here rather than being checked
against here, and doing that is a line of code once something consumes it." `kernel/src/pci.rs`
itself said the same thing at its own probe: "A machine whose tree has no such node... answers every
probe here with nobody home", true of x86_64 unconditionally, since it has no device tree at all.

## What was built

1. The seam. `memory::record_pci_regions` fills `memory::pci_regions()` directly (the same
   static every probe in `pci.rs` already reads), the x86_64 counterpart of how `memory::init` fills
   it from a `pci-host-ecam-generic` device-tree node on the other two architectures. Called once
   from `main.rs`'s boot tour, right after `read_acpi`, with the ECAM window from ACPI's MCFG and a
   hardcoded BAR window (see the BAR paragraph below).

2. The real complication, found by measurement rather than assumed away. Reading ACPI's MCFG is
   not sufficient by itself. QEMU's monitor was used to check directly (`xp` on the physical address
   the MCFG names, before any kernel code ran): the address **faults** ("Cannot access memory"),
   not the all-ones an absent PCI *device* would read. The chipset's ECAM decode is off until
   something programs the host bridge's `PCIEXBAR` register (offset `0x60` in bus 0, device 0,
   function 0's configuration space): a real BIOS or UEFI does this before ever handing control to
   an OS, and PVH (the protocol this port boots through) is a hypervisor entry point, not firmware,
   so nothing has. `arch::x86_64::machine::enable_pcie_ecam` programs it through the legacy
   `CONFIG_ADDRESS`/`CONFIG_DATA` ports (`0xcf8`/`0xcfc`, already-existing primitives in
   `arch::x86_64::port`, written for exactly this and unused until now), and the same address was
   then confirmed, still via the monitor, to read the host bridge's own vendor/device id
   (`8086:29c0`) once the register was enabled. This is very likely a PVH-only step: real firmware
   (what milestone 87's OptiPlex will boot through) does this already, and rewriting the same base to
   the same value there should be a no-op. It is written unconditionally rather than assumed so this
   port does not depend on that being true.

3. **The BAR window is hardcoded, not discovered, and this is a real, permanent limitation rather
   than a temporary gap.** ACPI's MCFG names only the ECAM window; the 32-bit MMIO window BARs are
   placed in lives in the PCI host bridge's `_CRS` object on a real ACPI machine, which is AML, and
   this kernel has no AML interpreter (`notes/x86-port/acpi-and-pci.md`'s own "What is deliberately not decoded").
   `arch::x86_64::mmu::PCI_BAR_PHYS` is a hardcoded constant (`0xc000_0000`, q35's conventional PCI
   hole), confirmed disjoint from RAM, ECAM, the HPET and both APICs by reading QEMU's `info mtree`,
   but **not exercised by an actual BAR placement**: no PCI function that needs one is on the bus
   under the current runner (see below).

4. Proof, under QEMU's `q35`. `kernel::pci::tests::acpi_mcfg_wires_a_real_ecam_window_that_finds_the_host_bridge`
   (new, x86_64-only) enumerates the bus through the ACPI-sourced, now-enabled ECAM window and
   asserts it finds q35's own host bridge (bus 0, device 0, function 0, vendor `8086`, device
   `29c0`), a chipset function present regardless of what `-device` flags the runner does or does
   not pass. `script/test` on all three architectures: aarch64 295/1, riscv64 298/1, x86_64 99/6 (one
   skip converted to one pass versus the pre-existing baseline; no regressions). `script/lint` green.

## What was not built, and what this means for §86, in appendices

- [`not-built.md`](0165-x86-64-pci-acpi-mcfg/not-built.md): the runner this block left alone, and the
  NVMe attach it investigated and did not make.
- [`decision-86.md`](0165-x86-64-pci-acpi-mcfg/decision-86.md): which half of the evidence
  [DECISIONS §86](../decisions/0086-el0-nvme-driver.md) waits on this work reaches, and which half it
  cannot.

## What 2026-09-02 found, and what it corrects

Three things, and the first of them is why this block did not stay closed as a wiring exercise.

The first, the ACPI walk refusing every table on a machine with real RAM, is told in
[`acpi-walk-bound.md`](0165-x86-64-pci-acpi-mcfg/acpi-walk-bound.md) with the gate that now watches
for it.

### The PCIEXBAR write was unconditional, and the fault that justified it does not reproduce

`enable_pcie_ecam` wrote the register unconditionally with a 256 MiB length field, reasoning (in
this block, above) that rewriting the same base under real firmware "should be a no-op". It is a
no-op only if the length agrees too. Firmware may size the window at 128 or 64 MiB, and the write
then widens the chipset's decode over whatever physical addresses sit above it, which on xenon
is discovered at a null modem. Chipsets that lock the register after firmware writes it are the
other half: there the write is dropped and only a read back would ever say so.

It now reads first, treats "enabled at the base the MCFG itself reports" as firmware's own work, and
writes only otherwise, with the length derived from the MCFG's bus count. A bus count the register
cannot encode leaves the window as found and says so.

And the measurement above is corrected. Re-measured on QEMU 11.1.1 from inside the guest, which
is the side being served, the register reads `0xb0000001` on the PVH path **before anything writes
it**, and the whole suite's PCI half (the MCFG witness, NVMe, both userspace PCIe driver tests)
passes with this function writing nothing. The monitor still answers "Cannot access memory" at that
address on the same boot, so the monitor and the guest disagree, and this block's original reading
of the monitor as evidence about the guest's decode does not hold. Nothing needs to decide whether
QEMU changed or the inference was always wrong, because the register is asked rather than assumed.
The consequence to record is that the writing arm is now unexercised on both paths this kernel
boots, and stays for the machine that genuinely arrives with the decode off.

### The BAR window: measured under real firmware, and it is not a corner case

Item 3 above called the hardcoded BAR window "a real, permanent limitation" and noted it was not
exercised by an actual placement. It is exercised now (milestone 215 drives a `virtio-blk-pci` disk
whose BARs this kernel places), so the open question changed shape: not whether the placement works,
but how much of the machine it moves, and whether the place it moves things to is free there.

`pci::bar_census` (provisional name) answers both in one read-only pass of config space, printed on
the x86 boot line: functions on the bus, and functions carrying a BAR outside the window
`mmu::map_everything` maps. Measured 2026-09-02: 5 of 8 under PVH, 3 of 6 under OVMF, 4 of 7 with
a `virtio-blk-pci` disk attached. Real firmware placing the addresses does not change the shape of
the problem, only which addresses, and `place_bars` relocates them all into `PCI_BAR_PHYS`
(`0xc000_0000`, 2 MiB mapped), a constant checked once against QEMU's `info mtree`.

That is the number to read first on xenon's first boot: a machine whose RAM reaches above
`0xc000_0000` would have this kernel move most of its bus on top of memory, and the line says so
before anything is driven.

## Two decisions this block had left open, now made

- No MCFG means no PCI, with no fallback to the legacy `0xcf8`/`0xcfc` mechanism, which this
  kernel could reach (`enable_pcie_ecam` uses it) and which would enumerate bus 0 with no ACPI at
  all. Those ports see only the first 256 bytes of a function's configuration space, so a machine
  that fell back would enumerate a **different** set of capabilities than one that did not, every
  extended capability simply absent, and a driver that then failed would fail somewhere else
  entirely. This is milestone 215's own refusal one level up: when a machine wants MSI and meets a
  function without MSI-X, bring-up fails loudly rather than degrading into the original bug.
- **An MCFG whose first bus is not 0 is refused rather than adjusted.** `kernel/src/pci.rs`
  addresses a function as `base + (bus << 20 | ...)` with an absolute bus number, and subtracting
  `lo << 20` (the arithmetic that looks like the fix) names a base below the window
  `mmu::map_everything` maps, turning config reads into reads of whatever is underneath it. Every
  machine seen reports 0 and none is required to, so it is checked and said out loud.

## What is proven where

On patagonia, under QEMU: enumeration through an ACPI-sourced ECAM window on both boot paths,
including one where the MCFG's base (`0xe0000000`) differs from the constant, and including tables
read at a physical address a real machine would use. Real firmware leaving the ECAM decode already
enabled, so this kernel writes nothing. A count of how much of the bus this kernel relocates, under
both firmware and no firmware.

Only xenon can confirm: that `PCI_BAR_PHYS` is free on that machine, which is the census's whole
point; that its firmware presents an MCFG with first bus 0 and a bus count `PCIEXBAR` can encode;
and that its ACPI tables are below 4 GiB, which every machine's are and none promises.

One of milestone 215's three xenon-only questions is now half answered. That block owed "that a
real function's MSI-X table is reachable once *firmware* rather than this kernel has placed its
BARs". Under OVMF firmware does place them, and the census says where: outside this kernel's window,
so `place_bars` moves them and the MSI-X table is reached through the kernel's own address rather
than firmware's. What is still unproven is the same thing on silicon, and the reason it cannot be
proven here is the handoff below. When this was written the suite did not run under real firmware,
only the tour did. Milestone 195 (finish the UEFI boot path) now runs it under OVMF.

## What is owed

- **The 32-bit MMIO window should come from the machine, not from a constant.** Now milestone 256
  (x86_64 places PCI BARs in a hardcoded window), built 2026-09-04. `_CRS` is AML and stays
  refused, but AML is not the only source: the memory map firmware already hands over names the
  gaps, and Intel's host bridge reports `TOLUD` in its own configuration space. Its measure is
  `pci::bar_census`'s second number becoming zero, or the window being one this machine agreed to.
- **Run the x86_64 suite under real firmware, not only the tour.** Now milestone 195 (finish the
  UEFI boot path), built 2026-09-02. `cargo xtask uefi-image` staged the boot-tour kernel; the test
  kernel is a different binary and nothing staged it. Doing so would close two of milestone 215 (a PCI function's interrupt reaches nothing on x86_64)'s
  three xenon-only questions on patagonia rather than at a null modem, because OVMF places BARs and
  enables interrupt remapping the same way real firmware does.
- VT-d and PCI interrupt routing, which this block owed and no longer does: milestone 161's item
  6 and milestone 215 respectively.

## Follow-on

- **Milestone 195.** Run the x86_64 suite under real firmware, not only the boot tour. This block
  listed it as an unnumbered proposal on the day milestone 195 shipped it: `cargo xtask uefi-test`
  runs the kernel suite under OVMF with the same 192 passes and 68 skips as under PVH.
- **Milestone 161.** VT-d, which this block owed and could not touch. It is milestone 161's roadmap
  item 6, and it is what a confined userspace NVMe driver on x86 (DECISIONS §86) is gated on.
- **Milestone 215.** PCI interrupt routing, the other thing this block owed while it was PARTIAL.
- **Refused.** Falling back to the legacy `0xcf8`/`0xcfc` mechanism when a machine presents no MCFG.
  Those ports see only the first 256 bytes of a function's configuration space, so a fallback would
  enumerate a different set of capabilities than the ECAM path does, with every extended capability
  simply absent, and a driver that then failed would fail somewhere else entirely. No MCFG means no
  PCI, said out loud.
- **Refused.** Adjusting for an MCFG whose first bus is not 0. `kernel/src/pci.rs` addresses a
  function with an absolute bus number, and subtracting `lo << 20` names a base below the window
  `mmu::map_everything` maps, turning config reads into reads of whatever sits underneath it. Every
  machine seen reports 0 and none is required to, so it is checked and refused rather than fixed up.
  This refusal is milestone 463 (design/roadmap/0463-an-mcfg-whose-first-bus-is-not-zero.md), which
  carries it with the condition that would change it.
- **Recorded.** In `design/roadmap/0165-x86-64-pci-acpi-mcfg.md`: the `PCIEXBAR` writing arm is
  unexercised on both paths this kernel boots, since QEMU's PVH path already reports the window
  enabled and OVMF enables it too. It stays for the machine that genuinely arrives with the decode
  off, and nothing has run it.
- **Recorded.** In `design/roadmap/0165-x86-64-pci-acpi-mcfg.md`: three things only xenon can
  confirm, which are that `PCI_BAR_PHYS` is free on that machine, that its firmware presents an MCFG
  with first bus 0 and a bus count `PCIEXBAR` can encode, and that its ACPI tables sit below 4 GiB.
  `pci::bar_census` prints the first of them on the boot line, so it is the number to read first.
- **Milestone 256.** Take the 32-bit MMIO window that BARs are placed in from the machine rather
  than from the hardcoded `arch::x86_64::mmu::PCI_BAR_PHYS`. `_CRS` is AML and stays refused, but the
  firmware memory map already names the gaps and Intel's host bridge reports `TOLUD` in its own
  configuration space. The measure is `pci::bar_census`'s second number reaching zero, or a window
  the machine agreed to. Recording it is not enough: a machine whose RAM reaches above `0xc000_0000`
  would have this kernel relocate most of its bus on top of memory.

  This stopped being a prediction on 2026-09-04. xenon's second boot panicked at
  `AlreadyMapped mapping pci bar window 0xc0000000..0xc0200000: 0xc0000000 is also claimed by ram
  above the image 0xb9fbf000..0xc8940000`, with the census one line above reading **13 of 15
  functions outside the window**. The sentence above about reading that number first was written two
  days earlier and is why the boot needed no second attempt to diagnose.

## Index row

Scoped as a parallel check on whether x86_64 reaches a real-hardware data point for DECISIONS §86
faster than a from-scratch JH7110 PCIe driver would. Wires `memory::pci_regions()` from ACPI's
MCFG instead of a device-tree node, and finds a real complication along the way: PVH boots run no
firmware, so nothing has enabled the chipset's ECAM decode, which this milestone does itself
(measured on QEMU's monitor: the address faults until the host bridge's PCIEXBAR register is
programmed). Proven under `q35`: real PCI enumeration finds the machine's own host bridge over the
ACPI-sourced window. Does not reach §86's confinement half: x86 had no IOMMU driver when this
was raised, so a real NVMe-behind-IOMMU data point was gated on VT-d, not on anything here. VT-d
landed 2026-08-25 as milestone 161's item 6 (the x86_64 kernel port). Closed 2026-09-02, and the
thing that kept it open was not the wiring: the ACPI
walk bounded itself at 1 GiB while `boot.s`'s direct map covers 4, and both runners passed `-m
256M`, so firmware's tables fit and every gate passed. At 2 GiB under OVMF the same kernel found
no ACPI at all, so no APIC, no timer, no PCI and no VT-d, which was unconditional on any real
machine. The UEFI gate now boots where firmware's tables are high, the PCIEXBAR write reads before
it writes so it cannot widen a window firmware sized, and `pci::bar_census` prints how much of the
bus this kernel relocates into its hardcoded BAR window (5 of 8 under PVH, 3 of 6 under OVMF).
