# riscv64 `spawn_reap` since the 2026-09-26 floor: four costs, a double copy and the tick

*An appendix to [`notes/benchmarks.md`](../benchmarks.md). Name: provisional (lane
`bench/riscv64-spawn-reap-drift`, 2026-10-22 UTC); [the naming record](README.md) holds the ratified
ones.*

## The question

The unexplained-drift chart (#2554) named riscv64 `spawn_reap` at +9.1% over its anchor with no
record accounting for it, in three steps: +3.30% absorbed by the 2026-10-03 save, +4.25% absorbed
by the 2026-10-08 save, and `main` 3.0% over the floor. Which merges raised it, by how much, and was
each cost meant? riscv64 `yield_switch`, at +5.6% unexplained, was answered from the same runs.

## How it was measured

riscv64 icount on QEMU 11.1.2 and `nightly-2026-10-21`, at every point, from `52d21046e` (the
merge that brought the 2026-09-26 floor) to `main` at `43a2d16e1`, 1,119 first-parent merges. The
compiler is held fixed: the floor commit reads 34,490 on today's nightly against its saved 34,489.
Each step was found by recursive bisection, so every step is found, not only the first. A tick is
100 instructions on riscv64 ([icount-tick-scales](icount-tick-scales.md)), and a `spawn_reap`
reading is 64 iterations. The kernel is the -O0 build the gate measures.

Every point was measured twice: as the bench runs, and rebuilt with `TICK_HZ` 1 so no preemption
tick lands in a window. The second is the cost. Commits from 2026-09-26 to 2026-10-03 that predate
`964bd5a88` ran with that commit's one-line fix applied, since their bench refuses to start without a
disk image it never reads.

## The tick is a fifth of the drift

`spawn_reap`'s window is about 3.8 million instructions and the tick comes every 10 million, so
whether one lands inside is decided by where unrelated code left the timer's phase. On `main` one
does: 37,702 ticks with the tick against 36,940 without, a 2.1% term that is no code's cost.

It moves the floors on its own. Tick-free, #1525 and #1611 cost nothing and #1632 saves nothing,
but with the tick they read +498, +498 and -529. #2331 reads +733 with the tick and +13 without.
Milestone 939 (the icount bench runs without a preemption tick in its windows) is the remedy.

## The merges

Each step against the merge's parent, in ticks of 64 iterations. "With tick" is what the floors
saw; "cost" is tick-free. Merges that move neither by more than 10 are left out.

| merge | with tick | cost | what |
|---|---:|---:|---|
| #1360, milestone 126 (the `procps` package) | +1,075 | **+576** | the 24-to-32 slot table and the switch counter ([#1486](drift-since-the-2026-09-26-floors.md)) |
| #1489, milestone 629 (the context-switch statistic stops costing the switch path) | -373 | -86 | the counter made cheap |
| #1620, the IPC fastpaths shrunk | +374 | +86 | |
| #1614, fatal risk 4's `null_syscall` contention | +380 | **+645** | the reaper frees a thread's stack outside `IPC_TABLES` and takes it again to remove the thread |
| #1617, milestones 754 and 761 | +873 | **+916** | below |
| #1743 | -133 | -134 | |
| #1852, milestone 139's queue token | +123 | +124 | |
| #1892, milestone 812 (threads in one address space) | +569 | **+569** | |
| #2148 | -177 | -177 | |
| #2188 | -41 | -84 | |
| #2331, `SUSPEND` and `RESUME` | +733 | +13 | the tick |

The tick-only merges with no cost: #1443 +51, #1473 -264, #1525 +498, #1542 -211, #1600 -283,
#1611 +498, #1634 +52, #1632 -529.

### #1617: two chosen costs and an accidental third

Inside the merge, each milestone on its own base: 754 (the table grows to 64 slots) +140, 761 (each
table gets its own lock, in the TCB page) +23. Together +917. A per-function profile of 64
iterations (a QEMU plugin counting each translation block, the run at 128 iterations minus the run
at 64) put +82,560 of the difference in `memcpy`. 761 moved the table out of the `Thread` and into
`init_capability_table`'s `ptr::write` of a constant. At -O0 that write is a call, so the constant
is built into a temporary and then copied again: two copies of 2 KiB on every thread's birth, and
754 doubled both.

## What was fixed

Two changes, at `main` `43a2d16e1`, percent of the row:

- **The table is copied once**, from a static image, in `thread::write_empty_capability_table`.
- **`IrqSafeMutex::lock_masked` is always inlined.** 761 split `lock` in two, which at -O0 is a
  second call on every acquisition of every lock. A profile of 761's yield put +56 instructions a
  round trip on it.

| row | riscv64 with tick | riscv64 tick-free | aarch64 tick-free | x86_64 tick-free |
|---|---:|---:|---:|---:|
| `spawn_reap` | -3.20 | -2.81 | -2.27 | -1.11 |
| `spawn_el0` | -1.17 | -1.07 | -0.89 | |
| `yield_switch` | -0.52 | -0.52 | -0.49 | -0.52 |
| `ipc_rtt` | -0.64 | -0.54 | -0.50 | -0.54 |
| `call_reply` | -0.81 | -0.80 | -0.74 | -0.80 |
| `broker_rtt` | -0.80 | -0.80 | -0.74 | -0.80 |
| `ctx_switch` | -0.50 | -0.48 | -0.46 | |
| `ipc_rtt_el0` | -0.75 | -0.73 | -0.68 | |

With the tick, riscv64 `spawn_reap` reads the tick again: the copy alone takes it to 36,176
(-4.05%, the window loses its tick), and the inlining on top reads 36,494 (+0.88%, the tick
returns) while it costs -0.75%. `null_syscall` does not move. aarch64 with the tick reads `ipc_rtt` +2.07%, its bimodal mode
switching ([drift-since-the-2026-10-10-floor](drift-since-the-2026-10-10-floor.md)); tick-free it is
-0.50%.

## What stays, and why

- **#1614, +645 (+1.8%).** Chosen: freeing six stack pages under the global lock stalled every
  other core's IPC on radon. The second critical section is the price.
- **#1892, +569 (+1.6%).** Chosen: milestone 812's process join, as #2426 recorded on aarch64.
- **#1360 and #1617's table bytes.** Chosen: milestones 126 and 754 grew the table, and spawning
  initializes it. Milestone 778 (capability tables sized per process) is where that is reconsidered.

## `yield_switch`

`main` reads 183,330, under its 184,370 floor. Its +5.6% unexplained is two absorbed steps. #1360's
counter (+5,382, +2.88%) is #1486's, and milestone 629 later took 4,116 of it back. #1617 is the
other, +6,443 (+3.42%), and all of it is milestone 761: +318 instructions a round trip. Of those,
+256 is `set_current` storing the running thread's table pointer on every switch, which is 761's
design priced at -O0. The other +56 is the lock split fixed above.

## BUGS

- The bisection finds a step only between two points that differ. Two equal and opposite steps
  inside one interval cancel and are not seen. The tick-free series makes that unlikely for costs,
  not impossible.
- `set_current`'s store costs about 136 instructions a round trip on `main` at -O0, mostly
  `cpu::id()` and an out-of-line atomic store. Making it cheaper means moving the pointer into
  `PerCpu`, whose 128-byte size is itself a measured choice. Not attempted.
