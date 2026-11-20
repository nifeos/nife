---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1093. `globally_unique_identifier_partition_table`: what an outside maintainer would fix before 0.1.0

Raised by a read-only review of `crates/globally_unique_identifier_partition_table` as a stranger
to this tree would meet it, before its move to `nifeos/globally_unique_identifier_partition_table`
at step 2 of milestone 1017 (the contract crates leave in this order). It changes nothing; it lists
what the next lanes change.

The review took its accepted baseline from
[the review package](../../notes/crate-reviews/globally-unique-identifier-partition-table.md) and
reopens none of the decisions it settles or holds. Those are the crate name and the six ratified
names, and the sixteen rows of its name table. They include its two decisions for the architect,
now ruled below: where the two nife type GUIDs live, and whether `lba` and `mbr` spell out. They also include its two
open questions: a totality harness for `parse`, and whether `VERSION` stays. Where this block touches a name the package lists, it says
so and adds only what the package did not see.

Reuse: the template `nifeos/crate-workflows` and its scaffold, Kani, cargo-mutants, cargo-fuzz, the
crate's own `testing::sample_disk` and its two real fixtures. No new tool. The one new dependency
is this tree's own `master_boot_record`, which holds the protective-MBR code.

## Finding status

Every item not Done blocks the first crates.io publish, which waits until every outside-review
milestone reaches delivery.

- P0-1, nife vocabulary in the sources: Open. The move commit strips the twelve `Name:` blocks and
  rewrites the comment on `Cargo.toml` line 11.
- P1-2, `Entry::blocks` panics on a hand-built entry: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-3, no way in through the backup table: Done. `parse_backup` and its host tests are in
  [#3566](https://github.com/nifeos/nife/pull/3566); the Kani round trip through
  `write_backup_header`, with its patch, is in [#3837](https://github.com/nifeos/nife/pull/3837).
- P1-4, nothing compares the header with the device: Done for this crate. `check_disk_size` is in
  [#3566](https://github.com/nifeos/nife/pull/3566). No nife caller calls it, and each caller's
  `BUGS` section says so ([#3837](https://github.com/nifeos/nife/pull/3837)); wiring the check in is each program's own change.
- P1-5, `parse` cannot tell where the array came from: Open. The cheap form (documentation and the
  README example) is in [#3566](https://github.com/nifeos/nife/pull/3566); the firm form, a
  `parse` that takes the block it read the array from, is two-way before the tag and the
  maintainer rules it provisionally.
- P1-6, "no `unsafe`" is only prose: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-7, twenty-three undocumented public items: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-8, the error type is not an `Error`: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-9, the name gates refuse ten register names and one test name: Done for the names Decisions 2
  and 5 rule, as provisional names in [#3821](https://github.com/nifeos/nife/pull/3821). The names
  the decisions do not reach (`ATTR_*`, `Error::EntrySize`, `BackupMismatch::EntrySize`, the
  `transfer` parameters, `PartitionsOverlap { a, b }`, `entry`) stay in the package's name table.
- P1-10, the prose gate refuses nineteen acronyms: Done, [#3837](https://github.com/nifeos/nife/pull/3837). Each is expanded at first use or
  reworded. The gate still refuses `GPT` and `RFC` beside their expansions, a false positive of
  the template's (see `BUGS`).
- P1-11, the mutation verdict needs the recorded equivalents: Open. The move lane carries the six
  equivalents into `.github/mutants-exempts` after the first full run in the new repository.
- P1-12, the README is not the template's: Open. The badge row, the measured Kani figures and a
  stranger run need the new repository.
- P2-13, `check_backup` is stricter than the specification: Done (documented, strictness kept),
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-14, the proofs stop short of the hostile-input surface: Open for the fuzz target only, which
  is in nife's `fuzz/fuzz_targets/` and moves with the crate. The `blocks` harness is in
  [#3566](https://github.com/nifeos/nife/pull/3566). The harnesses for the protective MBR check,
  `Span::covering`, `name_utf8` and the two stages of `Header::decode`, each with its patch, are in
  [#3837](https://github.com/nifeos/nife/pull/3837); the protective MBR harness lives in `master_boot_record`.
- P2-15, `Debug` prints the entry array: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-16, two casts narrow on a 32-bit host: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-17, nothing checks the entry's reserved tail: Done (the crate root states which rules it
  enforces), [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-18, a longer header re-emits as 92 bytes: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-19, callers add `uuid` themselves: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-20, a dead MBR record reads as a hybrid: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- Decision 1, the type name `PartitionTable`: Ruled (maintainer, provisional, 2026-11-14 UTC,
  [#3589](https://github.com/nifeos/nife/pull/3589)). Built as a provisional name
  ([#3821](https://github.com/nifeos/nife/pull/3821)), with no alias; the architect ratifies it with
  the name table.
- Decision 2, the size constants take their unit: Ruled (maintainer, provisional, 2026-11-19 UTC).
  Built as provisional names ([#3821](https://github.com/nifeos/nife/pull/3821)).
- Decision 3, `uuid` stays in the public API: Ruled (architect, 2026-11-19 UTC). The crate documentation
  carries the item ([#3821](https://github.com/nifeos/nife/pull/3821)); the dependency was already
  `1.28.0` with default features off.
- Decision 4, where the nife type GUIDs live: Ruled (architect, 2026-11-19 UTC). Built
  ([#3821](https://github.com/nifeos/nife/pull/3821)): `NIFE_DATA` and `NIFE_BOOT` live in the
  crate `nife_partition_types` (provisional name), values unchanged, and `uefi_loader` and every
  reader use it.
- Decision 5, spelling out `lba` and `mbr`: Ruled (architect, 2026-11-19 UTC). Built as
  provisional names ([#3821](https://github.com/nifeos/nife/pull/3821)); the spelling-out reaches
  every `lba` component and the `mbr` module, with the specification's spellings as
  `#[doc(alias)]`.
- Decision 6, review clearance and repository go: Ruled (maintainer, provisional, 2026-11-19 UTC).
  Clearance follows decisions 4 and 5 and the name ratification.
- Package questions, a totality harness for `parse` and whether `VERSION` stays: Open. Both are
  two-way and the maintainer rules them provisionally; `VERSION` is at `src/lib.rs` line 54.
- Follow-on, the protective-MBR code moves to `master_boot_record`: Ruled (maintainer, provisional,
  2026-11-19 UTC). Built, [#3837](https://github.com/nifeos/nife/pull/3837): the record's write and check are `master_boot_record::protective`
  (provisional names), and this crate depends on it. `master_boot_record::write`, `validate` and
  `MasterBootRecordProblem` stay this crate's entry points, fitted to any accepted block size.

## Open decisions

None. The review clearance follows the name ratification, which is the architect's, one name at a time.

## Goal

A stranger who finds the crate on crates.io can tell in one screen what it is for. The stranger can
build, test, prove and falsify it from the README alone. The stranger can recover a disk whose
primary table has damage (the case the format exists for). Every function the crate exports is total
on the inputs its documentation admits. The repository's gates are green for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1093-globally-unique-identifier-partition-table-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1093-globally-unique-identifier-partition-table-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S half a day, M a day or two. A door is one-way when
somebody outside the repository acts on it after the release (a public name, a published behavior,
a layout, a dependency); everything else is two-way and the maintainer decides it. Line numbers are
at the head this review read.

### P0: blocks the move to its own repository

1. The sources carry nife's vocabulary, which the template's `check / standalone` refuses.
   LEFT. The move commit strips the `Name:` blocks and the manifest comment.
   Finding: `Name:` provenance blocks at `src/lib.rs` line 17, `src/lib.rs` line 44, `src/lib.rs` line 53,
   `src/lib.rs` line 59, `src/header.rs` line 19, `src/guid.rs` line 19, `src/guid.rs` line 33, `src/mbr.rs` line 7,
   `src/table.rs` lines 4, 102 and 350 and `src/error.rs` line 3. The `publish = false` line at `Cargo.toml` line 11 carries a
   comment naming `crates/publish-allowlist.txt`, a file the new repository will not have. The
   `lib.rs` line 17 block cites a `design/decisions/` section. The template's `standalone.py` refuses a
   `Name:` block and the template's `AGENTS.md` refuses `design/` paths. Two of the blocks quote a
   person, which the tree's own rule for records forbids; stripping them resolves that too.
   Evidence: the lines above; `crate-workflows/AGENTS.md`, "Comments and names".
   Change: the move commit strips every `Name:` block (the ruled procedure) and replaces the manifest
   comment with one sentence a stranger can act on ("set to `true` only in the release that
   publishes"). The runbook already owns the strip; this item is here so the gate's first run is
   not its first discovery.
   Door: two-way. Effort: XS, inside the move lane.

### P2: later

The P2 items are in [the appendix](1093-globally-unique-identifier-partition-table-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1093-globally-unique-identifier-partition-table-outside-review/before-the-tag.md).

## Decisions for the architect

Each decision below is the rule in force from 2026-11-19 UTC. The maintainer's rulings are provisional.

### Decision 1: the name of the one type a user holds

Background. The crate's central type is `GloballyUniqueIdentifierPartitionTable`, forty-two
characters, reached as `globally_unique_identifier_partition_table::GloballyUniqueIdentifierPartitionTable`.
The crate name stays as ratified and this block does not touch it. The type's name is a separate name,
listed in the package's public API table without a row of its own, and it will be in every
signature, error message and search result a user meets. In nife, `xtask/src/confirm.rs` already
imports it `as Table`.

Recommendation: `PartitionTable`. The crate path already says which partition table, so the type
says what it is inside that path, as `std::collections::hash_map::HashMap` does not repeat
"collections" and `gpt_disk_types::GptHeader` repeats only the three letters. The recommendation is
about reading, not effort: the rename is mechanical across sixteen files outside the crate. The cost is one more name
in the family (the crate and its type no longer match), which `uuid::Uuid` shows is not the only
convention. Refused alternatives: `Table` (the generic word the naming page warns against) and
`Gpt` (the acronym the ratified crate name avoids).

Prior art: `gpt::GptDisk`, `gpt_disk_types::GptHeader` and `GptPartitionEntryArray`,
`uuid::Uuid`, `HashMap`. Concrete example: today's README line reads
`GloballyUniqueIdentifierPartitionTable::parse(&disk[512..1024], &disk[1024..1024 + 16384])?`; the
recommendation makes it `PartitionTable::parse(...)`.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The type becomes `PartitionTable`, with no alias. Three callers
already import it `as Table` (`xtask/src/confirm.rs` line 41, `xtask/src/rollback.rs` line 66,
`uefi_loader/src/chooser.rs` line 38). A tag is a pre-release that only nifeos repositories pin.
Effort: XS.

### Decision 2: the size constants take their unit

Rule (maintainer, provisional, 2026-11-19 UTC; two-way until the first tag). The crate renames
`entry::SIZE` to `ENTRY_BYTES`, the private `SIZE_U32` to `ENTRY_BYTES_U32`, `HEADER_SIZE` to
`HEADER_BYTES`. `at::HEADER_SIZE` and `at::ENTRY_SIZE` become `at::HEADER_BYTES` and `at::ENTRY_BYTES`.
`Header::entry_size` becomes `entry_bytes` (alias `SizeOfPartitionEntry`), `MINIMUM_BLOCK_SIZE` and
`MAXIMUM_BLOCK_SIZE` become `MINIMUM_BLOCK_BYTES` and `MAXIMUM_BLOCK_BYTES`. The field `block_size` becomes
`block_bytes`. `mbr::SIZE_LBA` becomes `mbr::SIZE_BLOCKS` (alias `SizeInLBA`). The test becomes
`the_three_reads_a_partition_table_reader_makes`. One rename lane does this with the package's
name table. This pull request changes no crate code.

Why. The naming rules end a size with its unit, and the template's name gates refuse all eleven
names. The units rule came after #2797 ratified the two block limits.

Prior art. `gpt_disk_types` leaves the unit to a `BlockSize` type; Linux writes
`sizeof_partition_entry`; this tree's page contracts spell `PAGE_BYTES`. Nife puts the unit in the
name. Example: `[0u8; entry::ENTRY_BYTES]` reads as 128 bytes.

### Decision 3: `uuid` stays in the public API

Rule (architect, 2026-11-19 UTC). The crate keeps `uuid::Uuid` in its public signatures, with a
caret floor of 1.28.0 and default features off. A GUID is an opaque sixteen-byte identifier: the
crate checks and generates no version or variant. The rename lane adds a documentation item to
the crate docs that says so (the maintainer's two-way call).

Why. `Uuid` is what users print, parse and compare (253 million recent downloads against 705
thousand for `uguid`), and a proof covers the two byte swaps this crate adds over all 2^128 values.

Prior art. `gpt` uses `uuid`; `gpt_disk_types` and `uefi` use `uguid`. Nife keeps the order every
tool prints and pays sixteen byte moves per read. Example: a downstream holding a `uefi` `Guid`
converts with `Uuid::from_bytes(guid.to_bytes())`.

### Decision 4: where the nife partition type GUIDs live

Rule (architect, 2026-11-19 UTC). `NIFE_DATA` and `NIFE_BOOT` move, values unchanged, to a crate
that stays in nife, provisionally `nife_partition_types`, and leave `types::name`. Whether the
identifiers say nife or Moho stays open and two-way while the crate is unpublished.

Why. Every installed disk carries these values, so they never change, and a standalone crate may
not name one system.

Prior art. `gpt_disk_types` and `gpt` carry the well-known types and leave a system's private
types to that system; systemd keeps its Discoverable Partitions GUIDs in systemd. Example:
`uefi_loader` reads the data type from the new crate, and `types::name` lists only public types.

### Decision 5: spelling out `lba` and `mbr`

Rule (architect, 2026-11-19 UTC). The crate spells out `logical_block_address` and
`master_boot_record`, as §154 (the acronym test is whether the phrase is spoken) spelled out `gpt`.
The rename lane applies it to the header fields, `Entry::first_lba` and `last_lba`,
`PRIMARY_HEADER_LBA`, the table readers, two error variants and the `mbr` module, enum, variant and
functions.

Prior art. `gpt_disk_types` writes `Lba` and `MasterBootRecord`; Linux and FreeBSD write `lba`.
Nife applies the acronym test, which most systems do not. Example: `backup_header_lba()` becomes
`backup_header_logical_block_address()`.

### Decision 6: review clearance and the repository

Rule (maintainer, provisional, 2026-11-19 UTC). The review clears once decisions 4 and 5 and the
name ratification have landed in their lanes. The architect ratifies the names and gives the go
from [the review package](../../notes/crate-reviews/globally-unique-identifier-partition-table.md)
then. No tag and no publication.

Prior art. `generational_table`, `block_roster` and `file_allocation_table` each moved on a cleared
review with no tag. Example: `script/crate-move globally_unique_identifier_partition_table` then
runs with the renamed constants and no nife GUID.

## Done when

The checks are in [the appendix](1093-globally-unique-identifier-partition-table-outside-review/done-when.md).

## BUGS

- Item 11's figures come from the recorded baseline and triage, not from a run. The first full
  `mutation.yml` run on the new repository is the figure that counts. The two recorded timeouts (the
  exhaustive sweeps) may need the shard timeout raised.
- Items 16 and 17 and the 32-bit remarks come from reading; the crate names no 32-bit target.
- Item 13's and item 20's "no tool does this" stay unverified beyond the two fixtures.
- Item 3's normalization of the backup to the primary's orientation assumes the primary array sits
  at block 2, which every tool writes and the specification does not require. The restored header
  says so, and a caller that knows better can edit the `Header` before writing.
- The template's `prose_acronyms` gate refuses `GPT` and `RFC` at first use even beside their
  expansions, and `U+FFFD` outside code. The fix is the template's;
  until it lands, the crate keeps the expansions and writes the code point as code.

## Follow-on

- **Outstanding.** The architect's review clearance and name ratification, then the move lane:
  P0-1, P1-11, P1-12 and the fuzz target of P2-14, which need the new repository. The maintainer
  rules P1-5's firm form and the two package questions provisionally. The template fixes its
  `prose_acronyms` false positives.

## Index row

An outside review of `globally_unique_identifier_partition_table` before its move and first
release: one item that blocks the move (nife's vocabulary in the sources), eleven to land before the
tag, and eight for later. The eleven cover these. A panic in `Entry::blocks` on a hand-built entry.
The missing path through the backup table that the specification requires. A disk-size check no
caller performs. The array-location guess every caller makes. The `forbid(unsafe_code)` line. The
twenty-three undocumented items. The error type's traits. The template's name, prose and mutation
gates. The README's proof sections. The central type becomes `PartitionTable`; the size
constants take their unit, `uuid` stays, the nife type GUIDs leave, and `lba` and `mbr` spell out.
