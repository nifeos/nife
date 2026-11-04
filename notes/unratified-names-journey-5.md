# Names journey 5 has not ratified yet

*Name: provisional, minted 2026-11-03 (UTC) by lane/unratified-names-journey-5. The file name says what the list holds; `names-awaiting-ratification` was another candidate.*

This note lists, for every crate on [journey 5](../design/journeys/05-crates-released-on-their-own.md) that still sits under `crates/`, each module, type, function, constant and register whose `Name:` block is missing or still provisional, in groups an architect can rule on one at a time. It proposes and renames nothing. Names are an architect's call ([design/naming.md](../design/naming.md)); every recommendation below is provisional and leaves the current spelling in force until a ruling.

`script/crate-standalone` step 0 passes a crate only when it finds every `Name:` block in it ratified, so each group here stands between a crate and its move. The lists come from `python3 helpers/name_release.py --draft <crate>`: a row whose status reads `no block` or starts with `provisional`. Rerun it for the live state; nothing regenerates this note, and a ruling removes its group.

`extensible_host_controller_interface`, `file_allocation_table`, `generational_table`, `jh7110_clock_and_reset` and `non_volatile_memory_express` have no unratified name left. `jh7110_entropy` has already left `crates/`.

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

1. The `no` groups of designware_mobile_storage (`DM7`, `DM8` and `DM10`): ratify as they stand, which clears the module apart from DM9.

The groups that need a real ruling are the register and bit constants: `E7` and `E8`. Decide the designware_ethernet ones first, since #3094 set the pattern, then reuse it for e1000e.

## `designware_ethernet`

## `designware_mobile_storage`

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

### E12. Reset sequence (10)

`pch::sequence::BlockedAfterPhysicalLayerDeviceReset`, `pch::sequence::Error`, `pch::sequence::Note`, `pch::sequence::PhysicalLayerDevice`, `pch::sequence::PhysicalLayerDeviceInaccessible`, `pch::sequence::PhysicalLayerDeviceReached`, `pch::sequence::PhysicalLayerDeviceResetNotAsserted`, `pch::sequence::Reached`, `pch::sequence::global_reset`, `pch::sequence::transmit_errata`

- Breaks the spelled-out rule: no.
- Provisional recommendation: Ratify as they stand, including the five provisional `PhysicalLayerDevice*` and `BlockedAfterPhysicalLayerDeviceReset` variants; all spell out their words.

### E13. Ultra low power and simulator (7)

`pch::ulp::WAIT_MS`, `pch::ulp::done`, `pch::ulp::release`, `pch::ulp::request`, `pch::simulator::Sim`, `pch::simulator::Start`, `pch::simulator::new`

- Breaks the spelled-out rule: one.
- Provisional recommendation: Ratify `done`, `release`, `request` and the simulator names; recommend `WAIT_MS` become `WAIT_MILLISECONDS`, matching `POLL_MICROSECONDS`.

## Not covered

The lists hold names with no block or a provisional one. A name already ratified keeps its spelling until its own review. Cargo features carry no block and are not listed. A crate may carry further unratified names that `name_release.py` does not read, for example inside macro expansions.
