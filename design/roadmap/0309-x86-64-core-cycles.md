---
status: BUILT
raised: 2026-09-16
built: 2026-09-17
promoted_from: cycles-per-tick-on-the-other-two-architectures
---
# 309. Unhalted core cycles on `x86_64`, so `cycles_per_tick` means the same thing on two architectures

Promoted from
`design/roadmap/proposals/cycles-per-tick-on-the-other-two-architectures.md`, whose central
recommendation is corrected here rather than implemented. Built by a lane on
`milestone/309-x86-64-core-cycles`.
*(Number provisional until the merge queue lands it.)*

Nothing gated building it, and that was the proposal's own reading: the counter is
architectural and this kernel already had every mechanism it needs (`CPUID`, `rdmsr`, `wrmsr`). What
no machine here has is one that *models* it. QEMU-TCG reports no architectural performance
monitoring at all, so the only outcome this probe can print on the merge machine is `unavailable`,
honestly, and `xenon` is where a number comes from. That is a caveat on the reading rather than a
gate on the work, which is why it is in BUGS below and not in a status line.

## The measurement, first, because it is the deliverable

QEMU, `-cpu max` (`script/bench --x86`):

```
  probe: cycles_per_tick unavailable (NoPerfmonLeaf)
```

**and on the boot line of every `x86_64` boot, bench or not:**

```
  cycles      : no architectural performance monitoring (cpuid leaf 0x0a); TSC ticks only
```

`-cpu max,pmu=on` prints exactly the same two lines. The property is a KVM one; under TCG QEMU
zeroes `CPUID` leaf `0x0A` either way, which is worth recording because it is the obvious next thing
a reader would try. `script/test --arch x86_64 --cpu max,pmu=on`: 241 passed, 41 skipped.

That is the result, and it is a real one rather than a shortfall. The emulator models no
performance monitoring unit, this kernel asked, was told no, and said so in the place a reader meets
the number. The alternative outcome, a probe that printed something, is the failure this milestone
exists to prevent.

## Why the proposal's recommendation is wrong, which is the part most likely to be re-derived

The proposal's closing paragraph says: *"Build the x86_64 probe now with the TSC and label the line
for what it measures."* **Do not.** It also, to its credit, names the alternative and calls it "the
design fork inside this option"; the fork has an answer and the answer is not the TSC.

`kernel/src/arch/x86_64/timer.rs`'s `now()` is `rdtsc`. One line, checked rather than recalled:

```rust
pub fn now() -> u64 {
    rdtsc()
}
```

So a `cycles_per_tick` probe on this architecture built the proposal's way would read the TSC at
both ends of the window and divide it by itself. It would print exactly `1.00`, on QEMU, on
`xenon`, and on every part ever made. That is milestone 16a's "implausibly exact 100.00" again, with
one difference that makes it worse: 16a's artifact is *emulated* (one virtual clock driving two
CSRs) and goes away on silicon, and this one is structural and never would.

The other two architectures do not have the problem, because on both of them the OS clock and the
cycle counter are different hardware. Verified in this tree rather than assumed:

| | `arch::timer::now()` | the cycle counter | independent? |
|---|---|---|---|
| aarch64 | `CNTVCT_EL0` (`now` in `kernel/src/arch/aarch64/timer.rs`) | `PMCCNTR_EL0` | yes |
| riscv64 | `rdtime` (`now` in `kernel/src/arch/riscv64/timer.rs`) | `mcycle`/`hpmcounterN` via SBI PMU | yes |
| x86_64 | `rdtsc` (`now` in `kernel/src/arch/x86_64/timer.rs`) | `IA32_PERF_FIXED_CTR1` | **only with this milestone** |

**The proposal's own caveat was right and its conclusion did not follow from it.** It says plainly
that the TSC "is not a cycle counter in the sense riscv64's is" and that the difference "has to be
in the printed line". Both true. What it missed is that on this architecture the TSC is not merely a
*different* quantity from cycles, it is the *same* quantity as the denominator, so there is no ratio
left to print. Labeling cannot rescue a number that is 1.00 by construction.

## What was built instead

`kernel/src/arch/x86_64/pmu.rs`, following `arch/riscv64/pmu.rs`'s shape because that module already
solved this problem once:

- `IA32_PERF_FIXED_CTR1`, architectural fixed-function counter 1, `CPU_CLK_UNHALTED.CORE`. That
  is the same quantity riscv64's SBI PMU `CPU_CYCLES` and aarch64's `PMCCNTR_EL0` give, so the three
  architectures now report one thing under one name.
- Enabled through `IA32_FIXED_CTR_CTRL` (counter 1's field, both rings) and
  `IA32_PERF_GLOBAL_CTRL` (bit 33), read-modify-written so firmware's other bits survive.
- Gated on `CPUID` leaf `0x0A` before any MSR is touched, because `rdmsr` on an unimplemented
  MSR is a `#GP` and this kernel has no recovery path for a probe that could have asked first. Same
  discipline `isa::draw_rdseed` keeps for `RDSEED` one file over. This is what makes the QEMU
  outcome a printed line rather than a dead machine.
- An outcome enum with five ways of saying no, each with its own boot line:
  `NoPerfmonLeaf`, `NoFixedCounter`, `Stuck`, `InStepWithTheTsc`, `Running`.
- No fallback to the TSC, anywhere. `cycles()` answers `None` and the caller prints the reason.
  A probe that quietly degraded would reproduce the exact artifact above, in a line somebody would
  later quote as a fact about a machine.

`kernel/src/bench.rs`'s `cycles_per_tick` grew an `x86_64` arm (it was `#[cfg(target_arch =
"riscv64")]`), keeping the riscv64 arm's numeric format and its `unavailable` behaviour byte for
byte. `bench/baseline-*.txt` readers meet both, and milestone 74 (cycle counters)'s block records a riscv64 reading
against the existing window of 100,000 ticks. So that window stayed a constant rather than becoming
`frequency()/100` like x86's.

## Plausibility, not just presence, and what each case prints

A counter can be present and useless, which is how `arch::riscv64::pmu` came to have a `Stuck`
member at all (OpenSBI handed it `hpmcounter3` and QEMU models that as a constant zero). Two ways
that shape is available here, and both are refused with a distinct reason:

| what happened | outcome | what the boot prints |
|---|---|---|
| leaf `0x0A` absent, or version 0 | `NoPerfmonLeaf` | `no architectural performance monitoring (cpuid leaf 0x0a); TSC ticks only` |
| version < 2, or fewer than two fixed counters | `NoFixedCounter` | `perfmon vN has no fixed counter 1; TSC ticks only` |
| enabled, read twice across a timed spin, unmoved | `Stuck` | `enabled but did not advance over N TSC ticks; refused` |
| advanced by bit-exactly what the TSC advanced by | `InStepWithTheTsc` | `advanced C against T TSC ticks, identically; refused (it is the TSC)` |
| implemented, enabled, moving on its own | `Running` | `IA32_PERF_FIXED_CTR1 (unhalted core cycles), N bits, perfmon vN` |

`bench::cycles_per_tick` prints `bench-probe: cycles_per_tick unavailable ({outcome:?})` for the
first four and a number for the last, which is riscv64's existing contract unchanged.

## What the printed line says, and why there are two of them

A rate printed under one name on three machines will be compared across them, and that is a fact
that leaves the machine (AGENTS.md's *move fast on what can be undone*). So the meaning is printed
beside the number rather than left to this file:

```
bench-probe: cycles_per_tick 100.00 (10000189 cycles over 100000 ticks at cntfrq 10000000)
bench-probe: cycles_per_tick_means core cycles (SBI PMU CPU_CYCLES) per tick of the `time` CSR, a fixed-rate timebase the device tree states
```

and on `x86_64` the second line reads:

> unhalted core cycles (`IA32_PERF_FIXED_CTR1`) per TSC tick; the TSC is constant-rate and core
> cycles are not, so this ratio moves with frequency scaling and turbo

That difference is information rather than noise, and it is the whole reason the TSC alone cannot
answer this. On riscv64 the denominator is a timebase the device tree states (10 MHz on QEMU
`virt`, 4 MHz on radon's JH7110) and the ratio is a property of the core's clock. On `x86_64` the
denominator is constant-rate and the numerator is not, so a machine that scales frequency or turbos
prints a *different* ratio at a different load, correctly.

The second line is printed on riscv64 too, which is a change to an existing architecture's
output and deliberate. Levelling up rather than down: milestone 74's block is largely an account of
how badly a tick-versus-cycle confusion has already gone here, and a reader of radon's transcript
benefits from the same sentence. `bench-probe:` lines are echoed by `xtask` and never enter a
baseline, so nothing gates on either. riscv64's numeric line is untouched: `cycles_per_tick
100.00` under QEMU, the same value milestone 74 recorded.

## Parity, and what is deliberately not here

DECISIONS §19 is satisfied by construction and the scope note is the aarch64 half. Everything
added under `kernel/src/arch/x86_64/` is `x86_64` code by definition; `script/test` is green on all
three architectures (below). What §19 wants said out loud is the gap:

**aarch64 is ordered behind milestone 74's own aarch64 half and is not in this milestone.**
`PMCCNTR_EL0` reads zero until `PMCR_EL0.E` and `PMCNTENSET_EL0.C` are written, which this kernel
never does; `design/roadmap/0353-the-aarch64-half-of-74.md` covers exactly that and this
milestone would duplicate its first half to reach its own second. So `cycles_per_tick` is two
architectures of three after this, up from one, and the third has a named owner rather than a
silence.

**No authority question, and this must not create one.**
[DECISIONS §139 part 3](../decisions/0139-cycle-counter-authority.md) records that `x86_64`'s TSC is
already ambient (`CR4.TSD` clear at reset, never written by this kernel), which is why the negative
half of `a_granted_thread_reads_the_cycle_counter_and_an_ungranted_one_faults` skips here with a
reason. **This milestone does not change that.** `IA32_PERF_FIXED_CTR1` is read with `rdmsr`, ring 0
only; `CR4.PCE` stays clear, so `rdpmc` from ring 3 still faults and there is no new ambient counter
to grant or revoke. Counter 1 is enabled to count in both rings, which is what makes the quantity
match the other two architectures'; counting in ring 3 is not reading from ring 3.

## BUGS

- **This counter has never been read on silicon, and every outcome above is a fact about QEMU.**
  `xenon` has booted nife: on 2026-09-04, under its own UEFI firmware, it printed the tour and
  stopped in the mapper at `mmu.rs`'s `AlreadyMapped`, which is before anything this module touches.
  That cause is diagnosed and fixed on `main` and the next boot resumes one line further on
  (notes/x86-uefi-boot.md has the session, notes/xenon-firmware.md the firmware settings). So the
  honest caveat is about the counter, not about the machine. Until that boot happens, the four
  refusal paths are untested against a part that actually implements the leaf: QEMU takes the
  first branch and returns, so `NoFixedCounter`, `Stuck`, `InStepWithTheTsc` and `Running` are
  reasoned code rather than measured code.
- The in-step check is bit-exact equality, and that is a deliberate under-detection. A real core
  pegged at exactly its base frequency has core cycles and TSC ticks at the same *rate*, so an
  approximate band would refuse a legitimate counter on a legitimate machine. Two independent
  counters read by two different instructions a few tens of cycles apart do not produce equal deltas
  over a ten-millisecond window; one counter read twice does. A hypervisor that aliased the two
  *with an offset* would pass this check and print `1.00`. The printed meaning line is the defense
  that does not depend on a heuristic.
- The boot CPU only. These MSRs are per-logical-processor, `init` runs once, and a secondary
  that never ran it has fixed counter 1 disabled and would read zero. The one consumer today is a
  single-threaded bench probe. A per-CPU record is real work and belongs with a caller that needs
  it.
- **The counter is never stopped**, which is correct for a free-running counter only ever read as a
  difference, and is the same call `arch::riscv64::pmu` makes.
- **`IA32_PERF_GLOBAL_CTRL` is read-modify-written, not assigned.** Nothing else in this kernel
  programs a performance counter, so the preserved bits are firmware's. A second consumer would have
  to raise the arbitration; this module does not assume one.
- No `--check` tolerance and no baseline row, by the same reasoning riscv64's probe carries: it
  is a rate rather than a duration. A 10% gate would fail on any machine that scales frequency,
  which is every machine this is interesting on.

## What xenon's first bench boot has to do, and what it will answer

One file, `target/esp/EFI/BOOT/BOOTX64.EFI`, written by `cargo xtask uefi-image`;
`notes/x86-uefi-boot.md`'s "The bench" section is the procedure. That boot is calef's and is not a
lane's. What it decides, in order:

1. Whether the boot line says `IA32_PERF_FIXED_CTR1 ... perfmon vN` at all, which is the first time
   anything in this tree learns whether the 7050's part answers leaf `0x0A` the way the SDM says.
2. Whether the probe prints a number or one of the three remaining refusals. All four are acceptable
   outcomes of this milestone; a plausible-looking wrong number is not.
3. If it prints a number: what `cycles_per_tick` is on that part, which converts every
   tick-denominated row of an `x86_64` board bench into cycles at once, and gives milestone 25's
   cross-OS comparison and [§96](../decisions/0096-process-kernel-or-event-kernel.md) the currency the
   literature is denominated in.

## Names

**Provisional** (AGENTS.md: names are an architect's). `arch::x86_64::pmu` takes riscv64's module
name and is the only one of them a reader meets by path. The rest are this lane's: `CycleCounter`
and its five members (`NoPerfmonLeaf`, `NoFixedCounter`, `Stuck`, `InStepWithTheTsc`, `Running`, the
first three of which mirror riscv64's vocabulary), `elapsed_cycles`, `cycle_counter_width`, and in
`bench.rs` the three `cycle_probe_*` helpers and the printed token `cycles_per_tick_means`. That
last one is the one to look at hardest, because it is the only one that leaves the machine.

## Gates

| gate | result |
|---|---|
| `script/lint` | 0 |
| `script/test` (aarch64) | green |
| `script/test --arch riscv64` | green |
| `script/test --arch x86_64` | green, 241 passed / 41 skipped, four new `arch::x86_64::pmu` tests |
| `script/bench --x86` | `cycles_per_tick unavailable (NoPerfmonLeaf)` |
| `script/bench --riscv` | `cycles_per_tick 100.00`, unchanged, plus the new meaning line |

## Follow-on

- **Done.** `bench::cycles_per_tick` is two architectures of three, up from one. Milestone 74's
  measurement half, which `design/roadmap/0074-cycle-counters.md` records as a DECISIONS §19 scope
  gap, is that much smaller; the gap itself is not closed and the bullet below says who owns it.
- **Milestone 353.** The aarch64 half, where it already was: `PMCCNTR_EL0` reads zero until
  `PMCR_EL0.E`
  and `PMCNTENSET_EL0.C` are written, and this milestone would have had to build that proposal's
  first half to reach its own second. Ordered behind it rather than duplicated into it.
- **Recorded.** That this counter has never been read on silicon is a limitation stated where a
  reader meets the feature: `kernel/src/arch/x86_64/pmu.rs`'s own `BUGS` section, and this block's.
  Four of the five outcome paths are reasoned rather than measured, because QEMU takes the first
  branch and returns before any of them.
- **Recorded.** That the in-step-with-the-TSC check is bit-exact equality, and therefore misses an
  aliased counter carrying an offset, is beside the check in `kernel/src/arch/x86_64/pmu.rs`'s
  `BUGS`. The defense that does not depend on the heuristic is the printed meaning line, which is
  also recorded there.
- **Done.** An edit outside this lane's own block, named here because AGENTS.md says a lane edits
  its own roadmap block and only that. `design/roadmap/0074-cycle-counters.md`'s §19 scope
  note cited `design/roadmap/proposals/cycles-per-tick-on-the-other-two-architectures.md`, which
  this merge deletes, and asserted that the measurement half "is currently one" architecture and
  that x86_64 could be built on the `rdtsc` read already in `arch/x86_64/timer.rs`. All three
  clauses are falsified by this milestone: the file is gone, the count is two, and that read is the
  one that cannot work. A dangling path and a stale §19 scope note are the §76 failure this tree
  has already recorded once. So the paragraph is corrected rather than left, minimally and in
  place, keeping the wrong reasoning visible as the thing a reader would otherwise re-derive. The
  integrator should look at it as a separate change from the code.
- **Done.** `xenon`'s first reading needs no further code from a lane: the probe is already in the
  image `cargo xtask uefi-image` writes, and what remains is one boot, which is calef's rather than
  a lane's. This block's last section says what that boot decides.

## Index row

`kernel/src/bench.rs`'s `cycles_per_tick`, the one number that converts a whole tick-denominated
board bench into cycles, existed on one of three architectures. The proposal that raised this said to
build the `x86_64` half with the TSC, and that recommendation is wrong in a way worth keeping on
the record. `arch::x86_64::timer::now()` *is* `rdtsc`. So such a probe would divide one counter by
itself and print an exact `1.00` on every part ever made, which is milestone 16a's "implausibly exact
100.00" made structural rather than emulated. Built instead on `IA32_PERF_FIXED_CTR1`, unhalted core
cycles, the same quantity riscv64's SBI PMU `CPU_CYCLES` and aarch64's `PMCCNTR_EL0` give, so two
architectures now report one thing under one name and each prints what its own ratio is a ratio of.
Measured under QEMU: `cycles_per_tick unavailable (NoPerfmonLeaf)`, with `-cpu max,pmu=on`
identical, because TCG zeroes `CPUID` leaf `0x0A`; the `CPUID` gate is what makes that a printed line
rather than a `#GP`. Presence is not enough, so a counter stuck at zero and one advancing bit-exactly
with the TSC are each refused with their own reason, and there is no fallback to the TSC anywhere.
aarch64 is ordered behind milestone 74's own aarch64 half.
