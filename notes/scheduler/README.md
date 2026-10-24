# The scheduler: the appendices to notes/scheduler.md

[`notes/scheduler.md`](../scheduler.md) carries the shape, placement, stealing, wakes, the
undelivered-wake gate, the two hang watchdogs, and a summary of each closed bug. These files carry
the evidence its prose budget has no room for.

- [`undelivered-wake-gate.md`](undelivered-wake-gate.md): why the gate was built, the misreading
  of boot 8 that it was built against, its two companions, and the loom model.
- [`reclaim-lost-wakeup.md`](reclaim-lost-wakeup.md): milestone 72 (lost wakeup), a refused reclaim
  that killed the child a test was waiting for, and the thread accumulation it was not.
- [`missed-reschedule-poke.md`](missed-reschedule-poke.md): the 2026-08-28 lost wakeup, where a
  placement went remote without its reschedule SGI.
- [`test-flakes.md`](test-flakes.md): a per-CPU test asserting an affinity nobody promised, and
  the bounded-yield tests under host contention.
- [`thread-dump.md`](thread-dump.md): what each field of `dump_threads` can claim, and what it
  is for.
- [`switch.md`](switch.md): the design behind `sched/switch.rs` and `sched/switch_decision.rs`:
  `schedule`'s rules, the pick, the per-thread grants, and the reaper's far side.
- [`rendezvous.md`](rendezvous.md): the design behind `sched/rendezvous.rs`: send, receive, call
  and reply, the capability rules on each arrival order, and freeing stranded callers.
- [`the-current-cpu-page.md`](the-current-cpu-page.md): the page a thread reads its core from, and
  what its kernel tests prove.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `undelivered-wake-gate` Name: provisional. After the main page's own heading.
- `reclaim-lost-wakeup` Name: provisional.
- `missed-reschedule-poke` Name: provisional.
- `test-flakes` Name: provisional.
- `thread-dump` Name: provisional.
- `switch` Name: provisional. After the source file it explains.
- `rendezvous` Name: provisional. After the source file it explains.
