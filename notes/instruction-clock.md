# The instruction clock: timing claims a busy host cannot falsify

*(Milestone 78's last piece. `script/icount`, `kernel/src/icount.rs`, the `icount` feature, and the
`tick_trace` hooks in `kernel/src/arch/aarch64/timer.rs` and `kernel/src/arch/riscv64/timer.rs`.
Every name in that list is **provisional**: calef names the scripts, crates, programs and modules,
and a lane ships a provisional name and says so.)*

## The one fact everything here rests on

Under `-icount shift=0,sleep=off`, QEMU's virtual clock advances by exactly one nanosecond per
guest instruction retired, and by nothing else. Not by host time, other lanes' gates, or whether the emulator's thread was scheduled at all.

That property turns a family of unaskable questions into assertions.
`notes/load-sensitive-assertions.md` states the problem once per ISA, in the same words:
from inside the guest, a 30 ms handler and a 30 ms deschedule are the same observation. Milestone
78's other four rounds re-aimed everything that could be re-aimed at a property the host cannot
touch. Two claims survived because no wall-clock margin can separate those two cases, and a margin
wide enough to try is a margin that no longer catches the defect. That is DECISIONS §61's scar, and
the block forbids it by name.

An instruction count is a third option. It does not widen a bound; it changes the unit the bound is
written in, to one the host has no access to.

## Running it

```sh
script/icount                    # both ISAs, ~7 s including two kernel builds
script/icount --arch aarch64     # one leg
script/icount --arch riscv64
```

It prints what it measured and whether the claims hold. A violated claim is a panic in the
guest, with the measured number and the bound in the message; `xtask` reports the panic and exits
nonzero.

### EXAMPLES

A clean run, both legs, 2026-08-17, on the pinned QEMU:

```
--- icount: aarch64, single hart, TCG + icount (one instruction = one nanosecond) ---
  cntfrq 62500000
  instructions_per_counter_tick 16
  tick_interval 625000 10000000
  calibration 2000032 2000000
  ticks 64
  arrival_instructions min 1008 mean 1008 max 1008
  handler_instructions mean 1056 max 1056
  missed_ticks 0
  early_arrivals 0
  done
icount: aarch64 claims hold

--- icount: riscv64, single hart, TCG + icount (one instruction = one nanosecond) ---
  cntfrq 10000000
  instructions_per_counter_tick 100
  tick_interval 100000 10000000
  calibration 2000100 2000000
  ticks 64
  arrival_instructions min 300 mean 300 max 400
  handler_instructions mean 800 max 900
  missed_ticks 0
  early_arrivals 0
  done
icount: riscv64 claims hold
```

**Read `min` and `max` before anything else.** On aarch64 they are the same number across 64
consecutive ticks, which is the instrument's whole claim demonstrated rather than argued. The
measurement has *zero* variance, so a bound on it is a statement about this kernel's code and about
nothing else. The riscv64 pair differ by one counter tick, which is that ISA's 100-instruction
quantization showing (see "Resolution" below), not jitter in the machine.

## What is asserted, and what each claim rules out

*(There are four. Claim 4 was added on 2026-08-18 by milestone 62, after an injection showed that
claims 1 to 3 could not see the timer drift bug at all; the section below it is the evidence.)*

### 1. The timer fired at the deadline the kernel armed

`arrival_instructions` is the distance from the deadline that fired to the handler observing it:
interrupt delivery, the vector, the register save, the dispatch, the GIC acknowledge on aarch64, and
the tick bookkeeping.

On riscv64 this is the claim milestone 78 was left holding, and it is the whole reason the
instrument exists. SBI's `set_timer` is write-only. `DEADLINE` is the kernel's own array, so
reading it back proves only that the kernel can remember what it meant to write; the block says so
in as many words ("on aarch64 the equivalent value is in a register the hardware itself consults, so
the readback is evidence and on riscv64 it is bookkeeping"). The residual gap is an implementation
that keeps `DEADLINE` on the grid and arms SBI with something else, and it would pass every other
test in this tree.

It does not pass this one, proved by building it (see "The injections" below).

aarch64 asserts the same thing, at the same site, for the reason rule 5 gives: a claim that holds on
one ISA and is unmade on the other is the gap, not the parity.

### 2. The handler takes fewer than N instructions

`handler_instructions` is deadline to re-armed: claim 1's span plus the tick bookkeeping, the
deadline write, and on riscv64 the SBI `ecall` round trip into OpenSBI and back. That trip is inside
the measurement on purpose. It is a real part of what this ISA's handler costs and it is the part
aarch64 does not pay. So a measurement that stopped short of it would be comparing two different
spans across the two ISAs.

**This is the quantity `MISSED_TICKS` has always been a coarse proxy for.** A miss is this number
exceeding one whole tick period, which is **10,000,000 instructions of virtual time on both ISAs**:
ten milliseconds at 100 Hz, and one instruction is one nanosecond. The missed-tick assertions can
therefore only say "the handler did not take ten milliseconds". They cannot tell a handler that took
ten milliseconds from an emulator that was not running for ten milliseconds. The bounds here are
about 4,000 times tighter and have no second explanation. On aarch64 the tick is 625,000 counter
ticks at 16 instructions each, and the run's own `tick_interval 625000 10000000` line prints both
numbers side by side. Reading the first as instructions is a number in the wrong unit, the mistake
this note exists to make hard.

The bounds live in the arch layer (`ARRIVAL_BOUND`, `HANDLER_BOUND`) because the trap path is the
arch's own. They are **ceilings with room for ordinary codegen movement, not baselines**: a change
that halves the number is not a failure, and one that doubles it is a fact worth stopping for. As of
2026-08-17 the margins are roughly 2x on aarch64 and 3x on riscv64 over what the machine reports,
and the run prints what it saw beside the bound, so the margin is visible rather than asserted
about.

### 3. Zero missed ticks, which is only assertable here

This one closes a recorded `BUGS` entry rather than adding a claim. The miss taxonomy on both ISAs
exists to tell a slow handler from a descheduled emulator by how late the re-arm was.
`notes/load-sensitive-assertions.md` is honest that its cut leaves a window one tick period wide in
which a host deschedule is still blamed on this kernel. Measured on 2026-08-16, that was a lateness
of 0.83 of an interval from a probe holding a lock across two and a half periods.

Virtual time has no deschedules. So here a miss has exactly one possible cause and needs no taxonomy
at all, and `missed_ticks == 0` is a bare assertion.

## Why it is opt-in, and why that is the design rather than a compromise

`-icount` is not a flag that observes; it changes what QEMU is. It is not slower on compute, as
measured (2.47 to 2.61 s against 2.69 to 2.80 s). The two real reasons it stays off the test path:

- One virtual clock serves every vCPU, and an idle hart in `wfi` jumps it forward, so multi-hart
  timing is fiction. A suite run this way would silently stop proving every cross-core property.
- A clock-bound wait starts costing instructions. The suite's `spin_for`s and clock-bounded
  `wait_for`s would cost roughly five times what they cost today.

So it is its own boot mode, feature and command, like `script/bench`. Nothing on the test path
changes: the `tick_trace` hooks are `#[cfg(feature = "icount")]`, so what this boot
measures is the handler that ships. The feature implies `bench` rather than extending it, because
DECISIONS §38 (a suppression is scoped to an item) refuses the crate-level `allow(dead_code)` the
other way out would need. Both ISAs run in 7.0 s including two kernel builds. The full argument is
in [`instruction-clock/why-opt-in.md`](instruction-clock/why-opt-in.md).

## Resolution: what this instrument can and cannot see

Virtual time is exact in nanoseconds, but the guest reads it through a divided counter, and that
division is the instrument's resolution:

| ISA | counter | frequency | one counter tick |
|---|---|---|---|
| aarch64 | `CNTVCT_EL0` | 62.5 MHz | **16 instructions** |
| riscv64 | `rdtime` | 10 MHz | **100 instructions** |

The conversion is exact only because both frequencies divide a gigahertz; `icount.rs` asserts that
rather than assuming it, so a board whose counter did not would fail loudly instead of quietly
printing rounded numbers.

**Resolution is not the same as noise, and the difference is the useful part.** The aarch64 numbers
are identical across 64 consecutive ticks, so a change of 32 instructions in the handler moves the
reported number by exactly 2 counter ticks and cannot be mistaken for anything. The `bench --check` tripwire, by contrast, compares *whole-binary* counts across
*different builds* and is honest about ±5% codegen drift between them.

### The pricing question milestone 106 could not answer

Milestone 106 (a wait that ends on the interrupt or the deadline) could not price a 30-instruction
deadline check under the icount tripwire, whose ±5% drift swamps it. This instrument can on aarch64;
see the injections below.

### 4. The armed deadline advanced by exactly one interval per delivered tick

The re-arm law, and it is here because claims 1 to 3 are blind to a violation of it. Milestone 62
(tests that assert on time) injected the defect the aarch64 timer's module header has warned about since milestone 6 (threads
and preemption). It re-anchored the grid from `now` inside `rearm` rather than advancing it from the
deadline that fired: the bug that made 100 Hz configured into about 70 Hz delivered. `script/test`
went red. This instrument went green with every number byte-identical to a clean run (arrival
min/mean/max 1008, handler 1056, `missed_ticks 0`).

The reason is claim 1's own subject. It compares each arrival against *the deadline that fired*, so
a kernel that re-anchors the whole grid arms the timer with the very word it records and satisfies
the comparison on every tick forever. Claim 2's span starts at the same place, and claim 3 counts
misses, of which re-anchoring produces none. Nothing moves.

Here the law needs no retry loop, claim 3 paying for itself. The suite's twin must find a window in which `MISSED_TICKS` did not move, because a miss re-anchors the grid on
purpose. On a loaded host it sometimes cannot find one in eight tries, and that is the whole of
milestone 62's disposition of it. Virtual time has no deschedules, `missed == 0` is asserted a few
lines above, and the window is therefore miss-free by assertion.

| | clean | drift injected |
|---|---|---|
| aarch64, `deadline_delta` over 64 ticks | 40,000,000 of 40,000,000 | **40,004,032**, red |
| riscv64, `deadline_delta` over 64 ticks | 6,400,000 of 6,400,000 | **6,400,231**, red |

The excess prices the defect: 4,032 counter ticks over 64 is 63 per
tick, or 1,008 instructions, which is exactly the arrival latency the same run printed. On riscv64,
231 over 64 is 3.6 counter ticks, or ~360 instructions, against a printed arrival of 300 to 400.

## The injections, and what each one settled

Every injection was reverted. Each is recorded in full in
[`instruction-clock/the-injections.md`](instruction-clock/the-injections.md). What they settled:

- Two implementations of the defect claim 1 names, each keeping `DEADLINE` correct and arming SBI
  with something else, both went red here. The expectation was that the suite would not notice. It
  did, but it misdiagnosed both: once as a slow handler, once as a contended host. The icount
  message names the defect, on a number with no host term in it. So the instrument's value is
  diagnostic certainty rather than detection, which is the milestone's own thesis.
- Exactly 200 instructions injected into the aarch64 `tick` moved both numbers by 208 on every
  tick. That measures the resolution rather than reasoning about it, and it answers milestone 106's
  pricing question: on aarch64 a +30-instruction deadline check is two counter ticks of movement on
  a number that does not otherwise move. On riscv64 the 100-instruction resolution can say only
  "under 100 and not zero".

## The calibration, and why a boot refuses to measure without it

The failure this repository keeps writing comments about is the **manufactured fact**: a variable
set to a missing file, a device that was not attached, a flag that silently did nothing. Booted
without `-icount shift=0` this kernel runs perfectly well, and every number above would become a
wall-clock number wearing an instruction's units. That is worse than an error, because it looks like
a measurement.

So the boot proves it is on the instrument before it measures. The arch layer runs
a loop of a known instruction count: `subs`/`b.ne` on aarch64, `addi`/`bnez` on riscv64, two
instructions per iteration. It is assembly so the expected count is known rather than whatever the
optimizer decided. The virtual clock must agree to within 1%:

| leg | expected instructions | virtual time observed |
|---|---|---|
| aarch64 | 2,000,000 | 2,000,032 ns |
| riscv64 | 2,000,000 | 2,000,100 ns |

Those thirty-two and one hundred nanoseconds are the operand setup either side of the loop and the
counter's own quantization. The 1% tolerance admits neither of the other two ways this kernel runs. Plain TCG retires far fewer than one instruction per
nanosecond of wall clock, and HVF on this host retires several.

The assembly lives under `arch/` rather than in `icount.rs` because DECISIONS §3 puts every `asm!`
there.

## BUGS

- The riscv64 resolution is 100 instructions, and that is coarse enough to matter. A change
  smaller than one `rdtime` tick is invisible on that leg except as an occasional single-tick step in
  the max. Nothing in the kernel can fix this: the counter's frequency is the machine's. A finer
  probe would have to read `rdcycle`/`rdinstret`, which QEMU does map to the instruction count under
  icount. But S-mode may only read them when the firmware permits it (`mcounteren`), so it is a
  dependency on OpenSBI's configuration rather than on the architecture. Not tried here.
- The bounds are ceilings and a regression under them is invisible. A change that adds 300
  instructions to the aarch64 handler passes, because the bound is 2,000 and the number is 1,008.
  That is the deliberate trade against codegen drift: an exact baseline would be a second
  `bench --check` with its re-save churn and none of its coverage. If the
  numbers ever need to be tracked rather than bounded, a baseline file is the obvious next step and
  the run already prints everything one would contain.
- **Only the timer handler is instrumented.** Both claims this milestone was left holding are about
  the timer, and the file is deliberately not a framework. Any other
  path wanting an instruction-denominated claim needs its own `tick_trace`-shaped recording, which is
  three relaxed counters and a call site.
- The measured handler is the shipping handler, not the tested one. The `icount` build is not a
  test build, so the `#[cfg(test)]` watchdog feed and `miss_detail` recording are absent from the
  numbers above. That is the right subject, and it puts the figure a few instructions below what the suite's own
  handler executes.
- **`-smp 1`, always.** Under `-icount` all vCPUs share one virtual clock and an idle secondary
  parked in `wfi` jumps that clock forward to the next event, so a multi-hart timer measurement is
  measuring the other harts' idle jumps. This is the same constraint the bench instrument carries and
  the reason the placement probe stays on the wall clock (`notes/load-sensitive-assertions.md`).
- **The instrument is a boot mode, so it does not run under `script/test`.** It does run in
  `script/ci-build`'s `local` tier, and this line said otherwise until milestone 286; the
  `-icount` argument below is about a boot mode rather than about a command. It is
  wired into CI beside the bench tripwire, which shares its QEMU cache and its path filter. A
  developer who never runs `script/icount` locally will find out in CI rather than before pushing,
  which is the cost of not putting `-icount` on the test path.
- The failure path had to be bounded on the host, and finding that out leaked two emulators.
  The guest's panic handler halts rather than exiting, so a violated claim prints its message and
  then the pipe never closes. `xtask` blocked on the read forever and QEMU stayed alive at ~80% of a
  core, twice, on a laptop already carrying four other lanes' gates. Counting "a few more lines"
  after the panic hung the same way. The mechanism now is a watchdog thread with a 300 s deadline that a seen panic shortens to three
  seconds, and killing QEMU is what closes the pipe and ends the read. `run_bench` has the same
  shape and the same exposure, and was not changed here.
- Numbers are from one QEMU. icount counts guest instructions, so the emulator's version is part
  of what they mean, exactly as `bench/baseline-*.txt` records for itself. `script/qemu-check` warns
  when the QEMU on PATH is not the pinned one.

## See also

- design/roadmap/0078-load-sensitive-assertions.md: the milestone, and the day's evidence
- notes/load-sensitive-assertions.md: the five rounds, and both places the two claims were deferred
  to this instrument
- notes/benchmarks.md: the other icount consumer, why it is `-smp 1`, and the ±5% codegen drift
- notes/timed-wait.md: milestone 106's pricing lane, whose measurement this answers
- notes/cpu-models.md: the load-sensitivity evidence, including the control model failing
