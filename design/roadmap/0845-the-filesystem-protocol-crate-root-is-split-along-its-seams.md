---
status: BUILT
raised: 2026-10-08
built: 2026-10-11
milestone_dependencies: none
decision_dependencies: 266, 274
machine_requirements: none
specific_machine: none
needs_person: no
---
# 845. The filesystem-protocol crate root is split along its seams

*(The title and slug are provisional.)*

`crates/filesystem_protocol/src/lib.rs` is 5,744 lines at `0435a9aeb`. It was 5,327 on 2026-09-24,
so it is the slowest grower of the six files over 4,000. §266 (a Rust source file stays under 2,000
lines) sets the ceiling and a 2026-12-31 goal of no file over 4,000. Milestone 840 (the scheduler
file is split along its seams) is the model.

The cut and every module name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

The lane wrote it up on 2026-10-10 (UTC) as §274 (the filesystem-protocol crate root splits into its module files), `status: PROPOSED`, with the cut re-measured
on main at `396187b0b` and a recommendation on each fork. calef ruled on 2026-10-11 (UTC). Every
module moves, each with its tests beside it in `x/tests.rs`. `fixture` stays as `fixture.rs` until
milestone 886 (the filesystem test fixture moves to its own crate) moves it. The std copy's whitespace change is accepted, and §267 (a comment
states the constraint as it is now) is amended so a row moves with its block. §274 records the ruling and the cut as built. Built 2026-10-11 (UTC) on branch
`milestone/845-the-filesystem-protocol-crate-root-is-split-along-its-seams`.

Reuse: not applicable; this moves code and adds none.

## What is in the file

Measured on `0435a9aeb`. The file is already cut along its seams: every one is an inline
`pub mod`. Only the files are missing.

| lines | count | what |
|---|---|---|
| 1 to 193 | 193 | crate docs, `PAGE`, the request word and the error boundary |
| 194 to 307 | 114 | `pub mod blk`, the block protocol |
| 308 to 1003 | 696 | `pub mod fs`, the file protocol |
| 1004 to 1279 | 276 | `pub mod dir`, directory rights, with three Kani proofs |
| 1280 to 1724 | 445 | `pub mod verb` |
| 1725 to 1876 | 152 | `pub mod dirent` and `pub mod statfs` |
| 1877 to 2133 | 257 | `pub mod grant` and `pub mod nameset` |
| 2134 to 2565 | 432 | `pub mod xattr` |
| 2566 to 3807 | 1,242 | `pub mod fixture` |
| 3808 to 5744 | 1,937 | `mod tests`, 66 `#[test]` functions |

`fixture` is not protocol. It is the names and contents of the files the test image ships, and the
witnesses' constants: 255 public items, 842 of its lines comments, used at 185 sites outside the
crate. Tests and fixture together are 55% of the file.

## Why this is not a plain move

The modules already have their paths, so moving `pub mod fs { ... }` to `fs.rs` changes no caller.
What it breaks is a copy of this file that std holds.

`xtask/src/farm.rs` (`std_generate_modules`) copies `lib.rs` into the std farm as
`sys/pal/nife/fsproto.rs`. It strips `#![...]` lines, truncates at `#[cfg(test)] mod tests`, and
writes the rest verbatim, so `std::fs` cannot drift from the server. After a split, the copy would
hold `pub mod fs;` with no file beside it, and std would not build. Today std uses `fs`, `dir`,
`dirent`, `PAGE` and `reply_errno` from it, through `patches/std-nife/overlay/`.

So the work has two steps, and only the second touches the generator.

1. Move `mod tests` to `tests.rs`. `lib.rs` falls to 3,807 lines, under §266's 2026-12-31 goal.
   The generator's truncation still matches, since it looks for `#[cfg(test)]` then `mod tests`.
   `tests.rs` is 1,937 lines, under the ceiling.
2. Move the modules to files, and teach the generator to inline each one back as
   `pub mod x { ... }`, so the copy std builds is the same text it is today. `lib.rs` falls to about
   200 lines.

## A proposed cut

Every name except the existing module names is provisional. The existing ones are public paths and
do not change.

| file | from the table | about |
|---|---|---|
| `lib.rs` | 1 to 193, the `mod` lines | 210 |
| `blk.rs` | 194 to 307 | 115 |
| `fs.rs` | 308 to 1003 | 700 |
| `dir.rs` | 1004 to 1279 | 280 |
| `verb.rs` | 1280 to 1724 | 445 |
| `dirent.rs`, `statfs.rs`, `grant.rs`, `nameset.rs` | 1725 to 2133 | 410 in all |
| `xattr.rs` | 2134 to 2565 | 430 |
| `fixture.rs` | 2566 to 3807 | 1,240 |
| `tests.rs` | 3808 to 5744 | 1,940 |

## What an architect has to rule

1. Whether `fixture` leaves this crate. It is test data in a wire-protocol crate, and the generator
   copies it into std, which never reads it. Moving it to a crate of its own changes the path
   `filesystem_protocol::fixture` at 185 sites, or needs a re-export that makes this crate depend
   on it. Either is a public crate API change and an architect fork. The recommendation is to keep
   it here, as `fixture.rs`. That is a recommendation about effort and says so: at equal cost, test
   data would not live in a protocol crate.
2. Whether the generator inlines every module, or copies only what std uses. The recommendation is
   to inline every module, so the copy is byte-for-byte unchanged. Narrowing what std carries is a
   separate change with its own reason.
3. Whether step 1 alone is enough for now. It meets the 2026-12-31 goal with no tooling change.

No wire format changes under any answer: no opcode, layout or constant moves value.

## What moving the code breaks

Found with `grep` at `0435a9aeb`.

- `xtask/src/farm.rs` names the path twice: once in `std_inputs_stamp`, the hash that decides when std is rebuilt, and once in
  `std_generate_modules`. Both must cover the new files, or a change to `fs.rs` stops rebuilding
  the farm.
- `bench/blk-transfer-sweep.sh` and `bench/transfer-size-sweep.sh` set `LIB` to this file and `sed`
  `    pub const TRANSFER_BLOCKS` and its sibling, indented four spaces. After step 2 the constant is
  in `blk.rs` or `fs.rs` with no indent. Both scripts grep for their edit and fail loudly, so this
  breaks visibly.
- Three falsification patches under `crates/filesystem_protocol/falsifications/` patch the Kani
  proofs in `dir`. Their harness paths, `dir::proofs::*`, do not change; their diffs do. Each must be
  regenerated and shown to fail again. `kani-reach.yml` lists the crate by name and is unaffected.
- 112 Rust files outside the crate use it; none changes.

## Done when

1. Step 1: `lib.rs` is under 4,000 lines and the 66 host tests pass unchanged.
2. Step 2, if ruled: `lib.rs` is under 2,000 lines, and no file in the crate is over 2,000.
3. The generated `fsproto.rs` is what it was before apart from rustfmt's reflow of four items and
   each module's `#[cfg(test)] mod tests;` (ruling 3, which replaced "byte-for-byte"), and the std
   farm builds.
4. The three Kani proofs pass, and their falsification patches apply and fail.
5. No Rust file outside the crate changes, and `script/test` passes on aarch64, riscv64 and x86_64.

## Follow-on

- **Milestone 886.** The test fixture leaves this crate: milestone 886 (the filesystem test fixture
  moves to its own crate), ruled by calef on §274's fork 2.
- **Decision.** The cut, the test placement and the std copy's diff are in
  `design/decisions/0274-the-filesystem-protocol-crate-root-splits-into-its-module-files.md`.

## Index row

The filesystem protocol file is already cut into public modules; only the files are missing. Moving
its tests out meets §266's 2026-12-31 goal with no tooling change. Moving the modules out needs the
std PAL generator taught to inline them. Whether test fixtures leave the crate is a public API fork.
The cut and every new name are an architect's call.
