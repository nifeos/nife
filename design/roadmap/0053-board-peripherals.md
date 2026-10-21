---
status: PARTIAL
raised: 2026-07-31
milestone_dependencies: 163, 242, 859
decision_dependencies: none
machine_requirements: riscv64 silicon with an NVMe controller and a NIC
specific_machine: radon (the first runs of both halves)
needs_person: yes
---
# 53. The board's own peripherals: network and storage on real silicon

The storage half is built for QEMU (pull request #193). It is the `non_volatile_memory_express`
crate (queue mechanics, host-tested, 5 Kani harnesses), a rule-2 kernel driver confined through the
IOMMU before enable, class-code enumeration over §18 (the PCIe transport), and an end-to-end boot
test on both ISAs. The network half is built to the bench (below, "The network half"), and so is the
radon SD card and eMMC half (below, "The storage half on radon"). The network half has passed its
radon bench boot; the storage half's first read on radon failed and is outstanding. What else
remains is two things. The board-side PLDA XpressRICH root
complex that carries the NVMe driver to the real M.2 slot is milestone 163 (the JH7110's PCIe root
complex), NOT-STARTED. And the EL0 NVMe driver that §86 (whether an NVMe driver can leave the
kernel) decides on is unbuilt. Scope and honest limits: notes/non-volatile-memory-express.md, BUGS
included.

In the second sense: the board is here and this needs hands on it. Bringing up
an NVMe controller and a real NIC means flashing, a serial console, and power-cycling a board that
will wedge, none of which a background lane can do. Both halves of the *old* gate are gone. The hardware cleared 2026-08-14: the board
is on the desk and 16a boots it through the full tour, so the two drivers finally have silicon to
exist on. The storage fork was decided by calef on 2026-08-15: NVMe first. The backup workload
(milestone 55 (Time Machine)) measures sustained sequential write and endurance, which SD media
fails. And a real
PCIe root-complex driver compounds into the x86 machine of milestone 87 (the x86_64 bare-metal machine) where an
MSHC driver serves one slot on one board. SD/eMMC stays in scope as the later path, undecided only
in its ordering against the network driver.

In brief. Milestone 16a boots a VisionFive 2 (firmware contract, NS16550, PLIC, Sv39). It does not
give the board a network or a disk. Everything above needs both, and this is where virtio stops
carrying us: every driver we have talks to QEMU's paravirtual devices, and real silicon has none.

What it needs.

- Ethernet. The JH7110 uses a Synopsys DesignWare GMAC (`dwmac`). Our net_stack (smoltcp) is
  device-agnostic above the driver, so this is a driver, not a stack rewrite. Rule 2 applies: it takes
  a base address and knows nothing else.
- Storage, and there is a real choice here. The SD/eMMC controller is the simplest path; NVMe
  over PCIe is the better one, because §18's PCIe transport already exists and NVMe would give the
  backup target actual throughput. Deciding which comes first is a fork, and it should be decided on
  measurement of what the backup workload needs rather than on what is easiest.
- Persistence proven the hard way. RedoxFS on the real device, with crash consistency tested by
  **actually cutting power**, which is a test QEMU cannot run.

**The parity note this milestone must carry.** These drivers are board-specific and aarch64 has no
equivalent board yet, so rule 5's "a scope note records the gap and the plan" applies rather than its
"ships on every architecture". Say so explicitly; do not let it look like an oversight.

**Effort: not estimated.** Two device drivers against real hardware with no emulator to iterate
against is a different activity from everything done so far, and estimates calibrated on QEMU work
do not transfer.
## The network half

calef, 2026-10-06 (UTC): radon is the most convenient lab machine, so move its hardware support
forward, and do not split this milestone. Built in pull request #1762, every name provisional.
notes/designware-ethernet.md has the split, the evidence and the runbook.

- `crates/designware_ethernet`: the controller (`snps,dwmac-5.20`) and its YT8531 PHY as
  host-tested logic, 47 tests against a simulated controller, two Kani harnesses with replayable
  falsifications. `crates/jh7110_clock_and_reset` gains the AON domain and `gmac0`'s plans.
- `kernel/src/designware_ethernet.rs` and `designware_ethernet_service.rs`, and a third NIC in
  `net_stack`, in milestone 494 (a driver for the network card a PC actually has)'s shape; the
  `network_bench` boot runs on radon over it.
- radon's DMA is coherent, by measurement: the bring-up's loopback probe, which tells the two
  directions of non-coherence apart, read `Coherent` on all three radon boots of 2026-10-20 (UTC).
  Every device tree and kernel policy read had said so; one Rust driver for the board flushes the
  L2 for its descriptors anyway, which this board does not need. What it means for milestone 655
  (DMA on a non-coherent RISC-V machine) is a proposal in the note.
- Parity scope note (rule 5). The driver is JH7110-specific and riscv64-only by design: no
  aarch64 or x86_64 machine nife runs on has this controller. The stack above it is shared by all
  three. A second board with a DesignWare QoS controller reuses the crate and needs only its glue.
- Run on radon, 2026-10-20 (UTC), from `290d3ba8b`: `verdict LEASED-AND-MEASURED`, 1000 Mbit/s
  full duplex, 100 MiB in 363 s, 2.3 Mbit/s (`bench/radon-2026-10-20/network-bench-3.log`; the
  note's Results row). The booted system brings the port up (`PROVEN_ON_SILICON` is `true`).
  The rate is outstanding, below.

Reuse: OpenBSD's `dwqe` and `ytphy` (ISC), adapted with the notice carried in the crate root,
because its JH7110 glue handles radon's v1.3B transmit clock (`starfive,tx-use-rgmii-clk`).
FreeBSD's `if_eqos_starfive.c` and `mcommphy.c` (BSD-2-Clause) also support this chip and were
read as an independent cross-check. Linux stmmac, U-Boot `dwc_eth_qos` and both JH7110 trees are
GPL and were read for hardware facts only. crates.io has one Rust driver for this board,
`dwmac-my` 0.2.0 (MIT, 3,650 lines). The search, on 2026-10-06, was for `dwmac`, `stmmac`,
`eqos`, `designware ethernet`, `jh7110` and `starfive`. It is refused as a dependency, for the
reasons 494 refused Redox's `e1000d`. It slices a receive buffer by the descriptor's length unchecked, up to 32 KiB against a
2 KiB buffer. It hard-codes the clock controllers' addresses (rule 2). It is one object, with no
place for the kernel/process split, and it would bring `bitflags` and `log`. It was read as a
third cross-check.

## The storage half on radon

calef, 2026-10-06 (UTC): radon is the most convenient lab machine, so move its hardware support
forward, and do not split this milestone. Every name in the storage half is provisional.
notes/designware-mobile-storage.md has the evidence, the safety argument and the runbook.

- The part is a Synopsys DesignWare Mobile Storage Host Controller (`snps,dw-mshc` in the vendor
  tree radon hands over, `starfive,jh7110-mmc` in mainline): the microSD slot at `0x1602_0000`
  and the eMMC socket at `0x1601_0000`.
- `crates/designware_mobile_storage`: the whole driver behind one `Registers` trait, tested
  against a simulated controller and card (SDHC, SD 1.x, eMMC, an empty slot, CRC faults, an old
  release's FIFO, a short tail), four Kani harnesses with replayable falsifications.
  `crates/jh7110_clock_and_reset` gains both controllers' SYS clocks and resets.
- `kernel/src/storage_bench.rs`, behind `storage_bench`: identifies the card, reads its partition
  table, times an 8 MiB read, then reads it again through the EL0 block server. Read-only unless
  built with `NIFE_STORAGE_BENCH_WRITE=scratch`, and even then it writes only the sectors before
  the first partition, and restores them.
- `components/src/designware_mobile_storage.rs`: `filesystem_contract::blk` at EL0 over one
  register page and a window of the card, the virtio and NVMe servers' shape. The booted system
  does not start it (`PROVEN_ON_SILICON`).
- The CPU moves every byte through the FIFO, so the first silicon read tests the controller and
  the card protocol without depending on radon's DMA coherence, which nobody has measured yet.
- Parity scope note (rule 5): the controller is the JH7110's, so the kernel half and the bench
  boot are riscv64-only, and `storage_bench` is a compile error elsewhere. The crate is
  architecture-neutral.
- Step 1 run on radon, 2026-10-20 (UTC), from `290d3ba8b`, read-only: the card identified, then
  the first block read failed, `Host(Data(17, 2060))` (`bench/radon-2026-10-20/storage-bench-read.log`;
  the note's Results). Nothing was written.

Reuse: OpenBSD's `dwmmc` (`sys/dev/fdt/dwmmc.c` 1.33, ISC), adapted with the notice carried in
the crate root; it matches both compatibles above. `starfive-jh7110-dwmmc` 0.1.8 on crates.io
(Apache-2.0) was read and refused as a dependency: 333 lines over `dwmmc-host` 0.4.2 (5,253 lines)
and three more crates from one young tree, with an IDMAC-only data path and volatile accesses inside
the driver core. Its JH7110 constants were a third cross-check. Linux's `dw_mmc`, U-Boot's driver
and both trees are GPL and were read for hardware facts only.

## Follow-on

- **Done.** The network half's first run on radon, 2026-10-20 (UTC): `LEASED-AND-MEASURED`
  with `coherence : Coherent`, and `designware_ethernet_service::PROVEN_ON_SILICON` is `true`
  citing it. notes/designware-ethernet.md's Results row and its proposal to 655 have the detail.
- **Outstanding.** The network half's rate on radon. The 2.3 Mbit/s row was a one-segment
  receive window times a scheduler tick per round trip, both in `net_stack`, reproduced under QEMU
  and fixed (notes/designware-ethernet/the-receive-rate.md). That page's bench step measures the
  fix on radon, which has not run.
- **Outstanding.** radon's second port, `gmac1`, which this driver does not bring up. Pull request
  #2470 proposes the milestone for it, unmerged.
- **Milestone 163.** The board-side PLDA XpressRICH root complex that would carry the NVMe driver
  to the real M.2 slot. NOT-STARTED on a HARDWARE gate.
- **Decision.** Whether the NVMe driver can leave the kernel is answered:
  `design/decisions/0086-el0-nvme-driver.md` is DECIDED, option 2a now with option 4 addable without
  reshaping the EL0 driver, and the choice between them settled by measurement.
- **Outstanding.** Waiting on calef: RedoxFS crash consistency proven by actually cutting power on
  radon. Nothing can power-cycle the board: milestone 224 (nothing can power-cycle radon) is
  REFUSED, because calef chose manual power, and its own measurements record zero replies from the
  plug on the subnet. It also needs a working storage read, below. And it needs a bench boot that
  puts RedoxFS on radon's card and writes through it, which nothing builds yet: `storage_bench`
  and the EL0 block server never mount a filesystem.
- **Outstanding.** The storage half's first block read on radon. Step 1 (read-only) identified the
  card, then `READ_SINGLE_BLOCK` of block 0 ended with `RINTSTS 0x080c`: command done, data
  transfer over, and a FIFO underrun or overrun, with no CRC error or timeout. The likely cause,
  the card read threshold enabled at 512 bytes for a 128-byte FIFO, is left off. A failed read
  prints the byte counts that tell an overrun from an underrun.
  notes/designware-mobile-storage.md's Results and `BUGS` have the decode and the evidence. calef
  reruns step 1, then steps 2 and 3. Passing them lifts the block server's `PROVEN_ON_SILICON`.
- **Done.** The architect's call on which part of radon's storage the booted system's block server
  serves: a USB drive (calef, 2026-10-07 UTC; §260 (radon's block server serves a USB drive)). It
  refused the second microSD partition, the eMMC socket and NVMe for now. It needs milestone 163 (the
  JH7110's PCIe root complex), because radon's USB 3 ports sit behind a VL805 on PCIe. It needs the
  xHCI driver of milestone 242 (USB host and HID, because on commodity hardware the keyboard is not
  a UART). It also needs a USB mass-storage driver, milestone 859 (a USB mass-storage driver:
  bulk-only transport and SCSI, as a confined server), which joined this block's dependencies at
  its promotion. The microSD bench
  step above still proves the MSHC driver; it is no longer the block server's target.
- **Outstanding.** The IDMAC data path, unblocked. Its gate was radon's DMA coherence, which the
  network half's probe has read as `Coherent`, so it needs no cache maintenance (milestone 655
  (DMA on a non-coherent RISC-V machine) is not on its path). It waits on the polled read working,
  so that a failing DMA read is not also a failing FIFO read. Then high speed and an 8-bit eMMC
  bus, measured against the polled rate step 1 prints.
- **Done.** The rule-5 parity note this block says it must carry is carried, in `notes/non-volatile-memory-express.md`
  under a heading naming this milestone, stating what ships on all three architectures and what is
  board-specific.
- **Recorded.** Half the reason NVMe was picked over SD is gone: milestone 55 is REMOVED as of
  2026-08-30, so the backup workload whose sustained sequential write decided the fork no longer
  exists. The root-complex-compounds-into-87 half of that argument survives; the measurement half
  has no customer, and `notes/non-volatile-memory-express.md` still cites 55's storage bench as the thing that will want
  real queue depth.

## Index row

16a boots the board; this is what makes it able to *do* anything, and it is where virtio stops
carrying us
