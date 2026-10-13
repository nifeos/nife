---
status: BUILT
raised: 2026-10-11
built: 2026-10-12
milestone_dependencies: none
decision_dependencies: 266, 282, 293
machine_requirements: none
specific_machine: none
needs_person: no
---
# 875. The ACPI table parser is split along its seams

*(The title and slug are provisional.)*

`crates/machine_discovery/src/acpi.rs` is the Advanced Configuration and Power Interface (ACPI)
table parser, and it is 3,009 lines at `396187b0b`. It is a
module of the `machine_discovery` crate, not its root. §266 (a Rust source file stays under 2,000
lines) sets the ceiling. Milestone 840 (the scheduler file is split along its seams) is the model.

The cut and every module name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

§293 (the ACPI table parser splits into one private module per table) is that write-up, decided,
with the file measured on main at `17b23264e`. Three decisions answer forks 1 and 2 below: §272 (the grant-plan crate root splits into
private modules), §274 (the filesystem-protocol crate root splits into its module files) and §279
(the system-initializer crate root splits into private modules). §282 (when a file is split, the
parent becomes an index) moves the RSDP, header and root walk out of `acpi.rs` too. §293 applies
them and asks nothing. The maintainer recorded it as settled by precedent on #1972 the same day,
and §293 is decided. Every module name ships provisional.

Reuse: not applicable; this moves code and adds none. The precedent is in this module already.
The IVRS, AMD's counterpart of the DMAR, lives in `acpi/ivrs.rs` (694 lines) with its own tests.
It became a child module because it is a different table.

## What is in the file

Measured on `396187b0b` from the file's own top-level items and its section banners. Ranges are
approximate at the edges, where a doc comment belongs to the item below it.

| lines | count | what |
|---|---|---|
| 1 to 62 | 62 | module docs and BUGS |
| 63 to 241 | 179 | `AcpiError`, the checksum, the RSDP, the SDT header, the root-table walk |
| 242 to 433 | 192 | the MADT and its entry walk |
| 434 to 534 | 101 | legacy IRQ routing out of the MADT's overrides, `isa_irq_table` |
| 535 to 585 | 51 | the MCFG and its ECAM windows |
| 586 to 727 | 142 | the DMAR's fixed part and structure walk, and `pub mod ivrs` |
| 728 to 1145 | 418 | `DmarUnits`: device scopes, RMRRs, which unit owns which device |
| 1146 to 1283 | 138 | GTDT, SPCR and the FADT's Arm boot flags |
| 1284 to 1352 | 69 | the FADT's reset register |
| 1353 to 1367 | 15 | byte readers `u16`, `u32`, `u64`, which `ivrs` also uses |
| 1368 to 1630 | 263 | `mod verification`, 8 Kani harnesses |
| 1631 to 3009 | 1,379 | `mod tests`, 61 `#[test]` functions |

The code is 1,367 lines. Tests and proofs are the other 1,642. The tests are in no order by
table: the Arm tests come first, and a "survivor triage" block at the end mixes DMAR, MADT, FADT
and GTDT. Sorted by what they test, they fall like this.

| tests of | about |
|---|---|
| the RSDP, the header, the root walk, and the shared builders `seal`, `sdt`, `rsdp_v1` | 210 |
| the MADT, both x86 and Arm entries | 325 |
| legacy IRQ routing | 95 |
| the MCFG | 80 |
| the DMAR's fixed part and walk | 165 |
| `DmarUnits` | 360 |
| GTDT, SPCR and FADT | 145 |

The FADT's reset register sits under the banner "The tables an Arm machine has that an x86 one
does not". It is an x86 feature, read by `kernel/src/arch/x86_64/reset.rs`. The banner is wrong
for it, and the split fixes that for free.

## The verdict: split it by table, with tests and proofs beside each

Moving only the tests out gives `acpi.rs` 1,630 lines and passes. It is the wrong axis here. The
module is not one type. It is seven tables that share a header, a checksum and three byte readers,
and each table is read by different callers. The x86 kernel reads the MADT, MCFG, DMAR and FADT.
The aarch64 loader reads the MADT, GTDT, SPCR and FADT. Each table already has its own tests,
and the root walk, MADT, MCFG and DMAR have their own harnesses. They are only stored in one place.
A by-table split keeps each table's code, tests and proofs in one file, which is the shape `ivrs`
already has.

A child module reaches its parent's private items, so the shared readers and `AcpiError` stay
private to `acpi`. Nothing widens past `acpi`.

## A proposed cut

Every name here is provisional. The table names are the signatures ACPI gives them.

| module, provisional | from the first table | about |
|---|---|---|
| `acpi` (`acpi.rs`) | 1 to 241, the byte readers, the root's 3 harnesses and tests, `pub use` lines | 570 |
| `acpi::madt` | 242 to 534, 2 harnesses | 790 |
| `acpi::mcfg` | 535 to 585, 1 harness | 175 |
| `acpi::dmar` | 586 to 1145, 2 harnesses | 1,140 |
| `acpi::fadt` | the Arm boot flags and the reset register | 195 |
| `acpi::gtdt` | the GTDT | 85 |
| `acpi::spcr` | the SPCR | 90 |
| `acpi::ivrs` | unchanged | 694 |

Legacy IRQ routing goes in `madt` because `isa_irq_table` takes a MADT body and reads its
overrides. GTDT and SPCR could share one module for the Arm-only tables; one per table matches
`ivrs`. `acpi.rs` ends near 570 lines, and `dmar` is the largest new file at about 1,140.

The shared test builders stay in `acpi`'s `tests` as `pub(super)`. One test,
`no_parser_reads_past_a_table_that_ends_early`, calls the RSDP, header, MADT and DMAR parsers.
It stays in `acpi` and reaches the MADT and DMAR fixtures through `pub(super)`. Its body does not
change.

## What an architect has to rule

1. Private modules with `pub use`, or public modules like `ivrs`. The recommendation is private.
   Thirteen Rust files outside the crate name `machine_discovery::acpi` items, in code or in
   comments. With re-exports none of them changes. Public modules would add a second path to every item, such as
   `acpi::dmar::DmarUnits`, which is a public crate API change. The reason holds at equal cost.
2. Whether the Kani harnesses move beside their tables or stay together as `acpi/verification.rs`.
   The recommendation is beside their tables. `riscv64`, `x86_64` and `framebuffer` in this crate
   each carry their own `verification`. The eight falsification patches have to be regenerated
   either way, since every one diffs `acpi.rs`. Renaming five of them at the same time costs
   little. The reason holds at equal cost.
3. Every module name in the second table.

No wire format, syscall or public path changes under any answer here.

## What moving the code breaks

Found with `grep` at `396187b0b`. A lane has to carry each one.

- Eight falsification patches in `crates/machine_discovery/falsifications/` diff `acpi.rs`.
  Their file names carry the harness's module path, `acpi.verification.<name>.patch`. Under
  ruling 2's recommendation five of them move: two to `acpi.madt.verification.*`, two to
  `acpi.dmar.verification.*`, one to `acpi.mcfg.verification.*`. The three root harnesses keep
  their names. Each patch is regenerated, each `Falsification: replayable` line in the source is
  updated to match, and each is shown to fail again. `script/falsifications --check` fails until
  the names and records agree.
- The per-pull-request falsification sweep is crate-granular, per `script/falsifications`. A
  change to this crate replays all 16 of its patches, not just these 8.
- `kani-reach.yml` lists `machine_discovery:8` by package and shard count, so it is unaffected.
  `notes/kani-reach-2026-10-04.csv` names the eight harness paths; it is a dated measurement and
  stays as it is.
- No fuzz target under `fuzz/` reads ACPI.
- No live citation of the form `acpi.rs:NNNN` exists. Milestone 438 (would a diff-scoped mutation check
  have caught the 55) cites `acpi.rs:491:25` in a table of results from its day, and that stays.
- 10 notes and design files name the path, besides the patches and two baseline files. All
  still resolve, since `acpi.rs` stays. Milestone 161 (the x86_64 kernel port: bring up the HAL's
  third architecture) and notes on the HPET describe what `acpi.rs` decodes, and each is history.
- `design/file-length-baseline.tsv` has a row for this file at 3,009. It goes in the same change.
  `design/comment-block-baseline.tsv` holds the module header (`acpi.rs:1`, 61 lines), which
  stays in `acpi.rs`. Its list of what is here is already out of date: it names MADT and MCFG
  only. Rewriting it as a list of the child modules must not grow it past 61.
- The doc comments link items like [`DmarUnits`] and [`mcfg_entry`] by bare name. An intra-doc
  link resolves from the module that holds it, so each one that crosses a new boundary needs a
  path.
- 4 first-parent merges in the 14 days to 2026-10-11 touched it. The file is not hot.

## Done when

1. `acpi.rs` is under 1,000 lines, and no new file is over 1,500.
2. No Rust file outside `crates/machine_discovery/src/` changes, and every `machine_discovery::acpi`
   path callers use today still resolves.
3. The crate's 61 ACPI host tests pass with no test body changed. `script/test` passes on
   aarch64, riscv64 and x86_64, since the x86 kernel and the aarch64 loader both read these tables.
4. The 8 harnesses pass, and their 8 patches apply and fail under their new names.
5. `cargo doc` for the crate reports no broken intra-doc link.
6. Each new module carries a `//! Name:` block marked provisional.

## What was built

Built 2026-10-12 (UTC). `crates/machine_discovery/src/acpi.rs` went from 3,009 lines to 94, an
index of module docs and BUGS, `mod` lines and named re-exports, so its file-length and
parent-index rows are gone. Every name below is provisional.

| file | lines | holds |
|---|---|---|
| `acpi/sdt.rs` | 150 | `AcpiError`, the checksum, the table header; 1 harness, 2 tests |
| `acpi/root.rs` | 299 | the RSDP and the root walk; 2 harnesses, 7 tests |
| `acpi/madt.rs` | 775 | the MADT and legacy IRQ routing; 2 harnesses, 18 tests |
| `acpi/mcfg.rs` | 191 | the MCFG; 1 harness, 3 tests |
| `acpi/dmar.rs` | 1,143 | the DMAR and `DmarUnits`; 2 harnesses, 23 tests |
| `acpi/gtdt.rs`, `acpi/spcr.rs`, `acpi/fadt.rs` | 91, 97, 219 | one table each; 2, 2 and 3 tests |
| `acpi/bytes.rs` | 21 | the three readers `ivrs` also uses |
| `acpi/tests.rs` | 150 | shared fixtures and the one test that reads four tables |

Each item on the list above holds. The 61 tests pass under the same names with no body changed.
The 8 harnesses verify, and their patches, renamed to `acpi.<table>.verification.*`, apply and
go red. `cargo doc` with private items reports no warning. `script/test` passes on all three
architectures. `git diff --color-moved` shows every changed code line moved, except visibility,
imports and module scaffolding.

## Follow-on

- **Decision.** `design/decisions/0293-the-acpi-table-parser-splits-into-one-private-module-per-table.md`
  records the cut. Every module name it lists waits for calef's naming pass, marked at each file's
  `//! Name:` block.

## Index row

The ACPI parser is 3,009 lines and grew 1,043 in two weeks. It is seven firmware tables sharing a
header and a checksum, with their tests and proofs stored at the bottom in no order. This splits
it into one private child module per table, each holding its own tests and harnesses, beside the
`ivrs` module that already works that way. The cut, the harness placement and every name are an
architect's call.
