---
status: NOT-STARTED
raised: 2026-10-11
milestone_dependencies: none
decision_dependencies: 266
machine_requirements: none
specific_machine: none
needs_person: no
---
# 881. The kernel benchmark file is split along its seams

*(The title and slug are provisional.)*

`kernel/src/bench.rs` is 2,110 lines at `396187b0b`. §266 (a
Rust source file stays under 2,000 lines) sets the ceiling. Milestone 840 (the scheduler file is
split along its seams) is the model. This file is compiled only under `--features bench`, and its
one entry point, `bench::run`, never returns.

The cut and every module name below are calef's call. A lane writes them up as a
`status: PROPOSED` file in `design/decisions/` before it moves a line, and its pull request
carries `needs-architect`.

Reuse: not applicable; this moves code and adds none. The kernel already holds two benchmark
siblings under their own features, `network_bench.rs` and `storage_bench.rs`.

## What is in the file

Measured on `396187b0b` from the file's own top-level items. Ranges are approximate at the edges,
where a doc comment belongs to the item below it.

| lines | count | what |
|---|---|---|
| 1 to 63 | 63 | module docs, iteration counts, `WARMUP`, `timed` |
| 64 to 129 | 66 | `run`: every row, in a fixed order |
| 130 to 280 | 151 | `yield_switch` and the three x86_64 TSS port-map rows |
| 281 to 504 | 224 | kernel-thread IPC rows: `ipc_rtt`, `relay_rtt`, `call_reply`, `broker_rtt`, `spawn_reap` |
| 505 to 652 | 148 | `map_new` |
| 653 to 786 | 134 | `cycles_per_tick`, its per-architecture probes, `rfence_self` |
| 787 to 924 | 138 | EL0 roles, `null_syscall_el0`, `ctx_switch_el0`, `ipc_rtt_el0` |
| 925 to 1060 | 136 | milestone 134's tier A set-up and E1, `ipc_thread_scaling` |
| 1061 to 1297 | 237 | E4, `app_displacement`, and its `Racy` buffer |
| 1298 to 1451 | 154 | `sink_throughput`, `map_el0`, `spawn_el0` |
| 1452 to 1497 | 46 | `coremark_compute` |
| 1498 to 1867 | 370 | `fs_read`, `fs_throughput`, `fs_walk`, `fs_walk_bound`, `rg_search` and their readers |
| 1868 to 2110 | 243 | multi-hart throughput: `tp_*`, `tc_*`, `busy`, `smp_throughput` |

E1 and E4 are the names milestone 134 (the register of measures) gave two experiments.

The file holds no test, no Kani harness and no `cfg(feature = "bench")` of its own. Its one gate is
at `kernel/src/lib.rs:48`, on the `mod bench;` line, and every submodule inherits it. The other 76
`feature = "bench"` sites across 26 kernel files are boot-path call sites and no-op twins, and none
of them names anything inside the module but `bench::run`.

Inside, the `cfg` variants are per architecture. `CYCLE_PROBE_MEANING`, `cycle_probe_window_ticks`
and `cycle_probe_delta` each come in two or three, and so does `TCG_VIRT_CNTFRQ_HZ`. Each set moves
as one. Two feature probes, `fastpath_pad` and `ipc_stack_depth`, sit inside `run` and stay there.

## The verdict: split by family, as child modules

The tests-only axis does not apply: there are no tests. The question is whether to split at all,
and how far.

This is not one type that should stay whole. It is a list of free functions, one per row, joined
by `run`'s order and a few shared helpers: `timed`, `WARMUP`, `real_single_hart_or_skip` and
`busy`. Each family has its own constants and its own helpers. That is a seam.

The smallest move would take one family out. Moving the multi-hart set alone leaves about 1,500
lines. But the file grew 262 lines in 17 days, and it would cross 2,000 again. A split by family
gives each new row an obvious home, and it leaves `bench.rs` as the order of the run, which is what
a reader opens it for.

Child modules, not siblings like `network_bench`. A child reaches `timed` and `WARMUP` as they are,
private. As siblings they would become `pub(crate)`.

## A proposed cut

Every name here is provisional. The modules are private to `bench`.

| module, provisional | from the table | about |
|---|---|---|
| `bench` (`bench.rs`) | 1 to 129, `real_single_hart_or_skip` | 180 |
| `bench::kernel_thread` | 130 to 652 | 520 |
| `bench::cycles` | 653 to 786, 1452 to 1497 | 180 |
| `bench::user_mode` | 787 to 924, 1298 to 1451 | 290 |
| `bench::scaling` | 925 to 1297, 1868 to 2110 | 610 |
| `bench::filesystem` | 1498 to 1867 | 370 |

`bench::scaling` holds E1 and E4 with the multi-hart rows because E1 reuses the multi-hart batch,
`tp_batch` and `tp_best`, and E4 uses its `busy`. The parent ends near 180 lines, and no new file
is near 1,500.

## What an architect has to rule

1. Split by family, or move one family out. The recommendation is by family, for the reason above.
   The one-family move is less work, so this recommendation is not about effort; it holds at equal
   cost.
2. Child modules of `bench`, or siblings like `network_bench`. The recommendation is children,
   because nothing widens. That holds at equal cost.
3. Every module name in the table.

No public crate API, wire format or syscall changes under any answer. `bench::run` keeps its path.

## What moving the code breaks

Found with `grep` at `396187b0b`.

- Row names are string literals. `xtask/src/bench.rs` parses `bench: <name>` and `bench-probe:`
  lines, and `bench/baseline-*.txt` is keyed on row names, so neither keys on the path. `run`'s
  order must not change, because the icount numbers depend on what ran first.
- Codegen units are partitioned by module, so a split can change inlining. That is from memory, not
  read. The module's own docs say icount numbers shift across binaries for unrelated changes. Run
  `script/bench --check` before and after; a shift past the 10% tripwire is re-saved with `--why`.
- `script/fastpath-footprint` matches mangled names under `kernel5sched` and `kernel7syscall`, not
  under `bench`. No script or helper keys on a `bench` symbol. Its numbers should not move.
- No falsification patch diffs this file. `bench/radon-2026-10-05/slot64.patch` names it in a
  comment and patches a fixture.
- Two citations of the form `bench.rs:NNNN` remain, in milestone 101 (the L4 calibration, read from
  the IPC number that pays for the trap) and milestone 811 (the boot services leave the kernel).
  Both have already drifted from the line they meant. 46 tracked files name the path in all.
- Comments in the three `pmu.rs` files and in `smp.rs` name `bench::cycles_per_tick` and
  `bench::real_single_hart_or_skip`. The first gets a new path under this cut.
- `design/file-length-baseline.tsv` lists the file at 2,110; that row goes in the same change.
  `design/comment-block-baseline.tsv` keys two rows on this path, and both blocks move.
  `bench.rs:589` (64 lines, the doc of `cycles_per_tick`) goes with its function into
  `bench/cycles.rs`. `bench.rs:1868` (42 lines, the plain comment opening the multi-hart
  throughput section) lands in `bench/scaling.rs`. The ratchet admits no row on a path the merge
  base gave none, and a block in a new file meets the 40-line cap outright. So it fails both
  unless each is cut to 40 lines in the same change. The alternative is a fix to the ratchet,
  letting a row move with its block, ruled separately.
- `notes/unsafe-obligations.md` counts `Racy`'s `unsafe impl Sync` by kind, not by file, so moving
  it changes no count. The note names `kernel/src/bench.rs`.
- `script/icount`'s comment says `feature = "icount"` appears in `bench.rs`. At `396187b0b` it does
  not; the comment is stale already.
- 6 first-parent merges in the 14 days to 2026-10-11 touched it.
- Pull request #1892 was in flight at `396187b0b` and does not touch this file at its head
  `43d8c5d`; if it changes the file before it merges, re-measure every range here.

## Done when

1. `kernel/src/bench.rs` is under 2,000 lines, and no new file is over 1,500.
2. No Rust file outside `kernel/src/bench.rs` and its new children changes, except the comments
   that name a moved function.
3. `script/bench` on aarch64, riscv64 and x86_64 prints the same rows in the same order, and
   `--check` passes, or each re-saved floor carries a `--why`.
4. `script/fastpath-footprint` is unchanged on each architecture.
5. Each new module carries a `//! Name:` block marked provisional.

## Index row

`kernel/src/bench.rs` is 2,110 lines and grew 262 in 17 days. It is a list of benchmark rows in
families, joined only by the order `run` calls them. This moves each family into a private child
module and leaves `bench.rs` as the run order. The cut and every name are an architect's call.
