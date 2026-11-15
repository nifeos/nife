# Milestone 161 appendix: early boot, steps 1 to 10

An appendix to [milestone 161 (the x86_64 kernel port)](../0161-x86-64-kernel-port.md), moved out
of that page on 2026-10-12 (UTC) under [§212 (a prose budget)](../../decisions/0212-a-prose-budget-for-every-document.md).
The main page is complete without it; this is the per-step record of what was built and how each
step was verified. Steps 1 to 8 landed on 2026-08-23, steps 9 and 10 on 2026-08-24. They are ordered
as they were built, because each step is what made the next one debuggable. Long sentences were
split to meet [§213 (writing standards)](../../decisions/0213-writing-standards.md); no fact changed.

## 1. The boot path

`kernel/link-x86_64.ld` and `kernel/src/arch/x86_64/boot.s`: a 32-bit trampoline into long mode and
the high half. The boot protocol is the surprise. It is written up at length in both files and in
`notes/x86-port/boot.md`. Multiboot 1 cannot boot a 64-bit kernel: QEMU refuses the image rather
than ignoring the header. QEMU 11 has no Multiboot 2. So the image carries a PVH ELF note instead.
PVH also hands over the ACPI RSDP address. That is the nearest thing x86 has to the single
device-tree pointer the other two architectures pass.

## 2. The console

The existing NS16550 driver, unchanged in substance. x86 puts the same part at an I/O port rather
than a memory address. So `drivers/ns16550.rs` gained a `RegisterSpace` type parameter, defaulting
to MMIO so nothing else changed, and `arch/x86_64/port.rs` supplies the `in`/`out` half. One driver,
two address spaces, which is also what the real machine of
milestone 87 (the x86_64 bare-metal machine) will want.

## 3. The trap path

`segments.rs` (GDT + TSS, with the double fault on its own IST) and `exceptions.rs` + `trap.s` (an
IDT, 256 generated stubs, one trap frame). Proven by taking an `int3` and returning from it. Before
this line a fault is a triple fault and a silent machine reset.

## 4. The page-table format

Behind the existing seam of milestone 20 (a portable HAL, proven on a second architecture): `crates/page_tables/src/x86_64.rs`, sixty lines of entry codec plus
seven host tests and six Kani harnesses, with no change to the generic walk. That is the
milestone-20 claim tested and holding.

## 5. The boot handoff, parsed

`crates/machine_discovery/src/x86_64.rs` decodes `hvm_start_info`, host-tested (seven tests), beside
the two ISA records already in that crate. It is a parser, so it lives where a parser can be proved
in milliseconds rather than in an emulator.

The tour prints the memory map, which proves the handoff carries real data: nine regions, 255 MiB
usable, and the PCIe ECAM window reported as reserved. It is reserved at exactly the address
`arch::mmu::PCI_ECAM_PHYS` hardcodes. QEMU's PVH loader leaves the ACPI RSDP field zero. So the root
pointer has to be found by scanning the BIOS area, which is a prerequisite for the APIC step rather
than a detail.

## 6. ACPI, which is what x86 has instead of a device tree

`crates/machine_discovery/src/acpi.rs` decodes the RSDP, the root-table walk, the SDT header, the
MADT and the MCFG, host-tested (thirteen tests). It sits beside the arch records rather than inside
the x86_64 one, because ACPI is not an x86 standard. The kernel side scans for the RSDP, since QEMU's
PVH loader leaves the field zero. Every table's checksum is verified before it is believed.

Verified against real firmware on q35:

- five tables found;
- the local APIC at `0xfee00000`;
- one enabled CPU;
- the 8259s reported present;
- the IO APIC at `0xfec00000`;
- the ECAM window at `0xb0000000`.

That ECAM address is exactly what `arch::mmu::PCI_ECAM_PHYS` hardcodes, and what the PVH memory map
independently reports as reserved.

## 7. The local APIC and a calibrated periodic timer

`irq.rs` masks the 8259s. That is an obligation the MADT states, and their power-on vectors overlap
the CPU's own exception vectors. It then enables the local APIC at the address ACPI gave and owns
the timer's LVT. `timer.rs` measures both the TSC and the APIC timer against the PIT in one
ten-millisecond window. x86 has four clocks and no architected way to ask any of them its rate.

Verified: 20 ticks in ~0.2 s at 100 Hz, 20 routed, 0 spurious. That proves the whole interrupt path
rather than only the clock. A missed EOI is a hang on this architecture, so exactly one tick would
have been the failure to expect.

## 8. The frame allocator

`memory::init` was split into a device-tree front end and `memory::bring_up_page_frames`. The latter
takes a RAM slice and a forbidden slice and does not care where they came from. The x86 front end
builds those from the PVH map. Verified: 254 MiB total, 254 MiB free, first frame 0x16f000.

The whole first megabyte is clipped rather than reserved. It holds the IVT, the BDA, the EBDA, the
boot info, and the page a STARTUP IPI's vector can name. The kernel image is the one entry on the
forbidden list. It covers the boot page tables, because the linker script puts `.boot_scratch`
inside the image bounds.

## 9. Fine-grained page tables, on a direct map with a base of its own

2026-08-24, the open list's item 1. `mmu::init` builds a W^X four-level map and switches `CR3` to
it. `.text` is executable and read-only, and `.rodata` is neither writable nor executable.
Everything else is non-executable, every guard page is a hole, and device registers are
device-typed. There is no identity map. The direct map now covers all of physical memory rather
than the low gigabyte.

The address-space decision item 1 recorded was taken as recorded: two bases, Linux's. The image sits
at `KERNEL_VA_BASE`, where the code model pins it. The direct map sits at `0xffff888000000000`
(PML4[273]). `virt_to_phys` inverts both, which is Linux's `__pa()` shape. `crates/page_tables` again
needed nothing.

The sequencing hazard was removed rather than sequenced around, and that is the part worth carrying
forward. Item 1 prescribed carrying the old alias across the `CR3` switch and dropping it afterwards.
The reason was that `phys_to_virt` changes meaning at that instant, and the frame bitmap is a stored
`&'static mut [u8]` derived through it. Instead `boot.s` installs PML4[273] itself, pointing at the
same low PDPT the identity map already used. That is eight bytes and no memory, and `phys_to_virt`
means one thing for the machine's whole life. `mmu::device_va` existed only because the direct map
could not reach a device. That foot gun was deleted outright rather than migrated.

Verified on q35 with `-m 256M`: the tour keeps printing across the `mov cr3`, and the timer keeps
ticking. The map costs 556 KiB of page tables for 254 MiB of RAM. That 0.2% is the price of 4 KiB
leaves and is recorded in `arch/x86_64/mmu.rs`'s `BUGS`, with two other honest limits:

- `mmu::init` must draw its tables from below 4 GiB, which is all the boot map reaches;
- `CR4.PGE` is still off, so the `G` bit the kernel's `Flags` set is ignored and every `mov cr3`
  flushes everything.

## 10. The IO APIC, and a real device line routed through it

2026-08-24, the open list's item 2. `irq.rs` gained the redirection table: the `IOREGSEL`/`IOWIN`
index-and-data register pair, an entry count read from the version register, and every entry
masked on the way in. `route_gsi`, `mask_gsi` and `enable` each write one entry. The arch contract's
`enable(intid)` now means what it means on the other two architectures.
`exceptions::enable_external` stopped being an `unimplemented!()` and became a documented no-op.
x86 has one interrupt gate (`RFLAGS.IF`) where RISC-V has two.

The trap the item flagged is real, and the fix is in the crate rather than the kernel. A legacy IRQ
number is not an IO APIC input. q35's MADT rewires ISA IRQ 0 to GSI 2: the PIT is on pin 2, and pin
0 carries the 8259 cascade. It also makes IRQs 5, 9, 10 and 11 level-triggered. Resolving that is
pure logic over table bytes, so it is `machine_discovery::acpi::isa_irq_table`, with six host tests.
One of them is the IRQ-0 case by name.

The flags word is two two-bit fields, not two bits, and a test exists for exactly that misreading.
`0x000d` is active high, level. A decoder testing bit 1 for polarity would answer active low and arm
a line that never fires.

Verified on q35: `pit irq 0 -> gsi 2 on vector 0x32: 20 interrupts in ~0.2 s at 100 Hz`. The local
APIC timer was masked for that window, so the count is the PIT's alone. Twenty is the same number
the local APIC timer produces in the same window against the same TSC. That is what makes it a
measurement rather than a nonzero.

The vector map is flat (`GSI_VECTOR_BASE + gsi`, base 0x30), so a stray vector in a fault report
names its line by subtraction. It costs the ability to express a priority policy, and there is not
one to express.

## The build wiring, and the evidence the split is real

Plus the build wiring it all needs: `kernel/build.rs`, `.cargo/config.toml` (including the static
relocation model, which this target does not default to), `rust-toolchain.toml`,
`helpers/qemu-runner-x86_64.sh`, and an x86_64 pass in `script/lint`.

The evidence that the `arch/` split is real: making the entire kernel compile for a third
architecture took 42 compiler errors. Every one was a missing `arch::` name, plus four `cfg` arms in
files that already had two. `crates/page_tables` needed nothing.
