---
status: BUILT
raised: 2026-10-11
built: 2026-10-12
milestone_dependencies: none
decision_dependencies: 266, 282, 284
machine_requirements: none
specific_machine: none
needs_person: no
---
# 876. The kernel crate root is split along its seams

*(The title and slug are provisional.)*

`kernel/src/lib.rs` is 2,984 lines at `396187b0b`. It is the kernel crate root, and it holds the boot self-tests AGENTS.md calls the model. §266 (a
Rust source file stays under 2,000 lines) sets the ceiling. Milestone 840 (the scheduler file is
split along its seams) is the model, and milestone 843 (the system-initializer crate root is split
along its seams) is the nearest case, because most of this file is one function.

The cut and every module name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

Reuse: not applicable; this moves code and adds none.

Pull request #1892 (milestone 812 (`std::thread::spawn` runs real threads in one address space))
was in flight when this was written, and at its head `43d8c5df3` it does not touch this file. If it
changes this file before it merges, every line range below must be re-measured.

## The ratchet already binds

`design/file-length-baseline.tsv` lists this file at 2,984, which is its size today. Under §266 a
listed file may not grow, so the next lane that adds a line here must first take one out. 18
first-parent merges in the 14 days to 2026-10-11 touched it. That makes this split more urgent than
the file's size alone suggests.

## What is in the file

Measured on `396187b0b` from the file's own top-level items. Ranges are approximate at the edges.

| lines | count | what |
|---|---|---|
| 1 to 46 | 46 | crate docs and attributes |
| 47 to 184 | 138 | 46 `mod` declarations, the `_print` and `skip!` re-exports |
| 185 to 264 | 80 | `system_test_access`, the facade for the system-test image |
| 265 to 334 | 70 | `system_tests_main`, `run_test_suite`, `DTB`, `device_tree` |
| 335 to 2387 | 2,053 | `kernel_main`, one function (broken down below) |
| 2388 to 2475 | 88 | riscv64 timing helpers: `micros`, `micros_between`, `bytes_per_second`, `mode_note` |
| 2476 to 2515 | 40 | `image_for_virtio`, `interrupts_init`, `stack_top` |
| 2516 to 2585 | 70 | `riscv_hand_over` |
| 2586 to 2673 | 88 | `x86_hand_over` |
| 2674 to 2856 | 183 | `print_machine_description` |
| 2857 to 2984 | 128 | `mod tests`, 7 `#[test_case]` boot self-tests |

Inside `kernel_main`:

| lines | count | what |
|---|---|---|
| 335 to 424 | 90 | the per-CPU pointer, the console, the screen, aarch64's opening line |
| 425 to 1066 | 642 | the `x86_64` boot, a `cfg(target_arch)` block that ends in its hand-over |
| 1067 to 1960 | 894 | the riscv64 boot, the same shape |
| 1961 to 2387 | 427 | everything after, which only aarch64 reaches: arch, memory, heap, interrupts, the test suite, `icount`, `bench`, the hand-over |

The two blocks are 1,536 lines, more than half the file. Each takes `boot_info_pointer`, and the
riscv64 block also reads the `screen` local bound at line 391. Each ends in `sched::exit()` or a
`run()` that diverges.

## The verdict: split by feature

Moving the tests out does not work. They are 128 lines, and AGENTS.md points at this file for them.

Moving only the items after `kernel_main` does not work either. They total 469 lines, which leaves
the file at about 2,515.

The seams are the two per-architecture boots. Each is a self-contained part with its own helpers:
the riscv64 boot alone calls the four timing helpers and `riscv_hand_over`, and the `x86_64` boot
alone calls `x86_hand_over`. Each becomes a function in a child module of the crate root, taking
what it reads today. A child module reaches the crate root's private items, such as
`print_machine_description` and `run_test_suite`, so nothing widens.

This is a little more than a move, which milestone 843 would call a refactor. It is the smallest
kind: a block becomes a call, with one or two arguments and no shared state. Declaring each function
`-> !` lets the compiler prove that it still diverges. If some configuration does not, it returns
`()` and the call site keeps today's meaning.

## A proposed cut

Every name here is provisional.

| module, provisional | from the tables | about |
|---|---|---|
| `lib.rs` | everything not below | 1,230 |
| `x86_64_boot` | 425 to 1066, `x86_hand_over` | 750 |
| `riscv64_boot` | 1067 to 1960, 2388 to 2475, `riscv_hand_over` | 1,070 |

Each is declared in `lib.rs` as `#[cfg(target_arch = "...")] mod ...;`, with the `cfg` on the
declaration. `kernel_main` falls to about 520 lines.

## What an architect has to rule

1. Whether to extract the two boots as functions. The recommendation is yes: a reader of
   `kernel_main` then sees the shared path without 1,536 lines of the other two architectures. That
   reason holds at equal cost, so it is not about effort.
2. Whether the two modules live at the crate root or under `kernel/src/arch/<port>/`. The
   recommendation is the crate root. AGENTS.md rule 1 (all architecture-specific code lives under
   `kernel/src/arch/`) names assembly, `asm!` and system registers. `grep` finds no `asm!` in this
   file, and the only register reads (`CurrentEL`, lines 409, 2708 and 2920) are outside both
   blocks. The boots call `sched`, `user` and the drivers, so under `arch/` they would point the
   layering upward. `helpers/lint_arch_parity.py` also skips every file under `arch/`, so they
   would leave its check. These reasons hold at equal cost.
3. Whether `print_machine_description` and the boot self-tests stay in `lib.rs`. The
   recommendation is yes. `script/lint` reads that function from this path, and AGENTS.md names
   this file for the tests.
4. Every module name in the table.

No public crate API, wire format or syscall changes under any answer here.

## What it is not

The boots stay per-architecture. Bringing riscv64 and `x86_64` onto aarch64's shared path, as the
crate's header comment hopes, is a parity change, and not this milestone.

## What moving the code breaks

Found with `grep` at `396187b0b`. A lane has to carry each one.

- No falsification patch anywhere names this file, and no test here carries a `Falsification:`
  record. No Kani harness is in it.
- `design/file-length-baseline.tsv` lists it at 2,984. The split takes it under 2,000, so the row
  is removed in the same change. `design/comment-block-baseline.tsv` has no row for it.
- One `script/lint` check comes from milestone 267 (the milestone tour is three things wearing one
  name, and only one of them belongs in the kernel). It opens `kernel/src/lib.rs` and fails if
  `print_machine_description` is missing, which stays put under this cut.
- `helpers/lint_arch_parity.py` starts its walk at this file and follows `mod` lines, taking each
  module's architectures from the `cfg` on its declaration. A `cfg` placed only inside the new file
  would hold that file to all three ports.
- Doc comments in `xtask/src/swish_check.rs` (lines 1460 and 1465) say `x86_hand_over` is in this
  file. `xtask/src/uefi.rs:803`, `helpers/qemu-runner-x86_64.sh:136` and `script/lint:243` describe
  "`kernel_main`'s `x86_64` arm". Each goes stale and needs the new path.
- 12 citations of the form `lib.rs:NNNN` mean this file, all in the block of milestone 811 (the boot
  services leave the kernel: the kernel starts only the progenitor), which dates them to one commit.
  22 tracked files name the path.
- Two doc comments have drifted off their functions. Lines 2389 to 2408 document `interrupts_init`
  and `mode_note`, but they sit above `micros`, so rustdoc gives them to `micros`. A lane moves each
  to its owner.
- `kernel_main` is `#[unsafe(no_mangle)]`, so its symbol does not move. `script/fastpath-footprint`
  and `script/icount` key on no symbol here, and `helpers/cap_abbreviation.py` has no row.
- Each block is indented one level deeper than its function body will be, so `script/fmt` reflows
  it. Review the move with `git diff --color-moved --color-moved-ws=allow-indentation-change`.
- Codegen units follow modules, and the boot stack's deepest frame may change shape. That is from
  memory, not read; neither `Cargo.toml` sets `codegen-units` or `lto`. Measure it with
  `script/fastpath-footprint`, `script/icount`, `script/bench` and `script/stack-frame-check`.

## Done when

1. `kernel/src/lib.rs` is under 2,000 lines, no new file is over 1,500, and the file-length row is
   gone.
2. No Rust code outside `kernel/src/` changes, and no item's visibility widens. *(Amended
   2026-10-12 (UTC) by the lane: comments outside it that named `lib.rs` for a moved item are
   repointed, as "What moving the code breaks" already required for `xtask/src/swish_check.rs`.)*
3. `script/test` passes on aarch64, riscv64 and x86_64, with the 7 boot self-tests still at
   `crate::tests` and no test body changed. `script/swish-check` passes on all three. *(Amended
   2026-10-12 (UTC): "still in `lib.rs`" predates §282, under which they move to `tests.rs`.)*
4. `script/fastpath-footprint`, `script/icount` and `script/stack-frame-check` are unchanged, or
   each difference is explained. `script/bench` floors hold.
5. `script/lint` passes, its milestone 267 check and the arch-parity lint among them.
6. Each new module carries a `//! Name:` block marked provisional.

## What was built

Built on `17b23264e`, with `kernel_main` split by boot phase (calef's ruling, 2026-10-09 UTC,
and his "A" on #1963). The cut is
[§284 (the kernel crate root splits into child modules, one per boot)](../decisions/0284-the-kernel-crate-root-splits-into-child-modules-one-per-boot.md),
decided. Its questions 1 and 3 above were answered by those rulings and by §282 (when a file is
split, the parent becomes an index); calef answered question 2 on 2026-10-12 (UTC): the crate root,
with both boots temporary.

`kernel/src/lib.rs` went from 2,984 lines to 293, an index, so its file-length and parent-index
rows are gone. `kernel_main` went from 977 code lines to 59. Every name is provisional.

| file | lines | holds |
|---|---|---|
| `lib.rs` | 293 | crate docs, `mod` lines, `system_test_access`, re-exports |
| `boot.rs` | 432 | `kernel_main`, `DTB`, `device_tree`, the aarch64 bring-up, the test-suite hook, the measurement boots |
| `x86_64_boot.rs` | 768 | the `x86_64` boot's phases, `x86_hand_over` |
| `riscv64_boot.rs` | 1,093 | the riscv64 boot's phases, its timing helpers, `riscv_hand_over` |
| `milestone_tour.rs` | 260 | aarch64's tour, `image_for_virtio` |
| `machine_description.rs` | 206 | `print_machine_description`, `stack_top` |
| `tests.rs` | 124 | the seven boot self-tests |

One correction rode along, found by building the lint's feature matrix. The phase split had
gathered the three measurement tails into a `run_measurement_boot` returning `()`, which stopped
`soak_test`, `job_mix` and `lock_wait` kernels type-checking, and it had moved `bench::run()` out
from under the `allow(unreachable_code)` that covered the `x86_64` arm. `run_measurement_boot` is
now `-> !` in a build with a measurement feature and a no-op without, which lets both boots say
`-> !`; `x86_64_scheduler` carries the bench allows. The two drifted doc comments are back on
`interrupts_init` and `mode_note`.

## Follow-on

- **Decision.** calef ruled §284's question on 2026-10-12 (UTC), "We should treat them as
  temporary": the two boots stay at the crate root as each port's route to the shared path. Every
  module name is still his:
  `design/decisions/0284-the-kernel-crate-root-splits-into-child-modules-one-per-boot.md`.
- **Milestone 890.** Bringing riscv64 and `x86_64` onto the shared path, which retires the two boot
  files, as milestone 890 (riscv64 and x86_64 join the shared boot path):
  `design/roadmap/0890-riscv64-and-x86-64-join-the-shared-boot-path.md`.
- **Recorded.** AGENTS.md's Testing section names `kernel/src/lib.rs` for the boot self-tests,
  which are in `kernel/src/tests.rs` now. A developer lane does not edit AGENTS.md, so the
  pull request names it for the maintainer.
- **Refused.** Repointing the 12 citations of `lib.rs` by line in milestone 811's block. A developer
  lane edits no record under `design/` but its own block. The pull request lists each with the
  symbol it should name.
- **Done.** `kernel_main`'s row in `design/lint-baseline.tsv` (977 code lines) is removed, since it
  is 59 now, and criterion 4 of milestone 888 (a ratchet on function length) is marked done.

## Index row

`kernel/src/lib.rs` is 2,984 lines, exactly its §266 ceiling, so it can no longer grow by a line.
More than half of it is two `cfg(target_arch)` blocks inside `kernel_main`: the whole `x86_64` boot
and the whole riscv64 boot. This extracts each into a child module of the crate root, leaving the
boot self-tests and the machine description where they are. The cut and every name are an
architect's call.
