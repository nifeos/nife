# Every baseline save, audited

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and
lets a reader act without opening this file. This one holds the 2026-09-15 walk of every save for
drift that accumulated under the tripwire, with the dates, tables and corrections behind them. Name:
ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## 2026-09-15: every baseline save since the first, audited for what accumulated under the tripwire

The tripwire compares against the last saved floor, so successive sub-threshold steps accumulate
without it ever firing: ten 9% steps are a 136% regression that never trips. Whether that had
happened was never checked. This is the check. It is arithmetic over `git log`, not a measurement:
the three baseline files are text under version control, so every number any save committed is
recoverable without booting anything. No QEMU was run for it.

### Method, and the two things that would make the numbers lie

Every commit touching `bench/baseline-<arch>.txt` is walked oldest-first, and each row is compared
against the same row in the previous save. Two corrections, each of which would otherwise
manufacture findings:

- Per-iteration, not per-total. A save that changes a benchmark's iteration count changes its tick
  total without changing its cost. Every step below is `ticks / iters` against `ticks / iters`, so
  an iteration-count change reads as zero.
- `git log --follow` crosses the rename into the wrong file. `bench/baseline-x86_64.txt` first
  appeared whole on 2026-08-25, and `--follow` walks back from it into `baseline-riscv64.txt`'s
  history, because the two were similar at the rename boundary. Taken literally that yields eight
  x86_64 "steps" of around +9800%, which are riscv64's numbers compared against x86_64's. The x86_64
  table therefore starts at its real birth, `d31aa77c`, verified with `git log --diff-filter=A`.

One more boundary. The aarch64 file's first seven saves are the harness under construction: the
benchmark set changes under it. `60e75545` (2026-07-28) pinned the bench to one hart after finding
the `-smp 4` counter was fiction, which changes the meaning of every earlier number (see
[the icount-drift appendix](icount-drift-and-provenance.md)). A cumulative figure from 2026-07-23
would measure the instrument's construction. So the cumulative table anchors aarch64 at `74431429`
(2026-07-30), the first save after both the one-hart fix and the QEMU pin. The table anchors the
other two at their birth.

### Every save, per architecture

`moved` counts rows whose per-iteration cost changed at all; `over 10%` names the rows that would
have tripped the gate.

#### `bench/baseline-aarch64.txt`, every save

| # | date | commit | moved | largest up | largest down | over 10% |
|---|---|---|---:|---:|---:|---|
| 1 | 2026-07-23 | `69994047` | (birth, 5 rows) | - | - | - |
| 2 | 2026-07-25 | `5c29d2af` | 5 (+1 new) | spawn_reap +22.17% | map_new -0.06% | yield_switch +13.7%, spawn_reap +22.2% |
| 3 | 2026-07-25 | `16235027` | 4 (+1 new) | ipc_rtt +1.79% | yield_switch -6.67% | no |
| 4 | 2026-07-25 | `01e23ab9` | 6 (+1 new) | map_new +3379.49% | call_reply -35.09% | ipc_rtt -22.3%, call_reply -35.1%, map_new +3379.5% |
| 5 | 2026-07-25 | `e782a840` | 7 (+1 new) | spawn_reap +1324.48% | map_new -97.13% | ipc_rtt -41.5%, call_reply +23.9%, spawn_reap +1324.5%, map_new -97.1%, ctx_switch +59.8% |
| 6 | 2026-07-25 | `b85eff98` | 8 (+1 new) | ipc_rtt +113.32% | spawn_reap -66.86% | ipc_rtt +113.3%, call_reply +37.5%, spawn_reap -66.9%, ctx_switch -37.3%, ipc_rtt_el0 -32.4% |
| 7 | 2026-07-26 | `7444e8bc` | 10 (+1 new) | ipc_rtt_el0 +38.32% | spawn_reap -78.52% | yield_switch -43.3%, call_reply -45.3%, spawn_reap -78.5%, ctx_switch -10.1%, ipc_rtt_el0 +38.3% |
| 8 | 2026-07-26 | `df3f6486` | 9 | call_reply +133.11% | ipc_rtt -48.67% | ipc_rtt -48.7%, call_reply +133.1%, ipc_rtt_el0 -35.7%, spawn_el0 +28.6% |
| 9 | 2026-07-26 | `a0d4584d` | 11 | spawn_reap +942.08% | call_reply -27.94% | ipc_rtt +137.0%, call_reply -27.9%, spawn_reap +942.1%, ctx_switch +94.6%, ipc_rtt_el0 +110.2%, spawn_el0 -21.1% |
| 10 | 2026-07-28 | `60e75545` | 11 | null_syscall +7.06% | spawn_reap -90.06% | ipc_rtt -63.6%, call_reply -48.7%, spawn_reap -90.1%, ctx_switch -57.4%, ipc_rtt_el0 -52.7%, spawn_el0 -44.0% |
| 11 | 2026-07-29 | `9890eb02` | 11 (+1 new) | spawn_el0 +8.54% | null_syscall -0.04% | no |
| 12 | 2026-07-29 | `9368657c` | 8 | null_syscall +0.27% | - | no |
| 13 | 2026-07-30 | `1ce094a5` | 0 | - | - | no |
| 14 | 2026-07-30 | `c2beba43` | 10 | relay_rtt +0.20% | ipc_rtt -1.44% | no |
| 15 | 2026-07-30 | `20c3efc7` | 9 (+1 new) | ipc_rtt_el0 +0.32% | spawn_reap -2.31% | no |
| 16 | 2026-07-30 | `74431429` | 7 | spawn_el0 +0.06% | ipc_rtt_el0 -0.33% | no |
| 17 | 2026-08-03 | `206b1342` | 0 | - | - | no |
| 18 | 2026-08-03 | `8c279536` | (+1 new: sink_throughput) | - | - | - |
| 19 | 2026-08-14 | `fa271cf4` | 1 | - | spawn_reap -10.95% | spawn_reap -10.9% |
| 20 | 2026-08-15 | `86422662` | 14 | spawn_reap +34.08% | map_new -0.05% | spawn_reap +34.1% |
| 21 | 2026-08-16 | `d5c4da34` | 1 | - | null_syscall -11.71% | null_syscall -11.7% |
| 22 | 2026-08-27 | `1259dc07` | 14 | map_el0 +8.57% | spawn_reap -1.45% | no |
| 23 | 2026-08-27 | `eed48f02` | 10 | spawn_el0 +0.19% | map_el0 -16.86% | map_el0 -16.9% |
| 24 | 2026-08-27 | `b918337b` | 14 | map_el0 +20.99% | spawn_el0 -41.53% | map_el0 +21.0%, spawn_el0 -41.5% |
| 25 | 2026-08-27 | `a79fdb95` | 9 | spawn_el0 +0.84% | map_el0 -16.76% | map_el0 -16.8% |
| 26 | 2026-09-15 | `85edb1ed` | 14 | yield_switch +6.49% | coremark -0.01% | no |

Saves 13 and 17 touched the file without moving a number: `1ce094a5` pinned the QEMU version into
the header and `206b1342` renamed the file to carry its ISA.

#### `bench/baseline-riscv64.txt`, every save

| # | date | commit | moved | largest up | largest down | over 10% |
|---|---|---|---:|---:|---:|---|
| 1 | 2026-08-03 | `8c279536` | (birth, 14 rows) | - | - | - |
| 2 | 2026-08-04 | `7cebc3df` | 14 | map_el0 +2.10% | spawn_reap -0.08% | no |
| 3 | 2026-08-14 | `0df3c7c9` | 1 | - | spawn_reap -12.87% | spawn_reap -12.9% |
| 4 | 2026-08-15 | `86422662` | 12 | spawn_reap +31.07% | coremark -0.01% | spawn_reap +31.1% |
| 5 | 2026-08-16 | `d5c4da34` | 1 | null_syscall +10.06% | - | null_syscall +10.1% |
| 6 | 2026-08-27 | `1259dc07` | 14 (+1 new) | map_el0 +9.78% | - | no |
| 7 | 2026-08-27 | `eed48f02` | 11 | spawn_el0 +0.18% | map_el0 -16.38% | map_el0 -16.4% |
| 8 | 2026-08-27 | `b918337b` | 13 | map_el0 +20.30% | spawn_el0 -42.16% | map_el0 +20.3%, spawn_el0 -42.2% |
| 9 | 2026-08-27 | `a79fdb95` | 9 | spawn_el0 +0.57% | map_el0 -16.29% | map_el0 -16.3% |
| 10 | 2026-09-15 | `85edb1ed` | 12 | yield_switch +6.61% | coremark -0.01% | no |

#### `bench/baseline-x86_64.txt`, every save

| # | date | commit | moved | largest up | largest down | over 10% |
|---|---|---|---:|---:|---:|---|
| 1 | 2026-08-25 | `d31aa77c` | (birth, 9 rows) | - | - | - |
| 2 | 2026-09-15 | `44890a8a` | 8 (+2 new) | yield_switch +9.94% | coremark -0.00% | no |

Two saves, ever, and the window between them is 1,526 commits. Nothing fired in it, for the reason
given under the x86_64 save below.

### The cumulative drift

From the anchor save to `main` on 2026-09-15. `steps` counts the saves that moved the row after the
anchor; `sub-10 up` counts how many of those were upward steps the tripwire would have passed.

| arch | bench | anchor | today | **cumulative** | steps | sub-10 up | largest single step |
|---|---|---:|---:|---:|---:|---:|---:|
| aarch64 | yield_switch | 534.84 | 583.82 | **+9.16%** | 5 | 4 | +6.49% |
| aarch64 | ipc_rtt | 967.51 | 1051.93 | **+8.73%** | 6 | 5 | +4.95% |
| aarch64 | ctx_switch | 574.06 | 617.86 | **+7.63%** | 5 | 2 | +6.19% |
| aarch64 | broker_rtt | 2010.23 | 2153.89 | +7.15% | 5 | 5 | +3.71% |
| aarch64 | call_reply | 1006.99 | 1078.68 | +7.12% | 4 | 4 | +3.70% |
| aarch64 | relay_rtt | 1965.41 | 2099.96 | +6.85% | 5 | 5 | +3.55% |
| aarch64 | ipc_rtt_el0 | 2094.11 | 2219.77 | +6.00% | 6 | 4 | +3.34% |
| aarch64 | spawn_reap | 2714.72 | 3306.58 | +21.80% | 7 | 4 | +34.08% |
| aarch64 | map_new | 242.48 | 246.00 | +1.45% | 5 | 3 | +1.50% |
| aarch64 | coremark | 81705.47 | 81702.67 | -0.00% | 6 | 3 | +0.01% |
| aarch64 | map_el0 | 838.61 | 777.35 | -7.30% | 6 | 3 | +20.99% |
| aarch64 | null_syscall | 22.95 | 20.26 | -11.71% | 5 | 2 | +0.06% |
| aarch64 | spawn_el0 | 19077.13 | 12841.47 | -32.69% | 6 | 5 | +5.80% |
| riscv64 | ctx_switch | 94.35 | 104.51 | **+10.78%** | 7 | 5 | +6.14% |
| riscv64 | yield_switch | 89.55 | 97.95 | **+9.39%** | 7 | 6 | +6.61% |
| riscv64 | ipc_rtt | 160.32 | 175.15 | **+9.25%** | 7 | 6 | +4.34% |
| riscv64 | ipc_rtt_el0 | 349.85 | 375.68 | +7.38% | 7 | 6 | +3.15% |
| riscv64 | broker_rtt | 338.01 | 362.33 | +7.19% | 5 | 5 | +3.72% |
| riscv64 | call_reply | 169.28 | 181.43 | +7.17% | 6 | 5 | +3.70% |
| riscv64 | relay_rtt | 328.53 | 351.42 | +6.97% | 5 | 4 | +3.57% |
| riscv64 | sink_throughput | 8.94 | 9.53 | +6.55% | 7 | 6 | +2.64% |
| riscv64 | spawn_reap | 445.69 | 530.92 | +19.12% | 8 | 3 | +31.07% |
| riscv64 | null_syscall | 3.28 | 3.61 | +10.06% | 6 | 3 | +10.06% |
| riscv64 | map_new | 36.92 | 37.44 | +1.40% | 3 | 1 | +1.48% |
| riscv64 | coremark | 14275.47 | 14274.92 | -0.00% | 7 | 5 | +0.01% |
| riscv64 | map_el0 | 132.02 | 124.62 | -5.61% | 5 | 2 | +20.30% |
| riscv64 | spawn_el0 | 3024.19 | 2036.31 | -32.67% | 7 | 6 | +6.45% |
| x86_64 | yield_switch | 9132.11 | 10040.08 | **+9.94%** | 1 | 1 | +9.94% |
| x86_64 | tss_iomap_switch | 11512.31 | 12409.08 | +7.79% | 1 | 1 | +7.79% |
| x86_64 | spawn_reap | 41363.00 | 43812.20 | +5.92% | 1 | 1 | +5.92% |
| x86_64 | call_reply | 17150.60 | 18127.19 | +5.69% | 1 | 1 | +5.69% |
| x86_64 | broker_rtt | 34260.76 | 36208.75 | +5.69% | 1 | 1 | +5.69% |
| x86_64 | ipc_rtt | 16734.61 | 17639.19 | +5.41% | 1 | 1 | +5.41% |
| x86_64 | relay_rtt | 33400.69 | 35208.25 | +5.41% | 1 | 1 | +5.41% |
| x86_64 | coremark | 1196341.49 | 1196337.48 | -0.00% | 1 | 0 | +0.00% |
| x86_64 | map_new | 2780.64 | 2780.64 | +0.00% | 0 | 0 | +0.00% |

The structural claim is confirmed. riscv64 `ctx_switch` has accumulated +10.78%, past the threshold
the gate enforces, and no single save ever moved it more than +6.14%. aarch64 `yield_switch` sits at
+9.16% across five steps whose largest was +6.49%. The gate never fired on any of them, and could
not have: it compares against the floor the previous save wrote.

`coremark` is the control. Pure compute, no switches, and flat to four decimal places across every
save on all three architectures. Whatever moved the other rows was the kernel's switch and IPC paths,
not measurement noise.

### The six flagged steps, classified

A step is flagged if it is large but under 10%, unexplained by its own commit, or part of a
one-directional staircase.

| step | what moved | classification |
|---|---|---|
| `9890eb02` 2026-07-29 | every kernel-side IPC row +4 to +8.5% | **codegen churn, disclosed.** Adding `relay_rtt` moved rows it does not touch. The commit says so: *"Adding it shifted the other kernel-side IPC benches a few percent (ipc_rtt +6%, all sub-tripwire), so the baseline is re-saved here to absorb that"* |
| `86422662` 2026-08-15 | `spawn_reap` +34%, **every other row +2 to +5%** | **mixed, disclosed.** The +34% is the intended cost of 24 KiB stacks. The across-the-board +2 to +5% is separate and the commit names it: *"Every other row drifts 2-5%, under the file's 10% tripwire"* |
| `1259dc07` 2026-08-27 | `map_el0` +8.6/+9.8%, `spawn_el0` +5.8/+6.5% | **intended cost, disclosed and later recovered.** CRITICAL 1's reclamation sweep. `b918337b` then took `spawn_el0` down 41% by bounding the walk by occupancy |
| `85edb1ed` 2026-09-15 | switch family +6.5% | **mis-classified.** Milestone 300 (decompose the icount baseline drift) attributed it correctly to one commit and then classified it as intended feature cost. PR #886 shows ~91 to 93% of it is removable |
| `44890a8a` 2026-09-15 (x86_64) | `yield_switch` +9.94%, IPC family +5.4 to +5.9% | **mis-attributed, and removable.** See below |
| `d5c4da34` 2026-08-16 | `null_syscall` -11.7% / +10.1% | **deliberate re-record**, over the tripwire in both directions, its own commit |

Four of the six are honest: the cost was real, the commit named it, and the number moved for a stated
reason. The two from 2026-09-15 are not, and they are the same regression on different
architectures. Milestone 300's classification is in [the drift-decomposition appendix](drift-decomposition.md).

### The x86_64 save is the failure this audit was looking for

`44890a8a`'s own message attributes its +5 to +8% to the toolchain: *"It also folds in the same
nightly-2026-09-15 drift the other two carry (ipc_rtt, relay_rtt, spawn_reap move ~5-8% though the
port grant never touches them; coremark, pure compute, is flat)."*

Milestone 300 then measured the toolchain term across exactly those two endpoint nightlies and found
it to be ~0: byte-identical instruction counts on the same code. So the attribution in that commit is
false, and the cost it blessed into the x86_64 floor was something else. PR #886 identifies it: the
const-`false` cycle-counter element still threaded through the shared context-switch tuple. The
optimizer folds it in release but not in the debug build this gate measures, and it was never
`target_arch`-gated. #886 recovers ~5.9% on x86_64 by removing it.

So on 2026-09-15 the x86_64 committed floor contained a removable regression, blessed on a stated
cause that measures zero. `bench --x86 --check` passed throughout because 5.9% is under 10%. The
tripwire could not see it, and the record beside the number said something untrue.

At the time of the audit nothing would have caught it, because x86_64 had no gate.
`script/ci-build`'s bench entry was `script/bench --check && script/bench --riscv --check`. The
third leg existed, was committed, and was never pulled, and `ci.yml` carried a `BUGS` note saying so.
That is why x86_64's only inter-save window is 1,526 commits long while aarch64's median is a few
dozen: on the gated architectures a gross regression forces a save, and on the third nothing did.

*(Correction, 2026-09-24: `ba99c835` (2026-09-15, "ci: gate the x86_64 icount baseline, which
nothing ever ran") added `script/bench --x86 --check` to that entry the same day, so the x86_64 leg
has been gated since. It caught its first failure on the next push; see
[the preemption appendix](preemption-in-the-window.md). The removable regression itself was
recovered by #886, merged 2026-09-16.)*

### What this does not say

- It is not a claim that 6 to 10% of real cost was smuggled in. Most of the aarch64 and riscv64
  accumulation is disclosed in the commit that caused it. A good deal of it is whole-crate codegen
  churn that this instrument cannot separate from real cost; [the icount-drift
  appendix](icount-drift-and-provenance.md) establishes that, and it is why the gate is 10% rather
  than 2%.
- It does not re-baseline anything. Recovering the two flagged steps was PR #886's job, which merged
  2026-09-16 and recovered ~33 ticks/switch on aarch64, ~5.5 on riscv64 and ~5.9% on x86_64.
  Deciding the mechanism is an architect's.
- Two rows moved a long way down: `spawn_el0` -32.7% on both ISAs, which is `b918337b`'s occupancy
  bound and a genuine win (see [the `spawn_el0` appendix](spawn-el0.md)). Cumulative drift is not a
  one-directional story, and a mechanism that assumed it was would be wrong about these.

The mechanism this argues for has its own write-up, since it is an architect's call:
`design/roadmap/0415-sub-tripwire-drift-accumulates-across-baseline-saves.md`.

*(Since then, 2026-09-24: part of that mechanism has landed. Milestone 302 (a baseline records what
it was saved against) merged on 2026-09-23 in #1126, and `script/bench --save` now requires `--why`,
which writes a `# why:` line into the baseline above the numbers.)*
