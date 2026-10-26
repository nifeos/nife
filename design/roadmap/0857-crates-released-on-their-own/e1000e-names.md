# The names in `e1000e`, for ratification

<!-- prose-budget: exception. Marked 2026-10-26 (UTC) by lane e1000e-names, at 4,220 words. Reason: most of the file is the table, and each status cell copies a Name: block from the crate as this record's format asks; the rulings of 2026-10-26 put a block on every renamed item, which took the table past the cap of §212 (a prose budget of 3,000 words). The prose around the table stays under it. -->

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for the crate of milestone 815 (proven Intel e1000e ring logic, released on its own). A release-track crate's names take their ratification before it leaves this tree (ruled 2026-10-25 UTC). This file lists every name a stranger meets in it, in the record format `helpers/name_release.py` reads.

This file ratifies nothing. Until the line below carries a date, the crate's own `Name:` blocks stay the record, and the status column copies them as of 2026-10-26 (UTC). `python3 helpers/name_release.py --draft e1000e` prints a fresh copy. At ratification each row's status takes the ruling and any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

The rulings that have reached this crate so far, each performed in its own blocks, which the rows
below copy:

- Ruled 2026-10-26 (UTC), the crate's own names: `Hw` becomes `Hardware`, `pch::nvm` becomes
  `pch::non_volatile_memory`, and `Mdi`, `Lcd`, `has_me`, the simulator's `firmware_holds_swflag`,
  `Plan.reg`, `SIG_*`, `LINEAR_ADDR_MASK`, `addr`, `status16` and every `smbus` the crate authors
  are spelled out. `dword`
  gives way to the Rust type it is (`read_u32`, `MTA_ENTRIES`), since a word is 16 bits in Intel's
  documents and 32 in Arm's and RISC-V's. `rctl`, `tctl`, `tipg` and `rxdctl` keep their spelling,
  as do the register and field names copied from Intel's and FreeBSD's sources and the NVM's
  `WORDS` (Intel's 16-bit flash word).
- Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out wherever this tree authors the
  name (`ReceiveVerdict`, `TransmitError`, `receive_descriptor`, `TRANSMIT_ENTRIES`), and register
  and bit names copied from a datasheet (`RDT`, `DD`, `EOP`) keep their spelling.
- Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out `physical_layer_device`, IEEE
  802.3's phrase, wherever this tree authors the name (`pch::physical_layer_device`,
  `PhysicalLayerDeviceResetBlocked`), and not `physical_layer`, which would blur the chip with the
  layer. Register names copied from FreeBSD (`PHY_ID1`, `STATUS.PHYRA`) keep their spelling.
- Ruled 2026-10-26 (UTC), tree-wide: `init` meaning initialize is spelled out `initialize`
  wherever this tree authors the name (`initialize_hardware`,
  `initialize_physical_layer_device_workarounds`). Intel's `STATUS.LAN_INIT_DONE`, the
  `LanInitTimeout` that names it, and FreeBSD's function names in docs keep their spelling.
- Ruled 2026-10-26 (UTC), tree-wide: every `regs` module becomes `registers`, e1000e's two and
  its one-letter inline `r` among them, and the constants inside keep their sources' names.
- Ruled 2026-10-26 (UTC), tree-wide: `min` and `max` are spelled out `minimum` and `maximum`
  (`MAXIMUM_FRAME`), and names copied from a source (`TXDCTL_MAX_TX_DESC_PREFETCH`) keep theirs.
- Ruled 2026-10-26 (UTC), tree-wide: the naming rules apply to function parameters as well as
  public items, so `off`, `v`, `len`, `src`, `i`, `e`, `g`, `n`, `f`, `s` and
  `data_plane_phys` are spelled out where they are parameters. They have no rows here.
- Ruled 2026-10-26 (UTC): a register or bit name copied from a cited source keeps the source's
  spelling, strictly, apart from a prefix the module states once. Each constant was checked against
  FreeBSD's headers; the shortened copies now carry FreeBSD's full names (`PHY_CONTROL`,
  `HV_KMRN_MODE_CTRL`, `PCICFG_DESC_RING_STATUS`, `FLUSH_DESC_REQUIRED` among the public ones).

The spawn handoff (`Handoff` with `pack`, `unpack` and `is_role`, and `ROLE_TAG`) sits in the `e1000e` module of the nife-internal crate `driver_process_contract`, which does not publish (ruled 2026-10-26 UTC). Those names follow nife's ordinary naming and have no row here.

156 names: 96 no block, 35 provisional, 25 ratified. A status of `unrecorded` means the crate holds no block for the name. The column `outside` says whether a caller outside the crate can reach the name; 12 sit in private modules without a re-export. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them.

The last column reads each name against design/naming.md's tests and 857's Fork 5, which asks for names that stand on their own because the crate may outlive nife. 11 rows say something other than `clear` or `term`. This file renames nothing.

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
| `layout` | module | `src/layout.rs` | unrecorded. | yes | nife: nife's own process plan |
| `flush` | module | `src/pch/flush.rs` | unrecorded. | yes | clear |
| `hardware` | module | `src/pch/hardware.rs` | provisional. | no | clear |
| `pch` | module | `src/pch/mod.rs` | unrecorded. | yes | term |
| `non_volatile_memory` | module | `src/pch/non_volatile_memory.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `nvm` (an abbreviation). | yes | clear |
| `physical_layer_device` | module | `src/pch/physical_layer_device.rs` | ratified 2026-10-26 (UTC), in the tree-wide ruling that spells out `phy` (the chip) as `physical_layer_device`, IEEE 802.3's phrase. Refused `phy` (an abbreviation), `physical_layer` (blurs the chip with the layer). | yes | clear |
| `registers` | module | `src/pch/registers.rs` | ratified 2026-10-26 (UTC), in the tree-wide ruling that turns every `regs` module into `registers`, which names e1000e's. The constants inside keep their sources' names. Refused `regs` (an abbreviation). | yes | clear |
| `reset` | module | `src/pch/reset.rs` | unrecorded. | yes | clear |
| `sequence` | module | `src/pch/sequence.rs` | unrecorded. | yes | clear |
| `sim` | module | `src/pch/sim.rs` | unrecorded. | no | clear; private |
| `tests` | module | `src/pch/tests.rs` | unrecorded. | no | clear; private |
| `ulp` | module | `src/pch/ulp.rs` | unrecorded. | yes | unclear: Intel's ultra low power, an abbreviation |
| `proofs` | module | `src/proofs.rs` | unrecorded. | no | clear; private |
| `registers` | module | `src/registers.rs` | ratified 2026-10-26 (UTC), in the tree-wide ruling that turns every `regs` module into `registers`, which names e1000e's. The constants inside keep their sources' names. Refused `regs` (an abbreviation). | yes | clear |
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
| `TransmitError` | item | `src/data_plane.rs` | ratified 2026-10-26 (UTC), in the tree-wide ruling that spells out rx and tx, which names it. Refused `TxError` (tx abbreviates transmit). | yes | clear |
| `can_transmit` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `new` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `receive` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `receive_cursor` | item | `src/data_plane.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `rx_cursor` (rx abbreviates receive). | yes | clear |
| `set_receive_tail` | item | `src/data_plane.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `set_rx_tail` (rx abbreviates receive). | yes | clear |
| `set_transmit_tail` | item | `src/data_plane.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `set_tx_tail` (tx abbreviates transmit). | yes | clear |
| `start` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `transmit` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `BUFFER_SIZE` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `MAXIMUM_FRAME` | item | `src/descriptor.rs` | ratified 2026-10-26 (UTC), in the tree-wide ruling that spells out `max` as `maximum`, which names `MAXIMUM_FRAME` in `designware_ethernet`. Refused `MAX_FRAME` (an abbreviation). | yes | clear |
| `RECEIVE_ENTRIES` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `RX_ENTRIES` (rx abbreviates receive). | yes | clear |
| `RECEIVE_ERRORS_FRAME` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `RX_ERRORS_FRAME` (rx abbreviates receive). | yes | clear |
| `RECEIVE_QUEUE_PAGE` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `RX_QUEUE_PAGE` (rx abbreviates receive). | yes | clear |
| `RECEIVE_STATUS_DD` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `RX_STATUS_DD` (rx abbreviates receive). | yes | clear |
| `RECEIVE_STATUS_EOP` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `RX_STATUS_EOP` (rx abbreviates receive). | yes | clear |
| `ReceiveVerdict` | item | `src/descriptor.rs` | ratified 2026-10-26 (UTC), in the tree-wide ruling that spells out rx and tx, which names it. Refused `RxVerdict` (rx abbreviates receive). | yes | clear |
| `TAIL_IN_PAGE` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TRANSMIT_COMMAND` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `TX_COMMAND` (tx abbreviates transmit). | yes | clear |
| `TRANSMIT_ENTRIES` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `TX_ENTRIES` (tx abbreviates transmit). | yes | clear |
| `TRANSMIT_QUEUE_PAGE` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `TX_QUEUE_PAGE` (tx abbreviates transmit). | yes | clear |
| `TRANSMIT_STATUS_DD` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `TX_STATUS_DD` (tx abbreviates transmit). | yes | clear |
| `receive_verdict` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `rx_verdict` (rx abbreviates receive). | yes | clear |
| `transmit_word` | item | `src/descriptor.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `tx_word` (tx abbreviates transmit). | yes | unclear: which word of a transmit descriptor |
| `DEVICE_IDS` | item | `src/device.rs` | unrecorded. | yes | clear |
| `VENDOR_INTEL` | item | `src/device.rs` | unrecorded. | yes | clear |
| `is_supported` | item | `src/device.rs` | unrecorded. | yes | clear |
| `model` | item | `src/device.rs` | unrecorded. | yes | clear |
| `BYTES` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `PAGES` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `RECEIVE_BUFFERS` | item | `src/layout.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `RX_BUFFERS` (rx abbreviates receive). | yes | clear |
| `RECEIVE_RING` | item | `src/layout.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `RX_RING` (rx abbreviates receive). | yes | clear |
| `TRANSMIT_BUFFERS` | item | `src/layout.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `TX_BUFFERS` (tx abbreviates transmit). | yes | clear |
| `TRANSMIT_RING` | item | `src/layout.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `TX_RING` (tx abbreviates transmit). | yes | clear |
| `receive_buffer` | item | `src/layout.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `rx_buffer` (rx abbreviates receive). | yes | clear |
| `receive_descriptor` | item | `src/layout.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `rx_descriptor` (rx abbreviates receive). | yes | clear |
| `transmit_buffer` | item | `src/layout.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `tx_buffer` (tx abbreviates transmit). | yes | clear |
| `transmit_descriptor` | item | `src/layout.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `tx_descriptor` (tx abbreviates transmit). | yes | clear |
| `FEXTNVM11_DISABLE_MULR_FIX` | item | `src/pch/flush.rs` | unrecorded. | yes | clear |
| `FLUSH_DESC_REQUIRED` | item | `src/pch/flush.rs` | ratified 2026-10-26 (UTC), in the ruling that keeps a register or bit name copied from a cited source, strictly: FreeBSD's `FLUSH_DESC_REQUIRED` (`if_em.h`). Refused `REQUIRED` (a shortened copy). | yes | term |
| `PCICFG_DESC_RING_STATUS` | item | `src/pch/flush.rs` | ratified 2026-10-26 (UTC), in the ruling that keeps a register or bit name copied from a cited source, strictly: FreeBSD's `PCICFG_DESC_RING_STATUS` (`if_em.h`). Refused `DESC_RING_STATUS` (a shortened copy). | yes | term |
| `required` | item | `src/pch/flush.rs` | unrecorded. | yes | generic |
| `rxdctl` | item | `src/pch/flush.rs` | unrecorded. | yes | term |
| `tail_after` | item | `src/pch/flush.rs` | unrecorded. | yes | clear |
| `transmit_word` | item | `src/pch/flush.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` are spelled out; no ruling covers the rest of the name. Refused `tx_word` (tx abbreviates transmit). | yes | unclear: which word of a transmit descriptor |
| `Hardware` | item | `src/pch/hardware.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `Hw` (an abbreviation of hardware). | yes | clear |
| `is_pch` | item | `src/pch/hardware.rs` | unrecorded. | yes | term |
| `DONE` | item | `src/pch/non_volatile_memory.rs` | unrecorded. | yes | clear |
| `ERROR` | item | `src/pch/non_volatile_memory.rs` | unrecorded. | yes | clear |
| `Error` | item | `src/pch/non_volatile_memory.rs` | unrecorded. | yes | clear |
| `GO` | item | `src/pch/non_volatile_memory.rs` | unrecorded. | yes | clear |
| `WORDS` | item | `src/pch/non_volatile_memory.rs` | unrecorded. | yes | clear |
| `read` | item | `src/pch/non_volatile_memory.rs` | unrecorded. | yes | clear |
| `ERROR` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | clear |
| `Error` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | clear |
| `HV_KMRN_MODE_CTRL` | item | `src/pch/physical_layer_device.rs` | ratified 2026-10-26 (UTC), in the ruling that keeps a register or bit name copied from a cited source, strictly: FreeBSD's `HV_KMRN_MODE_CTRL` (`e1000_ich8lan.h`). Refused `KMRN_MODE_CTRL` (a shortened copy). | yes | term |
| `ManagementDataInterface` | item | `src/pch/physical_layer_device.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `Mdi` (an abbreviation of management data interface, the MDI in `MDIC`). | yes | clear |
| `PHY_CONTROL` | item | `src/pch/physical_layer_device.rs` | ratified 2026-10-26 (UTC), in the ruling that keeps a register or bit name copied from a cited source, strictly: FreeBSD's `PHY_CONTROL` (`e1000_defines.h`). Refused `CONTROL` (a shortened copy). | yes | term |
| `POLL_US` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | clear |
| `Plan` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | generic |
| `READY` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | clear |
| `Swflag` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | term |
| `id_at` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | unclear: what id, at what |
| `paged_offset` | item | `src/pch/physical_layer_device.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `regs` and `r` are spelled out `registers`; no ruling covers this function. Refused `reg` (an abbreviation, as in FreeBSD's `PHY_REG`). | yes | clear |
| `physical_layer_device_address` | item | `src/pch/physical_layer_device.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out `physical_layer_device`; no ruling covers the rest of the name. Refused `phy` (an abbreviation, and the field holds the PHY's address, not the PHY). | yes | clear |
| `plan` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | generic |
| `read` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | clear |
| `read_mdic` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | term |
| `register` | item | `src/pch/physical_layer_device.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `reg` (an abbreviation). | yes | clear |
| `registers` | item | `src/pch/physical_layer_device.rs` | ratified 2026-10-26 (UTC), in the tree-wide ruling that turns every `regs` module into `registers`, which names e1000e's one-letter `r`. The constants inside keep their sources' names. Refused `r` (a letter, not a name). | yes | clear |
| `release` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | clear |
| `update` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | generic |
| `write` | item | `src/pch/physical_layer_device.rs` | unrecorded. | yes | clear |
| `FEXTNVM11` | item | `src/pch/registers.rs` | unrecorded. | yes | term |
| `RXDCTL` | item | `src/pch/registers.rs` | unrecorded. | yes | term |
| `BlockedAfterPhysicalLayerDeviceReset` | item | `src/pch/sequence.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out `physical_layer_device`; no ruling covers the rest of the name. Refused `BlockedAfterPhyReset` (an abbreviation). | yes | clear |
| `Error` | item | `src/pch/sequence.rs` | unrecorded. | yes | clear |
| `InSystemManagementBus` | item | `src/pch/sequence.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `InSmbus` (an abbreviation of System Management Bus). | yes | clear |
| `LanConnectedDeviceConfigured` | item | `src/pch/sequence.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `LcdConfigured` (`Lcd` reads as a display). | yes | clear |
| `LanConnectedDeviceNotConfigured` | item | `src/pch/sequence.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `LcdNotConfigured` (as `LcdConfigured`). | yes | clear |
| `NonVolatileMemory` | item | `src/pch/sequence.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `Nvm` (an abbreviation, as the module's `nvm` was). | yes | clear |
| `Note` | item | `src/pch/sequence.rs` | unrecorded. | yes | generic |
| `PhysicalLayerDevice` | item | `src/pch/sequence.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out `physical_layer_device`; no ruling covers the rest of the name. Refused `Phy` (an abbreviation). | yes | clear |
| `PhysicalLayerDeviceInaccessible` | item | `src/pch/sequence.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out `physical_layer_device`; no ruling covers the rest of the name. Refused `PhyInaccessible` (an abbreviation). | yes | clear |
| `PhysicalLayerDeviceReached` | item | `src/pch/sequence.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out `physical_layer_device`; no ruling covers the rest of the name. Refused `PhyReached` (an abbreviation). | yes | clear |
| `PhysicalLayerDeviceResetBlocked` | item | `src/pch/sequence.rs` | ratified 2026-10-26 (UTC), in the tree-wide ruling that spells out `phy` (the chip) as `physical_layer_device`, which names it. Refused `PhyResetBlocked` (an abbreviation). | yes | clear |
| `PhysicalLayerDeviceResetNotAsserted` | item | `src/pch/sequence.rs` | provisional. Ruled 2026-10-26 (UTC), tree-wide: `phy` (the chip) is spelled out `physical_layer_device`; no ruling covers the rest of the name. Refused `PhyResetNotAsserted` (an abbreviation). | yes | clear |
| `Reached` | item | `src/pch/sequence.rs` | unrecorded. | yes | generic |
| `SystemManagementBusFrequencyUnsupported` | item | `src/pch/sequence.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `SmbusFrequencyUnsupported` (an abbreviation of System Management Bus). | yes | clear |
| `global_reset` | item | `src/pch/sequence.rs` | unrecorded. | yes | clear |
| `initialize_hardware` | item | `src/pch/sequence.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names and the tree-wide ruling that spells out `init` as `initialize`, which names it. Refused `init_hw` (`hw` abbreviates hardware), `init_hardware` (`init` abbreviates initialize). | yes | clear |
| `initialize_physical_layer_device_workarounds` | item | `src/pch/sequence.rs` | ratified 2026-10-26 (UTC), in the tree-wide rulings that spell out `phy` (the chip) as `physical_layer_device` and `init` as `initialize`, the second of which names it. Refused `init_phy_workarounds` (both abbreviations). | yes | clear |
| `transmit_errata` | item | `src/pch/sequence.rs` | unrecorded. | yes | clear |
| `Sim` | item | `src/pch/sim.rs` | unrecorded. | no | clear |
| `Start` | item | `src/pch/sim.rs` | unrecorded. | no | clear |
| `new` | item | `src/pch/sim.rs` | unrecorded. | no | clear |
| `done` | item | `src/pch/ulp.rs` | unrecorded. | yes | clear |
| `release` | item | `src/pch/ulp.rs` | unrecorded. | yes | clear |
| `request` | item | `src/pch/ulp.rs` | unrecorded. | yes | clear |
| `CTRL` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CTRL_EXT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `ICR` | item | `src/registers.rs` | unrecorded. | yes | term |
| `IMC` | item | `src/registers.rs` | unrecorded. | yes | term |
| `MTA` | item | `src/registers.rs` | unrecorded. | yes | term |
| `MTA_ENTRIES` | item | `src/registers.rs` | ratified 2026-10-26 (UTC), in the ruling on e1000e's names. Refused `MTA_DWORDS` (a DWORD is 32 bits in Intel's word sizes and not in Arm's or RISC-V's, so the count names what the table holds). | yes | clear |
| `RAH0` | item | `src/registers.rs` | unrecorded. | yes | term |
| `RAL0` | item | `src/registers.rs` | unrecorded. | yes | term |
| `RCTL` | item | `src/registers.rs` | unrecorded. | yes | term |
| `RDBAH` | item | `src/registers.rs` | unrecorded. | yes | term |
| `RDBAL` | item | `src/registers.rs` | unrecorded. | yes | term |
| `RDH` | item | `src/registers.rs` | unrecorded. | yes | term |
| `RDLEN` | item | `src/registers.rs` | unrecorded. | yes | term |
| `RDT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `RFCTL` | item | `src/registers.rs` | unrecorded. | yes | term |
| `STATUS` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TCTL` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TDBAH` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TDBAL` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TDH` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TDLEN` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TDT` | item | `src/registers.rs` | unrecorded. | yes | term |
| `TIPG` | item | `src/registers.rs` | unrecorded. | yes | term |

Every `pub` module and item in the crate's source has a row. `script/names` does not count these as names of their own, so they have none:

- the inline modules `status` and `control` in `src/pch/non_volatile_memory.rs`, and `mdic` in `src/pch/physical_layer_device.rs`, whose constants have rows;
- the fields of `DataPlane` (1), `physical_layer_device::Plan` (3) and the private `Sim` (22);
- the variants of `TransmitError` (2), `ReceiveVerdict` (3), `non_volatile_memory::Error` (5), `physical_layer_device::Error` (7), `sequence::Error` (6), `Reached` (4), `Note` (12) and the private `Start` (3);
- the methods of the traits `Region` (4), `Tails` (3) and `Hardware` (6).

The `pub use` lines re-export items that have rows where they are defined. The one field of the crate-private `non_volatile_memory::Geometry` is reachable from nowhere outside.
