---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 956, 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1096. `interfaces_note`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/interfaces_note` as a stranger to this tree would meet it,
on a fresh clone at the head of `main`. It changes nothing; it lists what the move lane changes
before `script/crate-move` can finish, and what the architect must rule first.

The crate has no review package in `notes/crate-reviews/`. The settled baseline is therefore the
block of [milestone 956 (the linked interfaces carry checkable versions)](0956-the-linked-interfaces-carry-checkable-versions.md),
which rules the note type `INTERFACES` = 4. It also rules the descriptor shape (a version word, a count, then records of a `u64` id and a `u32` version in strictly ascending id order). It rules the id table with `21` for this crate, the equality check at spawn and the rule that a missing note or a missing entry passes. The audit report "from the package to the grant" under `design/audit-reports/` accepted
the missing-note rule. None of those is reopened here. Step 3 of
[milestone 1017 (the contract crates leave in this order)](1017-the-contract-crates-leave-in-this-order.md)
puts this crate last in its step and says it leaves like any leaf.

Reuse: `script/crate-move`, the template `nifeos/crate-workflows` and its scaffold, Kani,
cargo-mutants, cargo-fuzz. No new tool.

## Finding status

Every item not Done blocks the first crates.io publish, which waits until every outside-review
milestone reaches delivery.

- Item 1, the manifest does not stand alone: Open. `Cargo.toml` still inherits `license` and
  `[lints]` and names a milestone; the move writes the crate's own rows.
- Item 2, no README and no license files: Open. The move stages the scaffold's README and the two
  license files.
- Item 3, the documentation cites this tree: Open. The module doc still speaks of the progenitor, a
  package and nife (`src/lib.rs` lines 3, 4, 15, 16, 41, 47 and 364); the move rewrites it.
- Item 4, the owner string `nife` is the wire format: Open. The bytes keep their spelling behind the
  `// wire literal` marker (decision 1, [#3575](https://github.com/nifeos/nife/pull/3575)); the
  prose lines that name nife reword with item 3.
- Item 5, passive voice over the template's ceiling: Open. The template's gate runs in the moved
  repository, and the rewrite lands with item 3.
- Item 6, two acronyms unexpanded: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 7, a stranger cannot place the note: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 8, the only example does not compile: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 9, private lengths carry a unit: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 9, public lengths carry a unit: Ruled (decision 2, maintainer, provisional, 2026-11-14 UTC,
  [#3589](https://github.com/nifeos/nife/pull/3589)). Not built: `INTERFACE_RECORD` (`src/lib.rs`
  line 111), `interfaces_len` (line 127) and `interfaces_note_len` (line 323) keep their names.
- Item 10, `Declared` names a state: Ruled (decision 2, maintainer, provisional, 2026-11-14 UTC,
  [#3589](https://github.com/nifeos/nife/pull/3589)). Not built: `struct Declared` (`src/lib.rs`
  line 117) is not yet `DeclaredInterface`.
- Item 11, no fuzz target: Open. The opt-in is a per-repository choice, off by default, made at the
  move.
- Item 12, the proof runs one direction: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 13, a test name the abbreviation report flags: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 14, `iter` yields by value: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 15, the triaged timeouts travel nowhere: Open. The move carries the three rows into
  `.github/mutants-exempts`.
- Decision 1, the owner string: Done, [#3575](https://github.com/nifeos/nife/pull/3575).
- Decision 2, ratifying the twenty public names: Open. Calef's (see "Ratify the twenty public
  names" below).
- Decision 3, the crate name: Done, ruled 2026-11-14 UTC with nothing to build; the crate's `Name:`
  block records it ([#3589](https://github.com/nifeos/nife/pull/3589)).
- BUGS, the `NIFE_INTERFACES_NOTE` static: Open. `src/lib.rs` line 381; two-way, so the maintainer
  rules its new name provisionally and a lane renames it before the move.
- Follow-on, review clearance and the repository: Open. Calef's (see "Review clearance and the
  repository" below).

## Open decisions

### Ratify the twenty public names

Background. Every `Name:` block in the crate reads `provisional`, and `script/crate-standalone`
step 0 refuses to move a crate whose names are not ratified. The names become one-way at the first
tag that nife pins, because every program built against the crate spells them. The maintainer
ruled four renames provisionally (decision 2 in
[the appendix](1096-interfaces-note-outside-review/ruled-decisions.md)): `INTERFACE_RECORD_BYTES`,
`descriptor_bytes`, `note_bytes` and `DeclaredInterface`. Sixteen names keep their spelling. Only
calef ratifies.

Recommendation: A, ratify the twenty as decision 2 leaves them, once the four renames land.

- A. Ratify the twenty with the four renames, no alias of the old spellings.
- B. Ratify the sixteen now and the four once they land.
- C. Send one or more back, named, for a new provisional spelling.

Door: one-way at the tag; ratifying is what lets the move begin.

Prior art. `goblin::elf::note::Note` and `object::read::elf::Note` name the record for what it is,
and std names an element count `len` and a byte count `size_of`. Where nife differs: the unit is in
the name and not only in a doc comment. Example: under A,
`let bytes: [u8; descriptor_bytes(3)] = encode_interfaces(&list);` reads as 44 bytes for three
records.

Blocks: the move.

### Review clearance and the repository

Background. Clearing a crate review is the go to create its repository under nifeos. This crate
has no review package in `notes/crate-reviews/`; this milestone is its review. It leaves last in
step 3 of milestone 1017 (the contract crates leave in this order), after the three crates ahead of
it.

Recommendation: A, clear once the four renames land and calef ratifies the names, with no tag and
no publication.

- A. Clear after the renames and the ratification.
- B. Clear now, and let the move wait for them.
- C. Hold until a dry run also stages the move's own items (1 to 5, 11, 15).

Door: one-way: a repository that exists has a name others link to.

Prior art. In this tree `generational_table`, `block_roster` and `file_allocation_table` each moved
on a cleared review with no tag. Where nife differs: nothing outside nife links the crate. Example:
under A, `nifeos/interfaces_note` opens with the README of item 2, and nife pins it by revision.

Blocks: the move.

## Goal

A stranger who opens `nifeos/interfaces_note` can tell in one screen what the note is, place one in
a program from the README alone, and decode one from untrusted bytes without a panic. The move's
dry run exits 0 at the newest template release, with its build step run, and every public name stands
ratified.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1096-interfaces-note-outside-review/what-was-checked.md).

## The items

Each item: what, where, why, the fix, effort (S/M/L), door.

### P0: the move's own dry run fails on these

1. **The manifest does not stand alone.** `crates/interfaces_note/Cargo.toml` line 5 inherits
   `license` and `:12` inherits `[lints]` from a workspace that will not be there, and there is no
   `rust-version`. The staged tree fails at `cargo generate-lockfile` before any test runs, and
   `standalone.py` fails three lines on it. Fix: `license = "MIT OR Apache-2.0"`,
   `rust-version = "1.85"`, the workspace's `cfg(kani)` check-cfg inlined under `[lints.rust]`,
   plus `description`, `readme`, `keywords` and `categories` (`no-std`, `no-std::no-alloc`,
   `encoding`). S. Two-way.
2. **No README and no license files.** The dry run stages the scaffold's README shape and
   `standalone.py` fails on `LICENSE-MIT` and `LICENSE-APACHE`. Fix: a README in the shared order
   (`readme_sections.py`: title and summary, `## What it does`, `## EXAMPLES`, `## What is proven`,
   `## BUGS`, `## Minimum Rust version`, `## License`) and the two license files from the template's
   root. M, because the README carries what items 7 and 8 ask for. Two-way.
3. **The documentation cites this tree.** `Cargo.toml` line 8 and `src/lib.rs` line 6, `:73-76` (the id
   table "Milestone 956 holds", "`script/lint` holds the tree to it"), `:232` and `:498` cite a milestone or a script. The module doc speaks of "the progenitor", "spawn", "a package" and `capability_manifest_note`, none of which a stranger has. The `Name:` blocks, which also carry
   the lane names and the architect's name, vanish in the move and need no edit. Fix: rewrite the module doc to state the format, the reader's rule and what a loader does with it, in the words "the program" and "the system that loads it". The rule is that an unknown version, a count over 64, a length the count does not predict and ids out of order all refuse. S. Two-way.
4. **The word the lint forbids is the wire format.** `src/lib.rs` line 3, `:15`, `:32`, `:34`, `:310`,
   `:326` and `:492` say `nife`. Two of them (`:15`, `:32`) are prose and reword. The other five are
   the owner string `b"nife"`, the section `.note.nife.manifest` and the test that asserts those
   bytes; they are the format itself. The move check and `lint_externalized_crates.py` both fail
   them, and no rewording removes them. Fix: decision 1 keeps the bytes behind a `// wire literal`
   marker, which #3575 applied; the two prose lines reword with item 3. S. One-way, because the
   owner string is in every built program.

### P1: before the tag

5. Passive voice over the template's ceiling. `src/lib.rs`: 19 passives in 2,408 words; the
   template's `main` allows 4 and fails the rest. `v7` does not run the gate; the next release will,
   and the move pins the newest release. Fix inside item 3's rewrite. S. Two-way.
6. Two acronyms unexpanded. `src/lib.rs` line 5 (ABI) and `:28` (NUL). The template's `main`
   fails them at a ceiling of 0. Fix: "application binary interface (ABI)" and "NUL terminator"
   at first use. S. Two-way.
7. A stranger cannot place the note from the documentation. The macro writes to
   `.note.nife.manifest`, and nothing in the crate says that a link script must `KEEP` that input
   section into an output section inside a `PT_NOTE` (and a readable `PT_LOAD`), as
   `crates/user_mode_runtime/link.ld` lines 70 to 72 does. Nothing says that every word is little-endian
   whatever the target's byte order (`put4` writes `to_le_bytes` at `src/lib.rs` line 132, and an ELF
   note's header is in the file's byte order, so a big-endian ELF would carry a wrong header).
   Nothing says where the id registry lives once the crate has left: with the system that reads the
   note, the crate reserving `21` for itself. Nothing in the README will say that `check_served`
   passes an absent entry (`src/lib.rs` lines 246 to 262) unless the rewrite carries it. Fix: a
   `## The contract` section in the README with the byte table from `notes/interfaces-note.md`,
   the link-script requirement, the endianness rule, the registry rule and the absent-entry rule.
   M. Two-way.
8. The only example does not compile. The macro's doctest at `src/lib.rs` lines 313 to 317 is `ignore`
   and names `abi::`, which no reader has. A `#[used]` static in a named section compiles and links
   on a host, so the example can run with literal ids. `encode_interfaces`, `decode_interfaces`,
   `version_of` and `check_served` have no doctest. Fix: four doctests, the macro one with
   `Declared { id: 1, version: 1 }`. S. Two-way.
9. Three lengths carry no unit. `src/lib.rs` line 37 `NAME_FIELD` (8 bytes), `:40` `HEADER_LEN`
   (12 bytes; `register_names.py` refuses it) and the public `:96` `INTERFACE_RECORD` (12 bytes).
   `interfaces_len` (`:112`) and `interfaces_note_len` (`:269`) return byte counts and read as
   counts of interfaces; `interfaces_len(3)` is 44. The rule is design/naming.md's "a length or a
   limit carries its unit". Fix: the private two now (`NAME_FIELD_BYTES`, `HEADER_BYTES`); the
   public three as decision 2 decides, provisionally. S. Two-way before the first tag.
10. `Declared` names a state, not a thing. `src/lib.rs` line 102. A stranger reads
    `interfaces_note::Declared { id, version }` and asks "declared what?". Design/naming.md asks
    for a noun. Fix: `DeclaredInterface`, as decision 2 decides, provisionally. S. Two-way before
    the first tag.

### P2: for later

11. No fuzz target. The crate has no `unsafe` and a decoder fed untrusted bytes, the shape the
    weekly fuzz opt-in targets, and `fuzz/` has no target for it. Fix: a target that decodes
    arbitrary bytes and, on `Ok`, re-encodes and compares, plus the opt-in in the moved repository.
    S. Two-way.
12. The proof runs one direction. `a_decoded_interface_list_encodes_back_to_its_bytes` proves
    decode-then-encode for a list of two; encode-then-decode is a test on fixed data. Fix: a harness
    over two symbolic ascending ids and two symbolic versions that encodes, decodes and compares,
    with a replayable falsification patch as milestone 1037 (a new Kani harness must arrive
    falsified) requires. S. Two-way.
13. A test name the abbreviation report flags. `src/lib.rs` line 475
    `an_unsorted_list_is_not_encoded`: `unsorted` is not in the word lists. Report only. Fix:
    `a_list_out_of_order_is_not_encoded`. S. Two-way.
14. `Interfaces::iter` yields by value. `src/lib.rs` line 178 returns `Declared`, where std's
    `iter` yields references. The iterator decodes records on the fly, so by value is the honest shape;
    the doc comment should say so in one clause. S. Two-way.
15. The three triaged timeouts travel nowhere. `notes/project-metrics/mutation-triage.csv`
    holds three `+=` to `*=` loop-counter mutants as equivalent hangs. The dry run carries only a
    `.cargo/mutants-baseline.txt` row, and this crate has none. If the template's verdict counts a
    timeout as a miss (unverified), the moved repository's first full mutation run is red for a
    reason already judged. Fix: carry the three rows into `.github/mutants-exempts` at the move,
    with their reason. S. Two-way.

## Decisions for the architect

The rules in force, with their background and prior art, are in
[the appendix](1096-interfaces-note-outside-review/ruled-decisions.md):

- Decision 1, the owner string: ruled 2026-11-14 (UTC). The owner and the section name keep their
  bytes, each line marked `// wire literal`, and prose says "the owner string".
- Decision 2, the public names: the maintainer's provisional ruling of 2026-11-14 (UTC).
  `INTERFACE_RECORD_BYTES`, `descriptor_bytes`, `note_bytes` and `DeclaredInterface`, with no
  alias; sixteen names keep their spelling.
- Decision 3, the crate name: ruled 2026-11-14 (UTC). The crate and repository take
  `interfaces_note`.

## Prior art

The comparison with nearby work is in [the appendix](1096-interfaces-note-outside-review/prior-art.md).

## Follow-on

- **Outstanding.** Every finding marked Open or Ruled under "Finding status", each checked against
  the crate's sources on main.

## Done when

The checks are in [the appendix](1096-interfaces-note-outside-review/done-when.md).

## BUGS

- The `NIFE_INTERFACES_NOTE` static that the macro emits is an identifier and not a wire literal, so
  the marker does not cover it. It needs a rename lane before the crate leaves; the lint counts it
  until then.
- The template's `v7` scaffold itself names nife and the architect in `deny.toml`, `rustfmt.toml`,
  `.githooks/commit-msg`, `docs/stranger-test.md` and `.github/pull_request_template.md`, which
  the dry run counts among its findings. The template's `main` has removed them; the move waits for
  the release that carries the fix or notes the five lines as the template's.
- Item 15 rests on an unverified reading of the template's mutation verdict.
- The review did not replay the two falsification patches under Kani; the dry run applied them with
  `patch --dry-run -F0` only. `script/verify` replays them in the tree.
- A whole-system refusal (a program with a wrong ABI entry sent through the image path) comes
  from `image_request.rs` and its tests, not from a booted system.

## Index row

An outside review of `interfaces_note` before its move. Eight findings are done. The owner string
keeps its bytes behind a marker, and calef ratified the crate name. The maintainer ruled four
renames provisionally, and no lane has built them. The move's own work remains: the manifest, README, citations, prose,
fuzz opt-in and mutation exemptions. Ratifying the public names and review clearance are calef's.
