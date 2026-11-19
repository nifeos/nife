---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 886, 956, 1017
decision_dependencies: 151, 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1091. `filesystem_fixture`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/filesystem_fixture` as a stranger would meet it, on a fresh
clone of `nifeos/nife` at `137639a3a`. It changes nothing; it lists what the lanes that move the
crate change, and the two decisions only an architect can take.

There is no review package for this crate: `notes/crate-reviews/` has no `filesystem-fixture.md`
and `notes/crate-reviews.md` has no row for it. This review therefore has no settled baseline to
respect beyond the rulings already recorded in the tree. Milestone 886 (the filesystem test fixture
moves to its own crate) ruled where the line between this crate and `filesystem_contract` sits and
which protocol words stay behind. Milestone 956 (the linked interfaces carry checkable versions)
assigned interface id 16 at version 1. Milestone 1017 (the contract crates leave in this order)
placed the crate in step 2. None of those is reopened here.

The crate is on `crates/externalization-list.txt`. Milestone 1017's table pairs it with
`filesystem_contract` ("the two leave together") because the contract had a dev-dependency on the
fixture. That dev-dependency is gone: commit `ce4ea2015` (pull request #3042) moved the tests that
needed the fixture into the fixture's own test file, and `crates/filesystem_contract/Cargo.toml`
now has no dependency table of any kind. The dependency runs one way, fixture to contract. The two
still leave together, for a different reason: Decision 1 puts them in one repository as a two-crate
workspace, where the fixture links the contract by path. The README and crate-root `BUGS` that say
the contract leaves first and this crate then pins it by revision are stale (item 2).

Reuse: the template `nifeos/crate-workflows` and its scaffold and helpers; `script/crate-standalone`,
`script/crate-review-packet`, `helpers/lint_externalized_crates.py` and `helpers/name_release.py`
in this tree; cargo-mutants' recorded census. No new tool.

## Finding status

One line per finding, in the review's order: items 1 to 5 in
[the blocks-the-move appendix](1091-filesystem-fixture-outside-review/blocks-the-move.md), items 6
to 12 in [the before-the-tag appendix](1091-filesystem-fixture-outside-review/before-the-tag.md),
items 13 to 18 in [the later-items appendix](1091-filesystem-fixture-outside-review/later-items.md),
then the decisions and the release step.

- Item 1 (P0), the template's cargo-deny refuses a nifeos git dependency: Done, in the template:
  [crate-workflows#66](https://github.com/nifeos/crate-workflows/pull/66) allows git sources on the
  organization, and [crate-workflows#64](https://github.com/nifeos/crate-workflows/pull/64) supports
  a repository of several crates.
- Item 2 (P0), the order and pairing are stale: Ruled (Decision 1, 2026-11-14 UTC). Not built: the
  crate's README (line 58) and root `BUGS` (`src/lib.rs` line 12) still say the contract leaves
  first and this crate then pins it by revision. Under the ruling both say the crate depends on
  `filesystem_contract` by path inside one workspace. Milestone 1017's step 2 row still gives the
  removed dev-dependency as the reason for the pairing, and its owner corrects it.
- Item 3 (P0), acronyms and British spelling: Open. The doc comments read in full
  ([#3566](https://github.com/nifeos/nife/pull/3566)); `EL0` remains in the `MOTD` literal
  (`src/lib.rs` line 44), which the system tests compare, and changes with item 6.
- Item 4 (P0), names the gates refuse: Ruled (Decision 2, maintainer, provisional, 2026-11-14 UTC).
  Not built: `SCRATCH_INIT`, `MADE_DIR`, `NAV_DIR`, `TOUCH_MTIME_ADVANCED`, `twotrees`, `navscape`
  and the rest of the three tables are still in the crate; one rename lane applies them across the
  nine manifests, with aliases on the kept command names.
- Item 5 (P0), no review package and 244 unratified names: Open. `notes/crate-reviews/` has no
  package for this crate; it comes after item 4 and may cover both crates of the shared repository.
- Item 6 (P1), the crate describes this tree: Open. Eight mentions of nife remain in `src/lib.rs`
  and one in the README, and the data literals (`"nife walk entry\n"`, `"nife boot"`) change with
  the image builder and `xtask/src/disk_check.rs`.
- Item 7 (P1), history residue: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 8 (P1), the `durability` module is unused: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566) deleted the module.
- Item 9 (P1), the shared-frame witness doc states the refuted outcome: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 10 (P1), 14 equivalent mutants: Done, [#3566](https://github.com/nifeos/nife/pull/3566). The
  sites now read `1` and `0xFF`.
- Item 11 (P1), caller workflows, manifest and stranger run: Open. The manifest has no
  `repository` field and its `publish` comment cites this tree's allowlist; the caller workflows
  (three targets, `allow-unpublished`), accepted words and the stranger run come with the move.
- Item 12 (P1), the README's claims mislead: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 13 (P2), `rg_match_path` unbounded: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 14 (P2), `wide_name` wraps: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 15 (P2), the throughput widths mix `usize` and `u64`: Open. `throughput::UNIT` is still a
  `usize`; one width changes the casts in the kernel bench and the client with it.
- Item 16 (P2), hand-copied distinct-bits lists: Open. Public `ALL` arrays per witness module, new
  provisional names, with one test over them.
- Item 17 (P2), the doc comments are a note: Open. A rewrite of about 11,000 words, the reasons
  moving to the new repository's `docs/`.
- Item 18 (P2), outsiders get nothing to run: Open. An outsider-runnable harness is outside this
  milestone's scope and has no owner; this line is its record.
- Decision 1, the repository: Ruled (2026-11-14 UTC): one workspace repository shared with
  `filesystem_contract`. Not built: the crate text (item 2), and the move at milestone 1017's
  step 2, after items 1 to 5.
- Decision 2, the names: Ruled (maintainer, provisional, 2026-11-14 UTC). Not built: item 4 is the
  work.
- Review clearance of the shared repository: Open. Under Open decisions.

## Open decisions

### Clear the review of the shared `filesystem_contract` and `filesystem_fixture` repository

This is one decision with milestone 1090's (`filesystem_contract`: what an outside maintainer would
fix before it leaves), and its brief is there: [clear the review of `filesystem_contract` and
`filesystem_fixture`, and create their
repository](1090-filesystem-contract-outside-review.md#clear-the-review-of-filesystem_contract-and-filesystem_fixture-and-create-their-repository).
For this crate, the clearance ratifies the 244 names of item 5's package, after Decision 2's renames
have landed. Door: one-way, a repository and names that other repositories pin. Blocks: the move.

## Goal

A stranger who opens `nifeos/filesystem_fixture` can tell in one screen what it holds and which
programs read it. The stranger can build and test it from the README alone on stable Rust. Every
gate of the shared template is green for the crate's own reasons. The crate names no program of this tree,
describes no internals of it, and carries no history. Nothing in it goes to crates.io, by design, and the manifest says so in words a stranger can act on.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1091-filesystem-fixture-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1091-filesystem-fixture-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S half a day, M a day or two. A door is one-way when
somebody outside this repository acts on it after the move (a repository created, a published name
a pin reaches, a layout another repository reads); everything else is two-way and the maintainer
decides it. Line numbers are in `crates/filesystem_fixture/src/lib.rs` unless a path says otherwise.

### P0: blocks the move

The P0 items are in [the appendix](1091-filesystem-fixture-outside-review/blocks-the-move.md).

### P2: later

The P2 items are in [the appendix](1091-filesystem-fixture-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1091-filesystem-fixture-outside-review/before-the-tag.md).

## Decisions for the architect

Both decisions are ruled. Decision 1 was a one-way door; Decision 2 is the maintainer's provisional
ruling on a two-way one. Everything else above is the maintainer's.

### Decision 1: the fixture shares one repository with `filesystem_contract`

Ruled 2026-11-14 (UTC): `filesystem_fixture` gets no repository of its own. It shares one
repository with `filesystem_contract`, as two crates in one Cargo workspace, and keeps
`publish = false`. The two move together at milestone 1017's step 2, after items 1 to 5 have
landed. Before the move a lane checks that the template handles a two-crate workspace, which
[crate-workflows#64](https://github.com/nifeos/crate-workflows/pull/64) added.

Why. The test of whether a crate leaves is that it is publishable or used by more than one
repository. This crate never publishes (it is test data, and the template's `allow-unpublished`
input exists for the case), so the test is sharing. Eleven manifests in this tree link it. After
1017's step 9 the `redoxfs` repository links it too, for four constants: the server's compile-time
assertion on `throughput::RECORD`, the crash injector's `crash::CUT`, `mkfs`'s `blank` layout and
the walk model. The ruling refuses copying those four constants into `redoxfs` instead, because they
are agreements between a server in one repository and a test in another, and a copied constant is
how two sides drift. The fixture depends on the contract, seven of its nine consumers link the
contract as well, and one repository keeps the contract and its test data agreeing in one pull
request.

Prior art: xfstests and pjdfstest are repositories of filesystem test data shared by several
implementations. `futures-test` and `tokio-test` are test-support crates kept in the workspace of
the library they test. `wasmtime-test-util` is a workspace test-utility crate that never publishes.

### Decision 2: the names the template refuses, and the units and shapes the naming page requires

Recommendation first, one line per name, grouped by the rule that forces it. One rename lane applies a ruled name across the nine dependents.

Refused by `spelled_out` and `register_names` at ceiling 0 (these block the move):

| today | recommended | why |
|---|---|---|
| `SCRATCH_INIT` | `SCRATCH_PLACEHOLDER` | `init` is spelled out; the doc comment already calls it placeholder contents, and "initialize" would be the wrong word |
| `tree::MADE_DIR`, `navscape::MADE_DIR` | `MADE_DIRECTORY` | `dir` is spelled out |
| `dirscape::MADE_A_DIR` | `MADE_A_DIRECTORY` | same |
| `tree::NAV_DIR` | `NAVIGATION_DIRECTORY` | same, with the family below |
| `tree::GLOB_DIR` | `GLOB_DIRECTORY` | same; `glob` is a standard term and stays |
| binding `dir` in `rg_match_path` | `directory` | same |
| `navscape::TOUCH_MTIME_ADVANCED` | `TOUCH_MODIFICATION_TIME_ADVANCED` | `mtime` is spelled out |
| `walk::SIZES`, `walk::SIZE_FILES` | `BYTE_REGION`, `BYTE_REGION_FILES` | the gate wants a unit after "size"; the doc comment already calls this "the byte region", and the directory on disk keeps the name `sizes` |

Required by the naming page's unit rule (no gate catches them; the sweep lane that spells units out
has not reached this crate):

| today | recommended | why |
|---|---|---|
| `blank::LBA` | `LOGICAL_BLOCK_BYTES` | it is a byte count (512), and `LBA` is an acronym a reader says as a phrase |
| `throughput::UNIT`, `TOTAL`, `RECORD` | `UNIT_BYTES`, `TOTAL_BYTES`, `RECORD_BYTES` | byte counts |
| `throughput::WARMUP` | `WARMUP_TRANSFERS` | a count names its things |
| `tree::NAV_TOUCH_AT_UNIX` | `..._UNIX_SECONDS` | a quantity carries its unit |
| `walk::WIDE_COUNT` | `WIDE_FILES` | a count names its things |

Required by the shape and abbreviation rules (the `abbreviations` report is report-only, so these
do not block the move; this review brings them now so the architect reads the crate's names once):

| today | recommended | why |
|---|---|---|
| `twotrees` | `mounted_tree` | the naming page's own example: name the property, not the count |
| `twodir` | `endpoint_boundary` | the module's headline is that the endpoint is the boundary; `dir` is spelled out |
| `dirscape` | `directory_escape` | `dir`; `escape` is the sibling module's word |
| `navscape` | `navigation` | a coined word a reader cannot say |
| `globscape` | `globbing` | same |
| `attrs`, `WROTE_ATTR`, `GRANTED_ATTRS_FAILED`, `READ_ATTRS`, `SET_AN_ATTR`, `ATTRS_FAILED` | `attributes`, `WROTE_ATTRIBUTE`, `GRANTED_ATTRIBUTES_FAILED`, `READ_ATTRIBUTES`, `SET_AN_ATTRIBUTE`, `ATTRIBUTES_FAILED` | an abbreviation that needs a decoder |
| `ABS_INNER`, `ABS_SECRET` | `ABSOLUTE_INNER`, `ABSOLUTE_SECRET` | same |
| `REDIR`, `REDIR_ONE`, `REDIR_TWO`, `REDIR_BODY` | `REDIRECTION`, ... | same |
| `SEQ_WRITE`, `SEQ_READ`, `RAND_READ`, `RAND_WRITE` | `SEQUENTIAL_WRITE`, `SEQUENTIAL_READ`, `RANDOM_READ`, `RANDOM_WRITE` | same |
| the `NAV_*` family (14 names) | `NAVIGATION_*`, or a nested `tree::navigation` module whose items drop the prefix | same; the on-disk names `nav-kept` and the rest stay as they are |
| `ESP` | `EFI_SYSTEM_PARTITION` | the phrase is spoken; `efi` is a kept acronym |
| `RG_*`, `rg_match_path`, `rm`, `RM_*`, `RMDIR_*`, `UNLINK*`, `PWD_*`, `MOTD*`, `GLOB*` | keep, with `#[doc(alias)]` and a doc comment that opens with the full name and the owner | a command name a reader types keeps its owner's spelling |

Background. The template runs `spelled_out` and `register_names` at ceiling 0 with no baseline, so
the eight and two names above make `check / standalone` red the day the repository exists. nife's
own baseline lists seven of them and stays behind. The unit and shape names do not fail a gate but
break the rule the naming page states, and a release review reopens names ratified before a later
rule, so ruling them now saves a second pass. Every name here is two-way until the move; after the
move, nine manifests in this tree and later the `redoxfs` repository pin them, which is why this review brings them as one decision rather than leaving them provisional.

Prior art: Linux's `fs/` spells `dentry` and `inode` because those are terms of art, and spells
`attribute` out in `xattr` only where POSIX wrote it. The Rust standard library's `std::fs` writes
`set_modified`, `read_dir` and `remove_dir` (Unix's words kept, its own words whole). The concrete
example is `TOUCH_MTIME_ADVANCED`. The template's gate names the fix (`modification_time`), and the
tree already spells `set_modified` on the std side of the same wire.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. Every pinner is a nifeos repository and the rename is one lane, so
the door is two-way. The three tables hold as written, with no alias of the old spellings. The
command names a reader types (`rm`, `RMDIR_*`, `PWD_*`, `RG_*` and the rest of the last row) keep
their spelling, each with `#[doc(alias)]` and a doc line naming the command. Item 4 is the work.

## Done when

The checks are in [the appendix](1091-filesystem-fixture-outside-review/done-when.md).

## BUGS

- Item 1 is the template's to fix and gates every later crate that links a contract; until it
  lands, no crate with a `git` dependency on another `nifeos` crate passes `check / supply-chain`.
  The review inferred from the repository list, not from reading each manifest, that no existing crate repository has such a dependency.
- Whether `proofs.yml` is green with no harness was not run; the `counter_frequency_contract` move
  will show it first.
- The interface version staying at 1 after the data literals of item 6 change is a judgment, not a
  rule: both peers of each literal are in this tree today. If the `redoxfs` repository leaves
  before that lane runs, `mkfs` reads `blank::MADE_BODY` from across a boundary, and the lane
  checks the order again.
- The census figures are the tree's weekly runs, not a run for this review; the window for a
  census is 08:00 to 13:00 UTC and this review ran outside it.

## Follow-on

- **Outstanding.** Every finding that Finding status above marks Open or Ruled, each with what
  remains, and each open decision under Open decisions. A lane checked each finding line against
  the crate and its pull requests on main.

## Index row

`filesystem_fixture` is 1,178 lines of test vocabulary with no `unsafe`, no proof and sixteen
passing tests, green on stable 1.88 and the pinned nightly. It leaves with `filesystem_contract`, as
the second crate of one workspace repository (ruled 2026-11-14 UTC), and never publishes. The
template now allows git sources from the organization and several crates per repository. Four things
come before the move. The crate's text states the shared workspace. A lane renames the names the
gates refuse, as Decision 2 rules. The `EL0` literal and the mentions of this tree go. The architect
ratifies the crate's names from a review package that does not yet exist and clears the shared
repository.
