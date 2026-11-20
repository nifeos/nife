---
status: DECIDED
raised: 2026-09-21
decided: 2026-09-21
ratified_by: calef
---

# 204. How userspace asks where a thread runs

calef, 2026-09-21. *(Number provisional until the merge queue lands it.)*
Raised while unblocking the job-mix supervisor's move to userspace, which needs a placement census
from a userspace supervisor. (That milestone's block is not yet on `main`, so it is named here
rather than cited; the citation lands when it does.) The question grew past that: it is how any per-thread fact reaches userspace, and it
amends §150 inside the window §150 itself left open.

## What was decided

Two mechanisms, because there are two different questions.

1. Observing another thread is a selector. `abi::rendezvous::SURVEY` takes an argument naming
   which record the caller wants, and returns that record's words. A new fact is a new selector
   value, not a new register.
2. A thread observing itself is a per-thread page, on the shape Linux's `rseq` uses: the kernel
   writes the current CPU into a small per-thread structure the thread reads with no system_call.

## What this overturns, and why that is on purpose

§150 (how does a thread's CPU time reach userspace?) ruled sub-choice 3, *"a widened `SURVEY`
return, not a second method ... a fourth carries the figure"*, and flagged it as the irreversible
one: *"the moment to overturn it is before milestone 282 ships, not after."* Milestone 282 (a
thread's CPU time, and the `top` it makes possible) is NOT-STARTED, so this is that moment, used as
designed. The ruling that CPU time is tick-sampled, per-thread and scheduled on-CPU time stands
untouched; only how it reaches a reader changes, from the fourth word to a selector record.

## Why, and the deciding fact was a forecast rather than an argument

The register row could hold five words with no new mechanism, and appending one is the fewest moving
parts if the row stops. calef's answer was that he expects a third and a fourth fact, which
decides it: a mechanism that must be redesigned at the sixth field is the wrong mechanism at the
fourth.

The comparable systems agree and were read rather than recalled:

| system | per-thread facts | mechanism |
|---|---|---|
| Linux, `/proc/[pid]/task/[tid]/stat` | **52 fields**, `(39) processor` being *"CPU number last executed on"* | a page-backed pseudo-file |
| Zircon | 4 topics on a thread, ~11 fields, of **41** `ZX_INFO_*` topics | `zx_object_get_info(handle, topic, buffer, size)`, a selector |
| macOS, `thread_basic_info` | ~8 | a fixed record |
| seL4 | none; no thread enumeration exists | by design: the supervisor built the threads |

Nobody grows a register row. And Zircon's `zx_info_thread_stats_t` holds `total_runtime` and
`last_scheduled_cpu` in one struct, which is this decision's two fields already living together in
the system closest to nife's shape. Fuchsia also needed a governance RFC (RFC-0084) whose purpose
was adding more fields to a runtime record, which is the third and fourth arriving in someone else's
tree with paperwork.

## Why the selector rather than a domain-wide statistics page

A shared page mapped into a supervisor was the other candidate and was refused, for reasons that are
this project's own rather than general:

- It is ambient authority. An invocation checks `ENUMERATE` at the moment of the call; a mapped
  page keeps answering after the capability is revoked, unless revocation also walks mappings. Two
  lanes spent 2026-09-21 on exactly that class (a capability parked in a hand-off slot surviving a
  sweep, and `map_physical` never recording its mapping). Creating a second authority that outlives
  its capability, to save a system call, is the wrong direction for the claim this kernel is a
  demonstration of.
- The side channel goes from polled to continuous. §150 weighed a polled aggregate leak and
  accepted it; a tick-updated page is a different magnitude and would need re-arguing.
- Slot reuse yields plausible wrong data. A dead thread's slot is reused, and a reader sees a
  well-formed number belonging to a different thread, absent generation counters. calef's ruling the
  same day, on the counter frequency, was that a wrong number is worse than no number.
- A fixed table caps how many threads a domain can expose, which is the ceiling the selector was
  chosen to remove.
- It is prover-hostile where risk 2 is already weakest: a lock-free structure with a concurrent
  writer and reader under the weak ordering rule 4 tells us to assume.

And the speed it buys is below the noise floor of its consumer. An IPC round trip measures
~705 ns (debug-build QEMU numbers, so an upper bound), and a `SURVEY` is one system call, cheaper
than a round trip. Sixty-four threads at a 1 Hz refresh is about 45 microseconds per second,
roughly 0.005% of a core.

## Why a page is nonetheless right for the self case

The high-frequency consumer is real, and identifying it is what separated the two mechanisms: a
memory allocator keeping a per-CPU cache reads *its own* CPU on every allocation. That is
`rseq`'s reason for existing. Linux made it a memory read rather than a system call because the
vDSO path is x86-specific and impossible to implement on AArch64. The memory approach measured
20x faster on x86 and 35x on ARM.

**That consumer wants one thread's own CPU, not a table of everyone's**, so it carries none of the
objections above: no slot reuse, no table cap, and the page is the thread's own.

## What is not decided here

- Affinity. Whether a program may *choose* where its threads run is seL4's position and remains
  open. calef's reading, 2026-09-21: placement could be a userspace program that gets smart about
  it. Placement at thread creation is cheap and works today. Re-placing a running thread needs
  migration, which this kernel does not do and seL4 deliberately refuses. That is a question the
  size of §96 (process kernel or event kernel) rather than a feature.
- A push mechanism for a profiler. Sampling at 1 to 4 kHz wants the kernel to push into a ring
  buffer rather than anyone to poll. Named here so a later reader does not mistake the selector's
  scope for it.

## BUGS

- Two mechanisms are more to learn than one, and the tree owes a reader one sentence, where they
  meet either, saying which question the other answers.
- The selector's record layouts are still wire formats. This decision moves the growth problem
  from registers to records; it does not abolish it. Each new selector value is as expensive to
  un-ship as a fourth word would have been, and the gain is that adding one requires no change to
  what an existing reader already parses.
