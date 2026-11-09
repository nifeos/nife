# Prior art, and what the DesignWare pair should share

An appendix to [the milestone](../1050-designware-mobile-storage-outside-review.md).

## Prior art

| system | what it offers | what a user would miss here | what this crate has that it lacks |
|---|---|---|---|
| Linux `dw_mmc` | interrupts, card detect, UHS and HS200/HS400 tuning, 8-bit and DDR eMMC, `CMD23`, erase, SDIO, external DMA and 64-bit IDMAC, power management, error recovery with `CMD12` | all of the first column except the IDMAC; most of all the recovery (P1-1) and bus speeds | no `unsafe`, host tests against a simulated card, 23 machine-checked harnesses, a driver that compiles for three bare-metal targets with no kernel |
| U-Boot `dw_mmc` | polled, IDMAC, high speed, 8-bit eMMC, `CMD12` on error | high speed, 8-bit, recovery | the same as above; the closest shape to this crate |
| `dwmmc-host` 0.4.2 (crates.io, 742 downloads in 90 days, 1,861 total; `rcore-os/tgoskits`) | a DesignWare backend for `sdmmc-protocol`, FIFO and IDMAC, `no_std` | an existing ecosystem split between protocol and controller | the proofs, the falsification discipline, the totality results against any controller; whether it uses raw pointers is unverified |
| `sdmmc` 0.1.0 (548 in 90 days) | a `no_std` SD/MMC/eMMC library for ARM64 platforms | a second controller family | the same |
| `sdio-host` 0.9.0 (520,132 in 90 days, unchanged since 2022) | CID, CSD, OCR and SCR decoding as types, no controller | richer card-register decoding (SCR, switch status) | a controller driver |
| `embedded-sdmmc` 0.10.0 (54,704 in 90 days) | SPI-mode SD plus a FAT filesystem | a filesystem; SPI mode | a native 4-bit controller path with DMA |

Demand, as measured on crates.io at review time: the two generic protocol crates draw hundreds of
thousands of downloads a quarter. The two DesignWare and ARM64 driver crates draw under a thousand
each. The unique claim is the proof and test discipline, not the feature list, and the README
already states it that way.

## What the DesignWare pair should share

`designware_ethernet` is the sibling, not yet extracted. The two crates already have one shape:

- a register map adapted from OpenBSD, with the ISC notice in the crate root;
- spelled-out constants with the source spelling as `doc(alias)`;
- a `jh7110` module whose `from_node` takes a property closure, and two `COMPATIBLE_*` constants;
- a `#[cfg(test)]` simulator with a bus model, and deadline waits;
- Kani harnesses with falsification patches, and the same eight workflow callers.

They differ in the DMA region trait. The trait of this crate takes `u32` offsets and puts `barrier`
on the region (`DirectMemoryAccessRegion` in `src/internal_direct_memory_access.rs`). The trait of
the sibling takes `u64` offsets and puts the barrier on a separate tail-pointer trait.

The combine review should decide four things. One is whether there is one DMA region trait or two
(the milestone recommends two for now). Another is whether the simulator-as-feature of P2-2 becomes
the pattern for both. A third is whether a shared JH7110 device-tree helper is worth a crate. The
last is that the workflow files of the two callers are identical apart from `targets`. Inferred
from reading the sources of the sibling in the nife tree; not verified against an extracted
repository.
