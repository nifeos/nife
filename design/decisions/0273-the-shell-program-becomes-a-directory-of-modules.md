---
status: DECIDED
raised: 2026-10-10
decided: 2026-10-11
ratified_by: calef
---

# 273. The shell program becomes a directory of modules

Raised 2026-10-10 (UTC) as the cut that
[milestone 844 (the shell program is split along its seams)](../roadmap/0844-the-shell-program-is-split-along-its-seams.md)
says is calef's call before a line moves. Written by lane
`milestone/844-the-shell-program-is-split-along-its-seams`. *(Section number provisional until the
merge queue lands it.)*

## The ruling

calef ruled on 2026-10-11 at 07:23 (UTC), in the maintainer session, and the maintainer recorded it
on [PR #1914](https://github.com/nifeos/nife/pull/1914#issuecomment-6106644498). Three blocking
questions, three answers:

| Question | calef's words | What it decides |
|---|---|---|
| Do the witness roles get their own module? | *"Couldn't we design a better implementation that the witnesses are individual programs and we run them all with scripts rather than a specific build?"*, then, on doing both, *"Proceed with both"* | `witness.rs` holds them, as a temporary home. Pull request #1922 gives the shell a non-interactive script mode, turns the six witnesses into scripts the shipping shell runs, and deletes `witness.rs` and the role numbers. |
| Does `components/` get its first program directory? | *"Yes, program directory"* | `components/src/swish/`, with `main.rs` as the crate root. |
| Do helpers that four or more modules use live in `main.rs`? | *"Agreed"* | `main.rs` is the program's shared vocabulary. |

Every module name ships provisional. calef deferred them to a name-ratification pass.

**Why the directory is the right layout beyond this file.** calef noted with the second answer that
`swish` will later move to its own repository, be packaged in basalt and be versioned with `jig`.
A program that is a directory with `main.rs` and its modules beside it is the shape that extraction
lifts whole: the directory becomes the new repository's `src/`, and nothing in it names a sibling
program's file.

## What the file was

`components/src/swish.rs` was 5,920 lines on main at `396187b0b` and at `9532eeb53`, 60 fewer than
the block measured, all from the `package` builtin leaving in `7b9d12565`. Every seam was between
items, so this was a move.

## The cut, as it landed

The program moved to `components/src/swish/main.rs`, with child modules beside it.
`components/Cargo.toml`'s `[[bin]]` path follows. Every module is a child of the crate root.

| file | holds | lines |
|---|---|---|
| `main.rs` | header, the capped heap and `mod heap`, clock, `holdings`, the role table and `_start`, `interactive` through `run`, and the helpers four or more modules share | 1,090 |
| `navigation.rs` | `Nav`, the navigation builtins, `ls`'s constants, pattern expansion | 605 |
| `prompt.rs` | `apropos`, `read_line`, the shell editing its own line, Tab completion | 406 |
| `running.rs` | argv pages, the words grant, `run_image`, the manifest note, `dir_grant`, `spawn`, draining, `outcome` | 1,172 |
| `capabilities.rs` | `caps`, `caps_image`, the live generation table, `bare` | 319 |
| `pipeline.rs` | the operators `>`, `<` and `\|`, and the files the shell serves behind them | 995 |
| `jobs.rs` | `spawn_interruptible`, `watch`, `forcible`, `reclaim` | 328 |
| `witness.rs` | every witness role and its helpers, and the three witness-only `Nav` constructors and methods (`rooted`, `rooted_twice`, `name_call`) as a second `impl Nav` | 1,130 |

No file is over 1,500. `main.rs` is the parent and is well under 3,000.

The proposal called the fifth module `caps`, after the builtin. It ships as `capabilities`,
because calef ruled on 2026-10-06 (UTC) that an identifier does not abbreviate capability as
`cap`, and `script/lint` counts a module name as one. The builtin keeps its name.

The shared helpers in `main.rs` are the ones this section listed. They are the status words
(`record`, `refused`, `failed`, `current`, `last`) and the output helpers (`print`, `stage`,
`print_num`, `put_page`, `get_page`). With them are the file-system window constants, `PAGE`,
`memory_region_split`, `delegate`, and the window-disjointness assertion with `SH_CLOCK_VA`, the
one window that exists only for it. Applying the rule calef agreed to, rather than the list, added
three that four or more modules call. Two are `expansion` and `echo`, the wrappers over the `swish`
crate, with the comment that describes them as a pair; the third is `send_cap`, `delegate`'s
one-line wrapper. That moved `echo` out of `prompt`, where the proposal's table put it.

`crates/swish/src/lib.rs` (3,850 lines) is outside this milestone, and
milestone 884 (the shell logic crate is split along its seams) now covers it.

## What the move cost beyond moving code

- Visibility. 98 one-word edits: 58 top-level items, 20 `Nav` and other methods and 20 fields
  became `pub(super)`, as milestone 840 (the scheduler file is split along its seams) does. Each
  module opens with `use super::*;`, and `main.rs` globs each module, so a sibling reaches another's
  items through the parent. Five of the 58 are visible only so an intra-doc link still resolves.
  The shell is a program, so none of it is a public API.
- The alloc gate. `script/lint` greps the whole directory for `extern crate alloc`, requires
  exactly one hit, in `main.rs`, four spaces in, inside `mod heap {`, and still requires none in
  `crates/swish`. A missing directory gives zero hits and fails. It was falsified on 2026-10-11
  (UTC) against the check as committed, and it passes on the tree. It fails on an
  `extern crate alloc` appended to `prompt.rs`, on the declaration moved into a module in
  `navigation.rs`, on it moved to the crate root of `main.rs`, and with the directory moved away.
- The comment-block ratchet. The header stays one 100-line block at `main.rs:1`, untrimmed. Its
  baseline row moves from `swish.rs:1` to `swish/main.rs:1` at the same number, which §267 (a comment
  states the constraint as it is now) allows once its amendment for a split file lands (branch
  `maintainer/comment-ratchet-rows-follow-a-split`). No other block in the file was over 40, and the
  module headers this adds are under 40.
- The file-length ratchet drops its row for `swish.rs`.
- unsafe. 15 `unsafe` blocks with 15 `SAFETY` comments, all inside items that moved whole.
- ELF size, release, `.text` and friends as `llvm-size` reports them:

  | architecture | before | after | change | one codegen unit, before to after |
  |---|---:|---:|---:|---:|
  | aarch64 | 231,832 | 240,216 | +8,384 (3.6%) | +208 |
  | riscv64 | 208,918 | 218,520 | +9,602 (4.6%) | +216 |
  | x86_64 | 250,423 | 259,319 | +8,896 (3.6%) | +216 |

  The growth is codegen partitioning, as this section predicted: release builds use 16 codegen
  units without LTO, units follow modules, and a small helper called across a module line is no
  longer inlined. Built again with `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1`, the split costs about
  210 bytes on each architecture, which is the longer source paths in panic locations. Debug builds
  grew by under 1 KB. The shell is not on a measured hot path, so this lands as a recorded cost
  rather than a reason to add `#[inline]`.
- Tools that listed programs as `components/src/<name>.rs`. `script/names` and `script/metrics`
  now also read a program as `<name>/main.rs`, so `swish` stays a program with its ratified block
  rather than turning into a module. `xtask/src/manual.rs` does the same for the repository
  documentation store, so `apropos` still finds the shell's header. That is one Rust file outside
  the directory, against the block's done-when 2; without it the header would have dropped out of
  the store silently.
- Prose. Notes that state where something lives now name the module file. Records that describe
  a past state (audit reports, older roadmap blocks, decisions, dated runs, generated tables) keep
  the old path, since it was true when they were written. The 16 Rust files outside the directory
  whose comments name the old path are unchanged under done-when 2; notes/shell.md's BUGS lists
  them for a follow-up sweep.
- Proof. The program has no host tests. `script/test` on aarch64, riscv64 and x86_64 runs the
  `system_tests` modules that spawn the shell in each witness role, and `script/swish-check` runs
  too.

## What it does not decide

No public crate API, wire format or syscall changes. The role numbers `system_tests` passes at spawn
do not move.
