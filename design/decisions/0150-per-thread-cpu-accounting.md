---
status: DECIDED
raised: 2026-08-26
decided: 2026-09-13
ratified_by: calef
---

# 150. How does a thread's CPU time reach userspace?

calef, 2026-09-13. Sub-choice 3 was overturned on 2026-09-21, inside the
window this section named, by §204 (how userspace asks where a thread runs), which replaces the
widened `SURVEY` return with a selector. Everything else here stands: tick-sampled, per-thread,
scheduled on-CPU time is unchanged, and only how the figure reaches a reader moved.

It crosses the system call surface, so it is decided here rather than built on the way past. Ruled
option 2, scheduled on-CPU time.

What it unblocks: `top`, which is milestone 282 (a thread's CPU time, and the `top` it makes
possible). Nothing else in milestone 126 (who else is running, and who is allowed to ask) depends on it.

## The premise the fork had to correct first

Milestone 126's own `BUGS` section said `top` needed CPU accounting "that does not exist at all:
`QuotaToken` is dead code". `QuotaToken` and `spawn_with_quota` are real and are exactly as dead as
quoted, and they have nothing to do with CPU time: a `QuotaToken` is a reserved slot in a
*spawn-count* budget, returned when the thread is reaped. Wiring `top` through it would produce a
child-count limiter wearing `top`'s name.

The verified finding is stronger than the entry it replaced: there is no per-thread CPU accounting
anywhere in this kernel, dead or live, and no partial mechanism to wire. `Thread` carries no
time-on-CPU field. `sched::on_tick()` sets `need_resched` and touches no per-thread state. The only
counter is a machine-wide `preemptions()`. So this is new kernel state, not new wiring, which is why
it was a fork.

## What was decided

Scheduled (on-CPU) time, accumulated: what Linux's `utime`/`stime` and Fuchsia's
`zx_object_get_info` runtime are, and the one a reader expecting `top` would recognize.

Two options were refused, and the reasons are the valuable half.

- Wall-clock age (one `spawn_instant: u64` set at `START`). The cheapest thing that compiles,
  and it is not what `%CPU` means anywhere else: a thread alive five minutes and a thread that ran
  continuously for five minutes read identically. Naming that "CPU accounting" would mislead a
  reader who knows Unix `top`, which is the fault milestone 115's naming rules exist to catch.
- Sampling over `SURVEY` (poll repeatedly, estimate from how often a tid reads `RUNNING`). The
  only option needing zero new kernel state and zero new system call surface, and not accounting in any
  real sense: a thread running in the gaps between samples is invisible, and short bursts are
  systematically mis-counted depending on phase. Named because the tenet requires naming the
  zero-cost option even when it is not the recommendation. No mainstream `top` ships pure
  sampling as its primary source, which is some evidence it does not hold up even where it is
  free.

## The three sub-choices, carried from the fork's recommendation

calef ruled "option 2". The fork's recommendation named three further choices inside it, with
reasons; they are recorded here as decided on that recommendation rather than separately
argued, and the third is the one that is expensive to un-ship.

1. **Tick-sampled, not switch-accumulated.** One branch and one increment inside `on_tick()`, code
   that already runs every tick on every core, against touching two `Thread`s on every voluntary
   yield in `schedule()`'s hot path. Linux's `jiffies`-based `utime` accumulates at the scheduler
   tick for exactly this reason: cheap, coarse, good enough.
2. **Per-thread, not per-process.** This kernel's native unit is the thread, which is what `ps` and
   `pgrep` already report. There is no process/thread-group construct to aggregate into, and
   inventing one only for `top` would be new state with no other consumer, the speculative
   abstraction §46 declines elsewhere.
3. **A widened `SURVEY` return, not a second method.** `abi::rendezvous::SURVEY` returns three words
   (`next_cursor`, `tid`, `state`); a fourth carries the figure. This widens an existing method's
   shape rather than adding a system call number, mirroring how `pmap` added a method to a different
   object type instead of a system call (§114 (`pmap` gets its listing)).

Sub-choice 3 is the irreversible one and is called out here so a reader meets it. It is a wire
format the kernel and every future `SURVEY` reader agree on, which the *move fast on what can be
undone* tenet puts in the category that cannot be recalled. It is cheap to prototype and expensive
to un-ship, so the moment to overturn it is before milestone 282 ships, not after.

## What the reader is owed, and it is not a number

Per-thread CPU time is an aggregate statistic, and capabilities do not close that side channel.
Milestone 126's own text says so. A confined viewer holding `ENUMERATE` learns something about
threads it cannot otherwise name, and the design should say what that leak is worth rather than
discover it later. `ENUMERATE` is already defined as "the right to learn what exists, as distinct
from acting on it", so the authority question has an answer; what is new is that the thing learned
is now continuous rather than a state word.

## The cross-core read

Each core's tick touches only the thread running on that core, so the write needs no cross-core
synchronization. A reader on one core observing a counter another core is actively incrementing is
an ordinary relaxed-load race, the same shape the per-CPU `TICKS` array already accepts. Rule 4
applies: this is a weakly-ordered machine and the accepted race must be stated rather than assumed.
