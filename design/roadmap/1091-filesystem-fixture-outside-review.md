---
status: NOT-STARTED
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
respect beyond the rulings already recorded in the tree: milestone 886 (the filesystem test fixture
moves to its own crate) ruled where the line between this crate and `filesystem_contract` sits and
which protocol words stay behind; milestone 956 (the linked interfaces carry checkable versions)
assigned interface id 16 at version 1; milestone 1017 (the contract crates leave in this order)
placed the crate in step 2. None of those is reopened here.

The crate is on `crates/externalization-list.txt`. Milestone 1017's table pairs it with
`filesystem_contract` ("the two leave together") because the contract had a dev-dependency on the
fixture. That dev-dependency is gone: commit `ce4ea2015` (pull request #3042) moved the tests that
needed the fixture into the fixture's own test file, and `crates/filesystem_contract/Cargo.toml`
now has no dependency table of any kind. The dependency runs one way, fixture to contract, so the
contract leaves first and the fixture follows, pinning the contract by `git` and `rev` as §305
allows. The two can leave separately, and the README and crate-root `BUGS` that still say otherwise
are stale (item 2). The sibling review of `filesystem_contract` reached the same conclusion.

Reuse: the template `nifeos/crate-workflows` and its scaffold and helpers; `script/crate-standalone`,
`script/crate-review-packet`, `helpers/lint_externalized_crates.py` and `helpers/name_release.py`
in this tree; cargo-mutants' recorded census. No new tool.

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

1. The template's cargo-deny policy refuses the one dependency the moved crate will have.
   Finding: the moved manifest pins `filesystem_contract` by `git` and `rev` on `nifeos/<crate>`,
   the form §305 allows and 1017 prescribes. The template's `deny.toml` and its scaffold's copy both
   say `unknown-git = "deny"` under `[sources]` and allow no git source, so `check / supply-chain`
   is red on the first pull request of the new repository. No crate repository in the organization
   depends on another yet (unverified beyond the repository list), so this is the first time the
   policy meets the case.
   Evidence: `crate-workflows/deny.toml` and `crate-workflows/scaffold/deny.toml`, section
   `[sources]`; 1017, "The version policy".
   Change: one line in the template's two `deny.toml` files, `allow-org = { github = ["nifeos"] }`
   under `[sources]`, released and pinned before anyone creates this crate's repository. Every later
   crate that links a contract (step 3's `user_mode_runtime` links six) hits the same line, so the
   template is the place, not a per-crate exception.
   Door: two-way. Effort: S (a template pull request and its release).

2. The order and pairing are stale in three places.
   Finding: `README.md` line 56 and the crate root `BUGS` (lines 11 to 12) say the fixture and the
   contract leave together; 1017's step 2 row says the same, citing a dev-dependency that commit
   `ce4ea2015` removed. The contract links nothing and leaves first; the fixture follows and pins it.
   The reverse coupling is `glob`, which dev-depends on the fixture and leaves in step 5, which the
   bottom-up rule allows. The sibling review of `glob` proposes moving `crates/glob/tests/
   fixture_agreement.rs` into `crates/filesystem_fixture/tests/` with `glob` as a dev-dependency.
   That would make this crate link a crate that leaves three steps later and block step 2 until
   `glob` has left, which is the pairing problem reintroduced the other way round.
   Evidence: `README.md` lines 55 to 56; lines 11 to 12; 1017 step 2 row; `crates/glob/Cargo.toml`
   dev-dependencies; `crates/glob/tests/fixture_agreement.rs` header.
   Change: restate both `BUGS` entries as "depends on `filesystem_contract` by path until that crate
   has left; then by `git` and `rev`"; the recording lane corrects 1017's row. For the glob test,
   `crates/grant_plan/tests/` (which already depends on `glob` and leaves in step 6, after both) or
   leaving it in `glob` keeps the order; this crate must not gain a dev-dependency on `glob`.
   Door: two-way. Effort: XS.

3. Two template gates at ceiling 0 are red on the shipped text.
   Finding: `prose_acronyms` finds ten acronyms used before expansion (SMB line 98, EL0 158, PRNG
   175, RFC 360, FS 765, DOS 890, ISA 1079, BFS 1098, EFI 1151, GUID 1153). The gate judges Rust doc
   comments once per crate at ceiling 0, with no baseline. `typos` with the template's en-us locale refuses
   `defence` (`src/tests.rs` line 209) and `behaviour` (line 258), which the house style refuses as
   well.
   Evidence: the helper outputs above; `crate-workflows/README.md`, rows "acronym gate" and "house
   style, typos". Change: expand each acronym at first use or drop it. `FS` and `DOS` can go, `EL0`
   becomes "user mode", and the text can write `RFC 3339` as the document name it is. Spell the two
   words the American way.
   Door: two-way. Effort: S.

4. Names the template's name gates refuse at ceiling 0, where nife's baseline does not travel.
   Finding: `spelled_out` refuses `SCRATCH_INIT` (line 49), `MADE_DIR` (316 and `navscape::MADE_DIR`
   918), `NAV_DIR` (339), `GLOB_DIR` (463), `dirscape::MADE_A_DIR` (733), the binding `dir` in
   `rg_match_path` (696) and `TOUCH_MTIME_ADVANCED` (967). `register_names` refuses `walk::SIZES`
   (564) and `walk::SIZE_FILES` (567) for carrying no unit. Seven of these sit in
   `design/naming-rule-baseline.txt`, which stays behind. Beyond the gates, the rules in
   `design/naming.md` catch more. The unit rule ("a length or a limit carries its unit") fails on
   `blank::LBA` (a byte count, 1149), `throughput::UNIT`, `TOTAL` and `RECORD` (byte counts, 140, 150, 192),
   `throughput::WARMUP` (a transfer count, 167), `tree::NAV_TOUCH_AT_UNIX` (seconds, 367) and
   `walk::WIDE_COUNT` (a count, which names its things, 560). Its shape rule ("name the property,
   not the count") fails on `twodir` (790) and `twotrees` (824). Its abbreviation rule fails on `attrs` and the four `_ATTR`/`_ATTRS` names,
   `ABS_INNER`/`ABS_SECRET`, `REDIR*`, `SEQ_*`/`RAND_*` and the `NAV_*` family. The `abbreviations`
   report lists 88 components; `rm`, `rmdir`, `unlink`, `pwd`, `glob`, `motd` and `rg` are Unix or
   ripgrep spellings a reader types. They keep their owner's form with a `#[doc(alias)]`.
   Evidence: the helper outputs; `design/naming.md`, sections "Units", "Shapes" and "Acronyms and
   abbreviations".
   Change: the architect rules the names (Decision 2 below). One rename lane applies them with the
   consumers, since every rename crosses nine manifests.
   Door: the names become identifiers other repositories pin after the move; before the move they
   are two-way. Effort: S after the ruling.

5. The crate's names are not ratified and no review package exists.
   Finding: `script/crate-standalone` step 0 fails on the crate name (provisional) and on 244
   unratified names. The ruling that every name a stranger meets has its ratification before a crate moves stands, and the move commit strips every `Name:` block, so the ratification has to happen here.
   There is no `notes/crate-reviews/filesystem-fixture.md` and no row in `notes/crate-reviews.md`,
   so the architect has no package to read.
   Evidence: `script/crate-standalone filesystem_fixture`; `ls notes/crate-reviews/`.
   Change: a lane writes the package in the shape of the existing ones (identity, public API,
   proofs, names, license, what the move still needs, recommendation), adds the index row, and
   attaches `python3 helpers/name_release.py --draft filesystem_fixture`. Item 4's renames go in
   the same ratification so the architect reads each name once.
   Door: two-way. Effort: M.

### P2: later

The P2 items are in [the appendix](1091-filesystem-fixture-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1091-filesystem-fixture-outside-review/before-the-tag.md).

## Decisions for the architect

Two one-way doors. Everything else above is the maintainer's.

### Decision 1: the repository `nifeos/filesystem_fixture` is created, after `filesystem_contract` has left

Recommendation first: approve the repository, on the condition that `filesystem_contract` has left
and items 1 to 5 have landed, and not paired with any other crate.

Background. A repository per crate exists only with the architect's approval of that exact
repository; no lane creates one. The ruling that a crate leaves only if it is publishable or used by
more than one repository is the test. This crate never publishes (it is test data, and the template's
`allow-unpublished` input exists for the case), so the test is sharing. Today eleven manifests in
this tree link it. After 1017's step 9, the `redoxfs` repository links it too: the server's
compile-time assertion on `throughput::RECORD`, the crash injector's `crash::CUT`, `mkfs`'s
`blank` layout and the walk model. That is two repositories, and the bottom-up rule puts the
fixture before `redoxfs`.

The alternative is that the crate stays in nife and `redoxfs` copies the four constants it needs.
Every rule allows that, it costs nothing to undo, and this review refuses it for the reason the crate exists. Those four values are agreements between a server in one repository and a test in another,
and a copied constant is how two sides drift. The sibling `block.rs` assertion is the concrete
example: it holds only while both sides read one `RECORD`.

Prior art: xfstests and pjdfstest are repositories of filesystem test data shared by several
implementations, each created once the second implementation needed them. `wasmtime-test-util`
is a workspace test-utility crate that never publishes and stays with its consumers, which is the
alternative above.

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

## Index row

`filesystem_fixture` is 1,178 lines of test vocabulary with no `unsafe`, no proof and sixteen
passing tests, green on stable 1.88 and the pinned nightly. It can leave on its own after
`filesystem_contract`, not paired with it: the dev-dependency that paired them is gone. Before it
can, the template must allow git sources from the organization. Ten acronyms, two British spellings
and ten names must pass the template's gates. 18 mentions of this tree must go. The architect must
ratify 244 names from a review package that does not yet exist.
