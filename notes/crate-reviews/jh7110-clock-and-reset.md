# Review package: `jh7110_clock_and_reset`

*Name: provisional, minted by lane lane/crate-review-packages-five. The `jh7110_clock_and_reset`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the five packages
together outgrow the page.*

## In one paragraph

The StarFive JH7110's clock and reset generator as pure logic. It holds the register offsets and
bits of three domains (system, always-on, system top group) and the bring-up plans that ungate
clocks before releasing resets. It computes the I2C input clock, the SD card clock and the
standard-mode I2C timing. It also holds the strings by which the chip's two device trees (mainline
Linux and the vendor U-Boot) name each controller. It never touches a register; the caller maps the
window and stores. It is `no_std` with no `unsafe` and no dependency, and nine Kani harnesses prove
its decoders and the order of every plan. Every name has a ratified block. The crate's text,
README, manifest and falsification patches name no other project of this tree.

## Identity

| item | value |
|---|---|
| crate | `jh7110_clock_and_reset`, ratified, replacing `jh7110_crg`, and kept as the crates.io name (#1806) |
| version | `0.1.0`; no tag in this clone |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root |
| `publish` | `false`; no `repository` key yet |
| size | `src/lib.rs` of 1,798 lines and `src/tests.rs` of 1,037 |
| consumers in nife | `kernel`, `machine_discovery` and `jh7110_watchdog`, by path |

## Public API

One module and 69 public items (36 constants, 22 functions, 10 structs and the `Step` enum).

| group | items |
|---|---|
| words and bits | `CLOCK_ENABLE`, `CLOCK_MULTIPLEXER_MASK`, `with_parent`, `clock_divider`, `with_divider`, `is_clock_enabled`, `is_deasserted` |
| domains | `Domain` (with `clock_offset` and `reset_bit`), `ResetBit`, `SYSTEM`, `ALWAYS_ON`, `SYSTEM_TOP_GROUP`, `SYSTEM_BASE_PHYSICAL_ADDRESS`, the three `RegisterWindow` values and `WindowLocation::from_regions` |
| plans | `Step`, `ENTROPY_BRING_UP`, `GMAC0_SYSTEM_BRING_UP`, `GMAC0_ALWAYS_ON_BRING_UP`, `SD_INPUT_OUTPUT_BRING_UP`, `DevicePlan::from_specifiers`, `PowerManagementChipBus::from_nodes` |
| rates | `InterIntegratedCircuitClockWords`, `advanced_extensible_interface_configuration_0_hertz`, `inter_integrated_circuit_5_clock_frequency`, `one_based_divider`, `sd_card_divider_for`, `StandardMode`, `standard_mode_100_kilohertz` |
| device-tree strings | seven `COMPATIBLE_*` constants and `POWER_MANAGEMENT_CHIP_COMPATIBLES` |
| the PMIC reset | `POWER_MANAGEMENT_CHIP_RESET_REGISTER` (`0x32`) and `POWER_MANAGEMENT_CHIP_RESET_BIT` (6) |
| reporting | `BringUpReport` and its three questions |

No public signature names another project's type, and the caller looks phandles up through a
closure, so the crate reads no device tree. The crate has no error type, so there is nothing to
implement `Display` or `core::error::Error` for.

Provisional ruling by the maintainer, for the architect to keep or reverse. `Step`,
`BringUpReport`, `PowerManagementChipBus`, `DevicePlan` and `WindowLocation` are
`#[non_exhaustive]`: the restrictive option, because adding the marker later breaks callers and
removing it never does. A caller matches a `Step` with a wildcard arm and builds a `BringUpReport`
from `Default`; the kernel's four `Step` matches and its one report constructor already do. The
plain-data tables (`Domain`, `ResetBit`, `RegisterWindow`, `WindowSpelling`, `StandardMode` and
`InterIntegratedCircuitClockWords`) stay exhaustive because a caller builds the last one by hand
and the rest mirror fixed hardware layouts.

What a stranger may find surprising: the crate is one chip's knowledge, and several items are one
board's. The PMIC reset register and bit, the PMIC bus plan and the I2C5 clock chain belong to the
VisionFive 2's AXP15060 wiring. The README says so.

## Proofs and falsification patches

Nine harnesses, each with one patch.

| harness | what it proves |
|---|---|
| `with_parent_changes_only_the_mux_field` | every bit outside the parent field stays as it was, and an over-wide parent never reaches the enable bit |
| `a_clock_offset_is_one_of_the_domains_own_words` | for every domain and index, an offset is word-aligned and inside the domain, or `None` past its count |
| `a_reset_line_is_one_bit_of_one_of_the_domains_own_words` | a reset is exactly one bit, in assert and status words of the domain |
| `every_fixed_plan_ungates_its_clocks_before_releasing_a_reset` | each fixed plan holds only its domain's ids, enables every clock before releasing any reset, and picks a parent before enabling that clock |
| `a_device_plan_from_specifiers_ungates_before_it_releases` | a plan built from device-tree bytes holds in-bounds ids, stays within its bound and orders clocks first |
| `the_pmic_bus_plan_is_never_empty_and_ungates_before_it_releases` | the same for the PMIC bus plan, which is never empty |
| `is_deasserted_reads_the_bit_the_mask_names` | the status test reads exactly the masked bit |
| `is_clock_enabled_reads_bit_31_and_nothing_else` and `clock_divider_is_the_low_24_bits_and_nothing_else` | each reads exactly its own bit or field, for every word |

Not proved, and the README says so: that a plan's order is what the silicon needs, the specifier
decoders when the lookup answers differently, plans of more than three specifiers, and the I2C
timing and SD divider arithmetic. 45 host tests cover those by example and pin every offset against
both numberings. The README's usage example runs as a doctest.

## Names

`python3 helpers/name_release.py --draft jh7110_clock_and_reset` lists 80 names (the crate, the
test module and 78 items and fields), each with a ratified block.

The architect ratified `CLOCK_MULTIPLEXER_MASK` on 2026-11-11 (UTC): OpenBSD's `CLKMUX_MASK`
spelled out, with the three source spellings as aliases. `with_parent` and `Step::SelectParent`
keep "parent" for what the field holds.

Ratified names a later rule reached, renamed and ratified on 2026-11-08 (UTC) before the first
release, because names become permanent on first publication.

| was | rule | now |
|---|---|---|
| `SDIO_BRING_UP` | `SDIO` is not an accepted acronym | `SD_INPUT_OUTPUT_BRING_UP`, as `SYSTEM_CLOCK_SD_INPUT_OUTPUT_SD_CARD` already spells it |
| `BringUpReport::mux_before`, `mux_after`, `had_mux` | `mux` abbreviates multiplexer | `select_parent_before`, `select_parent_after`, `had_select_parent` |
| `SYSTEM_BASE` | an address says virtual or physical | `SYSTEM_BASE_PHYSICAL_ADDRESS` |
| `RegisterWindow::base`, `WindowLocation::base` | the same; both hold physical addresses | `base_physical_address` |
| `PowerManagementChipBus::controller` | an `(address, size)` pair with no word for either | `controller_physical_window` |
| `sdcard_divider_for` | `sdcard` fuses two words, while `MAXIMUM_SD_CARD_DIVIDER` separates them | `sd_card_divider_for` |

Parameter names of public functions, spelled out. They are provisional: a parameter name is no
part of a Rust signature's compatibility, so the maintainer rules them, pending the architect.

| parameter | of | now |
|---|---|---|
| `w` | `inter_integrated_circuit_5_clock_frequency`, `advanced_extensible_interface_configuration_0_hertz` | `words` |
| `power_management_chip_reg`, `bus_reg` | `PowerManagementChipBus::from_nodes` | `power_management_chip_register_property`, `bus_register_property`: `reg` clips the property's meaning, and a parameter cannot carry the kept form |
| `reg_names` | `WindowLocation::from_regions` | `register_names` |

## Naming rules

The template's checks at the commit the moved crate repositories pin (`cd1b484`), run as its check
job runs them over the tree `script/crate-move --dry-run` stages:

| rule | result |
|---|---|
| spelled-out words (`spelled_out.py`) | 7 findings. The private `SYSCLK_SDIO*`, `SYSRST_SDIO*` and `AONCLK_GMAC0_TX` constants open their doc comment with `(NAME, source)`, and the gate keeps no source spelling on an integer constant. |
| register rules (`register_names.py`) | 8 findings: five `len` and `size` fields with no unit, the public field `PowerManagementChipBus::power_management_chip_address` (an I2C bus address, with no `bus` qualifier) and two test names that say `addresses`. |
| acronyms in prose (`prose_acronyms.py`) | 28 findings, each an acronym in a doc comment or the README not expanded at its first use. |
| README prose caps and passives | pass. |
| units spelled out (`unit_symbols.py`) | 0 findings in 456 names. The checker reads suffixes only. |
| question shape (`question_shape.py`) | 0 findings. |
| British spelling | 0 findings. |
| aliases of an old nife spelling | none. Every `doc(alias)` is a specification, binding or databook term. |

The word list reaches only the words it lists. The sweep also read every identifier for clipped
and decoder-needing words. Renamed, all private:

| was | now |
|---|---|
| `Spec`, `spec`, `specs` | `Specifier`, `specifier`, `specifiers` |
| `OSC_HERTZ`, `PLL2_DEFAULT_HERTZ` | `OSCILLATOR_HERTZ`, `PHASE_LOCKED_LOOP_2_DEFAULT_HERTZ` |
| `parse_reg`, the test constant `BUS_REG` | `parse_register_property`, `BUS_REGISTER_PROPERTY` |
| `sys_provider` | `system_provider` |
| `VENDOR_CLKGEN_*`, `VENDOR_RSTGEN_*` | `VENDOR_CLOCK_GENERATOR_*`, `VENDOR_RESET_GENERATOR_*` |
| `ALWAYS_ON_BASE`, `SYSTEM_TOP_GROUP_BASE` and the three `*_SIZE` constants | `*_BASE_PHYSICAL_ADDRESS` and `*_SIZE_BYTES` |
| `STANDARD_MODE_CON` | `STANDARD_MODE_CONTROL` |
| the locals `fbdiv`, `prediv`, `prediv_raw`, `postdiv1`, `hcnt`, `lcnt`, `minimum_lcnt`, `u32be`, `div` | `feedback_divider`, `pre_divider`, `pre_divider_raw`, `post_divider_1`, `high_count`, `low_count`, `minimum_low_count`, `big_endian_word`, `divider` |
| the test locals `osc`, `pll2`, `frac`, `stg`, `aon`, `sys`, `rst` | words that say what they hold |
| the harness `the_pmic_bus_plan_is_never_empty_and_ungates_before_it_releases` and its patch | the same with `power_management_chip` for `pmic` |
| eight test names with `pmic`, `i2c5`, `osc`, `pll2`, `sys`, `sdio` or `syscrg` | the same with each word spelled out |

Every patch still applies and fails its own harness.

Borderline private names, kept as they are: decided by the maintainer, provisional (2026-11-11 UTC),
since none is public. The bus standards stay abbreviated only inside a device-tree clock or reset
id, as ruled on 2026-11-05 (UTC). The template gate's seven findings above are the rename lane's:

| names | why |
|---|---|
| `SYSCLK_*`, `SYSRST_*`, `AONCLK_*`, `AONRST_*`, `STGCLK_*` (about 20 constants) | `CLK` and `RST` are clipped, but each is a dt-binding identifier minus its `JH7110_` prefix, and the doc comment cites the binding. Only the eight the gate flagged are in the kept form. |
| the `PLL`, `TRNG`, `AHB`, `AXI`, `APB`, `PTP`, `GTXC`, `RMII` and `RTX` components of constants, tests and fields | each is a hardware term of art that the word list does not name, and each sits in a binding's id. |
| `AONCLK_GMAC0_TRANSMIT_PARENT_RMII_RTX` | `RMII_RTX` is the mainline clock's own name, quoted. |
| the locals `dacpd` and `dsmpd` | register-field names the vendor PLL code uses; nothing read here expands them. |

Public names the sweeps accept, checked: `GMAC0_*` (`GMAC` stays), the `SD_CARD` spellings, `id`,
`from_tree` (a device tree is the tree), and the hertz, kilohertz and bytes suffixes. No public name
carries a unit symbol. The register-field names
`phase_locked_loop_2_dacpd_dsmpd_and_feedback_divider_word` and its two siblings keep `DACPD`,
`DSMPD` and `FBDIV`, which the vendor PLL code uses; nothing read here expands the first two.

## License and provenance

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair.
Every number is a fact read from Linux mainline (`torvalds/linux` at `cbbddb6b08cc`) and the vendor
U-Boot (tag `VF2_v2.10.4`, commit `1ae835c25e3d`). OpenSBI's `jh7110.c` (BSD-2-Clause) holds the
same PMIC reset register and bit, which the crate takes from a board reading; its doc comment now
cites that file.

Milestone 1019 (the driver crates carry facts from their sources, not their text), as far as this crate goes:

- Item 1 (`standard_mode_100_kilohertz` from the I2C specification alone) stands done.
- Item 7. The two PMIC doc comments cite the node, the file and the revision and give the string
  and address as facts. Neither reproduces a run of node text.
- Item 11. Every vendor claim now links the revision it holds at. The 50 MHz card clock holds in
  mainline and at the vendor branch head, and not at `VF2_v2.10.4`; `u5_dw_i2c_clk_core` is a gate
  on `osc` at `VF2_v2.10.4` and absent at the branch head. The crate documentation carries the
  links.
- The doc comment on `Step` and a test comment quoted eleven words of a GPL comment in Linux's
  `reset-starfive-jh71x0.c`; both now describe the warning in the crate's own words and cite the
  file. A third doc quoted four words of it and is rewritten too.

What I verified and what I did not, on 2026-11-06 (UTC):

| claim | how |
|---|---|
| All 20 distinct URLs in `lib.rs` and the README return 200 | `curl` on each |
| Tag `VF2_v2.10.4` is commit `1ae835c25e3d`; the branch head is `c4c67bb66ae6`; the Linux commit exists | `gh api` |
| The PMIC node, `starfive_visionfive2.dts` at both vendor revisions: 50 MHz assigned at the head, none at the tag | fetched both files and read them |
| `u5_dw_i2c_clk_core` registered at the tag and not at the head; id 298 and `JH7110_CLK_SYS_REG_END` 190 | fetched both drivers and the header |
| Mainline's `jh7110-common.dtsi` assigns 50 MHz to both controllers and names `pmic@36` | fetched |
| `SYS_SYSCON_BASE 0x13030000` and `SYS_CRG_BASE 0x13020000`; `reset_release_all` asserts, then frees; `DM_FLAG_OS_PREPARE`; `jh7110_gmac_sel_tx_to_rgmii`; Linux's mask, enable and reset offsets | fetched and read |
| A doc comment gave the vendor header's macro as the crate's own spelled-out constant | a defect, fixed: the header's macro is `SYS_SYSCON_BASE`, now also an alias |
| OpenSBI's reset register, OpenBSD's and FreeBSD's spellings and the board EEPROM revision | not re-checked; carried from earlier lanes |

## Result of `script/crate-standalone`

Clean on 2026-11-11 (UTC), with Kani: the manifest, files, citations, names, the build on 1.88 for
the host and the three bare-metal targets, all nine harnesses and all nine falsifications. It runs
none of the template's naming and prose gates above. `cargo test -p jh7110_clock_and_reset` passes
45 tests and 2 doctests.

## Recommendation

Cleared to move, with one limit stated in the README: the PMIC reset path has not run on a board.
The move waits for the template gates above.

## Outstanding before the move

| item | owner |
|---|---|
| The 43 template gate findings above, in this tree: a rename of a ratified public name goes to the architect as one table; private names, parameters, tests and doc comments are the maintainer's provisional calls. | a rename lane |
| Run the PMIC reset path on a board, then lift the README's limitation. | maintainers, at the bench |
| `repository` in the manifest, the `v0.1.0` tag, and stripping the `Name:` blocks. | move lane |
| The stranger test in the new repository. | move lane |

## Clearance

The architect cleared the review on 2026-11-11 (UTC), with the board run following the move as the
README's stated limitation. Clearing approves creating `nifeos/jh7110_clock_and_reset`; the tag and
a publication stay uncleared, and the move waits for the template gates.

Decided by the maintainer, provisional (2026-11-11 UTC), and reversible until the first
publication:

- The VisionFive 2 board items (the PMIC reset register and bit, the PMIC bus plan, the I2C5 clock
  chain) stay in this chip crate, where the README names them as one board's facts. A board crate
  can take them before 0.1.0 publishes; after that, moving them breaks every caller.
- `Step`, `BringUpReport`, `PowerManagementChipBus`, `DevicePlan` and `WindowLocation` stay
  `#[non_exhaustive]`, as the API section says.
