# Review package: `designware_mobile_storage`

*Name: provisional, minted by lane lane/crate-review-packages-five. The `designware_mobile_storage`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the five packages
together outgrow the page.*

## In one paragraph

The whole SD and eMMC driver for the Synopsys DesignWare Mobile Storage Host Controller (`DW_mshc`,
Linux's `dw_mmc`) as pure logic. It holds the register map, the command word, the polled controller
sequences and the internal DMA controller's descriptor chains. It identifies the card, reads and
writes blocks, and decodes the card's identification and specific-data registers. The caller
implements one trait, `RegisterWindowAndClock` (read a register, write one, read a microsecond
clock), and every sequence runs on a host against a simulated controller and card. It is `no_std`,
with no allocator, no `unsafe` and no dependency. Eight Kani harnesses prove its parsers and bounds.
Every name is ratified. What the architect decides is how much of it to publish: it carries a
board's bench test and nine compile-time checks as public items.

## Identity

| item | value |
|---|---|
| crate | `designware_mobile_storage`, ratified on #1806, permanent on first publication |
| version | `0.1.0`; no tag in this clone |
| edition, `rust-version` | 2024, 1.88 |
| license | `(MIT OR Apache-2.0) AND ISC`; `LICENSE-MIT` and `LICENSE-APACHE` at the crate root, the ISC notice in `src/lib.rs` |
| `publish` | `false` |
| size | 7,107 lines in 13 files: 5,039 of library, 879 of shared tests, 969 of simulator, 220 of Kani harnesses |
| consumers in nife | `kernel`, `components` and `machine_discovery`, by path |

## Public API

Nine public modules and about 125 public items.

| module | what it holds |
|---|---|
| `registers` | the register offsets and bits the driver uses, `queue_offset`, `queue_width`, `uses_64_bit_direct_memory_access` |
| `command` | `Command`, `Response`, `Data` and the command word encoding |
| `host_controller` | `RegisterWindowAndClock`, `HostController` (reset, clock, send a command, read and write data), `Identity`, `Error`, `clock_divider`, `card_clock`, `queue_step` |
| `internal_direct_memory_access` | `InternalDirectMemoryAccess`, the `DirectMemoryAccessRegion` trait, `NoDirectMemoryAccess`, `Refusal` |
| `card` | `CardIdentification`, `CardSpecificData`, `extract_bits`, `r1_state` |
| `sd` | `identify`, `read_blocks`, `write_blocks`, `Card`, `CardKind`, `can_address_blocks`, the two error enums |
| `partition` | `MasterBootRecord::parse`, `PartitionEntry`, and `scratch_sector_range`, the rule for where a write test may land |
| `bench` | the read-only probe, the scratch write test, the read matrix and `QueueWatch`, which a board bring-up runs |
| `jh7110` | the two controllers' addresses, interrupts and queue depth on that chip, both device trees' compatible strings, and `Slot::from_node` |

No public signature names another crate's type.

What a stranger may find surprising:

- Nine public constants of type `()`, such as `registers::CLOCK_ENABLES_DO_NOT_OVERLAP`, exist only
  to run a `const` assertion. They appear in the documentation as API, and removing one later breaks
  any caller that named it. `const _: () = ...` gives the same check with no public name.
- `bench` is board bring-up code. `run_scratch_write_test` writes to the card (inside the gap before
  the first partition, which a harness proves), and `MATRIX_HZ` and `MatrixRow` serve one
  diagnosis on one board. A stranger driving a controller needs none of it.
- No error enum is `#[non_exhaustive]` (`host_controller::Error`, `Refusal`, `InitializationError`,
  `BlockTransferError`, `ReadOnlyProbeFailure`), and none implements `Display` or
  `core::error::Error`. A new variant after publication breaks every exhaustive `match`.
- Public structs expose public fields (`Card`, `Identity`, `Slot`, `MatrixRow` and others), so a
  field added later breaks callers who build them.

## Proofs and falsification patches

Eight harnesses in `src/proofs.rs`, each with one patch. `script/crate-standalone` proved all eight
on 1.88 and saw each patch fail its own harness.

| harness | what it proves |
|---|---|
| `the_scratch_range_never_touches_a_partition_or_sector_zero` | no partition table makes the write test's range touch a partition or sector 0 |
| `a_queue_step_never_overruns_the_queue_or_the_buffer` | a polled step moves no more than the transfer owes, and a write step no more than the queue has room for, whatever the status register says |
| `an_admitted_range_is_inside_the_card_and_the_address` | a block range the driver admits lies inside the card and inside a 32-bit command address |
| `a_chain_describes_its_bytes_inside_the_region` | a descriptor chain covers exactly the transfer's bytes, each buffer a page or less inside the region, each link inside the chain page |
| `the_master_boot_record_parse_is_total_and_reads_each_entry_from_its_own_bytes` | the MBR parse never panics, accepts exactly the sectors ending in the signature and reads each entry from its own bytes |
| `a_register_field_never_exceeds_its_width` | a field read from a 128-bit register never exceeds its width and never panics |
| `a_card_specific_data_decode_is_total_and_never_overflows` | decoding any specific-data register, as SD or eMMC, sizes the card without overflow |
| `an_extended_card_specific_data_sector_count_is_total` | the eMMC sector count reads from bytes 212 to 215 of any buffer, and a short buffer gives zero |

Not proved, and the README says so: the card bring-up state machine, the timing, and the
identification decode. 99 host tests drive those against the simulator.

## Names

`python3 helpers/name_release.py --draft designware_mobile_storage` lists 135 names, all ratified:
the crate on #1806 and the items on pull requests #3092 to #3144. Nothing is provisional.

Eight of them, and two parameters, use a unit symbol that the units rule of 2026-11-03 refuses. #3092 ratified them on
2026-11-02 and 2026-11-03, before or beside that rule. The rule leaves a ratified name
alone until its own review, and a release review is that review.

| name | kind | recommendation |
|---|---|---|
| `jh7110::CARD_INTERFACE_UNIT_HZ` | constant | `CARD_INTERFACE_UNIT_HERTZ` |
| `sd::TRANSFER_HZ` | constant | `TRANSFER_HERTZ` |
| `bench::MATRIX_HZ` | constant | `MATRIX_HERTZ`, or private if `bench` leaves the API (question 1) |
| `jh7110::Slot::card_interface_unit_hz` | field | `card_interface_unit_hertz` |
| `sd::Card::clock_hz` | field | `clock_hertz` |
| `bench::MatrixRow::asked_hz`, `card_hz` | fields | `asked_hertz`, `card_hertz` |
| `internal_direct_memory_access::Refusal::Above4GiB` | variant | `AboveFourGibibytes`; an alternative names the cause, `BeyondThirtyTwoBitAddress` |
| `base_hz` (`HostController::new`) and `hz` (`set_clock`) | parameters | `base_hertz`, `hertz`; a parameter name breaks no caller |

## License

`license = "(MIT OR Apache-2.0) AND ISC"`: the crate's own pair, then ISC. The crate adapts
OpenBSD's `sys/dev/fdt/dwmmc.c` revision 1.33 for the register map, reset, clock, bus width,
command flags and the polled transfer. `src/lib.rs` carries that file's copyright line and ISC notice. The card
identification follows only the order of OpenBSD's `sdmmc_mem.c` addressing commands and copies
none of its code. So the crate carries no notice for it, as milestone 1019 (the driver crates carry
facts from their sources, not their text) item 6 ruled. The authors read Linux, U-Boot and the
device trees for hardware facts only. The notice opens with prose, not an `SPDX-License-Identifier:` line, which the
proposal for a license check would add.

## What the move still needs

| item | owner |
|---|---|
| Shipped text names radon 29 times (`src/*.rs`); a stranger does not know the board's nickname. Say "a VisionFive 2". | move lane |
| The manifest comment speaks of "one small `Registers` trait"; the trait is `RegisterWindowAndClock`. | move lane |
| The README does not say how to install Kani. | move lane |
| `repository` in the manifest. | move lane |

Milestone 1019 item 11 (pin each vendor link to the revision its claim holds at) reads as done in
this crate: `jh7110.rs` and `host_controller.rs` now name the vendor tag and the branch head
separately and say which assigns 50 MHz. The milestone block still lists item 11 as outstanding,
for this crate and `jh7110_clock_and_reset`. The maintainer can confirm and close it.

## Proposed items

No defect found in the logic. The two text items above (the stale trait name, radon) are the only
inaccuracies this review read.

## Open questions

1. How much of the crate to publish. Background: `bench` and the nine `()` check constants serve
   this tree's bench and its own build, not a caller. Options: (a) publish as it stands; (b) make
   the nine checks anonymous (`const _: () = ...`) and put `bench` behind a non-default `bench`
   feature; (c) as (b), but move `bench` out of the crate and into the caller. Recommendation: (b).
   It keeps the bench sequence beside the driver it tests, publishes nothing a stranger could
   misread, and a later release can make a hidden item public without breaking anyone. The
   maintainer can rule it provisionally, as for the FAT package's question 2; the architect's
   clearance confirms it.
2. `#[non_exhaustive]` on the five error enums and `ScratchWriteVerdict`. Recommendation: add it
   before 0.1.0, the stricter choice, which a later release can relax (the FAT package's question
   3). The maintainer can rule it provisionally.
3. The unit spelling of the eight names in the table above. Recommendation: rename them all before
   the crate leaves. **architect** (renaming ratified names). One ruling can cover this crate,
   `counter_frequency_contract` and `jh7110_clock_and_reset`.

## Result of `script/crate-standalone`

Clean: manifest, files, citations, names, the build on 1.88 for the host and the three bare-metal
targets, all eight harnesses and all eight falsifications.

## Recommendation

Ready for review once the maintainer rules questions 1 and 2 provisionally and the architect rules
question 3. The move lane's text edits do not hold the review.
