---
status: NOT-STARTED
promoted_from: ipc-rows-run-without-a-preemption-tick
raised: 2026-10-19
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

## What it needs from a person

All three architectures' floors move when it lands, so it is a `--save --why` on each, a person's
act. riscv64 and x86_64 were not measured for the bimodality. The proposal assumes they share it,
because the ping-pong does, and the lane that builds it should check that first.

## Index row

`ipc_rtt` and `ipc_rtt_el0` are bimodal under the periodic tick, so unrelated changes move their floors by luck. A bench-only `TICK_HZ` low enough that no window holds a tick makes those rows read their cost, and every floor is resaved by a person when it lands.
