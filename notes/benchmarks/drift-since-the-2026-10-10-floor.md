# Drift since the 2026-10-10 aarch64 floor: four merges, and a preemption tick that doubles one of them

*An appendix to [`notes/benchmarks.md`](../benchmarks.md). Name: provisional (lane
`bench/aarch64-drift-since-1010`, 2026-10-19 UTC); [the naming record](README.md) holds the ratified
ones.*

## The question

By the 2026-10-19 nightly bump, `main` read 1.5% to 4.2% over the aarch64 floor saved on 2026-10-10
on eleven rows, with the compiler held fixed. Which commits raised which rows, by how much, and was
each cost meant?

Everything here is aarch64 icount on QEMU 11.1.2 and `nightly-2026-10-18`, the floor's commit
`9f60ba737` against `main` at `5635cb7da`. Each count is bit-identical run to run, and the floor
commit reproduces its saved rows to within one tick. A tick is 16 guest instructions on aarch64
([icount-tick-scales](icount-tick-scales.md)). The kernel is the opt-level 0 build the gate
measures, so every cost below is a debug-build cost. Release costs are smaller and were not measured.

## What the floor sees: four merges

Bisected over `main`'s 469 first-parent merges, finding every step on `ipc_rtt`, `relay_rtt`,
`spawn_el0` and `ctx_switch` rather than only the first. Rows in percent of the merge's parent:

| merge | `yield_switch` | `ipc_rtt` | `relay_rtt` | `call_reply` | `broker_rtt` | `null_syscall` | `ctx_switch` | `ipc_rtt_el0` | `spawn_el0` |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| #1892, milestone 812 (threads in one address space) | +1.22 | +2.04 | +0.71 | +0.71 | +0.71 | +1.49 | +1.19 | +1.53 | +3.51 |
| #2025, `schedule` split | +0.45 | **-3.24** | +0.23 | +0.22 | +0.22 | -0.09 | +0.41 | -0.47 | +0.12 |
| #2188, a thread's name read in one step | +0.82 | **+4.87** | +1.71 | +1.23 | +1.26 | +0.09 | +0.70 | +2.54 | +0.24 |
| #2331, `SUSPEND` and `RESUME` | +0.29 | +0.18 | +0.18 | +0.14 | +0.14 | 0.00 | +0.24 | -0.57 | -0.09 |

These four account for every row in the table above to within 0.2%. The rest of the 469 merges
move `ipc_rtt` by at most one tick. `coremark`, `map_new` and `spawn_reap` are flat.
`ipc_rtt_el0` moves at almost every merge that touches the build, by up to 0.5% either way.

The `ipc_rtt` column cannot be right as costs: a refactor its own commit calls "no behavior change"
does not make a round trip 3.2% cheaper.

## Why `ipc_rtt` steps twice as far as its code moves

`ipc_rtt` is a single-hart ping-pong between two kernel threads. Each receive either finds the
message already there or blocks for it, and the loop settles into one of those two modes.
A preemption tick (`TICK_HZ` 100, one every 10M instructions, about two inside the window) can
switch the loop from one mode to the other for the rest of the run. So where the ticks land
decides the count, and any change to the instruction count before or inside the window moves
where they land.

The evidence is two measurements. A QEMU plugin counting executed instructions per translation
block (deterministic: two runs of one commit match block for block) shows `ipc_receive`'s code
identical across a step while the number of receives taking the blocking path changes. Across
the whole run that number moves +8,327 at #2188, -6,403 at #2025 and +5,876 at #1892's process
commit. And rebuilding with `TICK_HZ` 1, so no tick lands in a window, removes the effect:

- #2025's -36,215 on `ipc_rtt` becomes **+2,500**.
- A crate move with no kernel change (#1968), which reads -11,240 on `ipc_rtt_el0` and -1,434 on
  `spawn_el0` with ticks, reads **0** on every row.
- Floor to tip, `ipc_rtt` is +2.33% without the tick against +3.72% with it, and `ipc_rtt_el0`
  +2.72% against +4.19%. Every other row reads the same both ways to within 0.1%, except
  `spawn_reap` (+1.85% against +0.38%).

So the tick-free numbers below are the costs, and the gap between the two `ipc_rtt` figures is
which way two ticks happened to fall.

## The costs, commit by commit, with no tick in the window

`ipc_rtt` ticks per 1,000 round trips, and the rows each commit moved most:

| commit | what it adds | `ipc_rtt` | also | meant? |
|---|---|---:|---|---|
| `4eeb67dc3` (#1892) | `TPIDR_EL0` saved and installed at every switch | +6,000 (+0.54%) | `yield_switch` +1.08%, `ctx_switch` +1.00% | yes, §269 (how threads share a process) fork 4. About 48 instructions a switch, because `TPIDR_EL0.get()` and `.set()` are out-of-line calls at opt-level 0 |
| `3d0f1543d` (#1892) | the futex | +750 (+0.07%) | | yes |
| `4a8adf510` (#1892) | threads join a process | +750 (+0.07%) | `spawn_el0` +4.11%, `null_syscall` +1.40%, `spawn_reap` +0.88% | yes: spawn does more, and `syscall::dispatch` has new arms, which cost every syscall at opt-level 0 |
| `44e1e0b59` (#2025) | `schedule` split into `choose_next` and `switch_operands` | +2,500 (+0.22%) | `yield_switch` +0.45%, `ctx_switch` +0.45% | a refactor; its commit records the debug-build cost and inlined both helpers to hold it there |
| `642db52f6` (#2188) | `read_current_thread` masks IRQs around the per-CPU load | **+13,875 (+1.24%)** | `relay_rtt` +1.72%, `ipc_rtt_el0` +1.45%, `sink_throughput` +1.41% | the read, yes (it closes a migration race); the cost, no. Its commit sized it on a release build at "the `DAIF` read and a branch". At opt-level 0 it is five calls on every IPC |
| `f7b50a1d4`, `83b761954` (#2331) | the suspension check in `schedule` | +2,000 (+0.18%) | | yes; the first cut cost +40,375 and the second commit took it back |

These sum to the whole tick-free drift on `yield_switch`, `ipc_rtt`, `relay_rtt`, `call_reply`,
`broker_rtt`, `null_syscall` and `ctx_switch`, each to within two ticks. What is left over is
`ipc_rtt_el0` +17,463 (+0.16%), `sink_throughput` +8,750 (+0.20%), `spawn_reap` +1,386 (+0.61%),
and `spawn_el0` -14,241 (-1.01%), which commits after these four took back. Those four remainders
are not attributed.

## The one fixed here

`read_current_thread` on aarch64 is one `asm!` block (`mrs daif`, `msr daifset`, `mrs tpidr_el1`,
`ldr`, `msr daif`), with the same masking and the same restore. Measured on `main` at `7e56fb61a`:

| row | tick-free | with the tick |
|---|---:|---:|
| `ipc_rtt` | -33,000 (-2.91%) | -77,340 (-6.81%) |
| `relay_rtt` | -88,000 (-4.01%) | -87,661 (-4.00%) |
| `call_reply` | -33,001 (-2.90%) | -33,237 (-2.91%) |
| `broker_rtt` | -65,999 (-2.90%) | -66,183 (-2.91%) |
| `yield_switch` | -22,000 (-1.93%) | -22,579 (-1.98%) |
| `ctx_switch` | -54,999 (-1.79%) | -55,067 (-1.79%) |
| `ipc_rtt_el0` | -378,966 (-3.40%) | -335,513 (-2.99%) |
| `sink_throughput` | -151,249 (-3.30%) | -151,740 (-3.31%) |
| `spawn_el0` | -5,500 (-0.38%) | -7,297 (-0.50%) |

It takes back more than #2188 added. The read it replaced, `cpu::current().current.load()`, was
already a chain of calls at opt-level 0.

## BUGS

- The `ipc_rtt` family is still bimodal under the tick. Milestone 939 (the icount bench runs
  without a preemption tick in its windows):
  [IPC rows run without a preemption tick](../../design/roadmap/0939-ipc-rows-run-without-a-preemption-tick.md).
  Until then, a step on `ipc_rtt` or `ipc_rtt_el0` is not a cost until it survives `TICK_HZ` 1.
- `TPIDR_EL0`'s hand-over pays the same opt-level 0 toll as the read fixed here (about 48
  instructions a switch for three). It was named, not measured, as a candidate for the same fix.
- riscv64 and x86_64 were not bisected. Their `read_current_thread` is already one load, so #2188
  should not have cost them the same; nothing here measured it.
