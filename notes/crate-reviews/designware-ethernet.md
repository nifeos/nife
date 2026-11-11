# Review package: `designware_ethernet`, names

*Name: provisional, minted by lane lane/spell-out-ethernet-clock-reset. The naming results of the
`designware_ethernet` section of [crate-reviews.md](../crate-reviews.md), in a page of their own
because that file is at its word budget.*

## Naming rules

The template's checks at the commit the moved crate repositories pin (`cd1b484`), run as its check
job runs them over the tree `script/crate-move --dry-run` stages:

| rule | result |
|---|---|
| spelled-out words (`spelled_out.py`) | 2 findings in 1,212 names: the two private `100BASE_TX` constants. `TX` there is the standard's name for the twisted-pair layer, not transmit, and the gate keeps no source spelling on an integer constant. |
| register rules (`register_names.py`) | 91 findings: 60 uses of `address` with no `bus` or `mac` qualifier (a PHY's management bus address, the station address), 20 lengths and sizes with no unit (burst lengths among them), 4 offsets that do not end `_REGISTER`, 2 constants the gate calls unexpanded, 3 doc comments that state a source spelling with no matching alias. |
| acronyms in prose (`prose_acronyms.py`) | 25 findings, each an acronym in a doc comment not expanded at its first use. |
| README prose caps and passives | pass. |
| units spelled out (`unit_symbols.py`) | 0 findings. Picoseconds, microamps and millivolts are spelled in the names that said `_PS`, `ua` and `mv`. |
| question shape (`question_shape.py`) | 0 findings. `region_in_reach` and the simulator's `receiving` take `is_`. |
| British spelling | 0 findings. |
| aliases of an old nife spelling | none. Every `doc(alias)` is a databook, OpenBSD, Linux or IEEE term. |

The word list reaches only the words it lists. The sweep also read every identifier for clipped
and decoder-needing words. Renamed, all private:

| was | now |
|---|---|
| the 66 `pub(crate)` register constants of `registers`, such as `CHAN_TX_CONTROL_ST` | spelled out, such as `CHANNEL_TRANSMIT_CONTROL_START_TRANSMISSION` |
| the 24 clause 22 register and bit constants of `management_data_input_output`, such as `BMCR_ANENABLE` | `BASIC_MODE_CONTROL_AUTONEGOTIATION_ENABLE` and its siblings |
| the Motorcomm register and bit constants, `PHY_INTF_RGMII` and the `_PS` constants | spelled out, with `_PICOSECONDS` |
| `ALWAYS_ON_SYSTEM_CONTROLLER_BASE` and `_SIZE` | `_BASE_PHYSICAL_ADDRESS` and `_SIZE_BYTES` |
| the `mdio` import alias | the module's own name |
| the harness `an_mdio_command_word_keeps_each_address_in_its_own_field` and its patch | `a_management_data_input_output_command_word_keeps_each_address_in_its_own_field` |
| the locals `nic`, `conf`, `rqs`, `tqs`, `rbsz`, `sel`, `des2`, `w01`, `w23`, `rxc`, `rxd` and the one-letter bindings of non-test code | words that say what they hold |

The source's spelling, such as `GMAC_CHAN_TX_CONTROL_ST` or `MII_BMCR`, is in each constant's doc
comment and in a `doc(alias)`. Every patch still applies and fails its own harness.

Parameter names of public functions, spelled out. They are provisional: a parameter name is no
part of a Rust signature's compatibility, so the maintainer rules them, pending the architect.

| parameter | of | now |
|---|---|---|
| `i` | `layout::receive_descriptor`, `layout::transmit_descriptor`, `registers::mac_hardware_feature_register`, and the crate-private `layout::receive_buffer` and `transmit_buffer` | `index` |
| `o` | `cache_coherence::classify`, `is_transmit_complete` | `observation` |
| `s` | `motorcomm_physical_layer_device::configure`, `apply_speed` | `settings` |
| `v` | `Hardware::write`, `DirectMemoryAccessRegion::write_u64`, `ManagementDataInputOutput::write`, `motorcomm_physical_layer_device::write_extended`, and every implementation of the three traits in the crate | `value` |

The simulator's `with(f)` keeps `f`, the standard library's spelling for a closure argument.

Ratified names that a later rule reached, and the names they now have.

| was | rule | now |
|---|---|---|
| `jh7110::SystemController::base`, `size` | an address says virtual or physical, and a size carries its unit | `base_physical_address`, `size_bytes` |
| `RECEIVE_DESCRIPTOR_WORD_3_BUFFER_1_ADDRESS_VALID` | the same address rule | `RECEIVE_DESCRIPTOR_WORD_3_BUFFER_1_PHYSICAL_ADDRESS_VALID` |
| the parameter `region` of `network_interface_controller::configure` | the same address rule: it is the base physical address of the controller's DMA region | `region_physical_address` |
| the parameter `address` of `Tails::set_receive_tail` and `set_transmit_tail`, and of every implementation | it is the low 32 bits of the tail pointer, not an address | `tail_pointer_low_bits` |

Borderline, kept as they are: decided by the maintainer, provisional (2026-11-11 UTC), since each
passes the template's gates and an unreleased crate can still rename them:

| names | why |
|---|---|
| `CRC_BYTES` and the `crc` in a test name | `CRC` passes the acronym test, since nobody says the expansion, but the accepted list names it nowhere. |
| the simulator's `dram` field | the same test. It is test-only. |
| `yt8531` | the part's model number. |
