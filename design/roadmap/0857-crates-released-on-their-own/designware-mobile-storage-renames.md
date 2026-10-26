# The renames in `designware_mobile_storage` before ratification

An appendix of [milestone 857 (crates released on their
own)](../0857-crates-released-on-their-own.md), beside the crate's names record,
[designware-mobile-storage-names.md](designware-mobile-storage-names.md). Each ruling below renamed
names in the crate before it publishes; the record's row for each new name says which ruling renamed
it and what it was.

*Name: provisional. Named for the record it sits beside.*

- Ruled 2026-10-26 (UTC): every `regs` module this tree names becomes `registers`, and the
  constants inside keep the databook's names; a `reg` or a one-letter `r` that holds a register or
  the register window follows. `dma` becomes `direct_memory_access` wherever this tree
  authors it: the host's region, the `issue` argument, `Error::DirectMemoryAccess`,
  `fifoth_for_direct_memory_access`, `direct_memory_access_interface` and the simulator's knobs.
  Names copied from the databook (`CTRL_DMA_RESET`, `CTRL_USE_INTERNAL_DMAC`) keep
  their spelling.
- Ruled 2026-10-26 (UTC): `init` meaning initialize takes its full spelling wherever this tree
  authors it. `InitError` becomes `InitializationError` and the bench's `ReadFailure::Init` becomes
  `ReadFailure::Initialization`.
- Ruled 2026-10-26 (UTC): `max` spells out `maximum` and `min` `minimum` tree-wide, and the naming
  rules apply to function parameters as well as public items. `Watch` and `MatrixRow`'s `max_fill`
  become `maximum_fill`, `identify`'s `max_bus_width` becomes `maximum_bus_width`, and the chain
  limit becomes `MAXIMUM_DESCRIPTORS`. The parameters `cmd`, `div`, `off`, `v`, `src`, `addr` and
  `r` (the bench's `ReadReport`) become `command`, `divider`, `offset`, `value`, `source`, `address`
  and `report`. Rust's own `u32::MAX`, `min` and `max` stay, and so do the unit symbols `hz`, `us`
  and `ms`.
- Ruled 2026-10-26 (UTC), tree-wide: `fifo` becomes `queue` in every name this tree authors, with
  `doc(alias = "fifo")`, and copied register names (`FIFOTH`, `CTRL_FIFO_RESET`) keep their
  spelling. `fifo_offset`, `fifo_width`, `fifo_step`, `fifo_depth`, `wait_fifo`, `Error::FifoWidth`
  and `FIFO_DEPTH` become `queue_offset`, `queue_width`, `queue_step`, `queue_depth`, `wait_queue`,
  `Error::QueueWidth` and `QUEUE_DEPTH`. `Slot` and the handoff's `fifo_depth` fields become
  `queue_depth`; the device tree's `fifo-depth` property keeps its spelling. The functions named for
  `FIFOTH` keep it.
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
- Ruled 2026-10-26 (UTC), the SD and MMC vocabulary: `sd` has earned its place and stays (the
  module, `SdV1`, `SdV2`, `SdHighCapacity`, `configure_sd`); `mmc` becomes `multimedia_card`
  (`Kind::Mmc`, `configure_mmc`, `mmc_power_up`, the `mmc` decoders, the `MMC_*` constants this
  crate authors). The card registers spell out, each with the specification's abbreviation as its
  alias. `Cid` and `cid_raw` become `CardIdentification` and `card_identification_raw`, and `Csd`
  and `csd_raw` become `CardSpecificData` and `card_specific_data_raw`. `ext_csd` becomes
  `extended_card_specific_data` and `ocr` becomes `operation_conditions`. `rca` and `rca_arg` become
  `relative_card_address` and `relative_card_address_argument`, `IF_COND_ARG` becomes
  `INTERFACE_CONDITION_ARGUMENT` and `UHS` becomes `ULTRA_HIGH_SPEED`. `raw` stays. The partition
  table's `Mbr` becomes `MasterBootRecord` and `TYPE_PROTECTIVE_GPT` becomes
  `TYPE_PROTECTIVE_GLOBALLY_UNIQUE_IDENTIFIER_PARTITION_TABLE`. The device tree's `mmc@` node names
  and `starfive,jh7110-mmc` keep their spelling.
- Ruled 2026-10-26 (UTC): a register or bit name copied from a cited source keeps that source's
  spelling strictly, and a name that only resembles it takes its spelling. Checked against
  OpenBSD's `dwmmc.c`, the bits that drifted take its spelling: `CMD_START` becomes
  `CMD_START_CMD`, `CMD_SEND_INIT` becomes `CMD_SEND_INITIALIZATION`, the other command bits,
  the `CLKENA`, `BMOD` and `CARDTHRCTL` bits follow, and the `INT_*` bits become `RINTSTS_*`.
  `VERID_240A` becomes Linux's `DW_MMC_240A`. Of the commands, `SD_SET_BUS_WIDTH` becomes the
  specification's `SET_BUS_WIDTH`, and `MMC_SEND_OP_COND`, which matches OpenBSD and Linux exactly,
  is re-sourced to JESD84-B51's `SEND_OP_COND` so that every command takes the specifications'
  unprefixed spelling. The `OCR_*`
  constants copy no source's name, so they follow the normal rules as `OPERATION_CONDITIONS_*`.
- Ruled 2026-10-26 (UTC), tree-wide: this tree spells out `rx` and `tx` wherever it authors the
  name. `fifoth_rx_watermark` becomes `fifoth_receive_watermark`, and the watermark locals in the
  `FIFOTH` functions follow.
- Ruled 2026-10-26 (UTC): a name kept because it copies a source carries its expansion in its
  documentation. Each register, bit and descriptor constant's doc opens with the databook's full
  name and the name it copies, and carries a plain-English `doc(alias)`. Each command's doc opens
  with its number and full name, and each `Response` kind's with what it carries and the
  specifications' sections.

Picks, pending confirmation:

- `dma_64bit` becomes `uses_64_bit_direct_memory_access`, so the function reads as the question it
  answers.
- `InitError` becomes `InitializationError`, the noun of the ruled verb.
- `fifo_count` becomes `queue_count`, not a copy of OpenBSD's `FIFO_COUNT`; the host's offset of the
  data queue is `queue_offset` and the simulator's queue is `queue`.
- `STATUS_FIFO_FULL` keeps its spelling: it is the databook's `fifo_full`, which `dwmmc-host` keeps.
- `len` stays, with the unit symbols.
- The simulator's `addr` parameter becomes `address`.
- The tests' channel ends become `sender` and `receiver`, the names of the standard library's own
  types.
