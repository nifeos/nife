# The names in `designware_ethernet`, for ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for the crate of milestone 816 (proven DesignWare Ethernet QoS logic, released on its own). A release-track crate's names take their ratification before it leaves this tree (ruled 2026-10-25 UTC). This file lists every name a stranger meets in it, in the record format `helpers/name_release.py` reads.

This file ratifies nothing. Until the line below carries a date, the crate's own `Name:` blocks stay the record, and the status column copies them as of 2026-10-25 (UTC). `python3 helpers/name_release.py --draft designware_ethernet` prints a fresh copy. At ratification each row's status takes the ruling and any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

154 names: 148 no block, 5 provisional, 1 ratified. A status of `unrecorded` means the crate holds no block for the name. The column `outside` says whether a caller outside the crate can reach the name; 8 sit in private modules without a re-export. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them.

The last column reads each name against design/naming.md's tests and 857's Fork 5, which asks for names that stand on their own because the crate may outlive nife. 33 rows say something other than `clear` or `term`. This file renames nothing.

- `clear`: a stranger reads it right.
- `term`: the specification or databook owns it, and it keeps that spelling, as design/naming.md asks of a name across somebody else's interface.
- `nife`: it names nife's own process plan (address-space windows, the handoff a server receives). A stranger has no such plan, and the crate may shed the name before it leaves.
- `scope`: it belongs to something other than the crate's subject.
- `generic`: a word that could name almost anything, design/naming.md's second failure mode.
- `unclear`: what a stranger would trip on, said in the cell.

| name | kind | path | status | outside | a stranger reads it |
|---|---|---|---|---|---|
| `designware_ethernet` | crate | `src/lib.rs` | ratified 2026-10-07. | yes | clear |
| `coherence` | module | `src/coherence.rs` | unrecorded. | yes | clear: cache coherence for the rings |
| `controller` | module | `src/controller.rs` | unrecorded. | yes | clear |
| `data_plane` | module | `src/data_plane.rs` | provisional. | no | clear |
| `descriptor` | module | `src/descriptor.rs` | provisional. | no | clear |
| `handoff` | module | `src/handoff.rs` | provisional. | no | nife: nife's own process plan |
| `hardware` | module | `src/hardware.rs` | provisional. | no | clear |
| `jh7110` | module | `src/jh7110.rs` | unrecorded. | yes | scope: one SoC's glue in a MAC crate |
| `layout` | module | `src/layout.rs` | unrecorded. | yes | nife: nife's own process plan |
| `mdio` | module | `src/mdio.rs` | unrecorded. | yes | term |
| `motorcomm` | module | `src/motorcomm.rs` | unrecorded. | yes | scope: one vendor's PHY in a MAC crate |
| `process` | module | `src/process.rs` | unrecorded. | yes | nife: nife's own process plan |
| `proofs` | module | `src/proofs.rs` | unrecorded. | no | clear; private |
| `regs` | module | `src/regs.rs` | unrecorded. | yes | clear |
| `sim` | module | `src/sim.rs` | unrecorded. | no | clear; private |
| `tests` | module | `src/tests.rs` | unrecorded. | no | clear; private |
| `Observation` | item | `src/coherence.rs` | unrecorded. | yes | generic |
| `POSTED` | item | `src/coherence.rs` | unrecorded. | yes | generic |
| `Payload` | item | `src/coherence.rs` | unrecorded. | yes | clear |
| `Verdict` | item | `src/coherence.rs` | unrecorded. | yes | generic |
| `WAIT_US` | item | `src/coherence.rs` | unrecorded. | yes | clear |
| `classify` | item | `src/coherence.rs` | unrecorded. | yes | generic |
| `delivered` | item | `src/coherence.rs` | unrecorded. | yes | generic |
| `is_transmit_complete` | item | `src/coherence.rs` | provisional. | yes | clear |
| `observe` | item | `src/coherence.rs` | unrecorded. | yes | generic |
| `prepare` | item | `src/coherence.rs` | unrecorded. | yes | generic |
| `Bus` | item | `src/controller.rs` | unrecorded. | yes | generic: two `Bus` types in two modules |
| `Error` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `Identity` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `configure` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `core_version` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `dma_bits` | item | `src/controller.rs` | unrecorded. | yes | unclear: `dma` and an unstated unit |
| `mac_conf` | item | `src/controller.rs` | unrecorded. | yes | term |
| `read` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `soft_reset` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `start` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `station_address` | item | `src/controller.rs` | unrecorded. | yes | term |
| `stop` | item | `src/controller.rs` | unrecorded. | yes | clear |
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
| `CRC_BYTES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `DESCRIPTOR_BYTES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `MAX_FRAME` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_BUF1V` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_CTXT` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_ES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_FD` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_IOC` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_LD` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_LENGTH` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_OWN` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RX_ENTRIES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RxVerdict` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES2_BUF1_LEN` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES2_IC` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES3_ES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES3_FRAME_LEN` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES3_FS` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES3_LS` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES3_OWN` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TX_ENTRIES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `rx_descriptor_words` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `rx_verdict` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `tx_descriptor_words` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `Handoff` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `ROLE_TAG` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `is_role` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `pack` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `unpack` | item | `src/handoff.rs` | unrecorded. | yes | nife: nife's own process plan |
| `Hw` | item | `src/hardware.rs` | unrecorded. | yes | unclear: an abbreviation of hardware |
| `BUS` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `Discovered` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `GMAC0_BASE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `GMAC0_SYSCON_OFFSET` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `GMAC1_BASE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `Syscon` | item | `src/jh7110.rs` | unrecorded. | yes | term |
| `discover` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `discover_aon_syscon` | item | `src/jh7110.rs` | unrecorded. | yes | term |
| `interface` | item | `src/jh7110.rs` | unrecorded. | yes | generic |
| `select_rgmii` | item | `src/jh7110.rs` | unrecorded. | yes | term |
| `BYTES` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `PAGES` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `rx_descriptor` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `tx_descriptor` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `BMCR_RESET` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `BMSR_ANEG_COMPLETE` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `Bus` | item | `src/mdio.rs` | unrecorded. | yes | generic: two `Bus` types in two modules |
| `COMMAND_LIMIT_US` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `CR_500_800` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `CR_SHIFT` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `Error` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `GB` | item | `src/mdio.rs` | unrecorded. | yes | term |
| `GOC_READ` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `GOC_WRITE` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `Mdio` | item | `src/mdio.rs` | unrecorded. | yes | term |
| `PA_SHIFT` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `RDA_SHIFT` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `command` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `link_is_up` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `phy_id` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `restart_autonegotiation` | item | `src/mdio.rs` | unrecorded. | yes | clear |
| `Applied` | item | `src/motorcomm.rs` | unrecorded. | yes | generic |
| `Drive` | item | `src/motorcomm.rs` | unrecorded. | yes | generic |
| `Link` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `SPECIFIC_STATUS` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `STATUS_DUPLEX` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `STATUS_LINK` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `STATUS_RESOLVED` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `STATUS_SPEED_SHIFT` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `Settings` | item | `src/motorcomm.rs` | unrecorded. | yes | generic |
| `Source` | item | `src/motorcomm.rs` | unrecorded. | yes | generic |
| `Speed` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `YT8531_ID` | item | `src/motorcomm.rs` | unrecorded. | yes | scope: one vendor's PHY in a MAC crate |
| `apply_speed` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `configure` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `link` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `link_from_status` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `mbps` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `read_ext` | item | `src/motorcomm.rs` | unrecorded. | yes | unclear: `ext` abbreviates extended |
| `write_ext` | item | `src/motorcomm.rs` | unrecorded. | yes | unclear: `ext` abbreviates extended |
| `DATA_PLANE_VA` | item | `src/process.rs` | unrecorded. | yes | nife: nife's own process plan |
| `DMA_PAGE_VA` | item | `src/process.rs` | unrecorded. | yes | nife: nife's own process plan |
| `CHAN_CUR_RX_DESC` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CHAN_CUR_TX_DESC` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CHAN_RX_TAIL` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CHAN_STATUS` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CHAN_STATUS_RPS` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CHAN_STATUS_TPS` | item | `src/regs.rs` | unrecorded. | yes | term |
| `CHAN_TX_TAIL` | item | `src/regs.rs` | unrecorded. | yes | term |
| `DMA_PAGE` | item | `src/regs.rs` | unrecorded. | yes | nife: nife's own process plan |
| `MAC_ADDR0_HI` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_ADDR0_LO` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_CONF_BE` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_CONF_DCRS` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_CONF_DM` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_CONF_FES` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_CONF_JD` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_CONF_LM` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_CONF_PS` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_MDIO_ADDR` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_MDIO_DATA` | item | `src/regs.rs` | unrecorded. | yes | term |
| `MAC_VERSION` | item | `src/regs.rs` | unrecorded. | yes | term |
| `WINDOW` | item | `src/regs.rs` | unrecorded. | yes | nife: nife's own process plan |
| `mac_hw_feature` | item | `src/regs.rs` | unrecorded. | yes | term |
| `with` | item | `src/sim.rs` | unrecorded. | no | clear |
