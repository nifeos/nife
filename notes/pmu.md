# The PMU, and the two clocks in an aarch64 core

The Performance Monitoring Unit (PMU) is counting hardware built into the CPU core, separate from
the part that runs instructions. Its job is to tally low-level events as the core executes: clock
cycles, instructions retired, cache misses, branch mispredictions, TLB misses, and dozens more. It is
the core keeping score on itself.

Picture a bank of hardware counters wired into the pipeline. Each can be told "count event 0x08"
(instructions retired) or "count event 0x03" (L1 data cache misses), and it ticks every time that
event fires, at hardware speed, with no software in the loop. This is the machinery behind `perf` on
Linux and Instruments on macOS.

## The counter that matters here: the cycle counter

aarch64 gives the PMU one always-present counter, `PMCCNTR_EL0`, that counts CPU clock cycles. On
an Apple M-series core near 4 GHz that is one tick roughly every 0.25 ns. You read it with a single
`mrs` (move-from-system-register), which itself costs only a handful of cycles.

That resolution and that cheapness are why cycle-accurate microbenchmarks reach for it. To time one
operation:

```
t0 = read PMCCNTR
do_the_thing()        // one syscall, one IPC, one page map
t1 = read PMCCNTR
cost = t1 - t0        // cycles, resolvable to nearly a single cycle
```

A whole seL4 IPC is a few hundred cycles, so a single-shot measurement of it *needs* this resolution.
seL4's one published aarch64 platform measures 413 for the call and 426 for the reply, one-way
each; notes/benchmarks.md compares against that pair and notes/aarch64-board-survey.md says which
machine it is. At ~0.25 ns per cycle, one operation of that size is unresolvable by a 41 ns tick.
That is exactly how `sel4bench` works (notes/benchmarks.md), and exactly why it could not run on this
Mac.

## Two clocks, and why the difference is the whole story

An aarch64 core has two unrelated counters, and confusing them is a category error:

| | PMU cycle counter (`PMCCNTR_EL0`) | Generic timer (`CNTVCT_EL0` / `CNTPCT_EL0`) |
|---|---|---|
| counts | CPU clock cycles | a fixed reference tick |
| rate | the CPU clock (~4 GHz), and it varies with frequency scaling | fixed, advertised in `CNTFRQ_EL0` (24 MHz here) |
| resolution | ~0.25 ns | ~41 ns |
| what it is for | profiling, microbenchmarks | wall-clock timekeeping |

The generic timer is the OS's clock: a steady reference tick used to tell time and schedule
deadlines (`CNTPCT`, `CNTP_CVAL`; see interrupts.md). It is what our own bench reads, through
`user_mode_runtime::now` at EL0 (abi.md opened `CNTKCTL_EL1.EL0VCTEN` for exactly this). It is coarse, ~41 ns per
tick, so one IPC reads as "1 tick, maybe 2." We beat the coarseness by timing a loop of thousands
of operations and dividing; the per-op cost falls out cleanly and the tick noise averages away.

The PMU cycle counter is the opposite trade: fine enough to time a single operation, but it counts
*cycles*, not time. The cycle rate moves with clock scaling (DVFS), so turning cycles into
nanoseconds needs the current frequency, which is not fixed.

Two ways to measure a fast operation, then: one shot at high resolution (PMU, sel4bench) or a
long loop at low resolution (generic timer, ours). Both are valid; they fail under different
conditions.

## Why virtualization keeps the PMU out of reach

The generic timer is *architected* state, part of the CPU's published contract, so a hypervisor is
expected to pass it through, and Apple's HVF does: `CNTVCT` reads work fine inside a guest. That is why
our bench runs under QEMU-HVF at all.

The PMU is different. It is microarchitectural, core-private, and awkward to expose safely: it can leak
information across VM boundaries, and it is real work to save and restore across guest switches. So it
is commonly left unvirtualized:

- QEMU-TCG (pure emulation, our deterministic `icount` mode) has no real cycles to count, it just
  translates code, so `PMCCNTR` returns quantized junk (we saw 0 and 1000). Measured again on
  2026-09-19 once this kernel started the counter (below): without `-icount` it moves in steps of
  1000, about 32 per generic-timer tick; under `-icount` it *is* the instruction count, 16 per tick at
  QEMU's 62.5 MHz `CNTFRQ`. Both move, so both pass the kernel's did-it-move check, and neither is a
  cycle.
- Apple HVF does not virtualize the guest PMU, so a guest's `PMCCNTR` reads are unstable.

Either way a single-shot cycle measurement has no usable clock. This is why `sel4bench` (single-shot,
PMU) cannot produce numbers on this Mac while our bench (long loop, generic timer) can. It is the
same constraint design/roadmap/0074-cycle-counters.md flags for any plan that wants real cycle
counts: they wait on real
silicon. A Raspberry Pi has a real PMU because it *is* real silicon, not a guest, which is where the
seL4 comparison goes (milestone 24).

The lesson worth keeping: **the coarse, boring generic timer is the one that survives virtualization.**
Choosing it plus long loops, back at milestone 19e, is what makes our cross-OS numbers possible on a
laptop instead of only on hardware. See notes/benchmarks.md for how the two instruments are used, and
notes/abi.md for how EL0 got read access to the generic timer.

## What the userspace counter tests assert

Two kernel tests check what a userspace program learns about time. Both live in
`system_tests/src/user/`, and both compare against the kernel's own statement rather than a
constant.

`counter_frequency_tests` checks the counter frequency. Under QEMU `virt` a hardcoded 10 MHz is
the right answer on riscv64, so a check like `freq > 0` passes it. On a board whose timebase is
4 MHz, the VisionFive 2's, the same constant makes every userspace duration 2.5 times too long. So
the test compares the number a program reports with the one the kernel read from the machine.
The vehicle is `coremark`, the one program that reports its own `cntfrq` over IPC. The kernel
spawns `coremark_builder` with `spawn_elf_child_builder`, which maps the timebase page on the
architectures that need one. `coremark_builder` then builds `coremark` through
`supervision_protocol::build_child_space`, the tree's one userspace ELF loader. The number the test
asserts on has crossed both hops. The second test checks that this machine's rate is inside
`counter_frequency_contract`'s plausibility band, the half its host tests cannot reach.

`cycle_counter_tests` checks the cycle-counter grant of milestone 229. The grant goes through a
`#[cfg(test)]` back door because no syscall method sets it. A method number is irreversible, and
milestone 147 (a profiler that holds exactly the counters it was granted) may subsume this one.
`sched`'s own test `a_running_thread_cannot_be_granted_the_cycle_counter` covers the rule that only
an embryo may be granted. The program reports the `CycleMeaning` that
`user_mode_runtime::cycle_reading` handed it, and the test states the expected meaning per
architecture itself. A runtime that labeled aarch64's count without the kernel's filter policy
behind it fails there. The difference of two reads is asserted only where the kernel has said the
counter runs, and the helper beside the test gives each architecture's reason. The same file runs
CoreMark through `coremark_builder` and pins its CRC. The
`n*n` of `least_authority_demo` proves only the mechanism, and CoreMark is a substantial program.

## How this kernel starts it, and what it prints (milestone 74's aarch64 half)

The cycle counter does not run until the kernel starts it. Without `PMCR_EL0.E` and
`PMCNTENSET_EL0.C`, `PMCCNTR_EL0` is a stopped counter that a thread holding milestone 229's grant
can read legally and get the same number from every time (milestone 229 (build the
cycle-counter grant DECISIONS 139 decided)). `kernel/src/arch/aarch64/pmu.rs` starts it
on every core, from `timer::init`, gated on `ID_AA64DFR0_EL1.PMUVer` the way the `PMUSERENR_EL0`
write beside it is:

1. `PMCCFILTR_EL0 = 0`: EL0 and EL1 counted, EL2 not, on every board. calef ruled it on
   2026-10-07 (UTC), option A1 in design/roadmap/0353-the-aarch64-half-of-74.md, because it decides
   what every published cycle number means and user plus kernel is what the other two
   architectures count. What firmware left is printed, for milestone 25 (cross-OS comparison).
2. `PMCR_EL0 = E | C | LC`, assigned rather than read-modify-written, so a divide-by-64 bit firmware
   left set cannot survive.
3. `PMCNTENSET_EL0` bit 31, the cycle counter's own enable.
4. Read the counter across 100 generic-timer ticks and refuse it if it did not move.

The boot prints one line after the secondaries are up, in every build:

```
  cycles      : PMCCNTR_EL0 running on 4 of 4 cores (33000 over 1125 ticks at boot), 6 event counters visible, PMCCFILTR_EL0 0x0 (EL0+EL1 counted, EL2 not)
```

and the other answers it can give are `enabled but did not advance ...; refused` (the `Stuck`
outcome), `no PMUv3 (ID_AA64DFR0_EL1.PMUVer 0x0); ticks only`, and a second line naming any core that
disagrees with the boot core. `script/bench` prints the probe that converts every tick-denominated row:

```
  probe: cycles_per_tick 16.00 (10000226 cycles over 625003 ticks at cntfrq 62500000)
```

That line is the emulator's instruction count, not a measurement, and the exact 16 is the tell.
Without `-icount`, QEMU-TCG advances the counter in steps of 1000. On
argon the ratio should be near the core clock over 19.2 MHz and **not** a clean integer.

Every PMU register is banked per core and resets to an architecturally UNKNOWN value (Arm DDI
0487), so each core assigns them rather than read-modify-writing: a field firmware left set is the
thing being removed. Linux's `armv8pmu_reset` (`drivers/perf/arm_pmuv3.c`) assigns `PMCR_EL0` the
same way, and seL4's `arm_init_ccnt` is the same sequence without `LC`. The control registers are
writable at EL1 once nothing above traps them: `boot.s`'s EL2 drop clears `MDCR_EL2.TPM`, and
`MDCR_EL3.TPM` is TF-A's. A counter that does not run is this kernel's failure to start it, or a
secure-world setting it can observe and not change.

### Why `PMCCFILTR_EL0` is zero

Its `P`, `U` and `NSH` bits decide whether cycles at EL1, EL0 and EL2 are counted, and so what every
cycle number printed means. Zero counts EL1 and EL0 (Secure and Non-secure alike, since `NSK` and
`NSU` equal them), does not count EL2, and counts EL3 where `MDCR_EL3` permits it at all. It is the
policy, not a placeholder, for four reasons:

- It is what the other two architectures count. riscv64's counter runs in every mode and `x86_64`'s
  fixed counter is programmed for rings 0 and 3, so a reader comparing one operation across
  architectures meets user plus kernel each time. Counting EL0 only would make aarch64 the one
  architecture whose IPC figures leave out the kernel doing the IPC.
- It is the only value both readers here run under. `bench::cycle_probe::cycles_per_tick` reads the
  counter at EL1 and the EL0 grant test reads it at EL0; `P` set stops the first and `U` the second.
- EL2 runs nothing in this kernel. `boot.s` drops to EL1 and installs no EL2 vectors, so `NSH`
  matters only under a hypervisor that lets a guest program it.
- It is a definite value. Matching firmware would make the policy a property of each board's boot
  chain.

What firmware left is still read before it is overwritten and printed, as evidence for milestone 25
(cross-OS comparison). seL4's published TX1 figures come from a build that never writes this
register (`arm_init_ccnt` and libsel4bench's `sel4bench_init` write it only under
`CONFIG_ARM_HYPERVISOR_SUPPORT`, which the published build leaves off), so they counted whatever the
TX1's firmware left there. argon runs the same family of firmware, so its first boot answers which
exception levels seL4's 413 and 426 include. If argon's firmware left `P` set, milestone 25 runs
nife a second time with the filter matched to seL4's and labels that run.

### Who may read the cycle counter from EL0

`CNTKCTL_EL1.EL0VCTEN` lets EL0 read the coarse virtual counter; `PMUSERENR_EL0` decides whether it
may also read `PMCCNTR_EL0` and the event counters. The two writes are one decision split across two
registers by the ISA, so both sit in `timer::init`; on riscv64 the same pair is the `TM` and `CY`
bits of `scounteren`. `arch::pmu` starts the counter and does not write `PMUSERENR_EL0`, so one file
owns every write of that register.

`timer::init` writes zero on every core. Every field of the register is an EL0 enable, Arm says
each resets to an UNKNOWN value, and it is banked per core, so a core that skipped the write would
run on whatever its firmware left. Linux writes the same zero for the same reason (`arm64: kernel:
enforce pmuserenr_el0 initialization and restore`). Nothing in the tree reads `PMCCNTR_EL0` from EL0
unless granted, so zero changes nothing a program was using.

A granted thread (DECISIONS 139 option 4) runs with exactly `CR`, bit 2, set: the field for an EL0
read of `PMCCNTR_EL0` alone. `EN` would open every PMU register and `ER` the event counters. The
context switch writes the register only when the value changes, like `mmu::switch_user_root`, and
caches what it last wrote per core rather than reading it back, because without `FEAT_PMUv3` an
`mrs` of the register is as UNDEFINED as an `msr`. No `isb` follows the write: the only way back to
EL0 is the `eret` that ends the return to user, which is context-synchronizing. The grant is accepted
on a part with no `FEAT_PMUv3` (a manifest is a declaration), and `is_cycle_counter_grantable` lets a
test say there is nothing to measure.

The closed register is not timing confinement. An ungranted thread can still time itself against
`CNTVCT_EL0`, and two threads sharing a word build a far finer clock (confinement-claims.md). What
it buys is that the cheap, accurate instrument is granted rather than ambient. Whether EL0 should
ever be granted it is the question of milestone 75 (who may read the cycle counter, and by
what authority).

### BUGS

- Nothing here has run on silicon. argon's bench procedure is milestone 127 (sel4)'s.
- No aarch64 cycle figure is a result until one is measured on silicon. The filter is settled
  (2026-10-07); the machine is not here.
- The `Stuck` refusal has never fired. Every QEMU `-cpu` this tree boots models PMUv3 and moves
  the counter, and `-cpu cortex-a72,pmu=off` takes the no-PMU path instead. The first machine that
  can exercise it is one whose secure firmware prohibits Non-secure counting.
- **HVF could not be tried.** This QEMU refuses HVF with a GICv2 (`HVF does not support GICv2
  emulation`), and this kernel's GIC driver is GICv2-only, so what HVF does to a started counter is
  still the unmeasured claim in the section above.
