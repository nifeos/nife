---
status: DECIDED
raised: 2026-10-11
decided: 2026-10-11
ratified_by: calef
---

# 280. The swish-check harness splits into submodules, with its script in one

Raised 2026-10-11 (UTC) as the cut that
[milestone 847 (the swish-check harness is split along its seams)](../roadmap/0847-the-swish-check-harness-is-split-along-its-seams.md)
says is calef's call before a line moves. Written by lane
`milestone/847-the-swish-check-harness-is-split-along-its-seams`. *(Section number provisional
until the merge queue lands it.)*

It blocked the move and nothing else.

## The ruling

calef asked nothing separate on this section. Its one question, whether the script table gets its
own module, is answered A by the precedent he set for every split on 2026-10-11 (UTC), on
[PR #1959](https://github.com/nifeos/nife/pull/1959): *"Agreed, make the parent an index."* §282
(when a file is split, the parent becomes an index) records that precedent. A parent that holds
only docs, `mod` lines, `use` and re-exports cannot keep the rows, so the table has its own module.
The maintainer recorded the ruling on
[PR #1958](https://github.com/nifeos/nife/pull/1958#issuecomment-6115401832). Module names were not
asked; they ship provisional for calef's naming pass.

## What the file is today

`xtask/src/swish_check.rs` is 4,274 lines on main at `ae6b91e79`, 302 fewer than the block
measured. Milestone 809 (the package client becomes a program) moved the gauge filter out to
`xtask/src/swish_check/gauge.rs` (381 lines) on 2026-10-10, beside the parent rather than under a
`mod.rs`. Measured from the file's own items:

| lines | count | what |
|---|---|---|
| 1 to 104 | 104 | module docs, `mod gauge`, imports, the harness's doc, the probes, flood counting |
| 105 to 166 | 62 | `swish_check`, the entry |
| 167 to 1439 | 1,273 | `Line`, `line` and `keys`, the keyboard-boot and after-reboot scripts, `SWISH_CHECK_SCRIPT` (1,106 lines, 168 rows), the `^C` lines |
| 1440 to 1598 | 159 | per-architecture omissions, the x86_64 hand-over markers, timeouts, the leg-cost verdict |
| 1599 to 1873 | 275 | reading a transcript: fault tokens, `free`'s row, markers, the boot claim |
| 1874 to 2152 | 279 | the per-architecture leg, the USB keyboard boot, `kvm_is_usable` |
| 2153 to 3203 | 1,051 | `swish_check_boot`, one function |
| 3204 to 3679 | 476 | keystrokes, the firmware marker, the reboot phase, the graphical launch, the `rg` walk |
| 3680 to 4274 | 595 | `mod tests`, 33 tests |

The script table grows. `SWISH_CHECK_SCRIPT` was 405 lines on 2026-09-23, 1,004 on 2026-10-04 and
1,099 on 2026-10-11. Of the 87 non-merge commits that touched the file since 2026-09-23, 51 edited
the table and 16 edited nothing else.

## The recommended cut

Submodules of `swish_check`, children of the parent, as §271 (the scheduler file splits into
submodules of `sched`) ruled for `sched`. The parent stays `swish_check.rs` beside
`swish_check/`, the layout `gauge` already uses and milestone 840 (the scheduler file is split
along its seams) kept. Every module name is provisional.

The parent is an index. calef set that precedent tree-wide on 2026-10-11 (UTC), on #1959, for
milestone 872 (the kernel's user-mode file is split along its seams): *"Agreed, make the parent an
index."* A split parent holds only module docs, `mod` lines, `use` and re-exports, and every type
and function lives in a child named for it. §282 (when a file is split, the parent becomes an
index) records it, with the lint check that holds it. So the entry and the probes go to a child,
`run`, and the parent re-exports `swish_check` from it by name, which keeps `main.rs`'s one import,
`crate::swish_check::swish_check`, unchanged.

| file | holds | lines, about |
|---|---|---|
| `swish_check.rs` | module docs, `mod` lines, the shared imports, one re-export | 35 |
| `swish_check/run.rs` | the harness's doc, the probes, flood counting, the entry | 160 |
| `swish_check/script.rs` | `Line` and its constructors, the three scripts, the `^C` lines | 1,280 |
| `swish_check/verdict.rs` | omissions, hand-over markers, timeouts, leg cost | 160 |
| `swish_check/transcript.rs` | fault tokens, markers, the boot claim | 280 |
| `swish_check/boot.rs` | the leg, the USB keyboard boot, `swish_check_boot` | 1,330 |
| `swish_check/launch.rs` | keystrokes, the reboot phase, the graphical launch, the `rg` walk | 480 |
| `swish_check/gauge.rs` | unchanged | 381 |

Tests go beside what they test as a `tests.rs` child, as §274 (the filesystem-protocol crate root
splits into its module files) ruled, with no test body changed. Thirteen test the gauge filter and
join `gauge/tests.rs`. Five check the script table itself, such as
`every_spawnable_program_has_a_swish_check_line`, and go to `script/tests.rs`, so `script.rs` holds
rows and nothing else. The rest go to `verdict`, `transcript` and `launch`. No file is over 1,500.

`swish_check_boot` moves whole. Decomposing it is a refactor that only `script/swish-check` on
three architectures could prove, so it is follow-on work, as the block says.

## The fork

**Does the script table get its own module?** The block asked it. The index precedent above
leaves only A: under B the rows would stay in the parent, which may no longer hold them. The fork is
kept here as it was raised.

| option | what it costs |
|---|---|
| A. Yes: `script.rs` holds `Line`, `line`, `keys` and all three scripts | A lane adding a check edits a file of rows. The harness reads them through `use super::*`. `script.rs` starts near 1,280 lines. |
| B. No: the scripts stay in the parent beside the entry | The parent starts near 1,460 lines, and every check-line edit touches the file that also holds the entry and the probes. |

Recommended: A. The rows are data, and the harness is the code that reads them. Milestone 885 (the
shell's test roles become scripts the shipping shell runs) already names this table as the shape
for its own assertions. A file of rows is the reader's whole task when adding a line, which
`notes/adding-a-program.md` asks of every new program. It holds at equal cost, so it is not a
recommendation about effort.

Both options leave one fact unchanged: the table will cross §266 (a Rust source file stays under
2,000 lines) on its own. It grew 95 lines in the last week and 694 in 18 days, so it has between
three and eight weeks before a further split. Under A that split happens inside `script/`, by
topic, without touching the harness. Under B it means cutting the parent again.

How this tree and others handled the same question:

| where | the table | where it lives |
|---|---|---|
| milestone 842 (the grant-plan crate root is split along its seams), §272 (the grant-plan crate root splits into private modules) fork 2 | the `programs!` table, edited once per new program | its own module, `program.rs` |
| milestone 844 (the shell program is split along its seams), §273 question 1 | the six shell witness roles | their own module, then scripts under milestone 885 |
| Rust's `compiletest` (from memory) | UI test cases | files under `tests/ui/`, apart from the harness in `src/tools/compiletest/` |
| LLVM `lit` (from memory) | `RUN:` and `CHECK:` lines | in each test file, apart from the runner |

Turning the table into a data file the harness reads at run time is the step milestone 885 takes
for the witnesses. Here it would lose the doc links and the rationale comments on each row, and it
is a design change, not a move. It is not offered as an option here.

B was the cut less `script.rs`, with the parent carrying the table. The index precedent rules
that out.

## What the move costs beyond moving code

- Visibility. Private items read across modules become `pub(super)`, as in §271 and §273 (the
  shell program becomes a directory of modules). Each child opens with `use super::*;` and the
  parent globs each child, so no path outside the directory changes.
- The falsification sweep. `script/falsifications` sets `SWISH_CHECK_SOURCE` to the one file. It
  must walk `xtask/src/swish_check.rs` and the directory, or the two records under
  `swish_check_leg` and `swish_check_boot` go unswept and nothing fails. Both functions move to
  `boot`, so the patch names follow the module path, to
  `swish_check.boot.swish_check_leg.patch` and `swish_check.boot.swish_check_boot.patch`.
  Milestone 843 (the system-initializer crate root is split along its seams) regenerates the same
  two patches, and whichever lands second carries both changes.
- The ratchets. `design/file-length-baseline.tsv` drops its row. The three listed comment blocks
  move with their rows, under the amendment for a split to §267 (a comment states the constraint
  as it is now).
- Citations. Five prose files cite `swish_check.rs:NNNN`. Each becomes a symbol under §277 (cite a
  heading or a symbol, never a line number).
- Proof. `cargo xtask`'s host tests, `script/swish-check` on aarch64, riscv64 and x86_64, the
  falsification replay of both records, and `script/lint`.

## What it does not decide

No public crate API, wire format or system call changes. `xtask` is a binary and `swish_check` is
`pub(crate)`. Every module name ships marked `Name: provisional`, deferred to calef's
name-ratification pass.
