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

Raised by a read-only review of `crates/jh7110_clock_and_reset` on `main`. The review also read the
diff of pull request #3475 (milestone 1062 (the device control planes leave the kernel) step 2, the
clock and reset server). It reads as a stranger to this tree would meet the crate in its own
repository. It changes nothing; it lists what the next
lanes change.

The review took its baseline from [the review package](../../notes/crate-reviews/jh7110-clock-and-reset.md)
and reopens none of the decisions it settles, except where a later rule now contradicts one and this
block says so. Those decisions are the crate name, the ratified public names, the
`#[non_exhaustive]` markers, the dual license, and the decision to keep the VisionFive 2 board items
in the chip crate. Pull request
#3475 touches the crate in one place: `POWER_MANAGEMENT_CHIP_BUS_BRING_UP` becomes `pub` with a
provisional name, and two new crates (`jh7110_clock_and_reset_contract`,
`jh7110_clock_and_reset_devices`) become its callers. The review read that diff; it did not build it.

Reuse: the template `nifeos/crate-workflows` and its helpers, Kani, cargo-mutants, cargo-llvm-cov,
the crate's own harness helpers (`any_domain`, `ungates_before_releasing`, `system_provider`) and
the pinned Linux and vendor U-Boot sources the crate cites. No new tool.

## Goal

A stranger who finds `jh7110_clock_and_reset` in its own repository can open a pull request and see
every template check green for the crate's own reasons. The same stranger can tell which facts are
the chip's and which are one board's. That stranger can decode a `reg` property of either common
cell layout without a misread, and can trust the recorded mutation results because they describe
the code that ships.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1072-jh7110-clock-and-reset-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the prior art appendix](1072-jh7110-clock-and-reset-outside-review/prior-art.md).

## The items

Each item: what is wrong, where, why it matters, the fix, effort (S under a day, M a few days, L
more), and whether the door is one-way (a published name, format, identity, layout or dependency)
or two-way. Line numbers are `main`'s; pull request #3475 shifts `src/lib.rs` by two lines after
line 413.

### P0: the repository cannot go green

1. `check / clippy` is red on the first pull request. `cargo clippy --all-targets -- -D
   warnings` reports 17 `doc_markdown` errors, 26 words: `VisionFive` (14), `OpenSBI` (9),
   `AXP15060` (2) and `PLL2` (1), at `src/lib.rs:25`, `:234`, `:352`, `:356`, `:402`, `:410`, `:489`, <!-- line-citation: exception. Reason: pinned -->
   `:525`, `:981`, `:1024`, `:1109`, `:1111`, `:1391` and `src/tests.rs:643`. Inside nife the root <!-- line-citation: exception. Reason: pinned -->
   `clippy.toml` lists these proper nouns under `doc-valid-idents`; the copied crate has no such file
   and the template's scaffold ships none, so the template's required check fails on stable and on
   1.88 alike. Fix: ship a `clippy.toml` with `doc-valid-idents = ["..", "VisionFive", "OpenSBI",
   "AXP15060", "PLL2"]`, for the reason nife's own file gives (a proper noun in backticks claims to
   be an identifier), or put the 26 words in backticks. S. Two-way.

2. `check / standalone` is red at three ceiling-0 steps.
   - `spelled_out.py` refuses seven private constants: `AONCLK_GMAC0_TX` (`src/lib.rs:233`, `tx`) and <!-- line-citation: exception. Reason: pinned -->
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
   - `register_names.py` refuses eight names. Three are private `len` fields (`:534`, `:681`,
     `:739`; a length carries its unit, and the standard-library exemption is for the method). Two
     are public `size` fields (`:1371`, `:1466`; decision 2). One is the public
     `power_management_chip_address` (`:556`; decision 3). Two are test names ending in
     `_addresses` (`src/tests.rs:763`, `:840`). The private fields gain their unit; the tests take <!-- line-citation: exception. Reason: pinned -->
     names that say what a test forgets or keeps without the word.
   - `prose_acronyms.py` refuses 28 first uses with no expansion. Four are in `README.md` (AON,
     SD, MHz and AXP15060). The rest are in `src/lib.rs`. They are PL080 (`:173`), AON (`:180`), CRG (`:206`),
     CSR (`:224`). Then come EEPROM (`:236`), PCB (`:237`), PLL0 (`:244`), FIFO (`:403`), MHz (`:430`), SD
     (`:454`), AXP15060 (`:525`), PLL (`:793`), APB (`:835`), PLL2 (`:844`), DACPD, DSMPD, FBDIV
     (`:851`), POSTDIV1, PREDIV (`:852`), SCL (`:1009`), SDA (`:1011`), NXP (`:1014`), UM10204
     (`:1015`), NUL (`:1588`). Expand each at its first use in the file. Alternatively, propose the
     words every reader knows (MHz, FIFO, EEPROM, PCB, NUL, SCL, SDA) for the template's
     `prose_acronyms.toml`.

   Fix: the renames and expansions above. M. Two-way for the private names and the prose; the public
   names are decisions 2 to 4.

### P1: wrong on a reachable input, or a record the move cannot carry

3. `parse_register_property` misreads a 12-byte `reg`. `src/lib.rs:589-608` reads 16 or more <!-- line-citation: exception. Reason: pinned -->
   bytes as `<address-hi address-lo size-hi size-lo>` and 8 to 15 bytes as `<address size>`. A
   property under `#address-cells = <2>` and `#size-cells = <1>` is 12 bytes and decodes as
   `(address-hi, address-lo)`: a base of 0 and a size equal to the real address. The caller,
   `kernel/src/reboot/pmic.rs:30`, maps that base and drives the DesignWare I2C master through it. <!-- line-citation: exception. Reason: pinned -->
   Both JH7110 trees use `<2>`/`<2>` (verified in `jh7110.dtsi` at the pinned commit), so no known
   board reaches it, but the function's doc claims "the cells either tree spelling uses" and the Kani
   harness gives it exactly 16 bytes. Fix: accept exactly 8 or exactly 16 bytes and answer `None`
   otherwise, or take the parent's cell counts; add a 12-byte host test; let the harness choose the
   length. S. Two-way.

4. `GMAC0_ALWAYS_ON_BRING_UP` carries one board's choice as a chip constant. `src/lib.rs:277-280` <!-- line-citation: exception. Reason: pinned -->
   selects parent 1 (`gmac0_rmii_rtx`) for the transmit clock. Mainline's
   `jh7110-starfive-visionfive-2-v1.3b.dts` assigns that parent; its `v1.2a.dts` has no `&gmac0`
   block, so a v1.2A leaves the mux where reset put it (verified at the pinned commit; whether
   parent 0 is what a v1.2A wants is not verified). The constant's doc at `:234-237` says v1.3B. The
   crate-level doc and the README call the plans what the Ethernet controller needs, and the
   README's BUGS list of board items (the PMIC reset, the PMIC bus plan, the I2C5 chain) omits it. Pull
   request #3475 hard-codes the plan for the device at `0x1603_0000` in
   `jh7110_clock_and_reset_devices::DEVICES`, so the choice reaches every client of the server. Fix
   now: name the transmit parent in the README's BUGS and in the constant's first sentence. S.
   Two-way. Whether the plan keeps the step is decision 5.

5. The mutation record describes code that no longer exists. The crate's rows in
   `notes/project-metrics/mutation-triage.csv` (287-289, 822-835, 1103-1105) name
   `every_reset_released`, `i2c5_ic_clk`, `PmicBus::from_nodes`, `CLOCK_MUX_MASK`, the test
   `the_standard_mode_timing_is_the_vendor_formula_worked_by_hand` and the statement `lcnt += diff`:
   none exists. `standard_mode_100_kilohertz` was rewritten from the vendor's formula to the
   specification's (milestone 1019 (the driver crates carry facts from their sources, not their text) item 1), and its three "equivalent" rows reason about the old
   body. The template's `.github/mutants-exempts` refuses a line naming a survivor that does not
   exist, so the ledger cannot move with the crate, and nothing green exists for `release / gate` to
   read. `cargo mutants --list` counts 261 mutants. Fix: a full census inside the 08:00-13:00 UTC
   window, then an exempts file written from it with one reason per survivor, and the stale nife
   rows retired. M. Two-way.

### P2: a stranger would ask

The P2 items are in [the appendix](1072-jh7110-clock-and-reset-outside-review/later-items.md).

## Decisions for the architect

Only one-way doors: public names and the meaning of a public constant, permanent at the crate's
first publication. Recommendation first, then background, prior art and an example.

1. Ratify `POWER_MANAGEMENT_CHIP_BUS_BRING_UP` as the name of the plan pull request #3475 makes
   public. Recommendation: ratify as spelled. Background: the constant was `pub(crate)`; the new
   `jh7110_clock_and_reset_devices` table needs it for the device at `0x1205_0000`, so #3475 opens
   it with a provisional `Name:` block, and `script/crate-standalone` refuses a crate with any
   unratified public name. It follows its siblings `ENTROPY_BRING_UP`, `GMAC0_SYSTEM_BRING_UP` and
   `GMAC0_ALWAYS_ON_BRING_UP`, and the type it names is already public (`PowerManagementChipBus`).
   Prior art: Linux exposes the same two steps as the `i2c5` node's `clocks` and `resets`. Example:
   `DEVICES[4].parts[0].plan == POWER_MANAGEMENT_CHIP_BUS_BRING_UP`.

2. Rename `WindowLocation::size` and `RegisterWindow::size` to `size_bytes`. Recommendation:
   rename before the first tag. Background: the review package ratified both fields, and each holds a byte count. The
   units rule says a length carries its unit, and the template's `register_names` gate refuses
   `size` at ceiling 0 (item 2). The review package renamed the sibling fields to
   `base_physical_address` under the address rule, and the pass did not reach `size`. Prior art: the tree's page contracts
   spell `PAGE_BYTES`; the crate's own private constants are `*_SIZE_BYTES`. Example:
   `SYSTEM_TOP_GROUP_WINDOW.fallback().size_bytes == 0x1_0000`.

3. Rename `PowerManagementChipBus::power_management_chip_address` to say which address space it
   is. Recommendation: `power_management_chip_bus_address`. Background: it is the PMIC's 7-bit
   address on its I2C bus, the node's `reg`. The register-names rule says an address is virtual or
   physical in every location, a rule for memory, and the gate refuses the field at ceiling 0. The alternative the gate offers, keeping the name with a `(reg, device tree)` opening
   and a `doc(alias)`, keeps a name that a reader of `controller_physical_window` beside it will
   misread as memory. Prior art: Linux's `i2c_client.addr`, the device tree's `reg` under an I2C
   controller. Example: `bus.power_management_chip_bus_address == Some(0x36)`.

4. Whether `SD_INPUT_OUTPUT_BRING_UP` and `SYSTEM_CLOCK_SD_INPUT_OUTPUT_SD_CARD` keep the
   vendor's instance label, spelled out. Recommendation: rename to `MOBILE_STORAGE_BRING_UP` and
   `SYSTEM_CLOCK_MOBILE_STORAGE_CARD`, and give the private constants of item 2 the same word.
   Background: the review package ratified both names as the spelled-out form of `SDIO`. The
   template's word list now says of `sdio` that the vendor's instance label is not a name and asks
   for what the controller is; the rule that a release review rechecks names ratified before a later
   rule applies. The controller is Synopsys's DesignWare Mobile Storage Host, which this tree already
   calls `designware_mobile_storage`; "SD input output" expands an acronym that elsewhere means an
   SD card carrying a peripheral, which these controllers do not serve. Prior art: Linux keeps the
   vendor label in `sdio0_ahb` and `sdio0_sdcard`; OpenBSD and FreeBSD attach the block as `dwmmc`
   (unverified). Example: `SYSTEM.clock_offset(SYSTEM_CLOCK_MOBILE_STORAGE_CARD[1]) == Some(0x178)`.

5. Whether `GMAC0_ALWAYS_ON_BRING_UP` keeps the v1.3B transmit parent. Recommendation: keep the
   step and document it as a board fact (item 4) while one board exists; this recommendation is
   about effort. At equal effort the shape to choose is Linux's. The chip plan holds the clocks and
   resets. The parent comes from the tree's `assigned-clocks` and `assigned-clock-parents`, the
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

The checks are in [the done-when appendix](1072-jh7110-clock-and-reset-outside-review/done-when.md).

## BUGS

- Item 5's census belongs in the 08:00-13:00 UTC window. Nobody mutated anything here, so the 261
  mutants' outcomes are not known, only that the recorded outcomes describe other code.
- Item 8's databook minimums come from memory. The fix that raises the floors waits on the databook
  or on a permissively licensed driver that states them; Linux's driver does not.
- Item 4 states that a v1.2A assigns no parent; it does not state what parent a v1.2A needs. Only
  that board, or its schematic, answers.
- The review read items 3, 9 and 10 and did not execute them; each is a few lines and its test is
  its proof.
- The review built `main`. Pull request #3475 adds two lines to the crate and two callers; the
  review read the diff and did not compile or test it.
- The review did not replay seven falsification patches and trusts the review package's run.

## Index row

An outside review of `jh7110_clock_and_reset` before its move. Two P0 items keep the template's
checks red: 17 `doc_markdown` errors and 43 ceiling-0 name and acronym refusals. Three P1 items
follow: a `reg` decoder that misreads a 12-byte property, a chip plan carrying one board's transmit
parent, and a mutation record that describes code that no longer exists. Nine P2 items and five
one-way decisions on public names and the plan's meaning complete it.
