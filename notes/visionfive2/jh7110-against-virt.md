# The JH7110 against QEMU virt, in detail

*Name: ratified 2026-10-18 (calef, "Approve all").*

An appendix to [notes/visionfive2.md](../visionfive2.md), which carries the summary table. This
file holds the evidence for each row: the Image header and its load-address trick, DRAM, the UART,
the PLIC, the CLINT, PCIe and the SBI extensions. Source tags such as [dtsi] are defined in the
parent.

## The Image header, and the load-address trick

U-Boot's `booti` refuses a payload that does not carry the RISC-V Linux Image header. That is a
64-byte prelude whose one checked field is the u32 magic 0x05435352 ("RSC\x05") at offset 0x38
[uboot-img]. The header format is Linux's [linux-hdr]. `kernel/src/arch/riscv64/boot.s` now emits
it (milestone 16a, of milestone 16 (real hardware + IOMMU-backed driver isolation)). QEMU never reads it (the ELF goes in via `-kernel`), so it is 64 dead bytes
there.

`booti` then relocates the image to `ram_base + text_offset` whenever the loaded file sits in RAM,
which it always does [uboot-img]:

```c
if (force_reloc ||
   (gd->ram_base <= image && image < gd->ram_base + gd->ram_size)) {
    *relocated_addr = gd->ram_base + lhdr->text_offset;
}
```

That line is why the kernel needs no board relink. The kernel is linked for physical 0x8020_0000
(`link-riscv64.ld`), which on QEMU `virt` is DRAM base + 2 MiB. The VF2's DRAM starts at
0x4000_0000 [dtsi]. So our header states `text_offset = 0x40200000`, and `booti` moves the image to
0x4000_0000 + 0x4020_0000 = 0x8020_0000. That is the linked address. It is comfortably inside DRAM
on every VF2 variant; even the 2 GB board's RAM runs to 0xC000_0000.

This is an exception and a foot gun, on the record. Linux uses `text_offset = 0x200000` ("2 MiB
into RAM, wherever RAM is"). Ours means "0x8020_0000 absolute, on any board whose RAM starts at
0x4000_0000". A future board with a different DRAM base gets the wrong address from this header.
There are two alternatives, if that day comes. One is a board-specific link: PHYS_START, plus the
boot page table's gigapage index in `arch/riscv64/mmu.rs`, plus this header value. The other is
teaching `boot.s` to run at an arbitrary 2 MiB-aligned load address the way Linux does. Both were
deliberately not built for one board that does not need them.

## DRAM

| | QEMU `virt` | VisionFive 2 |
|---|---|---|
| DRAM base | 0x8000_0000 | 0x4000_0000 [dtsi] |
| Kernel runs at | 0x8020_0000 | 0x8020_0000 (via the header, above) |
| Size | whatever `-m` says | 2/4/8 GB by variant; the 4 GB board's node is `reg = <0x0 0x40000000 0x1 0x0>` [dtsi] |

The kernel already handles the consequences. RAM extent comes from the DTB `/memory` node
(`kernel/src/memory.rs`), not from a constant, so the base difference is discovered rather than
assumed. And since 2026-08-14 the boot page table maps gigapage 1 as well (0x4000_0000..
0x8000_0000, `arch/riscv64/mmu.rs`). So a DTB at U-Boot's default `fdt_addr_r` = 0x4600_0000
[uboot-cfg] is readable before the fine tables exist. It used to fault there before the trap path
could print.

Still out of reach: `$fdtcontroladdr` (the control DTB) near the top of RAM. It sits above
gigapage 2 on every variant and above 4 GiB on an 8 GB board. The runbook moves the DTB to
0x8600_0000 (inside gigapage 2) for exactly that case. Keeping the `fdt move` in the manual first
boot also removes one variable from a first bring-up.

An 8 GB board's RAM spans past 4 GiB (0x4000_0000 + 8 GiB = 0x2_4000_0000). The JH7110 also
aliases DRAM uncached at 0x24_0000_0000 [uboot-doc]. The alias appears in no `/memory` node and
needs nothing from us.

## The UART

Same base address as QEMU `virt`, different silicon behind it. The JH7110's UART0 is a Synopsys
DesignWare DW_apb_uart, an 8250 derivative [dtsi]:

| | QEMU `virt` NS16550 | JH7110 UART0 |
|---|---|---|
| compatible | `ns16550a` | `starfive,jh7110-uart`, `snps,dw-apb-uart` [dtsi] |
| base | 0x1000_0000 | 0x1000_0000, size 0x10000 [dtsi] |
| reg-shift | 0 (byte registers, consecutive) | 2 (registers 4 bytes apart) [dtsi] |
| reg-io-width | 1 | 4 (32-bit accesses) [dtsi] |
| clock | 3.6864 MHz (QEMU ignores the divisor anyway) | 24 MHz [uboot-cfg] |
| PLIC irq | 10 | 32 [dtsi] |

`drivers/ns16550.rs` grew four things on 2026-08-14, built and QEMU-proven. The JH7110 side of each
is still a bench question, because QEMU emulates none of this silicon.

1. A register stride and access width, carried as data. The driver's `Shape` holds `reg-shift`
   and `reg-io-width`, defaulting to QEMU's byte wiring. On the board LSR lives at byte offset
   0x14, not 5. The old byte access at offset 5 read the middle of the IER word, so the THRE poll
   spun on garbage.
2. The divisor from the stated clock, and only from a stated clock.
   `console::configure_from_dtb` programs `clock-frequency / (16 x 115200)`, rounded. At 24 MHz
   that gives 13, an actual rate of 115385, 0.16% high. The two expected divisors are proved at
   compile time in the driver. When the tree states no clock, it leaves the divisor and line
   controls alone. Mainline JH7110 trees express the UART clock as a `clocks` phandle this kernel
   does not resolve. U-Boot has already programmed 115200 8N1 on any board that showed a prompt,
   so not touching it is correct there too. A divisor guessed against the wrong clock is 1.5 Mbaud
   garbage at the far terminal, which is the failure this rule exists to avoid. QEMU's tree states
   3.6864 MHz, so the suite now programs divisor 2 where it used to write a constant 1. QEMU
   ignores both.
3. The DW busy quirk, keyed on the `snps,dw-apb-uart` compatible. A DW_apb_uart ignores an LCR
   write while busy and latches a "busy" interrupt. So `init` drains the transmitter (LSR.TEMT,
   bounded) before touching LCR.
4. The shape is adopted before the first `println!`. `kernel_main` calls
   `console::configure_from_dtb(dtb)` immediately after `console::init`, so no output is ever
   produced with a stale stride. The node is matched by its name, `serial@10000000`, pinned beside
   the equally hardcoded base address. The jh7110 fixture test
   (crates/machine_discovery/tests/riscv64_jh7110.rs) is the witness for both.

With that built, the honest first-boot expectation moves up one rung: the banner should appear.
That holds provided the DTB U-Boot hands us is readable (see DRAM above) and the silicon matches the
dtsi's description. The parent's triage ladder still covers every way that can fail.

## The PLIC

The PLIC is at QEMU's address with a different context map. It is `sifive,plic-1.0.0` at
0xC00_0000, with 136 sources [dtsi]. On QEMU `virt` every hart has an M and an S context, and hart
h's S context is `2h + 1`. That is the formula `kernel/src/smp.rs` uses. On the JH7110 the disabled
S7 contributes only an M context. The dtsi's `interrupts-extended` reads
`<&cpu0_intc 11>, <&cpu1_intc 11>, <&cpu1_intc 9>, <&cpu2_intc 11>, <&cpu2_intc 9>, ...`. So the
layout is context 0 = hart 0 M, then for U74 hart h in 1..4, context `2h - 1` = M and context `2h`
= S. Hart h's S context is `2h` on this board, not `2h + 1`.

Built 2026-08-14: the mapping comes from the DTB now. `isa::plic::PlicContexts` decodes
`interrupts-extended`. Entry k is context k, interrupt 9 marks an S context, and phandles resolve to
harts through each cpu's `riscv,cpu-intc` child. `arch::irq::init_contexts` records it at boot. The
`2h + 1` formula survives only as the fallback for a tree that does not state the layout.

The PLIC node itself is found by its `sifive,plic-1.0.0` compatible rather than by name. The JH7110
spells the node `interrupt-controller@c000000` where QEMU says `plic@c000000`, and the old `plic@`
name-prefix read found nothing there. It is QEMU-proven in both directions. The kernel suite asserts
the live `virt` tree reproduces `2h + 1`. The host fixtures hold the JH7110's `2h` answer with no S
context for hart 0 (crates/machine_discovery/tests/riscv64_plic_contexts.rs). What QEMU cannot
prove, the real PLIC honoring context `2h`, is a bench fact like everything else here. (The board's
real control DTB later turned out to spell the PLIC `riscv,plic0`; see
[on-board-test-suite.md](on-board-test-suite.md).)

## The CLINT and the timebase

The CLINT is at QEMU's address: `starfive,jh7110-clint` at 0x200_0000 [dtsi]. Timer and IPI go
through SBI anyway, so this is OpenSBI's problem, not ours.

Timebase is 4 MHz (`/cpus/timebase-frequency` [dtsi]), against QEMU `virt`'s 10 MHz. It is already
handled: `arch/riscv64/timer.rs` reads the rate from the DTB and panics rather than assumes.

## PCIe

The JH7110's PCIe is a PLDA XpressRICH controller (`starfive,jh7110-pcie` in mainline trees). That
is not the `pci-host-ecam-generic` device QEMU's `virt` boards expose, and it has no driver here.
Driving it is its own milestone, not a bench fix.

Since 2026-08-14 the kernel's PCIe windows come from the device tree. Those are the ECAM config
space and the 32-bit memory window BARs are placed in. `memory::init` reads the generic-ECAM node's
`reg` and `ranges`. `mmu::map_everything` maps the windows only when the node exists. Every probe in
kernel/src/pci.rs reports nobody home when it does not (notes/pcie.md). On this board there is no
such node, so nothing PCIe is mapped or touched. That is the honest statement of where the PLDA
controller stands.

Before that the windows were QEMU constants, and the first bench boot paid for it. The BAR constant
0x4000_0000 is this board's DRAM base (see the DRAM table above). So `map_everything` tried to lay a
device mapping over memory step 1 had already direct-mapped. The mapper's overwrite refusal panicked
the boot right after the banner. It was the first casualty of a QEMU constant proven on silicon rather than predicted.

## SBI extensions

OpenSBI is the vendor firmware's M-mode resident. So TIME, IPI, RFENCE and HSM (the bring-up path
`arch::psci_cpu_on` uses) are the standard set. SRST (system reset) is how the board can reboot or
power off from S-mode. Two things are deliberately on the bench list: which OpenSBI version is in
the shipped flash, and whether its PMU extension is present with how many of the U74's hpmcounters
it exposes. The version banner prints on every boot and `sbi probe` answers the rest. Guessing a
counter count here would be exactly the manufactured fact this note exists to avoid.

## The test exit, as first proposed

There is no semihosting on this board. The riscv test exit (`arch/riscv64/semihosting.rs`) is not
semihosting at all. It is QEMU `virt`'s `sifive_test` finisher, an MMIO word at physical 0x10_0000
that tells QEMU to exit with a status. The JH7110 has no such device, and a store to 0x10_0000 there
is a bus error at best. So the kernel's test build, as it then stood, could not report pass/fail on
silicon.

The proposal was recorded then, and deliberately not built until the bench said it was needed. A
harness on the serial line greps for a fixed final UART line, `CRICKER-TEST-EXIT: PASS` or
`FAIL <code>`. SBI SRST shutdown follows, so the run terminates. Both halves are a dozen lines
against interfaces the kernel already has. The `sifive_test` path stays for QEMU, selected the same
way the finisher address already is. It landed on 2026-08-21 under the marker
`NIFE-TEST-EXIT` ([on-board-test-suite.md](on-board-test-suite.md)).
