# Names journey 5 has not ratified yet

*Name: provisional, minted 2026-11-03 (UTC) by lane/unratified-names-journey-5. The file name says what the list holds; `names-awaiting-ratification` was another candidate.*

This note lists, for every crate on [journey 5](../design/journeys/05-crates-released-on-their-own.md) that still sits under `crates/`, each module, type, function, constant and register whose `Name:` block is missing or still provisional, in groups an architect can rule on one at a time. It proposes and renames nothing. Names are an architect's call ([design/naming.md](../design/naming.md)); every recommendation below is provisional and leaves the current spelling in force until a ruling.

`script/crate-standalone` step 0 passes a crate only when it finds every `Name:` block in it ratified, so each group here stands between a crate and its move. The lists come from `python3 helpers/name_release.py --draft <crate>`: a row whose status reads `no block` or starts with `provisional`. Rerun it for the live state; nothing regenerates this note, and a ruling removes its group.

`extensible_host_controller_interface`, `file_allocation_table`, `generational_table` and `jh7110_clock_and_reset` have no unratified name left. `jh7110_entropy` has already left `crates/`.

## How a naming pull request edits this note and the baselines

Two pull requests that change the same or adjacent lines conflict in git, so no line here or in
the baselines is shared by two naming pull requests:

- This note carries no total, count table or per-crate count; `python3 helpers/name_release.py --draft <crate>` lists a crate's rows, and their number is the count. A pull request edits only the group it
  rules on: it deletes the group's section, or deletes the names it ratifies from the group's list,
  and changes that group's number in its heading.
- `design/module-block-baseline.txt` and `design/naming-rule-baseline.txt` hold one row per line,
  sorted, with a blank line after every row (`helpers/baseline_layout.py`). A pull request deletes
  the row lines it clears and nothing else: it keeps the blank lines, it does not re-sort, and it
  writes no total. `--bank` does exactly that. A rename re-keys a row by deleting the old line and
  writing the new row and a blank line after it at its sorted place.
- The ratchets still compare the live count with the baseline and still refuse a row that is not
  cleared or a row that is new. Both gates also fail a baseline whose rows touch or are out of order.

## How to read a group

Each group gives its items in their current spelling, as `path::name`, and a module by its own name. *Breaks the spelled-out rule* answers the 2026-11-02 rule in [design/naming.md](../design/naming.md): outside register modules, identifiers say what the thing is and not the specification's abbreviation. A hardware register, field or bit constant takes the spelled-out name with the source's spelling in `#[doc(alias)]`, OpenBSD's cited first. `yes` means the group has at least that kind of abbreviation; `one` means a few named items do; `no` means none. The recommendation is the stricter reading wherever a call was close, since loosening later is compatible and tightening is not.

## Smallest first decisions

In order, easiest to rule on first:

1. `N1`, the three NVMe namespace names: a full proposal already exists ([ratify-the-nvme-namespace-names](../design/roadmap/1002-ratify-the-nvme-namespace-names.md)).
2. The `no` groups of designware_mobile_storage (`DM2` to `DM4`, `DM6` to `DM8`, `DM10`): ratify as they stand, which clears the module apart from DM5 and DM9.

The groups that need a real ruling are the register and bit constants: `DE8`, `DE9`, `E7` and `E8`. Decide the designware_ethernet ones first, since #3094 set the pattern, then reuse it for e1000e.

## `designware_ethernet`

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
- Provisional recommendation: `WINDOW` is a plain word and stays as it is; `mac_hw_feature` abbreviates 'hardware', so recommend `mac_hardware_feature` (alias `mac_hw_feature`). `mac` stays an accepted acronym, as #3094 ruled.

## `designware_mobile_storage`

### DM2. Host controller (13)

`host_controller::Error`, `host_controller::Host`, `host_controller::Identity`, `host_controller::Registers`, `host_controller::card_clock`, `host_controller::clock_divider`, `host_controller::command`, `host_controller::into_registers`, `host_controller::new`, `host_controller::read`, `host_controller::registers`, `host_controller::reset`, `host_controller::write`

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

### E1. Module names (14)

`control`, `data_plane`, `descriptor`, `device`, `layout`, `pch`, `pch/flush`, `pch/hardware`, `pch/reset`, `pch/sequence`, `pch/tests`, `pch/ulp`, `proofs`, `tests`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify all but two. `pch` and `ulp` abbreviate 'platform controller hub' and 'ultra low power'; recommend `platform_controller_hub` and `ultra_low_power` (the stricter reading, a close call since Intel and Linux write both as letters; the sibling modules `non_volatile_memory` and `physical_layer_device` already spell out their words). The other twelve are plain.

### E2. Device identity (4)

`device::DEVICE_IDS`, `device::VENDOR_INTEL`, `device::is_supported`, `device::model`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand.

### E5. Descriptor status and command bits (7)

`descriptor::RECEIVE_ERRORS_FRAME`, `descriptor::RECEIVE_STATUS_DD`, `descriptor::RECEIVE_STATUS_EOP`, `descriptor::TRANSMIT_COMMAND`, `descriptor::TRANSMIT_STATUS_DD`, `descriptor::receive_verdict`, `descriptor::transmit_word`

- Breaks the spelled-out rule: yes.
- Provisional recommendation: Spell out `RECEIVE_STATUS_DD`, `RECEIVE_STATUS_EOP` and `TRANSMIT_STATUS_DD` (descriptor done, end of packet), keeping the Linux spelling as an alias; ratify the rest as they stand. These three are the only abbreviations in the module.

### E6. Buffer layout (10)

`layout::BYTES`, `layout::PAGES`, `layout::RECEIVE_BUFFERS`, `layout::RECEIVE_RING`, `layout::TRANSMIT_BUFFERS`, `layout::TRANSMIT_RING`, `layout::receive_buffer`, `layout::receive_descriptor`, `layout::transmit_buffer`, `layout::transmit_descriptor`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand; `rx` and `tx` already read `receive` and `transmit`.

### E7. Register offsets (24)

`registers::CTRL`, `registers::CTRL_EXT`, `registers::ICR`, `registers::IMC`, `registers::MTA`, `registers::RAH0`, `registers::RAL0`, `registers::RCTL`, `registers::RDBAH`, `registers::RDBAL`, `registers::RDH`, `registers::RDLEN`, `registers::RDT`, `registers::RFCTL`, `registers::STATUS`, `registers::TCTL`, `registers::TDBAH`, `registers::TDBAL`, `registers::TDH`, `registers::TDLEN`, `registers::TDT`, `registers::TIPG`, `pch::registers::FEXTNVM11`, `pch::registers::RXDCTL`

- Breaks the spelled-out rule: yes.
- Provisional recommendation: Spell out with the Linux `E1000_` spelling as alias (the docs already give each register's plain title, 'Device Control' and so on), and cite OpenBSD's em driver first if it names the register. One group because they share the pattern; the 2026-10-26 ruling for the `registers` module says the constants keep their sources' names, so the architect should say whether the 2026-11-02 rule overrides that for these.

### E8. Register bit constants and register-shaped helpers (12)

`control::CTRL_ASDE`, `control::CTRL_EXT_DRV_LOAD`, `control::CTRL_RST`, `control::CTRL_SLU`, `control::RAH_AV`, `control::RFCTL_EXTEN`, `control::STATUS_LU`, `control::rctl`, `control::tctl`, `control::tipg`, `pch::flush::FEXTNVM11_DISABLE_MULR_FIX`, `pch::flush::rxdctl`

- Breaks the spelled-out rule: yes.
- Provisional recommendation: Spell out with aliases, after the offsets: `CTRL_ASDE`, `CTRL_EXT_DRV_LOAD`, `CTRL_RST`, `CTRL_SLU`, `RAH_AV`, `RFCTL_EXTEN`, `STATUS_LU`, `FEXTNVM11_DISABLE_MULR_FIX`, and the functions `rctl`, `tctl`, `tipg`, `rxdctl` that build register values.

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

## `non_volatile_memory_express`

### N1. Namespace (3)

`lib::Namespace`, `lib::holds_block`, `lib::transfer_command`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Already proposed in full in design/roadmap/1002-ratify-the-nvme-namespace-names.md (ratify as they stand, plus the `size_bytes` to `capacity_bytes` field loose end). Do not repeat it; bring that proposal.

## Not covered

The lists hold names with no block or a provisional one. A name already ratified keeps its spelling until its own review. Cargo features carry no block and are not listed. A crate may carry further unratified names that `name_release.py` does not read, for example inside macro expansions.
