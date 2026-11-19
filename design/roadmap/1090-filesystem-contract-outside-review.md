---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1090. `filesystem_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/filesystem_contract` as a stranger would meet it, the way
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
crates leave in this order), paired there with `filesystem_fixture`. The manifest has no
dependencies of any kind, and the README says where the fixture's tests went. The two crates share
one workspace repository, ruled 2026-11-14 (UTC); the fixture never publishes.

Reuse: the template `nifeos/crate-workflows` and its scaffold, its helpers (`standalone.py`,
`falsify.py`, `coverage-floor.sh`, the name gates), Kani, cargo-mutants, cargo-llvm-cov, the
classification of equivalent mutants already in `notes/mutation-testing/`. No new tool.

## Finding status

One line per finding, in the review's order: items 1 to 5 below, items 6 to 13 in
[the before-the-tag appendix](1090-filesystem-contract-outside-review/before-the-tag.md), items 14
to 22 in [the later-items appendix](1090-filesystem-contract-outside-review/later-items.md), then
the decisions and the release steps. The item texts use the names the review read; Decision B's
renames are in force (`dir` is `directory`, `xattr` is `extended_attribute`, `blk` is `block`).

- Item 1 (P0), the crate does not stand alone: Open. `nife` still appears in `README.md` lines 5
  and 87, `src/lib.rs` line 33, `src/grant.rs` lines 11 and 13 and `src/extended_attribute.rs` line
  76; the `publish` comment points at a file of this tree, and the manifest has no `repository`
  field. The move commit rewords them and strips the `Name:` blocks.
- Item 2 (P0), clippy refuses `RedoxFS` without a `clippy.toml`: Open. The crate-root
  `clippy.toml` with `doc-valid-idents` comes with the move commit.
- Item 3 (P0), the falsification patches carry this tree's path: Open. All eleven patches diff
  `a/crates/filesystem_contract/...` and the README says `patch -p3`; the move commit rewrites them
  to `a/src/...` and `git apply -p1`.
- Item 4 (P0), `rm.rs` uncovered: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
  `a_status_is_the_errno_and_success_is_zero`.
- Item 5 (P0), names the gates refuse: Ruled (Decision B, maintainer, provisional, 2026-11-14 UTC).
  The renames are in force ([#3726](https://github.com/nifeos/nife/pull/3726)). Not built: a
  `doc(alias)` and source line on each kept POSIX verb and errno name (only `xattr` has an alias),
  and the `register_names` exemption for `block::SIZE` once the template's exemption file exists.
- Item 6 (P1), `reply_errno` not total: Done, [#3577](https://github.com/nifeos/nife/pull/3577).
  Built with Decision C.
- Item 7 (P1), the packers mask: Open. `fs::req`, `rename_dst` and both `specification` packers
  still mask; each proxy call site that forwards a client's length or handle needs a refusal first,
  so a client cannot crash a caretaker, and then the packers assert.
- Item 8 (P1), seven unproven properties: Open. Six harnesses with falsification patches are in
  [#3566](https://github.com/nifeos/nife/pull/3566) and
  [#3637](https://github.com/nifeos/nife/pull/3637); the termination of the four walkers is still
  unproven.
- Item 9 (P1), equivalent mutants need a home: Open. The classes go into the new repository's
  `.github/mutants-exempts`, with one green full run before the tag.
- Item 10 (P1), the README is short of the scaffold: Open. No badge row, `patch -p3`, no Kani cost;
  the move commit brings the scaffold's shape.
- Item 11 (P1), twelve bare acronyms: Open. The crate's own text expands them
  ([#3566](https://github.com/nifeos/nife/pull/3566)); the new repository's
  `prose_acronyms.toml` carries the universally known ones.
- Item 12 (P1), `rm` is not filesystem protocol: Open. `src/rm.rs` is still in the crate, and
  `CLIENT_WINDOWS` is still documented as provisional with a citation of `abi::rendezvous::BADGE`;
  where `rm` lands (the program or `byte_sink_contract`) and whether the window count is a contract
  fact are placement calls the maintainer makes.
- Item 13 (P1), dates and British spellings: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 14 (P2), `req_blocks` decodes 256: Open. `block.rs` still returns `(w0 & 0xff) + 1`; clamp
  or `Option` is a two-way door the maintainer rules provisionally.
- Item 15 (P2), `directory_entry::encode` accepts an empty name: Open. Unchanged; refusing in the
  encoder or documenting it is a two-way door the maintainer rules provisionally.
- Item 16 (P2), three spellings of the errno vocabulary: Open. No `errno` module yet; its name is a
  provisional maintainer call, since Decision B's table does not list it.
- Item 17 (P2), the length-or-offset doc: Done, [#3566](https://github.com/nifeos/nife/pull/3566);
  the name is `MAXIMUM_LENGTH_BYTES` ([#3726](https://github.com/nifeos/nife/pull/3726)).
- Item 18 (P2), stale distinctness tests: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 19 (P2), `usize` width: Done, [#3566](https://github.com/nifeos/nife/pull/3566). A `const`
  assertion that `usize::BITS >= 40`.
- Item 20 (P2), `in_opcode_order` warns at 1.88: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 21 (P2), no fuzz target: Open. A `fuzz/` crate with one target per walker and one for the
  store, a lane of its own.
- Item 22 (P2), `extended_attribute::reply` masks a kind: Open. It asserts once item 7's call sites
  refuse.
- Decision A, the store directory: Done, [#3577](https://github.com/nifeos/nife/pull/3577). Ruled
  2026-11-14 UTC.
- Decision B, the names and units: Done, [#3726](https://github.com/nifeos/nife/pull/3726).
  Maintainer ruling, provisional, 2026-11-14 UTC; the aliases are item 5.
- Decision C, the reply error range: Done, [#3577](https://github.com/nifeos/nife/pull/3577). Ruled
  2026-11-14 UTC.
- §99 still names the store `.nife-attrs`: Open. Under Open decisions.
- Review package: Open. `notes/crate-reviews/` has no package for this crate; one package may cover
  both crates of the shared repository.
- Review clearance of the shared repository: Open. Under Open decisions.

## Open decisions

### Correct the store directory's name in §99

Background. §99 decides where macOS metadata (resource forks, Finder information) lands on a nife
filesystem: in a sidecar store directory beside the files. Decision A above renamed that directory
`.extended-attributes` (2026-11-14 UTC), and the crate, the server, the host tool and the notes
carry the new name. §99's own text still says `.nife-attrs`, in its bytes-at-rest row (line 160)
and its closing paragraph (line 202). A reader of the decision record now meets a name that no
code uses. A decision's words change only with the architect's approval, so no lane may make the
edit on its own.

Recommendation: A.

- A. Approve the edit: §99's two mentions become `.extended-attributes`, stated as the rule in
  force, with no other change to the section.
- B. Leave §99 as written and add a line in Decision A that it supersedes §99's name.
- C. Supersede §99 with a new section that carries the renamed store.

Door: two-way. It is a record's wording, and no image outside the maintainers' own ever carried
the old name. It comes to the architect only because §88 reserves a changed word in a decision to
him.

Prior art: §298 states that a document says what is in force now, with git as the history, which
is option A. Python's PEPs and Rust's RFCs keep their original text and point to the successor,
which is option B; this tree chose the other way. Concrete example: a reader of §99's cost table
counts "one 4096-byte block for the node's blob under `.nife-attrs`" and then finds
`STORE_DIRECTORY` holding `.extended-attributes`.

Blocks: nothing in the move or the tag; it is the record's accuracy.

### Clear the review of `filesystem_contract` and `filesystem_fixture`, and create their repository

Background. `filesystem_contract` defines the two message protocols a filesystem server and a
block server speak, and `filesystem_fixture` is the test image those servers are checked against.
They leave together for one workspace repository, ruled 2026-11-14 UTC; the fixture never
publishes. A crate leaves once its review package lists every public name and decision and the
architect clears that review, and the clearance is also the go to create the repository. Most of
the contract's public names are provisional, Decision B's renames included, and the fixture has
244 unratified names (milestone 1091). The decision is one clearance for both crates.

Recommendation: A, once the review package exists.

- A. Clear one review covering both crates when its package is written: ratify the names as the
  package lists them, and create the repository as `publish = false`. The move follows, with items
  1 to 3 and 10 here and items 6 and 11 of milestone 1091.
- B. Create the repository and leave the names provisional until the release review.
- C. Clear the contract alone and keep the fixture in this repository, against the 2026-11-14 ruling
  that they share one.

Door: one-way. The repository is an outward step, and a ratified protocol name binds every server
and client once anything outside this tree speaks it.

Prior art: `jh7110_clock_and_reset` and `designware_ethernet` were cleared from review packages
before their repositories were made. Outside, `rust-lang/futures-rs` and `tokio` keep a test
utility crate (`futures-test`, `tokio-test`) in the same workspace as the library it tests, which is
the shape ruled here. Concrete example: ratifying `nameset::IS_DIRECTORY` fixes the spelling
in the filesystem servers and in the fixture's witness documentation at once.

Blocks: the move.

## Goal

A stranger who finds `filesystem_contract` in its own repository can tell in one screen what the two
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

1. The crate does not stand alone.
   Finding: 21 externalization-lint hits, every one in prose. `nife` appears in `README.md` line 5 and `:84`,
   `src/lib.rs` line 29, `src/grant.rs` line 11 and `:13`, and `src/xattr.rs` line 76 and `:77`.
   Programs appear by name too. They are `disk_surveyor`, `disk_partitioner` and `mkfs`
   (`src/blk.rs` line 47, `:61`). They are `fs_nameset_caretaker`, `fs_subtree_caretaker`,
   `fs_file_caretaker` and `redoxfs_server` (`src/verb.rs` line 14, `:64`, `:119`, `:249`, `:333`;
   `src/xattr.rs` line 61, `:62`; `src/verb/tests.rs` line 133, `:174`). The last is
   `least_authority_demo` (`src/grant/tests.rs` line 40, `:44`, `:51`). Beyond the lint: `rm.rs`
   describes the `rm` program and cites `byte_sink_contract::eof()`; `fs.rs` line 298 cites
   `abi::rendezvous::BADGE`; `fs.rs` line 274 cites `bench/transfer-size-sweep.sh`; `fs.rs` line 232 cites
   `subtree_scope::Bindings::bind`; `nameset.rs` line 7 cites `grant_plan::Refusal::TooManyNames`;
   `xattr.rs` line 73 cites `redoxfs_host extract`. `standalone.py` adds: `publish = false`
   (`Cargo.toml` line 11, with a comment pointing at a nife file), 31 provenance blocks, 7 milestone
   citations, 4 lines naming the architect, 1 section citation. Of the provenance blocks,
   `src/fs.rs` line 255 quotes a ruling verbatim, against the standing rule that a record states the
   decision and never quotes the decider. The manifest has no `repository`, `documentation` or
   `homepage` field.
   Why it matters: the move rule is that nothing that leaves references back into nife; the
   template's `check / standalone` is a required check and reads every one of these.
   Change: reword each hit for any reader (a caretaker becomes "a proxy that narrows a directory
   capability", a program name becomes its role). The move commit strips the `Name:` blocks, as
   ruled, and the quotation goes with them. Replace the `publish` comment with one sentence a
   stranger can act on. Add the three URL fields. Delete the `rm` module (item 5) or reword it.
   Door: two-way, except `STORE_DIR` (Decision A). Effort: M.

2. `check / clippy` is red on the template's toolchain.
   Finding: stable clippy with `-D warnings` fails on `doc_markdown` for `RedoxFS` at
   `src/lib.rs` line 21, `:73`, `:108`, `src/blk.rs` line 41, `:57`, `src/grant.rs` line 11 and two more sites
   (8 in the library, 9 with the test build). nife's workspace `clippy.toml` carries
   `doc-valid-idents` with `RedoxFS`; the scaffold has no `clippy.toml`.
   Why it matters: the new repository's first pull request is red before anything else is judged.
   Change: a `clippy.toml` at the crate root with `doc-valid-idents = ["RedoxFS", ..]`, carrying
   nife's reason (backticks would render a proper noun as code). Backticking is the alternative
   and the worse one.
   Door: two-way. Effort: XS.

3. The falsification patches carry nife's path, and the README's command with them.
   Finding: all three patches diff `a/crates/filesystem_contract/src/dir.rs`
   (`falsifications/*.patch`, the `diff --git` lines). The template's `proofs / falsify` applies
   each patch crate-relative with `git apply -p1`, so after the move none applies and the job is
   red. The doc comments at `src/dir.rs` line 227, `:248` and `:266` name the same path, and
   `README.md` line 74 says `patch -p3`.
   Change: rewrite the three headers to `a/src/dir.rs`, the three doc comments to
   `falsifications/<name>.patch`, and the README to the scaffold's `git apply -p1` form with one
   real example and what the failing run prints. The move commit is the right place, since the
   paths change with it.
   Door: two-way. Effort: XS.

4. `check / coverage` is red on `src/rm.rs`.
   Finding: `rm::status` is never called inside the crate; the standalone lcov reports 0 of 3
   lines, under the per-file floor of 85 that `coverage-floor.sh` applies to every file with an
   executable line. The function's callers are the `rm` program and `filesystem_fixture`'s tests.
   Change: resolved by item 5 if the module leaves; otherwise one test that `status(errno)` is the
   errno and `OK` is 0.
   Door: two-way. Effort: XS.

5. The name gates refuse 22 names, and renames are the architect's.
   Finding: `spelled_out` (ceiling 0) refuses module `dir` (`src/dir.rs`). It refuses `IS_DIR` twice
   (`src/dirent.rs` line 4, `src/nameset.rs` line 33) and `STORE_DIR` (`src/xattr.rs` line 77). It
   refuses `grant::spec`, `spec_len` and `spec_rights` (`src/grant.rs` line 116, `:121`, `:126`). It
   refuses `xattr::spec`, `spec_kind` and `spec_value_len` (`src/xattr.rs` line 82, `:86`, `:90`).
   It also refuses seven `is_dir` and `spec` bindings in tests. `register_names` (ceiling 0): `blk::SIZE` (`src/blk.rs` line 27, an opcode, so a false
   positive), `BLOCK_SIZE` (`src/blk.rs` line 43), `MAXIMUM_LENGTH_OR_OFFSET` (`src/fs.rs` line 309,
   ratified), `statfs::LEN` (`src/statfs.rs` line 3). `question_shape`: `in_opcode_order`
   (`src/verb.rs` line 319). The naming worklist inside nife lists the same ten wire-or-format names.
   `GETMTIME`, `SETMTIME` and `SETMTIME_AT` (`src/fs.rs` line 193, `:204`, `:214`) contain `mtime`,
   which the spelled-out list expands, and the gate does not split `GETMTIME` into components, so
   it misses them; the rule still applies. No item in the crate carries a `#[doc(alias)]`, which
   the rule asks of every kept foreign spelling (the POSIX verb and errno names) and every
   spelled-out name a reader searches for by its abbreviation.
   Why it matters: `check / standalone` runs these at a ceiling of 0; a ratified name failing a
   check needs an exemption outside the `Name:` block, and an unratified one needs a ruling.
   Change: the names Decision B decides, provisionally; one rename lane applies them at the end,
   then adds the exemption for `blk::SIZE` with its reason and the aliases.
   Door: two-way before the first tag. Effort: S for the lane, after the ruling.

### P2: later

The P2 items are in [the appendix](1090-filesystem-contract-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1090-filesystem-contract-outside-review/before-the-tag.md).

## Decisions for the architect

Only one-way doors. Each with the background first, then the recommendation.

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

Background. The template runs `spelled_out`, `register_names` and `question_shape` at a ceiling
of 0 on every pull request. Seventeen names fail the first (the module `dir`, `IS_DIR`,
`STORE_DIR`, six `spec` functions, bindings), four the second, one the third. Separately, the
rule that a length or a limit carries its unit postdates the ratification of `MAXIMUM_NAME`
(twice), `MAXIMUM_VALUE`, `TRANSFER_MAXIMUM` and `MAXIMUM_LENGTH_OR_OFFSET`, and the rule that a
count names the things reaches `MAXIMUM_COUNT`. A release review rechecks such names. The verb
names (`OPENDIR`, `READDIR`, `MKDIR`, `RMDIR`, `FSTAT`, `GETXATTR` and the rest) and the errno
names are POSIX terms of art, which the rules keep with an alias and an opening line naming the
source. The module names `blk`, `fs`, `dirent`, `xattr`, `statfs` are abbreviations whose
expansions are spoken, except `fs`, which mirrors `std::fs`, and `statfs`, which is the call's
name.

Recommendation, one table, to rule as a set and apply in one lane after the move:

| today | proposed | rule |
|---|---|---|
| module `dir` | `directory` | spelled out (ruled word) |
| `dirent::IS_DIR`, `nameset::IS_DIR` | `IS_DIRECTORY` | spelled out |
| `xattr::STORE_DIR` | `STORE_DIRECTORY` | spelled out (Decision A rules the value) |
| `grant::spec`, `spec_len`, `spec_rights` | `specification`, `specification_len`, `specification_rights` | spelled out (ruled word) |
| `xattr::spec`, `spec_kind`, `spec_value_len` | `specification`, `specification_kind`, `specification_value_len` | spelled out |
| module `blk` | `block` | an expansion everyone says |
| module `dirent` | `directory_entry` | the same |
| module `xattr` | `extended_attribute` | the same; `#[doc(alias = "xattr")]` |
| module `fs` | keep, `#[doc(alias = "filesystem")]` | mirrors `std::fs` |
| module `statfs` | keep, alias `volume_statistics` | a POSIX call name |
| `GETMTIME`, `SETMTIME`, `SETMTIME_AT` | `GET_MODIFICATION_TIME`, `SET_MODIFICATION_TIME`, `SET_MODIFICATION_TIME_AT` | `mtime` is a ruled word; provisional today |
| `blk::BLOCK_SIZE` | `BLOCK_BYTES` | a size carries its unit |
| `statfs::LEN` | `RECORD_BYTES` | the same |
| `store::RECORD_HEADER` | `RECORD_HEADER_BYTES` | the same |
| `nameset::BYTES` | `MAXIMUM_BYTES` | a limit opens `MAXIMUM_` |
| `grant::WORDS_NAME` | `MAXIMUM_WORDS_NAME_BYTES` | a limit with its unit |
| `PAGE` | `PAGE_BYTES` | the page-contract spelling in `design/naming.md` |
| `xattr::MAXIMUM_NAME`, `grant::MAXIMUM_NAME` | `MAXIMUM_NAME_BYTES` | ratified before the units rule; reopened at release |
| `xattr::MAXIMUM_VALUE` | `MAXIMUM_VALUE_BYTES` | the same |
| `fs::TRANSFER_MAXIMUM`, `blk::TRANSFER_MAXIMUM` | `MAXIMUM_TRANSFER_BYTES` | the same |
| `fs::MAXIMUM_LENGTH_OR_OFFSET` | `MAXIMUM_LENGTH_BYTES` | the same, and the field carries no offset (item 17) |
| `xattr::MAXIMUM_COUNT` | `MAXIMUM_ATTRIBUTES` | a count names the things |
| `blk::SIZE` | keep, exempt from `register_names` | an opcode, not a size |
| `verb::in_opcode_order` | `is_in_opcode_order` | question shape |
| the POSIX verb and errno names | keep, each with `#[doc(alias)]` and a source line | a specification's term of art |

Prior art. Fuchsia spells `GetAttributes`, `UpdateAttributes` and `rights` in full in FIDL and
keeps POSIX names only at its POSIX compatibility layer. Capsicum keeps the Unix spellings
(`CAP_FUTIMES`) because they name Unix calls, which is the case the verb names are in. The `libc`
crate keeps every errno as the platform spells it, which is the case the errno names are in.

Concrete example. A caretaker that reads `verb::of(operation).takes_name()` today would read
`verb::of(operation).takes_name()` still; one that compares `grant::spec_rights(w)` reads
`grant::specification_rights(w)`; a client that sends `fs::req(fs::GETMTIME, ..)` sends
`fs::req(fs::GET_MODIFICATION_TIME, ..)`, and the number 20 does not change.

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

## Index row

An outside review of `filesystem_contract` before it leaves nife. Five items are red at the moving
commit (the stand-alone text, clippy's proper-noun list, the falsification patch paths, one
uncovered file, 22 names the gates refuse). Eight come before the first tag (a non-total errno
decoder, packers that mask, seven unproven properties, the equivalent-mutant list, the README and
prose gates, the `rm` module). Nine are for later. Three decisions: the on-disk name (A, ruled),
the naming pass with units (B, the maintainer's provisional ruling, built), and a bound on the
error range of a reply word (C, ruled). Two are open: §99's stale store name, and the clearance of
the repository this crate shares with `filesystem_fixture`.
