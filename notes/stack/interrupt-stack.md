# The per-CPU interrupt stack (milestone 124, 2026-08-16)

*An appendix to [`notes/stack.md`](../stack.md), which explains the stack and is the page to read.
This file holds milestone 124's per-CPU interrupt stack: what was built, the rule that holds it, and
what it measured (its BUGS are on the main page). The five appendices read in the order the main
page's table lists them, so "above", "below" and "this file" in them mean that whole sequence. The
directory `notes/stack/` and this file's stem are provisional names; naming is calef's.*


Everything above this line describes a kernel in which an interrupt was paid for by whoever it
interrupted. This section is the change that stopped that, and the honest account of how much it
bought, which is less than the headline suggests and in a different currency.

## The shape of the old cost

A trap arrives. The vector saves a frame at the live `sp`. Every frame of the handler above it is
built on the same stack, which is whichever stack was running: a kernel thread's 24 KiB, a
secondary's 64 KiB idle stack, or the boot stack the test suite runs on. So a kernel thread's stack
had to hold its own deepest chain plus a whole interrupt, and the interrupt could arrive at the
worst instant by definition, because that is what "asynchronous" means.

The 2026-08-15 section above has the arithmetic: ~11.7 KiB of thread, ~1.4 KiB resident while
blocked, and ~2.3 KiB for one preemption landing at the deepest point. Fifteen and a half of
sixteen. `STACK_PAGES` went 4 to 6 in response, and that block said in the same breath that growing
the stack was the interim and bounding the interrupt was the fix.

## What was built

One stack per core, 16 KiB over its own unmapped guard page, in a `(NOLOAD)` region beside the
secondary stacks (`.interrupt_stacks`, laid out by `kernel/src/interrupt_stack.rs`). A trap from
kernel mode saves its frame where it always did and then runs the handler over there; a trap from
user mode does not switch at all.

The mechanism is deliberately small. Rust decides (`interrupt_stack::top_for_trap`, which answers 0
for "stay") and three instructions of assembly move `sp`
(`dispatch_on_interrupt_stack` in vectors.s and trap.s). The dispatcher is split in two: an outer
half on the interrupted stack, and a body that runs on the interrupt stack.

Three things do not move, and each is a constraint rather than an omission.

- The trap frame. A preempted thread's frame must still be there when that thread runs again,
  arbitrarily later, and a per-core stack cannot promise that. 272 bytes on aarch64, 288 on riscv64,
  still billed to the interrupted stack.
- A trap from user mode. That thread's kernel stack is empty at the moment it traps, so there is
  nothing to relieve. And the syscall it is probably taking may block, which means its frames have
  to live on a stack that belongs to the thread.
- The deferred `schedule()`. This is the one that shapes the whole design, below.

## The one rule: nothing on an interrupt stack may context-switch away from it

A switch parks the running `sp` in the outgoing thread's `Context` and resumes it there later. Park
a per-core address and the thread comes back on bytes the next interrupt on that core has already
spent. That corrupts a stack rather than faulting, which puts it in the worst class of bug this
project has: invisible at the site, fatal somewhere else.

So the four lines of preemption moved out of `handle_irq` into `sched::preempt_if_needed`. They also
moved out of its riscv64 twin, where they had been written twice and had already drifted in their
comments. The new function is called by the dispatcher's outer half, which is provably back on the
interrupted thread's own stack.

Three mechanisms hold the rule, and the overlap is deliberate:

1. `sched::schedule` debug-asserts that `sp` is not on an interrupt stack.
2. `script/stack-depth-check` proves in CI, on both ISAs, that no context switch is *reachable* in
   the call graph from the interrupt-stack entry point. This is the strong one: it is a static
   property of the binary, checked every build, and it does not need the bad path to run.
3. The paragraph in `interrupt_stack.rs`, at the thing itself.

## What it measured, before and after

The static bound moved less than the headline suggests, and that is the interesting part.
`script/stack-depth-check`, which walks the call graph and hangs `-Z emit-stack-sizes` frames on it:

| at base commit `e56ae848` | aarch64 | riscv64 |
|---|---|---|
| handler chain a kernel-mode trap could leave on the interrupted stack, **before** | 3984 | 3888 |
| the same, **after** | 3728 | 3552 |
| deepest chain the interrupt stack itself carries (of 16384) | 3984 | 3888 |
| worst total on a thread stack, before | 13712 | 13344 |
| worst total on a thread stack, after | 13456 | 13008 |

Take these from a run and not from here, which the milestone 124 block says of its own numbers
for a reason this table then demonstrated. Merging `main` an hour later moved the aarch64 row from
3728 to 3904 and the total to 13632, with nothing in this change touching it: other lanes had
deepened what `schedule()` can reach. The riscv64 rows did not move. The gate prints the numbers and
the gate is the authority.

A 256-byte improvement in the worst-case bound is not what "moving 2.3 KiB off the thread" sounds
like, and the reason is worth reading. The remaining 3728 bytes are almost entirely `schedule()`'s
own chain: `preempt_if_needed` 16, `schedule` 448, `finish_switch` 224. Then comes the reaper
freeing a finished predecessor's address space, plus the panic-and-print tail the walker appends to
any chain that can reach a `panic!`. Every byte of that is the cost of scheduling at all, which a
thread already pays when it blocks voluntarily in `ipc_receive`. The handler is not on the thread
stack. That is the GIC or PLIC claim, the tick, the watchdog
(whose `dump_threads` alone carries a 1728-byte frame in a test build), the inbox drain, the
interrupt routing.

And the measured watermark did not move at all, which is the other half of the honest answer.
The suite was run on the same machine either side of the change, and every painted stack read the
same number to the byte:

| high-water, one full suite run | aarch64 before | aarch64 after | riscv64 before | riscv64 after |
|---|---|---|---|---|
| boot (64 KiB) | 53936 | 53936 | 54344 | 54344 |
| secondary (64 KiB) | 6624 | 6624 | 6568 | 6568 |
| thread (24 KiB) | 9536 | 9536 | 9344 | 9344 |
| **interrupt (16 KiB)** | n/a | **976** | n/a | **1088** |

(The aarch64 interrupt row reads 1024 on the merged tree rather than 976, 48 bytes more, because
`top_for_trap` became `#[inline(always)]` for the icount reason below and its locals now sit in the
dispatcher's frame. Fifteen further suite runs, ten aarch64 and five riscv64, reproduced every number
in this table to the byte.)

Read it carefully, because it says two things and only one of them is comfortable. The handler is
demonstrably running over there: ~1 KiB of a previously unpainted stack is now used, which nothing
but the switch could have done. And nothing got shallower, which means that in this suite the
deepest byte of every other stack was reached by ordinary code rather than by an interrupt landing
on top of it. That is exactly what a watermark can and cannot say. The
interrupt-at-the-worst-instant case is rare (which is why the CI fault was intermittent, one run in
six). A measurement of what did happen has nothing to report about a case that did not.

So the honest statement of what changed is not "the thread's worst case dropped by a third". It is:

> After this, a preemption costs the interrupted thread a trap frame plus the same scheduler tail
> it would have paid to block on its own. The handler is bounded on a stack of its own.

That is a structural property rather than a number, and it is the one that stops the budget being
spent by accident. A handler that grows now trips `script/stack-depth-check`'s interrupt-stack
ceiling or the high-water gate on that stack, instead of quietly making every thread's margin
smaller.

## `STACK_PAGES` could come down, and deliberately did not

The measured case for 6 pages was the ~15.5 KiB sum above, of which ~1.6 KiB has now moved. At 5
pages (20 KiB) the static worst case of 13456 still fits with 6.6 KiB spare, and 4 pages (16 KiB, the
size that overflowed in CI on 2026-08-15) fits the static bound with 2.5 KiB spare.

It stays at 6. #225 raised it one day earlier with a measurement in hand, after a real overflow
in CI, and lowering it needs its own measurement rather than the argument that something else got
better. The number to beat is a *measured* worst case under load on the runner that produced the
fault, not a static bound computed on a laptop. Nothing here produces that number, so nothing here
is entitled to spend it. It is a lane of its own, and the instruments it needs now exist.

## What it cost

160 KiB of RAM and address space: `MAX_CPUS` (8) slots of 16 KiB stack over a 4 KiB guard,
whether or not a core ever fills the seat, exactly like the secondary stacks beside them. It is a
`(NOLOAD)` region, so the flat image does not grow; `__image_size` does, which is what tells the
bootloader the memory is ours.

And instructions per trap, which the icount tripwire caught and which is the more interesting
half. The first version failed `script/bench --check` on three benchmarks at once: `ctx_switch`
+49.7%, `ipc_rtt_el0` +26.5%, `sink_throughput` +17.5%. None of those is a trap-heavy benchmark in
the way `null_syscall` is, and that pattern was the clue: all three are scheduler-heavy, and the
cost was the `debug_assert!` in `schedule()`. It asked each of eight slots in turn through two
non-inlined helpers, which is the obvious way to write "is this address in one of these ranges" and
costs about 145 ticks per context switch in the debug build the tripwire measures. Rewritten as one
subtraction against the contiguous region, all three returned inside the band.

Two smaller ones followed from asking a question one more time per trap, and both are worth knowing
because they are properties of the *debug* build rather than of the code:

- `interrupt_stack::top_for_trap` and `contains` are `#[inline(always)]`, for a measured reason. A
  debug build inlines nothing, so a policy function that answers on its first branch still costs a
  frame, a prologue and a return on every trap.
- `from_lower_el` (now `is_from_lower_el`) was `(8..=11).contains(&index)`, which compiles to a real
  call into `RangeInclusive::<u64>::contains::<u64>` at `-O0`, on every trap, three times over.
  Written as two comparisons it is free. That alone more than paid for this milestone's extra
  dispatcher frame: `null_syscall` ended 11.7% faster than its old baseline on aarch64 while
  riscv64, which had no equivalent generic call to lose, paid the split at +10.1%. Both baselines
  were re-recorded in their own commit; nothing else moved by more than 1.9%, so nothing else was
  touched.

## What this file's numbers cite

Glossed here because the split made this a file of its own, and `script/citations` asks each
file to say once what a number cites. Each gloss is the record's own title.

- milestone 124 (A thread is born where it lives: the spawn path's copies)
