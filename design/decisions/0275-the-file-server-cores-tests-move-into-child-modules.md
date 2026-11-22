---
status: DECIDED
raised: 2026-10-10
decided: 2026-10-11
ratified_by: calef
---

# 275. The file server core's tests move into child modules by topic

Raised 2026-10-10 (UTC) as the cut that
[milestone 846 (the file server core moves its tests out)](../roadmap/0846-the-file-server-core-moves-its-tests-out.md)
says is calef's call before a line moves. Written by lane
`milestone/846-the-file-server-core-moves-its-tests-out`. *(Section number provisional until the
merge queue lands it.)*

## The ruling

The lane proposed moving the tests alone and leaving the code whole. calef, 2026-10-11 07:32 (UTC),
on #1916, after asking earlier whether a split of the tests alone was the right axis: "b". That is
the other answer: the block layer leaves `lib.rs` too. As the maintainer relayed it, the block layer
is the device I/O and caching beneath redoxfs, and `Server` reaches it only through `Disk`, so it
moves into `block.rs` with its tests beside it. `lib.rs` keeps the server and its helpers. The
remaining tests split by topic and sit beside the code they test, as calef ruled for 842 (#1913) and
845 (#1915). File names are deferred to a name-ratification pass and ship provisional.

The scope widened by ruling. The milestone's title says the tests move out; the block layer moves
with them. The title, this section's heading and both slugs stay, because calef names things.

## What the file was

`redoxfs_server/src/lib.rs` was 4,785 lines on main at `396187b0b`. Code was lines 1 to 1,738:
`Server` and its helpers to 1,443, the block layer from 1,445. `mod tests` was lines 1,739 to 4,785,
3,047 lines and 77 tests.

## The layout

| file | holds | lines |
|---|---|---|
| `src/lib.rs` | crate docs, `Server` and its helpers, `mod block;` and a named re-export | 1,448 |
| `src/block.rs` | `BLOCK`, `CACHE_SLOTS`, the const assertions, `BlockIo`, `BlockDisk`, `BlockCache`, `cacheable`, `CachedDisk` | 307 |
| `src/block/tests.rs` | `CountingDisk`, the six `CachedDisk` tests and the header-ring test | 206 |
| `src/tests.rs` | shared helpers, sync, mtime, open, read, write and the chunked repeat writes | 489 |
| `src/tests/directory_capability.rs` | the directory capability, milestone 47 (navigation and naming) | 1,137 |
| `src/tests/entries.rs` | rename, statfs, create and truncate | 689 |
| `src/tests/extended_attributes.rs` | extended attributes | 529 |

`block` is a private module. `lib.rs` re-exports `BLOCK`, `BlockDisk`, `BlockIo`, `CACHE_SLOTS` and
`CachedDisk` by name, the 842 pattern, because the crate's binaries, its integration tests, an
example and `crash.rs` and `walk_model.rs` name them as `redoxfs_server::` or `crate::` paths. Every
such path still resolves, and no item outside the crate is touched.

The test files under `tests/` are children of the crate-level `tests` module, so they reach
`Server::fs` and `find_store` through `use super::*` as before. `image_with_tree`,
`server_with_tree` and `list` sit in `tests.rs` because the entries and attribute tests use them.

## What the move needed beyond moving lines

- **`VecIo` is `pub(crate)`, field and all.** It is the `BlockIo` fake the sync and create tests use
  and so does `cached_disk_reads_the_same_bytes_as_uncached`, which now lives under `block`. It is
  `cfg(test)`; nothing in any build's API changes.
- **`xattr.rs` became `extended_attributes.rs`.** A child module named `xattr` shadows the crate
  root's `filesystem_protocol::xattr` import under `use super::*`, and 25 references to
  `xattr::RAW` and its neighbors stopped resolving. The name is the banner's and is provisional.
- **`the_header_ring_is_never_cached` moved to `block/tests.rs`.** It sat in the directory
  capability range and reads `CachedDisk`'s private `inner` field, which only `block` can see.
- Imports at the top of each new file, and a four-line module doc on `block.rs`.

No test body changed. The 77 tests still pass, and each test's path gained its module
(`tests::entries::a_rename_survives_a_reopen`). Nothing filters on a module path, and the bare name
still works as a `cargo test` filter.

## Proof

Host `cargo test`, clippy and `script/lint` as usual, and `script/test` on the three architectures.
The release ELFs do not hash the same, which the proposal expected: panic locations carry file
names and line numbers, and the moved code lands in different codegen units. Function sizes match
except in six monomorphized functions in each architecture's server binary (inlining around
`CachedDisk` and `Transaction::commit`); `mkfs` matches on all three.

## Records that follow the move

- `design/file-length-baseline.tsv` loses the file's row.
- `design/comment-block-baseline.tsv`'s row for `redoxfs_server/src/lib.rs` moves to `block.rs`
  at the same 44 lines, which needs §267 (a comment states the constraint as it is now) to carry a row on a split
  (maintainer PR #1924).
- `notes/benchmarks/read-path-block-contract-and-metadata-cache.md`, `notes/fs-server/throughput.md`,
  `notes/shell-navigation.md`, `bench/cache-slots-sweep.sh` and one comment in
  `crates/file_system_contract` now name the new path.

## Found while moving

The 44-line block is two doc comments run together: `CachedDisk`'s (36 lines) and `cacheable`'s (8),
all attached to `fn cacheable`, so `pub struct CachedDisk` has no doc. Milestone 606 (a directory
walk costs what it does on Linux) inserted the function between the doc and its struct. Moving the
struct's doc back beside it is one commit, and it takes both blocks under §267's cap; it was left
out because this change moves and does not edit.

## What it does not decide

No public crate API, wire format or system call changes. The module and file names are provisional.
