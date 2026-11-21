# Review package: `filesystem_contract`

*Name: provisional, minted by lane lane/filesystem-contract-review-packages. This is the
`filesystem_contract` package of [crate-reviews.md](../crate-reviews.md). It has its own file because
the packages together outgrow the page.*

The architect reads this to clear the crate for its move to its own repository, at step 2 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
The crate shares that repository with `filesystem_fixture`, as two crates of one Cargo workspace in
which the fixture never publishes (ruled 2026-11-14 UTC). This file and
[the fixture's package](filesystem-fixture.md) are the one package that Decision E of
[milestone 1090 (`filesystem_contract`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1090-filesystem-contract-outside-review.md)
and Decision 3 of
[milestone 1091 (`filesystem_fixture`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1091-filesystem-fixture-outside-review.md)
rule: one file per crate, and one clearance of the shared repository, asked here. The package
describes the crate as `main` leaves it. The findings and their statuses are in milestone 1090; this
package does not restate them, and checks which have landed.

## In one paragraph

The two wire protocols of a filesystem stack, as code. The block protocol (interface 14) runs
between a filesystem server and a block server: read, write, size and flush, a block count in the
low byte of the first word. The file protocol (interface 15) runs between a client and the
filesystem server: 23 verbs (open, read, write, the directory verbs, the extended-attribute verbs,
`NARROW`, `BIND`), each an endpoint `CALL` of two words with bytes only in a shared region. Around
them sit the directory rights and their attenuation, the verb table a caretaker consults, the
`READDIR` and name-set encodings, the `STATFS` record, the grant packers and the extended-attribute
store. A reply word is a result or a negated error code bounded at 4095, and any other negative word
reads as `EPROTO` (Decision C). The crate is `no_std`, has no dependency of any kind and no `unsafe`,
and eleven Kani harnesses, each with a falsification patch, prove the packers, the rights and the
reply decoding.

## Identity

| item | value |
|---|---|
| crate | `filesystem_contract`, ratified (renamed from `fs_proto`; the `_contract` suffix ruled 2026-10-18 UTC) |
| identity values | `block::INTERFACE_ID` 14 and `fs::INTERFACE_ID` 15, each with `VERSION` 1, rows 14 and 15 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table. No magic word. The four constants carry provisional `Name:` blocks; see Names |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, literal in the manifest; `LICENSE-MIT` and `LICENSE-APACHE` in the crate |
| `publish` | `false`; milestone 1017 keeps moved contract crates unpublished until their own release milestone. The comment beside it cites this tree's allowlist (item 1) |
| `repository` | absent from the manifest (item 1) |
| dependencies | none, of any kind |
| size | `src` 4,121 lines in 23 files, tests included; 38 tracked files |
| harnesses | 11 Kani harnesses, 11 falsification patches |
| tests | 63 unit tests and 6 doctests |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency with a `version` key | `components`, `crates/disk_tree`, `crates/driver_process_contract`, `crates/filesystem_fixture`, `crates/login_contract`, `crates/subtree_scope`, `crates/swish`, `crates/system_initializer`, `fixtures`, `fuzz` (target `redoxfs_server_session`), `jig`, `kernel`, `redoxfs_server`, `std_exerciser`, `tools/redoxfs_host` |
| development dependency | `crates/grant_contract`, `crates/grant_plan`, `crates/program_table`, `system_tests` |
| source copied into `std` | `cargo xtask std-src` copies `src/lib.rs` into the patched standard library as `sys/pal/nife/fsproto.rs` (`xtask/src/farm/copied.rs`), so `std::fs` cannot drift from the server. The move must keep that source reachable (see What still blocks the move) |
| files that name the path | 29 files outside `design/`, `notes/` and `briefs/` name `crates/filesystem_contract` |

Nineteen manifests link the crate. After the move, `filesystem_fixture` links it by path inside the
shared workspace and every other consumer by `git` and `rev`.

## What was run

All commands ran in this lane's worktree, at `main` (`97d16ae8b`) with this lane's edits to documents
only.

| check | result |
|---|---|
| `script/crate-review-packet filesystem_contract` | the facts above: no dependency, no `unsafe`, no foreign type in a public signature, 11 harnesses paired with 11 patches, 19 dependents, four identity values |
| `script/crate-standalone filesystem_contract` | two failures, both move-time items. `names`: 172 names not ratified, the first `VERSION`. `citations`: one path into the nife tree, `src/proofs.rs` line 4 (the falsification patch path, item 3). Passing: the build on 1.88 for the host and `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf` and `x86_64-unknown-none`; all 11 Kani harnesses verified; each of the 11 patches makes its harness fail |
| `cargo test -p filesystem_contract -p filesystem_fixture` | 63 unit tests and 6 doctests in the contract crate pass (the fixture's are in its package) |
| `cargo clippy -p filesystem_contract -p filesystem_fixture --all-targets -- -D warnings` | no warning with this tree's `clippy.toml` |
| `cargo doc -p filesystem_contract -p filesystem_fixture --no-deps`, warnings denied | no warning |
| a two-crate workspace in a scratch directory, the two crates copied from `git ls-files` beside a `[workspace]` manifest that lists both | `cargo +1.88 test --workspace` passes the same 63 and 6, and `cargo +1.88 build --workspace` passes for the three bare-metal targets. `cargo +1.88 clippy --workspace --all-targets -- -D warnings` fails on nine `doc_markdown` findings (`RedoxFS`, `src/lib.rs` lines 23 and 55, `src/block.rs` line 41 and others): item 2, the crate-root `clippy.toml` the move commit writes |
| `script/names filesystem_contract` | the crate name ratified |
| `python3 helpers/name_release.py --draft filesystem_contract` | 193 names: 24 read ratified (the crate, ten standing `tests` modules, 13 items), 36 provisional, 133 with no block |
| the template's naming gates (`spelled_out`, `unit_symbols`, `register_names`, `british_spelling`, `service_model`, `contract_names`, `question_shape`, and `service_model --prose`), from a clone of `nifeos/crate-workflows` at `56437de`, with the arguments `check.yml` passes | two refusals over a ceiling of 0. `spelled_out`: the test `a_grant_spec_carries_its_length_and_its_direction_apart` (`src/grant/tests.rs` line 61), a `spec` that Decision B's table did not list. `register_names`: `block::SIZE` (`src/block.rs` line 27), the opcode Decision B keeps with an exemption that does not exist yet (item 5). The other five gates and the prose check pass, 660 names judged |
| the template's `abbreviations` report (report only) | 85 components in 22 files. Most are POSIX verb and errno spellings Decision B keeps (`opendir`, `readdir`, `enotdir`, `erofs` and the rest) and `errno` itself. The rest are `buf` (14), `req` (6), `rw` (3), `dst` (3), `blk` in three test names, and single test bindings (`dl`, `dh`, `na`, `nb`, `sl`, `ro`) |
| `python3 helpers/lint_externalized_crates.py --count` | 20 hits for this crate; the six mentions of nife in shipped text are item 1 |
| `script/lint` and `script/test` | see the pull request's checks |

Not run, and so not claimed: the stranger test, which waits for the README in the scaffold's shape
(item 10). Not run either: the template's `check`, `proofs` and `weekly` workflows, since no
repository exists, and `cargo-mutants`, whose census belongs to its own window (08:00 to 13:00 UTC);
the equivalent mutants are item 9. A first release has no baseline for `cargo-semver-checks`. The
lane ran neither `cargo package`, a fuzz target, Miri nor the QEMU-side filesystem tests.

## Public API

| module | items |
|---|---|
| crate root | `PAGE_BYTES` (4096), `OPERATION_SHIFT` (56), `req(operation)`, `operation(w0)`, `READY`, `DESCENT_REFUSED` |
| `reply` | `MAXIMUM_ERRNO` (4095), `EPROTO` (71), `reply_err(errno)`, `reply_errno(r0)` |
| `block` (interface 14) | `VERSION`, `INTERFACE_ID`; opcodes `READ`, `WRITE`, `SIZE`, `FLUSH`; `BLOCK_BYTES`, `SECTOR_BYTES`, `SECTORS_PER_BLOCK`, `TRANSFER_BLOCKS`, `MAXIMUM_TRANSFER_BYTES`; `req(operation, blocks)`, `req_blocks(w0)` |
| `fs` (interface 15) | `VERSION`, `INTERFACE_ID`, `ROOT`; 23 opcodes from `OPEN` (1) to `NARROW` (23), with `BIND`, `UNBIND`, `BIND_SYNC` and the three modification-time verbs; `TRANSFER_PAGES`, `MAXIMUM_TRANSFER_BYTES`, `CLIENT_WINDOWS`, `MAXIMUM_LENGTH_BYTES`, `MAXIMUM_HANDLE`; `req`, `req_handle`, `req_len`, `rename_dst`, `dst_handle`, `dst_len` |
| `directory` | the rights `ENUMERATE`, `READ`, `WRITE`, `CREATE`, `REMOVE`, `DESCEND`, `SETTIME`, `ALL`, `PARENT_RIGHTS`, `REMOVE_TREE`; six errno constants; `explain(errno)`; `Rights` with `root`, `attenuate`, `allows`, `denies_all`, `bits`, `resolve` |
| `verb` | `Verb` (seven public fields) with `carries_len`, `takes_name`, `reissues_handle`, `mutates`; `Operand`; `FIRST`, `LAST`, `TABLE` (23 rows), `of(operation)`; `verb::file_grant` with `Policy`, `POLICY`, `EBADF`, `of` |
| `directory_entry` | `IS_DIRECTORY`, `record_len`, `encode`, `iter`, `Entries` |
| `statfs` | `RECORD_BYTES`, `encode`, `decode`, `total_bytes`, `free_bytes` |
| `grant` | `READ`, `WRITE`, `MAXIMUM_NAME_BYTES`, `MAXIMUM_WORDS_NAME_BYTES`, `EROFS`, `ENOTDIR`, `HANDLE`, `WHOLE_NAMESPACE`; `pack_name`, `unpack_name`, `fits`, `fits_words`, `specification`, `specification_len`, `specification_rights`, `is_writable` |
| `nameset` | `MAXIMUM_NAMES`, `MAXIMUM_NAME_SET_BYTES`, `MAXIMUM_BYTES`, `IS_DIRECTORY`; `encode`, `iter`, `contains`, `count`, `Names` |
| `extended_attribute` (alias `xattr`) | `MAXIMUM_NAME_BYTES`, `MAXIMUM_VALUE_BYTES`, `MAXIMUM_ATTRIBUTES`, `MAXIMUM_KIND`, `RAW`, five errno constants, `STORE_DIRECTORY` (`.extended-attributes`); `specification`, `specification_kind`, `specification_value_len`, `reply`, `reply_kind`, `reply_value_len`, `is_valid_name`; the submodules `list` (`record_len`, `MAXIMUM_BYTES`, `encode`, `iter`, `Names`) and `store` (`RECORD_HEADER_BYTES`, `record_len`, `MAXIMUM_BYTES`, `file_name`, `iter`, `count`, `get`, `list`, `set`, `remove`, `Records`) |
| `rm` | `VERDICT_SLOT`, `OK`, `status(errno)` |

No type from another crate appears in a public signature, and the crate names no kernel type.

What a stranger may find surprising:

- Every name-taking verb refuses `.extended-attributes` with `EINVAL`, at every depth, and `READDIR`
  skips it (Decision A). A file of that name cannot exist on a volume this protocol serves.
- `block::req_blocks` decodes the low byte plus one, so a word whose low byte is 255 reads as 256
  blocks, past `TRANSFER_BLOCKS` (item 14, open).
- `directory_entry::encode` accepts an empty name (item 15, open).
- `fs::req`, `fs::rename_dst`, `grant::specification` and `extended_attribute::specification` mask an
  oversized field rather than refusing it, and `extended_attribute::reply` masks a kind (items 7 and
  22, open).
- `CLIENT_WINDOWS` (8) is documented as provisional, a capacity choice rather than a contract fact,
  and its documentation cites `abi::rendezvous::BADGE`, a type of this tree (item 12, open).
- `rm` is the verdict convention of one program, not filesystem protocol (item 12, open).

## What landed

The milestone sets no condition on this package beyond the other items landing; Decision E does not
hold the move for it. What the milestone's Finding status marks done, verified by reading the source
and by the runs above:

| item | state | evidence |
|---|---|---|
| 4, `rm.rs` covered | landed ([#3566](https://github.com/nifeos/nife/pull/3566)) | the test `a_status_is_the_errno_and_success_is_zero` |
| 5, Decision B's renames | landed ([#3726](https://github.com/nifeos/nife/pull/3726)) | the packet's public API carries every proposed name of the table; `#[doc(alias = "xattr")]` sits on `extended_attribute` (`src/lib.rs` line 137). The other aliases are not built |
| 6, `reply_errno` total | landed ([#3577](https://github.com/nifeos/nife/pull/3577)) | `reply::reply_errno` maps every out-of-range negative word to `EPROTO`; two harnesses prove it for every `i64`, and both fail under their patches |
| 8, proofs | partly landed ([#3566](https://github.com/nifeos/nife/pull/3566), [#3637](https://github.com/nifeos/nife/pull/3637)) | six of the 11 harnesses came with this item, each with its patch; the termination of the four walkers is unproven |
| 13, 17, 18, 19, 20 | landed ([#3566](https://github.com/nifeos/nife/pull/3566)) | no British spelling (`british_spelling` 0); the length-or-offset doc on `MAXIMUM_LENGTH_BYTES`; a `const` assertion that `usize::BITS >= 40`; no warning at 1.88 in the scratch workspace |
| Decision A, `.extended-attributes` | ruled 2026-11-14 UTC and built ([#3577](https://github.com/nifeos/nife/pull/3577)) | `extended_attribute::STORE_DIRECTORY` holds `.extended-attributes` |
| Decision C, the reply error range | ruled 2026-11-14 UTC and built ([#3577](https://github.com/nifeos/nife/pull/3577)) | `MAXIMUM_ERRNO` 4095, `EPROTO` 71, `reply_err` asserts its range in every build |
| Decision D, §99's store name | ruled 2026-11-19 UTC and built | §99 names `.extended-attributes` |

Still open, each as the milestone states it: items 1 to 3 and 10 (the move commit), the aliases and
the `block::SIZE` exemption of item 5, items 7, 9, 11, 12, 14, 15, 16, 21 and 22, and the walker
termination of item 8.

## Rulings in force

- Decision A (ruled 2026-11-14 UTC): the extended-attribute store is a directory in the image root
  named `.extended-attributes`. Every name-taking verb refuses that name with `EINVAL` in every
  directory and at every depth, and `READDIR` skips it. It is a disk layout and a wire rule.
- Decision C (ruled 2026-11-14 UTC): a reply's first word in `-4095..=-1` is the error code
  `1..=4095` (`MAXIMUM_ERRNO`); any other negative word is a protocol violation that `reply_errno`
  reads as `EPROTO` (71); `reply_err` asserts its code is in range in every build.
- Decision D (ruled 2026-11-19 UTC): §99 (where Apple's metadata lands: stream or sidecar) names the
  sidecar store `.extended-attributes`.
- The shared repository (ruled 2026-11-14 UTC): `filesystem_contract` and `filesystem_fixture` share
  one repository as two crates of one Cargo workspace; the fixture never publishes.
- Decision B (the maintainer's provisional ruling, 2026-11-14 UTC, a two-way door before the first
  tag): the renames of its table, applied; the POSIX verb and errno names keep their spelling, each
  with `#[doc(alias)]` and a line naming the source; no alias of the old spellings.
- Decision E (the maintainer's provisional ruling for the package step, 2026-11-19 UTC): one package
  covers both crates, and the move is not held for it. Ratifying the names and clearing the review,
  which is also the go to create the repository as `publish = false`, stay the architect's.

## Names

The architect ratified the crate name. Ten `tests` modules carry the standing module name ratified
2026-11-20 UTC. The roadmap records rulings this table does not reopen: `fs::VERSION`,
`block::VERSION` and the two `INTERFACE_ID`s, every opcode number, `ROOT` as 0, the 8-bit opcode
field at bits 63:56, the error-code choices, `NARROW`, `PARENT_RIGHTS` and `BIND_SYNC`, and the
limits of #2797. The `Name:` blocks of the constants among those still read provisional (the four
identity constants, `NARROW`, `PARENT_RIGHTS`) or carry no block (the opcodes, `ROOT`); a release
ratification records them. This lane ratifies nothing; the architect ratifies, changes or refuses
each row.

The names Decision B's table renamed, as built, provisional:

| built | was | rule |
|---|---|---|
| module `directory` | `dir` | spelled out |
| `directory_entry::IS_DIRECTORY`, `nameset::IS_DIRECTORY` | `IS_DIR` | spelled out |
| `extended_attribute::STORE_DIRECTORY` | `STORE_DIR` | spelled out; Decision A rules the value |
| `grant::specification`, `specification_len`, `specification_rights` | `spec`, `spec_len`, `spec_rights` | spelled out |
| `extended_attribute::specification`, `specification_kind`, `specification_value_len` | `spec`, `spec_kind`, `spec_value_len` | spelled out |
| module `block` | `blk` | an expansion everyone says |
| module `directory_entry` | `dirent` | the same |
| module `extended_attribute`, alias `xattr` | `xattr` | the same |
| `fs::GET_MODIFICATION_TIME`, `SET_MODIFICATION_TIME`, `SET_MODIFICATION_TIME_AT` | `GETMTIME`, `SETMTIME`, `SETMTIME_AT` | `mtime` is a ruled word |
| `block::BLOCK_BYTES` | `BLOCK_SIZE` | a size carries its unit |
| `statfs::RECORD_BYTES` | `LEN` | the same |
| `extended_attribute::store::RECORD_HEADER_BYTES` | `RECORD_HEADER` | the same |
| `nameset::MAXIMUM_BYTES` | `BYTES` | a limit opens `MAXIMUM_` |
| `grant::MAXIMUM_WORDS_NAME_BYTES` | `WORDS_NAME` | a limit with its unit |
| `PAGE_BYTES` | `PAGE` | the page-contract spelling |
| `verb::is_in_opcode_order` (a test helper) | `in_opcode_order` | question shape |

The names #2797 ratified that Decision B renamed under the later units and counts rules. Their
`Name:` blocks still read `ratified 2026-10-27 (#2797)`, carried over from the old spelling, so
`name_release.py` counts them ratified; each new spelling needs the architect's ratification:

| built | was |
|---|---|
| `extended_attribute::MAXIMUM_NAME_BYTES`, `grant::MAXIMUM_NAME_BYTES` | `MAXIMUM_NAME` |
| `extended_attribute::MAXIMUM_VALUE_BYTES` | `MAXIMUM_VALUE` |
| `fs::MAXIMUM_TRANSFER_BYTES`, `block::MAXIMUM_TRANSFER_BYTES` | `TRANSFER_MAXIMUM` |
| `fs::MAXIMUM_LENGTH_BYTES` | `MAXIMUM_LENGTH_OR_OFFSET` |
| `extended_attribute::MAXIMUM_ATTRIBUTES` | `MAXIMUM_COUNT` |

Kept as they are, with the reason, under Decision B:

| item | status | reason |
|---|---|---|
| the POSIX verbs (`OPEN`, `READ`, `WRITE`, `CLOSE`, `FSTAT`, `CREATE`, `TRUNCATE`, `OPENDIR`, `READDIR`, `MKDIR`, `RENAME`, `UNLINK`, `RMDIR`, `GETXATTR`, `SETXATTR`, `LISTXATTR`, `REMOVEXATTR`, `STATFS`, `SYNC`) | no block; the alias and the source line are not built (item 5) | a specification's term of art |
| the errno constants (`EROFS`, `EPERM`, `EISDIR`, `ENOTDIR`, `EINVAL`, `ENOTEMPTY`, `ENODATA`, `ERANGE`, `E2BIG`, `ENOSPC`, `ENOTSUP`, `EBADF`) | no block; aliases not built | the platform's spelling, as the `libc` crate keeps it |
| module `fs`, module `statfs` | no block; aliases `filesystem` and `volume_statistics` not built | `fs` mirrors `std::fs`; `statfs` is the call's name |
| `block::SIZE` | no block; the `register_names` exemption is not built | an opcode, not a size |

Built and provisional, not in Decision B's table:

| item | status | note |
|---|---|---|
| module `reply`, `reply::MAXIMUM_ERRNO`, `reply::EPROTO` | provisional (the lane of milestone 1090) | Decision C's names; `MAXIMUM_ERRNO` is Linux's `MAX_ERRNO`, `EPROTO` the platform's errno |
| module `proofs` | provisional | the module every other crate's harnesses live in |
| `directory::SETTIME` | provisional (2026-08-24) | a right, written module-qualified beside `fs::SET_MODIFICATION_TIME`; the `abbreviations` report lists `settime` |
| `directory::Rights::resolve` | provisional | |
| `grant::fits_words` | provisional | beside `fits` and `MAXIMUM_WORDS_NAME_BYTES` |
| `verb::Verb::reissues_handle` | provisional | |
| `block::SECTOR_BYTES` | provisional (milestone 886) | |
| `rm::VERDICT_SLOT` | provisional | item 12 may move the module |
| `DESCENT_REFUSED` | provisional | moved here from the fixture with `READY` (milestone 886) |

Every other public item (133 with no block) is a plain word or a standard-library spelling: `encode`,
`decode`, `iter`, `count`, `get`, `set`, `remove`, `contains`, `of`, `explain`, `record_len` and the
like. Two families the `abbreviations` report lists are not covered by Decision B: the request
packers `req`, `req_handle`, `req_len`, `req_blocks` and the rename-destination pair `rename_dst`,
`dst_handle`, `dst_len`. `byte_sink_contract` renamed its own `req` to `first_word` under P1-9 of
milestone 1079 (`byte_sink_contract`: what an outside maintainer would fix before it leaves). The
same rule would rename these, which a stranger meets in every call site; the package brings them as
a question (Decisions for the architect, item 4) and proposes no spelling.

The full table for the ratifying pull request is the output of
`python3 helpers/name_release.py --draft filesystem_contract`.

## What still blocks the move

All of these belong to the move or to items the milestone already holds open. No package item blocks
it.

| item | owner |
|---|---|
| Item 1: the six mentions of nife in `README.md` (lines 5 and 87), `src/lib.rs` line 33, `src/grant.rs` lines 11 and 13 and `src/extended_attribute.rs` line 76; the `publish` comment; a `repository` field; the `Name:` strip. | move lane |
| Item 2: a crate-root `clippy.toml` with `doc-valid-idents = ["RedoxFS", ".."]`. The scratch workspace above shows nine findings without it. | move lane |
| Item 3: the eleven patches rewritten from `a/crates/filesystem_contract/...` to `a/src/...` and `git apply -p1`, and the path in `src/proofs.rs` line 4. | move lane |
| Item 5: the aliases and source lines on the kept POSIX verb and errno names, the `fs` and `statfs` aliases, and the `register_names` exemption for `block::SIZE`. The test name `a_grant_spec_carries_its_length_and_its_direction_apart`, which `spelled_out` refuses, joins this item; its name follows Decision B's `spec` row. | a rename lane before the move |
| Item 10: the README in the scaffold's shape, with the badge row and the Kani cost. | move lane |
| The workspace: one repository root manifest listing both crates; the template's caller workflows with `allow-unpublished` for the fixture. Items 2, 3, 4 and 6 of milestone 1091 must land first. | move lane |
| The `std` farm copies `src/lib.rs` into the patched standard library; it must read the pinned checkout's source, or the move breaks the platform layer's build. | maintainer, as for `socket_contract` |
| Convert the nineteen consumers' path dependencies (including `fuzz`'s) to `git` and `rev` with `version = "0.1.0"`, and read every file `script/crate-move --dry-run` lists. | move lane |

Before the tag, as the milestone holds them: items 7, 8 (walker termination), 9, 11 and 12.

## Recommendation

Ready to clear, together with `filesystem_fixture`. The crate has no dependency and no `unsafe`, no
foreign type in its public API, eleven proofs that each fail under their patch, and builds and tests
green at 1.88 for the host and the three bare-metal targets, alone and inside a two-crate
workspace. The two failures of `script/crate-standalone` are the ones a move writes (the name
ratification and a patch path). The two template naming refusals are one test name and the kept
`block::SIZE`, both item 5. The decisions the milestone took to the architect (A, C, D) are ruled and
built.

The architect's name review acts on the Names table. Seven of Decision B's renames replace names
#2797 ratified before the units rule, and need ratifying again.

## Open questions

- Items 7 and 22 (the packers assert rather than mask) change what a release build does with a bad
  field. Both wait for the caretaker call sites that forward a client's length or handle to refuse
  first. They are two-way until the first tag and one-way after it.
- Items 14 (`req_blocks` decodes 256) and 15 (`directory_entry::encode` accepts an empty name) are
  two-way doors the maintainer rules provisionally; neither is ruled.
- Item 16 (one `errno` module for the three spellings of the error vocabulary) mints a module name,
  a provisional maintainer call that Decision B's table does not list.
- Item 12: where `rm` lands (the program or `byte_sink_contract`) and whether `CLIENT_WINDOWS` is a
  contract fact are placement calls the maintainer makes.

## Decisions for the architect

1. Clear the review of `filesystem_contract`, which with the fixture's clearance is the go to create
   the shared repository with `publish = false` for both crates. Recommendation: yes. The move waits
   for items 1 to 3, 5 and 10 here and items 2, 3, 4, 6 and 11 of milestone 1091.
2. Ratify the Names table: Decision B's renames as built, including the seven new spellings of names
   #2797 ratified (`MAXIMUM_NAME_BYTES` twice, `MAXIMUM_VALUE_BYTES`, `MAXIMUM_TRANSFER_BYTES` twice,
   `MAXIMUM_LENGTH_BYTES`, `MAXIMUM_ATTRIBUTES`), the provisional names that table does not list
   (`reply`, `MAXIMUM_ERRNO`, `EPROTO`, `proofs`, `SETTIME`, `resolve`, `fits_words`,
   `reissues_handle`, `SECTOR_BYTES`, `VERDICT_SLOT`, `DESCENT_REFUSED`), and the kept POSIX verb and
   errno spellings. Provisional names stay provisional until the architect rules.
3. Ratify the identity values `block::INTERFACE_ID` 14 and `fs::INTERFACE_ID` 15, each at `VERSION`
   1, as frozen, as for the other contract crates.
4. Decide whether the request packers `req`, `req_handle`, `req_len`, `req_blocks` and the
   rename-destination functions `rename_dst`, `dst_handle`, `dst_len` keep their spelling or take
   spelled-out names before the first release. Recommendation: rename them, as
   `byte_sink_contract` renamed its `req`, since the tightening is the one that cannot come later;
   the spellings are a rename lane's proposal. Two-way until the first release.

Nothing here asks for a tag or a publication.
