# Calibration against L4 and seL4

*An appendix to [`notes/benchmarks.md`](../benchmarks.md), which carries the current numbers and is written so a reader can act without opening this file. This one holds what the IPC numbers mean next to seL4's published cycles, and why sel4bench cannot run on this host, with the dates, tables and corrections behind them. Name: ratified 2026-09-24 (calef); [the naming record](README.md) holds it.*

## The first real numbers, for the record (2026-07-23, M-series host, HVF, debug build)

IPC round trip ~705 ns; call/reply ~886 ns; yield round trip ~437 ns; spawn-to-reap ~2.8 µs;
fresh-page map ~634 ns. These are debug-build numbers. Missing that cost the note a wrong
comparison for twelve days (see the calibration below). Statistical, single run, shared machine:
shapes, not gospel. The 24 MHz counter grain (~42 ns) makes per-iteration ticks coarse, so read the
totals over 1000+ iterations. Cycle-exact PMU numbers arrive with the real silicon of milestone 16 (real hardware and IOMMU-backed driver isolation), which
inherits this harness and swaps the clock.

## Calibration: what these numbers mean next to L4's (corrected 2026-08-04)

IPC cost is the microkernel number because IPC multiplies through the whole architecture. Mach's
~100 us IPC discredited microkernels in the 1980s. Liedtke's L4 rehabilitated them with ~250-cycle
IPC on a 486, the "sub-microsecond" banner seL4's few-hundred-cycle fastpath still carries. This
is the paragraph a reader quotes, and from 2026-07-23 to 2026-08-04 it was wrong.

### The three errors in the old paragraph, and why they hid each other

The first version converted `ipc_rtt`'s ~705 ns at an assumed 3.2 GHz to ~2,200 cycles. It
reported us "4 to 7 times heavier" than an L4-lineage fastpath's 300 to 600. It had three
independent defects, and they do not point the same way:

1. Wrong plane. `ipc_rtt` is kernel-side: two kernel threads call `sched::ipc_send/receive` directly,
   in one address space, taking no trap. L4's published numbers are user-to-user across address
   spaces with the trap included. The benchmark that pays what theirs pays is `ipc_rtt_el0`, which
   existed since the EL0 primitive suite landed; the calibration never followed it. Fixing this
   alone makes us look worse.
2. Wrong build. ~705 ns is a debug figure. L4's are optimized builds, and the debug-to-release tax
   on the IPC path is ~6.7x (measured, in [the cross-OS appendix](cross-os-primitives.md)). Fixing
   this alone makes us look better, by more than the plane correction costs.
3. Wrong convention, which nobody had noticed at all. seL4 publishes one-way costs, the call and
   the reply timed separately. Ours is a round trip. Comparing the two doubles the ratio for free.

Errors 1 and 3 inflate the ratio and error 2 deflates it. So "4 to 7 times heavier" landed in a
plausible place by cancellation. An arithmetic chain over figures from different runs can be wrong
in three ways and still read as sober.

### One clean run, both planes (2026-08-04)

`cargo xtask bench --release --real`, Apple M3 (Mac15,3, 4 P-cores + 4 E-cores), HVF, `-smp 1`,
five boots back to back off one build. Host load average was ~3.2 of 8 cores throughout, with other
agent lanes active. The run-to-run spread below says that did not matter much. Both figures come
from the same boot on every boot, the property the old comparison lacked.

| bench | plane | ns/iter, median of 5 | the five | what one iteration includes |
|---|---|---|---|---|
| `ipc_rtt` | kernel-side | **46** | 46, 46, 46, 46, 49 | two rendezvous, one address space, no trap |
| `ipc_rtt_el0` | EL0 to EL0 | **350** | 347, 348, 350, 357, 369 | two rendezvous, two address spaces, four `svc`s |
| `null_syscall` | EL0 | 27 | 27, 27, 27, 27, 28 | one trap and return, for scale |

These agree with [the 2026-07-29 refresh](per-core-and-multi-hart.md) (~50 and ~361) within
run-to-run noise. What was missing was never a measurement; it was the paragraph.

### The comparison, done on the right number

Cycles here are arithmetic, not a reading. HVF passes through no PMU ([notes/pmu.md](../pmu.md)),
so a cycle count is nanoseconds times an assumed clock. The old paragraph assumed 3.2 GHz, which is
not this machine. The host is an Apple M3: 4.05 GHz on a P-core, 2.75 GHz on an E-core, and nothing
pins the QEMU vCPU thread to either. So the range, not a point:

| | ns | cycles at 2.75 GHz | cycles at 4.05 GHz |
|---|---|---|---|
| `ipc_rtt_el0` round trip | 350 | ~960 | ~1,420 |

For the same-core, different-address-space path, seL4 publishes 413 cycles for the IPC call and 426
for the IPC reply, one-way each (sel4.systems/performance.html). The page does not put the machine
up front. It is the Jetson TX1, a Cortex-A57 at 1.9 GHz, in seL4's default configuration, and the
only aarch64 platform the page carries ([notes/aarch64-board-survey.md](../aarch64-board-survey.md),
read 2026-08-13). A round trip in our sense is their call plus their reply, ~839 cycles.

So the corrected figure is roughly 1.1x to 1.7x an L4-lineage round trip, not 4 to 7 times. That
arithmetic crosses silicon. On one core, with seL4 itself booted under the same HVF machine, the
ratio is about 4.6 ([arm-plan-under-hvf.md](../arm-plan-under-hvf.md)), and that measurement is the
one in force.
Converting the debug EL0 number instead (~2272 ns) gives ~6,200 to ~9,200 cycles and ~7x to ~11x.
That is the same class of mistake in the other direction: a debug number in a release comparison.

### Why that is not a win, and what is still not apples-to-apples

- Cortex-A57 is 2015 silicon and the M3 is 2023. A cycle is not a neutral unit across a decade of
  microarchitecture. The M3 is far wider and deeper, and an IPC path is mostly serially dependent
  loads, stores and unpredictable branches, which a bigger out-of-order window helps. **A large part
  of the gap closing is the machine, not the kernel.** The only fix is the same kernel on comparable
  silicon, which milestone 24 (a second aarch64 board) and milestone 74's PMU are for.
- Their fastpath is on and we have none. seL4's published figures are its best case, with the
  `Call`/`ReplyRecv` fastpath enabled. Ours is the fully general path every time: scheduler lock,
  proved rendezvous, generational ThreadId checks. Read the ratio as "the general path is within a small
  factor of a tuned fastpath on this silicon", a statement about headroom, not about having matched
  them.
- Their round trip is two syscalls; the `ipc_rtt_el0` benchmark's is four. seL4 fuses send-and-wait
  into `Call` and reply-and-wait into `ReplyRecv`. Our benchmark issues `SEND`, `RECEIVE`, `SEND`,
  `RECEIVE`. At ~27 ns (~110 cycles) per trap, the two extra crossings are ~220 cycles, a sixth to a
  quarter of our round trip, and self-inflicted rather than structural. The kernel-side
  `call_reply` bench measures the fused shape, but it has no EL0 twin. So the structurally matched
  comparison to seL4's published pair is not measured at all.
  *Corrected 2026-09-04 by milestone 188 (the IPC fastpath): a real service issues `CALL`,
  `RECEIVE_CAP`, `REPLY`, three syscalls to seL4's two, not four. Four is only `ipc_rtt_el0`'s count.
  The residual one is the `ReplyRecv` fusion this tree lacks, a syscall-surface question and
  calef's. See [the fastpath footprint appendix](fastpath-footprint-gate.md).*
- Different measurement methods. seL4 times a single operation through the PMU with the caches hot
  and a measured overhead subtracted. We average a 5,000-iteration loop against a 41.67 ns counter
  tick. Both are legitimate ([notes/pmu.md](../pmu.md), the two-clocks section), and they do not
  fail the same way.
- We run under a hypervisor and they do not. The tax on this path is small: no devices touched, so
  essentially no VM exits. The cost is indirect, via stage-2 TLB pressure and host cache pollution,
  which is why the bench loops keep devices out. Small is not zero.
- The clock is assumed. Every cycle figure above inherits a ~1.5x uncertainty from not knowing which
  core type the vCPU thread ran on. Milestone 74 (cycle counters) exists to retire this. Until it
  lands, quote no cycle ratio from these notes tighter than "same order".
- Both sides will be measured in a benchmarking build, and ours is the slower one (milestone 237 (the cycle-counter grant),
  which made it a measurement build). Reading `PMCCNTR_EL0` at EL0 needs milestone
  229's per-thread grant, which the context switch enforces. That grant sits behind
  `--features cycle_counter_grant`, not in the shipping kernel. Measured, it costs
  `sched::schedule` 136 bytes, taking `script/fastpath-footprint`'s aarch64 `ipc_fastpath` closure
  from 5852 to 5988. So any cycle figure taken with the instrument on is slightly pessimistic about
  nife, the right direction to err for a comparison we intend to publish.

  Milestone 221 (a soak that crosses cores) records the same rule: a soak number compares only with
  another soak number. It does not cut against us here, because seL4's published 413 and 426 come
  from a benchmarking build too. `sel4bench` reads `PMCCNTR_EL0` from user level, which on Arm
  needs `KernelArmExportPMUUser`. seL4's configuration reference describes that option as *"Grant
  user access to the performance monitoring unit. While useful for benchmarking, this option opens
  the possibility of timing channels"*, default `OFF`
  (docs.sel4.systems/projects/sel4/configurations.html, read 2026-09-03). Our benchmark kernel
  against theirs is like for like; our production kernel against their benchmark one would not be.

The kernel-side number stays. `ipc_rtt` at 46 ns (~130 to ~190 cycles) is the cost of the kernel's
own rendezvous. It is the right instrument for its job: the icount tripwire gates on it, and a
change to the kernel's IPC code moves it next to its commit. It is not a comparison number. Beside an
839-cycle round trip it would compare a path with no trap and no address-space switch against one
with both, the original error. Deleting it would be a second error: the gate needs it, and the
~300 ns gap between the two planes is the trap cost.

What the comparison supports: the Mach failure mode is nowhere in sight, and the general path is
viable at this price. Whether a fastpath is ever worth its complexity is a question for these
measurements, not for L4 envy.

### Where the two nanosecond figures for `ipc_rtt` came from

The note carried ~705 ns (2026-07-23) and ~951 ns (2026-07-25) for the same kernel-side benchmark.
That reads as a contradiction and is not one. Both are debug single runs of different binaries on
different days. The 19f object-capability refactor landed between them and grew the scheduler and
thread hot paths, on top of the whole-crate codegen drift in
[the icount drift appendix](icount-drift-and-provenance.md). The defect was leaving both on the page
with no dates and no build profile, which let a comparison quote the older one for eleven days.
Every figure now carries its build.

### seL4: built and booting, but stopped by the PMU wall (deferred to real hardware)

`sel4bench` was built: the seL4 kernel plus the benchmark app suite, for `qemu-arm-virt` aarch64,
with `RELEASE` and `FASTPATH` on, i.e. seL4 at its best. It boots on this Mac under both QEMU-TCG and
QEMU-HVF. It cannot produce valid numbers here, for the same constraint the roadmap named for our
own silicon-cycle plans.

sel4bench times a single operation per sample (one `seL4_Call`, `RUNS` samples). It reads the PMU
cycle counter, `PMCCNTR_EL0`, before and after ([notes/pmu.md](../pmu.md) explains why that counter
does not survive virtualization). That needs a real, high-resolution cycle counter (~0.25 ns per
tick at ~4 GHz). Neither virtualization mode on this host provides one:

- QEMU-TCG does not model a cycle counter. `PMCCNTR` returns quantized junk (we saw 0 and 1000), and
  sel4bench's stability check refuses to continue ("*Benchmarking overhead of a call is not
  stable*"). Milestone 74's aarch64 half measured the same from this kernel's side on 2026-09-19,
  once it started the counter. Without `-icount` it advances in steps of 1000, about 32 per
  `CNTVCT_EL0` tick. Under `-icount` it is the instruction count, exactly 16 per tick, which is
  `script/icount`'s `instructions_per_counter_tick`. Neither is a cycle; see
  [notes/pmu.md](../pmu.md).
- QEMU-HVF on Apple Silicon does not virtualize the guest PMU, so `PMCCNTR` is unstable there too,
  and the same check stops the run.

The only counter HVF passes through is the architected virtual counter, `CNTVCT_EL0`, at the host's
24 MHz `CNTFRQ`: 41 ns per tick, far too coarse for one ~50 ns IPC in a single shot.
`CONFIG_ALLOW_UNSTABLE_OVERHEAD` forces sel4bench past the check, but the numbers are the same junk.

Our bench works under HVF for exactly the reason sel4bench does not. We read `CNTVCT`, which HVF
passes through, and time a loop of thousands of operations per sample, so the 41 ns tick averages
away. A same-machine seL4 number would need either a rewrite of sel4bench to our method (CNTVCT plus
batched loops, real surgery on its measurement core) or a real PMU.

So the seL4 comparison is deferred to real hardware. The seL4 machine is argon, the Jetson TX1 of
seL4's own published figures. Milestone 127 (the seL4 machine) tracks it; see
[notes/bench-runbook.md](../bench-runbook.md). The build recipe, via the official
seL4 Podman image, with the board's `PLATFORM` in place of `qemu-arm-virt`:

```
podman pull docker.io/trustworthysystems/sel4        # ~3.6 GB, bundles repo/cmake/ninja/aarch64-gcc
mkdir sel4bench && cd sel4bench
podman run --rm -v "$PWD":/sel4bench:Z docker.io/trustworthysystems/sel4 bash -lc '
  cd /sel4bench
  repo init -u https://github.com/seL4/sel4bench-manifest.git && repo sync -j4
  mkdir build && cd build
  ../init-build.sh -DPLATFORM=qemu-arm-virt -DAARCH64=TRUE -DSIMULATION=TRUE   # -DPLATFORM=rpi4 for a Pi
  ninja'
# image at build/images/sel4benchapp-image-arm-qemu-arm-virt; run with build/simulate (qemu) or on the Pi
```

### The cross-OS comparison, when we build it

- Reuse an existing primitive suite where one exists: lmbench on Linux and macOS (it builds on
  both), `sel4bench` for seL4. We write the nife side (the microbenchmarks in
  [notes/benchmarks.md](../benchmarks.md), extended to match the metric set), not the whole thing.
- The peers. seL4 is the direct one: a capability microkernel that targets the same QEMU
  `aarch64 virt` machine we do, so it runs on the identical instrument (QEMU-HVF) and publishes
  comparable cycle counts. L4Re/Fiasco and Genode are more effort for less insight.
- Match the virtualization tier. QEMU with `-accel hvf` is virtualization (Hypervisor.framework on
  the real core), not emulation, so nife and Linux run virtualized under QEMU-HVF. macOS runs as a
  guest under Apple's Virtualization.framework: same hypervisor, different VMM shell. Native macOS
  is the bare-metal ceiling. For guest-internal microbenchmarks the VMM layer is off the hot path (no
  VM exit on a null syscall or context switch), so the QEMU-vs-VZ difference is a footnote.
- XNU is a hybrid; name it. macOS's kernel has a Mach core but runs BSD and drivers in the kernel,
  so most macOS syscalls are in-kernel BSD calls. Mach IPC is not on its hot path the way our
  endpoints are on ours. "Our IPC" against "macOS syscall latency" measures two different things.
