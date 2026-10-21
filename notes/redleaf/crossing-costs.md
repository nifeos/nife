# The cost per crossing: RedLeaf's table, nife's numbers, and Atmosphere's

An appendix to [`notes/redleaf.md`](../redleaf.md), under "Bet two: the cost per crossing". It
holds RedLeaf's Table 1 with its conditions, the caveats on it, the chain of ratios to nife's own
figures, and Atmosphere's Table 3 from the same group.

## RedLeaf's Table 1

It was measured on CloudLab c220g2: two Intel E5-2660 v3 10-core Haswell CPUs at 2.6 GHz and
160 GB RAM, bare metal. Hyper-threading, turbo boost, CPU idle states and frequency scaling were all
disabled, with *"seL4 configured without meltdown mitigations"*:

| operation | cycles |
|---|---|
| seL4 | 834 |
| VMFUNC (the instruction alone) | 169 |
| VMFUNC-based call/reply invocation | 396 |
| RedLeaf cross-domain invocation | **124** |
| RedLeaf cross-domain invocation (passing an `RRef<T>`) | **141** |
| RedLeaf cross-domain invocation via shadow | 279 |
| RedLeaf cross-domain via shadow (passing an `RRef<T>`) | 297 |

Their breakdown: *"a null cross-domain invocation via a proxy object ... introduces an overhead of
124 cycles. Saving the state of the thread, i.e., creating continuation, takes 86 cycles as it
requires saving all general registers. Passing one RRef<T> adds an overhead of 17 cycles."*

The quotable number is the ratio on one machine, not the absolute. 124 against 834 is roughly
6.7x, measured by one team on one box. That is the comparison that survives being moved between
papers. The absolute 124 should never be set beside a nife figure directly. It is a different ISA,
a different decade and a different measurement instrument.

Two caveats on their own table, in their favor and against. The shadow row disagrees with their
own prose, which says *"in case of a shadow the invocation crosses two proxies and a user-built
shadow domain and takes 286 cycles"* against the table's 279. Seven cycles, and it is worth noting
only because it is the kind of thing a reader quoting one of the two will not know about. And the
seL4 row is seL4's IPC, which is a *different guarantee*: a RedLeaf crossing does not change address
space, does not change privilege level, and does not schedule.

## What nife measures, for the shape rather than for a race

`notes/benchmarks.md` measures under HVF on an Apple M3, median of five boots. Its `ipc_rtt_el0`
is EL0 to EL0, two rendezvous, two address spaces and four `svc`s, at 350 ns. That note converts it
to ~960 to ~1,420 cycles across the M3's E-core and P-core clocks, because *"cycles here are
arithmetic, not a reading."* The same note anchors
against seL4's published Jetson TX1 figures, 413 + 426 = ~839 cycles for a round trip. That
conversion needs an assumed clock. Measured on one core instead, with seL4 booted under the same
HVF machine, nife's round trip is about 4.6 times seL4's `Call` plus `ReplyRecv`
([arm-plan-under-hvf.md](../arm-plan-under-hvf.md)).

So the honest statement of the gap is a chain of ratios rather than a subtraction. The chain has a
missing link this tree already knows about. `notes/benchmarks/calibration-against-sel4.md` says
`call_reply` *"has no EL0 twin. So the structurally matched comparison to seL4's published pair is
not measured at all"*. Milestone 188 (the IPC fastpath) owns closing that. Until it closes, nife's number and
RedLeaf's number are both roughly-6x-to-8x away from an L4-lineage crossing in opposite directions,
each measured against a different seL4 run on a different machine.

## Atmosphere's Table 3

The strongest evidence on this axis was published by RedLeaf's own authors, against themselves. Atmosphere (SOSP '25) is a verified Rust microkernel from the same group that *does*
use hardware isolation and a conventional syscall interface. Its Table 3, on CloudLab c220g5 (two
Intel Xeon Silver 4114 10-core at 2.20 GHz):

| system call | Atmosphere | seL4 |
|---|---|---|
| Call/reply | 1,058 | 1,026 |
| Map a page | 1,984 | 2,650 |
