---
status: DECIDED
raised: 2026-10-10
decided: 2026-10-11
ratified_by: calef
---

# 274. The filesystem-protocol crate root splits into its module files

Raised 2026-10-10 (UTC) as the cut that
[milestone 845 (the filesystem-protocol crate root is split along its seams)](../roadmap/0845-the-filesystem-protocol-crate-root-is-split-along-its-seams.md)
says is calef's call before a line moves. Written by lane
`milestone/845-the-filesystem-protocol-crate-root-is-split-along-its-seams`. *(Section number
provisional until the merge queue lands it.)*

## The ruling

calef ruled on the four forks on 2026-10-11 at 07:30 (UTC), on pull request #1915:

1. Move every module, not the tests alone: *"Yes, every module with tests beside it"*. Each module
   file ends with `#[cfg(test)] mod tests;` and its tests live in `x/tests.rs`. Tests that span
   modules stay in a crate-level `tests.rs`. This replaced the proposal's `tests.rs` plus
   `tests/fixture.rs`.
2. `fixture` stays in this crate as `fixture.rs`: *"Keep it for now, and open the follow-up
   milestone"*. The follow-up is
   [milestone 886 (the filesystem test fixture moves to its own crate)](../roadmap/0886-the-filesystem-test-fixture-moves-to-its-own-crate.md).
3. The std copy's whitespace changes on four items: *"Accept the whitespace change"*. The diff of
   the generated `fsproto.rs` is in the pull request.
4. The comment-block ratchet, which holds §267 (a comment states the constraint as it is now):
   *"Amend §267"*. A listed block's row moves with it when a file splits, at the same ceiling, in
   one maintainer pull request (`maintainer/comment-ratchet-rows-follow-a-split`) that this split
   and milestones 840 and 844 rely on.

The new file names ship provisional until a name-ratification pass.

## What the file was

`crates/file_system_contract/src/lib.rs` was 5,744 lines on main at `396187b0b`. Every seam was
already an inline `pub mod`. Bodies with their four-space indent removed: `blk` 103, `fs` 663, `dir`
228, `verb` 431, `dirent` 49, `statfs` 43, `grant` 87, `nameset` 91, `xattr` 425, `fixture` 1,239.
`mod tests` was 1,937 lines, 66 tests. The 234 lines between modules are their `///` docs.

## The cut

| file | holds | lines |
|---|---|---|
| `lib.rs` | the crate docs, `PAGE`, the request word, the error boundary, each module's `///` docs above its `pub mod x;`, and `mod tests;` | 440 |
| `blk.rs`, `fs.rs`, `dir.rs`, `verb.rs`, `dirent.rs`, `statfs.rs`, `grant.rs`, `nameset.rs`, `xattr.rs` | one module each, `dir` with its three Kani proofs | 46 to 666 |
| `fixture.rs` | `fixture` | 1,239 |
| `x/tests.rs`, one per module | that module's tests, 59 in all ten | 49 to 394 |
| `tests.rs` | the seven tests that span modules | 262 |

Every module keeps the path it has, so no Rust file outside the crate changes. The module docs stay
as `///` above each `pub mod x;` in `lib.rs`, which keeps the generated copy the same text and keeps
two of the over-cap doc blocks in `lib.rs`.

A test sits beside module `x` when every other module it names is one `x` itself names, since then
the others are only its inputs. The verb table is keyed by `fs` opcodes and refers to `dir` and
`grant`, so a test of it naming those is a `verb` test. `dir::explain` renders `xattr`'s errnos and
`xattr` refers to `dir`; the two tests of `explain` are `dir`'s. Seven stay at the crate: one test of
the root's error boundary, one that sets the `blk` and `fs` opcode spaces side by side, and five that
check a `fixture` against `grant`, `nameset`, `dir` or `xattr`. The section comments that grouped
tests in the old module (the directory capability, extended attributes, the mutation-run survivors)
are repeated in each file that holds tests from that section, so every test still sits under the
heading it had. Each milestone they cite gained the gloss the citations gate asks of a file.

## The generator

`xtask/src/farm.rs`'s `std_generate_modules` copies `lib.rs` into the std farm as
`sys/pal/nife/fsproto.rs`, stripping `#![...]` lines and truncating at `#[cfg(test)]\nmod tests`.
It now also replaces each column-0 `pub mod x;` with `pub mod x {`, `x.rs` indented four spaces, and
`}`, for any job. An empty line stays empty, so nothing gains trailing whitespace.
`std_inputs_stamp` hashes each such `x.rs` too, and `STD_SRC_PATCH_VERSION` went from 10 to 11.

The inlined copy keeps each module's `#[cfg(test)] mod tests;`. std never builds with `cfg(test)`,
and a module removed by `cfg` is never looked for on disk, so the `fsproto/x/tests.rs` it would
name does not have to exist; the std farm build shows it.

The generated `fsproto.rs` differs from the one before in two ways only: those ten two-line items,
and rustfmt reflowing four lines that wrapped because of the indent (a `use super::{...}` in
`xattr::store`, and `MOTD`, `WRITE_PATTERN` and `crash::INITIAL` in `fixture`).

## What the move cost beyond moving code

- The three falsification patches under `crates/file_system_contract/falsifications/` patch the
  proofs in `dir`. Each was regenerated against `src/dir.rs`; their names and the `Falsification:`
  lines did not change.
- `bench/blk-transfer-sweep.sh` and `bench/transfer-size-sweep.sh` now edit `blk.rs` and `fs.rs`,
  with no indent.
- The ratchets. `design/file-length-baseline.tsv` lost its row for this file. Of the seven
  comment-block rows for `lib.rs`, three stay in `lib.rs` and four move with their blocks, three to
  `fs.rs` and one to `verb.rs`, at the same ceilings, under ruling 4.
- Coverage. CI's per-file floor (85% of lines) measured `verb.rs` at 53.8% once it stood alone:
  `row` builds the verb table and runs only at compile time, which the 5,744-line file had hidden.
  One test in `verb/tests.rs` now runs it, so the crate has 67 tests.
- Prose. `notes/std.md` and the overlay's `mod.rs` say the copy is inlined. `mutation-triage.csv`
  and older notes cite `lib.rs` as a historical record and stay.
- Proof. The crate's 66 host tests, the three Kani proofs and their falsifications, the std farm
  build, `script/lint`, and `script/test` on aarch64, riscv64 and x86_64.

## What it does not decide

No wire format changes: no opcode, layout or constant moves value, and no public path changes.
