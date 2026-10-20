# Drift since the 2026-09-26 floors: one counter, one layering, and three ISAs

*An appendix to [`notes/benchmarks.md`](../benchmarks.md). Milestone 626 (drift since the 2026-09-26 floors), number provisional, in
`design/roadmap/`. Name: provisional (the riscv64-drift lane, 2026-10-02); [the naming
record](README.md) holds the ratified ones.*

## 2026-10-02: the question, and a premise that failed

PR #1474 (the `nightly-2026-10-02` bump) re-saved the riscv64 floors and its `# why:` line recorded
drift the compiler did not cause: `map_new` +7.89%, `spawn_reap` +3.30%, `yield_switch` +2.98%, seven
more rows between 1.4% and 2.6%. The question was which commits on `main` between the 2026-09-26
save (`50d32760a` plus the pin) and `40ac2d5f0` caused it, and why riscv64 alone.

**It was not riscv64 alone.** The aarch64 and x86_64 floors were only *restamped* in #1474, and a
restamp measures the compiler term at one commit; it never compares `main` against the floors. Run
against them at `40ac2d5f0`, both drifted with the same shape:

| row | aarch64 | riscv64 | x86_64 |
|---|---:|---:|---:|
| `yield_switch` | +3.47% | +2.93% | +2.90% |
| `ctx_switch` | +3.27% | +2.56% | (no row) |
| `ipc_rtt` | +2.10% | +2.11% | +2.02% |
| `call_reply` | +2.23% | +2.00% | +2.10% |
| `spawn_reap` | +1.56% | +3.15% | +0.86% |
| `map_new` | +7.50% | +7.89% | +8.96% |

x86_64's `map_new` is the row nearest the 10% tripwire, not riscv64's. Its floor still says 176,491.

## How it was measured

Debug icount, QEMU 11.1.1, `RUSTUP_TOOLCHAIN=nightly-2026-09-30` forced at every point so the
compiler term is held at zero. That is sound here: the floor commit measures 2,369 for `map_new`
on both its own pin and `nightly-2026-09-30`, and every row agrees within 0.01% except
`ipc_rtt_el0` (0.24%). All 124 first-parent commits in the range were measured (26 s each), then
the two series that held a step were bisected inside. One run at a time, nothing else heavy beside
it.

## The culprits (riscv64 ticks, whole run)

Three merges, from milestone 23 (a capability-routed component OS with live replacement),
milestone 206 (a program image has under 896 KiB) and milestone 126 (the `procps` package).

| commit | what | `yield_switch` | `ctx_switch` | `ipc_rtt` | `spawn_reap` | `map_new` |
|---|---|---:|---:|---:|---:|---:|
| `b4f657d8a` (#1373) | milestone 23: `RETYPE` takes a page count | | | | | +133 |
| `7a90fcc59` (#1352) | milestone 206: the user address-space map | | | | | +43 |
| `0d90e750c` (#1360) | milestone 126: `free`, `vmstat`; table to 32 slots | +5,471 | +13,640 | +2,404 | +1,016 | +10 |

Each delta is against the commit's parent (`71769ba45` for #1360's series, whose first two commits
do not build). Between them the commits move rows by about one quantum of 93 to 103 ticks on `yield_switch` and
`ctx_switch`, up and down, and `ipc_rtt` rose about 0.9% in small steps before #1360. None of that
was chased.

### Milestone 126's context-switch counter is the switch and IPC drift, on every ISA

`machine_statistics::context_switch()`, called once per `schedule()`, is a relaxed `fetch_add` on
this core's word in the machine statistics page. Stubbing that one call at `40ac2d5f0` returns the
rows to their floors:

| row | riscv64 main / stubbed | aarch64 main / stubbed | x86_64 main / stubbed |
|---|---|---|---|
| `yield_switch` | +2.93% / +0.06% | +3.47% / +0.14% | +2.90% / +0.00% |
| `ctx_switch` | +2.56% / -0.01% | +3.27% / +0.11% | |
| `ipc_rtt` | +2.11% / +0.54% | +2.10% / +1.20% | +2.02% / +0.43% |
| `map_new` | +7.89% / +7.89% | +7.50% / +7.49% | +8.96% / +8.96% |

**The capability table's growth from 24 to 32 slots, in the same commit, is not it.** Building the
kernel at 24 slots on `main` leaves `yield_switch`, `ctx_switch` and the IPC rows untouched. It is
`spawn_reap`'s other cause (34,695 at 24 slots against 35,576), and `spawn_el0`'s (-1.2%), because
spawning initializes and copies the table.

What the increment costs. At -O0, which is what the gate measures, it is a call chain:
`cpu::id`, `arch::percpu`, `word::cpu`, an out-of-line `AtomicPtr::load`, overflow and alignment
checks, and an out-of-line `fetch_add` that dispatches on the ordering through a jump table. That is
about 134 instructions a switch, and a yield round trip switches twice. In a release build it is 10
instructions (read `tp`, load `PAGE`, subtract, shift, index, `amoadd`), and it still costs:

| riscv64 release, icount | main | counter stubbed | cost |
|---|---:|---:|---:|
| `yield_switch` | 12,920 | 12,360 | +4.5% |
| `ipc_rtt` | 11,980 | 11,700 | +2.4% |
| `ctx_switch` | 59,603 | 58,403 | +2.1% |

(Release kernel built by hand with `--features bench --release` and run through
`helpers/qemu-runner-riscv64.sh` with the same `-icount shift=0,sleep=off`; `script/bench --release`
is HVF and statistical, so it could not answer this.)

### Milestone 23's `RETYPE` layering is `map_new`'s main step, and it is debug-only

`retype_page` became `retype_run(region, 1)`, which goes through `RegionTable::retype_run`,
`retype_new_watermark`, `retype_pages`, a tuple and a checked multiply. Doubling `MAP_ITERS` doubles
the step (+133 at 64, +264 at 128), so it is **2.06 ticks, about 206 instructions, per page
mapped**. In release the same commit moves `map_new` from 1,152 to 1,160 (+0.7%). The optimizer
removes the layers and the gate's -O0 build keeps them.

### The address-space map's +43 is a fixed term, mechanism not found

`7a90fcc59` adds 43 ticks to `map_new` whether the bench maps 64 pages or 128, so the per-map cost
did not change. That commit changes no kernel code on the `map_new` path (constants and the image
band check), so something it moved is paid once inside the timed window. It is recorded rather than
found (BUGS below).

## Why riscv64 looked singled out

Only because riscv64 was the leg whose restamp refused a row (`spawn_reap` +0.602% compiler term
against a 0.5% bound), which forced a full `--save` on that leg alone. The other two kept floors that
predate the drift. The weekly drift report (milestone 415 (sub-tripwire drift accumulates across baseline saves), item 3) reads committed floors, so it
cannot see drift on `main` that no save has captured either.

## BUGS

- The tables above are against the 2026-09-26 floors. Milestone 629 made the counter cheaper and
  591148372 re-saved all three floors after it, so the numbers here describe the drift as found,
  not the floors in force.
- The +43 fixed term from `7a90fcc59` is unexplained. It is one-off rather than per map, so it
  does not grow, but it is in the timed window of a row that is close to the tripwire on x86_64.
- `spawn_reap`'s two causes do not add up. Stubbing the counter takes off 612 ticks and
  shrinking the table takes off 881, against a total drift of 1,087. They were measured one at a
  time on `main`, not together, and the overlap is not attributed.
- Before 2026-10-02 `script/bench --riscv` attached five disks it does not read, inherited from
  `host::cargo`'s exported `NIFE_DISK`, and failed on a checkout where no aarch64 leg had run
  `mkdisk`. Fixed in the same pull request; without the disks the rows move by at most 0.05%
  (`yield_switch` +96 ticks).
