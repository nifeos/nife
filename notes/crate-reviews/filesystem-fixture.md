# Review package: `file_system_fixture`

*Name: provisional, minted by lane lane/filesystem-contract-review-packages. This is the
`file_system_fixture` package of [crate-reviews.md](../crate-reviews.md). It has its own file because
the packages together outgrow the page.*

The architect reads this to clear the crate for its move, with `file_system_contract`, into one
shared repository at step 2 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
The crate gets no repository of its own: it is the second crate of a two-crate Cargo workspace and
never publishes (Decision 1, ruled 2026-11-14 UTC). Decision 3 of
[milestone 1091 (`file_system_fixture`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1091-filesystem-fixture-outside-review.md)
rules one package for both crates, with Decision E of
[milestone 1090 (`file_system_contract`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1090-filesystem-contract-outside-review.md).
This file and [the contract's package](filesystem-contract.md) are that package. The contract's
package asks for the clearance of the shared repository; this file asks for the fixture's names and
its half of that clearance. The findings and their states are in milestone 1091's Finding status.

## In one paragraph

The test data both sides of the file protocol agree on. The image builder writes these files into a
filesystem image. The kernel's tests, `std_exerciser` and the system tests read the same constants
back and compare. They cover the fixed tree, the `walk` byte region, the escape and subtree-grant
witnesses, the extended-attribute cases, the crash cut point, the blank volume layout and the
throughput record sizes. A disagreement between the builder and a reader is a compile error, not a
failed boot. It is `no_std`, has no `unsafe`, and depends on `file_system_contract` alone. It has no
proofs, since it holds data rather than algorithms beyond four small helpers.

## Identity

| item | value |
|---|---|
| crate | `file_system_fixture`, provisional (milestone 886 (the filesystem test fixture moves to its own crate)) |
| identity values | `INTERFACE_ID` 16, `VERSION` 1, row 16 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table. No magic word. Both carry provisional `Name:` blocks |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, literal in the manifest |
| `publish` | `false`, for good (Decision 1); the comment beside it says the crate is test data |
| `repository` | `https://github.com/nifeos/file_system_contract`, the shared repository |
| dependencies | `file_system_contract = { version = "0.1.0", path = "../file_system_contract" }`; nothing else |
| size | `src/lib.rs` 1,428 lines, `src/tests.rs` 378 lines; 6 tracked files |
| harnesses | none |
| tests | 14 unit tests and 1 doctest (the README example) |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency | `components`, `crates/walk_pricing`, `fixtures`, `kernel`, `redoxfs_server`, `std_exerciser`, `xtask` |
| development dependency | `crates/grant_plan`, `system_tests` |

Nine manifests link the crate. After the move they link it by `git` and `rev`, and after 1017's
step 9 the `redoxfs` repository does too, for the four constants Decision 1 names.

## What was run

| check | result |
|---|---|
| `cargo test`, in the crate | 14 unit tests and 1 doctest pass |
| `cargo doc --no-deps`, in the crate | no warning |
| the template's naming gates (`spelled_out`, `register_names`, `unit_symbols`, `question_shape`, `service_model`), vendored in `vendor/crate-workflows/helpers` | 316 names judged, 0 refused, by each |
| `python3 helpers/lint_externalized_crates.py --count` | 0 hits for this crate, and no waiver |
| `python3 helpers/name_release.py --draft file_system_fixture` | 236 names: the crate name provisional, the `tests` module ratified (the standing module name), 60 provisional items, 174 items with no block |
| `script/lint` and the queue's gates | see the pull request's checks |

Not run, and so not claimed: the stranger test and the template's workflows, which need the shared
repository; `cargo-mutants`, whose census has its own window; and the QEMU system tests that read
the fixture back from a built image. A crate that never publishes has no `cargo-semver-checks`
baseline and no `cargo package` rehearsal.

## Public API

| module | items |
|---|---|
| crate root | `VERSION`, `INTERFACE_ID`; `MOTD_NAME`, `MOTD`, `SCRATCH_NAME`, `SCRATCH_PLACEHOLDER`, `WRITE_PATTERN`, `SUCCESS`; the shared-frame witness (`SHARED_VICTIM_NAME` and the six beside it); `THROUGHPUT_NAME` |
| `throughput` | `UNIT_BYTES`, `TOTAL_BYTES`, `BLOCKS`, `WARMUP_TRANSFERS`, `RECORD_BYTES`, the `SEQUENTIAL_*` and `RANDOM_*` phases, `PAYLOAD_FILL`, `PHASES`, `ROLE`; `name` |
| `tree` | the fixed tree the image builder writes: `SUB`, `INNER`, `SECRET`, `ABSOLUTE_INNER`, `ABSOLUTE_SECRET`, `MADE_DIRECTORY`, the `NAVIGATION_*` family, `REDIRECTION*`, `RMTREE` and `RM_*`, `GLOBSET`, `GLOB_*`, `MANY_*`, `ROOT_ENTRIES` |
| `walk` | `ROOT`, `CHAIN`, `LEVEL`, `DEPTH`, `WIDE`, `WIDE_FILES`, `NARROW`, `BYTE_REGION`, `BYTE_REGION_FILES`, the `WALK_*` totals, the `RG_*` search cases, `MATCH_PATH_BYTES`; `wide_name`, `sized_byte`, `rg_match_path` |
| `escape`, `directory_escape`, `endpoint_boundary`, `mounted_tree`, `navigation`, `globbing`, `attributes` | each witness's verdict bits and its `ALL`, the list of every bit; `attributes` adds `EXPECTED` and its probe names |
| `crash` | `NAME`, `INITIAL`, `A`, `B`, `CUT`, `SAW_A`, `SAW_B`, `SAW_NEITHER` |
| `blank` | the blank volume layout: `DISK_BLOCKS`, `LOGICAL_BLOCK_BYTES`, `EFI_SYSTEM_PARTITION`, `LINUX`, `DATA`, `PARTITIONS`, `NAMES`, `MADE_NAME`, `MADE_BODY` |

No type from another crate appears in a public signature; the crate uses `file_system_contract`'s
constants in its own values and its tests.

## Rulings in force

- Decision 1 (ruled 2026-11-14 UTC): one workspace repository shared with `file_system_contract`,
  `publish = false`; no copy of the four constants `redoxfs` reads.
- Decision 2 (the maintainer's provisional ruling, 2026-11-14 UTC, a two-way door before the first
  tag): the three rename tables hold as written, with no alias of the old spellings. The command
  names a reader types keep their spelling, each with `#[doc(alias)]` and a doc line naming the
  command. The rename lane built them.
- Decision 3 (the maintainer's provisional ruling for the package step, 2026-11-19 UTC): one package
  covers both crates, and the move is not held for it.
- The clearance and the names (ruled 2026-11-22 UTC, #4009): the architect cleared the crate as the
  second crate of the shared repository, `publish = false`, and ratified the crate name, the
  identity pair and the item names as built.

## Names

The architect ratified the crate's names as built (2026-11-22 UTC, #4009): the crate name, the
identity pair, and every item name that `python3 helpers/name_release.py --draft file_system_fixture`
lists. The `Name:` blocks in the tree still read provisional until the move commit strips them. In
groups:

| group | names | status |
|---|---|---|
| crate | `file_system_fixture` | ratified |
| identity | `VERSION` 1, `INTERFACE_ID` 16 | ratified |
| Decision 2's renames, as built | the gate refusals (`SCRATCH_PLACEHOLDER`, `MADE_DIRECTORY`, `MADE_A_DIRECTORY`, `NAVIGATION_DIRECTORY`, `GLOB_DIRECTORY`, `TOUCH_MODIFICATION_TIME_ADVANCED`, `BYTE_REGION`, `BYTE_REGION_FILES`); the unit rule (`LOGICAL_BLOCK_BYTES`, `UNIT_BYTES`, `TOTAL_BYTES`, `RECORD_BYTES`, `WARMUP_TRANSFERS`, `NAVIGATION_TOUCH_AT_UNIX_SECONDS`, `WIDE_FILES`); the seven witness modules; the `ATTRIBUTE`, `ABSOLUTE_`, `REDIRECTION`, `SEQUENTIAL_` and `RANDOM_` spellings; `EFI_SYSTEM_PARTITION` | ratified |
| kept command names | `RG_*`, `rg_match_path`, `RM_*`, `RMDIR_*`, `UNLINK*`, `PWD_*`, `MOTD*`, `GLOB*` | ratified, each `#[doc(alias)]` where the type allows one |
| the witness lists | `ALL` in each of the seven witness modules | ratified |
| everything else | 174 items with no block, each named for what it holds | ratified |

## What still blocks the move

| item | owner |
|---|---|
| The caller workflows with `allow-unpublished` and three targets, the accepted words, and the stranger run before a tag (item 11). | the move lane |
| The nine consumers' path dependencies become `git` and `rev`, and every file `script/crate-move --dry-run` lists is read. | the move lane |

Item 17 (the doc comments read as a note) stays open and does not block the move. Item 18 (no
harness a third party can run) waits on the shared repository, and the crate's `BUGS` records it.

## Ruling

Ruled 2026-11-22 UTC (#4009, option A), with item 1 of
[the contract's package](filesystem-contract.md#decisions-for-the-architect):

1. The architect cleared `file_system_fixture` as the second crate of the shared repository
   `nifeos/file_system_contract`, `publish = false`.
2. The architect ratified the crate name `file_system_fixture`.
3. The identity pair `INTERFACE_ID` 16 at `VERSION` 1 stays frozen.
4. The architect ratified the item names as built.

Nothing here allows a tag, a publication or a repository of the fixture's own.
