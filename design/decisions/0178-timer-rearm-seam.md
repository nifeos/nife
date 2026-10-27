---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-27
ratified_by: calef
---

# 178. Where the timer re-arm seam goes, and which miss behavior the kernel tick is meant to have

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s slice B, which read milestone 360 (`crates/timetable`'s proved `next_after` is not what the timer calls)'s
`DECISION` gate and found it naming no section. Milestone 197 named the fork and declined to take
it; the 2026-09-03 proposal sweep carried it forward.

## The ruling

Decided 2026-10-27 (UTC): option A. Lift the arithmetic only.

- Each software re-arm, `kernel/src/arch/aarch64/timer.rs::rearm` and
  `kernel/src/arch/riscv64/timer.rs::rearm`, reads its registers, calls
  `timetable::next_after(fired, interval, now)`, and arms the result.
- A kernel tick that arrives late comes back onto its original beat. It does not start a fresh
  interval from where it woke up. The behavior change on the late path is intended, and it restores
  the property milestone 6 (threads, the context switch, and preemption) chose the compare register for.
- Lateness detection and the `MISSED_TICKS` accounting stay in the arch layer, as does the
  `#[cfg(test)] miss_detail::record` hook. The crate returns a deadline, not a verdict.
- x86_64 is unchanged, because the local APIC reloads in periodic mode and there is no software
  re-arm to lift.
- The signature stays `pub const fn next_after(prev: u64, period: u64, now: u64) -> u64`.

Options B and C are not taken. The drift test and `miss_detail` are re-read against the new late
path by the lane that builds milestone 360 (`crates/timetable`'s proved `next_after` is not what the
timer calls, so nothing runs the proof), not assumed to hold.

## What is being decided

`crates/timetable::next_after` is proved and nothing on the kernel timer path calls it. Its only
caller in the tree is `Timetable::due`, one function further down its own file. The re-arm
arithmetic the kernel actually runs is written inside the register access, once per architecture.

The decision is where the seam goes: too high and the arch layer keeps its own arithmetic, too low
and every ISA restates what the crate exists to hold.

## The two re-arms, side by side, which is what a lane could produce before a ruling

Read on 2026-09-19. Two architectures re-arm in software, not three:
`kernel/src/arch/aarch64/timer.rs::rearm` and `kernel/src/arch/riscv64/timer.rs::rearm`. x86_64 arms
the local APIC in periodic mode and the hardware reloads, so there is no software re-arm there to
lift.

| | aarch64 | riscv64 |
|---|---|---|
| `now` | `CNTVCT_EL0.get()` | `now()` |
| the deadline that fired | `CNTV_CVAL_EL0.get()` | `DEADLINE[id].load(Relaxed)` |
| the period | the `interval` parameter | `interval()` |
| ordinary case | `next = fired + interval` | `next = fired + interval()` |
| late case | `MISSED_TICKS += 1`, then `next = now + interval` | `MISSED_TICKS[id] += 1`, then `next = now + interval()` |
| arming | `CNTV_CVAL_EL0.set(next)` | `DEADLINE[id].store(next)`, `sbi_set_timer(next)` |

The signature question the block poses is already answered. Milestone 360 asks what
`next_after`'s signature would have to become to serve all three, *"a deadline, a delta, or a raw
counter value"*. Both software re-arms already hold exactly the three values it takes, all `u64`:
`fired` is `prev`, `interval` is `period`, and `now` is `now`. `pub const fn next_after(prev: u64,
period: u64, now: u64) -> u64` serves both without changing.

## The premise is not quite true, and this is the part that decides the fork

Milestone 360 says `next_after` *"computes the same thing"* as the re-arm. **It does not, on the
late path, and the difference is the whole argument.**

`next_after` keeps the phase: its documented second property is that the result is congruent to
`prev` modulo `period`, *"so an entry that drifted late because the machine was busy comes back onto
its original beat instead of inheriting the delay forever"*. The kernel's re-arm, on a missed tick,
sets `next = now + interval`, which leaves the grid permanently.

That grid is the point of the register choice milestone 6 made. `kernel/src/arch/aarch64/timer.rs`'s
own header states it: re-arming from a relative countdown made *"100 Hz configured, ~70 Hz
observed"*, and `CVAL` was chosen because *"the deadlines sit on a fixed grid. A slow handler makes
one tick late; it does not make the next one late as well."*

So the kernel holds the grid in the ordinary case and abandons it on exactly the case the grid was
chosen for. It is bounded (one shift per missed tick, and missed ticks are counted), and it is the
residue of the defect milestone 6 fixed rather than a new one. It is also the reason this is a fork
rather than a refactor: lifting `next_after` in as it stands changes kernel behavior, and
whether that change is wanted is the question underneath "where does the seam go".

## The options

| | shape | cost |
|---|---|---|
| **A** | **Lift the arithmetic only.** Each `rearm` reads its registers, calls `next_after(fired, interval, now)`, and arms. Lateness is detected by the arch layer with one comparison it already makes. | The smallest change that binds the proof, and it fixes the grid on the late path for both ISAs at once. It changes behavior, so the drift test and `miss_detail` need re-reading rather than assuming. |
| **B** | **Lift the arithmetic and the miss accounting**, so the crate answers with both the next deadline and whether it was late, and the arch layer only reads and writes registers. | Binds more of the code to a proof. `MISSED_TICKS` and the `#[cfg(test)] miss_detail::record` are per-CPU kernel state, so the crate would return a verdict rather than own the counter, which is one more thing on a boundary that exists to be thin. |
| **C** | **Leave it, and record the counterfactual where the reader meets it.** | Free. The standing cost is the one milestone 360 names: `script/verify` reports a green proof beside proofs that do bind, with no way for a reader to tell them apart, and the demonstrator's claim is a verified core. |

The proposal made no recommendation, because A carries a behavior change to the kernel tick that
is an architect's call rather than a lane's.

The prior question for A, answered by the ruling above, was: should a kernel tick that
arrives late come back onto its original beat, or start a fresh interval from where it woke up?
`next_after` answers the first; both `rearm`s answer the second.

## What was blocked until this was answered

Milestone 360, now unblocked. Nothing else cites it. Three of `next_after`'s four properties are machine-checked
and the phase one is host-tested, and `crates/timetable/src/proofs.rs` says why in the place a reader
of the proofs would look, so the proof itself needs nothing from this.
