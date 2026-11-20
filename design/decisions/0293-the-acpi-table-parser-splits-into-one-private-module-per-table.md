---
status: DECIDED
raised: 2026-10-12
decided: 2026-10-12
ratified_by: calef
---

# 293. The ACPI table parser splits into one private module per table

Raised 2026-10-12 (UTC) as the cut that
[milestone 875 (the ACPI table parser is split along its seams)](../roadmap/0875-the-acpi-table-parser-is-split-along-its-seams.md)
says is calef's call before a line moves. Written by lane
`milestone/875-the-acpi-table-parser-is-split-along-its-seams`. *(Section number provisional until
the merge queue lands it.)*

It blocked the move and nothing else.

## The ruling

calef asked nothing separate on this section. The block named three forks on 2026-10-11, before
§282 (when a file is split, the parent becomes an index) was ruled, and rulings he had already
given answer each. Modules are private with named re-exports, per §272 (the grant-plan crate root
splits into private modules), whose fork 1 he answered *"Private modules"*, and §279 (the
system-initializer crate root splits into private modules); `ivrs` stays public because it already
is. Tests and Kani harnesses sit beside their table, per §274 (the filesystem-protocol crate root
splits into its module files) and his rulings on [#1913](https://github.com/nifeos/nife/pull/1913)
and [#1915](https://github.com/nifeos/nife/pull/1915). `acpi.rs` becomes an index, per §282, which
calef set on [#1959](https://github.com/nifeos/nife/pull/1959): *"Agreed, make the parent an
index."* That overrides the block's cut, which kept the RSDP, header and root walk in `acpi.rs`.
Names stay provisional for his naming pass. The maintainer recorded this as settled by precedent
on [PR #1972](https://github.com/nifeos/nife/pull/1972#issuecomment-6117079229) at 03:13 (UTC) on
2026-10-12. The forks are kept below as they were raised.

## What the file was

`crates/machine_discovery/src/acpi.rs` was 3,009 lines on main at `17b23264e`, 1,968 of them code
by `helpers/parent_index.py`'s count. The code was 1,367 lines: seven firmware tables (the MADT,
MCFG, DMAR, GTDT, SPCR and the FADT's two readings, plus the RSDP and root tables that reach them)
sharing `AcpiError`, a checksum, the table header and three byte readers. Eight Kani harnesses and
61 tests followed it, in no order by table. `acpi/ivrs.rs`, AMD's counterpart of the DMAR, was
already a child with its own tests.

## The cut

Private modules, children of `acpi`. `acpi.rs` re-exports every public item by name, so every
`machine_discovery::acpi::` path a caller uses today still resolves and no file outside
`crates/machine_discovery/src/` changes. Each table's tests and harnesses sit beside it as inline
`tests` and `verification` modules, the shape `ivrs` already has. Names are provisional.

| file | holds | lines |
|---|---|---|
| `acpi.rs` | module docs and BUGS, `mod` lines, named re-exports | 94 |
| `acpi/bytes.rs` | `u16`, `u32`, `u64` | 21 |
| `acpi/sdt.rs` | `AcpiError`, `is_checksum_ok`, `SdtHeader`, `parse_sdt_header`; 1 harness, 2 tests | 150 |
| `acpi/root.rs` | the RSDP, `root_entry_count`, `root_entry`; 2 harnesses, 7 tests | 299 |
| `acpi/madt.rs` | the MADT, its entry walk, legacy IRQ routing; 2 harnesses, 18 tests | 775 |
| `acpi/mcfg.rs` | `McfgEntry`, `mcfg_entry`; 1 harness, 3 tests | 191 |
| `acpi/dmar.rs` | the DMAR's fixed part and walk, `DmarUnits`; 2 harnesses, 23 tests | 1,143 |
| `acpi/gtdt.rs` | the GTDT; 2 tests | 91 |
| `acpi/spcr.rs` | the SPCR; 2 tests | 97 |
| `acpi/fadt.rs` | the Arm boot flags and the reset register; 3 tests | 219 |
| `acpi/tests.rs` | test fixtures more than one table uses, and the one test that reads four tables | 150 |
| `acpi/ivrs.rs` | unchanged | 694 |

No file is over 1,500. The block's own cut kept the RSDP, header and root walk in `acpi.rs` at
about 570 lines; §282 (when a file is split, the parent becomes an index) landed the same day and
moves them into `root` and `sdt`.

## The forks, as raised, and what answers each

1. **Private modules with named re-exports, or public modules like `ivrs`.** Answered by §272 (the
   grant-plan crate root splits into private modules) and §279 (the system-initializer crate root
   splits into private modules): private, with each public item re-exported by name. Public modules
   would give every item a second path, such as `acpi::dmar::DmarUnits`, which is a public API
   change of its own. `ivrs` stays public because it already is, and changing that would break the
   two callers that name `acpi::ivrs::`.
2. **Harnesses beside their tables, or together in `acpi/verification.rs`.** Answered by §274 (the
   filesystem-protocol crate root splits into its module files), which moved `dir`'s three Kani
   proofs with `dir`, and by the rule calef gave for 842 (#1913) and 845 (#1915), that tests sit
   beside the code they test. This crate's `riscv64`, `x86_64` and `framebuffer` modules each carry
   their own `verification` too. Beside the table is also the cut that widens nothing: the MADT and
   DMAR walk harnesses read the iterators' private cursor `at`, which a sibling module could not
   see.
3. **Every module name.** Not ruled on here. Names ship provisional, each file under a `//! Name:`
   block, for calef's naming pass.

Prior art outside the tree agrees with one module per table, though it does not decide fork 1. Read
on 2026-10-12: rust-osdev's `acpi` crate (`7bf9e4210d`) has `src/rsdp.rs` and `src/sdt/`, where
`sdt/mod.rs` holds `SdtHeader` and one public module per table (`fadt`, `hpet`, `madt`, `mcfg`,
`spcr` and others). Linux (`d6d3455d87`) keeps each Arm table's reader in its own file under
`drivers/acpi/arm64/` (`gtdt.c`, `iort.c`), and the DMAR in `drivers/iommu/intel/dmar.c`.

## What the move costs beyond moving code

- Visibility, all inside `acpi`. The byte readers become `pub(super)` in `bytes`, and `acpi.rs`
  imports them privately so `ivrs`' `use super::{u16, u32, u64}` resolves unchanged. Five private
  constants the shared fixtures and the four-table test read become `pub(super)`: `RSDP_V1_LEN`,
  `RSDP_V2_LEN`, `MADT_FIXED_LEN`, `DMAR_FIXED_LEN` and `DRHD_FIXED_LEN`. Nothing widens past
  `acpi`.
- Shared test fixtures. `seal`, `sdt`, `every_short_prefix_is_refused`, the two RSDPs and the q35
  MADT and DMAR bodies are used by more than one table's tests, so they sit in `acpi/tests.rs` as
  `pub(super)`, beside `no_parser_reads_past_a_table_that_ends_early`, the one test that reads four
  tables. No test body changes.
- The verification module's doc, which spoke for all eight harnesses, stays whole on `root`'s, the
  RSDP harness being the one it calls the crate's security-shaped claim. Each other table's
  `verification` points to it. Those whose harnesses take a byte buffer (`sdt`, `madt`, `dmar`)
  carry their own copy of the input length `N`, which the harnesses are linear in. `mcfg`'s
  harness takes two bus numbers and needs none. A private constant of one module cannot be named from a sibling's
  `cfg(kani)` module without widening it, so the knob is now per table.
- The eight falsification patches are renamed to their harness's new module path
  (`acpi.root.verification.*`, `acpi.sdt.verification.*`, `acpi.madt.verification.*`,
  `acpi.dmar.verification.*`, `acpi.mcfg.verification.*`), regenerated against the new file, and
  shown red again. Each `Falsification: replayable` line follows.
- The ratchets. `design/file-length-baseline.tsv` drops the file's row.
  `design/parent-index-baseline.tsv` drops it too, since `acpi.rs` is now an index. The module
  header stays at line 1 and does not grow past its 61 lines.

## What it does not decide

No public path, wire format or system call changes. No function body changes; the module headers, the
`//! Name:` blocks and the doc paths that cross a new module boundary are the only new prose.
