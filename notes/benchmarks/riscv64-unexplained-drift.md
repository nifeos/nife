# riscv64's unexplained drift: three causes on every ISA, a dozen merges and a compiler term that was the tick

*An appendix to [`notes/benchmarks.md`](../benchmarks.md). Name: provisional (lane
`lane/riscv64-benchmark-drift`, 2026-11-03 UTC); [the naming record](README.md) holds the ratified
ones.*

## The question

The weekly chart of unexplained drift on `main` (`notes/project-metrics/unexplained-drift.svg`,
from `helpers/baseline_drift.py`) read riscv64 at 0.4% in 2026W39, then 2.6, 6.9, 6.0, 3.7, 4.3
and 4.3 in 2026W45. Each figure is the largest drift on one row, since the 2026-08-04 anchor, that
no line in [`bench/attributions.tsv`](../../bench/attributions.tsv) accounts for. Which merges
raised it, what does each cost, and was it meant? aarch64 and x86_64 also read above zero, so the
first test was whether one cause moves all three.

The chart's riscv64 figure came from three rows in turn:

| weeks | row | where the unexplained part sat |
|---|---|---|
| 2026W40 | `spawn_el0` | `main`'s gap to the 2026-10-03 floor |
| 2026W41 to W43 | `call_reply`, `broker_rtt` | +2.00% absorbed by the 2026-10-03 save, +4.49% by the 2026-10-08 save |
| 2026W44, W45 | `map_el0` | +2.56% at the second 2026-10-22 save, +1.37% in `main`'s gap to the floor |

Each save absorbed drift on `main` that its `# why:` line named and did not separate. The
2026-10-22 saves on `spawn_el0` (+4.22%) sat behind the 2026W42 and W43 figures as well.

## How it was measured

riscv64 icount, the -O0 bench kernel the gate measures, QEMU 11.1.2, one hart. Four windows of
first-parent merges, each between two commits whose readings reproduce what the floors or the
merge queue recorded:

| window | from | to | merges | nightly held |
|---|---|---|---:|---|
| C | `52d21046e` (2026-09-26 floor) | `40ac2d5f0` | 122 | `nightly-2026-10-22` |
| B | `228e8024a` (2026-10-03 floor) | `9c9d6a25f` | 337 | `nightly-2026-10-22` |
| D | `bc06c6a23` (2026-10-10 floor) | `f9a29ca79` | 593 | `nightly-2026-10-22` |
| A | `8c221d5be` (2026W43 reading) | `7ea93b038` (2026W44 reading) | 307 | `nightly-2026-11-02` |

`228e8024a` reads its floor to the tick and `52d21046e` within 16 ticks of `call_reply`'s.
`8c221d5be` and `7ea93b038` read the merge queue's 2026W43 and W44 rows exactly on their own
pinned nightlies, and window A's held nightly moves only `null_syscall` (+0.30%, the restamp term
the 2026-11-02 floor already carries). So the compiler term in each window is zero to within a
tick.

Each point is one run:

```
RUSTUP_TOOLCHAIN=nightly-2026-10-22 helpers/qemu-bounded.sh 1500 cargo xtask bench --riscv
```

in a detached worktree at the commit. Commits before milestone 939 (the icount bench runs without
a preemption tick in its windows) ran with `TICK_HZ` set to 1 in
`kernel/src/arch/riscv64/timer.rs`, so the numbers are costs and not the tick's phase.
Commits before `964bd5a88` ran with its one-line `NIFE_DISK` fix applied. Every window was
sampled at 15 to 30 points, then each interval whose ends differed was split again until one merge
held the step. A step under 0.1% on every row was not chased.

The same commands with `cargo xtask bench --x86` and with no flag measured x86_64 and aarch64 on
the merges that move rows on all three.

Mechanisms come from a QEMU TCG plugin that counts executed instructions per translation block,
summed per kernel symbol from the ELF, for a merge and its parent. The plugin is not in the tree.
It reads the whole run, so a diff also carries boot work outside the windows (tick handling,
idle), and only the functions on a row's path are evidence.

## Three causes move every ISA

Percent of the merge's parent, tick-free:

| merge | row | riscv64 | aarch64 | x86_64 |
|---|---|---:|---:|---:|
| #1360, milestone 126 (the `procps` package) | `call_reply` | +1.65 | +1.76 | +1.77 |
| | `yield_switch` | +2.87 | +3.38 | +2.88 |
| #1617, milestones 754 and 761 | `call_reply` | +4.15 | +4.02 | +3.59 |
| | `ctx_switch` | +3.15 | +3.25 | |
| | `spawn_el0` | +4.37 | +3.92 | |
| #2734, the kernel lock becomes `crates/spin_lock` | `call_reply` | -2.44 | -2.26 | -1.72 |
| | `spawn_reap` | -3.50 | -3.05 | -2.73 |

**#1360** is the context-switch counter
[drift-since-the-2026-09-26-floors](drift-since-the-2026-09-26-floors.md) found, priced there on
the switch rows. It costs the IPC rows too, because a call and its reply switch twice.
Milestone 629 (the context-switch statistic stops costing the switch path) took most of it back
on 2026-10-03.

**#1617** is milestone 761 (capability lookup off the global lock). The profile of
`ccea3ba7f` against `1b4e13ef3` finds three costs, in executed instructions over the run.
`IpcTables`' lock splits in two, `lock` -17.2M and `lock_masked` +18.5M. Each capability lookup
takes the table's own lock: `lock_masked` on `CapabilityTable` +3.2M, `lock_found` +1.2M, the
lookup closures and slot reads about +2.9M net. And `set_current` stores the running thread's
table pointer, +2.1M.
[riscv64-spawn-reap-drift](riscv64-spawn-reap-drift.md) priced the same merge on `yield_switch` and
`spawn_reap`; the IPC rows pay the lookup's lock on top, since every call resolves an endpoint
capability. `0ec577bde` later inlined `lock_masked`.

**#2734** replaces `spin`'s `Mutex`, which wraps a `SpinMutex`, with one layer marked
`inline(always)`. At -O0 every guard's drop and deref went through two out-of-line layers: on the
`IpcTables` lock alone the run lost 13.8M instructions of `spin` guard code and gained 7.4M of
`spin_lock`'s. It is the broad drop in the 2026W43 to W44 readings on all three ISAs.

#1620 (milestone 758 (the IPC fast paths shrink back inside their band)) is the counterexample: +0.25% on riscv64
`call_reply`, -0.39% on aarch64 and -2.67% on x86_64. On riscv64 it takes `IpcTables` more often
(+6.3% of the run's instructions in its `lock`).

## The riscv64 merges, by window

Percent of the parent, tick-free, on the rows the chart read. Lines already in the file before
this note are left out (#1360's switch rows, #2565's `spawn_reap` and `yield_switch`).

| merge | `call_reply` | `relay_rtt` | `ctx_switch` | `sink_throughput` | `map_el0` | `spawn_el0` |
|---|---:|---:|---:|---:|---:|---:|
| C: #1360, milestone 126 (the `procps` package) | +1.65 | +1.56 | | +1.03 | +0.18 | -0.57 |
| C: #1351, milestone 151 (notification objects) | +0.23 | +0.40 | | +0.22 | | +0.20 |
| B: #1493, milestone 632 (graphics launched from the prompt) | +0.10 | | | | | |
| B: #1517, a region reclaims its out-of-order holes | | | | | | +1.64 |
| B: #1620, milestone 758 | +0.25 | +0.23 | +0.16 | +0.22 | +0.16 | +0.18 |
| B: #1614, fatal risk 4 (`null_syscall` under load) | | | -0.53 | -0.32 | -0.19 | +0.32 |
| B: #1617, milestones 754 and 761 | +4.15 | +2.35 | +3.15 | -1.77 | (save) | (save) |
| D: #1892, milestone 812 (threads in one address space) | +0.37 | +0.37 | +0.62 | +0.27 | +0.50 | +4.09 |
| D: #2025, `schedule` split | +0.22 | +0.24 | +0.46 | +0.15 | | |
| D: #2148, split parents to zero (6) | -0.76 | -0.81 | -0.59 | -0.69 | -0.29 | -0.36 |
| D: #2188, a thread's name read in one step | -1.81 | -2.55 | -1.11 | -2.07 | | -0.26 |
| D: #2331, `SUSPEND` and `RESUME` | +0.17 | +0.20 | +0.30 | +0.12 | | |
| D: #2454, TLB maintenance batched | | | | | | +0.60 |
| A: #2734, `crates/spin_lock` | -2.44 | -2.09 | -1.44 | -2.08 | -1.53 | -2.31 |
| A: #2933, milestone 961 step 1 | | | | +4.92 | +0.04 | -1.90 |
| A: #2965, milestone 969 | | | | | +2.89 | +1.15 |

"(save)" marks a row the 2026-10-08 save already explains whole. #1614 also takes 7.99% off
`null_syscall`, its own purpose. #1892 adds 1.20% to it.

#1351's profile: the thread's wait state becomes a `Wait` enum that also holds a notification
wait (`try_wake` +11%), and the IPC path takes `IpcTables` more often (+4.3% in its `lock`).

#2933, milestone 961 (the kernel's special boots become commands typed at the production shell)
step 1, moves the sink and map rows out of `os_primitives_benchmarker` into programs of their own.
The new `sink_consumer` unpacks each message into a buffer and counts its bytes inside the timed
loop, where the old one only received, so the row now prices what a real consumer does. The
benchmarker lost about 200 lines, so its image spawns cheaper. aarch64 reads the same: +5.16% and
-1.81%.

## `map_el0`: a compiler term that was the tick, and #2965

Two things make up nearly all of 2026W45's 4.27 points on `map_el0`.

**A compiler term that did not happen, about 3.0 points.** The 2026-10-22 nightly save recorded
`map_el0` -2.927% as the nightly-2026-10-22 compiler term, and the file carried it as an
explained step. The tree that A/B measured, `f9a29ca79`, reads 47,493 or 47,494 on both
`nightly-2026-10-21` and `nightly-2026-10-22`, with the tick and without it. The term is +0.002%.
The 46,103 the A/B saw was the preemption tick's other mode, which the second 2026-10-22 save's
`# why:` line already said. The explained improvement never happened, and dividing it out
inflated the row's unexplained figure by 1/0.97073, about 3.0 points. The line now carries the
measured term, and the save's step on this row is the tick's phase, reversed by the next save.

#2965, +2.89%, is milestone 969 (no frame is writable and executable in one address space).
`map_el0` maps read-only, so the new writable-and-executable check returns before it scans. The
cost is in `MappingHold::file`, which now returns where it filed the record. Its scan of the head
log page for a tombstone changed from `iter_mut().find` to `iter().position`, +117k instructions
over the run (+9.5% on that scan). The `Recorded` and `Result` plumbing adds about 36k more. aarch64 reads +2.81%.

That scan is the larger story. Deleting it on `main` at `7f54880c4`, and nothing else, takes
`map_el0` from 47,891 to 30,785 (-35.7%), about 3,400 of a map's 9,600 instructions; every other
row holds within a tick except `spawn_el0` (-0.24%). The scan runs on every map, whether or not
the page holds a tombstone.
[a-mapping-record-is-filed-without-a-scan](../../design/roadmap/1036-a-mapping-record-is-filed-without-a-scan.md)
proposes a tombstone count per log page.

Of the 4.27 points: the false term +3.02, #2965 +2.88, #1892 +0.50, #1360 +0.18, #2148 -0.29,
#2734 -1.52. With the lines the row reads -0.49%.

## What each cause accounts for

Points of the chart's riscv64 figure, by week, each the change in the row's unexplained drift
when that merge's lines are removed:

| week, row | before | after | the causes |
|---|---:|---:|---|
| 2026W40 `spawn_el0` | 2.65 | 0.87 | #1517 +1.65, #1614 +0.32, #1351 +0.20, #1620 +0.18, #1360 -0.57 |
| 2026W41 `call_reply` | 6.86 | -0.01 | #1617 +4.15, #1360 +1.65, #1892 +0.37, #1620 +0.25, #1351 +0.23, #1493 +0.10 |
| 2026W42 `call_reply` | 4.56 | 0.01 | #1617 +4.15, #1360 +1.65, #1892 +0.37, #1620 +0.25, #1351 +0.23, #2025 +0.22, #2331 +0.17, #1493 +0.10, #2148 -0.76, #2188 -1.81 |
| 2026W42 `spawn_el0` | 3.77 | 0.69 | #1892 +4.12, #1351 +0.20, #1360 -0.57, #2148 -0.36, #2188 -0.26 |
| 2026W45 `map_el0` | 4.27 | -0.49 | the false term +3.02, #2965 +2.88, #1892 +0.50, #1360 +0.18, #2148 -0.29, #2734 -1.52 |

With every line, riscv64's figure reads 0.38, 0.87, 1.43, 0.80, 0.12, 0.11 and 0.11 from 2026W39
to W45. `python3 helpers/baseline_drift.py` restates every week from the file; the committed
`notes/project-metrics/unexplained-drift.csv` changes past weeks only through
`script/metrics --backfill-measure unexplained-drift`.

The lines for improvements raise other architectures' figures, because an explained improvement no
longer cancels an unexplained cost. x86_64 reads 2.85 in 2026W45 against 0.98 before
(`spawn_reap`), and 5.80 in 2026W41 against 6.80. aarch64 falls or holds in every week (1.24 in
2026W45, at most 3.07).

## Which costs were meant

- **Chosen:** #1360 (the counter `free` and `vmstat` read; milestone 629 made it cheap), #1617
  (milestone 761's design; [riscv64-spawn-reap-drift](riscv64-spawn-reap-drift.md) fixed its lock
  split) and #1892 (milestone 812's process join). Also #1351 (notification waits), #2965 (claim
  11 of the confinement record) and #2933 (the sink row now pays for unpacking).
- **A fix that costs:** #1517 reclaims a region's holes so the riscv64 progenitor stops running
  out of memory; spawning pays for the reclaim.
- **Incidental:** #1620 costs riscv64 what it saves elsewhere. #2965's `position` scan is
  dearer than the `find` it replaced, a side effect of returning the slot. #2454's +0.60% on
  `spawn_el0` was not examined.
- **Savings:** #2734, #2188, #2148 and #1614.

## What stays unexplained

- riscv64 `ipc_rtt` reads +1.43% in 2026W41. #1525 (milestone 633 (an outside agent attacks the
  confinement claim)) moves it +3.07% tick-free and `sink_throughput` +0.87%, and nothing else.
  It clears a collected sender's `outgoing_cap` on the plain `RECV` path both rows take. The
  profile grows `Threads::get_mut` and `ipc_recv` by about 225k instructions over the run,
  against the 528k that `ipc_rtt`'s step needs. Part of the step is the ping-pong's mode, so #1525
  has no line here.
- riscv64 `spawn_el0` keeps +0.74% inside the 2026-10-03 save's step, in window C steps under 0.2%
  each, and `sink_throughput` at most +0.80%.
- [x86_64-unexplained-drift](x86_64-unexplained-drift.md) bisects x86_64's `spawn_reap` and
  `ipc_rtt`. Deleting #1525's one line there takes x86_64 `ipc_rtt` down 3.0% at three commits;
  the same test on riscv64 would settle whether its step costs instructions or only flips the mode.

## BUGS

- Recursive splitting finds a step only where two sampled points differ. Two equal and opposite
  steps inside one interval cancel unseen.
- `ipc_rtt` and `ipc_rtt_el0` are ping-pongs whose mode can flip with code that costs nothing, even
  without the tick. Their lines here are single readings and carry that doubt.
- The profiles count the whole run. A function's diff includes warmups and rows other than the one
  in question.
- Every cost here is a debug-build cost. Release builds were not measured.
