# Review package: `designware_ethernet`

*Name: provisional, minted by lane lane/spell-out-ethernet-clock-reset. The `designware_ethernet`
package of the index [crate-reviews.md](../crate-reviews.md), journey 5 step 7.*

## The package

Name and version. `designware_ethernet` `0.1.0`, ratified (Synopsys's name, spelled out). 173
`Name:` blocks, all ratified.

API. The DesignWare Ethernet QoS controller: `registers`, `network_interface_controller` (bring-up
behind the `Hardware` trait), `management_data_input_output`, `motorcomm_physical_layer_device` (the
YT8531 decode, `link_from_registers`), `cache_coherence` (the probe's verdict), `jh7110` (glue and what
each device tree says), `DataPlane`, descriptors and `layout`. No nife type appears in a public
signature.

Proofs. Six harnesses: the delivered frame, the ring offsets, the PHY link decode, the transmit
descriptor's length fields, the MDIO command word's address fields and the coherence verdict. All
six prove and all six patches fail. MDIO sequencing, the PHY configuration writes and controller bring-up keep unit tests
against the simulator and no proof.

Dependencies. None.

Provenance. The crate adapts OpenBSD's `dwqe` and `ytphy` (ISC, with copyright lines in `lib.rs`)
and `ukphy_status` (NetBSD Foundation BSD-2-Clause). Both notices are in the tree. Milestone 1019
items 5 and 9 hold. The manifest license field reads `(MIT OR Apache-2.0) AND ISC AND BSD-2-Clause`,
naming the license of each adapted source.

The YT8531 register sources, checked against each file at a pinned revision. The home of these
attributions is the module documentation of `motorcomm_physical_layer_device`.

| crate item | source that names it |
|---|---|
| the register and bit constants other than the two below, and `configure`'s order | OpenBSD `sys/dev/mii/ytphy.c` at `openbsd/src@9d7a05f92003` (revision 1.6): its `YT8511_` to `YT8531_` defines and `ytphy_yt8521_init`, `ytphy_yt8521_update`, `ytphy_yt8531_ds` and `ytphy_yt8531_init` |
| `MOTORCOMM_EXTENDED_CLOCK_GATING_REGISTER` (0x0c), `MOTORCOMM_EXTENDED_SLEEP_CONTROL_1_REGISTER` (0x27) | `ytphy.c`'s `YT8511_EXT_CLK_GATE` and `YT8511_EXT_SLEEP_CTRL`; FreeBSD `sys/dev/mii/mcommphy.c` at `freebsd/freebsd-src@2ad32d3a6868`, `PHY_CLOCK_GATING_REG` and `PHY_SLEEP_CONTROL1_REG`; Linux `drivers/net/phy/motorcomm.c` at `torvalds/linux@fc697291ad0a`, `YT8521_CLOCK_GATING_REG` and `YT8521_EXTREG_SLEEP_CONTROL1_REG` |
| `CGR_RX_CLK_EN` (bit 12), `ESC1R_SLEEP_SW` (bit 15) | `motorcomm.c` only, `YT8521_CGR_RX_CLK_EN` and `YT8521_ESC1R_SLEEP_SW`, which `yt8531_config_init` clears for `motorcomm,keep-pll-enabled` and `motorcomm,auto-sleep-disabled` |
| `MOTORCOMM_YT8531_PHYSICAL_LAYER_DEVICE_ID` (`0x4f51e91b`) | `motorcomm.c`'s `PHY_ID_YT8531`, matched with `PHY_ID_MATCH_EXACT`; `ytphy.c`'s `ytphys` and `mcommphy.c`'s `mcommphys` match the OUI and model |

Both register constants carry OpenBSD's spelling and FreeBSD's as aliases, and no Linux spelling or
earlier spelling of this tree, as `design/naming.md` requires before a first release.

Result of `script/crate-standalone`. Clean, with Kani, on 2026-11-11 (UTC). It runs none of the
template's naming and prose gates.

Names. Under Naming rules, below, with the template gates' 118 findings. The parameters carry
spelled-out names, provisional.

Outstanding before the move.

| item | owner |
|---|---|
| The 118 template gate findings, in this tree: a rename of a ratified public name goes to the architect as one table; private names, parameters, tests and doc comments are the maintainer's provisional calls. | a rename lane |

Recommendation. The architect cleared the move on 2026-11-11 (UTC), and has not cleared the tag or
publication. The move waits for the template gates.

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
