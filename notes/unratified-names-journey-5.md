# Names journey 5 has not ratified yet

*Name: provisional, minted 2026-11-03 (UTC) by lane/unratified-names-journey-5. The file name says what the list holds; `names-awaiting-ratification` was another candidate.*

This note lists, for every crate on [journey 5](../design/journeys/05-crates-released-on-their-own.md) that still sits under `crates/`, each module, type, function, constant and register whose `Name:` block is missing or still provisional, in groups an architect can rule on one at a time. It proposes and renames nothing. Names are an architect's call ([design/naming.md](../design/naming.md)); every recommendation below is provisional and leaves the current spelling in force until a ruling.

`script/crate-standalone` step 0 passes a crate only when every `Name:` block in it is ratified, so each group here stands between a crate and its move. The lists come from `python3 helpers/name_release.py --draft <crate>`: a row whose status reads `no block` or starts with `provisional`. Rerun it for the live state; nothing regenerates this note, and a ruling removes its group.

`file_allocation_table`, `generational_table` and `jh7110_clock_and_reset` have no unratified name left. `jh7110_entropy` has already left `crates/`.

## Counts

| crate | unratified | no block | provisional | groups |
|---|---|---|---|---|
| `designware_ethernet` | 110 | 91 | 19 | 13 |
| `designware_mobile_storage` | 74 | 74 | 0 | 10 |
| `e1000e` | 108 | 70 | 38 | 12 |
| `extensible_host_controller_interface` | 9 | 0 | 9 | 1 |
| `non_volatile_memory_express` | 3 | 0 | 3 | 1 |
| `generational_table` | 0 | 0 | 0 | 0 |
| `file_allocation_table` | 0 | 0 | 0 | 0 |
| `jh7110_clock_and_reset` | 0 | 0 | 0 | 0 |
| total | 304 | 235 | 69 | 37 |

## How to read a group

Each group gives its items in their current spelling, as `path::name`, and a module by its own name. *Breaks the spelled-out rule* answers the 2026-11-02 rule in [design/naming.md](../design/naming.md): outside register modules, identifiers say what the thing is and not the specification's abbreviation. A hardware register, field or bit constant takes the spelled-out name with the source's spelling in `#[doc(alias)]`, OpenBSD's cited first. `yes` means the group has at least that kind of abbreviation; `one` means a few named items do; `no` means none. The recommendation is the stricter reading wherever a call was close, since loosening later is compatible and tightening is not.

## Smallest first decisions

In order, easiest to rule on first:

1. `X1`, the nine `extensible_host_controller_interface` assertions: the same rename the architect already ruled for two sister crates.
2. `N1`, the three NVMe namespace names: a full proposal already exists ([ratify-the-nvme-namespace-names](../design/roadmap/proposals/ratify-the-nvme-namespace-names.md)).
3. `DE2`, the sixteen designware_ethernet assertions: the maintainer already chose them, and reversing costs one pull request.
4. The `no` groups of designware_mobile_storage (`DM1` to `DM4`, `DM6` to `DM8`, `DM10`): ratify as they stand, which clears the module apart from DM5 and DM9.

The groups that need a real ruling are the register and bit constants: `DE7` to `DE9` and `E8`. Decide the designware_ethernet ones first, since #3094 set the pattern, then reuse it for e1000e.

## `designware_ethernet`

110 unratified names: 91 with no block, 19 provisional.

### DE1. Module names (10)

`coherence`, `controller`, `data_plane`, `descriptor`, `hardware`, `jh7110`, `layout`, `motorcomm`, `proofs`, `tests`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify all ten as they stand. `data_plane`, `descriptor` and `hardware` are private and already provisional; `proofs` and `tests` follow the tree's usual names for harness and test modules; the rest say what the module holds.

### DE2. The sixteen non-overlap assertions (16)

`coherence::PROBE_PAYLOAD_SET_BIT_AND_COUNTER_DO_NOT_OVERLAP`, `controller::CONFIGURED_BUS_FIELDS_DO_NOT_OVERLAP`, `controller::MAC_CONFIGURATION_BITS_DO_NOT_OVERLAP`, `controller::RECEIVE_QUEUE_MODE_FIELDS_DO_NOT_OVERLAP`, `controller::TRANSMIT_QUEUE_MODE_FIELDS_DO_NOT_OVERLAP`, `descriptor::FIRST_AND_LAST_BITS_DO_NOT_OVERLAP`, `descriptor::RECEIVE_DESCRIPTOR_SET_BITS_DO_NOT_OVERLAP`, `descriptor::TRANSMIT_DESCRIPTOR_BITS_DO_NOT_OVERLAP`, `jh7110::BURST_LENGTH_BITS_DO_NOT_OVERLAP`, `jh7110::INTERFACE_SELECT_AND_CLEARED_WORD_DO_NOT_OVERLAP`, `management_data_input_output::AUTONEGOTIATION_ENABLE_AND_RESTART_DO_NOT_OVERLAP`, `management_data_input_output::COMMAND_WORD_FIELDS_DO_NOT_OVERLAP`, `management_data_input_output::PHYSICAL_LAYER_DEVICE_ID_HALVES_DO_NOT_OVERLAP`, `motorcomm::PARTNER_HUNDRED_MEGABIT_ABILITIES_DO_NOT_OVERLAP`, `motorcomm::RECEIVE_DATA_DRIVE_FIELDS_DO_NOT_OVERLAP`, `motorcomm::RGMII_DELAY_AND_DRIVE_FIELDS_DO_NOT_OVERLAP`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand. The maintainer chose the `..._DO_NOT_OVERLAP` form (the 2026-11-02 ruling that renamed the `THE_..._SHARE_NO_BIT` set) and reversing costs one pull request; each says which fields it checks. Each is a private compile-time assertion.

### DE3. Coherence probe (9)

`coherence::Observation`, `coherence::POSTED`, `coherence::Payload`, `coherence::Verdict`, `coherence::WAIT_MICROSECONDS`, `coherence::classify`, `coherence::delivered`, `coherence::observe`, `coherence::prepare`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand. Plain words, no abbreviation. `POSTED` is a count of posted descriptors, not the bus term, and its doc says so.

### DE4. Controller bring-up (10)

`controller::Bus`, `controller::Error`, `controller::Identity`, `controller::configure`, `controller::core_version`, `controller::read`, `controller::soft_reset`, `controller::start`, `controller::station_address`, `controller::stop`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand. Plain verbs and nouns; `core_version` and `station_address` say what they return.

### DE5. Data plane (7)

`data_plane::DataPlane`, `data_plane::Tails`, `data_plane::can_transmit`, `data_plane::new`, `data_plane::receive`, `data_plane::start`, `data_plane::transmit`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand, and decide together with the e1000e data plane group, which uses the same seven words, so the two crates keep agreeing.

### DE6. Descriptor sizes and the small constants (3)

`descriptor::BUFFER_SIZE`, `descriptor::CRC_BYTES`, `descriptor::DESCRIPTOR_BYTES`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify `CRC_BYTES` and `DESCRIPTOR_BYTES`; recommend `BUFFER_SIZE` become `BUFFER_BYTES` so the three sizes share the unit word (the stricter reading; `BUFFER_SIZE` does not break the spelled-out rule, it only disagrees with its neighbors).

### DE7. Descriptor bit and field constants (RDES3_*, TDES2_*, TDES3_*) (14)

`descriptor::RDES3_BUF1V`, `descriptor::RDES3_CTXT`, `descriptor::RDES3_ES`, `descriptor::RDES3_FD`, `descriptor::RDES3_LD`, `descriptor::RDES3_LENGTH`, `descriptor::RDES3_OWN`, `descriptor::TDES2_BUF1_LEN`, `descriptor::TDES2_IC`, `descriptor::TDES3_ES`, `descriptor::TDES3_FRAME_LEN`, `descriptor::TDES3_FS`, `descriptor::TDES3_LS`, `descriptor::TDES3_OWN`

- Breaks the spelled-out rule: yes.
- Provisional recommendation: Spell out, with the databook spelling kept as `#[doc(alias)]` and cited in the doc comment, as the 2026-11-02 rule and the earlier `RECEIVE_DESCRIPTOR_WORD_3_INTERRUPT_ON_COMPLETION` rename already do. The docs already carry the plain-words title of each (for example 'owned by the device'), so the candidate names come from the crate's own text. Decide as one group; the items share one pattern.

### DE8. MAC and channel register constants (CHAN_*, MAC_*) (15)

`registers::CHAN_CUR_RX_DESC`, `registers::CHAN_CUR_TX_DESC`, `registers::CHAN_STATUS`, `registers::CHAN_STATUS_RPS`, `registers::CHAN_STATUS_TPS`, `registers::MAC_ADDR0_HI`, `registers::MAC_ADDR0_LO`, `registers::MAC_CONF_BE`, `registers::MAC_CONF_DCRS`, `registers::MAC_CONF_DM`, `registers::MAC_CONF_FES`, `registers::MAC_CONF_JD`, `registers::MAC_CONF_PS`, `registers::MAC_MDIO_ADDR`, `registers::MAC_MDIO_DATA`

- Breaks the spelled-out rule: yes.
- Provisional recommendation: Spell out under the same rule, in the pattern of the nine renames in #3094 (for example `CHANNEL_RECEIVE_DESCRIPTOR_TAIL_POINTER_REGISTER`). Decide as one group; the `MAC_CONF_*` bits and the `CHAN_STATUS_*` fields can ride with their parent register.

### DE9. Register window and the hardware-feature accessor (2)

`registers::WINDOW`, `registers::mac_hw_feature`

- Breaks the spelled-out rule: yes (one name).
- Provisional recommendation: `WINDOW` is a plain word and can be ratified; `mac_hw_feature` abbreviates 'hardware', so recommend `mac_hardware_feature` (alias `mac_hw_feature`). `mac` stays an accepted acronym, as #3094 ruled.

### DE11. MDIO software surface (6)

`management_data_input_output::Bus`, `management_data_input_output::COMMAND_LIMIT_MICROSECONDS`, `management_data_input_output::Error`, `management_data_input_output::command`, `management_data_input_output::link_is_up`, `management_data_input_output::restart_autonegotiation`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand. The module name is already spelled out; these are ordinary names.

### DE12. Motorcomm PHY software surface (10)

`motorcomm::Applied`, `motorcomm::Drive`, `motorcomm::Link`, `motorcomm::Settings`, `motorcomm::Source`, `motorcomm::Speed`, `motorcomm::apply_speed`, `motorcomm::configure`, `motorcomm::link`, `motorcomm::mbps`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify all but `mbps`. `mbps` abbreviates a unit; recommend `megabits_per_second` (the stricter reading, a close call since 'Mbps' is a standard unit symbol).

### DE13. JH7110 node description (5)

`jh7110::BUS`, `jh7110::Discovered`, `jh7110::GMAC0_BASE`, `jh7110::GMAC1_BASE`, `jh7110::interface`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand. `GMAC0_BASE` and `GMAC1_BASE` keep `GMAC`, an accepted acronym since #3094, whose expansion is already in the doc comments.

### DE14. Layout and simulator odds and ends (3)

`layout::BYTES`, `layout::PAGES`, `simulator::with`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand: `BYTES` and `PAGES` are the layout sizes, `with` is the simulator builder.

## `designware_mobile_storage`

74 unratified names: 74 with no block, 0 provisional.

### DM1. Module names (9)

`bench`, `card`, `command`, `host`, `jh7110`, `partition`, `proofs`, `sd`, `tests`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify all nine. `sd` passes the acronym test (Secure Digital is a spoken phrase, but nobody says it); the rest are plain.

### DM2. Host controller (13)

`host::Error`, `host::Host`, `host::Identity`, `host::Registers`, `host::card_clock`, `host::clock_divider`, `host::command`, `host::into_registers`, `host::new`, `host::read`, `host::registers`, `host::reset`, `host::write`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand. The register-level names inside this module were already ratified spelled out under #3092; these are the software surface around them.

### DM3. Command words (9)

`command::Command`, `command::Data`, `command::GO_IDLE_STATE`, `command::Response`, `command::SEND_STATUS`, `command::SWITCH`, `command::bytes`, `command::new`, `command::word`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand. `GO_IDLE_STATE`, `SEND_STATUS` and `SWITCH` are the SD specification's own command names, spelled in full; OpenBSD's `sdmmcreg.h` uses the same words.

### DM4. Card status (6)

`card::R1_ERRORS`, `card::R1_READY_FOR_DATA`, `card::STATE_TRANSFER`, `card::bits`, `card::r1_state`, `card::sd`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand. `R1` is the specification's own name for the response type, a noun with no spelled-out form.

### DM5. Card access (9)

`sd::BLOCK`, `sd::Card`, `sd::IoError`, `sd::Kind`, `sd::address`, `sd::identify`, `sd::in_range`, `sd::read_blocks`, `sd::write_blocks`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify all but `BLOCK`. `BLOCK` is the 512-byte size; recommend `BLOCK_BYTES` so it names its unit, as `TRANSFER_HZ` already does (ratified #3092).

### DM6. Partition table (6)

`partition::Entry`, `partition::SIGNATURE`, `partition::contains`, `partition::is_used`, `partition::parse`, `partition::scratch`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand.

### DM7. Read and write bench (7)

`bench::ReadFailure`, `bench::ReadReport`, `bench::SCRATCH_SECTORS`, `bench::WriteVerdict`, `bench::pattern`, `bench::read_only`, `bench::scratch_write`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand.

### DM8. Simulator (5)

`simulator::Fault`, `simulator::Sim`, `simulator::depth`, `simulator::new`, `simulator::peek`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand.

### DM9. JH7110 node description (8)

`jh7110::COMPATIBLE_MAINLINE`, `jh7110::COMPATIBLE_VENDOR`, `jh7110::INTERRUPTS`, `jh7110::SDIO0_BASE`, `jh7110::SDIO1_BASE`, `jh7110::Slot`, `jh7110::WINDOW_SIZE`, `jh7110::role`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify as they stand, with one question: `SDIO0_BASE` and `SDIO1_BASE` keep `SDIO` (Secure Digital Input Output). Recommend treating it as an accepted acronym with its expansion in the doc comment, the way #3094 handled `GMAC`. `COMPATIBLE_MAINLINE` and `COMPATIBLE_VENDOR` are the names already ratified in designware_ethernet.

### DM10. Register window (2)

`registers::STATUS`, `registers::WINDOW_USED`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand.

## `e1000e`

108 unratified names: 70 with no block, 38 provisional.

### E1. Module names (14)

`control`, `data_plane`, `descriptor`, `device`, `layout`, `pch`, `pch/flush`, `pch/hardware`, `pch/reset`, `pch/sequence`, `pch/tests`, `pch/ulp`, `proofs`, `tests`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify all but two. `pch` and `ulp` abbreviate 'platform controller hub' and 'ultra low power'; recommend `platform_controller_hub` and `ultra_low_power` (the stricter reading, a close call since Intel and Linux write both as letters; the sibling modules `non_volatile_memory` and `physical_layer_device` already spell out their words). The other twelve are plain.

### E2. Device identity (4)

`device::DEVICE_IDS`, `device::VENDOR_INTEL`, `device::is_supported`, `device::model`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand.

### E3. Data plane (11)

`data_plane::DataPlane`, `data_plane::Region`, `data_plane::Tails`, `data_plane::can_transmit`, `data_plane::new`, `data_plane::receive`, `data_plane::receive_cursor`, `data_plane::set_receive_tail`, `data_plane::set_transmit_tail`, `data_plane::start`, `data_plane::transmit`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify as they stand, except `Region`: recommend `DirectMemoryAccessRegion` (alias `Region`), the name designware_ethernet already took in #3094, so both crates agree. Decide together with designware_ethernet's data plane group. `receive_cursor`, `set_receive_tail` and `set_transmit_tail` are provisional already, with `rx` and `tx` spelled out by the 2026-10-26 tree-wide ruling.

### E4. Descriptor ring sizes and positions (6)

`descriptor::BUFFER_SIZE`, `descriptor::RECEIVE_ENTRIES`, `descriptor::RECEIVE_QUEUE_PAGE`, `descriptor::TAIL_IN_PAGE`, `descriptor::TRANSMIT_ENTRIES`, `descriptor::TRANSMIT_QUEUE_PAGE`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand. `RECEIVE_ENTRIES`, `TRANSMIT_ENTRIES`, `*_QUEUE_PAGE`, `TAIL_IN_PAGE` and `BUFFER_SIZE` carry no abbreviation (`BUFFER_SIZE` has the same unit-word question as in designware_ethernet).

### E5. Descriptor status and command bits (7)

`descriptor::RECEIVE_ERRORS_FRAME`, `descriptor::RECEIVE_STATUS_DD`, `descriptor::RECEIVE_STATUS_EOP`, `descriptor::TRANSMIT_COMMAND`, `descriptor::TRANSMIT_STATUS_DD`, `descriptor::receive_verdict`, `descriptor::transmit_word`

- Breaks the spelled-out rule: yes.
- Provisional recommendation: Spell out `RECEIVE_STATUS_DD`, `RECEIVE_STATUS_EOP` and `TRANSMIT_STATUS_DD` (descriptor done, end of packet), keeping the Linux spelling as an alias; ratify the rest as they stand. These three are the only abbreviations in the module.

### E6. Buffer layout (10)

`layout::BYTES`, `layout::PAGES`, `layout::RECEIVE_BUFFERS`, `layout::RECEIVE_RING`, `layout::TRANSMIT_BUFFERS`, `layout::TRANSMIT_RING`, `layout::receive_buffer`, `layout::receive_descriptor`, `layout::transmit_buffer`, `layout::transmit_descriptor`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand; `rx` and `tx` already read `receive` and `transmit`.

### E8. Register bit constants and register-shaped helpers (12)

`control::CTRL_ASDE`, `control::CTRL_EXT_DRV_LOAD`, `control::CTRL_RST`, `control::CTRL_SLU`, `control::RAH_AV`, `control::RFCTL_EXTEN`, `control::STATUS_LU`, `control::rctl`, `control::tctl`, `control::tipg`, `pch::flush::FEXTNVM11_DISABLE_MULR_FIX`, `pch::flush::rxdctl`

- Breaks the spelled-out rule: yes.
- Provisional recommendation: Spell out with aliases: `CTRL_ASDE`, `CTRL_EXT_DRV_LOAD`, `CTRL_RST`, `CTRL_SLU`, `RAH_AV`, `RFCTL_EXTEN`, `STATUS_LU`, `FEXTNVM11_DISABLE_MULR_FIX`, and the functions `rctl`, `tctl`, `tipg`, `rxdctl` that build register values.

### E9. PCH flush and hardware (4)

`pch::flush::required`, `pch::flush::tail_after`, `pch::flush::transmit_word`, `pch::hardware::is_pch`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify `required`, `tail_after` and the provisional `transmit_word`; `is_pch` follows the module-name decision (`is_platform_controller_hub` if `pch` is spelled out).

### E10. Non-volatile memory words (6)

`pch::non_volatile_memory::DONE`, `pch::non_volatile_memory::ERROR`, `pch::non_volatile_memory::Error`, `pch::non_volatile_memory::GO`, `pch::non_volatile_memory::WORDS`, `pch::non_volatile_memory::read`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand.

### E11. Physical layer device access (17)

`pch::physical_layer_device::ERROR`, `pch::physical_layer_device::Error`, `pch::physical_layer_device::POLL_MICROSECONDS`, `pch::physical_layer_device::Plan`, `pch::physical_layer_device::READY`, `pch::physical_layer_device::SWFLAG_FREE_MS`, `pch::physical_layer_device::SWFLAG_TAKE_MS`, `pch::physical_layer_device::Swflag`, `pch::physical_layer_device::id_at`, `pch::physical_layer_device::paged_offset`, `pch::physical_layer_device::physical_layer_device_address`, `pch::physical_layer_device::plan`, `pch::physical_layer_device::read`, `pch::physical_layer_device::read_mdic`, `pch::physical_layer_device::release`, `pch::physical_layer_device::update`, `pch::physical_layer_device::write`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify as they stand except `Swflag`, `SWFLAG_FREE_MS`, `SWFLAG_TAKE_MS` and `read_mdic`, which abbreviate 'software flag', 'milliseconds' and the register MDIC; recommend `SoftwareFlag`, `SOFTWARE_FLAG_FREE_MILLISECONDS`, `SOFTWARE_FLAG_TAKE_MILLISECONDS` and keeping `read_mdic` as a register-named function with its expansion in the doc. `physical_layer_device_address` and `paged_offset` are provisional and ratify as they stand.

### E12. Reset sequence (10)

`pch::sequence::BlockedAfterPhysicalLayerDeviceReset`, `pch::sequence::Error`, `pch::sequence::Note`, `pch::sequence::PhysicalLayerDevice`, `pch::sequence::PhysicalLayerDeviceInaccessible`, `pch::sequence::PhysicalLayerDeviceReached`, `pch::sequence::PhysicalLayerDeviceResetNotAsserted`, `pch::sequence::Reached`, `pch::sequence::global_reset`, `pch::sequence::transmit_errata`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand, including the five provisional `PhysicalLayerDevice*` and `BlockedAfterPhysicalLayerDeviceReset` variants; all spell out their words.

### E13. Ultra low power and simulator (7)

`pch::ulp::WAIT_MS`, `pch::ulp::done`, `pch::ulp::release`, `pch::ulp::request`, `pch::simulator::Sim`, `pch::simulator::Start`, `pch::simulator::new`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify `done`, `release`, `request` and the simulator names; recommend `WAIT_MS` become `WAIT_MILLISECONDS`, matching `POLL_MICROSECONDS`.

## `extensible_host_controller_interface`

9 unratified names: 0 with no block, 9 provisional.

### X1. The nine non-overlap assertions (9)

`context::THE_ENDPOINT_CONTEXT_FIELDS_SHARE_NO_BIT`, `context::THE_SLOT_CONTEXT_FIELDS_SHARE_NO_BIT`, `event::THE_EVENT_POINTER_HALVES_SHARE_NO_BIT`, `port::THE_PORT_CHANGE_BITS_SHARE_NO_BIT`, `port::THE_PRESERVED_FIELDS_SHARE_NO_BIT`, `registers::THE_LEGACY_ENABLE_BITS_SHARE_NO_BIT`, `registers::THE_SCRATCHPAD_FIELDS_SHARE_NO_BIT`, `transfer_request_block::THE_CYCLE_BIT_AND_CLEARED_WORD_SHARE_NO_BIT`, `transfer_request_block::THE_TRANSFER_CONTROL_FIELDS_SHARE_NO_BIT`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Rename the nine from `THE_..._SHARE_NO_BIT` to `..._DO_NOT_OVERLAP` (dropping `THE_`), the form the maintainer chose and the architect's 2026-11-02 ruling applied in designware_ethernet and designware_mobile_storage, then ratify. Mechanical; same wording, so the least contentious group in this list.

## `non_volatile_memory_express`

3 unratified names: 0 with no block, 3 provisional.

### N1. Namespace (3)

`lib::Namespace`, `lib::holds_block`, `lib::transfer_command`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Already proposed in full in design/roadmap/proposals/ratify-the-nvme-namespace-names.md (ratify as they stand, plus the `size_bytes` to `capacity_bytes` field loose end). Do not repeat it; bring that proposal.

## Not covered

The lists hold names with no block or a provisional one. A name already ratified keeps its spelling until its own review. Cargo features carry no block and are not listed. A crate may carry further unratified names that `name_release.py` does not read, for example inside macro expansions.
