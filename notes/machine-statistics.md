# The machine statistics page

*Name: provisional. calef names notes.*

One frame holds the machine-wide counters `free`, `vmstat` and `top` read, for §225 (`free` sees the
machine and your share) part 2. They are the free frame count, and per core the busy and idle ticks,
the context switches, the interrupts and the run queue. The layout is
`crates/machine_statistics_contract`'s, and `kernel/src/machine_statistics.rs` is the writes.

## How the page is written

The counters live in the page itself, so there is no copy and no refresh to schedule. Each per-core
word is written only by its own core: the tick and the cross-core interrupt run on the core they
count, and the tick publishes the switch count the core keeps in its own `PerCpu` block. Each
core's words are one cache line of their own, so nothing contends. The frame count is written under
the allocator's own lock, by whoever changed it.

Before `publish`, the writes land in a static sink rather than behind a branch. The page pointer
starts at the sink, so a writer pays one load and one add whether or not the page exists yet, and
never a test. `publish` runs before the scheduler starts, so what is lost is at most the boot path's
own few interrupts. The switch count is not affected: it lives in `PerCpu`, and the tick copies the
whole of it.

The capability to the page is minted once, for the progenitor, with `READ` and `GRANT`
(`kernel::user::boot_progenitor`). Nothing else in the system can name the frame.

## BUGS

- A counter written before `publish` is lost, by the design above.
- The switch count on the page is up to one tick old, because the tick publishes it. A core that
  stops ticking leaves its last copy, like `RUNNABLE`.
- Interrupts are counted where they reach the scheduler: the tick, a cross-core poke, and a device
  interrupt routed to a driver. A spurious or unrouted interrupt is not counted.
- The soak build's timer-driven `irq_notify` counts as a device interrupt, because it takes the same
  path on purpose (`kernel/src/soak.rs`).
