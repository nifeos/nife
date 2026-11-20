---
status: DECIDED
raised: 2026-08-25
decided: 2026-08-25
ratified_by: calef
---

# 127. x86_64's `now()`/`cntfrq()`: PIT-calibrated `rdtsc`, ratifying what PR #476 already built

Ratifying an implementation already built, tested, and merged (PR #476,
2026-08-25) rather than proposing something new. Surfaced while surveying PARTIAL milestones for
genuine open decisions: [milestone 161](../roadmap/0161-x86-64-kernel-port.md) named this as "a
design fork" awaiting calef's call, but the fork was answered in code before it was answered in
this file. Recorded here so the decision has a home a reader can find, per this tree's own
ladder: a fact that exists only at a call site is rung zero.

## The question

`crates/user_rt` needs an x86_64 arm for `now()` and `cntfrq()`. aarch64 reads `CNTVCT_EL0`/
`CNTFRQ_EL0`; riscv64 reads the `time` CSR. Both are architected, self-describing timer registers.
x86 has neither: `rdtsc` reads a cycle counter, but nothing in the ISA says what rate it runs at.

## The answer: PIT-calibrated `rdtsc`, frequency delivered via a mapped page

**Calibration** (`kernel/src/arch/x86_64/timer.rs`): a single 10ms window, timed by polling PIT
channel 2 (1,193,182 Hz, fixed since the 8254's introduction in 1981). That channel was chosen
because it is the only channel with a software-controlled gate and a pollable output, so it needs no
interrupts and works before IRQs exist. Across that window the kernel reads both the TSC delta and
the local APIC timer's delta simultaneously (`measure_against_the_pit`), so the two calibrations
either agree or the disagreement is visible rather than silently trusted. `now()` is a bare `rdtsc`
read, no fence: the 10ms calibration window is coarse enough that reordering noise does not matter
at the call site.

The `cntfrq()` contract matches the other two architectures' own shape rather than inventing one.
aarch64 and riscv64 both answer it as an ambient, no-system-call read (a system register on aarch64; a
hardcoded QEMU constant on riscv64). That constant's own doc comment already predicts its eventual
real-hardware answer is "an aux-vector entry, the way Linux passes `AT_HWCAP`", since real riscv64
silicon has no register either. x86_64's answer, a kernel-computed frequency delivered to userspace
through a mapped `TimebasePage`, is that same aux-vector shape, not a new pattern: riscv64's own
comment already named where this was going.

## What was checked before ratifying, not assumed

- **Invariant TSC.** The timer module's own `BUGS` section is honest that this is assumed rather
  than checked: "QEMU's TSC is invariant. A real machine must have the bit checked
  (CPUID 0x80000007 EDX[8]), and milestone 87's is where that gets tested rather than argued."
  Correctly deferred to real-hardware bring-up rather than blocking the QEMU-only path this
  project runs under today (TCG, per this tree's own `.qemu-version` pin; no KVM/HVF on the dev
  machine per AGENTS.md's environment notes).
- Calibration precision, also self-documented rather than glossed: "a single 10ms window on a
  busy host under TCG can be off by a per cent or so." Named where the reader meets it, not a
  blocker.
- Prior art. Linux's `arch/x86/kernel/tsc.c` calibrates the TSC against the PIT (or HPET, or
  the ACPI PM timer, with the PIT as the universal fallback when neither is available) in the same
  shape. PIT-calibrated invariant TSC is the standard answer to this problem, not an idiosyncratic
  choice.

## What this does not decide

[Milestone 167](../roadmap/0167-timebase-page-delegation.md), handing a computed page to a
userspace-built child rather than the core calibration mechanism this entry ratifies, remains its
own, separately-scoped, unbuilt piece. This entry closes the *architectural* fork (what
`now()`/`cntfrq()` compute and how); it does not close 167's own delegation gap.
