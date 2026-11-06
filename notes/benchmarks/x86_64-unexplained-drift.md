# x86_64's unexplained drift: the merges inside four saves, and the tick's phase

*An appendix to [`notes/benchmarks.md`](../benchmarks.md). Name: provisional (lane
`lane/x86_64-benchmark-drift`, 2026-11-04 UTC); [the naming record](README.md) holds the ratified
ones.*

## The question

The weekly chart of unexplained drift on `main` read x86_64 at 5.80% in 2026W41 and 2.85% in
2026W45, from `spawn_reap` and `ipc_rtt`. Four saves absorbed `main`'s drift on `spawn_reap`
without separating it: 2026-09-24 (+2.32%), 2026-09-26 (-1.74%), 2026-10-03 (+1.31% beyond
#1360's line) and 2026-10-22 (+0.89%). The 2026-10-10 save held +1.09% of `ipc_rtt`'s. Which
merges moved those rows, and by how much?

## How it was measured

x86_64 icount, the -O0 bench kernel, the pinned QEMU 11.1.2, one hart. Each point is one run of

```
RUSTUP_TOOLCHAIN=<held nightly> helpers/qemu-bounded.sh 2400 cargo xtask bench --x86
```

in a detached worktree at the commit, with `TICK_HERTZ` set to 1 in
`kernel/src/arch/x86_64/timer.rs` on trees before milestone 939 (the icount bench runs without a
preemption tick in its windows). The bisect measured every first-parent merge, 1,342 points in all:

| window | from | to | merges | nightly held |
|---|---|---|---:|---|
| 1 | `4b4929639` (2026-09-23 floor) | `5297ef8e8` | 132 | `nightly-2026-09-24` |
| 2 | `5297ef8e8` | `a91d41934` | 52 | `nightly-2026-09-25` |
| 3 | `a91d41934` | `228e8024a` | 168 | `nightly-2026-10-22` |
| 4 | `228e8024a` | `bc06c6a23` | 380 | `nightly-2026-10-22` |
| 5 | `bc06c6a23` | `b6233a3b8` | 607 | `nightly-2026-10-22` |

Run with the tick, all six window ends read their committed floor to the instruction, and the
2026W39 and W41 readings match the merge queue's. `a91d41934` reads the same count on
`nightly-2026-09-25` and `nightly-2026-10-22`, so the held compiler adds nothing in windows 3 to 5.
For the first two saves, one tree built with both nightlies gives the compiler term.

## What each save held

Percent of the previous floor. "Tick" is the change in the preemption tick's phase, the ratio of
the with-tick and tick-free readings at the save against the same ratio at the previous floor.

| save | row | floor | merges and terms | tick |
|---|---|---:|---|---:|
| 2026-09-24 | `spawn_reap` | +2.32 | #1087 +0.20, #1133 +0.25 | +1.87 |
| | `tss_iomap_lazy_nop` | +1.16 | #1133 +1.16 | 0.00 |
| | `ipc_rtt` | +0.42 | #1133 +0.40 | +0.02 |
| 2026-09-26 | `spawn_reap` | -1.74 | none | -1.74 |
| | `ipc_rtt` | -0.11 | `nightly-2026-09-25` -0.12 | 0.00 |
| 2026-10-03 | `spawn_reap` | +2.26 | #1360 +0.94, #1489 (the save) -0.32 | +1.62 |
| 2026-10-10 | `spawn_reap` | +0.05 | #1617 +2.11, #1620 -2.19, #1614 +1.38, #1852 +0.43, #1645 +0.18, #1692 -0.13 | -1.69 |
| | `ipc_rtt` | +0.93 | #1525 +3.09, #1617 +2.07, #1620 -2.19, #1852 +0.49, #1600 -0.22, #1494 +0.16 | -2.40 |
| 2026-10-22 | `spawn_reap` | +0.89 | #1892 +1.90, #2057 +0.56, #2188 -0.24, #2446 -0.30, #2565 (the save) -1.11 | 0.00 |
| | `ipc_rtt` | +1.04 | #2188 -1.66, #2565 -0.54, #1892 +0.52, #2331 +0.22, #2025 +0.17, #1997 +0.15, #2446 -0.18 | +2.45 |

Steps under 0.1% add at most 0.11% to any row in a window. The switch rows move with the same
merges: #1133, #1892, #1997 (+1.10% on `tss_iomap_lazy_switch`), #2025, #2188, #2331, and
#2565. `bench/attributions.tsv` carries every row each merge moved by 0.1% or more.

## The 2026-10-10 save is not the emulator on `spawn_reap`

That save's own A/B found the QEMU 11.1.2 term zero on every x86_64 row. Its floor moved
`spawn_reap` only +0.05% because `main`'s +1.76% met the tick leaving the row's window (-1.69%).
So the merges carry that step, and the file has no `*` line for it. A `*` line there would leave
the tick's entry at 2026-10-03 with no exit to cancel against, 1.6 points of unexplained drift.

## #1525 is a cost, not the ping-pong's mode

The riscv64 bisect gave #1525 no line, because part of its `ipc_rtt` step might be the ping-pong's
mode ([riscv64-unexplained-drift](riscv64-unexplained-drift.md)). On x86_64 it moves `ipc_rtt`
+3.09% and no other row. Deleting its one line, the plain `RECV` path's clear of the collected
sender's `outgoing_cap`, takes `ipc_rtt` down 3.00% at `350c1da1f`, 2.99% at `bc06c6a23` and
3.04% at `b6233a3b8`, and moves nothing else. The cost held for three weeks, so it has a line. On
`main` the same deletion moves no row: since milestone 939 `ipc_rtt` runs receiver-first and never
collects a queued sender.

## Costs over 1%

Each is a real -O0 cost on a published row:

- #1892, milestone 812 (`std::thread::spawn` runs real threads in one address space): `spawn_reap`
  +1.90%, the process join it chose.
- #1614, fatal risk 4 (the per-crossing cost): `spawn_reap` +1.38%, the reaper freeing a thread's
  stack outside `IPC_TABLES` and taking the lock again.
- #1525, milestone 633 (an outside agent attacks the confinement claim): `ipc_rtt` +3.09%, a
  confinement fix.
- #1133, milestone 315 (a port revoke that reaches every core): `tss_iomap_lazy_nop` +1.16%. The
  switch installs the incoming thread's port grant while it holds `IPC_TABLES`, so a revocation's
  broadcast can reach every core.
- #1997, milestone 891 (a mutable static becomes an atomic, a lock or a recorded cell):
  `tss_iomap_lazy_switch` +1.10%. The TSS and port-grant statics became `SyncUnsafeCell`, whose
  `get` is a call at -O0.

## What stays unexplained

With these lines x86_64 reads 1.86, 0.26, 1.97, 0.25, 0.20, 0.22 and 0.22 from 2026W39 to W45. By
the ruling on #2565 (2026-10-22 UTC) the tick's phase gets no line. W39 and W41 keep it in the
merge queue's own readings: `18000c319` reads `spawn_reap` 1.87% above its tick-free count,
`ae7783389` 1.94%. The rest is steps under 0.1%, and the four tick phases on `spawn_reap`, which
nearly cancel.

## BUGS

- The tick's phase on `ipc_rtt` at the 2026-10-10 save (-2.40%) cancels against the 2026-10-22
  save's (+2.45%) only because no `*` line claims either step whole. A `*` line on either would
  turn the other into unexplained drift.
- Every cost here is a debug-build cost. Release builds were not measured.
