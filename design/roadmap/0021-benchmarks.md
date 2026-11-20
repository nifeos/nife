---
status: BUILT
raised: 2026-07-23
built: 2026-07-23
---
# 21. Performance measurement: benchmarks with teeth

In brief. icount microbenchmarks + committed baseline that fails on regression; HVF-native runs for real magnitudes

Why it matters. perf claims become measurements; regressions surface next to their cause. **Built**; notes/benchmarks.md

Added 2026-07-23, prompted by milestone 15 shipping a performance win nothing measures. The
requirement, stated by calef: identify performance issues, and identify the *introduction* of
performance problems proximate to the changes that introduce them.

Deliverable. In-kernel microbenchmarks over the paths a microkernel lives on (IPC round-trip,
call/reply, context switch, spawn-to-reap, untyped map, null system call), run under QEMU `-icount` so
virtual time is a deterministic function of instructions executed. A `script/bench` entry point
separate from `script/test`, and a committed baseline that `script/bench --check`
diffs against, failing loudly on regression. Updating the baseline is a deliberate act in the
same commit that changes performance, so the baseline file's git history *is* the performance
record, each delta next to its cause.

**Two instruments, because one cannot do both jobs.**

1. icount (TCG): the regression teeth. Deterministic instruction counts, tight thresholds,
   the committed baseline, commit-gating. Catches path-length regressions (an extra lock, an
   accidental O(n), a flush creeping back). Models no caches and no TLB, so magnitudes are
   fiction; the counts are the point.
2. HVF: the real magnitudes. On this host (Apple Silicon), `-accel hvf` runs the kernel
   natively under Hypervisor.framework: real caches, real TLBs, `CNTVCT_EL0` at the hardware's
   24 MHz. `script/bench --real` reports medians over repeated runs with loose bounds, not
   gates: it is a real machine shared with a desktop OS, so the numbers are statistical.
   This is where milestone 15's flush removal finally gets measured (an A/B flag restoring the
   old `vmalle1is` quantifies it), and it is the aarch64-on-aarch64 coincidence paying off.

Known limits: device-touching paths carry virtualization overhead under HVF (MMIO traps to the VMM).
The PMU is not virtualized (cycle-exact counters wait for the silicon of milestone 16 (real
hardware + IOMMU-backed driver isolation), which inherits
this harness and swaps the clock). The first thing to validate is that QEMU's
semihosting test-exit works under HVF at all; if not, the bench build reports over virtio
instead.

## Follow-on

- **Milestone 74.** Cycle-exact counters. HVF passes no PMU through, so `--real` reports the
  architected virtual counter and derives cycles by arithmetic. Milestone 74 is the SBI PMU half on
  riscv64 and the `PMCCNTR_EL0` half on aarch64, raised from an audit of what milestone 16a needs,
  and it is what swaps the clock this harness reads.
- **Recorded.** In `design/roadmap/0021-benchmarks.md`, where a reader meets the instrument:
  device-touching paths carry virtualization overhead under HVF, because MMIO traps to the VMM. So
  `--real` magnitudes for anything that touches a device are not the host's own numbers.
- **Recorded.** In `notes/benchmarks.md`, beside the numbers it qualifies. The semihosting question
  this block said to validate first was answered, and the answer was no. `hlt #0xf000` traps back to
  the guest under HVF, so the bench kernel never exits and the leg takes its verdict from the
  transcript instead.

## Index row

perf claims become measurements, and regressions surface next to their cause
