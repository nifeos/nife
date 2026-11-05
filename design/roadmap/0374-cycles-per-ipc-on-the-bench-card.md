---
status: NOT-STARTED
raised: 2026-09-04
milestone_dependencies: 74
decision_dependencies: none
machine_requirements: riscv64 silicon; PMU cycle counter
specific_machine: none
needs_person: yes
---
# 374. Cycles per IPC on the bench card, so E3's verdict is not a 250 ns quantum

Checked against the tree on 2026-09-19. `bench::timed` still
reads `arch::timer::now()` and still prints one tick count per row, so every row a bench card prints
is a `rdtime` tick and on the JH7110 that is still a 250 ns quantum. What has arrived since is the
denominator rather than the resolution: `kernel/src/bench.rs`'s `cycles_per_tick` probe reads real
cycles on all three architectures now (milestone 74's riscv64 half, milestone 309's x86_64 half) and
radon printed `cycles_per_tick 250.00` on 2026-09-16. That converts a tick but does not subdivide
one. The rows this file names still have no cycle column.

Milestone 74's riscv64 half landed 2026-09-04
(`kernel/src/arch/riscv64/pmu.rs`), so the counter exists; what is left is wiring it into
`kernel/src/bench.rs`'s rows and a bench session on radon to read them. Nothing here is blocked on
design.

## In brief

Every row a bench card prints is a `rdtime` tick count. On the JH7110 the timebase is **4 MHz**, so
one tick is **250 ns**. An IPC round trip on this class of core is single-digit microseconds, and
E3's whole question is whether a 1.86x footprint change moves it by a few percent. That is a handful
of ticks, measured with a ruler whose smallest mark is a large fraction of the effect.

`kernel/src/arch/riscv64/pmu.rs` reads real cycles through the SBI PMU extension, and milestone
229's per-thread grant (`cycle_counter_grant`) is the authority DECISIONS 139 decided. Neither is
wired into `kernel/src/bench.rs`'s rows. Wiring them turns E3's comparison from "a few ticks" into
"a few thousand cycles", which is the difference between a result and a shrug.

## What it is

`bench::timed` reads `arch::timer::now()`. The proposal is a second, additive reading: keep the tick
column exactly as it is, so every recorded baseline stays comparable, and print a cycle column
beside it when the counter is available and the build asked for it. The `cycle_counter_grant`,
which already exists for this reason, already costs a measured 136 bytes on the aarch64 fastpath
closure.

The rows that want it are the ones a bench session actually reads:
`ipc_rtt`, `ipc_rtt_el0`, `call_reply`, and E1's `ipc_scale_*`.

## Why it is not this lane's

Two reasons, and the second is the real one. It is a change to the shape of every benchmark row,
which is a published measurement format that `script/bench --check` compares against
`bench/baseline-*.txt`; that is a decision about a recorded artifact rather than an addition to one.
And it is worth doing **after** a first bench session rather than before, because the session will
say whether the tick quantum was actually the binding problem or whether boot-to-boot spread
dominates it anyway. Building the finer instrument first is the classic way to spend a day
sharpening a ruler nobody needed.

## What it still would not see

Cycles say *whether* a round trip got slower. They do not say the instruction cache is why. That is
M6 (instruction-cache misses per IPC) in milestone 134's tier B, and nothing in this tree reads a
cache-miss counter on any architecture. Whether the U74's PMU counts what M6 wants is unverified,
and that block's own BUGS warns that real PMUs do not implement every architected event.

## Index row

Every row a bench card prints is a `rdtime` tick count, and on the JH7110 one tick is 250 ns. An
IPC round trip is single-digit microseconds, and E3 asks whether a 1.86x footprint change moves it
by a few percent: a handful of ticks, measured with a ruler whose smallest mark is a large fraction
of the effect. The SBI PMU counter exists, and milestone 229's per-thread grant is the authority
DECISIONS §139 decided; neither is wired into a bench row. The proposal is additive. Keep the tick
column so every recorded baseline stays comparable, and print a cycle column beside it when the
counter is available. That changes a published format that `script/bench --check` compares against
`bench/baseline-*.txt`, so it is a decision about a recorded artifact. Do it after a bench session,
which says whether the quantum or the boot-to-boot spread was the binding problem. Cycles show that
a round trip got slower and not that the instruction cache is why, which is milestone 134's tier B.
