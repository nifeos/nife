# Review package: `designware_mobile_storage`

*Name: provisional, minted by lane lane/crate-review-packages-five. The `designware_mobile_storage`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the five packages
together outgrow the page.*

## In one paragraph

The whole SD and eMMC driver for the Synopsys DesignWare Mobile Storage Host Controller (`DW_mshc`,
Linux's `dw_mmc`) as pure logic. It holds the register map, the command word, the polled
controller sequences and the internal DMA controller's descriptor chains. It identifies the card,
reads and writes blocks, and decodes the card's identification and specific-data registers. The
caller implements one trait, `RegisterWindowAndClock` (read a register, write one, read a
microsecond clock), and every sequence runs on a host against a simulated controller and card. It
is `no_std`, with no allocator, no `unsafe` and no dependency. Nineteen Kani harnesses prove its
parsers and bounds, and that each controller sequence is total against any controller. Every public name has a ratified `Name:` block, and the compile-time checks have no public name.
The board's test steps live in the kernel's storage bench, not here. No decision is open.

## Identity

| item | value |
|---|---|
| crate | `designware_mobile_storage`, ratified on #1806, permanent on first publication |
| version | `0.1.0`; no tag in this clone |
| edition, `rust-version` | 2024, 1.88 |
| license | `(MIT OR Apache-2.0) AND ISC`; `LICENSE-MIT` and `LICENSE-APACHE` at the crate root, the ISC notice in `src/lib.rs` |
| `publish` | `false` |
| size | 6,570 lines in 12 files: about 4,690 of library, 687 of shared tests, 969 of simulator, 220 of Kani harnesses |
| consumers in nife | `kernel`, `components` and `machine_discovery`, by path |

## Public API

Eight public modules and about 100 public items.

| module | what it holds |
|---|---|
| `registers` | the register offsets and bits the driver uses, `queue_offset`, `queue_width_bytes`, `uses_64_bit_direct_memory_access` |
| `command` | `Command`, `Response`, `Data` and the command word encoding |
| `host_controller` | `RegisterWindowAndClock`, `HostController` (reset, clock, send a command, read and write data), `Identity`, `Error`, `clock_divider`, `card_clock_hertz`, `queue_step` |
| `internal_direct_memory_access` | `InternalDirectMemoryAccess`, the `DirectMemoryAccessRegion` trait, `NoDirectMemoryAccess`, `Refusal` |
| `card` | `CardIdentification`, `CardSpecificData`, `extract_bits`, `r1_state` |
| `sd` | `identify`, `read_blocks`, `write_blocks`, `Card`, `CardKind`, `can_address_blocks`, the two error enums |
| `partition` | `MasterBootRecord::parse`, `PartitionEntry`, and `scratch_sector_range`, the rule for where a write test may land |
| `jh7110` | the two controllers' addresses, interrupts and queue depth on that chip, both device trees' compatible strings, and `Slot::from_node` |

No public signature names another crate's type.

What a stranger may find surprising:

- The enums `host_controller::Error`, `Refusal`, `InitializationError`, `BlockTransferError` and
  `CardKind` are `#[non_exhaustive]` (ruled, below), so a new variant after publication
  breaks no `match`. The four error enums implement `Display` and `core::error::Error`; neither
  needed a choice that a later release cannot extend.
- Public structs expose public fields (`Card`, `Identity`, `Slot` and others), so a
  field added later breaks callers who build them. `Identity`, `Slot` and `Card` are values the
  crate returns, and a caller seldom builds one.
- The README is not the crate documentation. `src/lib.rs` carries the module index, the license
  notice and the BUGS list, and five doctests check the examples there.

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
identification decode. 91 host tests drive those against the simulator.

## Names

`python3 helpers/name_release.py --draft designware_mobile_storage` lists 111 rows. 110 read
ratified, and `DirectMemoryAccessRegion::direct_memory_access_address` is provisional.

Against the rulings in force, with the template's checks (`unit_symbols.py`, `spelled_out.py`,
`question_shape.py`, `british_spelling.py`, `register_names.py` and the report-only
`abbreviations.py`, run with `--manifest` over `src/*.rs`) and a read of every name:

| rule | result |
|---|---|
| units spelled out | passes. `_HERTZ`, `_MICROSECONDS` and `_BYTES` throughout. |
| spelled-out words, accepted acronyms only | passes. Every non-public name is spelled out: a register, bit, descriptor or command constant says what it is (`RAW_INTERRUPT_STATUS_REGISTER`, `SD_SEND_OPERATION_CONDITIONS`) and carries its source's spelling in `doc(alias)` (`RINTSTS`, `SD_SEND_OP_COND`), cited at the opening of its doc comment. `crc` is an accepted acronym, so the CRC constants keep it (`RAW_INTERRUPT_DATA_CRC_ERROR`, alias `RINTSTS_DCRC`). |
| register shape (`_REGISTER`, `_MASK`, a size's unit) | passes. Every offset ends `_REGISTER` and every multi-bit constant `_MASK`. |
| address naming | passes. The other `register_names.py` address findings name a card's relative address or a logical block address, which are not memory locations. |
| question shape | passes. |
| British spelling | passes. |
| `id`, `sd`, `crc`, `jh7110` | accepted words and a chip name. |
| aliases of an old nife spelling | none. Every `doc(alias)` is a specification, databook or OpenBSD term. |
| prose acronyms (`prose_acronyms.py`) | fails: 62 acronyms in the README and doc comments are not expanded where first used (`FIFO`, `CSD`, `OCR`, `MMIO`, the command numbers `CMD0` to `ACMD41` and others). |

The public names the rules refused, and where each stands:

| row | name before | rule | now | status |
|---|---|---|---|---|
| 1 | `registers::CONTROL`, `WRITE_PROTECT`, `TRANSFERRED_CARD_BYTE_COUNT`, `TRANSFERRED_HOST_BYTE_COUNT`, `HARDWARE_RESET`, `BUS_MODE` | a register offset ends `_REGISTER` | each with `_REGISTER` appended | done |
| 2 | `command::Response::has_crc` | `crc` was not an accepted acronym | `has_crc` | withdrawn: `crc` is an accepted acronym |
| 3 | `command::Data::block_size` | a size carries its unit | `block_bytes` | done |
| 4 | `sd::CardKind::SdV1`, `SdV2` | `V` abbreviates version | `SdVersion1`, `SdVersion2` | done |
| 5 | `internal_direct_memory_access::DirectMemoryAccessRegion::bus_address` | an address says what kind it is | `direct_memory_access_address`, aliases `bus address` and `dma_addr_t` | done, provisional |
| 6 | `jh7110::Slot::base`, and the `base` parameter of `Slot::from_node` | an address is virtual or physical | `registers_physical_address` | done |
| 7 | `jh7110::Slot::size`, and the `size` parameter of `Slot::from_node` | a size carries its unit | `registers_size_bytes` | done |
| 8 | `host_controller::clock_divider`, parameters `base` and `target` | a quantity carries its unit | `base_hertz`, `target_hertz` | done |
| 9 | `host_controller::card_clock`, and its parameter `base` | a quantity carries its unit | `card_clock_hertz`, `base_hertz` | done |
| 10 | `host_controller::queue_step`, parameters `depth` and `remaining` | a quantity carries its unit | `depth_words`, `remaining_bytes` | done |
| 11 | `card::extract_bits`, parameter `len` | a length carries its unit | `width_bits` | done |
| 12 | `registers::queue_width` | a quantity carries its unit | `queue_width_bytes` | done |
| 13 | `jh7110::QUEUE_DEPTH`, `Slot::queue_depth`, `HostController::queue_depth`, the `queue_depth` parameter of `HostController::new` | a count names the things | `QUEUE_DEPTH_WORDS`, `queue_depth_words` | done |

The two address registers, `CURRENT_DESCRIPTOR_PHYSICAL_ADDRESS_REGISTER` and
`CURRENT_BUFFER_PHYSICAL_ADDRESS_REGISTER`, hold the descriptor address and buffer address the
controller is on, as the bus sees them, which is physical on every board this driver runs on.
`DirectMemoryAccessRegion::direct_memory_access_address` is the region's address as the controller's DMA engine
uses it, which differs from the CPU physical address behind an IOMMU or a bus offset, so it does
not take `physical_address`. It is `direct_memory_access_address`, provisionally, with
`bus address` and `dma_addr_t` as aliases; the maintainer ruled it, and it is a two-way door
before the first release.
`Refusal::AboveFourGibibytes` spells its unit out. The parameters of `registers::queue_offset`,
`queue_width_bytes` and `uses_64_bit_direct_memory_access` are `version_identification` and
`hardware_configuration`, and `jh7110::Slot::from_node` takes `property`. The renamed items carry
the old spelling and the rule that refused it in their `Name:` blocks, and none carries a
`doc(alias)` of it.

## License

`license = "(MIT OR Apache-2.0) AND ISC"`: the crate's own pair, then ISC. The crate adapts
OpenBSD's `sys/dev/fdt/dwmmc.c` revision 1.33 for the register map, reset, clock, bus width,
command flags and the polled transfer. `src/lib.rs` carries that file's copyright line and ISC notice. The card
identification follows only the order of OpenBSD's `sdmmc_mem.c` addressing commands and copies
none of its code. So the crate carries no notice for it, as milestone 1019 (the driver crates carry
facts from their sources, not their text) item 6 ruled. The authors read Linux, U-Boot and the
device trees for hardware facts only. The notice opens with prose, not an `SPDX-License-Identifier:` line, which the
proposal for a license check would add.

## Rulings on the API

`#[non_exhaustive]` marks the four error enums and `CardKind`. It is the restrictive option:
relaxing it in a later release breaks nobody, and adding it after publication breaks every
exhaustive `match`. `CardKind` is on the list because its own comment already names SDUC as a
future kind. `Response` stays exhaustive, since the specification fixes its seven values. The
scratch write verdict, `ScratchWriteVerdict`, lives in the kernel's storage bench and not in this
crate.

## Outstanding before the move

| item | owner |
|---|---|
| `repository` in the manifest, and `publish = false` with its nife-tree comment; the template owns both. | move lane |
| The `Name:` blocks cite pull requests of this tree; the move commit strips them, as ruled. | move lane |
| Milestone 1019 item 11 (pin each vendor link to its revision) reads as done in this crate: `jh7110.rs` and `host_controller.rs` name the vendor tag and the branch head separately and say which assigns 50 MHz. The block still lists it as outstanding. | maintainer |

No shipped text names this tree, a milestone number or a lane. The board is "a VisionFive 2"
throughout.

## Proposed items

No defect found in the logic.

## Result of `script/crate-standalone`

Clean: manifest, files, citations, names, the build on 1.88 for the host and
the three bare-metal targets, all eight harnesses and all eight falsifications. The crate's 91
host tests and five doctests pass, `cargo clippy` reports nothing, and
`machine_discovery`, `driver_process_contract`, `components` and the kernel (with `storage_bench`)
still build.

## Recommendation

Ready for review.
