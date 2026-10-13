# The x86_64 port: ACPI, discovery, and PCI interrupts

*An appendix to [`notes/x86-port.md`](../x86-port.md), which is the page to read. This file holds
what the loader hands over, the ACPI tables, the discovery seam, and how a PCI function's interrupt
reaches a driver. It exists to verify or challenge the main page. A reader who only needs to build,
boot or test the x86_64 port should not have to open it. The directory `notes/x86-port/` and this
file's stem are provisional names; naming is calef's.*

*Records cited below: milestone 20 (a portable HAL), milestone 87 (the x86_64 bare-metal machine),
milestone 165 (x86_64 PCI enumeration), milestone 195 (finish the UEFI boot path) and milestone 215
(a PCI function's interrupt).*

## What the loader hands over, and what it does not

`machine_discovery::x86_64` decodes `hvm_start_info`, host-tested, for the same reason `crates/device_tree_blob`
exists rather than a device-tree reader living in `arch/aarch64/`: a parser proved only inside a
booting kernel is proved by nothing that runs in milliseconds. The kernel side
(`arch/x86_64/machine.rs`) does nothing but turn a physical address into bytes through the direct
map.

What q35 actually produces, read back out of the guest on 2026-08-23 with `-m 256M`:

```
  memory      : 9 regions from the PVH handoff, rsdp 0x0
                0x000000000000..0x00000009fc00  ram
                0x00000009fc00..0x0000000a0000  reserved
                0x0000000f0000..0x000000100000  reserved
                0x000000100000..0x00000ffdf000  ram
                0x00000ffdf000..0x000010000000  reserved
                0x0000b0000000..0x0000c0000000  reserved
                0x0000fed1c000..0x0000fed20000  reserved
                0x0000fffc0000..0x000100000000  reserved
                0x00fd00000000..0x010000000000  reserved
                usable ram: 261627 KiB
```

Two things worth taking from that beyond the RAM.

`0xb0000000..0xc0000000` is the PCIe ECAM window, reported as reserved, which independently
confirms the constant `arch::mmu::PCI_ECAM_PHYS` currently hardcodes. That constant should
eventually come from ACPI's MCFG table; until then the memory map is a second witness for it, which
is better than the constant standing alone.

`rsdp 0x0`: QEMU's PVH loader does not fill the ACPI root pointer in. The field exists and is
zero, which means the RSDP has to be found the older way, by scanning for the `"RSD PTR "`
signature. That is what `arch::x86_64::machine::find_rsdp` does, and it finds one at `0xf52e0`, in
the third reserved region above.

## ACPI, which is where the rest of the machine is described

`machine_discovery::acpi` decodes the RSDP, the root table, the SDT header, the MADT and the MCFG,
host-tested with thirteen tests. It sits beside the arch records rather than inside the `x86_64`
module for the reason `cpu_list` does: ACPI is not an x86 standard, and milestone 20's own text
expects the machine after the VisionFive 2 to be a UEFI/ACPI one.

The checksum is the whole defense and it is worth being explicit about why. The RSDP is found by
scanning memory for an eight-byte string, so without the checksum any sixteen bytes that happen to
spell `RSD PTR ` would be believed and the kernel would follow a pointer into somebody's data. Every
table is checksummed on the way in, and one that fails is reported as absent rather than used: a
corrupt MADT would hand out an APIC address and there is nothing downstream that could notice it was
wrong.

What q35 actually has, read on 2026-08-23:

```
  acpi        : rsdp at 0xf52e0 (revision 0), root table 0xffe2344 (rsdt)
                0x000ffe213c  FACP (244 bytes)
                0x000ffe2230  APIC (120 bytes)
                0x000ffe22a8  HPET (56 bytes)
                0x000ffe22e0  MCFG (60 bytes)
                0x000ffe231c  WAET (40 bytes)
                local apic 0xfee00000, 1 cpu(s) enabled, 0 disabled, 8259s present (must be masked)
                io apic 0 at 0xfec00000, gsi base 0
                pcie ecam 0xb0000000, buses 0..=255 (mmu::PCI_ECAM_PHYS says 0xb0000000)
```

Three things to take from that.

The ECAM window the MCFG describes is exactly the constant `arch::mmu::PCI_ECAM_PHYS` hardcodes,
which is now confirmed twice over (the PVH memory map reports the same range as reserved). Milestone
165 wired the consumer: `memory::record_pci_regions` fills `memory::pci_regions()` from this table's
answer, the same static `kernel/src/pci.rs`'s probes already read on the other two architectures, so
`PCI_ECAM_PHYS` is now only the print-time reference value above rather than what gets mapped.

The window also has to be decoding, and `arch::x86_64::machine::enable_pcie_ecam` is where that is
settled, through the legacy `0xcf8`/`0xcfc` ports, which are the only way to bootstrap: nothing can
read the ECAM window to turn the ECAM window on. It reads the host bridge's `PCIEXBAR` register
before writing it, and writes only when the register is not already enabled at the base the MCFG
itself reports. Firmware may have sized the window at 128 or 64 MiB where an unconditional write puts
256, and that write then widens the chipset's decode over whatever physical addresses sit above it.
Some chipsets also lock the register once firmware has written it, so the value read back is the only
thing that would ever have said the write was dropped.

And the complication that function was written for does not reproduce, which is a correction rather
than a detail. Milestone 165's block records QEMU's monitor answering "Cannot access memory" at the
MCFG's base before the kernel ran, read as the decode being off under PVH. Re-measured 2026-09-02 on
QEMU 11.1.1 from inside the guest, which is the side being served: the register reads
`0xb0000001` before anything writes it, and every PCI test passes with `enable_pcie_ecam` writing
nothing at all. The monitor still answers "Cannot access memory" on that same boot, so the two
disagree. Nothing here needs to decide which changed, because the register is now asked rather than
assumed. What follows from it is that the writing arm is unexercised on both paths this kernel
boots, and stays for the machine that genuinely arrives with the decode off.

`8259s present (must be masked)` is the MADT's `PCAT_COMPAT` flag, and it is a real obligation
rather than trivia. The legacy PICs are still wired and still raise interrupts, so whoever brings the
APIC up has to mask both of them first or a spurious interrupt arrives through a controller nothing
is driving.

What is deliberately not decoded is AML, the bytecode in the DSDT that describes everything with
no fixed table, including PCI interrupt routing (`_PRT`). AML needs an interpreter, which is a
project rather than a parser. That is the reason `arch::mmu::PCI_IRQ_BASE` is zero and honest about
it: a PCI function's legacy interrupt goes through a router the `_PRT` describes, and MSI bypasses
the routing entirely by writing a vector straight to the local APIC.

The MSI path is the one that got built (milestone 215), and the sentence above predicted it
before there was a device on the bus to need it. See "How a PCI function's interrupt reaches a
driver" below.

## The discovery seam milestone 20 promised and did not build

Milestone 20's deliverable had two abstraction shapes in it. The first, "a generic level-walk plus a
per-arch entry codec", was built and holds: `crates/paging` needed no change for a third format.
The second, "put device discovery behind a 'here is the hardware' interface (device tree today,
ACPI/PCI later)", was not built, and this port is where that shows.

`kernel/src/memory.rs`'s `init` took a device-tree pointer and read `Dtb` directly for the RAM
regions, the reservations, the interrupt controller, the RTC, the UART's interrupt and the PCIe
window. Nothing about that is wrong on two architectures that both have a device tree. On x86 there
is no tree, so the frame allocator could not come up at all.

The narrow half is now split out (`memory::bring_up_page_frames`), because without it nothing below
the allocator can exist on this architecture and the port would have stopped there. `init` is now
explicitly a device-tree *front end*: it reads the tree, assembles a RAM slice and a forbidden slice,
and hands them to a function that does not care where they came from.
`arch::x86_64::machine::bring_up_memory` is the x86 front end doing the same job from the PVH map.

The wide half is still partly owed. What crosses the seam today is RAM, reservations, and (as of
milestone 165) the PCIe ECAM window. The interrupt controller and the console UART's interrupt still
bypass `memory.rs`'s statics or stay unwired. Three facts still have two sources that do not know
about each other, which is the shape this should not be left in:

| Fact | Device-tree machines | x86 |
|---|---|---|
| RAM, reservations | `memory::init` | `machine::bring_up_memory` (shared consumer) |
| Interrupt controller | `memory::init` -> `GIC_REGIONS`/`PLIC_REGION` | ACPI MADT -> `machine::Acpi` -> `irq::init_{local,io}_apic` directly, bypassing the statics |
| PCIe ECAM window | `memory::init` -> `PCI_REGIONS` | ACPI MCFG -> `machine::Acpi` -> `memory::record_pci_regions` -> `PCI_REGIONS` (shared consumer, milestone 165). The BAR/mem32 half of the same static has no ACPI or AML source and stays a hardcoded constant (`arch::mmu::PCI_BAR_PHYS`); see that milestone for why. |
| Console UART interrupt | `memory::init` -> `UART_IRQ` | discoverable now (`Acpi::isa_irqs[4]` is COM1's), unwired |

The type at the seam is another loose end worth naming. `Region` is `device_tree_blob::Region`, which is a plain
`{ start, size }` pair and means nothing device-tree-specific. But a machine with no device tree
naming a device-tree type is a smell rather than a design.

## How a PCI function's interrupt reaches a driver, and why it is not the legacy pin

Milestone 215, and it is the piece that turned this port's PCI bus from a thing the kernel can
enumerate into a thing a userspace driver can operate.

The failure it fixed was a wrong answer that looked like a right one. `pci::intx_irq(base, dev,
pin)` is `base + ((dev + pin - 1) % 4)`, and `arch::mmu::PCI_IRQ_BASE` was `0`, so the virtio-blk
function at device 4 pin 1 resolved to intid `0`, and `irq::enable(0)` put that through
`isa_routing` to the PIT's line. A confined block server was armed on the timer and waited
forever. Nothing anywhere said so: the wiring succeeded, the driver blocked, and the suite wedged.

The two candidate answers, and why one lost.

*Legacy INTx.* On `q35` a function's pin goes through the ICH9 LPC bridge's PIRQ router to an IO
APIC input, and what states the mapping is ACPI's `_PRT`, which is AML. Two versions were
available: read `_PRT` (an AML interpreter, which is a project and one this tree should not grow
for four numbers) or hardcode what QEMU does. The hardcode is the one that fails badly rather than
loudly. It would pass every gate on this machine and be wrong on the OptiPlex, and milestone 87
would discover it at a null modem. That is the most expensive place in this project to discover
anything.

*MSI-X.* The device is handed the address to write and the value to write there, so there is no
board-specific routing table to be wrong about. It is more code than the hardcode and much less
than the interpreter, every device this port would ever attach has it (virtio-pci, NVMe), and it is
the direction real x86 systems went twenty years ago. It won on the OptiPlex risk, not on effort;
it happens to also be less work than the honest version of the alternative.

The design that fell out, and it is the part worth stealing. An x86 intid was already two
things: a *vector* for a local APIC source (there is no controller input to name) and a *legacy IRQ
number* for an IO APIC line. An MSI is a local APIC source in the only sense that matters, because
the device writes the vector straight to the APIC. So an MSI intid is its vector, and three
things collapse:

- `irq::enable` has nothing to do for one, which is correct rather than a stub: the message is
  edge-delivered and already over. A driver's `Irq::ACK` is correspondingly a no-op.
- The trap handler can ask `sched::irq_route(vector)` directly. The vector-to-intid inversion
  an IO APIC line would need (and which `exceptions.rs` still records as owed for one) never
  arises, because there is no line in between to have named it.
- Nothing above `arch/` changed shape. `kernel/src/pci.rs` asks `arch::irq::alloc_msi_vector`,
  which answers `None` on both `virt` boards and leaves their INTx swizzle exactly as it was.

A refusal, not a fallback. If a machine answers `Some` and the function has no MSI-X, bring-up
fails and says so. Falling back to `intx_irq(0, ..)` there is the original bug, and it is the kind
that reads as a graceful degradation.

What only real hardware can confirm. One of the three, now (milestone 195): that the OptiPlex's
firmware leaves VT-d interrupt remapping off, since a unit with it on rejects the compatibility-format
message this builds. That is a setting in somebody else's firmware and no emulator can answer it.

The other two were answered on patagonia, by running the kernel suite under OVMF with the PVH
runner's devices attached (`cargo xtask uefi-test`). That real firmware enumerates the bus and places
every BAR before nife exists is what made them answerable at a desk at all:

- *A real device's MSI-X table is where its capability says it is once firmware rather than this
  kernel has placed the BARs.* OVMF placed them, `pci::bar_census` reports five of eight functions
  outside the window this kernel maps, so `place_bars` moved five, and both milestone 215 tests that
  reach a `virtio-blk-pci` function through its MSI-X table pass afterwards.
- *A machine with more than one local APIC still delivers to the boot core's id.* The tour boots at
  two cores under OVMF and its PIT interrupt arrives 20 times in 0.2 s.

Neither is silicon, and neither is a Dell. What they are is two fewer things an evening at the bench
has to establish before it can look at the third. See notes/x86-uefi-boot.md.
