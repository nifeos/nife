---
status: BUILT
raised: 2026-10-08
built: 2026-10-11
milestone_dependencies: none
decision_dependencies: 266, 275
machine_requirements: none
specific_machine: none
needs_person: no
---
# 846. The file-server core moves its tests out

*(The title and slug are provisional.)*

`redoxfs_server/src/lib.rs` is 4,785 lines at `0435a9aeb`, up from 3,806 on 2026-09-24. §266 (a
Rust source file stays under 2,000 lines) sets the ceiling and a 2026-12-31 goal of no file over
4,000. Milestone 840 (the scheduler file is split along its seams) is the model, but this block
reaches a different answer, because the file is mostly tests.

The file layout and every name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

The lane wrote it up on 2026-10-10 (UTC) as §275 (the file server core's tests move into child modules by topic), `status: PROPOSED`, with the cut re-measured
on main at `396187b0b` and a recommendation on each fork. Nothing moves until calef rules.

**calef ruled on 2026-10-11 (UTC), against this block's verdict: "b".** The block layer moves into
`block.rs` with its tests beside it, `lib.rs` keeps `Server` and its helpers, and the remaining tests
split by topic. The scope widened by ruling: the title says the tests move out, and the block layer
moves with them. The title and slug stay, because calef names things. §275 now records the layout
as built; the verdict and layout below are the block's original proposal, kept as the record of
what was ruled on.

**Built 2026-10-11 (UTC)** by branch `milestone/846-the-file-server-core-moves-its-tests-out` (pull
request #1916). `lib.rs` is 1,448 lines and the largest new file 1,137. The 77 tests pass unchanged,
and `script/test` passed on all three architectures.

Reuse: not applicable; this moves code and adds none.

## What is in the file

Measured on `0435a9aeb` from the file's own top-level items. Ranges are approximate at the edges.

| lines | count | what |
|---|---|---|
| 1 to 109 | 109 | crate docs, module declarations, `MAX_FILE_END` |
| 110 to 1195 | 1,086 | `Entry`, `Server` and its one `impl` block |
| 1196 to 1445 | 250 | listing, `Fault`, and the extended-attribute store helpers |
| 1446 to 1738 | 293 | `BlockIo`, `BlockDisk`, the block cache and `CachedDisk` |
| 1740 to 4785 | 3,046 | `mod tests`, 77 `#[test]` functions |

The code is 1,739 lines, already under the ceiling. The tests are 64% of the file. They fall into
the topics their own banners name:

| lines | count | tests of |
|---|---|---|
| 1740 to 2166 | 427 | shared helpers, open, read, write, the block layer and its cache |
| 2167 to 3381 | 1,215 | the directory capability, milestone 47 (navigation and naming) |
| 3382 to 3673 | 292 | rename |
| 3674 to 3736 | 63 | statfs |
| 3737 to 4070 | 334 | create and truncate |
| 4071 to 4785 | 715 | extended attributes |

## The verdict: move the tests, not the code

Splitting the code buys little. `impl Server` is 1,086 lines of one type's methods, and the file
without its tests is 1,739 lines, which one read sees whole. Cutting `Server` across files would
make a reader open several to follow one handle table. The block layer could leave as `block.rs`,
but nothing needs it to.

§266's BUGS section says a split that moves tests out is the cheapest one and can leave the code
no shorter. Here that is the point. The code does not need to be shorter; the reader of the code
needs not to load 3,046 lines of tests.

The tests stay a child module, not integration tests under `redoxfs_server/tests/`. They reach
private state: the `Server::fs` field, to take the disk back after a run, and the private function
`find_store`. As integration tests those would have to become public, which widens the crate's API
to suit its tests.

## A proposed layout

Every name here is provisional.

| file, provisional | from the second table | about |
|---|---|---|
| `lib.rs` | 1 to 1739, and `#[cfg(test)] mod tests;` | 1,740 |
| `tests.rs` | shared helpers, open, read, write, the block layer | 430 |
| `tests/directory_capability.rs` | the directory capability | 1,215 |
| `tests/names.rs` | rename, statfs, create and truncate | 690 |
| `tests/xattr.rs` | extended attributes | 715 |

One `tests.rs` would be 3,046 lines and over the ceiling itself, hence the four.

## What an architect has to rule

1. Whether moving the tests out is enough, rather than a split of the code as well. The
   recommendation is yes, for the reasons above. They hold at equal cost.
2. Every file name in the table.

No public crate API, wire format or syscall changes under any answer here.

## What moving the code breaks

Found with `grep` at `0435a9aeb`.

- No falsification patch, Kani harness or `lib.rs:NNNN` citation names this file. Four tests
  carry a `Falsification:` record, three of them marked not swept; the records move with their
  tests.
- `notes/benchmarks/read-path-block-contract-and-metadata-cache.md` says the cache's host tests are
  in this file, and `notes/shell-navigation.md` points at "the test beside it". Both need the new
  path. Eleven tracked files name the path in all.
- `bench/cache-slots-sweep.sh` edits `CACHE_SLOTS` in the program, `src/bin/redoxfs_server.rs`, not
  in this file, so it is unaffected.
- 12 first-parent merges in the 14 days to 2026-10-08 touched it, the fewest of the six.

## Done when

1. `redoxfs_server/src/lib.rs` is under 2,000 lines, and no new file is over 2,000.
2. The 77 host tests pass with no test body changed, and so do the crate's integration tests.
3. No Rust file outside `redoxfs_server/src/` changes, and no item's visibility widens. As built,
   one comment in `crates/filesystem_protocol` follows the const assertions to their new path, and
   the test fake `VecIo` is `pub(crate)` so the block tests can reach it (§275).
4. `script/test` passes on aarch64, riscv64 and x86_64.

## Follow-on

- **Recorded.** `CachedDisk`'s doc comment sits on `fn cacheable`, run together with that
  function's own, so the public struct has none. Found while moving and left alone because this
  change moves and does not edit; written up under "Found while moving" in
  `design/decisions/0275-the-file-server-cores-tests-move-into-child-modules.md`. Putting it back is
  one commit and takes both blocks under §267 (a comment states the constraint as it is now)'s cap.
- **Decision.** The module and file names (`block`, `directory_capability`, `entries`,
  `extended_attributes`) ship provisional for calef's name-ratification pass, as
  `design/decisions/0275-the-file-server-cores-tests-move-into-child-modules.md` records.

## Index row

The file-server core is 4,785 lines, but its code is 1,739 and the rest is tests. Splitting the
code would buy little. This moves the tests into four child-module files by topic, which brings
the code file under 2,000 lines without widening anything. The layout and every name are an
architect's call.
