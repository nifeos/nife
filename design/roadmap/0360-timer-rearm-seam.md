---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: 178
machine_requirements: none
specific_machine: none
needs_person: no
---
# 360. `crates/timetable`'s proved `next_after` is not what the timer calls, so nothing runs the proof

Filed as a proposal on 2026-09-03 by the milestone 247 sweep, from
milestone 197's block; promoted by milestone 433 on 2026-09-19. Checked against the tree that day
and the counterfactual is intact: `timetable::next_after` has exactly one caller in the tree,
`crates/timetable/src/lib.rs`'s own `Timetable::due`, and nothing on the kernel timer path reaches it.
One correction to the body's "every ISA restates the arithmetic": two do, not three.
`kernel/src/arch/aarch64/timer.rs::rearm` and `kernel/src/arch/riscv64/timer.rs::rearm` each compute
`fired + interval` with their own copy of the skip-rather-than-catch-up rule, while x86_64 arms the
local APIC in periodic mode and the hardware reloads, so there is no software re-arm there to lift.

Where the seam goes is an architect's call. It is the whole of the work rather than
a detail of it. Too high and the arch layer keeps the milestone 6 (threads, the context switch, and
preemption) drift bug it has today; too low and every ISA restates the same arithmetic, which is the
thing the crate exists to stop. A lane can prepare the options and measure them, but it cannot pick
the line.

**In brief.** The timer re-arm arithmetic is currently written inside the register access, per
architecture. `crates/timetable::next_after` computes the same thing, is proved, and nothing on the
running path calls it. The work is to lift the arithmetic out of the register write so the proved
function is the one the timer actually uses, and the design question is which side of the seam each
piece lands on.

## Why this matters

This is the sharpest counterfactual the verification story has, and it currently points the wrong
way. A proof over a function nothing calls is worth nothing to the running system, and it is worse
than nothing to a reader: `script/verify` reports it green beside proofs that do bind, with no way
to tell the two apart. The demonstrator's claim is a verified core, and a stranger who checks this
one finds a property that holds over code the kernel does not execute.

The bug on the other side is concrete rather than hypothetical. Milestone 6's drift defect lives in
the per-architecture re-arm, and it lives there once per ISA. Every architecture added restates the
arithmetic and has its own chance to restate the bug with it.

## What would settle the decision

Two things a lane can produce before calef rules, and neither of them presumes the answer. The
first is what each architecture's re-arm actually does today, side by side, so the shared part and
the genuinely ISA-specific part are separated by reading rather than by assertion. The second is
what `next_after`'s signature would have to become to serve all three, since the seam is really the
question of what crosses that signature: a deadline, a delta, or a raw counter value.

## Where it came from

Milestone 197 (`user/` and `xtask` are out of reach of the prover) named it and declined to take it:
*"Lift the timer re-arm arithmetic out of the register access so `crates/timetable`'s already proved
`next_after` is what the timer actually calls. Where the seam goes is an architect's: too high and
the arch layer keeps the milestone 6 drift bug, too low and every ISA restates it. Until it moves,
the tree's sharpest counterfactual is a property proved over code that nothing runs."*

## Index row

The timer re-arm arithmetic is written inside the register access, per architecture.
`crates/timetable::next_after` computes the same thing, is proved by Kani, and nothing on the running
path calls it. That is the sharpest counterfactual the verification story has and it currently points
the wrong way: `script/verify` reports it green beside proofs that do bind, with no way for a reader
to tell the two apart. The bug on the other side is concrete, since milestone 6's drift defect lives
in the per-architecture re-arm and lives there once per ISA. Where the seam goes is an architect's
call and is the whole of the work rather than a detail of it. Too high and the arch layer keeps the
drift bug; too low and every ISA restates the arithmetic the crate exists to hold. A lane can put
the two re-arms side by side and say what `next_after`'s signature would have to become, which is
really the question of what crosses it: a deadline, a delta, or a raw counter value.
