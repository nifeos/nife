# aarch64's unexplained drift: an unclaimed gain, the tick-free costs, and the tick's phase

*An appendix to [`notes/benchmarks.md`](../benchmarks.md). Name: provisional (lane
`lane/bench/aarch64-unexplained-drift`, 2026-11-05 UTC); [the naming record](README.md) holds the
ratified ones.*

## The question

The weekly chart of unexplained drift on `main` read aarch64 at +1.24% in 2026W45, from `ipc_rtt`.
Eight other rows read between -2.53% and -4.46%, so an unexplained gain was cancelling part of
every unexplained cost. Which merges moved each row, by how much, and what is left?

## How it was measured

aarch64 icount, the -O0 bench kernel, QEMU 11.1.2, one hart, on an x86_64 Linux host. Each point is
one run of

```
RUSTUP_TOOLCHAIN=<held nightly> script/bench
```

in a detached worktree at the commit, with `TICK_HZ` set to 1 in
`kernel/src/arch/aarch64/timer.rs` on trees before milestone 939 (the icount bench runs without a
preemption tick in its windows). Each window was sampled, and every interval whose ends differed
was split until one merge held the step:

| window | from | to | merges | nightly held |
|---|---|---|---:|---|
| `main`'s gap | `8c221d5be` (2026W43 reading) | `7ea93b038` (2026W44 reading) | 307 | each merge's own |
| 2026-10-22 save | `4a58737d0` | `b6233a3b8` | 604 | `nightly-2026-10-22` |
| 2026-10-10 save | `493ee18ec` | `bc06c6a23` | 36 | `nightly-2026-10-22` |
| 2026-10-08 save | `62619d7ff` | `493ee18ec` | 225 | `nightly-2026-10-22` |
| 2026-09-26 save | `5297ef8e8` | `a91d41934` | 52 | `nightly-2026-09-25` |

The harness reproduces what the floors and the merge queue recorded. `8c221d5be` reads the merge queue's
2026W43 rows exactly, and `a38988a3b` its 2026W45 rows. Run with the tick on their pinned nightlies, `5297ef8e8`,
`a91d41934`, `bc06c6a23` and `b6233a3b8` read their floors to within 0.3%. `493ee18ec` does not: its
`ipc_rtt` reads 3.79% over its floor, as the 2026-10-10 save's `# why:` line recorded, because the
machine that saved it caught the ping-pong's other mode. Each window's ends read the same on the
held nightly and on their own, so the held compiler adds nothing.

The 2026-10-22 window was not bisected afresh. Its merges are the ones the earlier bisects named,
[drift-since-the-2026-10-10-floor](drift-since-the-2026-10-10-floor.md) on aarch64 and
[x86_64-unexplained-drift](x86_64-unexplained-drift.md), each measured against its first parent.
Together they account for the window's tick-free step to within 0.01% on ten rows.

## `main`'s gap: #2735, a gain nobody claimed

The merge queue read every row of the 2026-10-22 floor exactly at `8c221d5be`. By 2026W44 the
rows had fallen 1% to 6%. Two merges did all of it: #2734 (the kernel lock becomes
`crates/spin_lock`), which the file already carried, and #2735, milestone 707 (aarch64 registers
spelled the way riscv64 and x86_64 spell them):

| row | #2735 | | row | #2735 |
|---|---:|---|---|---:|
| `spawn_reap` | -2.88 | | `call_reply`, `broker_rtt` | -1.94 |
| `ctx_switch` | -2.76 | | `ipc_rtt`, `relay_rtt` | -1.77 |
| `null_syscall` | -2.41 | | `sink_throughput` | -1.76 |
| `yield_switch` | -2.35 | | `spawn_el0` | -1.38 |
| `ipc_rtt_el0` | -2.20 | | `map_el0` | -0.81 |

`map_new` moves -0.36% and `coremark` not at all. The register crates' trait methods stayed calls at
opt-level 0, and the in-tree accessors are `#[inline(always)]`. #2735's pull request measured the
drop and left the floor alone, since the optimized code did not change. The other 305 merges move
no row by 0.1%, apart from #2933 and #2965, which have lines.

## The 2026-10-22 save: the with-tick figures replaced

The file carried this window's aarch64 merges as the 2026-10-10 floor note measured them with the
tick. On `ipc_rtt` the tick's phase dominates those figures. Without it:

| merge | `ipc_rtt` with the tick | without | `ipc_rtt_el0` with | without |
|---|---:|---:|---:|---:|
| #1892, milestone 812 | +2.04 | +0.68 | +1.53 | +0.69 |
| #2025, `schedule` split | -3.24 | +0.22 | -0.47 | +0.24 |
| #2188, `read_current_thread` masks IRQs | +4.87 | +1.24 | +2.54 | +1.45 |
| #2331, `SUSPEND` and `RESUME` | +0.18 | +0.18 | -0.57 | +0.15 |
| #2426, one `asm!` block | -6.81 | -2.91 | -2.99 | -3.40 |

The tick-free figures match the note's own "costs" table. On the other rows the old lines hold
within 0.12%. Three merges had no aarch64 line: #2446 (`IrqSafeGuard::drop` inlined, -0.15% to -0.41% on
twelve rows), #2454 (`spawn_el0` +0.65%) and #2565, the save's own change (`spawn_reap` -2.26%,
nine other rows -0.27% to -0.89%), which its `# why:` line measured and no line carried.

## The 2026-10-10 save: #1852

#1852, milestone 139 (drive the unsafe count down) round 10, the scheduler queue ownership token,
is that window's whole tick-free step. It moves `ipc_rtt` +0.42% and `ipc_rtt_el0` -1.05%, the
two rows the save's `*` lines leave out.

## The 2026-10-08 save: four merges beside #1617

Beyond the lines the file carried, the bisect finds #1663 (the user-mode runtime's yield is one
trap per architecture, `ctx_switch` +0.94%), #1692, milestone 95 (`ctx_switch` -0.36%), #1659,
milestone 105 (`ipc_rtt_el0` +0.28%, `sink_throughput` +0.34%) and #1676 (`map_new` +0.15%). #1617
reproduces its line to the hundredth.

## The 2026-09-26 save: the compiler term alone

`5297ef8e8` reads the same as `a91d41934` on `nightly-2026-09-25`, so no merge in the window moved
a row. The step is the nightly: `yield_switch` -0.13%, `ctx_switch` -0.12%, `ipc_rtt` -0.12%,
`relay_rtt` -0.10%.

## What stays unexplained

`ipc_rtt` reads +0.74%. Each step left over is the tick's phase, or the mode the saving machine
caught, and by calef's ruling B on #2565 (2026-10-22 UTC) neither gets a line:

| save | floor step | tick-free step | the rest |
|---|---:|---:|---:|
| 2026-09-26 | +0.22 | -0.12 | +0.34 |
| 2026-10-08 | +0.42 | +2.13 | -1.67 |
| 2026-10-10 | +2.83 | +0.42 | +2.40 |
| 2026-10-22 | -1.68 | -1.41 | -0.28 |

The rests compound to +0.74%. Milestone 939's `*` line takes the tick out at the 2026-10-22 floor,
so these four never cancel. No merge is left to find, and a fix in the kernel cannot move them.

Every row, 2026W45, before and after these lines:

| row | before | after | | row | before | after |
|---|---:|---:|---|---|---:|---:|
| `ipc_rtt` | +1.24 | +0.74 | | `ctx_switch` | -3.07 | -0.20 |
| `spawn_el0` | -1.44 | +0.47 | | `yield_switch` | -3.49 | -0.33 |
| `map_el0` | -0.79 | +0.19 | | `relay_rtt` | -3.21 | -0.34 |
| `ipc_rtt_el0` | -4.46 | +0.15 | | `call_reply` | -3.45 | -0.42 |
| `null_syscall` | -2.53 | -0.03 | | `broker_rtt` | -3.50 | -0.45 |
| `map_new` | -0.76 | -0.13 | | `sink_throughput` | -0.44 | -0.23 |
| `spawn_reap` | -4.45 | -0.15 | | `coremark` | 0.00 | 0.00 |

The chart's aarch64 line reads 0.16, 0.06, 2.39, 2.39, 0.75, 0.74 and 0.74 from 2026W39 to W45,
against 0.16, 0.06, 1.93, 2.39, 3.07, 1.23 and 1.24 before. W41 rises on #1892's line: the merge queue read
`main` with the tick until milestone 939, so that week's reading carries the +2.04% the tick made
of #1892 on `ipc_rtt`, and the line now claims only its +0.68% cost.

## Which costs were meant

Every cost found is a debug-build cost of a change its pull request chose: #1617 and #1852
(their milestones' designs), #1659 (the death message's label), #1663 (a yield that traps once),
#1676 (the shared IOMMU and memory code AMD-Vi needed) and #1892 (threads in one address space).
None is a regression to take back. #2735, #2426, #2446 and #2565 are gains.

## BUGS

- The tick's phase holds `ipc_rtt` at +0.74% for good. Only a ruling that a measured phase earns a
  line, now that `TICK_HZ` 1 can measure it at any old floor, would move it.
- In the 2026-10-22 window `ipc_rtt_el0` (+0.16%), `sink_throughput` (+0.20%) and `spawn_el0`
  (+0.26%) keep remainders no named merge accounts for. The window was not bisected end to end.
- Steps under 0.1% were not chased: `spawn_reap` +0.09% in `main`'s gap, for one.
- Every cost here is a debug-build cost. Release builds were not measured.
