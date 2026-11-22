---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1090. `file_system_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/file_system_contract` as a stranger would meet it, the way
milestone 1048 (`generational_table`: what an outside maintainer would fix before 0.1.0) reviewed
that crate. It changes nothing; it lists what the move lane and the lanes after it change.

There is no review package for this crate in `notes/crate-reviews/`. The baseline this review
takes as settled is therefore the rulings recorded at the names themselves and in the roadmap.
They cover the crate name and the `_contract` suffix. They cover `fs::VERSION`, `blk::VERSION` and
the two `INTERFACE_ID`s. They cover every opcode number, `ROOT` as 0 and the 8-bit opcode field at
bits 63:56. Pull request #2797 ratified the limits `TRANSFER_MAXIMUM`, `MAXIMUM_LENGTH_OR_OFFSET`,
`MAXIMUM_HANDLE`, `grant::MAXIMUM_NAME`, `xattr::MAXIMUM_NAME`, `MAXIMUM_VALUE`, `MAXIMUM_COUNT`,
`MAXIMUM_KIND`, `list::MAXIMUM_BYTES`, `store::MAXIMUM_BYTES` and `nameset::MAXIMUM_NAMES`. The
rulings also cover the errno choices (`EROFS` over `EACCES`, `ENOTDIR` for a directory verb through
a file grant, `ENOENT` for a naming right), the `NARROW` verb, the `PARENT_RIGHTS` sentinel and
`BIND_SYNC`. This review reopens none of those. Where a rule written after a ratification contradicts a ratified name, the item says so and
leaves the reopening to the architect, under the ruling that a release review rechecks names
ratified before a later rule.

The crate is on `crates/externalization-list.txt` and is step 2 of milestone 1017 (the contract
crates leave in this order), paired there with `file_system_fixture`. The manifest has no
dependencies of any kind, and the README says where the fixture's tests went. The two crates share
one workspace repository, ruled 2026-11-14 (UTC); the fixture never publishes.

Reuse: the template `nifeos/crate-workflows` and its scaffold, its helpers (`standalone.py`,
`falsify.py`, `coverage-floor.sh`, the name gates), Kani, cargo-mutants, cargo-llvm-cov, the
classification of equivalent mutants already in `notes/mutation-testing/`. No new tool.

## Finding status

One line per finding, in the review's order: items 1 to 5 in [the blocks-the-move
appendix](1090-filesystem-contract-outside-review/blocks-the-move.md), items 6 to 13 in [the
before-the-tag appendix](1090-filesystem-contract-outside-review/before-the-tag.md), items 14 to 22
in [the later-items appendix](1090-filesystem-contract-outside-review/later-items.md), then the
decisions and the review package. The item texts use the names the review read; the crate's names
now are `file_system_contract`, `directory`, `extended_attribute` and `block`.

- Item 1 (P0), the crate does not stand alone: At the move. The prose and the manifest stand
  alone ([#3911](https://github.com/nifeos/nife/pull/3911), [#3999](https://github.com/nifeos/nife/pull/3999): the text names no program of this tree, and the externalization lint
  carries no waiver for the crate); the move commit strips the `Name:` blocks and the milestone
  citations inside them.
- Item 2 (P0), clippy refuses `RedoxFS` without a `clippy.toml`: Done, [#3911](https://github.com/nifeos/nife/pull/3911). The crate root
  carries `clippy.toml` with `doc-valid-idents`.
- Item 3 (P0), the falsification patches carry this tree's path: At the move. The eleven patches
  and the harness doc comments still name `crates/file_system_contract/`; the move commit rewrites
  them to the repository's own paths.
- Item 4 (P0), `rm.rs` uncovered: Done, [#3566](https://github.com/nifeos/nife/pull/3566). The test now lives with the module in
  `grant_plan::rm_report` ([#3999](https://github.com/nifeos/nife/pull/3999)).
- Item 5 (P0), names the gates refuse: Done, [#3726](https://github.com/nifeos/nife/pull/3726) and [#3999](https://github.com/nifeos/nife/pull/3999). Decision B's renames are in
  force with their aliases, `GET_SIZE_BYTES` carries its unit, and the last two refusals of the
  repository's naming gates (`spec`, `PAGE_NAME_SET_OFFSET`) now have new names.
- Item 6 (P1), `reply_errno` not total: Done, [#3577](https://github.com/nifeos/nife/pull/3577). Now `error::word` and `error::code`.
- Item 7 (P1), the packers mask: Open. `file_system::request`, `rename_destination`,
  `grant::specification` and `extended_attribute::specification` still mask. About 190 call sites pack with them, the attack fixtures among them. Before the packers can assert, each proxy that forwards a client's length or handle needs a refusal, and each attack fixture needs a raw packer.
- Item 8 (P1), seven unproven properties: Open. Six harnesses with falsification patches are in
  [#3566](https://github.com/nifeos/nife/pull/3566) and [#3637](https://github.com/nifeos/nife/pull/3637); the termination of the four walkers is still unproven.
- Item 9 (P1), equivalent mutants need a home: At the move. The classes go into the new
  repository's `.github/mutants-exempts`, with one green full run before the tag.
- Item 10 (P1), the README is short of the scaffold: Done, [#3911](https://github.com/nifeos/nife/pull/3911) and [#3999](https://github.com/nifeos/nife/pull/3999). The badge row,
  `patch -p1` and the single-harness command came first; the measured Kani time and peak memory
  follow.
- Item 11 (P1), twelve bare acronyms: Done, [#3566](https://github.com/nifeos/nife/pull/3566) and [#3911](https://github.com/nifeos/nife/pull/3911). The template's
  `prose_acronyms` gate passes on the crate's text.
- Item 12 (P1), `rm` is not filesystem protocol: Done, [#3999](https://github.com/nifeos/nife/pull/3999). `rm`'s report protocol is
  `grant_plan::rm_report`, beside `rm_options`, and the text states `CLIENT_WINDOWS` as a contract fact: a
  server refuses a badge at or past it.
- Item 13 (P1), dates and British spellings: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 14 (P2), `request_block_count` decodes 256: Done, [#3999](https://github.com/nifeos/nife/pull/3999). It answers `None` past
  `TRANSFER_BLOCKS`, and the three block servers refuse with `EINVAL` (provisional, restrictive
  first).
- Item 15 (P2), `directory_entry::encode` accepts an empty name: Done, [#3999](https://github.com/nifeos/nife/pull/3999). The encoder
  refuses one; the reader still reads one, so the refusal can relax later.
- Item 16 (P2), three spellings of the errno vocabulary: Waiting on the build of §326 (the names and numbers of the services'
  wire error codes), which replaces the errno numbers this item would gather into one module.
- Item 17 (P2), the length-or-offset doc: Done, [#3566](https://github.com/nifeos/nife/pull/3566); the name is `MAXIMUM_LENGTH_BYTES`
  ([#3726](https://github.com/nifeos/nife/pull/3726)).
- Item 18 (P2), stale distinctness tests: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 19 (P2), `usize` width: Done, [#3566](https://github.com/nifeos/nife/pull/3566). A `const` assertion that `usize::BITS >= 40`.
- Item 20 (P2), `in_opcode_order` warns at 1.88: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 21 (P2), no fuzz target: Waiting on the crate's own repository (milestone 1017 step 2). The
  `fuzz/` crate and the template's crash-input opt-in live there, and the standing approval of the fuzzing dependency covers only a crate repository's unpublished fuzz crate.
- Item 22 (P2), `extended_attribute::reply` masks a kind: Done, [#3999](https://github.com/nifeos/nife/pull/3999). The packer panics on a
  kind past `MAXIMUM_KIND`, and the store's walk stops before a record that carries one.
- Decision A, the store directory: Done, [#3577](https://github.com/nifeos/nife/pull/3577). Ruled 2026-11-14 UTC.
- Decision B, the names and units: Done, [#3726](https://github.com/nifeos/nife/pull/3726). Maintainer ruling, provisional, 2026-11-14
  UTC; the aliases are item 5.
- Decision C, the reply error range: Done, [#3577](https://github.com/nifeos/nife/pull/3577). Ruled 2026-11-14 UTC.
- Decision D, §99's store name: Done, [#3789](https://github.com/nifeos/nife/pull/3789). Ruled 2026-11-19 UTC; §99 names
  `.extended-attributes`.
- Decision E, the review package for the shared repository: Done, [#3889](https://github.com/nifeos/nife/pull/3889). [The contract's
  package](../../notes/crate-reviews/filesystem-contract.md) and [the
  fixture's](../../notes/crate-reviews/filesystem-fixture.md) form the one package.

## Open decisions

None. The review package carries the name ratification and the clearance of the review, which stay
the architect's.

## Goal

A stranger who finds `file_system_contract` in its own repository can tell in one screen what the two
protocols are. The stranger can build, test, prove and falsify it from the README alone on the
stated minimum Rust version. Every required check is green for the crate's own reasons. Nothing in
the crate names nife, a nife program or a nife package; the one on-disk name that does is ruled on
before the move.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1090-filesystem-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1090-filesystem-contract-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the release or when it is a layout on a disk or a
wire; a public name is one-way at release and the architect's call before it. Everything else is
two-way and the maintainer decides it.

### P0: red at the moving commit, or refused by the move rule

The P0 items are in [the appendix](1090-filesystem-contract-outside-review/blocks-the-move.md).

### P2: later

The P2 items are in [the appendix](1090-filesystem-contract-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1090-filesystem-contract-outside-review/before-the-tag.md).

## Decisions for the architect

The architect ruled A, C and D; B and E are the maintainer's provisional rulings on two-way doors. Each states the rule
in force.

### A. The reserved on-disk name: ruled 2026-11-14 (UTC)

In force: the extended-attribute store lives in a directory in the image root named
`.extended-attributes` (`xattr::STORE_DIR`, `src/xattr.rs` line 77). Decision B rules the
constant's own name. Every name-taking verb refuses that name with `EINVAL`, in every directory and at
every depth of a path, and `READDIR` skips it, so it is both a disk layout and a wire rule. The name
says what the directory holds and names no project. `redoxfs_host extract` and upstream's FUSE
mount carry it out as a directory. A recovery listing of an image shows
`.extended-attributes/0000002a`. Nife has no installs, so no image outside the maintainers' own
carried the earlier name.

Prior art. Samba's `streams_depot` keeps alternate streams under a `.streams` directory named for
the mechanism, not the vendor. macOS writes AppleDouble `._name` files and `.DS_Store`, both named
for the format or the feature. ext4 and NTFS keep attributes inside the inode or the MFT record
and reserve no name. Where this crate differs: it reserves one name at every depth, which is the
cost of a layer above an unchanged RedoxFS format.

Concrete example. A client gets `EINVAL` for `CREATE ".extended-attributes"` in the root and in any
subdirectory, `OPEN "sub/.extended-attributes"` is `EINVAL`, and `READDIR` of a root that holds the
store does not list it.

### B. The names the gates refuse, and the units on the ratified limits

The background, the table, the prior art and an example are in
[the appendix](1090-filesystem-contract-outside-review/decision-b-names.md).

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The table holds as written, applied by one lane, and
`STORE_DIRECTORY` holds `.extended-attributes`, the value Decision A rules. The POSIX verb and
errno names keep their spelling, each with `#[doc(alias)]` and a line naming the source. No alias of the old spellings.

### C. The error range of a reply word: ruled 2026-11-14 (UTC)

In force: a reply's first word is an `i64`. A word in `-4095..=-1` is the error code `1..=4095`
(`MAXIMUM_ERRNO`, Linux's bound). Any other negative word is a protocol violation, and
`reply_errno` reads it as `EPROTO` (71), never as success and never as a truncated code. A
non-negative word is a result. `reply_err` asserts its code is in `1..=MAXIMUM_ERRNO` in every
build: a bad code is a bug in the sender, and a release build that skipped the check would put a
success-looking or protocol-violating word on the wire. Two Kani harnesses prove the decoding for
every `i64`, and unit tests pin -1, -4095, -4096 and `i64::MIN`. Every reply any current server
sends keeps its word. Loosening the bound later is compatible; tightening is not.

Prior art. Linux `include/linux/err.h` (`MAX_ERRNO`, `IS_ERR_VALUE`); FreeBSD returns errno in a
separate register with the carry flag, which this two-word protocol does not have; 9P2000.L
carries the errno as its own field.

Concrete example. `reply_errno(-5_000_000_000)` is `Some(71)`, where a truncating cast gave
`Some(705_032_704)`, and `reply_errno(-2)` is `Some(2)`.

### D. The store directory's name in §99: ruled 2026-11-19 (UTC)

In force: §99 (where Apple's metadata lands: stream or sidecar) names the sidecar store
`.extended-attributes`, in its bytes-at-rest row and its closing paragraph, the name Decision A
rules. The section says what is in force now and changes nothing else.

Prior art: §298 (a document states what is in force now) keeps git as the history. Python's PEPs and
Rust's RFCs keep their original text and point to a successor; this tree chose the other way.

Concrete example: a reader of §99's cost table counts "one 4096-byte block for the node's blob under
`.extended-attributes`" and then finds `STORE_DIRECTORY` holding the same name.

### E. The review package and clearance of `file_system_contract` and `file_system_fixture`: maintainer ruling, provisional, 2026-11-19 (UTC)

In force: a lane writes one review package that covers both crates, which share one workspace
repository (ruled 2026-11-14 UTC; the fixture never publishes), once the other items here land. The
move is not held for it. The package step is the maintainer's provisional ruling on a two-way door.
Ratifying the names and clearing the review, which is also the go to create the repository as
`publish = false`, stay the architect's, and the package asks for them. The move follows, with
items 1 to 3 and 10 here and items 6 and 11 of milestone 1091 (`file_system_fixture`: what an outside
maintainer would fix before it leaves).

Prior art: `jh7110_clock_and_reset` and `designware_ethernet` each had a review package cleared
before the organization made their repositories. Outside, `rust-lang/futures-rs` and `tokio` keep a
test utility crate (`futures-test`, `tokio-test`) in the same workspace as the library it tests,
which is the shape ruled here.

Concrete example: ratifying `nameset::IS_DIRECTORY` fixes the spelling in the filesystem servers and
in the fixture's witness documentation at once, which is why the package lists both crates.

## Done when

The checks are in [the appendix](1090-filesystem-contract-outside-review/done-when.md).

## BUGS

- Item 19 came from reading, not from a 32-bit build; the crate names no 32-bit target.
- Item 9 relies on the classification in `notes/mutation-testing/`, which predates the file split
  of milestone 845 (the filesystem-protocol crate root is split along its seams). It names some
  tests by their old module. The groups still hold, and the line numbers do not.
- The review did not run the template's workflows end to end. The gate results come from the
  template's helpers, which the reviewer ran by hand on a standalone copy with the same arguments
  the workflows pass. The exception is `coverage`, where the reviewer applied the floor to a locally
  produced lcov.

## Follow-on

- **Outstanding.** Every finding that Finding status above marks Open or Ruled, each with what
  remains, and each open decision under Open decisions. A lane checked each finding line against
  the crate and its pull requests on main.

## Index row

An outside review of `file_system_contract` before it leaves nife. Five items are red at the moving
commit (the stand-alone text, clippy's proper-noun list, the falsification patch paths, one
uncovered file, 22 names the gates refuse). Eight come before the first tag (a non-total errno
decoder, packers that mask, seven unproven properties, the equivalent-mutant list, the README and
prose gates, the `rm` module). Nine are for later. Five decisions, none open. The architect ruled the on-disk name (A), the bound on the error range of
a reply word (C) and §99's store name (D). The maintainer rules provisionally on the naming pass with
units (B, built) and on the review package for the shared repository (E).
