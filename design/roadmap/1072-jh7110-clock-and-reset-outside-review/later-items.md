# P2: a stranger would ask

An appendix to [the milestone](../1072-jh7110-clock-and-reset-outside-review.md).

6. A fractional PLL2 reads as the vendor's default. `src/lib.rs:953-957` returns <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   1,188,000,000 Hz unless `DACPD` and `DSMPD` are both 1. Linux computes
   `(parent * frac / 2^24 + parent * fbdiv) / (prediv << postdiv1)` from `FRAC` in bits 23:0 of the
   word at `0x30` (verified), a word the crate already reads as
   `phase_locked_loop_2_post_divider_1_word`, and returns 0 only for the mixed mode. The recorded
   VisionFive 2 words are integer mode, so no board hits it today; a board that does gets a card
   clock and an I2C timing from a wrong parent rate. Fix: compute fractional mode as Linux does,
   keep the default for the mixed mode, and add the host case. M. Two-way.

7. `was_already_up` reads the last reset only. `BringUpReport` (`src/lib.rs:1272-1297`) keeps <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   one reset's words; for `GMAC0_ALWAYS_ON_BRING_UP`'s two resets, `was_already_up` (`:1343-1349`)
   consults the second alone while `all_resets_are_released` counts both. No kernel caller asks
   `was_already_up` of a two-reset plan today, and pull request #3475's `Counts::already_up` folds
   every reset, so nothing misreports yet; the type's doc ("if the plan had one, the reset") does
   not say which reset. Fix: say "the last" in both docs, or add a `resets_already_released` count.
   S. Two-way.

8. The I2C counts are safe by an undocumented margin, and their floor is below the controller's.
   `counts` (`src/lib.rs:1069-1075`) programs `ceil(t * ic_clk)`. Linux's `i2c_dw_scl_hcnt` and <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `i2c_dw_scl_lcnt` (verified) hold that the controller spends `HCNT + 3` (or `+ 8`) and `LCNT + 1`
   cycles per phase, so the crate's 198 and 297 at 49.5 MHz run the bus about 2% under 100 kHz.
   That is the safe direction, and the doc should state it. The floor of 1 is below the minimum
   programmable values the DesignWare databook gives for `IC_SS_SCL_HCNT` and `IC_SS_SCL_LCNT` (6 and
   8, from the reviewer's memory, unverified). Only an input clock under about 1.5 MHz reaches it,
   and there the test `a_slow_input_clock_still_produces_legal_counts` calls the counts legal. Fix: document the margin;
   once verified, raise the floors to the databook's and rename the test. S. Two-way.

9. `name_index` drops every empty entry, not the trailing one. `src/lib.rs:1593-1597` splits on <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   NUL and filters empties, so `"sys\0\0stg\0"` answers 1 for `stg` where a device tree's string-list
   index is 2. No published tree has an empty `reg-names` entry. Fix: strip one trailing NUL, then
   split. S. Two-way.

10. The two fallback paths disagree about the addresses. `from_nodes` (`src/lib.rs:660-673`) <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    keeps `controller_physical_window` and `power_management_chip_address` when the bus names no
    `clocks` and no `resets`, and forgets both when it names only foreign ones (the tests
    `a_bus_naming_no_clocks_or_resets_keeps_the_constant_plan_and_the_addresses` and
    `a_bus_naming_only_foreign_specifiers_falls_back_and_forgets_the_addresses` pin each). The field
    docs give no reason for the difference, and the direct reset write depends on it. Fix: state the
    reason at `controller_physical_window`, or keep the addresses on both paths. S. Two-way.

11. Doc comments carry fetch dates. `src/lib.rs:26-27`, `:184`, `:355`. The commit pins in the <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    header make them redundant, and the rule is that documents state the present and git keeps the
    history. Fix: delete the dates. S. Two-way.

12. The README misses what the scaffold's README asks for. It has no "Building and testing" section
    with the three bare-metal targets and `rustup target add`. It gives no figure for `cargo kani`
    (measured here: 9 harnesses, 3 minutes 59 seconds, 838 MB peak) and no single-harness command
    (`cargo kani --harness verification::<name> --exact`). It has no "Running one falsification
    patch" example with the `-p1` form the repository will use, and no badge row. The `release / gate` refuses
    until a stranger run is on record, and a stranger needs these. Fix: the sections, with the
    figures above. S. Two-way.

13. Manifest and repository plumbing for the move. `Cargo.toml` lacks `repository`,
    `documentation`, an `include` list and `[package.metadata.docs.rs] targets`; `keywords` could
    add `jh7110` and `starfive` and `categories` `no-std::no-alloc`, as `jh7110_entropy`'s manifest
    does. The repository needs `CHANGELOG.md`, `_typos.toml`, `deny.toml`, `rustfmt.toml`,
    `.kani-version`, `.github/accepted-words` with `jh` and its reason, the caller workflows, and
    the falsification patches rewritten from `-p3` to `-p1`. `src/lib.rs:1317` names the architect <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    inside a two-line `Name:` block; the move's strip must take the whole block, which
    `standalone.py` will confirm. `publish = false` stays until the release ruling. S. Two-way.

14. README prose for the template's prose steps. `prose_caps.py` reports one 62-word sentence
    over the 40-word cap and `passive_voice.py` 8 passive constructions in 876 words against an
    allowance of 1. Both run under `set -e` in `check / standalone`; whether each exits nonzero on
    a finding was not verified. Fix: split the sentence; put seven passives into the active voice.
    S. Two-way.
