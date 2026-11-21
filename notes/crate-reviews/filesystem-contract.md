# Review package: `filesystem_contract`

*Name: provisional, minted by lane lane/filesystem-contract-review-packages. This is the
`filesystem_contract` package of [crate-reviews.md](../crate-reviews.md). It has its own file because
the packages together outgrow the page.*

The architect reads this to clear the crate for its move to its own repository, at step 2 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
The crate shares that repository with `filesystem_fixture`, as two crates of one Cargo workspace in
which the fixture never publishes (ruled 2026-11-14 UTC). Decision E of
[milestone 1090 (`filesystem_contract`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1090-filesystem-contract-outside-review.md)
rules one package for both crates. This file and [the fixture's package](filesystem-fixture.md)
are that package, and this file asks for the one clearance of the shared repository. It describes
the crate as `main` leaves it. The findings and their statuses are in milestone 1090; this
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
| crate | `filesystem_contract`, ratified 2026-08-23 UTC |
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
| the template's `abbreviations` report (report only) | 85 components in 22 files: mostly the POSIX verbs and errnos Decision B keeps, then `buf` (14), `req` (6), `rw` and `dst` (3 each), and test bindings |
| `python3 helpers/lint_externalized_crates.py --count` | 20 hits for this crate; the six mentions of nife in shipped text are item 1 |
| `script/lint` and `script/test` | see the pull request's checks |

Not run, and so not claimed: the stranger test (it waits for item 10) and the template's workflows
(no repository exists). Nor `cargo-mutants` (its window is 08:00 to 13:00 UTC), `cargo package`,
fuzz, Miri or the QEMU filesystem tests.

## Public API

| module | items |
|---|---|
| crate root | `PAGE_BYTES`, `OPERATION_SHIFT`, `req`, `operation`, `READY`, `DESCENT_REFUSED` |
| `reply` | `MAXIMUM_ERRNO`, `EPROTO`, `reply_err`, `reply_errno` |
| `block` (interface 14) | identity, four opcodes, the block and sector sizes and transfer limits, `req`, `req_blocks` |
| `fs` (interface 15) | identity, `ROOT`, 23 opcodes, the transfer and window limits, the request and rename-destination packers |
| `directory` | ten rights, six errno constants, `explain`, `Rights` |
| `verb` | `Verb`, `Operand`, `TABLE` (23 rows), `of`; `file_grant` |
| `directory_entry`, `nameset`, `statfs` | the `READDIR`, name-set and `STATFS` encodings |
| `grant` | the grant packers and their limits |
| `extended_attribute` (alias `xattr`) | the specification and reply packers, `STORE_DIRECTORY`, and the submodules `list` and `store` |
| `rm` | `VERDICT_SLOT`, `OK`, `status` |

The packet lists all. No type from another crate appears in a public signature.

What a stranger may find surprising:

- Every name-taking verb refuses `.extended-attributes` with `EINVAL`, at every depth, and `READDIR`
  skips it (Decision A). A file of that name cannot exist on a volume this protocol serves.
- `block::req_blocks` decodes the low byte plus one, so a word whose low byte is 255 reads as 256
  blocks, past `TRANSFER_BLOCKS` (item 14, open).
- The packers mask an oversized field rather than refuse it (items 7 and 22, open).
- `rm` and `CLIENT_WINDOWS` are one program's convention and a capacity choice, not protocol (item
  12, open).

## What landed

What the milestone marks done, checked against the source and the runs above:

| item | state | evidence |
|---|---|---|
| 4, `rm.rs` covered | landed ([#3566](https://github.com/nifeos/nife/pull/3566)) | the test `a_status_is_the_errno_and_success_is_zero` |
| 5, Decision B's renames | landed ([#3726](https://github.com/nifeos/nife/pull/3726)) | the packet's public API carries every proposed name of the table; `#[doc(alias = "xattr")]` sits on `extended_attribute` (`src/lib.rs` line 137). The other aliases are not built |
| 6, `reply_errno` total | landed ([#3577](https://github.com/nifeos/nife/pull/3577)) | `reply::reply_errno` maps every out-of-range negative word to `EPROTO`; two harnesses prove it for every `i64`, and both fail under their patches |
| 8, proofs | partly landed ([#3566](https://github.com/nifeos/nife/pull/3566), [#3637](https://github.com/nifeos/nife/pull/3637)) | six of the 11 harnesses came with this item, each with its patch; the termination of the four walkers is unproven |
| 13, 17, 18, 19, 20 | landed ([#3566](https://github.com/nifeos/nife/pull/3566)) | no British spelling (`british_spelling` 0); the length-or-offset doc on `MAXIMUM_LENGTH_BYTES`; a `const` assertion that `usize::BITS >= 40`; no warning at 1.88 in the scratch workspace |
| Decision A, `.extended-attributes` | ruled 2026-11-14 UTC and built ([#3577](https://github.com/nifeos/nife/pull/3577)) | `extended_attribute::STORE_DIRECTORY` holds `.extended-attributes` |
| Decision C, the reply error range | ruled 2026-11-14 UTC and built ([#3577](https://github.com/nifeos/nife/pull/3577)) | `MAXIMUM_ERRNO` 4095, `EPROTO` 71, `reply_err` asserts its range in every build |
| Decision D, the store name in §99 (where Apple's metadata lands: stream or sidecar) | ruled 2026-11-19 UTC and built | §99 names `.extended-attributes` |

Still open, as the milestone states them: items 1 to 3 and 10 (the move commit) and part of item 5.
Also open: items 7, 9, 11, 12, 14 to 16, 21 and 22, and the walker termination of item 8.

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
| `block::SECTOR_BYTES` | provisional (milestone 886 (the filesystem test fixture moves to its own crate)) | |
| `rm::VERDICT_SLOT` | provisional | item 12 may move the module |
| `DESCENT_REFUSED` | provisional | with `READY`, the line milestone 886 drew |

The other 133 public items, with no block, are plain words or standard-library spellings (`encode`,
`iter`, `get`, `of`). Two families the `abbreviations` report lists are not covered by Decision B: the request
packers `req`, `req_handle`, `req_len`, `req_blocks` and the rename-destination pair `rename_dst`,
`dst_handle`, `dst_len`. `byte_sink_contract` renamed its own `req` to `first_word` under P1-9 of
milestone 1079 (`byte_sink_contract`: what an outside maintainer would fix before it leaves). The
same rule would rename these, which a stranger meets in every call site; the package brings them as
a question (Decisions for the architect, item 4) and proposes no spelling.

The full table for the ratifying pull request is the output of
`python3 helpers/name_release.py --draft filesystem_contract`.

## What still blocks the move

| item | owner |
|---|---|
| Item 1: the six mentions of nife in `README.md` (lines 5 and 87), `src/lib.rs` line 33, `src/grant.rs` lines 11 and 13 and `src/extended_attribute.rs` line 76; the `publish` comment; a `repository` field; the `Name:` strip. | move lane |
| Item 2: a crate-root `clippy.toml` with `doc-valid-idents = ["RedoxFS", ".."]`. The scratch workspace above shows nine findings without it. | move lane |
| Item 3: the eleven patches rewritten from `a/crates/filesystem_contract/...` to `a/src/...` and `git apply -p1`, and the path in `src/proofs.rs` line 4. | move lane |
| Item 5: the aliases and source lines on the kept POSIX verb and errno names, the `fs` and `statfs` aliases, and the `register_names` exemption for `block::SIZE`. The test name `a_grant_spec_carries_its_length_and_its_direction_apart`, which `spelled_out` refuses, joins this item; its name follows Decision B's `spec` row. | a rename lane before the move |
| Item 10: the README in the scaffold's shape, with the badge row and the Kani cost. | move lane |
| The workspace: one repository root manifest listing both crates; the template's caller workflows with `allow-unpublished` for the fixture. Items 2, 3, 4 and 6 of milestone 1091 (`filesystem_fixture`: what an outside maintainer would fix before it leaves) must land first. | move lane |
| The `std` farm copies `src/lib.rs` into the patched standard library; it must read the pinned checkout's source, or the move breaks the platform layer's build. | maintainer, as for `socket_contract` |
| Convert the nineteen consumers' path dependencies (including `fuzz`'s) to `git` and `rev` with `version = "0.1.0"`, and read every file `script/crate-move --dry-run` lists. | move lane |

Before the tag, as the milestone holds them: items 7, 8 (walker termination), 9, 11 and 12.

## Recommendation

Ready to clear, together with `filesystem_fixture`. The crate has no dependency, no `unsafe` and no
foreign type in its public API. Its eleven proofs each fail under their patch. It builds and tests
green at 1.88 for the host and the three bare-metal targets, alone and inside a two-crate
workspace. The two failures of `script/crate-standalone` are the ones a move writes (the name
ratification and a patch path). The two template naming refusals are one test name and the kept
`block::SIZE`, both item 5. The decisions the milestone took to the architect (A, C, D) are ruled and
built.

## Open questions

Items 7, 12, 14, 15, 16 and 22 are the maintainer's calls before the tag, and none is ruled. Items 7
and 22 change what a release build does with a bad field, so they turn one-way at the first tag.

## Decisions for the architect

1. Clear the review of `filesystem_contract`, which with the fixture's clearance is the go to create
   the shared repository with `publish = false` for both crates. Recommendation: yes. The move waits
   for items 1 to 3, 5 and 10 here and items 2, 3, 4, 6 and 11 of milestone 1091.
2. Ratify the Names section's four tables: Decision B's renames as built, the seven new spellings of
   names #2797 ratified, the kept POSIX verb and errno spellings, and the provisional names Decision
   B does not list. Provisional names stay provisional until the architect rules.
3. Ratify the identity values `block::INTERFACE_ID` 14 and `fs::INTERFACE_ID` 15, each at `VERSION`
   1, as frozen, as for the other contract crates.
4. Decide whether the request packers `req`, `req_handle`, `req_len`, `req_blocks` and the
   rename-destination functions `rename_dst`, `dst_handle`, `dst_len` keep their spelling or take
   spelled-out names before the first release. Recommendation: rename them, as
   `byte_sink_contract` renamed its `req`, since the tightening is the one that cannot come later;
   the spellings are a rename lane's proposal. Two-way until the first release.

Nothing here asks for a tag or a publish.
