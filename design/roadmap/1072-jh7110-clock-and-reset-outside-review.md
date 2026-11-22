---
status: BUILT
raised: 2026-11-11
built: 2026-11-22
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
     source spelling, so a later rule reached them. Their names follow decision 4:
     `ALWAYS_ON_CLOCK_GMAC0_TRANSMIT`, and `SYSTEM_CLOCK_SD_IO_0_ADVANCED_HIGH_PERFORMANCE_BUS`,
     `SYSTEM_CLOCK_SD_IO_0_SD_CARD` and `SYSTEM_RESET_SD_IO_0_ADVANCED_HIGH_PERFORMANCE_BUS` with
     their `_1` twins, each keeping the binding's identifier in `doc(alias)`.
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
   Two-way. Decision 5 takes the step out of the plan.

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

## Decisions

Decisions 1 to 5 ruled 2026-11-14 (UTC). Each is a public name or the meaning of a public constant, permanent at the
crate's first publication.

1. `POWER_MANAGEMENT_CHIP_BUS_BRING_UP` is ratified as spelled: the plan for the device at
   `0x1205_0000` in `jh7110_clock_and_reset_devices::DEVICES`, beside its siblings
   `ENTROPY_BRING_UP`, `GMAC0_SYSTEM_BRING_UP` and `GMAC0_ALWAYS_ON_BRING_UP`. Prior art: Linux
   exposes the same two steps as the `i2c5` node's `clocks` and `resets`. Example:
   `DEVICES[4].parts[0].plan == POWER_MANAGEMENT_CHIP_BUS_BRING_UP`.

2. `WindowLocation::size` and `RegisterWindow::size` are `size_bytes`: each holds a byte count, and
   a length carries its unit. Prior art: the tree's page contracts spell `PAGE_BYTES`. Example:
   `SYSTEM_TOP_GROUP_WINDOW.fallback().size_bytes == 0x1_0000`.

3. `PowerManagementChipBus::power_management_chip_address` is `power_management_chip_bus_address`:
   the PMIC's 7-bit address on its I2C bus, the node's `reg`, and no memory address. Prior art:
   Linux's `i2c_client.addr`. Example: `bus.power_management_chip_bus_address == Some(0x36)`.

4. `SDIO` is written `SD_IO`: `SD_IO_BRING_UP` and `SYSTEM_CLOCK_SD_IO_SD_CARD`, and the private
   constants of item 2 take the same word (`SYSTEM_CLOCK_SD_IO_0_SD_CARD` and its twins). `SD` and
   `io` are both short forms that stay, and `io` stays whole inside an expansion
   ([the naming rule](../naming.md)). The always-on constants are spelled out
   (`ALWAYS_ON_CLOCK_GMAC0_TRANSMIT`). Each keeps the binding's identifier in `doc(alias)`, and none
   carries an alias of this tree's earlier spelling. Prior art: Linux keeps the vendor label in
   `sdio0_ahb` and `sdio0_sdcard`. Example:
   `SYSTEM.clock_offset(SYSTEM_CLOCK_SD_IO_SD_CARD[1]) == Some(0x178)`.

5. The transmit clock's parent comes from the tree. `GMAC0_ALWAYS_ON_BRING_UP` holds the chip's
   clocks and resets and no parent step. `gmac0_transmit_parent` reads a `gmac0` node's
   `assigned-clocks` and `assigned-clock-parents`, the way `DevicePlan::from_specifiers` reads
   `clocks` and `resets`, and answers a parent step, none, or a refusal. The v1.3B value is the
   documented constant `GMAC0_TRANSMIT_PARENT_REDUCED_MEDIA_INDEPENDENT_INTERFACE_RECEIVE_TRANSMIT`.
   Prior art: Linux's SoC clock driver knows no parent choice, and the board file assigns it
   (`jh7110-starfive-visionfive-2-v1.3b.dts`: `assigned-clocks = <&aoncrg
   JH7110_AONCLK_GMAC0_TX>; assigned-clock-parents = <&aoncrg JH7110_AONCLK_GMAC0_RMII_RTX>;`);
   the v1.2A board file has no `&gmac0`. Example: a v1.2A tree gives no parent step, so its mux
   stays where reset put it.

6. The pre-release tag. Ruled 2026-11-19 (UTC): the tag `v0.1.0-pre.1` is approved for
   `jh7110_clock_and_reset`. The move lane places it once it finds the new repository matches this
   tree's copy and the release dry run is green, the same two conditions as the seven crates
   approved 2026-11-11. The tag names a commit for nife and basalt to pin and nothing publishes.
   The review's clearance (2026-11-11 UTC) approved creating the repository, and none exists yet. Prior art: Semantic Versioning 2.0.0 marks a candidate with a pre-release
   identifier, and Cargo never selects one for a requirement that does not name it. Example:
   `generational_table` carries `v0.1.0-pre.2`, and nife pins that tag's commit by `rev`.

## Done when

The checks are in [the done-when appendix](1072-jh7110-clock-and-reset-outside-review/done-when.md).

## BUGS

- Item 8's floors take a spike suppression length of 1, since the crate does not program
  `IC_FS_SPKLEN`. A controller whose reset value is longer needs higher floors, which only an input
  clock under 2 MHz would reach.
- Item 4 states that a v1.2A assigns no parent; it does not state what parent a v1.2A needs. Only
  that board, or its schematic, answers.
- The review did not replay seven falsification patches and trusts the review package's run.

## Finding status

- Item 1 (P0), `clippy` red standing alone: Done, [#3625](https://github.com/nifeos/nife/pull/3625).
- Item 2 (P0), `standalone` refusals: Done, [#3572](https://github.com/nifeos/nife/pull/3572) and [#3625](https://github.com/nifeos/nife/pull/3625).
- Item 3 (P1), 12-byte `reg`: Done, [#3625](https://github.com/nifeos/nife/pull/3625).
- Item 4 (P1), one board's transmit parent: Done, [#3572](https://github.com/nifeos/nife/pull/3572) and [#3625](https://github.com/nifeos/nife/pull/3625).
- Item 5 (P1), stale mutation record: At the move. The census of run 43876176886 tested every
  mutant and missed six algebraic equivalents, and [#3936](https://github.com/nifeos/nife/pull/3936) retires the stale triage rows for
  five that reason about the current body. The move writes `.github/mutants-exempts` from those
  rows, and the crate repository's first full `mutation / mutation` run tests the mutants added
  since the census.
- Item 6 (P2), fractional PLL2: Done, [#3625](https://github.com/nifeos/nife/pull/3625).
- Item 7 (P2), `was_already_up`: Done, [#3625](https://github.com/nifeos/nife/pull/3625).
- Item 8 (P2), I2C count margin and floor: Done, [#3625](https://github.com/nifeos/nife/pull/3625) and [#3936](https://github.com/nifeos/nife/pull/3936).
- Item 9 (P2), `name_index`: Done, [#3625](https://github.com/nifeos/nife/pull/3625).
- Item 10 (P2), fallback addresses: Done, [#3625](https://github.com/nifeos/nife/pull/3625).
- Item 11 (P2), fetch dates: Done, [#3625](https://github.com/nifeos/nife/pull/3625).
- Item 12 (P2), README sections: At the move. The sections and figures are in [#3625](https://github.com/nifeos/nife/pull/3625), and
  [#3936](https://github.com/nifeos/nife/pull/3936) drops the test count the template now refuses; the badge row needs the repository's
  caller workflows.
- Item 13 (P2), manifest and repository plumbing: At the move. The manifest fields are in
  [#3625](https://github.com/nifeos/nife/pull/3625); the repository files (`.github/accepted-words` with `jh`, the caller workflows, the
  `-p1` patches) and the strip of the `Name:` blocks are the move's.
- Item 14 (P2), README prose: Done, [#3625](https://github.com/nifeos/nife/pull/3625).
- Decision 1, `POWER_MANAGEMENT_CHIP_BUS_BRING_UP`: Done, [#3572](https://github.com/nifeos/nife/pull/3572).
- Decision 2, `size_bytes`: Done, [#3572](https://github.com/nifeos/nife/pull/3572).
- Decision 3, `power_management_chip_bus_address`: Done, [#3572](https://github.com/nifeos/nife/pull/3572).
- Decision 4, `SD_IO` and the always-on constants: Done, [#3572](https://github.com/nifeos/nife/pull/3572).
- Decision 5, transmit parent from the tree: Done, [#3572](https://github.com/nifeos/nife/pull/3572).
- Decision 6, the `v0.1.0-pre.1` tag: At the move. The move lane places it after the move check
  and a green release dry run.

## Open decisions

None. The publishing hold stands: no first publish happens until the last outside-review milestone
closes (ruled 2026-11-18 UTC).

## Follow-on

- **Milestone 819.** The move to nifeos/jh7110_clock_and_reset. The review has its clearance and
  the repository its approval (2026-11-11 UTC). The move carries items 5, 12 and 13 and decision 6,
  as the finding status says.
- **Recorded.** The server's transmit parent. `jh7110_clock_and_reset_devices` applies the v1.3B
  parent to the device at `0x1603_0000` on every board, and its BUGS section records it. The
  recommended fix is that the spawner reads the parent with
  `machine_discovery::jh7110::clock_and_reset::first_ethernet_port_transmit_parent` and passes it
  in a startup-page entry or a spawn word, the way the power-management-chip route starter passes
  its timing (`driver_process_contract::power_management_chip_route`).

## Index row

An outside review of `jh7110_clock_and_reset` before its move. Two P0 items keep the template's
checks red: 17 `doc_markdown` errors and 43 ceiling-0 name and acronym refusals. Three P1 items
follow: a `reg` decoder that misreads a 12-byte property, a chip plan carrying one board's transmit
parent, and a mutation record that describes code that no longer exists. Nine P2 items and six
ruled decisions on public names, the plan's meaning and the pre-release tag complete it.
