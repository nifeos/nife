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
its decoders and the order of every plan. Every name has a ratified block, one of them by the
maintainer pending the architect. The crate's text, README, manifest and falsification patches name
no other project of this tree.

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
| words and bits | `CLOCK_ENABLE`, `CLOCK_PARENT_MASK`, `with_parent`, `clock_divider`, `with_divider`, `is_clock_enabled`, `is_deasserted` |
| domains | `Domain` (with `clock_offset` and `reset_bit`), `ResetBit`, `SYSTEM`, `ALWAYS_ON`, `SYSTEM_TOP_GROUP`, `SYSTEM_BASE`, the three `RegisterWindow` values and `WindowLocation::from_regions` |
| plans | `Step`, `ENTROPY_BRING_UP`, `GMAC0_SYSTEM_BRING_UP`, `GMAC0_ALWAYS_ON_BRING_UP`, `SDIO_BRING_UP`, `DevicePlan::from_specifiers`, `PowerManagementChipBus::from_nodes` |
| rates | `InterIntegratedCircuitClockWords`, `advanced_extensible_interface_configuration_0_hertz`, `inter_integrated_circuit_5_clock_frequency`, `one_based_divider`, `sdcard_divider_for`, `StandardMode`, `standard_mode_100_kilohertz` |
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

Provisional. The maintainer ratified `CLOCK_PARENT_MASK`, pending the architect. It says what
the field holds, as `with_parent` and `Step::SelectParent` already do. The ruling refused
`CLOCK_MUX_MASK`, `CLOCK_MULTIPLEXER_MASK` and `CLOCK_PARENT_SELECT_MASK`. Recommendation: ratify.

Ratified earlier, failing a later rule. The architect's ruling keeps each until a name review, and
a release is that review, because names become permanent on first publication. This lane renames
none. `helpers/name_checks.py` finds the first only.

| name | rule | recommended rename |
|---|---|---|
| `SDIO_BRING_UP` | `SDIO` is not an accepted acronym | `SD_INPUT_OUTPUT_BRING_UP`, as `SYSTEM_CLOCK_SD_INPUT_OUTPUT_SD_CARD` already spells it |
| `BringUpReport::mux_before`, `mux_after`, `had_mux` | the ruling on `CLOCK_PARENT_MASK` refused `mux` | `select_parent_before`, `select_parent_after`, `had_select_parent`, reusing the step's name as `had_reset` reuses `DeassertReset` |
| `SYSTEM_BASE` | an address says virtual or physical | `SYSTEM_BASE_PHYSICAL_ADDRESS` |
| `RegisterWindow::base`, `WindowLocation::base` | the same; both hold physical addresses | `base_physical_address` |
| `PowerManagementChipBus::controller` | an `(address, size)` pair with no word for either | `controller_physical_window` |
| the parameter `w` of `inter_integrated_circuit_5_clock_frequency` and `advanced_extensible_interface_configuration_0_hertz` | a one-letter parameter in a public signature | `words` |
| `sdcard_divider_for` | `SD` stays but `sdcard` fuses two words, while `MAXIMUM_SD_CARD_DIVIDER` separates them | `sd_card_divider_for` |

Public names the sweeps accept, checked: `GMAC0_*` (`GMAC` stays), the `SD_CARD` spellings, `id`,
`from_tree` (a device tree is the tree), and the hertz, kilohertz and bytes suffixes. No public name
carries a unit symbol. The unit gate (`vendor/crate-workflows/helpers/unit_symbols.py`) reports 13
names in this crate, all private: `OSC_HZ`, `PLL2_DEFAULT_HZ`, four `STANDARD_*` constants,
`period_ns`, one test parameter, and `STGCLK_SEC_AHB`, `STGCLK_SEC_MISC_AHB` and three test
constants, where `SEC` means security and not seconds. The unit-suffix sweep lane owns all 13; the
five `SEC` names are a false positive of the gate that wants an exemption or a spelled-out
`SECURITY`. No alias of an old nife spelling exists. The aliases are the sources' spellings, as
`CLKMUX_MASK` and `JH7110_MUX_MASK` are for `CLOCK_PARENT_MASK`.

The register-field names `phase_locked_loop_2_dacpd_dsmpd_and_feedback_divider_word` and its two
siblings keep `DACPD`, `DSMPD` and `FBDIV`, which the vendor PLL code uses. Nothing read here
expands the first two; the docs do not claim a source does.

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

Clean on `main` before the edits and clean after them (manifest, files, citations, names, the build
on 1.88 for the host and the three bare-metal targets, all nine harnesses and all nine
falsifications). `cargo test -p jh7110_clock_and_reset` passes 45 tests and 2 doctests. The tests
of `machine_discovery` and `jh7110_watchdog` pass, and the kernel checks for `riscv64imac` with and
without `soak_test`.

## Recommendation

Ready for your review and clearance, with one limit stated in the README: the PMIC reset path has
not run on a board.

## Outstanding before the move

| item | owner |
|---|---|
| Decisions 1 to 4 below. | architect |
| Mark milestone 1019 items 7 and 11 done for this crate. | maintainer |
| The 13 private unit-symbol names, and the exemption for `SEC` meaning security. | unit-suffix sweep lane |
| Run the PMIC reset path on a board, then lift the README's limitation. | maintainers, at the bench |
| `repository` in the manifest, the `v0.1.0` tag, and stripping the `Name:` blocks. | move lane |
| The stranger test in the new repository. | move lane |

## Decisions for the architect

1. Clear now, or wait for the board run. Recommendation: clear now, with the README's limitation
   standing; the proofs and host tests do not depend on the board, and the board run follows the
   move. Background: the crate computes the I2C timing (198 high and 297 low counts at 49.5 MHz)
   that makes a VisionFive 2 reset itself, and no board has run that timing yet.
2. Ratify `CLOCK_PARENT_MASK`. Recommendation: ratify. Background: the maintainer chose it over
   three alternatives on 2026-11-05 and left it provisional for you.
3. Reopen the seven names in the table above. Recommendation: rename all before `0.1.0` in one
   pull request. Background: a later rule overtook each ruling, and names become permanent when the
   crate first publishes.
4. Keep the VisionFive 2 board items (the PMIC reset, the PMIC bus plan, the I2C5 clock chain) in
   this chip crate. Recommendation: keep them, as the README says they are board facts. Background:
   moving them to a board crate later would break every caller.
