# The machine statistics page

`crates/machine_statistics_contract` defines one read-only page of machine-wide counters: physical
frames, and per core the ticks spent busy and idle, the context switches, the interrupts taken and
the run queue. `free` reads the memory line, `vmstat` reads all of it, and `top`'s summary reads the
run queue and the busy share. Milestone 126 (the `procps` package) built it, under DECISIONS §225
(`free` sees the machine and your share) part 2.

## A page, not a method, and not ambient

§225 ruled the shape. The figures arrive in a page somebody was granted, so an owner can withhold
them, and they are read with a load rather than a crossing. Nothing makes the page reachable by
default. The progenitor holds the kernel's only capability to it and maps it into a child whose
manifest declares `machine`, the same way it hands `date` the clock.

The page sits at a pair page on the address-space map, `0x005F_F000`, which calef ratified on
2026-09-27 (#1360's table). The address was chosen before milestone 206 (a program image has under
896 KiB) moved every image and stack to the second gigabyte. It was then the last page of the 2 MiB
block a child's program and stack shared, so it cost a spawn no page-table frame. Under the map it
costs what any pair page does, which `system_initializer`'s `JOB_REGION_PAGES` already counts. The
number stayed because it was ratified and nothing collides with it.

## The kernel writes the counters in place

There is no refresh to schedule. Each word is an `AtomicU64` the kernel updates where the event
happens. A tick bumps its core's busy or idle count and samples its run queue, and the frame
allocator stores the free count after each change. The one copy is the switch count: the context
switch counts into its own core's private block, and that core's tick stores the total in the page.
Every per-core word has exactly one writer, the core it describes. Each core's words sit in their own
64-byte line, so no two cores ever write the same cache line.

So a reader sees each word as it was a moment ago, and the words are not a consistent snapshot of
each other. `/proc/stat` makes the same bargain. A seqlock would put a store and a fence on the
kernel's hottest path, for a consistency no reader of these figures needs.

## Limits

- `busy` is not split into user and system time. The tick knows which thread was running, not which
  privilege level it interrupted, so `vmstat` prints `us` and `sy` as one.
- The context-switch count is exact but up to one tick old, because the tick publishes it.
- The run queue is sampled once per tick per core, so it is up to one tick stale. A core that has
  stopped ticking (a parked core) keeps its last sample.
- There is no swap counter, because nife does not page out. There is no page cache or block I/O
  counter either, because the filesystem and the block driver are userspace programs.
