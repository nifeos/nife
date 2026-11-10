---
status: PARTIAL
raised: 2026-08-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: a rented Scaleway Elastic Metal RV1
specific_machine: none
needs_person: yes
---
# 89. Scaleway EM-RV1: a second RISC-V implementation, rented

Raised 2026-08-03 alongside milestone 88 (nife on rented silicon: Oracle's free tier first, Graviton metal for the
PMU), when the cloud-hardware survey
turned up rentable riscv64 silicon. Rewritten 2026-09-25 (UTC) as the port plan, after calef ruled
to rent the machine ([§215 (the second RISC-V machine is a rented Scaleway Elastic Metal
RV1)](../decisions/0215-the-second-risc-v-machine-is-a-scaleway-rv1.md)). Milestone 556 (a second
RISC-V implementation, for €16 a month) was the same proposal filed a second time and is folded in
here. The host-side PLIC prep is built; the rest is below.

One rented RV1, which needs calef's Scaleway account before anything runs.
Every step that can run under QEMU or on the host is ordered first, so the meter starts at a step
that needs the machine.

## What this is for

Fatal risk 9's implementation grain: a second machine of an architecture nife already boots
([`design/fatal-risks/README.md`](../fatal-risks/README.md)). radon is a StarFive JH7110 with SiFive U74 cores.
The RV1 is a T-Head TH1520 with four C910 cores. A second JH7110 would share every assumption this
tree could have made about one vendor's silicon; this machine shares almost none.

The verdict is a count. Every change the port needs lands in one of three places: under
`kernel/src/arch/riscv64/`, in portable code that reads a device tree, or elsewhere. The first two
are what §4 (kernel shape) promises. A change of the third kind is evidence
that the seam is not where the tree says it is, and the port's report lists each one.

## How far away it is, measured 2026-09-25

Read from Linux's `th1520.dtsi` at `165768bb7` and OpenSBI 1.9, then grepped against this tree.
radon's bring-up note opened by calling the JH7110 "startlingly close to QEMU's `virt` machine".
The TH1520 is not.

| Difference | What it breaks here | Where the fix lives |
|---|---|---|
| Peripherals at `0xff_d800_0000` and up, 40 bits | `phys_to_virt(pa) = pa + KERNEL_VA_BASE` only reaches 256 GiB under Sv39; the UART address overflows it at compile time | `arch/riscv64/mmu.rs` |
| UART0 at `0xff_e701_4000`, IRQ 36 | `console.rs`'s `UART_PHYS` and `UART_NODE` are one riscv64 constant, shared by luck with the JH7110 | a per-machine early-console constant, which `console.rs` already names as the right shape |
| PLIC states `thead,c900-plic`, two-cell interrupts | the region lookup and the context map both missed it | built: one shared list, `machine_discovery::plic::COMPATIBLES` |
| DRAM from physical 0 | the Image header's `text_offset` of `0x4020_0000` lands the kernel at the wrong address under `booti` | none if the FIT's own load address is used, which the first hour confirms |
| QEMU-only mappings (virtio window at `0x1000_1000`) are mapped unconditionally | that range is DRAM on the TH1520, and radon's first boot died of exactly this collision | `arch/riscv64/mmu.rs`, from the tree |
| XTheadMae: PTE bits 63:59 carry the memory type, and firmware turns it on | nife writes zero there; the behavior of zero is undefined in T-Head's spec | `crates/paging`'s Sv39 leaf encoding plus a probe in `arch/riscv64/` |
| XTheadVector's `sstatus.VS` sits at bits 24:23, not 10:9 | `fp.rs` clears the standard field only, which is the 2026-09-24 audit's fix missing this core | `arch/riscv64/fp.rs`, keyed on the vendor id |
| No Sstc, timebase 3 MHz | nothing: the timer is SBI TIME and the rate comes from the tree | none |
| DMA is not coherent (`dma-noncoherent`), no Zicbom | nothing for first light, which polls the UART. Every DMA driver later | follow-on |
| SBI SRST not in upstream OpenSBI for this SoC | the `board` build's test exit calls SRST | probe; power API as fallback |

## Steps, in order

### Before renting: host and QEMU (built 2026-10-06 (UTC), except where marked)

1. Built 2026-09-25: the PLIC is found by `thead,c900-plic` and its two-cell specifier decodes,
   witnessed by `crates/machine_discovery/tests/riscv64_th1520.rs` against a fixture modeled on the
   dtsi.
2. Built: a device window. Physical `0xff_c000_0000..0x100_0000_0000` appears at root index 511
   and `phys_to_virt` is piecewise (`arch/riscv64/mmu.rs`, `DEVICE_WINDOW_PA`). The direct map is
   capped at 255 GiB, and RAM past the cap panics rather than aliasing. Const-asserted.
3. Built: a per-machine early console under the `board_th1520` feature (implies `board`; name
   provisional). The address was written three times and now lives once, in
   `arch/riscv64/machine.rs`.
4. Built: the virtio-mmio window is mapped only when the tree names a `virtio,mmio` node, and the
   probe asks `arch::mmu::virtio_slots()`. On the TH1520 that address is DRAM; on radon it is
   UART0, which the probe had been reading as virtio slots.
5. Built: XTheadMae. `paging::Sv39Mae` (name provisional) is Sv39 plus Linux's memory types and is
   `arch::mmu::Format` under `board_th1520`; the boot table carries the same bits.
   `arch::isa::init` reads `th.sxstatus.MAEE` on a T-Head hart and panics if build and hart
   disagree. QEMU's `thead-c906` reports MAEE clear, so `script/cpu-matrix` runs the probe and the
   default build's half of the check; only the machine can run the `Sv39Mae` half.
6. Built: on a T-Head hart, `fp::init` also clears XTheadVector's `VS` at `sstatus[24:23]`.
7. Built: `script/board-image --th1520` builds the pair and writes a `.its` copied from Scaleway's
   ([`em-rv1`](https://github.com/scaleway/em-rv1)), with the kernel at `0x8020_0000` instead of
   `0x20_0000`, the archive as a ramdisk, and the tree and OpenSBI taken from the machine's own
   `boot.itb`. Checked with `dtc`, not `mkimage`.

Under `board_th1520` the boot table also names all 16 GiB, because nobody has seen where this
U-Boot leaves the tree. QEMU `virt` boots as before on `rv64` and `thead-c906`, with and without a
virtio-mmio disk.

### What the machine is, read 2026-10-06 (UTC)

| Fact | Value | Source |
|---|---|---|
| SoC | T-Head TH1520, four C910 at 1.85 GHz | [Scaleway Labs page](https://labs.scaleway.com/en/em-rv1/) |
| Board | Sipeed LM4A modules on a Lichee Cluster 4A carrier | the `#include` in Scaleway's `.dts` |
| RAM | 16 GiB at physical 0 | Labs page; RevyOS `th1520-lpi4a-cluster-16g.dts` (`memory@0`, `0x4_0000_0000`) |
| Storage | 128 GB eMMC, the only documented boot medium | Labs page; [RV1 guidelines](https://www.scaleway.com/en/docs/elastic-metal/reference-content/elastic-metal-rv1-guidelines/) |
| Boot | U-Boot looks for `boot.itb` on GPT partition 1 or 2, FAT32; the FIT holds `kernel`, `fdt`, `opensbi`, `env` and optionally `ramdisk` | RV1 guidelines |
| Firmware | U-Boot unpublished; OpenSBI travels inside the FIT | RV1 guidelines |
| Console | "a serial console accessible via SSH is activatable on your account"; how is not documented | Labs page FAQ |
| Rescue | supported: `scw baremetal server reboot <id> boot-type=rescue`, network-booted Ubuntu in RAM, user `rescue` | RV1 guidelines; [CLI docs](https://github.com/scaleway/scaleway-cli/blob/master/docs/commands/baremetal.md) |
| Zone | `fr-par-2` only | Labs page |
| Stock | "temporarily out of stock" for new servers on 2026-06-03, per a third party | [tunbury.org, 2026-06-03](https://www.tunbury.org/2026/06/03/emulated-riscv-workers/) |

Scaleway's custom-kernel guide is broken as written: its `.dts` includes a file no RevyOS branch
still has, and the repository was last pushed 2024-03-26. nife's path writes its own `.its` and
reuses the machine's tree, so it is unaffected. Unverified, and hour one's to settle: whether U-Boot honors the kernel's `load`, which of
`load` or `entry` becomes OpenSBI's next address, whether it writes `linux,initrd-start` for the
ramdisk, and how the console is switched on.

## Runbook

Every step is calef's, because it spends money or acts on his Scaleway account. Step 0 is free:
the serial-console request, stock and price, and `script/board-image --th1520`. Steps 1 and 2 rent
the machine, read it and try first light, with recovery through rescue mode. Step 3 is the soak and
step 4 the risk 9 count; step 5 deletes the server so billing stops. They are in
[`0089-scaleway-em-rv1/runbook.md`](0089-scaleway-em-rv1/runbook.md), moved there verbatim on
2026-10-13 (UTC), with the before-renting count table.

## What it costs

€0.042 an hour excluding VAT (€0.0504 with France's 20%), billed per allocated hour; the monthly
rate is €15.99 with a month's fee as commitment ([pricing](https://www.scaleway.com/en/elastic-metal-rv1/),
the Elastic Metal FAQ). Budget 30 hours through the soak: €1.26, or €1.51 with VAT. Spent so far:
nothing. The trap is the delete: a server left allocated for a month costs €30.66. The real cost
is attention to transcripts, which is why the host and QEMU steps came first.

## The seven questions

1. What else was considered. A second radon shares every assumption. A bought TH1520 board was
   refused by §203 (capacity is rented rather than bought) and needs a person at the bench. RISE's
   runners and Cloud-V boot no custom kernel with a console. §215 has each.
2. What this tree already does: radon's port, whose note, `notes/visionfive2.md`, is the template.
   Its lesson was that QEMU hid the DRAM base, the online-hart set, the PLIC context formula and a
   zero-latency UART. Steps 2 and 4 are that lesson applied before the machine instead of at it.
3. Prior art, read: Linux's `th1520.dtsi`, `errata/thead/errata.c` and `irq-sifive-plic.c`;
   OpenSBI's `platform/generic/thead`; T-Head's extension spec; Scaleway's RV1 guide and FIT
   source; QEMU's `th_csr.c` at `d7a65d17`. Linux's memory-type values are recalled, not re-read;
   the unit tests pin them as literals.
4. Is the premise true: that the RV1 boots a custom kernel was an open question on 2026-08-03. It
   is now documented. Scaleway's guide boots a user-supplied FIT image from eMMC. What is not
   documented is the U-Boot version and how the console is activated. Newer doubt: new RV1s were
   reported out of stock on 2026-06-03. Runbook step 0 answers that before anything is spent.
5. Cost: measured above in euros and estimated in hours. The hours are an estimate from radon's
   record, not a measurement.
6. Reversibility: code, plus an hourly rental. No wire format, syscall or ratified name.
7. Same cost for both: the window in step 2 is chosen over Linux's per-device `ioremap` because it
   is one table entry and one compare, not a virtual allocator. With equal effort the window still
   wins for a kernel that maps a handful of devices once at boot. If a machine ever puts devices in
   two distant gigabytes, that verdict changes.

## BUGS

- The fixture is modeled on Linux's tree, not dumped from the RV1's firmware. radon's modeled
  fixture got the PLIC's compatible string wrong, and only the real tree showed it. Hour one's
  second step exists to fix this one.
- A 0% SLA. Any gate built on this machine must degrade to a loud skip, as milestone 81 (an HVF
  leg: the test suite on the physical core) does.
- The PLIC here has 240 sources; `drivers/plic.rs` keys a table on `source % 128`. UART0's 36 is
  safe, and a source above 127 would alias.
- **If firmware leaves MAEE off, the `board_th1520` kernel dies silently.** Its boot table sets
  bits 63:59, which are reserved without MAEE, so the first walk faults before the probe that
  would explain it can run. That OpenSBI turns MAE on comes from Linux's errata code, not from
  this machine. A plain-Sv39 TH1520 build would be one feature line, if hour one needs it.
- Reading `th.sxstatus` from S-mode is what T-Head's manual and QEMU's model allow. No C910 here
  has done it. A trap there would be an illegal-instruction exception during `isa::init`.
- Physical 0 is RAM. Nothing in this tree has run where frame 0 could be allocated, and code that
  treats a zero address as "none" would misbehave. `/reserved-memory` covering OpenSBI at 0 keeps
  it out of the allocator; hour one checks the live tree for that node.
- SBI SRST is not in upstream OpenSBI for this SoC, so a `board` test exit may hang rather than
  power off. The boot tour and the soak do not exit, so neither depends on it.
- No QEMU machine models the TH1520, so the `board_th1520` image has been built and never run.
  What it does differently is const-asserted, host-tested (`Sv39Mae`), or run under QEMU by the
  default build.

## Follow-on

- **Outstanding.** The runbook, every step of it calef's: the serial-console request, the stock
  check, then hour one, first light, the soak and the risk 9 count. Nothing before renting is left.
  Checked 2026-10-06 (UTC) with `script/board-image --th1520`, which builds and writes the FIT
  source, and `grep -n 'CONSOLE_UART_PHYS' kernel/src/arch/riscv64/machine.rs`.
- **Outstanding.** A board profile for the RV1 in `script/board-console`, once a capture shows its firmware
  prologue. Until then the runbook replays captures under `xenon`'s empty prologue, which is the
  same refusal argon's missing profile records: a prologue read from documentation is a guess.
- **Milestone 655.** Milestone 655 (dMA on a non-coherent RISC-V machine). `design/roadmap/0655-dma-on-a-non-coherent-risc-v-machine.md`, DMA on a non-coherent RISC-V machine: T-Head's `th.dcache.cpa`, `ipa` and `cipa`,
  then `th.sync.s`, behind the same seam a Zicbom machine would use. No DMA driver runs on this
  machine until then.

## Index row

Real riscv64 silicon from a second vendor, a T-Head TH1520 with C910 cores, rented at €0.042 an
hour. It is fatal risk 9's implementation-grain experiment, and a count of changes outside
`arch/riscv64/` is its verdict. The distance was measured on 2026-09-25: MMIO past the Sv39 direct
map, T-Head's own page-table memory types, and a PLIC binding this tree did not accept.
