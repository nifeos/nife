# A saturated workload does not migrate under this scheduler

An appendix to [notes/soak.md](../soak.md), which states the finding and what it means for risk 5.
This file holds the measurement, the mechanism, the instrument that found it, and the instruments
the soak was deliberately not built on.

## The finding

This is the part worth reading, and it is the reason the milestone was worth running rather than
merely worth building. It is still true, and milestone 221 (the soak never crosses cores, so build the hook that makes it) did not repeal it. What that milestone
added is a thread that is *not* part of the saturated workload, precisely because nothing inside
the workload can be made to move.

The cross-core handoff count freezes within the first second and never moves again. It was measured
across three topologies: one caller per responder, three callers per responder, and twice as many
groups as cores. It was measured on both multicore architectures, at up to 65,000 round trips a
second. The workload runs on every core and contends on every shared scheduler structure. The threads
themselves stay exactly where `pick_spawn_target` put them.

## The mechanism, and every clause of it is in the tree already

- A rendezvous wake is local on purpose. `sched::wake` pushes the woken peer onto the *waker's* own
  run queue (§28 (SMP placement), part 2: the message is in registers and the cache is warm). So a communicating
  set converges onto one core within a few exchanges and stays there.
- `wake_load_aware`, the load-aware placement, is for device interrupts only. It is the function the
  one real defect was in, and no user workload can reach it: it takes an IRQ to get there.
- A work steal needs an idle core and a queued thread elsewhere. A rendezvous keeps at most two
  threads runnable per group, so run queues are almost always empty and there is nothing to give.
  Add compute threads to fill the queues, and no core is idle to ask. Both ends of the condition are
  hard to hold at once, and a steady-state workload holds neither.
- Nothing rebalances periodically. There is no such thing in this scheduler.

## A placement counter cannot see this migration

Counting `trace::Event::PlaceRemote` reports 23, frozen, which reads as "the threads are not
moving". That is true, but the counter could not have shown it. A rendezvous wake
queues its peer locally, so the placement is local *even when the thread has moved between cores*.
A placement counter is structurally blind to the migration this workload performs.

`thread::Thread::last_cpu` and `trace::Event::Migrated` answer the question where it cannot be
dodged. That is `schedule()`'s `switch_in`, the one place every path to a CPU passes through,
whatever moved the thread, and the finding holds under it.

Take the lesson, not just the number: a counter that is *near* the question is not the same as one
that answers it, and the two agree right up until they matter.

## What this means for risk 5

The decisive experiment as the risk states it, "sustained multi-core stress", is not one experiment.
It is at least two, and this milestone delivers the first:

- Concurrent contention on shared kernel state. Four harts enter `IPC_TABLES` tens of thousands of
  times a second, preempting each other, writing their own trace rings, retiring rendezvous. This is
  real weak-memory pressure, and it is what the soak sustains.
- Cross-core handoff. That is threads actually moving between cores under load, which is where the
  observed defect lived. The soak does not sustain this, and cannot, for the reasons above.

Saying so is the point. A run that quietly covered one and was quoted as covering both would be
exactly the misuse `design/roadmap/0219-a-workload-that-does-not-stop.md`'s BUGS section warns about.
`script/soak-test` prints the gap on every run, so that nobody has to have read this note to know.

The second half is now runnable, which is a different claim from "has been run". See
[tick-route.md](tick-route.md).

## Why this extends `board_console` and not the other two instruments

`script/repeat-under-load` and `script/interleaving-check` are the tree's existing load and
concurrency instruments, and neither was the right place for this.

- `script/repeat-under-load` repeats a terminating suite N times with the host deliberately loaded,
  and reports what the load actually was. A soak has no runs to repeat and does not terminate. The
  contention it wants is the guest's own rather than the host's. The two are complements. That one
  asks "does the suite still pass when the machine is busy". This one asks "does the machine stay
  correct when it is busy for hours".
- `script/interleaving-check` is loom over the extracted protocols, on the host, searching every
  interleaving the C11 model permits. It is the strongest evidence available about those protocols,
  and it says so honestly: loom models C11, not ARM and not RISC-V. A soak on silicon is the evidence
  loom cannot give, not a substitute for it.
- `crates/board_console` was the right one. The thing a soak needs that did not exist is a judgment
  about *silence*, and that crate already owned it.
