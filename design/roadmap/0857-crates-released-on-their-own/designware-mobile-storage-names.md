# The names in `designware_mobile_storage`, for ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for the crate of milestone 817 (a proven DesignWare SD and eMMC host, released on its own). A release-track crate's names take their ratification before it leaves this tree (ruled 2026-10-25 UTC). This file lists every name a stranger meets in it, in the record format `helpers/name_release.py` reads.

This file ratifies nothing. Until the line below carries a date, the crate's own `Name:` blocks stay the record, and the status column copies them as of 2026-10-26 (UTC). `python3 helpers/name_release.py --draft designware_mobile_storage` prints a fresh copy. At ratification each row's status takes the ruling and any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

118 names: 93 no block, 24 provisional, 1 ratified. A status of `unrecorded` means the crate holds no block for the name. The column `outside` says whether a caller outside the crate can reach the name; 8 sit in private modules without a re-export. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them.

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
| `idmac` | module | `src/idmac.rs` | provisional. | yes | term |
| `jh7110` | module | `src/jh7110.rs` | unrecorded. | yes | scope: one SoC's glue in a host controller crate |
| `partition` | module | `src/partition.rs` | unrecorded. | yes | scope: partitions |
| `proofs` | module | `src/proofs.rs` | unrecorded. | no | clear; private |
| `registers` | module | `src/registers.rs` | provisional. Ruled 2026-10-26 (UTC): every `regs` module this tree names becomes `registers`; was `regs`. | yes | clear |
| `sd` | module | `src/sd.rs` | unrecorded. | yes | term |
| `sim` | module | `src/sim.rs` | unrecorded. | no | clear; private |
| `tests` | module | `src/tests.rs` | unrecorded. | no | clear; private |
| `IdmacReport` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `MATRIX_BLOCKS` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `MATRIX_HZ` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `MatrixRow` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `ReadFailure` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `ReadReport` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `SCRATCH_SECTORS` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `Watch` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `WriteVerdict` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `clear` | item | `src/bench.rs` | provisional. | yes | clear |
| `idmac_read` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `new` | item | `src/bench.rs` | provisional. | yes | clear |
| `pattern` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `read_matrix` | item | `src/bench.rs` | provisional. | yes | scope: a bench |
| `read_only` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `scratch_write` | item | `src/bench.rs` | unrecorded. | yes | scope: a bench |
| `Cid` | item | `src/card.rs` | unrecorded. | yes | term |
| `Csd` | item | `src/card.rs` | unrecorded. | yes | term |
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
| `fifo_depth` | item | `src/host.rs` | unrecorded. | yes | clear |
| `fifo_step` | item | `src/host.rs` | unrecorded. | yes | clear |
| `into_registers` | item | `src/host.rs` | unrecorded. | yes | clear |
| `new` | item | `src/host.rs` | unrecorded. | yes | clear |
| `read` | item | `src/host.rs` | unrecorded. | yes | clear |
| `registers` | item | `src/host.rs` | unrecorded. | yes | clear |
| `reset` | item | `src/host.rs` | unrecorded. | yes | clear |
| `with_idmac` | item | `src/host.rs` | provisional. | yes | term |
| `write` | item | `src/host.rs` | unrecorded. | yes | clear |
| `DirectMemoryAccessRegion` | item | `src/idmac.rs` | provisional. | yes | clear, though `idmac` and `dma_64bit` beside it keep the acronym |
| `Idmac` | item | `src/idmac.rs` | provisional. | yes | term |
| `NoDirectMemoryAccess` | item | `src/idmac.rs` | provisional. | yes | clear, though `idmac` and `dma_64bit` beside it keep the acronym |
| `Refusal` | item | `src/idmac.rs` | provisional. | yes | generic |
| `new` | item | `src/idmac.rs` | provisional. | yes | clear |
| `COMPATIBLE_MAINLINE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `COMPATIBLE_VENDOR` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `FIFO_DEPTH` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `INTERRUPTS` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `NODE_NAMES` | item | `src/jh7110.rs` | provisional. | yes | clear |
| `SDIO0_BASE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `SDIO1_BASE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `Slot` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `WINDOW_SIZE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `from_node` | item | `src/jh7110.rs` | provisional. | yes | clear |
| `role` | item | `src/jh7110.rs` | unrecorded. | yes | nife: nife's own process plan |
| `Entry` | item | `src/partition.rs` | unrecorded. | yes | scope: partitions |
| `Mbr` | item | `src/partition.rs` | unrecorded. | yes | unclear: master boot record is a phrase people say (the acronym test); scope as above |
| `SIGNATURE` | item | `src/partition.rs` | unrecorded. | yes | scope: partitions |
| `contains` | item | `src/partition.rs` | unrecorded. | yes | clear |
| `is_used` | item | `src/partition.rs` | unrecorded. | yes | scope: partitions |
| `parse` | item | `src/partition.rs` | unrecorded. | yes | clear |
| `scratch` | item | `src/partition.rs` | unrecorded. | yes | scope: partitions |
| `BMOD` | item | `src/registers.rs` | unrecorded. | yes | term |
| `BUFADDR` | item | `src/registers.rs` | provisional. | yes | term |
| `CMD_RESPONSE_EXPECT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CMD_SEND_INIT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CMD_START` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CTRL` | item | `src/registers.rs` | unrecorded. | yes | term |
| `DSCADDR` | item | `src/registers.rs` | provisional. | yes | term |
| `IDSTS` | item | `src/registers.rs` | provisional. | yes | term |
| `RST_N` | item | `src/registers.rs` | unrecorded. | yes | term |
| `STATUS` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TBBCNT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TCBCNT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `UHS` | item | `src/registers.rs` | unrecorded. | yes | term |
| `WINDOW_USED` | item | `src/registers.rs` | unrecorded. | yes | nife: nife's own process plan |
| `WRTPRT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `uses_64_bit_direct_memory_access` | item | `src/registers.rs` | provisional. Ruled 2026-10-26 (UTC): `dma` is spelled out wherever this tree authors it; was `dma_64bit`. | yes | clear |
| `fifo_offset` | item | `src/registers.rs` | unrecorded. | yes | clear |
| `fifo_width` | item | `src/registers.rs` | unrecorded. | yes | clear |
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

- the fields of `ReadReport` (7), `IdmacReport` (3), `Watch` (2), `MatrixRow` (8), `Csd` (2), `Cid` (3), `Data` (3), `Command` (4), `Identity` (6), `Slot` (10), `Entry` (3), `Mbr` (1) and `Card` (10), and those of the private module `sim`'s types;
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
