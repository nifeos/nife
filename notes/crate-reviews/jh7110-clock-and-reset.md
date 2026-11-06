# Review package: `jh7110_clock_and_reset`

*Name: provisional, minted by lane lane/crate-review-packages-five. The `jh7110_clock_and_reset`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the five packages
together outgrow the page.*

## In one paragraph

The StarFive JH7110's clock and reset generator as pure logic. It holds the register offsets and
bits of three domains (system, always-on, system top group) and the bring-up plans that ungate
clocks before releasing resets. It computes the I2C input clock, the SD card clock and the
standard-mode I2C timing. It also holds the strings by which the chip's two device trees (mainline
Linux and the vendor U-Boot) name each controller. It never touches a register; the caller maps the window and stores.
It is `no_std` with no `unsafe` and no dependency, and nine Kani harnesses prove its decoders and
the order of every plan. Every name has a ratified block, one of them by the maintainer pending the
architect, and each name follows the rules that spell out register names and units.

## Identity

| item | value |
|---|---|
| crate | `jh7110_clock_and_reset`, ratified, replacing `jh7110_crg`, and kept as the crates.io name (#1806) |
| version | `0.1.0`; no tag in this clone |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root |
| `publish` | `false` |
| size | `src/lib.rs` of 1,776 lines (about 1,530 of library, 240 of Kani harnesses) and `src/tests.rs` of 1,038 |
| consumers in nife | `kernel`, `machine_discovery` and `jh7110_watchdog`, by path |

## Public API

One module and 69 public items.

| group | items |
|---|---|
| words and bits | `CLOCK_ENABLE`, `CLOCK_PARENT_MASK`, `with_parent`, `clock_divider`, `with_divider`, `is_clock_enabled`, `is_deasserted` |
| domains | `Domain` (with `clock_offset` and `reset_bit`), `ResetBit`, `SYSTEM`, `ALWAYS_ON`, `SYSTEM_TOP_GROUP`, `SYSTEM_BASE`, the three `RegisterWindow` values (`SYSTEM_WINDOW`, `ALWAYS_ON_WINDOW`, `SYSTEM_TOP_GROUP_WINDOW`) and `WindowLocation::from_regions` |
| plans | `Step`, `ENTROPY_BRING_UP`, `GMAC0_SYSTEM_BRING_UP`, `GMAC0_ALWAYS_ON_BRING_UP`, `SDIO_BRING_UP`, `DevicePlan::from_specifiers`, `PowerManagementChipBus::from_nodes` |
| rates | `InterIntegratedCircuitClockWords`, `advanced_extensible_interface_configuration_0_hertz`, `inter_integrated_circuit_5_clock_frequency`, `one_based_divider`, `sdcard_divider_for`, `StandardMode`, `standard_mode_100_kilohertz` |
| device-tree strings | seven `COMPATIBLE_*` constants and `POWER_MANAGEMENT_CHIP_COMPATIBLES` |
| the PMIC reset | `POWER_MANAGEMENT_CHIP_RESET_REGISTER` (`0x32`) and `POWER_MANAGEMENT_CHIP_RESET_BIT` (6) |
| reporting | `BringUpReport` and its three questions |

No public signature names another crate's type. The caller looks phandles up through a closure, so
the crate reads no device tree.

What a stranger may find surprising:

- The crate is one chip's knowledge, and several items are one board's: the PMIC reset register
  and bit, the PMIC bus plan and the I2C5 clock chain belong to the VisionFive 2's AXP15060 wiring.
- `BringUpReport` is a bench transcript's record with 16 public fields, and it derives `Default`.
  A field added later breaks every caller that builds one by hand.
- No item is `#[non_exhaustive]`; the crate has no error enum, so this matters only for `Step`
  and the public structs.

## Proofs and falsification patches

Nine harnesses, each with one patch.

| harness | what it proves |
|---|---|
| `with_parent_changes_only_the_mux_field` | every bit outside the parent field is unchanged, and an over-wide parent never reaches the enable bit |
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
both numberings.

## Names

`python3 helpers/name_release.py --draft jh7110_clock_and_reset` lists 71 names, each with a
ratified block. Two things still need the architect.

The maintainer's ruling. `CLOCK_PARENT_MASK` replaced `CLOCK_MUX_MASK`, and its block says it stays
provisional until an architect reviews it. Recommendation: ratify it; it names what the field holds,
as `with_parent` and `Step::SelectParent` already do.

A leftover of that ruling. The ruling refused `mux` as an abbreviation of multiplexer, yet three
`BringUpReport` fields keep it (`mux_before`, `mux_after` and `had_mux`). Recommendation:
`select_parent_before`, `select_parent_after` and `had_select_parent`, which reuse the step's name
the way `had_reset` reuses `DeassertReset`.

Every other name follows two rules: a register or field takes its spelled-out name, with the
source's spelling in `doc(alias)`, and a unit takes its full word.

| names | what they are |
|---|---|
| `advanced_extensible_interface_configuration_0_hertz`; parameters `parent_hertz`, `want_hertz` | a rate and its inputs |
| `standard_mode_100_kilohertz` | the 100 kHz timing |
| `StandardMode { control, high_count, low_count, data_hold }` | register values, each with the databook's name (`IC_CON` and peers) in `doc(alias)` |
| `InterIntegratedCircuitClockWords` | raw words read from the clock generator and the system controller; the phase-locked-loop words are spelled out (`phase_locked_loop_2_pre_divider_word` and its two siblings) |
| `SYSTEM_CLOCK_BUS_ROOT` and the other `SYSTEM_CLOCK_*` ids, `SYSTEM_TOP_GROUP_RESET_SECURITY_ADVANCED_HIGH_PERFORMANCE_BUS` | clock and reset ids, with the device-tree binding header's own names kept as `doc(alias)` |
| `SYSTEM_CONTROLLER_BASE_PHYSICAL_ADDRESS`, `SYSTEM_CONTROLLER_SIZE_BYTES` | the system controller's window, with `syscon` in `doc(alias)` |
| `COMPATIBLE_VENDOR_CLOCK_GENERATOR`, `COMPATIBLE_VENDOR_RESET_GENERATOR` | the vendor tree's compatible strings |
| `MAXIMUM_REGISTER_ENTRIES`, `MAXIMUM_SD_CARD_DIVIDER` | bounds |

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair.
Every number is a fact read from Linux mainline (`torvalds/linux` at `cbbddb6b08cc`) and the vendor
U-Boot (tag `VF2_v2.10.4`, commit `1ae835c25e3d`), with each link pinned. Milestone 1019 (the driver
crates carry facts from their sources, not their text) item 1 rewrote `standard_mode_100_kilohertz` from the
I2C specification's figures alone. OpenSBI's `jh7110.c` (BSD-2-Clause) holds the same PMIC reset
register and bit, which the crate takes from a bench reading; a citation would do, and no notice is
owed for a fact.

## What the move still needs

| item | owner |
|---|---|
| The doc comment on `Step` quotes eleven words of a comment in Linux's `reset-starfive-jh71x0.c`, a GPL source. The five-word rule of the `jh7110_entropy` audit, which milestone 1019 item 8 applies, keeps attributed quotations shorter; paraphrase it and keep the citation. | maintainer, provisional under that precedent |
| Item 11 of milestone 1019 (pin the vendor links): the crate now names the tag and the branch head separately for the 50 MHz claim. The milestone block still lists it as outstanding. | maintainer, to confirm and close |
| Shipped text names radon 15 times and cites "the driver's own `POLL_LIMIT`", which is the caller's. Say "a VisionFive 2" and "the caller's poll limit". | move lane |
| `repository` in the manifest. | move lane |
| The PMIC reset path with the rewritten `standard_mode_100_kilohertz` (198 high and 297 low counts at 49.5 MHz) has not run on a board, so milestone 819 (JH7110 clock and reset logic, proven, then released on its own) does not call it proven. | maintainers, at the bench |

## Proposed items

No defect found in the logic. The quotation above is the one text item that a rule refuses.

## Open questions

1. Ratify `CLOCK_PARENT_MASK` and rename the three `mux` fields. **architect** (names).
2. Whether `BringUpReport` belongs in the published API or in the caller that writes the bench
   transcript. Recommendation: keep it public and mark it `#[non_exhaustive]`, so fields can be
   added; it records the order of a bring-up, which only this crate knows. The maintainer can rule
   it provisionally.

## Result of `script/crate-standalone`

Clean: manifest, files, citations, names, the build on 1.88 for the host and the three bare-metal
targets, all nine harnesses and all nine falsifications. The names step passes because
`CLOCK_PARENT_MASK`'s block reads ratified, though the ruling behind it is the maintainer's.

## Recommendation

Blocked by question 1 and the unrun bench check. After both, ready for review.
