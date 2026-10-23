# Machine discovery: the shared readers

`crates/machine_discovery` reads what the machine is rather than what the target triple implies.
Its per-architecture records, the tiers of discovery and why there is no trait are in
[isa-discovery.md](isa-discovery.md). This note covers the rest: the aarch64 decoder's traps, and
the readers every architecture shares or that x86 alone needs. Each module's limits are in the
`BUGS` section of its own doc comment. The note name is provisional.

## aarch64

The aarch64 record decodes `MIDR_EL1`, `ID_AA64MMFR0_EL1`, `ID_AA64MMFR2_EL1` and
`ID_AA64ISAR0_EL1`, read by the kernel and passed in as raw words. It is much shorter than the
RISC-V half, and that asymmetry is the finding: the part describes itself, so there is nothing to
parse and nothing to disbelieve.

The three granule fields need care. `TGran4` and `TGran64` spell "supported" as `0b0000` and "not
supported" as `0b1111`. `TGran16` spells them `0b0001` and `0b0000`, so a decoder that treats the
three alike reports 16 KiB backwards on every part. `TGran4` also has a second yes, `0b0001`, which
Arm defines as support for 52-bit addresses with `FEAT_LPA2`. It is a stronger yes, and reading it
as reserved refuses the boot on QEMU's `-cpu max` and on any part with `FEAT_LPA2`. `TGran64` has no
LPA2 value at all. Tidying the three decode lines into one would bring one of these bugs back.

`Psci` is the one parser on this side. Whether a firmware call leaves at `hvc` or `smc` depends on
what runs above EL1, which is a property of the boot arrangement rather than of the part. A tree
with no `/psci` node is a legitimate machine (spin-table bring-up, or a uniprocessor) on which
starting a second core is impossible. A node the decoder cannot make sense of is a different
answer, and a caller wants to say different things about the two.

## The core list

Both architectures describe their cores the same way, so `cpu_list` sits beside the records rather
than in either: a `cpu@` node per core under `/cpus`, with the hardware id in `reg`. The id is an
`MPIDR_EL1` affinity value on aarch64 and a hart id on RISC-V. Neither this module nor its caller
needs to know which, because the only thing done with it is handing it back to the firmware call
that starts the core.

The `#address-cells` of `/cpus` is the whole decode. One cell is the usual case and the only one
QEMU produces. Two is not exotic: the Arm CPU binding requires two the moment any core's
`MPIDR_EL1.Aff3` is nonzero. A decoder that assumed one cell would read a 64-bit id as its high
half and start nothing.

`Cpu::is_startable` is the kernel's decision, kept in this crate so the board fixtures can hold it.
The VisionFive 2 tests assert which harts it admits (the four U74s) and which it refuses (the S7,
through either tree's lie). An unstated `enable-method` counts as startable on purpose: the RISC-V
CPU binding has no such property, so demanding it would refuse every RISC-V machine. `supervisor`
is required because `status` can lie where a hart's own `riscv,isa` does not
([visionfive2.md](visionfive2.md), the second stop). One decoder reads `status` for both the core
list and the RISC-V record, so the two walks cannot disagree about what "disabled" means. The
specification defines `"okay"`, `"disabled"`, `"reserved"`, `"fail"` and `"fail-sss"`, and only the
first means operational; `"ok"` is a deprecated spelling older firmware still emits.

## Interrupt ids

A device's interrupt line is a fact the device tree states, not a constant. QEMU `virt`'s NS16550
raises PLIC line 10. On the JH7110, UART0 raises line 32, and a kernel that armed 10 there enabled
an unrelated source, so a key press reached nothing ([visionfive2.md](visionfive2.md)).

Decoding the line is not one read, because the property's shape belongs to another node. An
`interrupts` value is a list of entries, each `#interrupt-cells` wide, and the device's interrupt
parent declares that count. The parent is named by phandle in an `interrupt-parent` property the
specification makes inheritable. QEMU riscv64 writes it on the serial node, QEMU aarch64 once on
the root, and the mainline JH7110 dtsi on `/soc`. The entry shapes:

- One cell (a RISC-V PLIC): the source number, verbatim.
- Two cells, when the parent is a PLIC (`<number trigger>`, the T-Head TH1520's form): the first
  cell is the source. The parent's `compatible` must be a PLIC's, because two cells mean other
  things on other controllers.
- Three cells (an Arm GIC): `<type number flags>`. The number is bank-relative, and the INTID the
  GIC delivers adds the bank base: SPIs (type 0) start at 32 and PPIs (type 1) at 16. QEMU's PL011
  says `<0 1 4>`, which is SPI 1, INTID 33.

## PLIC contexts

A PLIC context is a pair of a hart and a privilege mode. On QEMU `virt` every hart contributes an
M context and an S context in hart order, so hart *h*'s S context is `2h + 1`. The JH7110 breaks
that formula quietly. Its hart 0 is an S7 monitor core with no MMU that contributes only an M
context, so from hart 1 on hart *h*'s S context is `2h`. A kernel using the formula there programs
its neighbor's context, and every external interrupt sits pending forever.

The layout is the order of the PLIC node's `interrupts-extended` entries. Entry *k* names the
`riscv,cpu-intc` that context *k* feeds and the interrupt it raises there: 11 for machine external,
9 for supervisor external. Two walks stitch the answer together, because the property speaks in
phandles and the kernel in hart ids. The `cpu@` nodes give the hart ids, and each one's child
`riscv,cpu-intc` gives the phandle. The two lists align by tree order, since each intc is nested in
its `cpu@` node. The PLIC itself is found by binding, never by label: QEMU names it
`plic@c000000` and the JH7110 `interrupt-controller@c000000`.

The binding strings differ by machine. QEMU `virt` lists `sifive,plic-1.0.0` and `riscv,plic0`.
The VisionFive 2's U-Boot control DTB lists only `riscv,plic0`. The TH1520 lists
`thead,th1520-plic` and `thead,c900-plic` (Linux's `th1520.dtsi`). Its PLIC has the standard
register layout and two quirks that are firmware's or a driver's, not this parser's. M-mode must
set bit 0 of the control word at `0x1f_fffc` before S-mode may touch the PLIC, which OpenSBI does
(`PLIC_FLAG_THEAD_DELEGATION`). And an edge source must be completed before it is handled, or the
next edge is lost (Linux's `PLIC_QUIRK_EDGE_INTERRUPT`).

## The GIC

Taking a GIC node's first two `reg` blocks as the distributor and the CPU interface is true of a
GICv2 and false of a GICv3, whose second block is the redistributor array. GICv2 CPU-interface
writes land in a redistributor frame and do nothing, and the kernel boots and takes no interrupts
with nothing faulting. So `gic` answers two questions, and the second makes the first safe. What
does the firmware claim, read by binding? And does the hardware's own revision agree? The GICv2
revision is read from the very block the tree calls the CPU interface.

QEMU 11.1.1 writes the two bindings this way (`dumpdtb`, both versions of `virt`):

| | `gic-version=2` | `gic-version=3` |
|---|---|---|
| `compatible` | `arm,cortex-a15-gic` | `arm,gic-v3` |
| `reg[0]` | GICD, `0x8000000 + 0x10000` | GICD, `0x8000000 + 0x10000` |
| `reg[1]` | GICC, `0x8010000 + 0x10000` | GICR, `0x80a0000 + 0xf60000` |
| MSI child | `v2m@8020000` | `its@8080000` under TCG, `v2m@8020000` under HVF |

The kernel asks this question only on aarch64. On RISC-V a node named `interrupt-controller@...`
is the PLIC, and each `cpu@` node has a `riscv,cpu-intc` child. How the kernel drives each version
is in [interrupts.md](interrupts.md). The binding allows several redistributor regions
(`#redistributor-regions`) for machines whose frames are not contiguous; QEMU `virt` needs a second
only past 123 cores, and the kernel's `cpu::MAX_CPUS` is eight, so the decoder refuses more than one.

## The screen token

`framebuffer` is arch-neutral, so it sits beside `cpu_list` rather than in `x86_64`: a U-Boot
framebuffer handoff on aarch64 says the same sentence. Why the description rides PVH's command
line instead of a new handoff field is in [serial-less-output.md](serial-less-output.md).

The token is `screen=` followed by the base in `0x` hex and the width, height and stride in
decimal, then the pixel order. It is written by hand rather than through `core::fmt`, because the
only writer is a `no_std` UEFI application with no allocator. `MAX_LEN` is 72 against a worst case
of 63. One byte of headroom would be a trap for whoever adds a field. A malformed token reads as
no screen, because a kernel that refused to boot over a display description it could do without
would be the wrong trade for a diagnostic path.

`PixelOrder` is named for the order of the bytes in memory, not for the `u32` value. The memory
order is what a writer has to get right, and the `u32` spelling of it flips between the two. UEFI
names its formats the same way.

`screen-hold` is a debugging word for the screen gate, not for any machine booted for its own sake.
Its wait has a bound, and that bound is a constant in the kernel rather than a field in the token,
so a reader of the kernel meets the number and its reason in one place.

## x86_64

The other two architectures learn where RAM is from the device tree, handed over as one pointer.
x86 has no device tree. It has a boot protocol, PVH (see `kernel/src/arch/x86_64/boot.s` for why
not multiboot), which hands over one pointer to a structure carrying the memory map, the ACPI root
pointer, the command line and any loaded modules. The shape is the device-tree handoff's and only
the format differs. The decoding lives in this crate because a parser proved only inside a booting
kernel is proved by nothing that runs in milliseconds
([x86-port/acpi-and-pci.md](x86-port/acpi-and-pci.md)).

The structure, from Xen's `start_info.h`:

```text
struct hvm_start_info {          offset  size
    uint32_t magic;                   0     4   0x336ec578, "xEn3" little-endian
    uint32_t version;                 4     4
    uint32_t flags;                   8     4
    uint32_t nr_modules;             12     4
    uint64_t modlist_paddr;          16     8
    uint64_t cmdline_paddr;          24     8
    uint64_t rsdp_paddr;             32     8   the ACPI root pointer
    /* version >= 1 only: */
    uint64_t memmap_paddr;           40     8
    uint32_t memmap_entries;         48     4
    uint32_t reserved;               52     4
};
```

Version 0 stops at offset 40, so the decoder reads the memory-map fields only when the version
says they are there and reports zero entries otherwise. A caller that finds no entries would have
to fall back to the legacy E820 call, which this kernel cannot make because it needs real mode. In
practice a version-0 handoff means the kernel cannot find its RAM, and the caller decides what to
say.

A module is x86's answer to `/chosen/linux,initrd-start`. On the other two architectures the loader
writes the archive's bounds into the device tree and `memory::init` reads them out. Here it writes
them into a module list.

The `CPUID` half looked as though it held nothing for a host test, since asking takes one
instruction and there is no format to parse. Reading the answer is trivial and deciding what it
means is not. `CPUID` cannot say "I do not implement that leaf" except by answering with another
leaf's bits, and the rule for telling those apart is the thing that can be wrong. A read above the
maximum leaf answers with some other leaf's data, and the standard and extended maxima are
different numbers in different spaces.

`Gate` has a third value because of the hypervisor case. Calling the invariant TSC optional would
file a load-bearing assumption next to `RDSEED`, an optional convenience. Calling it required
would refuse every boot under QEMU's TCG, which never reports it. So the row names the machine it
refuses on.

Silence means different things on the two sides. A device tree that does not mention an extension
is firmware being terse about a machine that is nonetheless running the kernel, so the RISC-V
check treats it as unknown. A part that does not answer leaf `0x80000001` is a part from before
these features existed, with nobody in between to be terse.

## ACPI

`acpi` sits beside the architecture records rather than inside `x86_64`, for the reason `cpu_list`
does: ACPI is not an x86 standard. Every aarch64 server that is not a device-tree board describes
itself with the same tables. Why the checksum is the whole defense, and what QEMU's tables hold, is
in [x86-port/acpi-and-pci.md](x86-port/acpi-and-pci.md).

A device tree is one blob with one root. ACPI is a linked structure of independent tables, each
with its own signature and checksum, reached from a root pointer that is not itself a table:

```text
  RSDP  ("RSD PTR ")            found by scanning low memory, or handed over by the loader
   |
   +-- RSDT (32-bit pointers)   ACPI 1.0
   +-- XSDT (64-bit pointers)   ACPI 2.0+, and what to prefer when both exist
        |
        +-- APIC ("MADT")       the local APICs, the IO APICs, and the interrupt rewiring
        +-- MCFG                where the PCIe ECAM window is
        +-- DMAR                where the IOMMU is, on an Intel machine (VT-d)
        +-- IVRS                where the IOMMU is, on an AMD machine (AMD-Vi)
        +-- FACP, GTDT, SPCR, HPET, WAET, ...
```

AML is not here. It is the DSDT's bytecode for everything ACPI has no fixed table for, including
PCI interrupt routing (`_PRT`) and every power-management method. AML needs an interpreter, which
is a project rather than a parser.

The GTDT is where an Arm machine states the architected timer's interrupts. x86 has no
counterpart; its timers are the HPET and the local APIC. On a device-tree machine the same facts
are the `arm,armv8-timer` node's `interrupts`, describing the same four timers in the same order.
A GSIV is an absolute INTID, not a bank-relative number: the virtual timer's 27 is the INTID the
GIC delivers, where the device tree spells it `<1 11 ...>`, PPI 11.

The IVRS is AMD's counterpart of the DMAR, and the driver it feeds is in [amd-vi.md](amd-vi.md).
Its shape:

```text
  SDT header ("IVRS")
  IVinfo (4 bytes), reserved (8)
  IVHD  type 10h, 11h or 40h     one IOMMU: its register file and the device ids it serves
    device entries               select, range, alias, "all", special (IOAPIC/HPET), ACPI HID
  IVMD  type 20h, 21h or 22h     memory a device keeps DMA-ing into: AMD's RMRR
  ... more IVHDs and IVMDs
```

One IOMMU is often described twice. Section 5.2.2 defines type 10h for backward compatibility and
11h and 40h for the full feature set, and firmware lists the same unit once per type it supports.
QEMU writes a 10h and an 11h for its one `amd-iommu`. Linux decodes only the highest type it
understands (`get_highest_supported_ivhd_type`, read at v6.12), and so does the parser; otherwise
every unit would be brought up twice over one register file. Where QEMU's table builder
(`build_amd_iommu` in `hw/i386/acpi-build.c`, read at QEMU 11.1.1) decides a choice, the code says
so, because QEMU is the only AMD-Vi this tree has run on.

Ownership is by device id, not by path. A DMAR names a device by a bus-relative path that only the
live bridges can resolve. An IVHD names it by its 16-bit `bus:dev.fn` id directly (section 5.2.2.1,
Figure 5), so nothing in the IVRS decoder needs the bus.

A DMAR's ANDD names an ACPI-namespace device, such as an I2C or serial controller with no PCI
identity, that can issue DMA. The IVRS's type F0h entries are the same thing on AMD. This kernel
drives no such device, so none is confined. A machine where one matters needs ANDD and the type-5
scope decoded on Intel, and F0h on AMD. Linux treats an exclusion-range IVMD as a read-write unity
map too ("treat per-device exclusion ranges as r/w unity-mapped regions", recalled rather than
re-read), and `amd_vi::set_up` clears the exclusion registers. A read-only IVMD is mapped
read-only. QEMU writes no IVMD, so that path has run only in the host tests.

The DMAR decoder skips IOAPIC, HPET and ACPI-namespace device scopes, and does not decode the ATSR,
RHSA, ANDD, SATC or SIDP structures. A truncated decode answers an ownership question with
"unknown" rather than a guess.


## How it is proved

The crate's Kani harnesses, and the defects three of them found, are recorded in milestone 319
(the crate that parses firmware had no proofs), with what was deliberately left unproved and why.
Each harness's doc comment states its claim and how it could plausibly have been false.
