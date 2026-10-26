# The names in `designware_ethernet`, for ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for the crate of milestone 816 (proven DesignWare Ethernet QoS logic, released on its own). A release-track crate's names take their ratification before it leaves this tree (ruled 2026-10-25 UTC). This file lists every name a stranger meets in it, in the record format `helpers/name_release.py` reads.

This file ratifies nothing. Until the line below carries a date, the crate's own `Name:` blocks stay the record, and the status column copies them as of 2026-10-26 (UTC). `python3 helpers/name_release.py --draft designware_ethernet` prints a fresh copy. At ratification each row's status takes the ruling and any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

A row whose status refuses a spelling took its name from a ruling of 2026-10-26 (UTC), and
[designware-ethernet-renames.md](designware-ethernet-renames.md) states each ruling.

The spawn handoff (`Handoff` with `pack`, `unpack` and `is_role`, and `ROLE_TAG`) sits in the `designware_ethernet` module of the nife-internal crate `driver_process_contract`, which does not publish (ruled 2026-10-26 UTC). Those names follow nife's ordinary naming and have no row here.

155 names: 138 no block, 16 provisional, 1 ratified. A status of `unrecorded` means the crate holds no block for the name. The column `outside` says whether a caller outside the crate can reach the name; 7 sit in private modules without a re-export. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them.

The last column reads each name against design/naming.md's tests and 857's Fork 5, which asks for names that stand on their own because the crate may outlive nife. 28 rows say something other than `clear` or `term`. This file renames nothing.

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
| `hardware` | module | `src/hardware.rs` | provisional. | no | clear |
| `jh7110` | module | `src/jh7110.rs` | unrecorded. | yes | scope: one SoC's glue in a MAC crate |
| `layout` | module | `src/layout.rs` | unrecorded. | yes | nife: nife's own process plan |
| `management_data_input_output` | module | `src/management_data_input_output.rs` | provisional. Refused `mdio` (ruled 2026-10-26 UTC). | yes | clear |
| `motorcomm` | module | `src/motorcomm.rs` | unrecorded. | yes | scope: one vendor's PHY in a MAC crate |
| `proofs` | module | `src/proofs.rs` | unrecorded. | no | clear; private |
| `registers` | module | `src/registers.rs` | provisional. Refused `regs` (ruled 2026-10-26 UTC). | yes | clear |
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
| `direct_memory_access_address_bits` | item | `src/controller.rs` | provisional. Refused `dma_bits` (ruled 2026-10-26 UTC). | yes | clear |
| `mac_conf` | item | `src/controller.rs` | unrecorded. | yes | term |
| `read` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `soft_reset` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `start` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `station_address` | item | `src/controller.rs` | unrecorded. | yes | term |
| `stop` | item | `src/controller.rs` | unrecorded. | yes | clear |
| `DataPlane` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `Region` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `Tails` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `TransmitError` | item | `src/data_plane.rs` | provisional. Refused `TxError` (ruled 2026-10-26 UTC). | yes | clear |
| `can_transmit` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `new` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `receive` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `receive_cursor` | item | `src/data_plane.rs` | provisional. Refused `rx_cursor` (ruled 2026-10-26 UTC). | yes | clear |
| `start` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `transmit` | item | `src/data_plane.rs` | unrecorded. | yes | clear |
| `BUFFER_SIZE` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `CRC_BYTES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `DESCRIPTOR_BYTES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `MAXIMUM_FRAME` | item | `src/descriptor.rs` | provisional. Refused `MAX_FRAME` (ruled 2026-10-26 UTC). | yes | clear |
| `RDES3_BUF1V` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_CTXT` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_ES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_FD` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_IC` | item | `src/descriptor.rs` | provisional. Refused `RDES3_IOC` (ruled 2026-10-26 UTC). | yes | clear |
| `RDES3_LD` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_LENGTH` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RDES3_OWN` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `RECEIVE_ENTRIES` | item | `src/descriptor.rs` | provisional. Refused `RX_ENTRIES` (ruled 2026-10-26 UTC). | yes | clear |
| `ReceiveVerdict` | item | `src/descriptor.rs` | provisional. Refused `RxVerdict` (ruled 2026-10-26 UTC). | yes | clear |
| `TDES2_BUF1_LEN` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES2_IC` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES3_ES` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES3_FRAME_LEN` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES3_FS` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES3_LS` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TDES3_OWN` | item | `src/descriptor.rs` | unrecorded. | yes | clear |
| `TRANSMIT_ENTRIES` | item | `src/descriptor.rs` | provisional. Refused `TX_ENTRIES` (ruled 2026-10-26 UTC). | yes | clear |
| `receive_descriptor_words` | item | `src/descriptor.rs` | provisional. Refused `rx_descriptor_words` (ruled 2026-10-26 UTC). | yes | clear |
| `receive_verdict` | item | `src/descriptor.rs` | provisional. Refused `rx_verdict` (ruled 2026-10-26 UTC). | yes | clear |
| `transmit_descriptor_words` | item | `src/descriptor.rs` | provisional. Refused `tx_descriptor_words` (ruled 2026-10-26 UTC). | yes | clear |
| `Hardware` | item | `src/hardware.rs` | provisional. Refused `Hw` (ruled 2026-10-26 UTC). | yes | clear |
| `ALWAYS_ON_SYSTEM_CONTROLLER_NODE` | item | `src/jh7110.rs` | provisional. Refused `AON_SYSCON_NODE` (ruled 2026-10-26 UTC). | yes | clear |
| `BUS` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `COMPATIBLES` | item | `src/jh7110.rs` | provisional. | yes | clear |
| `COMPATIBLE_ALWAYS_ON_SYSTEM_CONTROLLER` | item | `src/jh7110.rs` | provisional. Refused `COMPATIBLE_AON_SYSCON` (ruled 2026-10-26 UTC). | yes | clear |
| `COMPATIBLE_MAINLINE` | item | `src/jh7110.rs` | provisional. | yes | clear |
| `COMPATIBLE_VENDOR` | item | `src/jh7110.rs` | provisional. | yes | clear |
| `Discovered` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `GMAC0_BASE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `GMAC0_SYSTEM_CONTROLLER_OFFSET` | item | `src/jh7110.rs` | provisional. Refused `GMAC0_SYSCON_OFFSET` (ruled 2026-10-26 UTC). | yes | clear |
| `GMAC1_BASE` | item | `src/jh7110.rs` | unrecorded. | yes | clear |
| `SystemController` | item | `src/jh7110.rs` | provisional. Refused `Syscon` (ruled 2026-10-26 UTC). | yes | clear |
| `VENDOR_PHYSICAL_LAYER_DEVICE_NODE` | item | `src/jh7110.rs` | provisional. Refused `VENDOR_PHY_NODE` (ruled 2026-10-26 UTC). | yes | clear |
| `fallback` | item | `src/jh7110.rs` | provisional. | yes | clear |
| `first_cell` | item | `src/jh7110.rs` | provisional. | yes | scope: a device-tree cell reader in a MAC crate |
| `from_node` | item | `src/jh7110.rs` | provisional. | yes | clear |
| `interface` | item | `src/jh7110.rs` | unrecorded. | yes | generic |
| `mainline_physical_layer_device` | item | `src/jh7110.rs` | provisional. Refused `mainline_phy` (ruled 2026-10-26 UTC). | yes | unclear: reads as a PHY; it reads a PHY node into settings |
| `select_reduced_gigabit_media_independent_interface` | item | `src/jh7110.rs` | provisional. Refused `select_rgmii` (ruled 2026-10-26 UTC). | yes | clear |
| `vendor_physical_layer_device` | item | `src/jh7110.rs` | provisional. Refused `vendor_phy` (ruled 2026-10-26 UTC). | yes | unclear: reads as a PHY; it reads a PHY node into settings |
| `BYTES` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `PAGES` | item | `src/layout.rs` | unrecorded. | yes | clear |
| `receive_descriptor` | item | `src/layout.rs` | provisional. Refused `rx_descriptor` (ruled 2026-10-26 UTC). | yes | clear |
| `transmit_descriptor` | item | `src/layout.rs` | provisional. Refused `tx_descriptor` (ruled 2026-10-26 UTC). | yes | clear |
| `BMCR_RESET` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `BMSR_ANEGCOMPLETE` | item | `src/management_data_input_output.rs` | provisional. Refused `BMSR_ANEG_COMPLETE` (ruled 2026-10-26 UTC). | yes | clear |
| `Bus` | item | `src/management_data_input_output.rs` | unrecorded. | yes | generic: two `Bus` types in two modules |
| `COMMAND_LIMIT_US` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `CR_500_800` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `CR_SHIFT` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `Error` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `GB` | item | `src/management_data_input_output.rs` | unrecorded. | yes | term |
| `GOC_READ` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `GOC_WRITE` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `ManagementDataInputOutput` | item | `src/management_data_input_output.rs` | provisional. Refused `Mdio` (ruled 2026-10-26 UTC). | yes | clear |
| `PA_SHIFT` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `RDA_SHIFT` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `command` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `link_is_up` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `physical_layer_device_id` | item | `src/management_data_input_output.rs` | provisional. Refused `phy_id` (ruled 2026-10-26 UTC). | yes | clear |
| `restart_autonegotiation` | item | `src/management_data_input_output.rs` | unrecorded. | yes | clear |
| `Applied` | item | `src/motorcomm.rs` | unrecorded. | yes | generic |
| `Drive` | item | `src/motorcomm.rs` | unrecorded. | yes | generic |
| `FALLBACK_GMAC0` | item | `src/motorcomm.rs` | provisional. | yes | scope: one board's PHY values in a MAC crate |
| `Link` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `SPECIFIC_STATUS_REG` | item | `src/motorcomm.rs` | provisional. Refused `SPECIFIC_STATUS` (ruled 2026-10-26 UTC). | yes | clear |
| `SSR_DUPLEX` | item | `src/motorcomm.rs` | provisional. Refused `STATUS_DUPLEX` (ruled 2026-10-26 UTC). | yes | clear |
| `SSR_LINK` | item | `src/motorcomm.rs` | provisional. Refused `STATUS_LINK` (ruled 2026-10-26 UTC). | yes | clear |
| `SSR_SPEED_DUPLEX_RESOLVED` | item | `src/motorcomm.rs` | provisional. Refused `STATUS_RESOLVED` (ruled 2026-10-26 UTC). | yes | clear |
| `SSR_SPEED_SHIFT` | item | `src/motorcomm.rs` | provisional. Refused `STATUS_SPEED_SHIFT` (ruled 2026-10-26 UTC). | yes | clear |
| `Settings` | item | `src/motorcomm.rs` | unrecorded. | yes | generic |
| `Source` | item | `src/motorcomm.rs` | unrecorded. | yes | generic |
| `Speed` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `PHY_ID_YT8531` | item | `src/motorcomm.rs` | provisional. Refused `YT8531_ID` (ruled 2026-10-26 UTC). | yes | clear |
| `apply_speed` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `configure` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `link` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `link_from_status` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `mbps` | item | `src/motorcomm.rs` | unrecorded. | yes | clear |
| `read_extended` | item | `src/motorcomm.rs` | provisional. Refused `read_ext` (ruled 2026-10-26 UTC). | yes | clear |
| `write_extended` | item | `src/motorcomm.rs` | provisional. Refused `write_ext` (ruled 2026-10-26 UTC). | yes | clear |
| `CHAN_CUR_RX_DESC` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CHAN_CUR_TX_DESC` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CHAN_RX_END_ADDR` | item | `src/registers.rs` | provisional. Refused `CHAN_RX_TAIL` (ruled 2026-10-26 UTC). | yes | clear |
| `CHAN_STATUS` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CHAN_STATUS_RPS` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CHAN_STATUS_TPS` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CHAN_TX_END_ADDR` | item | `src/registers.rs` | provisional. Refused `CHAN_TX_TAIL` (ruled 2026-10-26 UTC). | yes | clear |
| `DIRECT_MEMORY_ACCESS_REGISTER_PAGE` | item | `src/registers.rs` | provisional. Refused `DMA_PAGE` (ruled 2026-10-26 UTC). | yes | clear |
| `MAC_ADDR0_HI` | item | `src/registers.rs` | unrecorded. | yes | term |
| `MAC_ADDR0_LO` | item | `src/registers.rs` | unrecorded. | yes | term |
| `MAC_CONF_BE` | item | `src/registers.rs` | unrecorded. | yes | term |
| `MAC_CONF_DCRS` | item | `src/registers.rs` | unrecorded. | yes | term |
| `MAC_CONF_DM` | item | `src/registers.rs` | unrecorded. | yes | term |
| `MAC_CONF_FES` | item | `src/registers.rs` | unrecorded. | yes | term |
| `MAC_CONF_JD` | item | `src/registers.rs` | unrecorded. | yes | term |
| `CONFIG_LM` | item | `src/registers.rs` | provisional. Refused `MAC_CONF_LM` (ruled 2026-10-26 UTC). | yes | clear |
| `MAC_CONF_PS` | item | `src/registers.rs` | unrecorded. | yes | term |
| `MAC_MDIO_ADDR` | item | `src/registers.rs` | unrecorded. | yes | term |
| `MAC_MDIO_DATA` | item | `src/registers.rs` | unrecorded. | yes | term |
| `VERSION` | item | `src/registers.rs` | provisional. Refused `MAC_VERSION` (ruled 2026-10-26 UTC). | yes | clear |
| `WINDOW` | item | `src/registers.rs` | unrecorded. | yes | nife: nife's own process plan |
| `mac_hw_feature` | item | `src/registers.rs` | unrecorded. | yes | term |
| `with` | item | `src/sim.rs` | unrecorded. | no | clear |

Every `pub` module and item in the crate's source has a row. `script/names` does not count these as names of their own, so they have none:

- the fields of `Observation` (4), `Identity` (2), `controller::Bus` (8), `DataPlane` (1), `Discovered` (9), `SystemController` (3), `Link` (2), `Settings` (7) and `Applied` (3);
- the variants of `Payload` (3), `Verdict` (7), `controller::Error` (3), `ReceiveVerdict` (3), `management_data_input_output::Error` (1), `TransmitError` (2), `Speed` (3), `Drive` (2) and `Source` (3);
- the methods of the traits `Region` (4), `Tails` (3), `Hardware` (3) and `ManagementDataInputOutput` (2).

The `pub use` lines re-export items that have rows where they are defined. The one `pub` field in `src/sim.rs` belongs to a crate-private type.
