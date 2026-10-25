# designware_mobile_storage

An SD card and eMMC driver for the Synopsys DesignWare Mobile Storage Host Controller
(`snps,dw-mshc`, Linux's `dw_mmc`), as pure logic. It holds the register map, the command encoding,
the polled controller sequences, the internal DMA controller (IDMAC), card identification, block
reads and writes, the card registers' decoding, and an MBR rule for where a write test may land.
It never touches memory itself: you implement one small trait, `host::Registers` (read a register,
write one, read a microsecond clock), over your own mapped window. The crate is `no_std`, has no
allocator, and was built against the StarFive JH7110 (VisionFive 2).

## What is proven

Each property below has a Kani harness in `src/proofs.rs`:

- No partition table makes the write test's scratch range touch a partition or sector 0.
- A FIFO step never moves more than the transfer still owes, and a write step never more than the
  FIFO has room for, whatever `STATUS` says.
- A block range the driver admits lies inside the card and inside what the command's 32-bit
  address can name.
- An IDMAC descriptor chain describes exactly the transfer's bytes, keeps each buffer inside the
  region's data pages, and links only to descriptors in its own chain page.

Rerun them with `cargo kani`. Each harness has a patch at
`falsifications/<module.path>.<harness>.patch` that plants the defect it rules out; apply one with
`patch -p3` from the crate directory and its harness fails at its own assertion.

## Usage

Hand `Host::new` your register window, the controller's input clock and the FIFO depth from the
device tree, then identify the card and move whole 512-byte blocks:

```rust
use designware_mobile_storage::host::{Host, Registers};
use designware_mobile_storage::sd;

fn first_sector<R: Registers>(regs: R) -> Option<[u8; 512]> {
    let (mut host, _id) = Host::new(regs, 50_000_000, Some(32)).ok()?;
    let card = sd::identify(&mut host, 4).ok()?; // at most a 4-bit bus
    let mut buf = [0u8; 512];
    sd::read_blocks(&mut host, &card, 0, &mut buf).ok()?;
    Some(buf)
}
```

`partition::Mbr::parse` reads that sector, and `Mbr::scratch` names the sectors before the first
partition that a write test may use. `Host::with_idmac` moves later transfers through a DMA region
you supply (`idmac::DirectMemoryAccessRegion`). The `jh7110` module holds that SoC's constants.

## Limitations

- Default speed only: 25 MHz, 4-bit SD, 1-bit eMMC. No tuning, high speed, UHS or 8-bit bus.
- Polled, with interrupts masked. No card-detect or write-protect handling.
- A 16- or 64-bit FIFO is refused, and the IDMAC builds only 32-bit descriptors, so its region
  must sit below 4 GiB.
- The polled path has read a real card; the IDMAC path has so far run only against the simulated
  controller in the tests.
- The IDMAC moves at most its region's data pages per command and copies through them once each
  way.
- The card bring-up state machine, timing and the CID decode have no proofs; the MBR parse and
  the capacity decoders are not yet proven total.
- The device-tree fixtures under `tests/fixtures/` are excerpts of the mainline and vendor JH7110
  trees whose licenses are not yet recorded.

The minimum supported Rust version is 1.88.

## License

Licensed under either of the MIT license (`LICENSE-MIT`) or the Apache License, Version 2.0
(`LICENSE-APACHE`), at your option. Parts are adapted from OpenBSD's `dwmmc.c` under the ISC
license; `src/lib.rs` carries its notice.
