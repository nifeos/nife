# The names in `e1000e`, for ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for the crate of milestone 815 (proven Intel e1000e ring logic, released on its own). A release-track crate's names take their ratification before it leaves this tree (ruled 2026-10-25 UTC). This file lists every name a stranger meets in it, in the record format `helpers/name_release.py` reads.

This file ratifies nothing. Until the line below carries a date, the crate's own `Name:` blocks stay the record, and the status column copies them as of 2026-10-26 (UTC). `python3 helpers/name_release.py --draft e1000e` prints a fresh copy. At ratification each row's status takes the ruling and any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

145 names: 138 no block, 6 provisional, 1 ratified. A status of `unrecorded` means the crate holds no block for the name. The column `outside` says whether a caller outside the crate can reach the name; 13 sit in private modules without a re-export. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them.

The last column reads each name against design/naming.md's tests and 857's Fork 5, which asks for names that stand on their own because the crate may outlive nife. 21 rows say something other than `clear` or `term`. This file renames nothing.

- `clear`: a stranger reads it right.
- `term`: the specification or databook owns it, and it keeps that spelling, as design/naming.md asks of a name across somebody else's interface.
- `nife`: it names nife's own process plan (address-space windows, the handoff a server receives). A stranger has no such plan, and the crate may shed the name before it leaves.
- `scope`: it belongs to something other than the crate's subject.
- `generic`: a word that could name almost anything, design/naming.md's second failure mode.
- `unclear`: what a stranger would trip on, said in the cell.

| name | kind | path | status | outside | a stranger reads it |
|---|---|---|---|---|---|
| `e1000e` | crate | `src/lib.rs` | ratified 2026-10-07. | yes | clear |
| `control` | module | `src/control.rs` | provisional. | no | clear |
| `data_plane` | module | `src/data_plane.rs` | provisional. | no | clear |
| `descriptor` | module | `src/descriptor.rs` | provisional. | no | clear |
| `device` | module | `src/device.rs` | provisional. | no | clear |
| `handoff` | module | `src/handoff.rs` | provisional. | no | nife: nife's own process plan |
| `layout` | module | `src/layout.rs` | unrecorded. | yes | nife: nife's own process plan |
| `flush` | module | `src/pch/flush.rs` | unrecorded. | yes | clear |
| `hardware` | module | `src/pch/hardware.rs` | provisional. | no | clear |
| `pch` | module | `src/pch/mod.rs` | unrecorded. | yes | term |
| `nvm` | module | `src/pch/nvm.rs` | unrecorded. | yes | clear |
| `phy` | module | `src/pch/phy.rs` | unrecorded. | yes | clear |
| `regs` | module | `src/pch/regs.rs` | unrecorded. | yes | clear |
| `reset` | module | `src/pch/reset.rs` | unrecorded. | yes | clear |
| `sequence` | module | `src/pch/sequence.rs` | unrecorded. | yes | clear |
| `sim` | module | `src/pch/sim.rs` | unrecorded. | no | clear; private |
| `tests` | module | `src/pch/tests.rs` | unrecorded. | no | clear; private |
| `ulp` | module | `src/pch/ulp.rs` | unrecorded. | yes | unclear: Intel's ultra low power, an abbreviation |
| `proofs` | module | `src/proofs.rs` | unrecorded. | no | clear; private |
| `regs` | module | `src/regs.rs` | unrecorded. | yes | clear |
| `tests` | module | `src/tests.rs` | unrecorded. | no | clear; private |
| `CTRL_ASDE` | item | `src/control.rs` | unrecorded. | yes | clear |
| `CTRL_EXT_DRV_LOAD` | item | `src/control.rs` | unrecorded. | yes | clear |
| `CTRL_RST` | item | `src/control.rs` | unrecorded. | yes | clear |
| `CTRL_SLU` | item | `src/control.rs` | unrecorded. | yes | clear |
| `RAH_AV` | item | `src/control.rs` | unrecorded. | yes | clear |
| `RFCTL_EXTEN` | item | `src/control.rs` | unrecorded. | yes | clear |
| `STATUS_LU` | item | `src/control.rs` | unrecorded. | yes | clear |
| `mac_from_receive_address` | item | `src/control.rs` | unrecorded. | yes | clear |
| `rctl` | item | `src/control.rs` | unrecorded. | yes | term |
| `tctl` | item | `src/control.rs` | unrecorded. | yes | term |
| `tipg` | item | `src/control.rs` | unrecorded. | yes | term |
| `DataPlane` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `Region` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `Tails` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `TxError` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `can_transmit` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `new` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `receive` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `rx_cursor` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `start` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `transmit` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `BUFFER_SIZE` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `MAX_FRAME` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RX_ENTRIES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RX_ERRORS_FRAME` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RX_QUEUE_PAGE` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RX_STATUS_DD` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RX_STATUS_EOP` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RxVerdict` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TAIL_IN_PAGE` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TX_COMMAND` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TX_ENTRIES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TX_QUEUE_PAGE` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TX_STATUS_DD` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `rx_verdict` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `tx_word` | item | `src/descriptor.rs` | unrecorded. | yes | unclear: which word of a transmit descriptor |
| `DEVICE_IDS` | item | `src/device.rs` | unrecorded. | yes | clear |
| `VENDOR_INTEL` | item | `src/device.rs` | unrecorded. | yes | clear |
| `is_supported` | item | `src/device.rs` | unrecorded. | yes | clear |
| `model` | item | `src/device.rs` | unrecorded. | yes | clear |
| `Handoff` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `ROLE_TAG` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `is_role` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `pack` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `unpack` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `BYTES` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `PAGES` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `RX_BUFFERS` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `RX_RING` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `TX_BUFFERS` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `TX_RING` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `rx_buffer` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `rx_descriptor` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `tx_buffer` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `tx_descriptor` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `DESC_RING_STATUS` | item | `src/pch/flush.rs` | unrecorded. | yes | clear |
| `FEXTNVM11_DISABLE_MULR_FIX` | item | `src/pch/flush.rs` | unrecorded. | yes | clear |
| `REQUIRED` | item | `src/pch/flush.rs` | unrecorded. | yes | generic |
| `required` | item | `src/pch/flush.rs` | unrecorded. | yes | generic |
| `rxdctl` | item | `src/pch/flush.rs` | unrecorded. | yes | term |
| `tail_after` | item | `src/pch/flush.rs` | unrecorded. | yes | clear |
| `tx_word` | item | `src/pch/flush.rs` | unrecorded. | yes | unclear: which word of a transmit descriptor |
| `Hw` | item | `src/pch/hardware.rs` | unrecorded. | yes | unclear: an abbreviation of hardware |
| `is_pch` | item | `src/pch/hardware.rs` | unrecorded. | yes | term |
| `DONE` | item | `src/pch/nvm.rs` | unrecorded. | yes | clear |
| `ERROR` | item | `src/pch/nvm.rs` | unrecorded. | yes | clear |
| `Error` | item | `src/pch/nvm.rs` | unrecorded. | yes | clear |
| `GO` | item | `src/pch/nvm.rs` | unrecorded. | yes | clear |
| `WORDS` | item | `src/pch/nvm.rs` | unrecorded. | yes | clear |
| `read` | item | `src/pch/nvm.rs` | unrecorded. | yes | clear |
| `CONTROL` | item | `src/pch/phy.rs` | unrecorded. | yes | clear |
| `ERROR` | item | `src/pch/phy.rs` | unrecorded. | yes | clear |
| `Error` | item | `src/pch/phy.rs` | unrecorded. | yes | clear |
| `KMRN_MODE_CTRL` | item | `src/pch/phy.rs` | unrecorded. | yes | clear |
| `POLL_US` | item | `src/pch/phy.rs` | unrecorded. | yes | clear |
| `Plan` | item | `src/pch/phy.rs` | unrecorded. | yes | generic |
| `READY` | item | `src/pch/phy.rs` | unrecorded. | yes | clear |
| `Swflag` | item | `src/pch/phy.rs` | unrecorded. | yes | term |
| `id_at` | item | `src/pch/phy.rs` | unrecorded. | yes | unclear: what id, at what |
| `plan` | item | `src/pch/phy.rs` | unrecorded. | yes | generic |
| `read` | item | `src/pch/phy.rs` | unrecorded. | yes | clear |
| `read_mdic` | item | `src/pch/phy.rs` | unrecorded. | yes | term |
| `reg` | item | `src/pch/phy.rs` | unrecorded. | yes | unclear: an abbreviation |
| `release` | item | `src/pch/phy.rs` | unrecorded. | yes | clear |
| `update` | item | `src/pch/phy.rs` | unrecorded. | yes | generic |
| `write` | item | `src/pch/phy.rs` | unrecorded. | yes | clear |
| `FEXTNVM11` | item | `src/pch/regs.rs` | unrecorded. | yes | term |
| `RXDCTL` | item | `src/pch/regs.rs` | unrecorded. | yes | term |
| `Error` | item | `src/pch/sequence.rs` | unrecorded. | yes | clear |
| `Note` | item | `src/pch/sequence.rs` | unrecorded. | yes | generic |
| `Reached` | item | `src/pch/sequence.rs` | unrecorded. | yes | generic |
| `global_reset` | item | `src/pch/sequence.rs` | unrecorded. | yes | clear |
| `init_hw` | item | `src/pch/sequence.rs` | unrecorded. | yes | unclear: `hw` abbreviates hardware |
| `init_phy_workarounds` | item | `src/pch/sequence.rs` | unrecorded. | yes | clear |
| `transmit_errata` | item | `src/pch/sequence.rs` | unrecorded. | yes | clear |
| `Sim` | item | `src/pch/sim.rs` | unrecorded. | no | clear |
| `Start` | item | `src/pch/sim.rs` | unrecorded. | no | clear |
| `new` | item | `src/pch/sim.rs` | unrecorded. | no | clear |
| `done` | item | `src/pch/ulp.rs` | unrecorded. | yes | clear |
| `release` | item | `src/pch/ulp.rs` | unrecorded. | yes | clear |
| `request` | item | `src/pch/ulp.rs` | unrecorded. | yes | clear |
| `CTRL` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CTRL_EXT` | item | `src/regs.rs` | unrecorded. | yes | term |
| `ICR` | item | `src/regs.rs` | unrecorded. | yes | term |
| `IMC` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MTA` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MTA_DWORDS` | item | `src/regs.rs` | unrecorded. | yes | term |
| `RAH0` | item | `src/regs.rs` | unrecorded. | yes | term |
| `RAL0` | item | `src/regs.rs` | unrecorded. | yes | term |
| `RCTL` | item | `src/regs.rs` | unrecorded. | yes | term |
| `RDBAH` | item | `src/regs.rs` | unrecorded. | yes | term |
| `RDBAL` | item | `src/regs.rs` | unrecorded. | yes | term |
| `RDH` | item | `src/regs.rs` | unrecorded. | yes | term |
| `RDLEN` | item | `src/regs.rs` | unrecorded. | yes | term |
| `RDT` | item | `src/regs.rs` | unrecorded. | yes | term |
| `RFCTL` | item | `src/regs.rs` | unrecorded. | yes | term |
| `STATUS` | item | `src/regs.rs` | unrecorded. | yes | term |
| `TCTL` | item | `src/regs.rs` | unrecorded. | yes | term |
| `TDBAH` | item | `src/regs.rs` | unrecorded. | yes | term |
| `TDBAL` | item | `src/regs.rs` | unrecorded. | yes | term |
| `TDH` | item | `src/regs.rs` | unrecorded. | yes | term |
| `TDLEN` | item | `src/regs.rs` | unrecorded. | yes | term |
| `TDT` | item | `src/regs.rs` | unrecorded. | yes | term |
| `TIPG` | item | `src/regs.rs` | unrecorded. | yes | term |

Every `pub` module and item in the crate's source has a row. `script/names` does not count these as names of their own, so they have none:

- the inline modules `status` and `control` in `src/pch/nvm.rs`, and `mdic` and `r` in `src/pch/phy.rs`, whose constants have rows;
- the fields of `DataPlane` (1), `Handoff` (2), `phy::Plan` (3) and the private `Sim` (22);
- the variants of `TxError` (2), `RxVerdict` (3), `nvm::Error` (5), `phy::Error` (7), `sequence::Error` (6), `Reached` (4), `Note` (12) and the private `Start` (3);
- the methods of the traits `Region` (4), `Tails` (3) and `Hw` (6).

The `pub use` lines re-export items that have rows where they are defined. The one field of the crate-private `nvm::Geometry` is reachable from nowhere outside.
