---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 886, 956, 1017
decision_dependencies: 151, 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1091. `file_system_fixture`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/file_system_fixture` as a stranger would meet it, on a fresh
clone of `nifeos/nife` at `137639a3a`. It changes nothing; it lists what the lanes that move the
crate change, and the two decisions only an architect can take.

This review respects the rulings recorded in the tree and no earlier package; the crate's review
package is [`filesystem-fixture.md`](../../notes/crate-reviews/filesystem-fixture.md). Milestone 886 (the filesystem test fixture
moves to its own crate) ruled where the line between this crate and `file_system_contract` sits and
which protocol words stay behind. Milestone 956 (the linked interfaces carry checkable versions)
assigned interface id 16 at version 1. Milestone 1017 (the contract crates leave in this order)
placed the crate in step 2. None of those is reopened here.

The crate is on `crates/externalization-list.txt`. Milestone 1017's table pairs it with
`file_system_contract` ("the two leave together") because the contract had a dev-dependency on the
fixture. That dev-dependency is gone: commit `ce4ea2015` (pull request #3042) moved the tests that
needed the fixture into the fixture's own test file, and `crates/file_system_contract/Cargo.toml`
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
then the three decisions.

- Item 1 (P0), the template's cargo-deny refuses a nifeos git dependency: Done,
  [nifeos/crate-workflows#66](https://github.com/nifeos/crate-workflows/pull/66) allows git sources
  on the organization and
  [nifeos/crate-workflows#64](https://github.com/nifeos/crate-workflows/pull/64) supports a
  repository of several crates.
- Item 2 (P0), the order and pairing are stale: Done, [#3909](https://github.com/nifeos/nife/pull/3909).
  The README and the crate root say the dependency on `file_system_contract` is a path inside one
  workspace.
- Item 3 (P0), acronyms and British spelling: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 4 (P0), names the gates refuse: Done, [#3910](https://github.com/nifeos/nife/pull/3910) applied
  Decision 2's three tables as provisional names, and the naming gates report none refused.
- Item 5 (P0), no review package and unratified names: Done, [#4009](https://github.com/nifeos/nife/pull/4009).
  The architect ratified the crate name, `INTERFACE_ID` 16 at `VERSION` 1 and the item names as
  built, ruled 2026-11-22 UTC.
- Item 6 (P1), the crate describes this tree: Done, [#4000](https://github.com/nifeos/nife/pull/4000).
  The externalization lint counts no consumer name, and the crate has no waiver.
- Item 7 (P1), history residue: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 8 (P1), the `durability` module is unused: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 9 (P1), the shared-frame witness doc states the refuted outcome: Done,
  [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 10 (P1), 14 equivalent mutants: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 11 (P1), caller workflows, manifest and stranger run: Done in part,
  [#4000](https://github.com/nifeos/nife/pull/4000) and [#4020](https://github.com/nifeos/nife/pull/4020). The manifest and the caller
  workflows (three targets, `allow-unpublished`) are in place; the stranger run comes before a tag.
- Item 12 (P1), the README's claims mislead: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 13 (P2), `rg_match_path` unbounded: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 14 (P2), `wide_name` wraps: Done, [#3566](https://github.com/nifeos/nife/pull/3566).
- Item 15 (P2), the throughput widths mix `usize` and `u64`: Done,
  [#4000](https://github.com/nifeos/nife/pull/4000).
- Item 16 (P2), hand-copied distinct-bits lists: Done, [#4000](https://github.com/nifeos/nife/pull/4000).
  Each witness module has a public `ALL`, and a test holds each to the bits its module declares.
- Item 17 (P2), the doc comments are a note: Open.
- Item 18 (P2), outsiders get nothing to run: Waiting on milestone 1017's move. A harness a third
  party can run belongs to the shared repository once both crates are there; the README and the
  crate root's `BUGS` record the gap.
- Decision 1, the repository: Done, [#4020](https://github.com/nifeos/nife/pull/4020). One workspace repository shared with
  `file_system_contract`, ruled 2026-11-14 (UTC); nife depends on both crates at commit
  7577a7d0df339107f75747c3681f5990f116cf61.
- Decision 2, the names: Done, [#3910](https://github.com/nifeos/nife/pull/3910).
- Decision 3, the review package and clearance: Done, [#4009](https://github.com/nifeos/nife/pull/4009). Ruled 2026-11-22
  UTC: `file_system_fixture` is cleared as the second crate of the shared repository
  `nifeos/file_system_contract`, `publish = false`.

## Open decisions

None.

## Goal

A stranger who opens `nifeos/file_system_fixture` can tell in one screen what it holds and which
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
decides it. Line numbers are in `crates/file_system_fixture/src/lib.rs` unless a path says otherwise.

### P0: blocks the move

The P0 items are in [the appendix](1091-filesystem-fixture-outside-review/blocks-the-move.md).

### P2: later

The P2 items are in [the appendix](1091-filesystem-fixture-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1091-filesystem-fixture-outside-review/before-the-tag.md).

## Decisions for the architect

All three decisions are ruled. Decision 1 was a one-way door; Decisions 2 and 3 are the maintainer's
provisional rulings on two-way doors. Everything else above is the maintainer's.

### Decision 1: the fixture shares one repository with `file_system_contract`

Ruled 2026-11-14 (UTC): `file_system_fixture` gets no repository of its own. It shares one
repository with `file_system_contract`, as two crates in one Cargo workspace, and keeps
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

### Decision 3: the review package and clearance of the shared repository: maintainer ruling, provisional, 2026-11-19 (UTC)

In force: one review package covers both crates of the shared repository. A lane writes it once the
other items land, and the move is not held for it. The package step is the maintainer's provisional
ruling on a two-way door. Ratifying the 244 names after Decision 2's renames, and clearing the review,
which is also the go to create the repository as `publish = false`, stay the architect's and are asked
from the package. This is one decision with the one in milestone 1090 (`file_system_contract`: what an
outside maintainer would fix before it leaves), Decision E there.

Prior art: `jh7110_clock_and_reset` and `designware_ethernet` each had a review package cleared
before the organization made their repositories. Outside, `futures-rs` keeps `futures-test` in the
workspace of the library it tests.

Concrete example: ratifying one fixture name such as `SCRATCH_INIT` after its rename fixes the
spelling in every manifest that uses it, which is why one package lists both crates.

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

`file_system_fixture` is 1,178 lines of test vocabulary with no `unsafe`, no proof and sixteen
passing tests, green on stable 1.88 and the pinned nightly. It leaves with `file_system_contract`, as
the second crate of one workspace repository (ruled 2026-11-14 UTC), and never publishes. The
template now allows git sources from the organization and several crates per repository. Four things
come before the move. The crate's text states the shared workspace. A lane renames the names the
gates refuse, as Decision 2 rules. The `EL0` literal and the mentions of this tree go. The architect
ratifies the crate's names and clears the shared repository from a review package that a lane
writes once the other items land (Decision 3).
