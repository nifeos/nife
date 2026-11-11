---
status: NOT-STARTED
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
takes as settled is therefore the rulings recorded at the names themselves and in the roadmap: the
crate name and the `_contract` suffix; `fs::VERSION`, `blk::VERSION` and the two `INTERFACE_ID`s;
every opcode number, `ROOT` as 0 and the 8-bit opcode field at bits 63:56; the limits ratified in
pull request #2797 (`TRANSFER_MAXIMUM`, `MAXIMUM_LENGTH_OR_OFFSET`, `MAXIMUM_HANDLE`,
`grant::MAXIMUM_NAME`, `xattr::MAXIMUM_NAME`, `MAXIMUM_VALUE`, `MAXIMUM_COUNT`, `MAXIMUM_KIND`,
`list::MAXIMUM_BYTES`, `store::MAXIMUM_BYTES`, `nameset::MAXIMUM_NAMES`); the errno choices
(`EROFS` over `EACCES`, `ENOTDIR` for a directory verb through a file grant, `ENOENT` for a naming
right); the `NARROW` verb and the `PARENT_RIGHTS` sentinel; `BIND_SYNC`. None of those is reopened
here. Where a rule written after a ratification contradicts a ratified name, the item says so and
leaves the reopening to the architect, under the ruling that a release review rechecks names
ratified before a later rule.

The crate is on `crates/externalization-list.txt` and is step 2 of milestone 1017 (the contract
crates leave in this order), paired there with `filesystem_fixture` because of a dev-dependency.
That dev-dependency no longer exists: the manifest has no dependencies of any kind, and the README
says where the fixture's tests went. The two crates can leave separately.

Reuse: the template `nifeos/crate-workflows` and its scaffold, its helpers (`standalone.py`,
`falsify.py`, `coverage-floor.sh`, the name gates), Kani, cargo-mutants, cargo-llvm-cov, the
classification of equivalent mutants already in `notes/mutation-testing/`. No new tool.

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
   Finding: 21 externalization-lint hits, every one in prose. `nife` in `README.md` line 5 and `:84`,
   `src/lib.rs` line 29, `src/grant.rs` line 11 and `:13`, `src/xattr.rs` line 76 and `:77`; the programs
   `disk_surveyor`, `disk_partitioner`, `mkfs` (`src/blk.rs` line 47, `:61`), `fs_nameset_caretaker`,
   `fs_subtree_caretaker`, `fs_file_caretaker`, `redoxfs_server` (`src/verb.rs` line 14, `:64`, `:119`,
   `:249`, `:333`; `src/xattr.rs` line 61, `:62`; `src/verb/tests.rs` line 133, `:174`) and
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
   Finding: `spelled_out` (ceiling 0): module `dir` (`src/dir.rs`), `IS_DIR` twice
   (`src/dirent.rs` line 4, `src/nameset.rs` line 33), `STORE_DIR` (`src/xattr.rs` line 77), `grant::spec`,
   `spec_len`, `spec_rights` (`src/grant.rs` line 116, `:121`, `:126`), `xattr::spec`, `spec_kind`,
   `spec_value_len` (`src/xattr.rs` line 82, `:86`, `:90`), and seven `is_dir` and `spec` bindings in
   tests. `register_names` (ceiling 0): `blk::SIZE` (`src/blk.rs` line 27, an opcode, so a false
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
   Change: Decision B rules the names; one rename lane applies them at the end, then adds the
   exemption for `blk::SIZE` with its reason and the aliases.
   Door: one-way at release; the architect's before it. Effort: S for the lane, after the ruling.

### P2: later

The P2 items are in [the appendix](1090-filesystem-contract-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1090-filesystem-contract-outside-review/before-the-tag.md).

## Decisions for the architect

Only one-way doors. Each with the background first, then the recommendation.

### A. The reserved on-disk name `.nife-attrs`

Background. The extended-attribute store lives in a directory in the image root named
`xattr::STORE_DIR = ".nife-attrs"` (`src/xattr.rs` line 77). Every name-taking verb refuses that name
with `EINVAL` and `READDIR` skips it, so it is both a disk layout and a wire rule. It contains the
word `nife`, so the externalization lint refuses it, and `typos` reads it as a misspelling of
`knife`. Changing it changes what a recovery tool looks for and what an existing image holds. Nife
has no installs, so no image outside the maintainers' own exists today (the ruling that text may
stay public while nife has no users rests on the same fact).

Recommendation: rename it now, before the move, to a name that says what it holds and names no
project: `.extended-attributes`. The `redoxfs_host` extractor and upstream's FUSE mount carry it
out as a directory either way. Add a `_typos.toml` entry only if the chosen name needs one.

Prior art. Samba's `streams_depot` keeps alternate streams under a `.streams` directory named for
the mechanism, not the vendor. macOS writes AppleDouble `._name` files and `.DS_Store`, both named
for the format or the feature. ext4 and NTFS keep attributes inside the inode or the MFT record
and reserve no name. Where this crate differs: it reserves one name at every depth, which is the
cost of a layer above an unchanged RedoxFS format.

Concrete example. A client that today gets `EINVAL` for `CREATE ".nife-attrs"` gets `EINVAL` for
`CREATE ".extended-attributes"` instead, and `READDIR` of a root that holds the store returns one
fewer entry under the new name. A recovery listing of an image shows `.extended-attributes/0000002a`
where it showed `.nife-attrs/0000002a`.

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

### C. The error range of a reply word

Background. A reply's first word is an `i64`; negative is a negated errno. Nothing bounds the
errno, so `reply_errno` must map every negative word, and `i64::MIN` has no negation. Linux
settled the same convention by bounding it: an error is `-4095..=-1`, and anything else is a
value. The bound is a statement about the wire that every implementer reads.

Recommendation: adopt Linux's bound, `MAXIMUM_ERRNO = 4095`, now, before any implementer outside
nife exists. A negative word outside `-MAXIMUM_ERRNO..=-1` is a protocol violation and decodes as
`EPROTO` (71), so it is never read as success. `reply_err` asserts its argument is in
`1..=MAXIMUM_ERRNO`. Every reply any current server sends is unchanged. Tightening later would
not be compatible; this is the restrictive choice.

Prior art. Linux `include/linux/err.h` (`MAX_ERRNO`, `IS_ERR_VALUE`); FreeBSD returns errno in a
separate register with the carry flag, which this two-word protocol does not have; 9P2000.L
carries the errno as its own field.

Concrete example. Today `reply_errno(-5_000_000_000)` is `Some(705_032_704)` (a truncated
cast). Under the rule it is `Some(71)`, and `reply_errno(-2)` stays `Some(2)`.

## Done when

The checks are in [the appendix](1090-filesystem-contract-outside-review/done-when.md).

## BUGS

- Item 19 came from reading, not from a 32-bit build; the crate names no 32-bit target.
- Item 9 relies on the classification in `notes/mutation-testing/`, which predates the file split
  of milestone 845 and names some tests by their old module; the groups still hold, the line
  numbers do not.
- The review did not run the template's workflows end to end. The gate results come from the
  template's helpers, which the reviewer ran by hand on a standalone copy with the same arguments
  the workflows pass. The exception is `coverage`, where the reviewer applied the floor to a locally
  produced lcov.
- Milestone 1017's table pairs this crate with `filesystem_fixture` for a dev-dependency that is
  gone; the recording lane should correct that row or note it here.

## Index row

An outside review of `filesystem_contract` before it leaves nife. Five items are red at the moving
commit (the stand-alone text, clippy's proper-noun list, the falsification patch paths, one
uncovered file, 22 names the gates refuse). Eight come before the first tag (a non-total errno
decoder, packers that mask, seven unproven properties, the equivalent-mutant list, the README and
prose gates, the `rm` module). Nine are for later. Three decisions go to the architect: the on-disk
`.nife-attrs` name, the naming pass with units, and a bound on the error range of a reply word.
