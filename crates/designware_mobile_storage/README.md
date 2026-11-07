# designware_mobile_storage

An SD card and eMMC driver for the Synopsys DesignWare Mobile Storage Host Controller
(`snps,dw-mshc`, Linux's `dw_mmc`), as pure logic. It holds the register map, the command encoding,
the polled controller sequences, the internal DMA controller (IDMAC), card identification, block
reads and writes, the card registers' decoding, and an MBR rule for where a write test may land.
It never touches memory itself: you implement one small trait, `host_controller::RegisterWindowAndClock` (read a register,
write one, read a microsecond clock), over your own mapped window. The crate is `no_std`, has no
allocator, and was built against the StarFive JH7110 (VisionFive 2).

## What is proven

Each property below has a Kani harness in `src/proofs.rs`:

- No partition table makes the write test's scratch range touch a partition or sector 0.
- A FIFO step never moves more than the transfer still owes, and a write step never more than the
  FIFO has room for, whatever `STATUS_REGISTER` says.
- A block range the driver admits lies inside the card and inside what the command's 32-bit
  address can name.
- An IDMAC descriptor chain describes exactly the transfer's bytes, keeps each buffer inside the
  region's data pages, and links only to descriptors in its own chain page.
- The MBR parse accepts exactly the sectors that end in the signature, reads each entry from its
  own bytes, and never panics on any sector 0.
- A field read from a 128-bit register never exceeds its width. Decoding any CSD, as SD or as
  eMMC, sizes the card without overflow and sizes nothing for an unknown structure or a block
  length under 512 bytes. The `EXT_CSD` sector count is read from bytes 212 to 215 of any buffer,
  and a buffer too short for it gives zero.

The harnesses below prove safety against a controller and a clock that owe the driver nothing:
every register read, every descriptor word in DMA memory and every clock reading is whatever the
solver picks. They prove that no path panics, overflows or indexes out of bounds, and that each
wait ends. They do not prove that the driver does the right thing with a faithful controller; the
host tests against the simulated controller cover that, and only a board shows it for silicon.
The one promise the clock is held to is that time passes: from some reading on, each reading is
more than the deadline past every earlier one. Before that reading it may wrap past 2^64, stand
still or jump either way.

- A wait ends at the first clock reading past its deadline and reads the clock no more.
- The IDMAC wait ends once the clock has passed its deadline, however the controller rewrites the
  descriptors: the deadline restarts only when one more descriptor has closed, so at most once per
  descriptor in the chain (bounded here at two).
- The card clock divider is total for any base and target (no zero divisor, no overflow, no
  panic), and it bypasses the divider exactly when the base is at or below the target.

The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
checker, so it needs network access. CI uses version 0.68.0, and another version may verify
differently.

Rerun them with `cargo kani`. Each harness has a patch at
`falsifications/<module.path>.<harness>.patch` that plants the defect it rules out; apply one with
`patch -p3` from the crate directory and its harness fails at its own assertion.

## Usage

Hand `HostController::new` your register window, the controller's input clock and the FIFO depth from the
device tree, then identify the card and move whole 512-byte blocks:

```rust
use designware_mobile_storage::host_controller::{HostController, RegisterWindowAndClock};
use designware_mobile_storage::sd;

fn first_sector<R: RegisterWindowAndClock>(registers: R) -> Option<[u8; 512]> {
    let (mut host, _id) = HostController::new(registers, 50_000_000, Some(32)).ok()?;
    let card = sd::identify(&mut host, 4).ok()?; // at most a 4-bit bus
    let mut buffer = [0u8; 512];
    sd::read_blocks(&mut host, &card, 0, &mut buffer).ok()?;
    Some(buffer)
}
```

`partition::MasterBootRecord::parse` reads that sector, and
`MasterBootRecord::scratch_sector_range` names the
sectors before the first partition that a write test may use.
`HostController::with_internal_direct_memory_access` moves later transfers through a DMA region you supply
(`internal_direct_memory_access::DirectMemoryAccessRegion`). The `jh7110` module holds that SoC's
constants and `Slot::from_node`, which decodes one slot from the properties of a device-tree node
you found. The crate reads no device tree itself.

Every error enum (`host_controller::Error`, `sd::InitializationError`, `sd::BlockTransferError`,
`internal_direct_memory_access::Refusal`) implements `Display` and `core::error::Error`. They and
`sd::CardKind` are `#[non_exhaustive]`, so a `match` on one needs a wildcard arm and a later release
can add a variant without breaking you.

## Limitations

- Default speed only: 25 MHz, 4-bit SD, 1-bit eMMC. No tuning, high speed, UHS or 8-bit bus.
- Polled, with interrupts masked. No card-detect or write-protect handling.
- A 16- or 64-bit FIFO is refused, and the IDMAC builds only 32-bit descriptors, so its region
  must sit below 4 GiB.
- The polled path has read a real card; the IDMAC path has so far run only against the simulated
  controller in the tests.
- The IDMAC moves at most its region's data pages per command and copies through them once each
  way.
- No proof shows that the card clock a divider gives stays at or below the target: a harness
  that compares them makes the solver build two symbolic 32-bit divisions, and it ran past five
  minutes with every solver tried and with the base bounded to 2^28. A host test checks it for
  every target at 50 MHz.
- A clock that never passes a deadline hangs the wait on it: the trait promises microseconds, and
  the driver has no other way to tell a slow controller from a dead one.
- The card bring-up state machine, timing and the CID decode have no proofs. The MBR parse does
  not know the card's size, so whether a partition fits the card is the caller's check.

The minimum supported Rust version is 1.88.

## License

Licensed under either of the MIT license (`LICENSE-MIT`) or the Apache License, Version 2.0
(`LICENSE-APACHE`), at your option. Parts are adapted from OpenBSD's `dwmmc.c` under the
ISC license; `src/lib.rs` carries its notice. The manifest's SPDX expression is
`(MIT OR Apache-2.0) AND ISC`. Card identification follows the SD specification and takes only the
order of its addressing commands from OpenBSD's `sdmmc_mem.c`, whose code is not adapted.
