# radon's SD card and eMMC: the DesignWare Mobile Storage Host Controller

Milestone 53 (the board's own peripherals: network and storage on real silicon), the storage half.
The driver is `crates/designware_mobile_storage`. The kernel's part is
`kernel/src/designware_mobile_storage.rs` (a register window and a clock),
`kernel/src/user/designware_mobile_storage_service.rs` (the EL0 server's wiring) and
`kernel/src/storage_bench.rs` (the bench boot). The EL0 block server, which answers
`filesystem_contract::blk` like the virtio and NVMe servers, is
`components/src/designware_mobile_storage.rs`. Names provisional.

## What the part is, and how we know

The JH7110 has two SD/MMC controllers: `0x1601_0000` wired to the VisionFive 2's eMMC socket
(8-bit), and `0x1602_0000` wired to its microSD slot (4-bit), the card radon boots from. Both are a
**Synopsys DesignWare Mobile Storage Host Controller** (`DW_mshc`, Linux's `dw_mmc`). Three sources
agree, read 2026-10-06 (UTC):

- the vendor U-Boot tree radon's firmware hands over names `sdio0@16010000` and
  `sdio1@16020000`, `compatible = "snps,dw-mshc"`, with no `interrupts` property;
- mainline Linux names `mmc@16010000` and `mmc@16020000`, `compatible = "starfive,jh7110-mmc"`,
  whose binding includes `synopsys-dw-mshc-common.yaml`, on interrupts 74 and 75;
- radon's own U-Boot banner: `MMC: sdio0@16010000: 0, sdio1@16020000: 1`
  (`bench/radon-2026-10-05/boot5-main.log`).

It is not the newer DesignWare Cores Mobile Storage Host (`DWC_mshc`, `sdhci-of-dwcmshc`), which
is SDHCI-compatible and has a different register map. Which release this one is (`VERID`) and how
wide its FIFO is (`HCON`) are the first two things the bench step reads. On radon both slots read
`VERID 0x5342290a` (release 2.90a) and `HCON 0x00c43cc1`: a 32-bit FIFO, 32 words deep, the kind
this driver moves (Results, below).
`crates/designware_mobile_storage/src/jh7110.rs` carries the node-by-node evidence.

## What was reused, and what was not

Reuse: adapted from OpenBSD's `sys/dev/fdt/dwmmc.c` (revision 1.33, 2026-03-11, ISC), which
matches both compatibles above and so already drives this part on this board. Its register map,
reset, clock and bus-width sequences, command flags and polled FIFO transfer are the crate's
`regs`, `command` and `host` modules; the identification order follows its `sdmmc_mem.c`. The ISC
notice is in the crate root.

The crates.io candidate, `starfive-jh7110-dwmmc` 0.1.8 (Apache-2.0, from `rcore-os/tgoskits`), was
read and not taken. It is 333 lines of JH7110 policy over `dwmmc-host` 0.4.2 (5,253 lines), which
pulls in `sdmmc-host`, `sdmmc-protocol`, `dma-api`, `mmio-api`, `volatile`, `bitfield-struct` and
`embedded-hal`. Four reasons, in order of weight:

1. Its data path is IDMAC only ("there is no FIFO fallback"), so its first read on radon would test
   DMA coherence and the card protocol at once. See the next section for why this lane wanted them
   apart.
2. Its volatile accesses live inside the driver core. This tree's split puts every register access
   behind one trait the kernel implements, which is what lets the whole driver run against a
   simulation on a host and under Kani.
3. Five crates from one young tree, with sixteen `dwmmc-host` releases since August 2026. A
   dependency that size is an architect's ruling (§46 (thin primitives or whole subsystems; we
   write everything in between)). Taking it would not have removed the need for a simulation to
   test against.
4. Effort was not the reason. Had both been the same work, the split and the PIO-first bring-up
   would still have decided it.

What it did give: its JH7110 constants (a 50 MHz reference clock, a 32-word, 32-bit FIFO) match
both device trees, a third independent agreement. Linux's `dw_mmc.c` and `dw_mmc-starfive.c`,
U-Boot's driver and both device trees are GPL and were read only for facts about the hardware.

## Why the CPU moves the data, for now

The controller has its own DMA engine (the IDMAC). This driver does not use it yet. Through the
FIFO, nothing but MMIO stands between the card and the CPU, so a silicon read tests the controller
and the card protocol and nothing else. The cost is throughput, and the bench step prints it
rather than anyone guessing.

radon's DMA is coherent with its caches: the network half's `designware_ethernet::coherence`
probe read `Coherent` on 2026-10-20 (UTC) (notes/designware-ethernet.md). So the IDMAC path needs
no cache maintenance on radon, only the `fence` the Ethernet driver already uses, and it is a
contained change: descriptors in one page, the same `Registers` trait, and a sim that models the
descriptor walk. It waits on the polled read working first (Results, below), so that a DMA read
that fails is not also a FIFO read that fails.

## Never break the card radon boots from

radon's microSD card holds the FAT32 partition U-Boot loads the boot script, kernel and archive
from (notes/visionfive2.md). Three rules keep this driver from costing that card:

- **The first bench step cannot write.** `storage_bench` without `NIFE_STORAGE_BENCH_WRITE=scratch`
  compiles no write test at all, and `bench::read_only` sends no write command (a host test checks
  the command log).
- **A write test lands only before the first partition.** `partition::Mbr::scratch` names at most
  eight sectors, ending just before the first partition and starting no lower than sector 1. It
  refuses a GPT disk, an empty table, or a partition at sector 1. A Kani harness proves that range
  touches no partition for every possible table.
- **It puts back what it found.** The test reads the range first, writes a pattern, reads it back,
  writes the original back, and reads that back too. The eMMC socket is never written by this
  boot.

The gap before the first partition belongs to no filesystem, and U-Boot reads nothing there when
the board boots from QSPI flash, which is how radon is set up (DIP switches both low).

## calef's bench step on radon

About fifteen minutes, three boots, and the card is never removed. radon's card was written once
with `--tftp` (milestone 257 (boot radon over the network)), so each boot is a build and a power
cycle. Do these on patagonia, in this branch's worktree.

Before you start. radon's DIP switches on QSPI (both low), the UART on patagonia, the Ethernet
cable in the port U-Boot netboots over. radon's card asks for a stale server address and an old
archive name; notes/designware-ethernet.md's bench step says what to type at U-Boot and what to
serve until the card is rewritten. In a second terminal, leave the TFTP server running:

```sh
script/board-netboot
```

Power is radon's own outlet. **Never switch any other outlet, and never unplug a USB hub.** Which
outlet is which is kept off-tree by the maintainers.

### Step 1: read-only

```sh
git log -1 --format=%h
script/board-image --tftp --extra-features storage_bench
mkdir -p bench/radon-$(date -u +%F)
script/board-console --for 3m --until none --log bench/radon-$(date -u +%F)/storage-bench-read.log
```

Power-cycle radon with plug 2 while `board-console` is watching. The banner line says
`Read-only build: nothing on any card is written.` If it says anything else, stop.

What to look for, all lines prefixed `storage-bench:`:

| line | what it settles |
|---|---|
| `clocks before ... already up true` | whether U-Boot handed the controller over running |
| `VERID ... HCON ... (FIFO at 0x200, Some(4) bytes wide, 32 words deep ...)` | the release, and that the FIFO is the 32-bit kind this driver moves |
| `card SdHighCapacity, ... blocks (... MiB), block-addressed, 4-bit at 25000000 Hz` | identification worked |
| `partition 1: type 0x0b, sectors N..` and `boot sector: signature 55aa, type field "FAT32   "` | reads return the card's real bytes |
| `scratch range a write test would use: Some((N-8, 8))` | where step 2 would write, before it does |
| `timed read: 16384 blocks in ... us, ... KiB/s` | the polled read rate |
| `EL0 block server: Served` | the same card through the EL0 server, as the FS server would call it: block 0 of partition 1 is the boot sector, a write is refused `EROFS`, a block past the window `EINVAL` |
| `slot 0 (eMMC socket): ...` | whether an eMMC module is fitted (`NO-CARD` if not) |
| `verdict READ-OK ... KiB/s, EL0 block server served` | the step passed |

`NO-CARD` on slot 1 or any `FAILED` verdict: stop, keep the log, and send it. Nothing was written.

### Step 2: the scratch write test

Only if step 1 said `READ-OK` and printed a scratch range. The range is the up to eight sectors
just before partition 1's first sector; on a card made by `diskutil eraseDisk FAT32 NIFE MBR` that
is inside the alignment gap no filesystem uses.

```sh
NIFE_STORAGE_BENCH_WRITE=scratch script/board-image --tftp --extra-features storage_bench
script/board-console --for 3m --until none --log bench/radon-$(date -u +%F)/storage-bench-write.log
```

Power-cycle with plug 2. The banner now says `WRITE TEST BUILT IN: the microSD card's
pre-partition gap only.` Look for:

| verdict | meaning |
|---|---|
| `READ-OK ..., WRITE-VERIFIED sectors A..B (were zero), restored, EL0 block server served` | writes work from the kernel and through the EL0 server (one block, the same range), and the range is back as it was |
| `READ-OK ..., WRITE-REFUSED: no gap before the first partition` | nothing was written; the card's layout leaves no room |
| `FAILED: write test PatternMismatch(..)` or `Failed(..)` | the original was still written back; send the log |
| `FAILED: write test RestoreMismatch(n)` | sector `n`, inside the gap, did not take its original back. Partition 1 was never touched |

### Step 3: put the ordinary image back

```sh
script/board-image --tftp
```

One more power cycle, and radon boots what everyone else's bench work expects. Then commit the two
logs under `bench/radon-<date>/` and say which commit they came from.

### Results

| date (UTC) | commit | step | slot 1 (microSD) | slot 0 (eMMC) | verdict |
|---|---|---|---|---|---|
| 2026-10-20 | `290d3ba8b` | 1, read-only | identified; the first block read failed, `Host(Data(17, 2060))` | `NoCard(NoResponse(1))` | `FAILED: read of block 0` |

The log is `bench/radon-2026-10-20/storage-bench-read.log`, and that directory's `README.md` says
how the boot was run. The banner said `Read-only build: nothing on any card is written.` Steps 2
and 3 were not run, as step 1 says. What it settled:

- U-Boot hands both controllers over running (`already up true`). It leaves the microSD
  controller with `CTRL 0x2000000` (its IDMAC selected), `CLKDIV 0x2`, `CLKENA 0x10001` and a
  4-bit bus; `Host::reset` clears the DMA selection before this driver uses the FIFO.
- **Identification works on silicon.** `bench::read_only` prints the `card` line only after the
  whole probe returns, so the line's absence does not mean identification failed: the failure is
  `ReadFailure::Read(0, ..)`, which `sd::identify` returning `Ok` precedes. Every command of
  identification answered, through `ACMD6` to 4-bit, `CMD16` and the clock raised to 25 MHz.
- **The first data phase failed.** `Host(Data(17, 2060))` is `Error::Data`: command 17
  (`READ_SINGLE_BLOCK`, block 0) and the `RINTSTS` word at the failure, `0x080c`. That is bits 2
  (`CD`, command done), 3 (`DTO`, data transfer over) and 11 (`FRUN`, FIFO underrun or overrun),
  and nothing else: no CRC error (`DCRC`), no start-bit or end-bit error, no read timeout
  (`DRTO`, `HTO`). `RXDR` is absent, but `Host::read` clears it after each batch it drains, so
  its absence does not say whether any words moved. The card answered and sent its block; the
  FIFO's accounting between the card and the CPU broke. `FRUN` is in `INT_DATA_ERRORS`, so the
  driver stopped there.
- Slot 0: no eMMC answered `CMD1`, consistent with no module fitted.

The cause is not diagnosed. Two places in the code fit the bits, and nothing yet tells them
apart (`BUGS`).

## Parity

Rule 5's scope note: these controllers are the JH7110's, on one riscv64 board, so the kernel half
and the bench boot are riscv64-only (`storage_bench` is a compile error elsewhere). The crate is
architecture-neutral; a board with this controller on another ISA (several Rockchip arm64 boards
have one) would need the kernel half's twenty lines and a device-tree match.

## BUGS

- **The first block read fails on radon** with `FRUN` (Results, above), so nothing past
  identification has worked on the device. Two candidates, neither confirmed. First, `Host::issue`
  programs the card read threshold (`CARDTHRCTL`) to the block size, 512 bytes, but radon's FIFO
  holds 128 (32 words of 4 bytes): a threshold the FIFO can never meet. Linux's `dw_mmc` leaves
  the threshold off when a block is larger than the FIFO (recalled, not re-read). Second, the
  driver reading a word the FIFO did not hold, an underrun. The sim models neither the threshold
  nor a card-side overrun, which is why no host test failed. The next bench boot wants the
  threshold left off when a block exceeds the FIFO, and the failure to carry the bytes moved and
  `STATUS`; that is milestone 53's outstanding item.
- Every other test is against `sim`, which models the contract as the databook and OpenBSD
  describe it.
- **The booted system has no disk on radon yet.** The EL0 block server exists and the bench boot
  calls it. `PROVEN_ON_SILICON` keeps the booted system from starting it, and which part of the
  card nife's filesystem would live on is an architect's call (the 53 block).
- The EL0 server holds the controller's DMA registers, which share its one page. The JH7110 has
  no IOMMU, so the server is as confined as its own code. That is milestone 261 (the NVMe driver
  leaves the kernel)'s position for a machine with no IOMMU. The server does no DMA.
- Polled, default speed, no DMA. 25 MHz, 4-bit on SD and 1-bit on eMMC, the CPU moving every
  word. No high-speed switch, no UHS-I, no HS200 tuning, no 8-bit eMMC bus.
- A 64-bit FIFO is refused rather than handled; `HCON` says which radon has.
- A read's tail may raise no FIFO request. Linux's `fifo-watermark-aligned` property, which
  both mainline JH7110 nodes carry, exists because a transfer whose tail is shorter than the
  receive watermark can end with words in the FIFO and no `RXDR`. This driver drains the FIFO on
  data-transfer-over as well, and a host test reads a block whose tail is below the watermark.
  Whether radon's controller behaves that way is not known; the test covers both.
- No card detect, no write protect. `CDETECT` is printed and not acted on.
