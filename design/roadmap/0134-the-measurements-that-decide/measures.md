# The measures, defined: E1 to E4 and M5 to M12

This appendix belongs to [milestone 134 (the register of measures)](../0134-the-measurements-that-decide.md).
It was moved out on 2026-10-12 (UTC) under §212 (a prose budget), and the main page is complete
without it. It holds each measure's full definition: the mechanism, the prediction, the instrument
and what each outcome settles.

## Tier A: available today, on the dev machine under Hypervisor.framework

### E1. IPC latency against thread count. Decides §96.

The mechanism, stated so the experiment can falsify it. Thread kernel stacks are `STACK_SLOT_SPAN`
apart, which is 28 KiB. Each IPC that switches threads touches a different stack, so the lines are
cold; an event kernel touches the same per-core stack every time and they are hot. The penalty
therefore scales with the number of distinct threads cycling through the kernel, and it is invisible
at two.

The prediction, with the arithmetic shown so it can be wrong. Take the smallest L1d among machines
we run on (32 KB, the SiFive U74). Take one IPC to touch roughly 1 to 2 KiB of its own kernel stack.
Then the stacks alone fill L1d somewhere between 16 and 32 threads. So the curve should be flat and
then bend, with a knee in the low tens. (The per-IPC figure was measured on 2026-09-19 at about 600
bytes, which moves the capacity knee to about 54 threads; the main page's BUGS say what that means.)

The measurement. `ipc_rtt_el0` with N client-server pairs in the rotation, N from 2 to 128, wall
clock, reported per iteration. Nothing about it needs a cycle counter.

What each outcome settles. A flat curve to 128 threads says the process kernel costs us nothing
on this axis and §96 (process kernel) is answered no, on data. A knee near the prediction reproduces
Warton's effect on our own kernel and gives its magnitude, which is the number §96 says it lacks.

It must not run under icount, which models no caches and would report a flat line by
construction, and that flat line would look like an answer.

### E2. The thread census on the customer path. Decides whether §96 matters at all.

Cheapest of the four and possibly decisive on its own. How many threads does the actual workload
create? If the SMB and filesystem path runs a dozen, the rotation never approaches the knee, the
stacks stay hot, and §96 is moot for this system whatever E1's curve looks like. §96's memory input
is already dead (3.00 MiB, 0.069% of a 4 GiB board); this asks whether its performance input is dead
too.

Instrumented run of the existing service path, counting live threads at peak. No new benchmark.

### E3. Footprint perturbation. Decides §95's premise, with no PMU.

Pad the IPC fastpath with resident dead code until `script/fastpath-footprint` reports roughly
double, and measure the latency change. If footprint is the binding constraint the padding hurts; if
latency does not move, 5.6 KiB is not costing anything on that machine.

This is how to test Liedtke's claim without a cache counter, which is what makes it available
today.

Its power is asymmetric, and that is the limitation to state loudly. A positive result on the
dev Mac is conclusive: if padding hurts on a machine with large caches, it certainly hurts on a
32 KB L1i. A negative result on that machine proves little, because an M-series core may simply
absorb the whole path. So E3 answers §95 cheaply in one direction and needs a small-cache board in
the other.

### E4. Application displacement. The Liedtke measurement proper.

Everything above measures the kernel. Liedtke's actual claim is about the cost the kernel imposes
on the application after the system call returns, which no kernel-side benchmark can see by
construction, and which is the reason Mach's IPC looked acceptable in isolation.

Shape: a userspace program with a known, tunable working set, measured with and without concurrent
IPC traffic. The number is the throughput the application loses, which is the AIM7 shape and the one
figure that speaks to the thesis rather than to a subsystem.

This is the most valuable of the four and the least defined. It is listed last for that reason, not
because it matters least.

## Tier B: gated on the counters, the silicon, or both

Each of these names its instrument and what it settles. None is blocked on design work here. They
are blocked on milestone 74 (the counters), on milestone 75 (who may read the cycle counter, and by
what authority), and on milestone 127 or a VisionFive 2 (silicon with a real PMU and a small cache).

The recurring reason they need hardware: QEMU-TCG models no cache, no TLB and no cycle, and HVF
passes through no PMU. So on the dev machine every one of these reads as either zero or as
nanoseconds times an assumed clock.

### M5. Cycles per IPC round trip

Instrument: `PMCCNTR_EL0`, and the SBI PMU or the `cycle` CSR on RISC-V. What it settles: the ±1.5x
uncertainty milestone 101 (the L4 calibration) records. That uncertainty comes from not knowing
whether the vCPU thread ran on a 2.75 GHz E-core or a 4.05 GHz P-core, and it is why no ratio in
that file may be quoted tighter than "same order". Until this exists, every cycle figure in this
project is arithmetic, and the seL4 comparison rests on it. Reference point: seL4's published 413
and 426 cycles one-way on the same class of silicon.

### M6. Instruction-cache misses per IPC

The Liedtke quantity for §95 (hand-written IPC), and the thing `script/fastpath-footprint` is a
proxy for rather than a measurement of. Milestone 132 (the fast path's footprint) says so in its own
BUGS. A footprint of 5.6 KiB against a 32 KB L1i predicts near-zero steady-state misses; if the
measurement disagrees with the prediction, the proxy is wrong and the gate's threshold should move.
Good: flat in steady state. Bad: misses tracking footprint.

### M7. Data-cache misses per IPC, attributed to the kernel stack region

The direct form of §96. E1's curve is the symptom; this is the cause, and the two together are
what make the process-kernel question answerable rather than arguable. The specific test is narrow:
do D-misses rise with thread count, and do the missing lines fall inside the stack area? A rise with
no stack attribution refutes the mechanism while leaving the symptom, which would be the most
informative outcome of the whole register.

### M8. TLB misses per IPC, instruction and data

Settles two claims at once. Whether the 28 KiB stack stride costs TLB coverage as threads
multiply, which is the other half of M7. And whether the ASID work does what
notes/address-space-identifiers.md claims: that the context-switch flush disappeared on aarch64,
while RISC-V keeps flushing when `satp.ASID` is zero bits wide. That asymmetry is currently an
argument from the code, and this is what would turn it into a number.

### M9. Per-phase cycle attribution across one IPC

Trap entry, decode and dispatch, rendezvous, switch, trap exit. What it settles: where fastpath
effort should go, which nothing currently knows. Milestone 132 found `system_call::dispatch` is the
largest single symbol at 2,024 bytes, and §95 reasons from that to it being the obvious thing to
skip. Large is not the same as costly, and this measure is the only thing that can tell the
difference. It could easily refute the premise of the fastpath as currently sketched.

### M10. Application working-set displacement per IPC

E4 measures this as lost throughput, which is the honest indirect form. This is the direct one:
how many lines of the application's working set does one IPC evict. It is Liedtke's actual
claim, stated as a number, and it is the figure that speaks to the thesis rather than to a
subsystem. `notes/benchmarks/kernel-footprint-and-caches.md` already sets a target of under 1 KiB of
data touched per IPC, roughly 16 lines, and nothing measures it; this is what would.

### M11. Interrupt latency, worst observed and worst bounded

The counterweight to §95's fastpath. Direct process switch "generally ignore[s] priorities",
which is why seL4 made it subject to them and Fiasco.OC made it optional. A kernel that gets faster
at IPC while getting less predictable under interrupt has made a trade nobody priced. Needs real
hardware because interrupt delivery under emulation is not the thing being measured.

### M12. The same measures on seL4, on the same board

This is milestone 25 (cross-OS performance comparison)'s deliverable on milestone 127 (seL4
machine)'s TX1, cited rather than duplicated. What belongs here is the constraint that makes it
honest. M5 through M10 must be captured on both kernels, on the same silicon, in the same build
configuration. Otherwise the comparison inherits exactly the class of error milestone 101 recorded,
where three separate mistakes cancelled into a plausible-looking ratio.
