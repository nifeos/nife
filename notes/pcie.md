# PCIe, and driving a disk over it

The PCIe transport (DECISIONS §18): how the kernel finds a device on a PCI bus, brings it up, and
runs the same userspace virtio driver over it that runs over virtio-mmio. Scoped in
notes/pcie-transport-scope.md; this note is the "what actually happened and what the words mean"
companion after the build.

## The shape of PCI, in one screen

A PCI function is addressed by **BDF**: bus (8 bits), device (5), function (3). Every function
owns 4 KB of configuration space, and ECAM (Enhanced Configuration Access Mechanism) is
the modern way to reach it: one flat memory window, the function's page at
`base + (bus << 20 | dev << 15 | fn << 12)`. No magic I/O ports, no indirection registers; config
space is just memory-mapped bytes, which is why an empty slot "reads all-ones" (nobody drives the
bus, the read floats high) and why enumeration is a loop, not a protocol.

The first 64 bytes of config space are standardized. They hold the vendor/device id (how you
recognize what it is), the command/status registers, and six BARs (Base Address Registers). A BAR answers "where
do this function's actual registers live in memory?" and it is writable. Firmware assigns each
function an address by writing one. Sizing is the famous dance: write all-ones, read back which
bits stuck (the low bits that stay zero encode the size and alignment), restore. Past the header,
optional features hang off the capability list, a linked list in config space. Virtio-modern puts
everything it needs there as vendor capabilities: which BAR (and offset) holds the
common-config block, the notify doorbell, the ISR byte, the device config.

Two command-register bits matter here. Memory-Space Enable makes the BARs decode at all.
**Bus-Master Enable is DMA permission at the bus level**: a device without it cannot issue a
single memory transaction. The kernel grants it last, after the confined transport is registered,
because it is the bus-level twin of the authority the confinement layer polices.

## What the kernel is, on this bus

With `-bios default`, OpenSBI does no PCI setup: every BAR arrives zero. So the kernel is the
firmware here: it sizes each BAR and places it in the board's 32-bit PCI memory window,
bump-allocated and size-aligned. On a UEFI machine the firmware would have done this and the
kernel would only read; both paths go through the same code, because `read_bars` reports
assigned bases and the kernel places only the zeros.

The window itself, and the ECAM config window, come from the device tree, not from constants:
`memory::init` reads the `pci-host-ecam-generic` node's `reg` and `ranges` (the 32-bit
non-prefetchable entry, parsed by the pci crate's `mem32_window`) and `memory::pci_regions`
answers everyone else. They were QEMU constants (`PCI_ECAM_BASE`, `PCI_BAR_BASE`) until the
first VisionFive 2 boot, where 0x4000_0000 is DRAM base and mapping it collided with the direct
map (notes/visionfive2.md). A machine whose tree has no such node gets no window mapped and
every PCI probe answers "nobody home", the same degradation as an absent virtio-mmio device.

Division of labor, same as the mmio side. The pci crate is pure decode logic (ECAM math,
enumeration, BAR sizing, capability parsing, the INTx swizzle), host-tested against a fake config
space. kernel/src/pci.rs supplies the volatile accessors and the policy (which device, where BARs
go, which bits to set). The driver stays in userspace, unchanged.

## The transport seam

virtio is one device model over multiple buses. The queue machinery (descriptor table, available
and used rings, DMA against physical addresses) is bus-independent; only "where are the
registers and what do they look like" differs. `virtio::Transport` is that difference, contained.
The mmio variant passes the vocabulary through. The pci variant translates each register name to
the virtio-pci common-config layout, the ISR byte (whose *read* is the ack, deasserting INTx),
and the notify doorbell (`notify_base + queue_notify_off * multiplier`, resolvable only with the
queue selected). Registers pci has no equivalent for (magic, version, device id) are synthesized,
so the driver's sanity checks mean the same thing on both buses.

Everything above the seam is one copy: the shadow ring, the descriptor validator, the queue
layout contract, the userspace driver binary. The DMA confinement was written once and now
polices two buses, which is the demonstrator's argument in miniature.

## INTx

The legacy PCI interrupt is four shared wires (INTA..INTD) routed up through the bridge with a
standard rotation ("swizzle"). Device `d` pin `p` lands on line `(d + p - 1) % 4`, then the board
maps the four lines onto interrupt controller inputs (32..35 on riscv `virt`). It is
level-triggered: the line stays asserted until the ISR byte is read. That plugs directly into the
kernel's existing model. The PLIC delivery masks the source, the driver's WAIT wakes, its
INTERRUPT_STATUS read (the ISR, via the transport) deasserts the line, and its Irq-capability ACK
re-enables the source. MSI-X (the device writes a message to raise an interrupt, many vectors,
no sharing) is the modern mechanism and a deliberate later step. Nothing we drive needs it.

The swizzle is a hardcoded formula with a **witness**: host tests parse the riscv fixture's
device tree and hold `intx_irq` against all sixteen of the machine's own `interrupt-map` entries
(crates/pci/tests/qemu_virt_dtb.rs), the UART pattern. The ECAM base was a constant held the same
way; it is now parsed from the tree (above), and the same test pins the parse to the old value.

## What is proven, and where the edges are

The riscv suite's `a_userspace_driver_reads_a_file_over_the_pcie_transport` runs the whole line.
ECAM enumeration finds 00:01.0, and the kernel places its BARs and sets up queue 0 through
common-config. The driver (byte-identical to the mmio one) submits a request past the shadow-ring
validator, the doorbell rings, and the completion arrives as INTx through the PLIC.

Both boards run it: riscv (INTx via the PLIC) and aarch64 (INTx via the GIC, SPIs 3..6, and the
**highmem** ECAM at 0x40_1000_0000). The machine names the node `pcie@10000000` after the low MMIO
base. Trusting the name instead of the `reg` is a mistake the witness test now guards. The per-arch
cost of the second board was constants plus two map entries, the portability claim in
concrete form.

Edges, honestly: INTx only, no MSI-X; the modern function only (`disable-legacy=on` in the runner,
because QEMU's default virtio-blk-pci is transitional and we do not drive the legacy layout);
both boards keep their working mmio paths alongside. The DMA
confinement is unchanged in spirit and in code; what PCIe adds to the trust story is Bus-Master
Enable, the bus-level DMA switch the kernel now controls explicitly.

## Placing BARs, and routing interrupts

This is the design behind `kernel/src/pci.rs`.

The windows come from the machine. Both `virt` boards fill `memory::pci_regions()` from the
`pci-host-ecam-generic` node. `x86_64` has no device tree, so it fills the same static from ACPI's
MCFG (`arch::x86_64::machine::enable_pcie_ecam`, called once from `main.rs`). One cursor places
every BAR the kernel assigns, the virtio devices' and on riscv the IOMMU's, because two bump
allocators would hand out one address twice. Its limit is the window base plus `PCI_BAR_MAPPED`,
the clamp `map_everything` maps.

`place_bars` has three arms. A BAR already inside the window the kernel mapped is left alone. A
nonzero BAR outside it is adopted where it stands. Anything else is placed from the cursor. A
nonzero BAR is not evidence of a mapped one. Under `x86_64`'s PVH boot nothing runs firmware, yet
QEMU's `-device nvme` resets with a live BAR0 (`0xfebd4000`) outside the kernel's window, and
trusting it faulted on first touch. A UEFI machine's firmware picks its own addresses too.

Adoption honors firmware's placement because firmware read the host bridge's `_CRS`, which this
kernel cannot: `_CRS` is AML. Relocating a real machine's bus is the wrong answer. On xenon 13 of
15 functions carried a BAR outside the kernel's 2 MiB window, which would have been exhausted by
the first function that wanted a megabyte. On the device-tree boards every BAR arrives zero and
adoption is unreachable.

`adopt` asks one safety question, the weakest that suffices: does the BAR overlap usable RAM? A
device mapped over RAM answers at addresses the frame allocator hands out. Whether the address also
decodes to the bus is the machine's claim, made by writing the BAR. It also refuses a BAR with a
page that cannot be mapped, since a half-mapped BAR faults on first touch. `AlreadyMapped` is
success there and nowhere else in the tree. Two BARs of one function share a page, or a BAR sits
inside the window mapped up front; either way it is one page mapped device-typed twice with the
same flags.

`bar_census` counts functions and stranded ones. A function is stranded when a
memory BAR is nonzero, outside the kernel's window and on top of RAM, which is what stops
adoption. Zero is the passing answer. It reads raw BAR registers, because sizing writes all-ones
and a census must write nothing. Unsized, it checks a BAR's first page only, and `adopt` refuses a
BAR that runs into RAM on a span it has sized. Counting functions is not bringing them up either:
`count_block_devices` and the NVMe count read config space only, since bring-up would disturb
whichever driver owns the function.

How a function interrupts is the machine's answer. `arch::irq::alloc_msi_vector` is asked first. On `x86_64`
it reserves a local APIC vector and the kernel programs the function's MSI-X table. A legacy pin
on `q35` goes through a PIRQ router only ACPI's `_PRT` describes, and `_PRT` is AML. A hardcoded
swizzle that matches `q35` may not match a real board. MSI-X has no board-specific routing: the
device is handed an address and a value, and only the interrupt controller's own message format
must be right. Both `virt` boards answer `None` and use the INTx swizzle onto the PLIC (sources
32..35) on riscv and GIC SPIs (INTIDs 35..38) on aarch64; host-run witnesses hold it against each
machine's tree.

A machine that answered `Some` has no INTx route, so a function with no MSI-X is refused rather
than swizzled. There `PCI_IRQ_BASE` is 0, and the swizzle would resolve to the PIT's line and arm a
driver on the timer. A function with MSI and no MSI-X, such as the xHCI on an Intel PCH, takes
plain MSI when its caller allows it. Virtio functions do not, because a virtio queue names its
vector by MSI-X table index. That vector travels with the transport: the queue is told it at
setup time with the queue selected, in `virtio::setup_queue`, and a device reset clears it.

The kernel programs entry 0 only, since every device attached uses a single queue's completion.
The entry is written before MSI-X Enable, with its per-entry mask cleared last, so nothing is live
half-written. Function Mask is cleared explicitly: some parts set it at reset, and a masked function
never interrupts. The table lives in a BAR, so it is programmed after `place_bars`.

A virtio-gpu is PCIe only, because no configuration attached puts one
on either `virt` machine's mmio bus. The keyboard rides PCIe by choice, though both machines offer
`virtio-keyboard-device` over mmio. It sits behind the same IOMMU domain as the GPU, and its
buffers are where every keystroke lands. The RNG exists on both buses, and an RNG's buffer is the
one place in memory whose contents must not be guessable. On `q35`, which has no virtio-mmio bus,
`find_block_device_n` answers "the second disk".

## More than one bus (milestone 320)

The kernel enumerates every bus behind a bridge, not bus 0 alone. QEMU's `virt` is flat, but on
xenon, a Dell OptiPlex 7050, the M.2 NVMe sits behind a PCIe root port. Enumerating bus 0 alone,
`find_nvme_device` returns `None` and the confinement test skips with QEMU's explanation for an
absence (`NIFE_NVME not set on this leg?`), which has a different cause entirely, and nothing fails
(`bench/xenon-2026-09-17/nvme-attempt-2-no-controller-found.log`). A limitation recorded honestly
still hides when the machine that violates it is the first real one you meet.

**Where the topology is written down is the bridge**, not ACPI and not the device tree. A PCIe root
port is a PCI function with a type-1 header, and bytes 0x18..0x1a of that header are the primary,
secondary and subordinate bus numbers whatever enumerated the machine wrote there. `pci::walk` reads
them and descends breadth-first over a 256-bit visited set, so a cycle in firmware's bus numbering
terminates rather than recursing, and each bus is scanned once.

**Why not simply map and scan all 128 buses the MCFG describes**, which is less work and needs no
new code at all? Measured on `q35` under QEMU with 256 MiB of RAM, it is also not very expensive:

| ECAM mapped | page tables |
|---|---|
| 1 bus (1 MiB), the old behavior | 560 KiB |
| 2 buses, the root-port topology | 560 KiB |
| 128 buses (128 MiB), what xenon's MCFG describes | 812 KiB |

252 KiB against a boot that spends 32,840 KiB on page tables is 0.8%, so the cost argument against
the flat scan is weak and should not be the one anyone repeats. The reason to walk the bridges is
the other one. A flat scan over a range ACPI happens to describe is the same species of assumption
that produced this bug. It issues configuration reads to bus numbers no bridge on the machine
decodes, and it stops being right on the first machine that numbers its buses differently.

How the kernel knows how much to map before it has read anything. `pci::survey` runs from
`kernel_main` before `arch::mmu::init`, which is the only window in the boot where the answer is
free: the x86 boot tables still cover the low 4 GiB indiscriminately, so every bus the MCFG
describes is already readable. The survey walks, prints every function it found, and records the
bus count; `map_everything` a hundred lines later maps exactly that. No lazy mapping, no second
pass, no arch-specific special case in the walk itself.

### BUGS

- The mapped range is contiguous, not the set of buses found. `pci::survey` records
  `highest_bus + 1` and the direct map covers bus 0 through that, empty buses included. A machine
  whose firmware numbers a root port's subtree 0x60 pays sixty-one buses of page tables to reach two
  buses of devices. The cost is bounded by what mapping the whole MCFG window would have cost, which
  the table above prices, so this is a waste rather than a hazard. Nothing measured does it yet.

- A bridge's memory window is neither read nor written. A PCI-to-PCI bridge forwards a memory
  transaction only inside the window its own base/limit registers describe, and this kernel does not
  touch them. Where firmware programmed them (every real machine so far) adoption keeps every BAR
  inside a window that already works. Where it did not, a device behind the bridge enumerates and
  does not work: under `NIFE_PCIE_ROOT_PORT=1` on QEMU, which boots PVH with no firmware, the
  controller on bus 1 answers configuration reads and its BAR does not decode. That is why
  `kernel::pci::tests::a_controller_behind_a_bridge_is_found_on_the_bus_behind_it` asserts
  enumeration and not a working disk. See
  `design/roadmap/proposals/a-bridge-window-the-kernel-programs-itself.md`.

- **Only `x86_64` surveys.** Both `virt` boards keep `PCI_ECAM_BUSES = 1`, which is the truth on a
  machine whose device tree describes a flat root complex and where QEMU puts nothing behind a
  bridge. The walk itself is the same code on all three and would follow a bridge the moment one
  appeared, except that it would refuse to descend past the one mapped bus and would say so. The
  survey has no home on those two architectures because there is no equivalent of the x86 boot map.
  Their MMU is off until `mmu::init`, so there is no moment before the fine map where an arbitrary
  bus is readable through `phys_to_virt`. If a device-tree board ever grows a root port, that is the
  problem to solve. Lazy per-bus mapping (`pci::adopt` already maps a page at enumeration time,
  so the machinery exists) is the likely shape.

- The topology is read once, at boot. Hot plug would change it and nothing re-surveys.
