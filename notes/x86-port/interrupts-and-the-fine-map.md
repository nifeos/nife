# The x86_64 port: the clock, the IO APIC, and the fine map

*An appendix to [`notes/x86-port.md`](../x86-port.md), which is the page to read. This file holds
the timer calibration, the IO APIC bring-up, and the W^X kernel page tables. It exists to verify or
challenge the main page. A reader who only needs to build, boot or test the x86_64 port should not
have to open it. The directory `notes/x86-port/` and this file's stem are provisional names; naming
is calef's.*

*Records cited below: milestone 161 (the x86_64 kernel port).*

## The clock, and why it needed a calibration loop

The other two architectures read their timer's rate out of a register or a device-tree property.
x86 has at least four clocks and no architected way to ask any of them, on the parts this has to run
on. `CPUID` leaf 0x15 gives the TSC's ratio to a crystal whose frequency leaf 0x16 may not report.
The local APIC timer counts a bus clock nothing reports at all.

So the rate is measured, against the one device on a PC whose frequency is a fixed number: the
8254 PIT at 1193182 Hz. That number has not changed since 1981 because it came from dividing the
NTSC color-burst crystal and every clone copied it. One ten-millisecond window, polled on channel 2
(the only channel whose gate is under software control and whose output can be read), with both the
TSC and the APIC timer sampled across it. So one wait produces both numbers and they cannot disagree
with each other.

Measured on QEMU TCG, 2026-08-23:

```
  apic        : local apic 0xfee00000 up, id 0, version 0x14, 8259s masked
  clocks      : tsc 1001 MHz, apic timer 62 MHz (both measured against the PIT)
  timer       : 20 ticks in ~0.2s at 100 Hz (20 routed, 0 spurious)
```

Twenty ticks in a fifth of a second at 100 Hz is the number that proves the whole interrupt path,
not just the clock. An interrupt the CPU did not ask for arrived, the IDT dispatched it by vector,
and the handler acknowledged it so a second could follow. A missed EOI is a hang rather than an
error on this architecture, so exactly one tick would have been the failure to expect. The 62 MHz
APIC timer is 1 GHz divided by 16, which is the divider `irq.rs` programs, so the two measurements
agree with each other as well as with the emulator's nominal rate.

Two obligations the ACPI tables state and this code honors, both of which would otherwise show up
much later as a hang:

- The 8259 PICs are masked before the local APIC is enabled. Their power-on vector base overlaps
  the CPU's own exception vectors, so an interrupt from one arrives as, for instance, the double
  fault. The order matters: the reverse leaves a window in which the IDT is live and they are not
  masked.
- The Task Priority Register is zeroed. Anything else silently drops interrupts below that
  priority class, which looks exactly like a controller that was never wired up.

## The IO APIC, and the number that is not the number

Milestone 161's roadmap item 2, built 2026-08-24. The local APIC's timer proves the CPU accepts an
interrupt it did not raise. A device line is a different claim, and the IO APIC is what makes
it: it takes physical interrupt inputs and turns each into a vector delivered to some local APIC.

### The register interface is two words, and that is the first surprise

The whole device is a 4 KiB page with two 32-bit registers in it. `IOREGSEL` at offset 0 takes the
*number* of the register you want; `IOWIN` at offset 0x10 is a window onto whatever `IOREGSEL` last
named. So every access is a pair, the device is stateful, and two CPUs doing this concurrently would
interleave and each read the other's register. Nothing here is concurrent yet (one CPU,
single-threaded boot), and `irq.rs` says out loud that a lock belongs there the day SMP lands.

What is behind the window:

| Register | What it is |
|---|---|
| `0x00` | this IO APIC's id, bits 27:24 |
| `0x01` | version in bits 7:0, and the entry count minus one in bits 23:16 |
| `0x10 + 2n` | redirection entry `n`, low word: vector, delivery mode, polarity (bit 13), trigger (bit 15), mask (bit 16) |
| `0x11 + 2n` | redirection entry `n`, high word: the destination local APIC id in bits 63:56 |

The "minus one" in the version register is the field's definition rather than an off-by-one to
correct for: a 24-entry part reports 23. And the two words are written high first, so the
destination is in place before the low word's mask bit clears and the line goes live.

### A legacy IRQ number is not a pin number, and this is the whole trap

The PIT's IRQ 0 does not arrive on IO APIC input 0. On q35, and on essentially every PC:

```
                io apic 0 at 0xfec00000, gsi base 0
                isa irq 0 -> gsi 2 (active high, edge)
                isa irq 5 -> gsi 5 (active high, level)
                isa irq 9 -> gsi 9 (active high, level)
                isa irq 10 -> gsi 10 (active high, level)
                isa irq 11 -> gsi 11 (active high, level)
```

The PIT is wired to pin 2 because pin 0 carries the 8259 cascade. A kernel that armed redirection
entry 0 for "the timer" would have armed a line nothing drives, and the failure is the quiet kind:
no interrupts, no error, nothing anywhere to say why.

Resolving that is pure logic over table bytes, so it is in the crate rather than in the kernel:
`machine_discovery::acpi::isa_irq_table` walks the MADT's interrupt source overrides once and
returns all sixteen legacy IRQs resolved, with six host tests holding it. `irq::record_isa_routing`
copies the answer into a static, and `irq::enable(intid)` takes a *legacy IRQ number* the way the
arch contract's other two implementations take an INTID or a PLIC source.

The flags word is two two-bit fields, not two bits, and reading it as two bits gets both wrong.
Bits 1:0 are polarity (`00` conforms to the bus, `01` active high, `11` active low) and bits 3:2 are
trigger mode (`00` conforms, `01` edge, `11` level). QEMU emits `0x000d` for the PCI-link IRQs,
which is `0b1101`: active high, level. A decoder that tested bit 1 for polarity would answer
"active low" and arm a line that never asserts. There is a test named for exactly that misreading.

`00` meaning "conforms to the bus" is also not a synonym for "active high, edge" even though the two
coincide on the ISA bus; the default lives in one place (`IsaIrqRouting::isa_default`) so a future
reader cannot change it in only one of the two.

### What it was proved with, and why the PIT

The PIT twice over: `timer.rs` already drives it for calibration, and IRQ 0 is the one line every PC
rewires, so routing it is simultaneously the easiest device to reach and the strongest test of the
override table. Channel 0 is the one whose output goes to the interrupt controller, which is
exactly why calibration cannot use it and why this can use nothing else. Mode 2 (rate generator)
pulses the line once per reload and reloads itself.

Measured on QEMU TCG, 2026-08-24, with the local APIC timer masked for the window so the count is
the PIT's alone:

```
  io apic     : id 0 at 0xfec00000 up, version 0x20, 24 redirection entries, gsi base 0
  device irq  : pit irq 0 -> gsi 2 on vector 0x32: 20 interrupts in ~0.2s at 100 Hz
```

Twenty is the same number the local APIC timer produces in the same window against the same TSC,
which is what makes it a measurement rather than a nonzero.

### The choices, and what each one costs

The vector map is flat: `GSI_VECTOR_BASE + gsi`, with the base at 0x30 so that 0x20..0x2f stays
free for the local APIC's own LVT sources (the timer, and later thermal, performance, error and
inter-processor vectors). Flat means a stray vector in a fault report names its line by subtraction.
It costs the ability to express a priority policy, since x86 priority is the vector's top four bits
and a flat map fixes which line lands in which class. There is no policy to express yet.

Physical destination mode, fixed delivery, at the boot CPU. Not lowest-priority delivery and not
a logical group: the simplest thing that is correct on one CPU and stays correct on several.
Distributing interrupts is a policy too.

Every entry is masked during bring-up. They power on masked, so this changes nothing on a cold
boot. It matters on a warm one, where firmware may have armed a line for its own use and left it
armed. An inherited interrupt arriving on a vector this kernel never assigned is a puzzle with
no clue in it.

The 8259s stay masked rather than being remapped. The same device line reaches both controllers,
so an unmasked 8259 would deliver a second copy of every interrupt the redirection table routes, on
a vector that is an exception number.

### What this did not touch

PCI interrupt routing, which is blocked on AML rather than on tables: a PCI function's legacy
interrupt goes through a router the DSDT's `_PRT` describes, and there is no interpreter here.
MSI, which bypasses the redirection table entirely by writing a vector straight to the local
APIC, is the path worth building for that reason and is its own piece of work.

## The fine map, and the hazard that was designed out instead of sequenced around

Milestone 161's roadmap item 1, built 2026-08-24. What `boot.s` leaves behind is enough to run and
not enough to be a kernel. It has 2 MiB pages, everything present, writable *and* executable, in both
halves, plus an identity map of the low 4 GiB sitting in the half ring 3 will get. `mmu::init`
replaces it with a four-level map built through the shared `paging::Mapper`, exactly as the other
two architectures do.

What the new map says, in the order `map_everything` builds it:

| What | Where | Flags |
|---|---|---|
| All of RAM, minus the kernel image's own frames | `DIRECT_MAP_BASE + pa` | `kernel_data` (RW, never X) |
| The first megabyte, and reserved entries below the top of RAM | `DIRECT_MAP_BASE + pa` | `kernel_data` |
| `.text` | `KERNEL_VA_BASE + pa` | `kernel_code` (X, never W) |
| `.rodata` | `KERNEL_VA_BASE + pa` | `kernel_rodata` (neither) |
| `.data` + `.bss`, the boot stack, the per-CPU secondary and interrupt stacks | `KERNEL_VA_BASE + pa` | `kernel_data` |
| Every guard page | -- | not mapped, and `verify` asserts it |
| The local APIC (from the MADT), the IO APIC, bus 0 of the PCIe ECAM window | `DIRECT_MAP_BASE + pa` | `device` (uncacheable) |
| Physical page 0, and the identity map | -- | not mapped |

The image's own frames are skipped in the direct map, which is a difference from the other two
ports rather than a copy of them. There, the image and the direct map share a base, so a direct-map
entry for those frames would collide with the section mappings and the mapper's overwrite refusal
catches the ordering mistake. Here the bases differ, so it would not collide: it would quietly be a
second, writable alias of `.text`. W^X that a second mapping undoes is not W^X.

### The hazard the roadmap flagged, and why there is no sequencing step

`phys_to_virt` changes meaning the instant a new `CR3` is installed, if the old and new tables put
the direct map in different places. That is not hypothetical: `memory::bring_up_page_frames` turns the
frame bitmap's physical address into a `&'static mut [u8]` and stores it, and the PVH structure
and the ACPI tables are read the same way, all before any fine map exists.

The roadmap's prescribed answer was to carry both aliases across the switch and drop the old one
afterwards. The answer taken instead was to make the arithmetic never change. `boot.s` writes
PML4[273] itself, pointing at the same low PDPT the identity map already uses. So the direct map
exists from before the first Rust instruction and `mmu::init` widens it rather than introducing it.
Eight bytes, no extra memory, no second step, and nothing to remember. The two constants are kept in
agreement by a `const` assertion in `mmu.rs` that recomputes PML4[273] from `DIRECT_MAP_BASE`, so
changing one without the other fails the build rather than the boot.

The one thing that *does* change across the switch is the local APIC page's memory type, from
the boot map's cacheable to device. `irq.rs` keeps its base as a direct-map address and needs no
re-derivation.

### What made it debuggable

`verify` walks the finished tables in software and asserts the five things that would kill the
machine, before `mov cr3` bets on them. This function's own code is mapped, executable and not
writable. The current stack is mapped. The frame bitmap is reachable through the direct map. PML4[0]
is zero (the identity map is gone). And every guard page is a hole. The identity-map check has to
read the PML4 entry rather than call `translate`, because the kernel mapper serves the high half
and would answer `None` for a low address whatever the tables said. That is the shape of assertion
this file exists to warn about: one that passes for a reason unrelated to the thing it names.

The console is the other reason this step was less frightening here than on the other two
architectures: COM1 is an I/O port, so nothing the page tables do can make the machine go silent.

### What it costs, measured

`556 KiB of page tables for 254 MiB of RAM` on q35 with `-m 256M`, printed on every boot. That 0.2%
is the price of 4 KiB leaves, which is all `crates/paging` maps; Linux uses 2 MiB and 1 GiB leaves
for this map and would pay a few kilobytes. Extrapolated, a 32 GiB machine would spend ~64 MiB of
RAM describing its own RAM. Adding larger leaves is a change to the shared format trait that all
three architectures would want, so it is recorded in `arch/x86_64/mmu.rs`'s `BUGS` rather than
patched here.

Two other honest limits live in that same `BUGS` section. `mmu::init` must draw its table frames
from below 4 GiB, because that is all the boot map's direct map reaches and the mapper writes
every table through it. (The frame allocator hands out the lowest free frame first, so this holds
today.) And `CR4.PGE` is off, so the `G` bit the kernel's `Flags` set is ignored and every
`mov cr3` flushes the whole TLB: correct and slow, and worth revisiting with ring 3, when a context
switch starts happening often enough to measure.

## The direct map in blocks (2026-09-19)

`crates/paging` maps 2 MiB and 1 GiB leaves now (`PageSize`, `Mapper::map_span`, `map_block`), and
the direct map's RAM uses them: 560 KiB of page tables on QEMU's 256 MiB became 60 KiB, and at
4 GiB, 8,252 KiB became 64 KiB. 1 GiB leaves are used only where `CPUID` leaf 0x80000001 reports
`Page1GB` (`mmu::largest_leaf`); `-cpu qemu64` does not, and boots in 2 MiB blocks. Device windows
and firmware reservations stay in 4 KiB pages, because this kernel does not read the MTRRs and a
large page spanning two memory types is undefined (`mmu.rs`'s BUGS). aarch64 and riscv64 adopted the
same rule for their direct maps in the same change, so this is not an x86 difference.
