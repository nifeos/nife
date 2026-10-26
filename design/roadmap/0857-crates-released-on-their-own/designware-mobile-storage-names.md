# The names in `designware_mobile_storage`, for ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for the crate of milestone 817 (a proven DesignWare SD and eMMC host, released on its own). A release-track crate's names take their ratification before it leaves this tree (ruled 2026-10-25 UTC). This file lists every name a stranger meets in it, in the record format `helpers/name_release.py` reads.

This file ratifies nothing. Until the line below carries a date, the crate's own `Name:` blocks stay the record, and the status column copies them as of 2026-10-26 (UTC). `python3 helpers/name_release.py --draft designware_mobile_storage` prints a fresh copy. At ratification each row's status takes the ruling and any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

118 names: 82 no block, 35 provisional, 1 ratified. A status of `unrecorded` means the crate holds no block for the name. The column `outside` says whether a caller outside the crate can reach the name; 8 sit in private modules without a re-export. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them.

The last column reads each name against design/naming.md's tests and 857's Fork 5, which asks for names that stand on their own because the crate may outlive nife. 29 rows say something other than `clear` or `term`. This file renames nothing.

- `clear`: a stranger reads it right.
- `term`: the specification or databook owns it, and it keeps that spelling, as design/naming.md asks of a name across somebody else's interface.
- `nife`: it names nife's own process plan (address-space windows, the handoff a server receives). A stranger has no such plan, and the crate may shed the name before it leaves.
- `scope`: it belongs to something other than the crate's subject.
- `generic`: a word that could name almost anything, design/naming.md's second failure mode.
- `unclear`: what a stranger would trip on, said in the cell.

| name | kind | path | status | outside | a stranger reads it |
|---|---|---|---|---|---|
| `designware_mobile_storage` | crate | `src/lib.rs` | ratified 2026-10-07. | yes | clear |
| `bench` | module | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `card` | module | `src/card.rs` | unrecorded. | yes | clear |
| `command` | module | `src/command.rs` | unrecorded. | yes | clear |
| `host` | module | `src/host.rs` | unrecorded. | yes | clear |
| `internal_direct_memory_access` | module | `src/internal_direct_memory_access.rs` | provisional. Ruled 2026-10-26 (UTC): `idmac` and the names built on it spell out `internal_direct_memory_access`, with `doc(alias = "idmac")`; was `idmac`. | yes | clear |
| `jh7110` | module | `src/jh7110.rs` | unrecorded. | yes | scope: one SoC's glue in a host controller crate |
| `partition` | module | `src/partition.rs` | unrecorded. | yes | scope: partitions |
| `proofs` | module | `src/proofs.rs` | unrecorded. | no | clear; private |
| `registers` | module | `src/registers.rs` | provisional. Ruled 2026-10-26 (UTC): every `regs` module this tree names becomes `registers`; was `regs`. | yes | clear |
| `sd` | module | `src/sd.rs` | unrecorded. | yes | term |
| `sim` | module | `src/sim.rs` | unrecorded. | no | clear; private |
| `tests` | module | `src/tests.rs` | unrecorded. | no | clear; private |
| `InternalDirectMemoryAccessReport` | item | `src/bench.rs` | provisional. Ruled 2026-10-26 (UTC): `idmac` and the names built on it spell out `internal_direct_memory_access`, with `doc(alias = "idmac")`; was `IdmacReport`. | yes | scope: a bench |
| `MATRIX_BLOCKS` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `MATRIX_HZ` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `MatrixRow` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `ReadFailure` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `ReadReport` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `SCRATCH_SECTORS` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `Watch` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `WriteVerdict` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `clear` | item | `src/bench.rs` | provisional. | yes | clear |
| `internal_direct_memory_access_read` | item | `src/bench.rs` | provisional. Ruled 2026-10-26 (UTC): `idmac` and the names built on it spell out `internal_direct_memory_access`, with `doc(alias = "idmac")`; was `idmac_read`. | yes | scope: a bench |
| `new` | item | `src/bench.rs` | provisional. | yes | clear |
| `pattern` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `read_matrix` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `read_only` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `scratch_write` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `CardIdentification` | item | `src/card.rs` | provisional. Ruled 2026-10-26 (UTC): the SD and MMC vocabulary this tree authors spells out, with the specification's abbreviation as a `doc(alias)`; was `Cid`. | yes | clear |
| `CardSpecificData` | item | `src/card.rs` | provisional. Ruled 2026-10-26 (UTC): the SD and MMC vocabulary this tree authors spells out, with the specification's abbreviation as a `doc(alias)`; was `Csd`. | yes | clear |
| `R1_ERRORS` | item | `src/card.rs` | unrecorded. | yes | clear |
| `R1_READY_FOR_DATA` | item | `src/card.rs` | unrecorded. | yes | clear |
| `STATE_TRANSFER` | item | `src/card.rs` | unrecorded. | yes | term |
| `bits` | item | `src/card.rs` | unrecorded. | yes | generic |
| `r1_state` | item | `src/card.rs` | unrecorded. | yes | term |
| `sd` | item | `src/card.rs` | unrecorded. | yes | term |
| `Command` | item | `src/command.rs` | unrecorded. | yes | clear |
| `Data` | item | `src/command.rs` | unrecorded. | yes | generic |
| `GO_IDLE_STATE` | item | `src/command.rs` | unrecorded. | yes | clear |
| `Response` | item | `src/command.rs` | unrecorded. | yes | clear |
| `SEND_STATUS` | item | `src/command.rs` | unrecorded. | yes | clear |
| `SWITCH` | item | `src/command.rs` | unrecorded. | yes | clear |
| `bytes` | item | `src/command.rs` | unrecorded. | yes | clear |
| `new` | item | `src/command.rs` | unrecorded. | yes | clear |
| `word` | item | `src/command.rs` | unrecorded. | yes | generic |
| `Error` | item | `src/host.rs` | unrecorded. | yes | clear |
| `Host` | item | `src/host.rs` | unrecorded. | yes | clear |
| `Identity` | item | `src/host.rs` | unrecorded. | yes | clear |
| `Registers` | item | `src/host.rs` | unrecorded. | yes | clear |
| `card_clock` | item | `src/host.rs` | unrecorded. | yes | clear |
| `clock_divider` | item | `src/host.rs` | unrecorded. | yes | clear |
| `command` | item | `src/host.rs` | unrecorded. | yes | clear |
| `queue_depth` | item | `src/host.rs` | provisional. Ruled 2026-10-26 (UTC): `fifo` becomes `queue` in every name this tree authors; was `fifo_depth`. | yes | clear |
| `queue_step` | item | `src/host.rs` | provisional. Ruled 2026-10-26 (UTC): `fifo` becomes `queue` in every name this tree authors; was `fifo_step`. | yes | clear |
| `into_registers` | item | `src/host.rs` | unrecorded. | yes | clear |
| `new` | item | `src/host.rs` | unrecorded. | yes | clear |
| `read` | item | `src/host.rs` | unrecorded. | yes | clear |
| `registers` | item | `src/host.rs` | unrecorded. | yes | clear |
| `reset` | item | `src/host.rs` | unrecorded. | yes | clear |
| `with_internal_direct_memory_access` | item | `src/host.rs` | provisional. Ruled 2026-10-26 (UTC): `idmac` and the names built on it spell out `internal_direct_memory_access`, with `doc(alias = "idmac")`; was `with_idmac`. | yes | clear |
| `write` | item | `src/host.rs` | unrecorded. | yes | clear |
| `DirectMemoryAccessRegion` | item | `src/internal_direct_memory_access.rs` | provisional. | yes | clear |
| `InternalDirectMemoryAccess` | item | `src/internal_direct_memory_access.rs` | provisional. Ruled 2026-10-26 (UTC): `idmac` and the names built on it spell out `internal_direct_memory_access`, with `doc(alias = "idmac")`; was `Idmac`. | yes | clear |
| `NoDirectMemoryAccess` | item | `src/internal_direct_memory_access.rs` | provisional. | yes | clear |
| `Refusal` | item | `src/internal_direct_memory_access.rs` | provisional. | yes | generic |
| `new` | item | `src/internal_direct_memory_access.rs` | provisional. | yes | clear |
| `COMPATIBLE_MAINLINE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `COMPATIBLE_VENDOR` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `QUEUE_DEPTH` | item | `src/jh7110.rs` | provisional. Ruled 2026-10-26 (UTC): `fifo` becomes `queue` in every name this tree authors, and the device tree's `fifo-depth` keeps its spelling; was `FIFO_DEPTH`. | yes | clear |
| `INTERRUPTS` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `NODE_NAMES` | item | `src/jh7110.rs` | provisional. | yes | clear |
| `SDIO0_BASE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `SDIO1_BASE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `Slot` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `WINDOW_SIZE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `from_node` | item | `src/jh7110.rs` | provisional. | yes | clear |
| `role` | item | `src/jh7110.rs` | unrecorded. | yes | nife: nife's own process plan |
| `Entry` | item | `src/partition.rs` | unrecorded. | yes | scope: partitions |
| `MasterBootRecord` | item | `src/partition.rs` | provisional. Ruled 2026-10-26 (UTC): the SD and MMC vocabulary this tree authors spells out, with the specification's abbreviation as a `doc(alias)`; was `Mbr`. | yes | scope: partitions |
| `SIGNATURE` | item | `src/partition.rs` | unrecorded. | yes | scope: partitions |
| `contains` | item | `src/partition.rs` | unrecorded. | yes | clear |
| `is_used` | item | `src/partition.rs` | unrecorded. | yes | scope: partitions |
| `parse` | item | `src/partition.rs` | unrecorded. | yes | clear |
| `scratch` | item | `src/partition.rs` | unrecorded. | yes | scope: partitions |
| `BMOD` | item | `src/registers.rs` | unrecorded. | yes | term |
| `BUFADDR` | item | `src/registers.rs` | provisional. | yes | term |
| `CMD_RESPONSE_EXPECT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CMD_SEND_INITIALIZATION` | item | `src/registers.rs` | provisional. Ruled 2026-10-26 (UTC): a register or bit name copied from a cited source keeps that source's spelling strictly, and a name that only resembles it is corrected to it; was `CMD_SEND_INIT`. | yes | term |
| `CMD_START_CMD` | item | `src/registers.rs` | provisional. Ruled 2026-10-26 (UTC): a register or bit name copied from a cited source keeps that source's spelling strictly, and a name that only resembles it is corrected to it; was `CMD_START`. | yes | term |
| `CTRL` | item | `src/registers.rs` | unrecorded. | yes | term |
| `DSCADDR` | item | `src/registers.rs` | provisional. | yes | term |
| `IDSTS` | item | `src/registers.rs` | provisional. | yes | term |
| `RST_N` | item | `src/registers.rs` | unrecorded. | yes | term |
| `STATUS` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TBBCNT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TCBCNT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `ULTRA_HIGH_SPEED` | item | `src/registers.rs` | provisional. Ruled 2026-10-26 (UTC): the SD and MMC vocabulary this tree authors spells out, with the specification's abbreviation as a `doc(alias)`; was `UHS`. | yes | clear |
| `WINDOW_USED` | item | `src/registers.rs` | unrecorded. | yes | nife: nife's own process plan |
| `WRTPRT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `uses_64_bit_direct_memory_access` | item | `src/registers.rs` | provisional. Ruled 2026-10-26 (UTC): `dma` is spelled out wherever this tree authors it; was `dma_64bit`. | yes | clear |
| `queue_offset` | item | `src/registers.rs` | provisional. Ruled 2026-10-26 (UTC): `fifo` becomes `queue` in every name this tree authors, and copied register names keep `FIFO`; was `fifo_offset`. | yes | clear |
| `queue_width` | item | `src/registers.rs` | provisional. Ruled 2026-10-26 (UTC): `fifo` becomes `queue` in every name this tree authors; was `fifo_width`. | yes | clear |
| `BLOCK` | item | `src/sd.rs` | unrecorded. | yes | clear |
| `Card` | item | `src/sd.rs` | unrecorded. | yes | clear |
| `InitializationError` | item | `src/sd.rs` | provisional. Ruled 2026-10-26 (UTC): `init` meaning initialize is spelled out wherever this tree authors it; was `InitError`. | yes | clear |
| `IoError` | item | `src/sd.rs` | unrecorded. | yes | clear |
| `Kind` | item | `src/sd.rs` | unrecorded. | yes | generic |
| `address` | item | `src/sd.rs` | unrecorded. | yes | clear |
| `identify` | item | `src/sd.rs` | unrecorded. | yes | clear |
| `in_range` | item | `src/sd.rs` | unrecorded. | yes | clear |
| `read_blocks` | item | `src/sd.rs` | unrecorded. | yes | clear |
| `write_blocks` | item | `src/sd.rs` | unrecorded. | yes | clear |
| `Fault` | item | `src/sim.rs` | unrecorded. | no | clear |
| `Sim` | item | `src/sim.rs` | unrecorded. | no | clear |
| `depth` | item | `src/sim.rs` | unrecorded. | no | clear |
| `new` | item | `src/sim.rs` | unrecorded. | no | clear |
| `peek` | item | `src/sim.rs` | unrecorded. | no | clear |

Every `pub` module and item in the crate's source has a row. `script/names` does not count these as names of their own, so they have none:

- the fields of `ReadReport` (7), `InternalDirectMemoryAccessReport` (3), `Watch` (2), `MatrixRow` (8), `CardSpecificData` (2), `CardIdentification` (3), `Data` (3), `Command` (4), `Identity` (6), `Slot` (10), `Entry` (3), `MasterBootRecord` (1) and `Card` (10), and those of the private module `sim`'s types;
- the variants of `ReadFailure` (2), `WriteVerdict` (5), `Response` (7), `host::Error` (12), `Refusal` (5), `Kind` (4), `InitializationError` (9), `IoError` (4) and the private `Fault` (4);
- the methods of the traits `Registers` (3) and `DirectMemoryAccessRegion` (7).

## Rulings applied before ratification

Each renamed row's status says which ruling renamed it and what it was.

- Ruled 2026-10-26 (UTC): every `regs` module this tree names becomes `registers`, and the
  constants inside keep the databook's names; a `reg` or a one-letter `r` that holds a register or
  the register window follows. `dma` is spelled out `direct_memory_access` wherever this tree
  authors it: the host's region, the `issue` argument, `Error::DirectMemoryAccess`,
  `fifoth_for_direct_memory_access`, `direct_memory_access_interface` and the simulator's knobs.
  `dma_64bit` becomes `uses_64_bit_direct_memory_access`, which also reads as the question it
  answers. Names copied from the databook (`CTRL_DMA_RESET`, `CTRL_USE_INTERNAL_DMAC`) keep
  their spelling.
- Ruled 2026-10-26 (UTC): `init` meaning initialize is spelled out wherever this tree authors it.
  `InitError` becomes `InitializationError`, the noun of the ruled verb, and the bench's
  `ReadFailure::Init` becomes `ReadFailure::Initialization`.
- Ruled 2026-10-26 (UTC): `max` spells out `maximum` and `min` `minimum` tree-wide, and the naming
  rules apply to function parameters as well as public items. `Watch` and `MatrixRow`'s `max_fill`
  become `maximum_fill`, `identify`'s `max_bus_width` becomes `maximum_bus_width`, and the chain
  limit becomes `MAXIMUM_DESCRIPTORS`. The parameters `cmd`, `div`, `off`, `v` and `src` become
  `command`, `divider`, `offset`, `value` and `source`. Rust's own `u32::MAX`, `min` and `max`
  stay, and so do `len` and the unit symbols `hz`, `us` and `ms`.
- Ruled 2026-10-26 (UTC), tree-wide: `fifo` becomes `queue` in every name this tree authors, with
  `doc(alias = "fifo")`, and copied register names (`FIFOTH`, `CTRL_FIFO_RESET`,
  `STATUS_FIFO_FULL`) keep their spelling. `fifo_count`, `fifo_offset`, `fifo_width`,
  `fifo_step`, `fifo_depth`, `wait_fifo`, `Error::FifoWidth` and `FIFO_DEPTH` become `queue_count`,
  `queue_offset`, `queue_width`, `queue_step`, `queue_depth`, `wait_queue`, `Error::QueueWidth`
  and `QUEUE_DEPTH`. The host's offset of the data queue is `queue_offset` and the simulator's
  queue is `queue`. `Slot` and the handoff's `fifo_depth` fields become `queue_depth`; the device
  tree's `fifo-depth` property keeps its spelling. The functions named for `FIFOTH` keep it.
- Ruled 2026-10-26 (UTC): `lba` spells out `logical_block_address`, as the earlier ratifications'
  `lba_shift` did. The block functions' and the bench's `lba` arguments take it, and so do the
  simulator's `first_lba` and the window proof in driver_process_contract.
- Ruled 2026-10-26 (UTC), this crate's own names: `idmac` and the names built on it spell out
  `internal_direct_memory_access`, with `doc(alias = "idmac")`: the module, `Idmac`,
  `IdmacReport`, `Refusal::NoIdmac`, `with_idmac` and every `idmac_*` function become
  `InternalDirectMemoryAccess`, `InternalDirectMemoryAccessReport`,
  `Refusal::NoInternalDirectMemoryAccess`, `with_internal_direct_memory_access` and
  `internal_direct_memory_access_*`. `ciu_hz` becomes `card_interface_unit_hz`, `be32`
  `big_endian_u32`, the simulator's `ctrl_resets_cleared` and `bmod_resets_cleared`
  `control_resets_cleared` and `bus_mode_resets_cleared`, and `ReadReport::fs_type`
  `file_system_type`. `arg` becomes `argument` (the `Command` field and its constructors'
  argument) and `buf` becomes `buffer`. `Hz` stays.
- Ruled 2026-10-26 (UTC), the SD and MMC vocabulary: `sd` is earned and stays (the module,
  `SdV1`, `SdV2`, `SdHighCapacity`, `configure_sd`); `mmc` becomes `multimedia_card` (`Kind::Mmc`,
  `configure_mmc`, `mmc_power_up`, the `mmc` decoders, the `MMC_*` constants this crate authors).
  The card registers spell out, each with the specification's abbreviation as its alias. `Cid` and
  `cid_raw` become `CardIdentification` and `card_identification_raw`, and `Csd` and `csd_raw`
  become `CardSpecificData` and `card_specific_data_raw`. `ext_csd` becomes
  `extended_card_specific_data` and `ocr` becomes `operation_conditions`. `rca` and `rca_arg`
  become `relative_card_address` and `relative_card_address_argument`, `IF_COND_ARG` becomes
  `INTERFACE_CONDITION_ARGUMENT` and `UHS` becomes `ULTRA_HIGH_SPEED`. `raw` stays. The partition
  table's `Mbr` becomes `MasterBootRecord` and `TYPE_PROTECTIVE_GPT` becomes
  `TYPE_PROTECTIVE_GLOBALLY_UNIQUE_IDENTIFIER_PARTITION_TABLE`. The device tree's `mmc@` node
  names and `starfive,jh7110-mmc` keep their spelling.
- Ruled 2026-10-26 (UTC): a register or bit name copied from a cited source keeps that source's
  spelling strictly, and a name that only resembles it is corrected to it. Checked against
  OpenBSD's `dwmmc.c`, the bits that drifted take its spelling: `CMD_START` becomes
  `CMD_START_CMD`, `CMD_SEND_INIT` becomes `CMD_SEND_INITIALIZATION`, the other command bits,
  the `CLKENA`, `BMOD` and `CARDTHRCTL` bits follow, and the `INT_*` bits become `RINTSTS_*`.
  `VERID_240A` becomes Linux's `DW_MMC_240A`. Of the commands, `MMC_SEND_OP_COND` and
  `SD_SET_BUS_WIDTH` become the specifications' `SEND_OP_COND` and `SET_BUS_WIDTH`. The `OCR_*`
  constants copy no source's name, so they follow the normal rules as `OPERATION_CONDITIONS_*`.
- Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out wherever this tree authors the
  name. `fifoth_rx_watermark` becomes `fifoth_receive_watermark`, and the watermark locals in the
  `FIFOTH` functions follow. The tests' channel ends become `sender` and `receiver`, the names of
  the standard library's own types.
- Ruled 2026-10-26 (UTC): a name kept because it copies a source carries its expansion in its
  documentation. Each register, bit and descriptor constant's doc opens with the databook's full
  name and the name it copies, and carries a plain-English `doc(alias)`. Each command's doc opens
  with its number and full name, and each `Response` kind's with what it carries and the
  specifications' sections.
