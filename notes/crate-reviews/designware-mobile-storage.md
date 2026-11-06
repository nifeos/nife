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
is `no_std`, with no allocator, no `unsafe` and no dependency. Eight Kani harnesses prove its
parsers and bounds. Every public name is ratified. The board's bench test sits behind the
non-default `bench` feature, and the compile-time checks have no public name. What the architect
decides is one unit spelled inside a variant name, whether `bench` ships in 0.1.0, and the
ratified names that a later rule reaches.

## Identity

| item | value |
|---|---|
| crate | `designware_mobile_storage`, ratified on #1806, permanent on first publication |
| version | `0.1.0`; no tag in this clone |
| edition, `rust-version` | 2024, 1.88 |
| license | `(MIT OR Apache-2.0) AND ISC`; `LICENSE-MIT` and `LICENSE-APACHE` at the crate root, the ISC notice in `src/lib.rs` |
| `publish` | `false` |
| size | 7,254 lines in 13 files: about 5,150 of library, 911 of shared tests, 969 of simulator, 220 of Kani harnesses |
| consumers in nife | `kernel`, `components` and `machine_discovery`, by path |

## Public API

Eight public modules, a ninth (`bench`) behind the `bench` feature, and about 115 public items.

| module | what it holds |
|---|---|
| `registers` | the register offsets and bits the driver uses, `queue_offset`, `queue_width`, `uses_64_bit_direct_memory_access` |
| `command` | `Command`, `Response`, `Data` and the command word encoding |
| `host_controller` | `RegisterWindowAndClock`, `HostController` (reset, clock, send a command, read and write data), `Identity`, `Error`, `clock_divider`, `card_clock`, `queue_step` |
| `internal_direct_memory_access` | `InternalDirectMemoryAccess`, the `DirectMemoryAccessRegion` trait, `NoDirectMemoryAccess`, `Refusal` |
| `card` | `CardIdentification`, `CardSpecificData`, `extract_bits`, `r1_state` |
| `sd` | `identify`, `read_blocks`, `write_blocks`, `Card`, `CardKind`, `can_address_blocks`, the two error enums |
| `partition` | `MasterBootRecord::parse`, `PartitionEntry`, and `scratch_sector_range`, the rule for where a write test may land |
| `bench` (feature `bench`) | the read-only probe, the scratch write test, the read matrix and `QueueWatch`, which a board bring-up runs |
| `jh7110` | the two controllers' addresses, interrupts and queue depth on that chip, both device trees' compatible strings, and `Slot::from_node` |

No public signature names another crate's type.

What a stranger may find surprising:

- `bench` is board bring-up code, built only with the `bench` feature (or for the crate's own
  tests). `run_scratch_write_test` writes to the card (inside the gap before the first partition,
  which a harness proves), and `MATRIX_HERTZ` and `MatrixRow` serve one diagnosis on one board. A
  stranger driving a controller needs none of it, and with the feature off none of it is public.
- The enums `host_controller::Error`, `Refusal`, `InitializationError`, `BlockTransferError`,
  `ReadOnlyProbeFailure`, `ScratchWriteVerdict` and `CardKind` are `#[non_exhaustive]`
  (provisional, below), so a new variant after publication breaks no `match`. The four error enums and
  `ReadOnlyProbeFailure` implement `Display` and `core::error::Error`; neither needed a choice
  that a later release cannot extend.
- Public structs expose public fields (`Card`, `Identity`, `Slot`, `MatrixRow` and others), so a
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
identification decode. 99 host tests drive those against the simulator.

## Names

`python3 helpers/name_release.py --draft designware_mobile_storage` lists 130 rows. 129 read
ratified: the crate on #1806 and the items on pull requests #3092 to #3144, #3096, #3101 and #3276. The 130th is the `bench` feature, which has no block. Nothing else is
provisional.

Against the rulings in force, checked with the template's own checks
(`vendor/crate-workflows/helpers/`, run with `--manifest` over `src/*.rs`, 1,561 names judged):

| rule | result |
|---|---|
| units spelled out (`unit_symbols.py`) | 0 findings. `_HERTZ`, `_MICROSECONDS` and `_BYTES` throughout. The checker reads suffixes only, so it does not see `Above4GiB` (below). |
| spelled-out words (`spelled_out.py`) | 0 findings. Six private source-kept constants open their doc comment with `(NAME, source)`, and no test name says `spec`. |
| question shape (`question_shape.py`) | 0 findings. The private wait is `is_condition_met_within`, and the simulator predicates take `is_`. |
| British spelling | 0 findings. |
| `id`, `sd`, `jh7110` | accepted words and a chip name. |
| aliases of an old nife spelling | none. Every `doc(alias)` is a specification or databook term. |

Ratified public names that a later rule reaches. They are not renamed here:

| name | rule | recommended name |
|---|---|---|
| `internal_direct_memory_access::Refusal::Above4GiB` | units are spelled out, in any position | `AboveFourGibibytes` |
| `registers::CURRENT_DESCRIPTOR_ADDRESS_REGISTER` | an address is `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS` in every location | `CURRENT_DESCRIPTOR_PHYSICAL_ADDRESS_REGISTER` |
| `registers::CURRENT_BUFFER_ADDRESS_REGISTER` | the same | `CURRENT_BUFFER_PHYSICAL_ADDRESS_REGISTER` |
| `registers::queue_offset(verid)`, `queue_width(hcon)`, `uses_64_bit_direct_memory_access(hcon)` | a parameter spells out the databook word | `version_identification`, `hardware_configuration` |
| `jh7110::Slot::from_node(.., prop)` | `prop` abbreviates `property` | `property` |

The two address registers hold the descriptor address and buffer address the controller is on, as
the bus sees them, which is physical on every board this driver runs on. A parameter name is not
part of a Rust signature's compatibility, so the parameter renames cost no caller anything.

The private `pub(crate)` constants keep the databook and OpenBSD spellings
(`CONTROL_FIFO_RESET`, `CONTROL_DMA_RESET`, `SEND_OP_COND`) through the kept-name form.

## License

`license = "(MIT OR Apache-2.0) AND ISC"`: the crate's own pair, then ISC. The crate adapts
OpenBSD's `sys/dev/fdt/dwmmc.c` revision 1.33 for the register map, reset, clock, bus width,
command flags and the polled transfer. `src/lib.rs` carries that file's copyright line and ISC notice. The card
identification follows only the order of OpenBSD's `sdmmc_mem.c` addressing commands and copies
none of its code. So the crate carries no notice for it, as milestone 1019 (the driver crates carry
facts from their sources, not their text) item 6 ruled. The authors read Linux, U-Boot and the
device trees for hardware facts only. The notice opens with prose, not an `SPDX-License-Identifier:` line, which the
proposal for a license check would add.

## Provisional rulings by the maintainer

`#[non_exhaustive]` is applied to the five error enums, `ScratchWriteVerdict` and `CardKind`. It is
the restrictive option: relaxing it in a later release breaks nobody, and adding it after
publication breaks every exhaustive `match`. `CardKind` joins the list because its own comment
already names SDUC as a future kind. `Response` stays exhaustive, since the specification fixes
its seven values. The kernel's storage bench matches `ReadOnlyProbeFailure` with a wildcard arm, the
one `match` in this tree that needed it. The architect's clearance of the API confirms or reverses
this.

## Outstanding before the move

| item | owner |
|---|---|
| `repository` in the manifest, and `publish = false` with its nife-tree comment; the template owns both. | move lane |
| The `Name:` blocks cite pull requests of this tree; the move commit strips them, as ruled. | move lane |
| `bench` stays or leaves (decision 1). If it leaves, its 448 lines, its tests and `STATUS_FIFO_FULL` go with it into the kernel's storage bench. | maintainer, after the ruling |
| Milestone 1019 item 11 (pin each vendor link to its revision) reads as done in this crate: `jh7110.rs` and `host_controller.rs` name the vendor tag and the branch head separately and say which assigns 50 MHz. The block still lists it as outstanding. | maintainer |

No shipped text names this tree, a milestone number or a lane. The board is "a VisionFive 2"
throughout.

## Proposed items

No defect found in the logic.

## Result of `script/crate-standalone`

Clean: manifest, files, citations, names, the build on 1.88 for the host and
the three bare-metal targets, all eight harnesses and all eight falsifications. The crate's 100
host tests pass, `cargo clippy` reports nothing, and
`machine_discovery`, `driver_process_contract`, `components` and the kernel (with `storage_bench`)
still build.

## Recommendation

Ready for the architect's review. Five decisions below remain, and none of them blocks reading the crate.

## Decisions for the architect

1. `bench` leaves 0.1.0: recommend moving it into the kernel's storage bench. Background: it is
   board bring-up code, and adding a feature later is compatible while removing one is not.
2. Rename `Refusal::Above4GiB` to `AboveFourGibibytes`: recommend yes. Background: the units rule
   bans symbols, and the checker only reads suffixes, so a unit inside a name slipped through.
3. Rename the two address registers to say `PHYSICAL`: recommend yes. Background: the naming rule
   for addresses postdates their ratification, and these two hold bus addresses.
4. Spell out the parameters `verid`, `hcon` and `prop`: recommend yes. Background: a later rule
   forbids databook abbreviations in parameters, and a parameter rename breaks no caller.
5. Confirm the maintainer's `#[non_exhaustive]` set, `CardKind` included: recommend confirm.
   Background: the restrictive option can be relaxed later and cannot be added later.
