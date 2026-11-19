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
names, and the sixteen rows of its name table. They include its two decisions for the architect:
where the two nife type GUIDs live, and whether `lba` and `mbr` spell out. They also include its two
open questions: a totality harness for `parse`, and whether `VERSION` stays. Where this block touches a name the package lists, it says
so and adds only what the package did not see.

Reuse: the template `nifeos/crate-workflows` and its scaffold, Kani, cargo-mutants, cargo-fuzz, the
crate's own `testing::sample_disk` and its two real fixtures. No new tool and no new dependency.

## Finding status

Every item not Done blocks the first crates.io publish, which waits until every outside-review
milestone is delivered.

- P0-1, nife vocabulary in the sources: Open. The move commit strips the twelve `Name:` blocks and
  rewrites the comment on `Cargo.toml` line 11.
- P1-2, `Entry::blocks` panics on a hand-built entry: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-3, no way in through the backup table: Open. `parse_backup` and its host tests are in
  [#3566](https://github.com/nifeos/nife/pull/3566); the Kani round trip through
  `write_backup_header` is not written.
- P1-4, nothing compares the header with the device: Open. `check_disk_size` is in
  [#3566](https://github.com/nifeos/nife/pull/3566); no nife caller calls it, and no caller has a
  `BUGS` line saying so.
- P1-5, `parse` cannot tell where the array came from: Open. The cheap form (documentation and the
  README example) is in [#3566](https://github.com/nifeos/nife/pull/3566); the firm form, a
  `parse` that takes the block it read the array from, is two-way before the tag and the
  maintainer rules it provisionally.
- P1-6, "no `unsafe`" is only prose: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-7, twenty-three undocumented public items: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-8, the error type is not an `Error`: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P1-9, the name gates refuse ten register names and one test name: Open. The replacement names
  are calef's to ratify (see "The size constants take their unit" below); a rename lane follows.
- P1-10, the prose gate refuses nineteen acronyms: Open. The move lane runs the template's
  `prose_acronyms` gate and expands each at first use.
- P1-11, the mutation verdict needs the recorded equivalents: Open. The move lane carries the six
  equivalents into `.github/mutants-exempts` after the first full run in the new repository.
- P1-12, the README is not the template's: Open. The badge row, the measured Kani figures and a
  stranger run need the new repository.
- P2-13, `check_backup` is stricter than the specification: Done (documented, strictness kept),
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-14, the proofs stop short of the hostile-input surface: Open. The `blocks` harness is in
  [#3566](https://github.com/nifeos/nife/pull/3566); the harnesses for `mbr::validate`,
  `Span::covering`, `name_utf8` and the two-stage `Header::decode` are not written, and the fuzz
  target is in nife's `fuzz/fuzz_targets/`.
- P2-15, `Debug` prints the entry array: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-16, two casts narrow on a 32-bit host: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-17, the entry's reserved tail is unchecked: Done (the crate root states which rules it
  enforces), [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-18, a longer header re-emits as 92 bytes: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-19, callers add `uuid` themselves: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- P2-20, a dead MBR record reads as a hybrid: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- Decision 1, the type name `PartitionTable`: Ruled (maintainer, provisional, 2026-11-14 UTC,
  [#3589](https://github.com/nifeos/nife/pull/3589)). Not built: `src/table.rs` line 13 still
  declares `GloballyUniqueIdentifierPartitionTable`; calef ratifies with the name table.
- Decision 2, `uuid` in the public API: Open. The maintainer's provisional ruling keeps it; the
  door turns one-way at the first publish, so it is calef's (see "`uuid` stays in the public API"
  below).
- Package decision 1, where the two nife type GUIDs live: Open. Calef's (see "Where the nife
  partition type GUIDs live" below); `NIFE_DATA` is at `src/guid.rs` line 146.
- Package decision 2, whether `lba` and `mbr` are spelled out: Open. Calef's (see "Whether `lba`
  and `mbr` are spelled out" below).
- Package decision 3, review clearance and repository go: Open. Calef's (see "Review clearance and
  the repository" below).
- Package questions, a totality harness for `parse` and whether `VERSION` stays: Open. Both are
  two-way and the maintainer rules them provisionally; `VERSION` is at `src/lib.rs` line 54.
- Follow-on, the protective-MBR code moves to `master_boot_record`: Ruled (maintainer, provisional,
  2026-11-19 UTC). Not built: it waits for the staged `master_boot_record` crate
  ([#3731](https://github.com/nifeos/nife/pull/3731)), after which `src/mbr.rs`'s write and
  validate code moves there and this crate depends on it.

## Open decisions

### The size constants take their unit

Background. The crate's sizes are named `entry::SIZE`, `header::HEADER_SIZE`,
`header::at::HEADER_SIZE`, `header::at::ENTRY_SIZE`, `Header::entry_size`, `MINIMUM_BLOCK_SIZE`,
`MAXIMUM_BLOCK_SIZE`, `mbr::SIZE_LBA`, the field `block_size` and the private `entry::SIZE_U32`;
one test is `the_three_reads_a_gpt_reader_makes`. The naming rules say a size or a limit ends with
its unit, and `gpt` is spelled out. The template's name gates refuse all eleven, so the new
repository's checks are red until they change. Two of them (`MINIMUM_BLOCK_SIZE`,
`MAXIMUM_BLOCK_SIZE`) were ratified in #2797 before the units rule; a release review rechecks a
name ratified before a later rule. After the tag each public one is a breaking rename.

Recommendation: A. Rename them to the review package's spellings plus item 9's four:
`ENTRY_BYTES` and `ENTRY_BYTES_U32`, `HEADER_BYTES`, `at::HEADER_BYTES` and `at::ENTRY_BYTES`,
`entry_bytes` (alias `SizeOfPartitionEntry`), `MINIMUM_BLOCK_BYTES`, `MAXIMUM_BLOCK_BYTES`,
`block_bytes`, `mbr::SIZE_BLOCKS` (alias `SizeInLBA`) and
`the_three_reads_a_partition_table_reader_makes`.

- A. The spellings above, in one rename lane with the rest of the package's name table.
- B. Ratify the public names as they stand and rename only the private ones the gate refuses; the
  gate then needs an exemption for each public one.
- C. Choose other unit words (for example `_LENGTH`), which the units rule does not use.

Door: one-way at the first tag that a nifeos repository pins and at publish; two-way until then.

Prior art. `gpt_disk_types` spells `BlockSize` as a type and leaves the unit to it; Linux's
`efi.c` writes `sizeof_partition_entry`; this tree's page contracts spell `PAGE_BYTES`, and the
units rule's own example is `MAXIMUM_NAME_BYTES`. Where nife differs: the unit is in the
name, not in a type or a comment. Example: `[0u8; entry::ENTRY_BYTES]` reads as 128 bytes, where
`[0u8; entry::SIZE]` leaves the reader to look it up.

Blocks: the tag.

### `uuid` stays in the public API

Background. The one type from outside in a public signature is `uuid::Uuid`, with default features
off. The GPT ecosystem's alternative is `uguid::Guid`, which stores a GUID in the on-disk
mixed-endian order so that reading sixteen bytes is a copy, and which the `uefi` crate and
`gpt_disk_types` share. After the first publish the choice is fixed: a published signature's type
cannot change without a major version. The maintainer ruled provisionally on 2026-11-14 (UTC) to
keep `uuid`; `uuid` is already a dependency (1.28.0, default features off).

Recommendation: A, keep `uuid`. It is the type a user prints, parses and compares everywhere else
(253 million recent downloads against 705 thousand), and the crate's RFC 9562 order is what every
printed GUID shows. The two byte swaps this crate adds hold over all 2^128 values (proved), and
`pub use uuid` saves a caller the second manifest line.

- A. Keep `uuid::Uuid`.
- B. Switch to `uguid::Guid`, matching `gpt_disk_types` and `uefi`.
- C. A sixteen-byte newtype of the crate's own: a third GUID type with no parser or printer.

Door: one-way at the first crates.io publish; two-way until then.

Prior art. `gpt` 4.1 uses `uuid`; `gpt_disk_types` and `uefi` use `uguid`. Where nife differs: it
keeps the order every tool prints and pays sixteen byte moves per GUID read. Example:
`uefi_loader/Cargo.toml` depends on this crate and `uuid` and converts nothing; a downstream that
uses `uefi`'s `Guid` converts through `Uuid::from_bytes(guid.to_bytes())` plus `guid::from_disk`,
two lines.

Blocks: the first publish.

### Where the nife partition type GUIDs live

Background. `types::NIFE_DATA` and `types::NIFE_BOOT` are the partition type GUIDs of nife's data
partition and boot slot. Every installed disk carries them, and the loader, the installer and the
surveyor match on them, so their values never change. A crate that leaves for its own repository
may not mention nife, and these two name it in both identifier and purpose. The move cannot finish
while they are here.

Recommendation: A. Move them, values unchanged, to a crate that stays in nife, and drop them from
`types::name`.

- A. A crate that stays in nife holds them.
- B. Keep them in this crate under neutral names: the crate still carries one system's identity.
- C. Keep them and exempt them from the standalone rule, as wire literals are exempt.

Door: one-way for where a published crate's constants are; the values themselves never change.

Prior art. `gpt_disk_types` and `gpt` carry the well-known types (EFI system, Linux filesystem,
Microsoft basic data) and leave a system's private types to that system; systemd's Discoverable
Partitions Specification keeps its GUIDs in systemd. Where nife differs: nothing; this follows
them. Example: after A, `uefi_loader` reads `nife_partition_types::DATA` (name to choose) and this
crate's `types::name` lists only public types.

Blocks: the move.

### Whether `lba` and `mbr` are spelled out

Background. The crate uses `lba` (logical block address) in the five `Header` address fields,
`Entry::first_lba` and `last_lba`, `PRIMARY_HEADER_LBA`, six table readers and the
`BackupMismatch` and `MbrProblem` variants, and `mbr` (master boot record) in a module, an error
enum, an `Error` variant and two functions. The tree's acronym test asks whether the phrase is
spoken; §154 (the acronym test is whether the phrase is spoken) spelled out `gpt`, an acronym from
the same specification. The answer settles most of the package's name table.

Recommendation: A, spell both out, as §154 spelled out `gpt`.

- A. Spell out both (`logical_block_address`, `master_boot_record`).
- B. Keep both whole, as spoken acronyms, with each expanded at first use in the documentation.
- C. Spell out `mbr` only; `lba` is the specification's field suffix in every header name.

Door: one-way at the first tag that a nifeos repository pins and at publish.

Prior art. `gpt_disk_types` writes `Lba`, `LbaLe` and `MasterBootRecord`; Linux writes `lba` in
`efi.c`; FreeBSD writes `hdr_lba_alt`. The protective-MBR follow-on names its target crate
`master_boot_record`. Where nife differs: the acronym test, which most systems do not apply.
Example: under A, `table.backup_header_lba()` becomes `table.backup_header_logical_block_address()`;
under B it stays.

Blocks: the move.

### Review clearance and the repository

Background. The crate's review package is
[`notes/crate-reviews/globally-unique-identifier-partition-table.md`](../../notes/crate-reviews/globally-unique-identifier-partition-table.md).
Clearing a crate review is the go to create its repository under nifeos. The package recommends
clearing once the two decisions above and the name ratification land, with no tag and no
publication.

Recommendation: A, clear once "Where the nife partition type GUIDs live", "Whether `lba` and `mbr`
are spelled out" and the name ratification are ruled and built.

- A. Clear after those rulings land.
- B. Clear now, and let the move lane carry the rulings into the new repository.
- C. Hold the move until the P1 items are done as well.

Door: one-way: a repository that exists has a name others link to.

Prior art. In this tree, `generational_table`, `block_roster` and `file_allocation_table` each
moved on a cleared review with no tag. Where nife differs: nothing outside nife pins the crate yet.
Example: after A, `script/crate-move globally_unique_identifier_partition_table` runs with the
renamed constants and no nife GUID.

Blocks: the move.

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
   Finding: `Name:` provenance blocks at `src/lib.rs` line 13, `src/lib.rs` line 39, `src/lib.rs` line 47,
   `src/lib.rs` line 55, `src/header.rs` line 19, `src/guid.rs` line 19, `src/guid.rs` line 33, `src/mbr.rs` line 7,
   `src/table.rs` line 4 and `src/error.rs` line 3. The `publish = false` line at `Cargo.toml` line 11 carries a
   comment naming `crates/publish-allowlist.txt`, a file the new repository will not have. The
   `lib.rs` line 13 block cites a `design/decisions/` section. The template's `standalone.py` refuses a
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

The decision below is ruled provisionally by the maintainer. The `uuid` dependency, the package's
two decisions and the size-constant names are calef's and are under "Open decisions" above.

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
- The template's `prose_acronyms` false positive on `U+FFFD` is the template's to fix; until it
  lands, the crate rewords the one comment.

## Index row

An outside review of `globally_unique_identifier_partition_table` before its move and first
release: one item that blocks the move (nife's vocabulary in the sources), eleven to land before the
tag, and eight for later. The eleven cover these. A panic in `Entry::blocks` on a hand-built entry.
The missing path through the backup table that the specification requires. A disk-size check no
caller performs. The array-location guess every caller makes. The `forbid(unsafe_code)` line. The
twenty-three undocumented items. The error type's traits. The template's name, prose and mutation
gates. The README's proof sections. The central type becomes `PartitionTable`; the `uuid`
dependency, the size-constant names, the nife type GUIDs and `lba` and `mbr` are calef's.
