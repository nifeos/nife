# The instruction clock: the injections, and what each one settled

*Name: provisional. Named after the parent's own heading.*

An appendix to [`notes/instruction-clock.md`](../instruction-clock.md). It records the three defects
injected into the timer handler on 2026-08-17 (UTC), and the pricing question for milestone 106 (a
wait that ends on the interrupt or the deadline) that the third one answers.

Every injection was reverted. Recorded in full, including the expectation they refuted, because a
failed prediction is how the useful facts arrived here as well.

## Two implementations of the defect claim 1 names

The block names the residual exactly: *"an implementation that maintains `DEADLINE` correctly and
arms SBI with something else"*. Two were written, both one line in `rearm`, both keeping the grid
store untouched.

| injection | what it is | `script/icount --arch riscv64` | the riscv64 leg of `script/test` |
|---|---|---|---|
| `sbi_set_timer(now + interval())` | re-anchor on every tick, the classic drift bug, with the grid still kept correctly beside it | **red**: arrival 420,400 instructions against a bound of 1,500 | **red**, at `the_handler_keeps_up_when_no_lock_is_held` |
| `sbi_set_timer(next + interval() / 4)` | a *fixed* quarter-period offset: no drift, no accumulation, the delivered rate still exactly 100 Hz | **red**: arrival 2,500,400 instructions, which is the injected 25,000 counter ticks plus the usual ~400, on every one of the 64 ticks | **red**, at `ticks_arrive_at_the_configured_rate` |

The expectation was that the suite would not notice, and it did. That is worth stating plainly
because it was written down as a prediction first. What it did not do is say what was wrong:

- the first one failed as *"the timer handler is taking longer than a whole tick period, with no lock
  held... Late by less than one interval means the handler itself is slow, which is this kernel's
  bug."* The handler is not slow. It is the assertion that broke #204, #210 and #215 on aarch64 in a
  single afternoon, so a reader triaging that red run has to decide "known or real" about the exact
  assertion this milestone was raised over.
- the second failed as *"no miss-free measurement window in eight tries: either the host is too
  contended to observe the grid, or the handler is slower than a whole tick period"*. The first half
  of that sentence is an invitation to re-run and move on.

The icount message names the defect: *"either the trap path grew, or the timer was armed with
something other than the deadline the kernel recorded"*, on a number with no host term in it.

So the instrument's value is diagnostic certainty rather than detection, and that is the
milestone's own thesis rather than a consolation. The block's cost line is "every red check in this
repository currently needs a human to decide 'known or real', and on 2026-08-03 that judgment was
made at least six times and got the wrong answer twice." Both of these injections produce exactly
that judgment call on the test path and none of it here.

## A known instruction count, to measure the resolution

The third injection asks what this instrument can see rather than whether it fires: exactly 200
instructions added to the top of the aarch64 `tick`, via the same assembly loop the calibration
uses.

| | arrival | handler |
|---|---|---|
| clean | 1,008 | 1,056 |
| +200 instructions injected | 1,216 | 1,264 |
| **delta** | **+208** | **+208** |

Both numbers moved by the same amount, on every one of the 64 ticks, and the eight-instruction
residual over the 200 injected is the operand setup around the loop: smaller than one counter
tick. That is the resolution claim measured rather than reasoned, and it is what makes the
paragraph below about milestone 106 a statement of fact.

## The pricing question milestone 106 could not answer

`notes/timed-wait.md` (the pricing lane for milestone 51's deadline fork) wanted "a dedicated
`--features bench` probe under `-icount shift=0`" and could not build one. So it priced the
deadline check by reading disassembly instead: +30 instructions on aarch64 and +31 on riscv64 in
`sched::on_tick`, against a whole-tick path of ~491 and ~400 static instructions. Its own words:
"Nothing was measured under icount, because the icount tripwire's own note records ±5% codegen drift
between binaries, which swamps a 30-instruction change: the disassembly is the finer instrument
here, not the coarser one."

That was true of the tripwire and it is not true of this. The quantity here is a localized span
inside one run, not two binaries' totals, and it has no variance at all:

- aarch64: yes, and it is measured rather than argued. Injecting exactly 200 instructions into
  the handler moved the reported number by 208, on every one of the 64 ticks, with the residual
  smaller than one counter tick (the table above it). A +30 change is therefore two counter ticks of
  movement on a number that does not otherwise move at all. Build with the prototype, build without
  it, run `script/icount --arch aarch64` twice, and the difference is the answer.
- riscv64: not at this resolution. One counter tick is 100 instructions, so a +31 change is
  visible only as an occasional single-tick step, and the honest reading is "under 100 and not zero".
  The disassembly stays the finer instrument on that ISA.

The caveat both legs share: the measured span is the timer handler, so it answers "what does a
deadline check cost the tick" and not "what does it cost anything else".
