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
This file and [the contract's package](filesystem-contract.md) are that package. The clearance of the shared repository is asked once, in the contract's package; this file
asks for the fixture's own names and its half of that clearance. The findings and their statuses
are in milestone 1091; this package does not restate them, and checks which have landed.

## In one paragraph

The test data both sides of the file protocol agree on. The image builder writes these files into a filesystem image.
The kernel's tests, `std_exerciser` and the system tests read the same constants back and compare.
They cover the fixed tree, the `walk` byte region, the escape and subtree-grant witnesses, the
extended-attribute cases, the crash cut point, the blank volume layout and the throughput record
sizes. A disagreement between the builder and a reader is a
compile error, not a failed boot. It is `no_std`, has no `unsafe`, and depends on
`file_system_contract` alone. It has no proofs, since it holds data rather than algorithms beyond
four small helpers.

## Identity

| item | value |
|---|---|
| crate | `file_system_fixture`, provisional (milestone 886 (the filesystem test fixture moves to its own crate)) |
| identity values | `INTERFACE_ID` 16, `VERSION` 1, row 16 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table. No magic word. Both carry provisional `Name:` blocks |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, literal in the manifest |
| `publish` | `false`, for good (Decision 1). The comment beside it cites this tree's allowlist (item 11) |
| `repository` | absent from the manifest (item 11) |
| dependencies | `file_system_contract = { version = "0.1.0", path = "../file_system_contract" }`; nothing else |
| size | `src/lib.rs` 1,145 lines, `src/tests.rs` 592 lines; 6 tracked files |
| harnesses | none |
| tests | 19 unit tests and 1 doctest (the README example) |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency | `components`, `crates/walk_pricing`, `fixtures`, `kernel`, `redoxfs_server`, `std_exerciser`, `xtask` (`install.rs`, `disk.rs`, `disk_check.rs`, `farm/tests.rs`, `suite.rs`) |
| development dependency | `crates/grant_plan`, `system_tests` |
| files that name the path | 8 files outside `design/`, `notes/` and `briefs/` name `crates/file_system_fixture` |

Nine manifests link the crate. After the move they link it by `git` and `rev`, and after 1017's
step 9 the `redoxfs` repository does too, for the four constants Decision 1 names.

## What was run

All commands ran in this lane's worktree, at `main` (`97d16ae8b`) with this lane's edits to documents
only.

| check | result |
|---|---|
| `script/crate-review-packet file_system_fixture` | the facts above: one dependency, no `unsafe`, no harness, nine dependents, two identity values |
| `script/crate-standalone file_system_fixture` | five failures. `name`: the crate name is provisional. `names`: 239 names not ratified, the first `VERSION`. `manifest`: the path dependency on `file_system_contract` leaves the copied crate. `build` and `proofs`: skipped because of that path. The last three are the tool copying one crate where the move copies two; the workspace run below stands in for them |
| a two-crate workspace in a scratch directory, both crates copied from `git ls-files` beside a `[workspace]` manifest that lists both | `cargo +1.88 test --workspace` passes the fixture's 19 unit tests and its doctest, and `cargo +1.88 build --workspace` passes for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`. The workspace's clippy failure is the contract's `RedoxFS` finding (its item 2); the fixture adds none |
| `cargo test -p file_system_fixture` | 19 unit tests and 1 doctest pass |
| `cargo clippy -p file_system_fixture --all-targets -- -D warnings` | no warning with this tree's `clippy.toml` |
| `cargo doc -p file_system_fixture --no-deps`, warnings denied | no warning |
| `script/names file_system_fixture` | the crate name provisional |
| `python3 helpers/name_release.py --draft file_system_fixture` | 229 names: the crate name provisional, the `tests` module ratified (the standing module name), 216 with no block, the rest provisional |
| the template's naming gates, from a clone of `nifeos/crate-workflows` at `56437de`, with the arguments `check.yml` passes | ten refusals over a ceiling of 0. `spelled_out` refuses eight: `SCRATCH_INIT` (`src/lib.rs` line 49), `tree::MADE_DIR` (285), `NAV_DIR` (308), `GLOB_DIR` (431), the binding `dir` (670), `MADE_A_DIR` (707), `navscape::MADE_DIR` (891) and `TOUCH_MTIME_ADVANCED` (940). `register_names` refuses two: `walk::SIZES` (529) and `SIZE_FILES` (532). Each is a row of Decision 2's first table. The other five gates and the prose check pass, 323 names judged |
| the template's `abbreviations` report (report only) | 88 components in 3 files: `nav` (14), `rm` (11), `rg` (8), `dir` (6), `redir` (4), `attrs` (4) and others, every one a row of Decision 2's third table or a command name it keeps |
| `python3 helpers/lint_externalized_crates.py --count` | 16 hits for this crate; the mentions of nife in shipped text are item 6 |
| `script/lint` and `script/test` | see the pull request's checks |

Not run, and so not claimed: the stranger test, which comes with the move (item 11), and the
template's workflows, since no repository exists. Not run either: `cargo-mutants`, whose census
belongs to its own window (08:00 to 13:00 UTC), and the QEMU system tests that read the fixture
back from a built image. A
crate that never publishes has no `cargo-semver-checks` baseline and no `cargo package` rehearsal.

## Public API

| module | items |
|---|---|
| crate root | `VERSION`, `INTERFACE_ID`; `MOTD_NAME`, `MOTD`, `SCRATCH_NAME`, `SCRATCH_INIT`, `WRITE_PATTERN`, `SUCCESS`; the shared-frame witness (`SHARED_VICTIM_NAME` and the six beside it); `THROUGHPUT_NAME` |
| `throughput` | `UNIT`, `TOTAL`, `BLOCKS`, `WARMUP`, `SEQ_WRITE`, `SEQ_READ`, `RAND_READ`, `RAND_WRITE`, `RECORD_READ`, `RECORD`, `PAYLOAD_FILL`, `PHASES`, `ROLE`; `name` |
| `escape`, `dirscape` | the path-escape witnesses and their verdict strings, among them `WROTE_ATTR`, `GRANTED_ATTRS_FAILED`, `MADE_A_DIR`, `READ_ATTRS`, `SET_AN_ATTR` |
| `tree` | the fixed tree the image builder writes: `SUB`, `INNER`, `SECRET`, `ABS_INNER`, `ABS_SECRET`, `MADE_DIR`, the `NAV_*` family (14 names, `NAV_DIR` and `NAV_TOUCH_AT_UNIX` among them), `REDIR*`, `RMTREE` and `RM_*`, `GLOBSET`, `GLOB_*` (`GLOB_DIR` among them), `MANY_*`, `ROOT_ENTRIES` |
| `walk` | `ROOT`, `CHAIN`, `LEVEL`, `DEPTH`, `WIDE`, `WIDE_COUNT`, `NARROW`, `SIZES`, `SIZE_FILES`, the `WALK_*` totals, the `RG_*` search cases, `MATCH_PATH_BYTES`; `wide_name`, `sized_byte`, `rg_match_path` |
| `twodir`, `twotrees` | the endpoint-boundary and mounted-tree witnesses (`PWD_STARTS_AT_ROOT`, `RM_IN_MOUNT_REFUSED` and the rest) |
| `navscape`, `globscape` | the navigation and globbing verdicts, among them `MADE_DIR`, `TOUCH_MTIME_ADVANCED`, `RMDIR_*` |
| `attrs` | the extended-attribute verdicts and probe (`STORE_UNNAMEABLE`, `ATTRS_FAILED`, `NAME`, `VALUE`, `KIND` and the rest) |
| `crash` | `NAME`, `INITIAL`, `A`, `B`, `CUT`, `SAW_A`, `SAW_B`, `SAW_NEITHER` |
| `blank` | the blank volume layout: `DISK_BLOCKS`, `LBA`, `ESP`, `LINUX`, `DATA`, `PARTITIONS`, `NAMES`, `MADE_NAME`, `MADE_BODY` |

234 public constants and four functions in all. No type from another crate appears in a public
signature; the crate uses `file_system_contract`'s constants in its own values and its tests.

What a stranger may find surprising:

- `MOTD` contains the bare acronym `EL0`, and the system tests compare it byte for byte, so it
  changes with the image builder (item 3).
- `walk::SMALL_BODY` (`"nife walk entry\n"`), the partition names `blank::NAMES` (`"nife boot"`,
  `"nife root"`, `"nife data"`) and `blank::MADE_BODY` ("created by nife") are data the image
  builder writes and `xtask/src/disk_check.rs` reads back (item 6).
- `throughput::UNIT` is a `usize` beside `u64` neighbors, and the casts live in the kernel bench
  and the client (item 15, open).

## What landed

| item | state | evidence |
|---|---|---|
| 1, the template accepts a nifeos git dependency and several crates | landed in the template ([crate-workflows#66](https://github.com/nifeos/crate-workflows/pull/66), [crate-workflows#64](https://github.com/nifeos/crate-workflows/pull/64)) | the scratch workspace builds and tests as one |
| 7, 9, 10, 12, 13, 14 | landed ([#3566](https://github.com/nifeos/nife/pull/3566)) | `rg_match_path` bounded and `wide_name` no longer wraps, both tested (`the_rg_match_paths_meet_at_the_region_boundaries` and its neighbors) |
| 8, the unused `durability` module | landed ([#3566](https://github.com/nifeos/nife/pull/3566)) | the module is gone |
| Decision 1, the shared repository | ruled 2026-11-14 UTC | the crate text that states the pairing is item 2 |

Still open, as the milestone states them: items 2, 3, 4, 6 and 11, and items 15 to 18. Under item
2 the README (line 58) and the root `BUGS` (`src/lib.rs` line 12) still say the contract leaves
first and this crate then pins it by revision. Milestone 1017's step 2 row still gives the removed
development dependency as the reason for the pairing; its owner corrects it.

## Rulings in force

- Decision 1 (ruled 2026-11-14 UTC): one workspace repository shared with `file_system_contract`,
  `publish = false`; no copy of the four constants `redoxfs` reads.
- Decision 2 (the maintainer's provisional ruling, 2026-11-14 UTC, a two-way door before the first
  tag): the three rename tables hold as written, with no alias of the old spellings. The command names a reader types keep their spelling, each with `#[doc(alias)]` and a doc line naming the
  command.
- Decision 3 (the maintainer's provisional ruling for the package step, 2026-11-19 UTC): one package
  covers both crates, and the move is not held for it. Ratifying the names and clearing the review,
  which is also the go to create the repository as `publish = false`, stay the architect's. The move
  follows with 1090's items 1 to 3 and 10 and this milestone's items 6 and 11.

## Names

The crate name is provisional, and nothing else in the crate is ratified except the standing `tests`
module name. 216 public items carry no `Name:` block. This lane ratifies nothing.

Decision 2's renames are not built (item 4). Ratifying the names as they stand would ratify
spellings a ruling already replaces, and ten of them fail the template's gates. So this package asks
for no ratification of the item names now. It asks for two things the rename lane does not change:

| item | status | note |
|---|---|---|
| crate `file_system_fixture` | provisional | the spelling every one of the nine manifests and the shared repository's workspace use; Decision 1 rests on it |
| `VERSION` 1, `INTERFACE_ID` 16 | provisional | milestone 956's row 16; the identity pair, like the contract's |

The rename lane applies Decision 2's three tables across the nine dependents, with aliases on the
kept command names. Its pull request then carries the full table from
`python3 helpers/name_release.py --draft file_system_fixture` for ratification. The renames as
ruled, for the architect's reading now:

| table | names |
|---|---|
| refused by the template's gates | `SCRATCH_PLACEHOLDER`, `MADE_DIRECTORY` (twice), `MADE_A_DIRECTORY`, `NAVIGATION_DIRECTORY`, `GLOB_DIRECTORY`, the binding `directory`, `TOUCH_MODIFICATION_TIME_ADVANCED`, `BYTE_REGION`, `BYTE_REGION_FILES` |
| the unit rule | `LOGICAL_BLOCK_BYTES`, `UNIT_BYTES`, `TOTAL_BYTES`, `RECORD_BYTES`, `WARMUP_TRANSFERS`, `NAV_TOUCH_AT_UNIX_SECONDS` (under its `NAVIGATION_` spelling), `WIDE_FILES` |
| shape and abbreviation | modules `mounted_tree`, `endpoint_boundary`, `directory_escape`, `navigation`, `globbing`, `attributes`; the `ATTRIBUTE`, `ABSOLUTE_`, `REDIRECTION`, `SEQUENTIAL_` and `RANDOM_` spellings; the `NAVIGATION_*` family or a nested `tree::navigation` module; `EFI_SYSTEM_PARTITION` |
| kept command names | `RG_*`, `rg_match_path`, `rm`, `RM_*`, `RMDIR_*`, `UNLINK*`, `PWD_*`, `MOTD*`, `GLOB*`, each with `#[doc(alias)]` |

The `NAV_*` row leaves a choice to the rename lane (a prefix or a nested module); the maintainer
takes it provisionally when the lane brings it.

## What still blocks the move

| item | owner |
|---|---|
| Item 2: the README (line 58) and the root `BUGS` (`src/lib.rs` line 12) state the shared workspace and the path dependency. | a lane before the move |
| Item 3: `EL0` in the `MOTD` literal, changed with item 6. | the same lane as item 6 |
| Item 4: Decision 2's three tables, across the nine manifests. Without it the template's `spelled_out` and `register_names` gates fail the day the repository exists. | one rename lane |
| Item 6: the mentions of nife in the README (line 7) and `src/lib.rs` (lines 13, 536, 583, 606, 621, 1124, 1134 and 1141), changed together with the image builder and `xtask/src/disk_check.rs` that write and read the data literals. | a lane before the move |
| Item 11: a `repository` field, the `publish` comment, the caller workflows with `allow-unpublished` and three targets, the accepted words, and the stranger run. | move lane |
| Convert the nine consumers' path dependencies to `git` and `rev`, and read every file `script/crate-move --dry-run` lists. | move lane |

Before the tag, as the milestone holds them: items 15 to 17. Item 18 (an outsider-runnable harness)
has no owner and is recorded in the milestone.

## Recommendation

Ready to clear with `file_system_contract`, for the shared repository only. The crate is test data
with no `unsafe`, one dependency on its sibling, and green builds and tests at 1.88 for the host
and the three bare-metal targets inside the two-crate workspace. Every gate refusal is a row of
Decision 2, whose renames are one lane's work and not yet built; the move waits for that lane and
for items 2, 3, 6 and 11.

## Open questions

- Item 15 (one width for the throughput sizes) touches the kernel bench and the client; it is a
  two-way door for the maintainer before the tag.
- Item 16 mints public `ALL` arrays, new provisional names, per witness module.
- Item 18 has no owner. An outsider-runnable harness belongs to the shared repository once it
  exists, or to `redoxfs` after 1017's step 9.

## Decisions for the architect

1. Clear the review of `file_system_fixture` as the second crate of the shared repository, together
   with item 1 of [the contract's package](filesystem-contract.md#decisions-for-the-architect).
   Recommendation: yes. The move waits for items 2, 3, 4, 6 and 11 here and items 1 to 3, 5 and 10
   of milestone 1090.
2. Ratify the crate name `file_system_fixture`. Recommendation: yes; it names what the crate holds,
   in the `filesystem_` family it shares a repository with.
3. Ratify the identity values `INTERFACE_ID` 16 at `VERSION` 1 as frozen, as for the contract's 14
   and 15.
4. Leave the item names unratified until Decision 2's rename lane lands, and ratify them from that
   lane's table. Recommendation: yes, so no name is ratified in a spelling a ruling already
   replaces.

Nothing here asks for a tag, a publication or a repository of the fixture's own.
