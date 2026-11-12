---
status: NOT-STARTED
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

Two one-way doors are open that the review package does not hold. The package's own two decisions
(where `NIFE_DATA` and `NIFE_BOOT` live; whether `lba` and `mbr` spell out) stand before these and
are not restated.

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
about reading, not effort: the rename is mechanical across seven callers. The cost is one more name
in the family (the crate and its type no longer match), which `uuid::Uuid` shows is not the only
convention. Refused alternatives: `Table` (the generic word the naming page warns against) and
`Gpt` (the acronym the ratified crate name avoids).

Prior art: `gpt::GptDisk`, `gpt_disk_types::GptHeader` and `GptPartitionEntryArray`,
`uuid::Uuid`, `HashMap`. Concrete example: today's README line reads
`GloballyUniqueIdentifierPartitionTable::parse(&disk[512..1024], &disk[1024..1024 + 16384])?`; the
recommendation makes it `PartitionTable::parse(...)`.

One-way after the first tag. Effort: XS.

### Decision 2: the dependency the public API carries

Background. The one type from outside in a public signature is `uuid::Uuid`, with default features
off. The package lists it as a fact and no record rules it. The alternative the GPT ecosystem uses
is `uguid::Guid` (google), which stores a GUID in the on-disk mixed-endian order so that reading
sixteen bytes is a copy, and which the `uefi` crate and `gpt_disk_types` share. After the first
tag the choice stays fixed: a published signature's type cannot change without a major version.

Recommendation: keep `uuid`. It is the type a user prints, parses and compares everywhere else (253
million recent downloads against 705 thousand). The two swaps this crate adds hold over all
2^128 values (proved). The crate's RFC 9562 order is what every printed GUID in a tool's output is. The cost is the two swaps on every GUID read, which is sixteen byte moves, and a second
`uuid` line in a caller's manifest (item 19 removes the need). Refused alternative: a sixteen-byte
newtype of the crate's own, which would be a third GUID type in the ecosystem with no parser or
printer.

Prior art: `gpt` 4.1 uses `uuid`; `gpt_disk_types` and `uefi` use `uguid`. Concrete example:
`uefi_loader/Cargo.toml` depends on both this crate and `uuid`, and converts nothing, because the
loader does not use the `uefi` crate's `Guid`; a downstream that does would convert through
`Uuid::from_bytes(guid.to_bytes())` plus `guid::from_disk`, two lines.

One-way after the first tag. Effort: none; the decision records the state.

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
gates. The README's proof sections. Two decisions go to the architect: the central type's name
and the `uuid` dependency.
