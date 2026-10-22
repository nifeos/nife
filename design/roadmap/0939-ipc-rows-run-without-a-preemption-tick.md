---
status: BUILT
promoted_from: ipc-rows-run-without-a-preemption-tick
raised: 2026-10-19
built: 2026-10-22
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 939. The icount bench runs without a preemption tick in its windows

Raised by the lane `bench/aarch64-drift-since-1010`, while attributing the aarch64 drift since the
2026-10-10 floor ([the attribution](../../notes/benchmarks/drift-since-the-2026-10-10-floor.md)).
Name: provisional.

Reuse: the reasoning and tests of milestone 541 (a timed window that excludes preemption); the
existing `TICK_HZ` constant. Nothing new is written beyond a `bench`-feature value and the floors it
moves.

## The failure

`ipc_rtt` and `ipc_rtt_el0` are bimodal under the periodic tick. Each is a ping-pong in which a
receive either finds its message waiting or blocks for it, and a tick landing in the window can
switch the loop between those modes for the rest of the run. Where the ticks land depends on every
instruction executed before and inside the window, so any change anywhere moves these rows by
amounts unrelated to its cost.

Measured on aarch64 between 2026-10-10 and 2026-10-19: a `schedule` refactor read -3.24% on
`ipc_rtt` and costs +0.22%. A one-step read of the running thread read +4.87% and costs +1.24%. A
crate move with no kernel change moved `ipc_rtt_el0` 0.1%. With `TICK_HZ` 1, so no tick lands in a
window, all three read their cost, and the crate move reads zero on every row. Since the floor,
`ipc_rtt` drifted +3.72% under the tick and +2.33% without it. A floor saved under the tick
records that luck as cost, and the restamp's 0.5% per-row bound refuses nightlies for it.

Milestone 541 fixed the same defect for `map_new` by masking interrupts across the window. That
cannot work here: these windows block and switch, and the other thread runs with interrupts on.

## Options weighed

### A `bench`-feature `TICK_HZ` low enough that no window holds a tick (recommended)

One constant under `cfg(feature = "bench")`. The whole aarch64 suite ran at `TICK_HZ` 1 in this
lane's measurements, with every row produced (its wall time was not compared). Every row that does
not ping-pong read the same with and without the tick to within 0.1%. `spawn_reap` is the exception,
and it moved toward the cost: +0.38% under the tick against +1.85% without, across the same commits.

What it gives up: a row that means to measure being preempted. None of today's rows does. 541's
rule that `yield_switch` and `spawn_reap` must not mask still holds, because they measure voluntary
switching, which this leaves intact.

### A per-window hold on preemption that the tick honors

`timed` sets a per-core flag that the tick handler reads, so the tick is taken but does not
reschedule inside a window. More precise, and it keeps the timer's behavior for everything outside
windows. It adds a branch to the tick path in the bench build and a mechanism that has to be right
on three architectures. It would need its own tests in the shape of 541's. It was not measured.

### Leave the tick and report the mode beside the row

Count blocking receives in the window and print them, so a reader can see a mode switch. It explains
a step and does not remove it, and the floor still records luck.

## Measured on all three architectures

Eight consecutive merges on `main` (`2583c88dc` to `ae940dc46`, 2026-10-22), each benched under the
100 Hz tick and at `TICK_HZ` 1, plus `ae940dc46` at 91, 97, 103 and 109 Hz, which moves only where
the ticks fall. The merges are renames, a frame-ledger fix and a nightly bump; none touches IPC.
Ticks of the row, QEMU 11.1.2, each merge's own nightly.

| row | 100 Hz across the merges | `TICK_HZ` 1 across the merges | 91 / 97 / 103 / 109 Hz |
|---|---|---|---|
| aarch64 `ipc_rtt` | 1,054,693 | 1,098,942 | 1,060,121 / 1,099,589 / 1,072,129 / 1,099,266 |
| aarch64 `ipc_rtt_el0` | 10,805,681 to 10,842,526 | 10,716,546 to 10,716,547 | 10,798,861 / 10,723,617 / 10,725,046 / 10,750,100 |
| riscv64 `ipc_rtt` | 175,709 to 175,710 | 178,570 to 178,571 | 177,489 / 178,657 / 178,657 / 178,657 |
| riscv64 `ipc_rtt_el0` | 1,800,481 to 1,807,390 | 1,799,369 | 1,819,330 / 1,801,774 / 1,807,502 / 1,812,274 |
| riscv64 `map_el0` | 46,103 to 47,494 | 47,493 to 47,494 | 47,535 / 47,535 / 47,535 / 47,493 |
| riscv64 `spawn_reap` | 37,702 to 37,706 | 36,940 to 36,944 | 36,987 / 37,543 / 36,943 / 36,943 |
| x86_64 `ipc_rtt` | 17,827,240 | 17,820,066 | 17,827,240 / 17,397,186 / 17,827,240 / 17,357,289 |
| x86_64 `spawn_reap` | 2,986,735 to 2,986,799 | 2,986,735 to 2,986,799 | 3,044,611 / 2,990,386 / 2,986,799 / 2,986,799 |

riscv64 and x86_64 share the bimodality. `ipc_rtt` moves 1.7% on riscv64 and 2.7% on x86_64 with
the tick's phase alone, and 4.1% on aarch64. x86_64 has no EL0 rows. On the merges sampled its
phase happened to stay in one mode, which is the luck a floor records. Two rows not in the proposal
are tick-phase rows too: riscv64 `map_el0` reads 46,103 or 47,493 by phase (3.0%), and `spawn_reap`
moves 2.1% on riscv64 and 1.9% on x86_64. At `TICK_HZ` 1 every row on every architecture reads the
same across the eight merges to within one tick in ten thousand.

## What was built

- `TICK_HZ` is 1 under `cfg(feature = "bench")` on all three architectures, and 100 otherwise.
- `TickWindow` (`kernel/src/bench/timing.rs`) waits for this core's next tick, so a window starts a
  full second before the next one, and panics if a tick arrives before it closes. `timed` opens
  one for every kernel-thread row. The EL0 rows open one before spawning the program that
  self-times and close it on its report. `map_new` opens its window before masking, since a
  window cannot wait for a tick with interrupts masked, and keeps 541's mask.
- `yield_switch` and `spawn_reap` still do not mask interrupts (541's rule).
- `cargo xtask bench` stops at a `[PANIC]` line and fails the run. A bench kernel that panics halts
  in `wfi` and never exits, so the gate hung where it now fails. `a_kernel_panic_ends_the_run_without_done`
  holds it to that.
- x86_64's APIC count is a checked conversion, because at 1 Hz it is the whole APIC rate.

`ipc_rtt` starts in the mode the tick before its window leaves it in. That tick preempts the
waiting bench thread and reschedules, and the ping-pong then runs in its faster mode on all three
architectures. It reads 1,050,692 on aarch64, 173,351 on riscv64 and 17,138,066 on x86_64, against
1,098,942, 178,571 and 17,820,066 at `TICK_HZ` 1 with no alignment. Every other row reads what it
read at `TICK_HZ` 1 to within a few ticks. The mode is fixed by the window's construction rather
than by where unrelated code left the tick.

The exit criterion is met on all three architectures. With this built onto `2583c88dc`,
`e972aa622` and `d31c86226`, no row moves by more than 0.011% (a few ticks of `spawn_reap`). With
the bench tick put back to 100 Hz, the riscv64 bench fails at its first row, `yield_switch`, naming
the row and the tick. Each floor is resaved with `--save --why` by a person.

## Follow-on

- **None.** The riscv64 steps that #2565 left on the tick's phase are listed in this milestone's
  pull request, for that lane's attribution lines to re-measure.

## Index row

`ipc_rtt` and `ipc_rtt_el0` are bimodal under the periodic tick, so unrelated changes move their floors by luck. A bench-only `TICK_HZ` low enough that no window holds a tick makes those rows read their cost, and every floor is resaved by a person when it lands.
