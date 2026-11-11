---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1019, 1062
decision_dependencies: 320
machine_requirements: none for the items; a VisionFive 2 for the board questions the review leaves open
specific_machine: none
needs_person: yes
---
# 1072. `jh7110_clock_and_reset`: what an outside maintainer would fix before the move

Raised by a read-only review of `crates/jh7110_clock_and_reset` on `main`, read together with the
diff of pull request #3475 (milestone 1062 step 2, the clock and reset server), as a stranger to
this tree would meet the crate in its own repository. It changes nothing; it lists what the next
lanes change.

The review took its baseline from [the review package](../../notes/crate-reviews/jh7110-clock-and-reset.md)
and reopens none of the decisions it settles, except where a later rule now contradicts one and this
block says so: the crate name, the ratified public names, the `#[non_exhaustive]` markers, the dual
license, and the decision to keep the VisionFive 2 board items in the chip crate. Pull request
#3475 touches the crate in one place: `POWER_MANAGEMENT_CHIP_BUS_BRING_UP` becomes `pub` with a
provisional name, and two new crates (`jh7110_clock_and_reset_contract`,
`jh7110_clock_and_reset_devices`) become its callers. The review read that diff; it did not build it.

Reuse: the template `nifeos/crate-workflows` and its helpers, Kani, cargo-mutants, cargo-llvm-cov,
the crate's own harness helpers (`any_domain`, `ungates_before_releasing`, `system_provider`) and
the pinned Linux and vendor U-Boot sources the crate cites. No new tool.

## Goal

A stranger who finds `jh7110_clock_and_reset` in its own repository can open a pull request and see
every template check green for the crate's own reasons. The same stranger can tell which facts are
the chip's and which are one board's, can decode a `reg` property of either common cell layout
without a misread, and can trust the recorded mutation results because they describe the code that
ships.

## What was checked, and how

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at the head of `main`, with the crate
copied out of the workspace into a directory of its own, as `script/crate-standalone` does:

- `cargo +1.88 test`: 45 unit tests and 2 documentation tests pass. `cargo +1.85 test` refuses the
  manifest, so `rust-version = "1.88"` is the true floor (let chains).
- `cargo +stable build --lib` for `riscv64imac-unknown-none-elf`, `aarch64-unknown-none-softfloat`
  and `x86_64-unknown-none`: clean. `cargo doc` with broken intra-doc links and invalid HTML tags
  denied: clean.
- `cargo clippy --all-targets -- -D warnings` on stable and on 1.88: **17 errors**, all
  `clippy::doc_markdown` (item 1). The same with the template's Kani shim and `--cfg kani`: the
  same 17 and nothing from the proof module.
- Kani 0.68.0, the version in the template's `.kani-version`: all nine harnesses verify; 3 minutes
  59 seconds wall, 838 MB peak resident, on one host. The two falsification patches read
  (`every_fixed_plan...`, `a_device_plan_from_specifiers...`) each invert the property their header
  names; the other seven were not replayed.
- cargo-llvm-cov: `src/lib.rs` 307 of 311 lines, 98%, over the template's per-file floor of 85.
- `cargo mutants --list`: 261 mutants. No census was run (item 5 says why).
- The template's helpers, as `check / standalone` runs them: `standalone.py` (three findings, all
  expected of a crate still in nife: `publish = false`, the `Name:` blocks, one line naming the
  architect inside a `Name:` block); `spelled_out.py` 7 refusals and `register_names.py` 8, both at
  ceiling 0 (item 2); `unit_symbols.py`, `question_shape.py`, `british_spelling.py`,
  `service_model.py`, `contract_names.py` 0; `prose_acronyms.py` 28 refusals at ceiling 0 (item 2);
  `prose_figures.py`, `history_phrases.py`, `file_length.py`, `comment_blocks.py`,
  `function_length.py`, `inflated_wording.py`, `readme_sections.py`, `readme_acronyms.py` clean;
  `prose_caps.py` one 62-word sentence over the 40-word cap and `passive_voice.py` 8 passives in
  876 words against an allowance of 1 (item 14); `abbreviations.py` one component, `jh`;
  `readme_badges.py` four findings, all wanting the repository that does not exist yet; `typos`
  clean over the source.
- Against the pinned Linux commit the crate cites: the PLL2 field positions (`DACPD` bit 15,
  `DSMPD` bit 16, `FBDIV` bits 28:17 at `0x2c`, `POSTDIV1` bits 29:28 at `0x30`, `PREDIV` at `0x34`),
  the PLL rate formula, the divider rule (`DIV_ROUND_UP` then clamp to `max_div`), `i2c5`'s `reg`
  and cells, `gmac0`'s clocks and resets, and the two board files' `&gmac0` blocks (item 4).
- The DesignWare count formulas in Linux's `i2c-designware-common.c` (item 8).
- The comparable crates on crates.io, through its API (prior art).

Inferred from reading, not run: the behavior of `parse_register_property` on a 12-byte property
(item 3), the `name_index` miscount (item 9) and the fallback asymmetry (item 10).

Not run: a mutation census (outside its window), `cargo semver-checks` (no release to compare
against), Miri, the stranger test, a build of pull request #3475. Not verified: the DesignWare
databook's minimum count values (item 8), OpenBSD's and FreeBSD's spellings beyond what the crate
cites, and whether a VisionFive 2 v1.2A wants the transmit parent the plan selects (item 4).

## Prior art

How the crate sits against what a reader already knows. Downloads are crates.io's "recent" figure
at the time of the review.

| where | shape | what a reader coming from it will miss here |
|---|---|---|
| Linux `clk-starfive-jh7110-{sys,aon,stg}.c`, `reset-starfive-jh71x0.c` | one table entry per clock (`JH71X0__GATE`, `_GDIV`, `_GMUX`), one word per clock, the reset controller an auxiliary device of the clock driver; a divider is `DIV_ROUND_UP(parent, rate)` clamped to `max_div`; a parent choice is board data (`assigned-clock-parents`) | set-rate for every divider, the PLL as a clock with presets, the ISP and VOUT domains, fractional PLL rates |
| mainline and vendor device trees | the bus's `clocks`/`resets` name what a device needs | nothing: `DevicePlan::from_specifiers` reads the same two properties |
| OpenBSD `stfclock.c`, FreeBSD `jh7110_clk.c` (as the crate cites them; not re-read) | the same per-clock word, a narrower or wider mux mask | the mux width disagreement the crate documents at `CLOCK_PARENT_MASK` |
| vendor U-Boot `clk-jh7110.c`, `starfive_visionfive2.c` | the rate chain the crate transcribes; the board file hard-codes the GMAC0 transmit parent | the crate makes the same board choice a chip constant (item 4) |
| OpenSBI `platform/generic/starfive/jh7110.c` | the PMIC reset register and bit | the same numbers, cited |
| `jh71xx-pac` 0.11.1 (819), `jh71xx-hal` 0.7.2 (743) | svd2rust register access to every block, a HAL above it | no bring-up order, no proofs, no device-tree spellings; register access this crate leaves to the caller |
| `jh7110-vf2-12a-pac`, `jh7110-vf2-13b-pac` 0.2.0 (6 each) | one package per board revision | the ecosystem already splits v1.2A from v1.3B, which item 4 and decision 5 are about |
| `starfive-jh7110-dwmmc` 0.1.8 (670), `axplat-riscv64-visionfive2` 0.1.4 (386) | a card driver and a platform layer for the same board | the card clock arithmetic this crate holds |

Where this crate differs: pure logic with no register access, plans whose order nine Kani harnesses
prove, both trees' spellings, bounds that turn a wrong-domain id into `None` rather than a store, and
host tests that pin every offset against both numberings. What it lacks: a fractional PLL2 rate,
any domain beyond STG, AON and SYS, and a line between the chip's facts and the board's.

## The items

Each item: what is wrong, where, why it matters, the fix, effort (S under a day, M a few days, L
more), and whether the door is one-way (a published name, format, identity, layout or dependency)
or two-way. Line numbers are `main`'s; pull request #3475 shifts `src/lib.rs` by two lines after
line 413.

### P0: the repository cannot go green

1. **`check / clippy` is red on the first pull request.** `cargo clippy --all-targets -- -D
   warnings` reports 17 `doc_markdown` errors, 26 words: `VisionFive` (14), `OpenSBI` (9),
   `AXP15060` (2) and `PLL2` (1), at `src/lib.rs:25`, `:234`, `:352`, `:356`, `:402`, `:410`, `:489`,
   `:525`, `:981`, `:1024`, `:1109`, `:1111`, `:1391` and `src/tests.rs:643`. Inside nife the root
   `clippy.toml` lists these proper nouns under `doc-valid-idents`; the copied crate has no such file
   and the template's scaffold ships none, so the template's required check fails on stable and on
   1.88 alike. Fix: ship a `clippy.toml` with `doc-valid-idents = ["..", "VisionFive", "OpenSBI",
   "AXP15060", "PLL2"]`, for the reason nife's own file gives (a proper noun in backticks claims to
   be an identifier), or put the 26 words in backticks. S. Two-way.

2. **`check / standalone` is red at three ceiling-0 steps.**
   - `spelled_out.py` refuses seven private constants: `AONCLK_GMAC0_TX` (`src/lib.rs:233`, `tx`) and
     `SYSCLK_SDIO0_AHB`, `SYSCLK_SDIO1_AHB`, `SYSCLK_SDIO0_SDCARD`, `SYSCLK_SDIO1_SDCARD`,
     `SYSRST_SDIO0_AHB`, `SYSRST_SDIO1_AHB` (`:422`, `:427`, `:438`, `:443`, `:448`, `:452`, `sdio`).
     The review package recorded 0 findings for these after they gained a `(NAME, source)` opening
     and a `doc(alias)`; the template's checker now holds that an integer constant never keeps its
     source spelling, so a later rule reached them. The word list's `sdio` entry asks for a name for
     what the controller is. Provisional names, for the maintainer: `ALWAYS_ON_CLOCK_GMAC0_TRANSMIT`,
     and `SYSTEM_CLOCK_MOBILE_STORAGE_0_BUS`, `SYSTEM_CLOCK_MOBILE_STORAGE_0_CARD`,
     `SYSTEM_RESET_MOBILE_STORAGE_0` with their `_1` twins, after `designware_mobile_storage`, the
     tree's own name for the controller; each keeps the binding's identifier in `doc(alias)`.
     Decision 4 asks the same word of the two public names.
   - `register_names.py` refuses eight: the private `len` fields (`:534`, `:681`, `:739`; a length
     carries its unit, and the standard-library exemption is for the method), the public `size`
     fields (`:1371`, `:1466`; decision 2), the public `power_management_chip_address` (`:556`;
     decision 3), and two test names ending in `_addresses` (`src/tests.rs:763`, `:840`). For the
     private fields `used_steps`; for the tests, names that say what is forgotten or kept without
     the word.
   - `prose_acronyms.py` refuses 28 first uses with no expansion: `README.md:4` (AON), `:6` (SD),
     `:96` (MHz), `:104` (AXP15060), and in `src/lib.rs` PL080 (`:173`), AON (`:180`), CRG (`:206`),
     CSR (`:224`), EEPROM (`:236`), PCB (`:237`), PLL0 (`:244`), FIFO (`:403`), MHz (`:430`), SD
     (`:454`), AXP15060 (`:525`), PLL (`:793`), APB (`:835`), PLL2 (`:844`), DACPD, DSMPD, FBDIV
     (`:851`), POSTDIV1, PREDIV (`:852`), SCL (`:1009`), SDA (`:1011`), NXP (`:1014`), UM10204
     (`:1015`), NUL (`:1588`). Expand each at its first use in the file, or propose the words every
     reader knows (MHz, FIFO, EEPROM, PCB, NUL, SCL, SDA) for the template's `prose_acronyms.toml`.

   Fix: the renames and expansions above. M. Two-way for the private names and the prose; the public
   names are decisions 2 to 4.

### P1: wrong on a reachable input, or a record the move cannot carry

3. **`parse_register_property` misreads a 12-byte `reg`.** `src/lib.rs:589-608` reads 16 or more
   bytes as `<address-hi address-lo size-hi size-lo>` and 8 to 15 bytes as `<address size>`. A
   property under `#address-cells = <2>` and `#size-cells = <1>` is 12 bytes and decodes as
   `(address-hi, address-lo)`: a base of 0 and a size equal to the real address. The caller,
   `kernel/src/reboot/pmic.rs:30`, maps that base and drives the DesignWare I2C master through it.
   Both JH7110 trees use `<2>`/`<2>` (verified in `jh7110.dtsi` at the pinned commit), so no known
   board reaches it, but the function's doc claims "the cells either tree spelling uses" and the Kani
   harness gives it exactly 16 bytes. Fix: accept exactly 8 or exactly 16 bytes and answer `None`
   otherwise, or take the parent's cell counts; add a 12-byte host test; let the harness choose the
   length. S. Two-way.

4. **`GMAC0_ALWAYS_ON_BRING_UP` carries one board's choice as a chip constant.** `src/lib.rs:277-280`
   selects parent 1 (`gmac0_rmii_rtx`) for the transmit clock. Mainline's
   `jh7110-starfive-visionfive-2-v1.3b.dts` assigns that parent; its `v1.2a.dts` has no `&gmac0`
   block, so a v1.2A leaves the mux where reset put it (verified at the pinned commit; whether
   parent 0 is what a v1.2A wants is not verified). The constant's doc at `:234-237` says v1.3B; the
   crate-level doc and the README call the plans what the Ethernet controller needs, and the README's
   BUGS list of board items (the PMIC reset, the PMIC bus plan, the I2C5 chain) omits it. Pull
   request #3475 hard-codes the plan for the device at `0x1603_0000` in
   `jh7110_clock_and_reset_devices::DEVICES`, so the choice reaches every client of the server. Fix
   now: name the transmit parent in the README's BUGS and in the constant's first sentence. S.
   Two-way. Whether the plan keeps the step is decision 5.

5. **The mutation record describes code that no longer exists.** The crate's rows in
   `notes/project-metrics/mutation-triage.csv` (287-289, 822-835, 1103-1105) name
   `every_reset_released`, `i2c5_ic_clk`, `PmicBus::from_nodes`, `CLOCK_MUX_MASK`, the test
   `the_standard_mode_timing_is_the_vendor_formula_worked_by_hand` and the statement `lcnt += diff`:
   none exists. `standard_mode_100_kilohertz` was rewritten from the vendor's formula to the
   specification's (milestone 1019 item 1), and its three "equivalent" rows reason about the old
   body. The template's `.github/mutants-exempts` refuses a line naming a survivor that does not
   exist, so the ledger cannot move with the crate, and nothing green exists for `release / gate` to
   read. `cargo mutants --list` counts 261 mutants. Fix: a full census inside the 08:00-13:00 UTC
   window, then an exempts file written from it with one reason per survivor, and the stale nife
   rows retired. M. Two-way.

### P2: a stranger would ask

6. **A fractional PLL2 reads as the vendor's default.** `src/lib.rs:953-957` returns
   1,188,000,000 Hz unless `DACPD` and `DSMPD` are both 1. Linux computes
   `(parent * frac / 2^24 + parent * fbdiv) / (prediv << postdiv1)` from `FRAC` in bits 23:0 of the
   word at `0x30` (verified), a word the crate already reads as
   `phase_locked_loop_2_post_divider_1_word`, and returns 0 only for the mixed mode. The recorded
   VisionFive 2 words are integer mode, so no board hits it today; a board that does gets a card
   clock and an I2C timing from a wrong parent rate. Fix: compute fractional mode as Linux does,
   keep the default for the mixed mode, and add the host case. M. Two-way.

7. **`was_already_up` reads the last reset only.** `BringUpReport` (`src/lib.rs:1272-1297`) keeps
   one reset's words; for `GMAC0_ALWAYS_ON_BRING_UP`'s two resets, `was_already_up` (`:1343-1349`)
   consults the second alone while `all_resets_are_released` counts both. No kernel caller asks
   `was_already_up` of a two-reset plan today, and pull request #3475's `Counts::already_up` folds
   every reset, so nothing misreports yet; the type's doc ("if the plan had one, the reset") does
   not say which reset. Fix: say "the last" in both docs, or add a `resets_already_released` count.
   S. Two-way.

8. **The I2C counts are safe by an undocumented margin, and their floor is below the controller's.**
   `counts` (`src/lib.rs:1069-1075`) programs `ceil(t * ic_clk)`. Linux's `i2c_dw_scl_hcnt` and
   `i2c_dw_scl_lcnt` (verified) hold that the controller spends `HCNT + 3` (or `+ 8`) and `LCNT + 1`
   cycles per phase, so the crate's 198 and 297 at 49.5 MHz run the bus about 2% under 100 kHz: the
   safe direction, which the doc should state. The floor of 1 is below the minimum programmable
   values the DesignWare databook gives for `IC_SS_SCL_HCNT` and `IC_SS_SCL_LCNT` (6 and 8, from the
   reviewer's memory, unverified), reachable only under about 1.5 MHz of input clock, where the test
   `a_slow_input_clock_still_produces_legal_counts` calls the counts legal. Fix: document the margin;
   once verified, raise the floors to the databook's and rename the test. S. Two-way.

9. **`name_index` drops every empty entry, not the trailing one.** `src/lib.rs:1593-1597` splits on
   NUL and filters empties, so `"sys\0\0stg\0"` answers 1 for `stg` where a device tree's string-list
   index is 2. No published tree has an empty `reg-names` entry. Fix: strip one trailing NUL, then
   split. S. Two-way.

10. **The two fallback paths disagree about the addresses.** `from_nodes` (`src/lib.rs:660-673`)
    keeps `controller_physical_window` and `power_management_chip_address` when the bus names no
    `clocks` and no `resets`, and forgets both when it names only foreign ones (the tests
    `a_bus_naming_no_clocks_or_resets_keeps_the_constant_plan_and_the_addresses` and
    `a_bus_naming_only_foreign_specifiers_falls_back_and_forgets_the_addresses` pin each). The field
    docs give no reason for the difference, and the direct reset write depends on it. Fix: state the
    reason at `controller_physical_window`, or keep the addresses on both paths. S. Two-way.

11. **Doc comments carry fetch dates.** `src/lib.rs:26-27`, `:184`, `:355`. The commit pins in the
    header make them redundant, and the rule is that documents state the present and git keeps the
    history. Fix: delete the dates. S. Two-way.

12. **The README misses what the scaffold's README asks for.** No "Building and testing" section
    with the three bare-metal targets and `rustup target add`; no figure for `cargo kani` (measured
    here: 9 harnesses, 3 minutes 59 seconds, 838 MB peak) nor the single-harness command
    (`cargo kani --harness verification::<name> --exact`); no "Running one falsification patch"
    example with the `-p1` form the repository will use; no badge row. The `release / gate` refuses
    until a stranger run is on record, and a stranger needs these. Fix: the sections, with the
    figures above. S. Two-way.

13. **Manifest and repository plumbing for the move.** `Cargo.toml` lacks `repository`,
    `documentation`, an `include` list and `[package.metadata.docs.rs] targets`; `keywords` could
    add `jh7110` and `starfive` and `categories` `no-std::no-alloc`, as `jh7110_entropy`'s manifest
    does. The repository needs `CHANGELOG.md`, `_typos.toml`, `deny.toml`, `rustfmt.toml`,
    `.kani-version`, `.github/accepted-words` with `jh` and its reason, the caller workflows, and
    the falsification patches rewritten from `-p3` to `-p1`. `src/lib.rs:1317` names the architect
    inside a two-line `Name:` block; the move's strip must take the whole block, which
    `standalone.py` will confirm. `publish = false` stays until the release ruling. S. Two-way.

14. **README prose for the template's prose steps.** `prose_caps.py` reports one 62-word sentence
    over the 40-word cap and `passive_voice.py` 8 passive constructions in 876 words against an
    allowance of 1. Both run under `set -e` in `check / standalone`; whether each exits nonzero on
    a finding was not verified. Fix: split the sentence; put seven passives into the active voice.
    S. Two-way.

## Decisions for the architect

Only one-way doors: public names and the meaning of a public constant, permanent at the crate's
first publication. Recommendation first, then background, prior art and an example.

1. **Ratify `POWER_MANAGEMENT_CHIP_BUS_BRING_UP` as the name of the plan pull request #3475 makes
   public.** Recommendation: ratify as spelled. Background: the constant was `pub(crate)`; the new
   `jh7110_clock_and_reset_devices` table needs it for the device at `0x1205_0000`, so #3475 opens
   it with a provisional `Name:` block, and `script/crate-standalone` refuses a crate with any
   unratified public name. It follows its siblings `ENTROPY_BRING_UP`, `GMAC0_SYSTEM_BRING_UP` and
   `GMAC0_ALWAYS_ON_BRING_UP`, and the type it names is already public (`PowerManagementChipBus`).
   Prior art: Linux exposes the same two steps as the `i2c5` node's `clocks` and `resets`. Example:
   `DEVICES[4].parts[0].plan == POWER_MANAGEMENT_CHIP_BUS_BRING_UP`.

2. **Rename `WindowLocation::size` and `RegisterWindow::size` to `size_bytes`.** Recommendation:
   rename before the first tag. Background: both are ratified fields holding a byte count; the units
   rule says a length carries its unit, and the template's `register_names` gate refuses `size` at
   ceiling 0 (item 2). The sibling fields were renamed to `base_physical_address` in the review
   package under the address rule; `size` was not reached then. Prior art: the tree's page contracts
   spell `PAGE_BYTES`; the crate's own private constants are `*_SIZE_BYTES`. Example:
   `SYSTEM_TOP_GROUP_WINDOW.fallback().size_bytes == 0x1_0000`.

3. **Rename `PowerManagementChipBus::power_management_chip_address` to say which address space it
   is.** Recommendation: `power_management_chip_bus_address`. Background: it is the PMIC's 7-bit
   address on its I2C bus, the node's `reg`; the register-names rule says an address is virtual or
   physical in every location, which is written for memory, and the gate refuses the field at
   ceiling 0. The alternative the gate offers, keeping the name with a `(reg, device tree)` opening
   and a `doc(alias)`, keeps a name that a reader of `controller_physical_window` beside it will
   misread as memory. Prior art: Linux's `i2c_client.addr`, the device tree's `reg` under an I2C
   controller. Example: `bus.power_management_chip_bus_address == Some(0x36)`.

4. **Whether `SD_INPUT_OUTPUT_BRING_UP` and `SYSTEM_CLOCK_SD_INPUT_OUTPUT_SD_CARD` keep the
   vendor's instance label, spelled out.** Recommendation: rename to `MOBILE_STORAGE_BRING_UP` and
   `SYSTEM_CLOCK_MOBILE_STORAGE_CARD`, and give the private constants of item 2 the same word.
   Background: both names were ratified in the review package as the spelled-out form of `SDIO`. The
   template's word list now says of `sdio` that the vendor's instance label is not a name and asks
   for what the controller is; the rule that a release review rechecks names ratified before a later
   rule applies. The controller is Synopsys's DesignWare Mobile Storage Host, which this tree already
   calls `designware_mobile_storage`; "SD input output" expands an acronym that elsewhere means an
   SD card carrying a peripheral, which these controllers do not serve. Prior art: Linux keeps the
   vendor label in `sdio0_ahb` and `sdio0_sdcard`; OpenBSD and FreeBSD attach the block as `dwmmc`
   (unverified). Example: `SYSTEM.clock_offset(SYSTEM_CLOCK_MOBILE_STORAGE_CARD[1]) == Some(0x178)`.

5. **Whether `GMAC0_ALWAYS_ON_BRING_UP` keeps the v1.3B transmit parent.** Recommendation: keep the
   step and document it as a board fact (item 4) while one board exists; this recommendation is
   about effort. At equal effort the shape to choose is Linux's: the chip plan holds the clocks and
   resets, and the parent comes from the tree's `assigned-clocks` and `assigned-clock-parents` the
   way `DevicePlan::from_specifiers` already reads `clocks` and `resets`, with the constant as the
   documented fallback. Background: the constant's meaning is public; dropping the step later
   changes what every caller's walk does. Options: (a) keep and document, S; (b) drop
   `SelectParent` from the constant, make `AONCLK_GMAC0_TRANSMIT_PARENT_RMII_RTX` public, and let the
   two callers (the kernel's `designware_ethernet.rs`, #3475's `DEVICES`) add the step, M; (c) read
   the parent from the tree, L. Prior art: Linux's SoC clock driver knows no parent choice and the
   board file assigns it; the vendor U-Boot's board file hard-codes it in
   `jh7110_gmac_sel_tx_to_rgmii`; crates.io carries one PAC per board revision. Example: a v1.2A
   walking the current plan sets bit 24 of AON word `0x14`, which its own tree never asks for.

## Done when

- A pull request in the crate's repository runs `check / clippy`, `check / standalone`, `check /
  tests`, `check / coverage`, `proofs / kani` and `proofs / falsify` green, with the proper nouns
  of item 1 either in `clippy.toml` or in backticks.
- `spelled_out.py`, `register_names.py` and `prose_acronyms.py` report 0 over the crate, with the
  seven private constants, the three `len` fields and the two test names renamed and every acronym
  expanded at its first use, and the public renames of decisions 2 to 4 applied as ruled.
- `parse_register_property` answers `None` for every length other than 8 and 16, or decodes by the
  parent's cell counts, with a 12-byte host test and a harness that chooses the length.
- The README's BUGS names the GMAC0 transmit parent as a VisionFive 2 v1.3B fact, and the constant's
  first sentence says so; decision 5 is ruled and applied.
- One full `mutation / mutation` run on `main` is green with every mutant tested, and
  `.github/mutants-exempts` names each survivor with a reason that describes the current body; the
  crate's rows in nife's triage ledger are retired.
- The fractional PLL2 rate is computed as Linux computes it, with a host case; `was_already_up`'s
  doc names the reset it reads; `name_index` counts an empty entry; the fallback asymmetry has its
  reason at the field or is gone.
- The README carries the scaffold's sections with the measured Kani figures and a `-p1`
  falsification example, the badge row, no sentence over 40 words and at most one passive per
  thousand words; a stranger run is on record with no open finding against the README.
- `Cargo.toml` carries `repository`, `documentation`, `include` and the docs.rs targets; the
  repository carries the scaffold's files and `.github/accepted-words` with `jh`.

## BUGS

- Item 5's census belongs in the 08:00-13:00 UTC window; nothing here was mutated, so the 261
  mutants' outcomes are not known, only that the recorded outcomes describe other code.
- Item 8's databook minimums come from memory. The fix that raises the floors waits on the databook
  or on a permissively licensed driver that states them; Linux's driver does not.
- Item 4 states that a v1.2A assigns no parent; it does not state what parent a v1.2A needs. Only
  that board, or its schematic, answers.
- Items 3, 9 and 10 were read, not executed; each is a few lines and its test is its proof.
- The review built `main`. Pull request #3475 adds two lines to the crate and two callers; the
  review read the diff and did not compile or test it.
- The seven falsification patches not replayed are trusted from the review package's run.

## Index row

An outside review of `jh7110_clock_and_reset` before its move: two P0 items that keep the template's
checks red (17 `doc_markdown` errors, 43 ceiling-0 name and acronym refusals), three P1 items (a
`reg` decoder that misreads a 12-byte property, a chip plan carrying one board's transmit parent, a
mutation record that describes code that no longer exists), nine P2 items, and five one-way
decisions on public names and the plan's meaning.
