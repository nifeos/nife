# The DesignWare Mobile Storage driver, source by source

An appendix to [notes/designware-mobile-storage.md](../designware-mobile-storage.md), which keeps
the part, the reuse, the data paths and the bench step. This file holds the evidence and sequences
that `crates/designware_mobile_storage`'s module docs summarize: the device-tree evidence for the
part, the card identification steps, and what the host simulation models.

## The JH7110's two controllers, as its device trees describe them

Both trees name the part, read 2026-10-06 (UTC).

- Mainline Linux, `arch/riscv/boot/dts/starfive/jh7110.dtsi`: `mmc0: mmc@16010000` and
  `mmc1: mmc@16020000`, `compatible = "starfive,jh7110-mmc"`, whose binding
  (`Documentation/devicetree/bindings/mmc/starfive,jh7110-mmc.yaml`) includes
  `synopsys-dw-mshc-common.yaml`. `clocks` are SYS-domain `SDIOn_AHB` (`biu`) and
  `SDIOn_SDCARD` (`ciu`), `resets` SYS-domain `SDIOn_AHB`, `interrupts` 74 and 75, and
  `fifo-depth = <32>`. `jh7110-common.dtsi` gives the VisionFive 2's wiring: `mmc0` is 8-bit, the
  eMMC socket, and `mmc1` is 4-bit, the microSD slot, both with
  `assigned-clock-rates = <50000000>`.
- The vendor U-Boot's tree, the one radon's firmware hands over (`starfive-tech/u-boot`,
  `JH7110_VisionFive2_devel`, `arch/riscv/dts/jh7110.dtsi` and `starfive_visionfive2.dts`):
  `sdio0: sdio0@16010000` and `sdio1: sdio1@16020000`, `compatible = "snps,dw-mshc"`, the same
  clocks and resets under vendor names, the same widths and rate, `fifo-depth = <32>`, and no
  `interrupts` property.

radon's own U-Boot banner says the same thing from the other side: `MMC: sdio0@16010000: 0,
sdio1@16020000: 1` (`bench/radon-2026-10-05/boot5-main.log`, line 59). It loads its boot script
from `mmc 1:1`, the microSD slot.

So the part is the Synopsys DesignWare Mobile Storage Host Controller (`DW_mshc`, the databook's
name; Linux's `dw_mmc`), the older Synopsys SD/MMC core with its own register map and FIFO. It is
not the newer DesignWare Cores Mobile Storage Host (`DWC_mshc`, Linux's `sdhci-of-dwcmshc`), which
is SDHCI-compatible. The two share a vendor and an acronym and nothing else. `VERID` on silicon
names the release, and the bench step prints it.

## Bringing a card up

The identification sequence is the SD Physical Layer Simplified Specification's figure 4-2 (version
9.10), with JEDEC JESD84's eMMC sequence beside it, in the order OpenBSD's `sdmmc_mem_enable` and
`sdmmc_mem_init` take them (`sys/dev/sdmmc/sdmmc_mem.c`, ISC). Which kind of card is in the slot is
decided by what answers.

1. `CMD0`, everything to idle, at 400 kHz.
2. `CMD8` with the check pattern. An SD 2.0 card echoes it; an older SD card and an eMMC do not
   answer.
3. `ACMD41` (`CMD55`, then `CMD41`) until the card reports power-up complete. An eMMC does not
   answer `CMD55`, which is how it is told from an SD 1.x card; it is then brought up with `CMD1`
   instead.
4. `CMD2` for the CID, `CMD3` for the relative address (the card picks it on SD; the host assigns
   it on eMMC), `CMD9` for the CSD, and `CMD7` to select.
5. SD: `ACMD6` to 4-bit if the slot is wired for it, and `CMD16` to 512 bytes. eMMC: `CMD8` reads
   the 512-byte `EXT_CSD` for the true sector count, and the bus stays 1-bit (the crate's BUGS).
6. The clock goes up to 25 MHz, default speed. High speed and UHS modes are not attempted.

## What the simulation models

Nothing emulates this controller. QEMU has no `DW_mshc` model for any RISC-V machine, and the one
`dw_mmc`-shaped device QEMU does have, on some Arm boards, is not reachable from the machines this
tree boots. So the host tests run the real driver against `sim`, and the bench step checks `sim`
against silicon. It models the contract the driver relies on, as the databook and OpenBSD's driver
describe it, and no more.

- `CTRL`'s reset bits clear themselves one read after they are set.
- A write to `CMD` with the start bit either loads the clock (and clears the bit) or runs the
  command against the card. Running it sets the response words and `RINTSTS` (`CD`, plus `RTO`
  when nothing answered), and clears the bit.
- A data phase moves through a FIFO of `depth` words. Reads fill it from the card and raise `RXDR`,
  writes drain it to the card and raise `TXDR`, and the end raises `DTO`. A multi-block transfer
  also raises `ACD`, the automatic CMD12.
- `STATUS` reports the FIFO's fill and, for a few reads after a write, data busy.
- A read with the card read threshold (`CARDTHRCTL`) enabled above the FIFO's size ends at once
  with `FRUN` and `DTO`, the word radon's controller gave for it.
- With the IDMAC selected in `CTRL` and enabled in `BMOD`, the FIFO's other end is `Memory`. The
  walk starts at `DBADDR` and loads each descriptor. It suspends with `DU` on one it does not own,
  and stops with `FBE` on an address outside memory. It moves the buffer and closes the descriptor
  (clears `OWN`, raises `RI` or `TI` unless `DIC`), then stops after `LD` or follows `CH`. A read's
  last words reach memory after `DTO`, as on silicon, so the driver must wait for the descriptor
  too.

Time is one microsecond per call to `now_us`, so every deadline is reached in a bounded number of
polls, and a test that would hang on silicon fails here instead.
