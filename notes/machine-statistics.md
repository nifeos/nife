# The machine statistics page

*Name: provisional, minted 2026-10-24 (UTC) by comments/929-crates-18.*

`crates/machine_statistics_contract` defines one read-only page of machine-wide counters: physical
frames, and per core the ticks spent busy and idle, the context switches, the interrupts taken and
the run queue. `free` reads the memory line, `vmstat` reads all of it, and `top`'s summary reads the
run queue and the busy share. Milestone 126 (the `procps` package) built it, under DECISIONS §225
(`free` sees the machine and your share) part 2. `kernel/src/machine_statistics.rs` is the writes.

## A page, not a method, and not ambient

§225 ruled the shape. The figures arrive in a page somebody was granted, so an owner can withhold
them, and they are read with a load rather than a crossing. Nothing makes the page reachable by
default. The capability to it is minted once, for the progenitor, with `READ` and `GRANT`
(`kernel::user::boot_progenitor`), and nothing else in the system can name the frame. The progenitor
maps it into a child whose manifest declares `machine`, the same way it hands `date` the clock.

The page sits at a pair page on the address-space map, `0x005F_F000`, which calef ratified on
2026-09-27 (#1360's table). The address was chosen before milestone 206 (a program image has under
896 KiB) moved every image and stack to the second gigabyte. It was then the last page of the 2 MiB
block a child's program and stack shared, so it cost a spawn no page-table frame. Under the map it
costs what any pair page does, which `system_initializer`'s `JOB_REGION_PAGES` already counts. The
number stayed because it was ratified and nothing collides with it.

## The kernel writes the counters in place

There is no refresh to schedule. Each word is an `AtomicU64` the kernel updates where the event
happens. A tick bumps its core's busy or idle count and samples its run queue, and the cross-core
interrupt counts on the core it lands on. The frame allocator stores the free count after each
change, under its own lock, by whoever changed it. The one copy is the switch count: the context
switch counts into its own core's `PerCpu` block, and that core's tick stores the total in the page.
Every per-core word has exactly one writer, the core it describes. Each core's words sit in their own
64-byte line, so no two cores ever write the same cache line.

So a reader sees each word as it was a moment ago, and the words are not a consistent snapshot of
each other. `/proc/stat` makes the same bargain. A seqlock would put a store and a fence on the
kernel's hottest path, for a consistency no reader of these figures needs.

Before `publish`, the writes land in a static sink rather than behind a branch. The page pointer
starts at the sink, so a writer pays one load and one add whether or not the page exists yet, and
never a test. `publish` runs before the scheduler starts, so what is lost is at most the boot path's
own few interrupts. The switch count is not affected, because the tick copies the whole of it from
`PerCpu`.

## Limits

- `busy` is not split into user and system time. The tick knows which thread was running, not which
  privilege level it interrupted, so `vmstat` prints `us` and `sy` as one.
- The context-switch count is exact but up to one tick old, because the tick publishes it.
- The run queue is sampled once per tick per core, so it is up to one tick stale. A core that has
  stopped ticking (a parked core) keeps its last sample.
- A counter written before `publish` is lost, by the design above.
- Interrupts are counted where they reach the scheduler: the tick, a cross-core poke, and a device
  interrupt routed to a driver. A spurious or unrouted interrupt is not counted. The soak build's
  timer-driven `irq_notify` counts as a device interrupt, because it takes the same path on purpose
  (`kernel/src/soak.rs`).
- There is no swap counter, because nife does not page out. There is no page cache or block I/O
  counter either, because the filesystem and the block driver are userspace programs.
