# designware_mobile_storage

An SD card and eMMC (embedded MultiMediaCard) driver for the Synopsys DesignWare Mobile Storage Host
Controller (`snps,dw-mshc`, Linux's `dw_mmc`), as pure logic. It holds the register map, the command
encoding, the polled controller sequences, the internal direct memory access controller (IDMAC),
card identification, block reads and writes, the card registers' decoding, and a master boot record
(MBR) rule for where a write test may land. It never touches memory itself: you implement one small
trait, `host_controller::RegisterWindowAndClock` (read a register, write one, read a microsecond
clock), over your own mapped window. The crate is `no_std`, has no allocator, and was built against
the StarFive JH7110 (VisionFive 2).

## EXAMPLES

Hand `HostController::new` your register window, the controller's input clock and the FIFO (first
in, first out) depth from the device tree, then identify the card and move whole 512-byte blocks:

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

`partition::MasterBootRecord::parse` reads that sector, and `MasterBootRecord::scratch_sector_range`
names the sectors before the first partition that a write test may use.
`HostController::with_internal_direct_memory_access` moves later transfers through a direct memory
access (DMA) region you supply (`internal_direct_memory_access::DirectMemoryAccessRegion`). The
`jh7110` module holds that SoC's constants and `Slot::from_node`, which decodes one slot from the
properties of a device-tree node you found. The crate reads no device tree itself.

Every error enum (`host_controller::Error`, `sd::InitializationError`, `sd::BlockTransferError`,
`internal_direct_memory_access::Refusal`) implements `Display` and `core::error::Error`. They and
`sd::CardKind` are `#[non_exhaustive]`, so a `match` on one needs a wildcard arm and a later release
can add a variant without breaking you.

## What is proven

Each property below has a Kani harness in `src/proofs.rs`, twenty-three in all:

- No partition table makes the write test's scratch range touch a partition or sector 0.
- A FIFO step never moves more than the transfer still owes, and a write step never more than the
  FIFO has room for, whatever `STATUS_REGISTER` says.
- A block range the driver admits lies inside the card and inside what the command's 32-bit
  address can name.
- An IDMAC descriptor chain describes exactly the transfer's bytes, keeps each buffer inside the
  region's data pages, and links only to descriptors in its own chain page.
- The MBR parse accepts exactly the sectors that end in the signature, reads each entry from its
  own bytes, and never panics on any sector 0.
- A field read from a 128-bit register never exceeds its width. Decoding any card-specific data
  (CSD) register, as SD or as eMMC, sizes the card without overflow and sizes nothing for an unknown
  structure or a block length under 512 bytes. The `EXT_CSD` sector count is read from bytes 212 to
  215 of any buffer, and a buffer too short for it gives zero.

The harnesses below assume nothing of the controller: every register read and every descriptor
word in DMA memory is whatever the solver picks. Of the clock, whose readings the solver also picks,
they assume only that time passes: from some reading on, each reading is more than the deadline past
every earlier one. Before that reading it may wrap past 2^64, stand still or jump either way. They
prove that no path panics, overflows or indexes out of bounds, and that each wait ends. They do not
prove that the driver does the right thing with a faithful controller; the host tests against the
simulated controller cover that, and only a board shows it for silicon.

- A wait ends at the first clock reading past its deadline and reads the clock no more.
- The IDMAC wait ends once the clock has passed its deadline, however the controller rewrites the
  descriptors: the deadline restarts only when one more descriptor has closed, so at most once per
  descriptor in the chain (bounded here at two).
- The card clock divider is total for any base and target (no zero divisor, no overflow, no
  panic), and it bypasses the divider exactly when the base is at or below the target.
- A command word carries the start bit and the command's own index, never the clock-update-only
  bit, a data phase exactly when the command has one, and the write bit only on a write.
- Sending a command with no data phase ends and never panics, whatever the controller answers.
  Every error names the command sent, a bad response carries the status bit that made it bad, and
  a short response leaves words 1 to 3 zero.
- A reset and a card clock change end and never panic. From a 50 MHz base the card then runs at
  or below any target asked for, or at the slowest rate the divider has when the target is below
  it.

- A FIFO data phase stays inside its buffer and ends, whatever fill and status the controller
  reports, for data phases of one or two blocks of 4 or 8 bytes through a FIFO one or two words
  deep. The loop's arithmetic does not depend on the length, and the FIFO step proof above holds
  for any length.
- Reading one block through the IDMAC never sends a write command, and the driver touches the
  DMA region only inside it.
- Writing one block through the IDMAC sends at most one write command, of 512 bytes, addressed to
  the block asked for, and only when the card has that block.
- The steps of identification end, never panic and never send a write command: reset, `CMD0`,
  `CMD8`, power-up and addressing on the polled host, then configuration (an eMMC's `EXT_CSD`
  read through the IDMAC). A configured SD card has a capacity and runs no faster than 25 MHz.
  `identify` itself only builds a `Card` from these steps' results; as one harness it ran past
  five minutes.

The harnesses of whole sequences use a clock that passes every deadline at each reading, so they
bound their loops by what the controller reports rather than by time. The two wait harnesses
above cover a clock that takes longer, and every wait in the driver is one of those two loops.

The bring-up harnesses below judge `identify` against the card state machine of the SD
specification and JEDEC JESD84 (idle, ready, identification, standby, transfer). The card starts
in any state, with any address and bus width, and the controller with any bus width and clock, as
an earlier bring-up that failed anywhere might leave them. For each command the solver picks
whether the card answered, stayed silent or had its answer garbled, and the response words. The
controller reports that choice and otherwise carries out each command at once; a controller that
fails some other way reaches the driver as the same error from the same call, and the harnesses
above cover it. The one thing assumed of a card is that an SD card publishes a relative address
other than zero.

- Every command `identify` sends is legal in the state the card is in: no step runs before the
  one it needs, an application command always follows `CMD55`, `CMD9` and `CMD7` go to the
  card's own address, and no command writes.
- A card comes back from `identify` only after it went from `CMD0` through every state to
  transfer. The `Card` names the address, operation conditions register (OCR) and kind the card
  has; an SD card has its 512-byte block length and the bus width the slot allows, an eMMC has
  had its `EXT_CSD` read on a 1-bit bus, and the controller runs the transfer clock.
- Whatever a failed bring-up left, the next one resets the controller, powers the slot, drops it
  to a 1-bit bus at the identification clock and sends `CMD0` before anything else, and a card it
  hands back runs one bus width on the card, the controller and the `Card`.
- Against a card that reports busy to every `ACMD41` or `CMD1`, power-up ends with
  `InitializationError::PowerUp` and the last OCR, and sends at most one more power-up command
  once the clock has passed its one-second deadline. Its clock runs forward at any pace, from any
  start below 2^62.

The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
checker, so it needs network access. CI uses version 0.68.0, and another version may verify
differently.

Rerun them with `cargo kani`. Each harness has a patch at
`falsifications/<module.path>.<harness>.patch` that plants the defect it rules out; apply one with
`patch -p3` from the crate directory and its harness fails at its own assertion.

## BUGS

- Default speed only: 25 MHz, 4-bit SD, 1-bit eMMC. No tuning, high speed, ultra high speed (UHS) or
  8-bit bus.
- Polled, with interrupts masked. No card-detect or write-protect handling.
- A 16- or 64-bit FIFO is refused, and the IDMAC builds only 32-bit descriptors, so its region
  must sit below 4 GiB.
- The polled path has read a real card; the IDMAC path has so far run only against the simulated
  controller in the tests.
- The IDMAC moves at most its region's data pages per command and copies through them once each
  way.
- That the card clock stays at or below the target is proved for a 50 MHz base only. For a base
  the solver picks, the comparison makes it build two symbolic 32-bit divisions, and that ran
  past five minutes with every solver tried and with the base bounded to 2^28.
- A clock that never passes a deadline hangs the wait on it: the trait promises microseconds, and
  the driver has no other way to tell a slow controller from a dead one.
- The block harnesses move one block, and through the IDMAC only. The polled path's FIFO loop is
  proved for data phases of 16 bytes or less: a 512-byte block took too many FIFO steps for the
  solver. The host tests move whole blocks and multi-block transfers both ways, through a 32- and
  a 48-word FIFO and with a confused fill count.
- Identification is proved to walk the card state machine in order, but nothing proves that it
  reads the right capacity or tells SD from SDHC (SD High Capacity); the host tests do that against
  simulated SD, SDHC and eMMC cards. The MBR parse does not know the card's size, so whether a
  partition fits the card is the check of the layer that mounts it.
- The address an SD card publishes with `CMD3` is taken as it comes. A card that published zero
  would get `CMD7` at zero, which deselects every card, and the bring-up proofs assume a card
  publishes a nonzero address.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of the MIT license (`LICENSE-MIT`) or the Apache License, Version 2.0
(`LICENSE-APACHE`), at your option. Parts are adapted from OpenBSD's `dwmmc.c` under the Internet
Systems Consortium (ISC) license; `src/lib.rs` carries its notice. The manifest's SPDX expression is
`(MIT OR Apache-2.0) AND ISC`. Card identification follows the SD specification and takes only the
order of its addressing commands from OpenBSD's `sdmmc_mem.c`, whose code is not adapted.
