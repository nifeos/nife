---
status: DECIDED
raised: 2026-08-18
decided: 2026-08-25
ratified_by: calef
---

# 96. Process kernel or event kernel, and how to decide it

calef, 2026-08-25, in conversation, on the recommendation below as written:
*"Don't decide yet."* Raised 2026-08-18 by calef, asking how to *think about* this as a decision
rather than asking for the answer. It exists because notes/l4-lessons.md found that this tree has a
process kernel, that seL4, OKL4 and NOVA all replaced theirs with event kernels, and that nothing
anywhere in this repository records the model as a choice. It is what the code grew into.

**Ratified as the three-step plan below, not as a deferral without a record.** Take the cheap half
(direct process switch, under §95's Option 2/3 split) now, as a process kernel. Build the instrument
that could actually decide the larger question (a real multi-tasking workload benchmark, since
every instrument this project owns today is a micro-benchmark that would show approximately nothing),
minted as [milestone 168](../roadmap/0168-multitasking-benchmark.md). Re-open this section once
that number exists, or sooner if a real customer-path workload starts creating threads in the
hundreds.

What is blocked: nothing. The system runs. This is a decision about whether to spend, and the
useful output of writing it down is that the question stops being invisible.

## What the two things are

A process kernel gives every thread its own kernel stack, so a thread that blocks inside the
kernel simply stops on that stack and resumes there. That is what we do: `STACK_PAGES` is 6, plus a
guard page, and the word "continuation" appears nowhere in `kernel/src`.

An event kernel has one kernel stack per core. A thread that must block stores a continuation,
a small explicit record of what to do when it resumes, and the kernel stack unwinds. seL4, OKL4 and
NOVA are all this.

## What class of decision it is, which is the part calef actually asked

Run this tree's own test from CLAUDE.md, which is **not** "can I revert the commit" but "who else
has already acted on this":

- **No user program can tell.** The syscall ABI does not change. Nothing about `SEND`, `RECV` or
  `CALL` is different under either model.
- **No wire format, no name, no published fact, no stored secret.** None of the irreversible
  categories is touched.
- So by the stated test this is reversible, and it is expensive in exactly the currency agents
  made cheap: kernel code.

**But it has a property the reversible category does not usually have, and this is the thing worth
naming.** Every blocking operation written *after* the decision is written in the chosen idiom. The
cost of switching therefore grows with every feature added, and what grows is not only code but a
convention in the next contributor's head, which CLAUDE.md lists among the things that did **not** get
cheaper. So it is neither of the two categories that file describes. It is a third: reversible, but
appreciating.

The practical consequence is the opposite of the usual advice for reversible decisions. Deferring is
not free, so the thing to avoid is not a hasty choice, it is an *unmade* choice that keeps getting
more expensive while nobody is looking. Which is precisely the state this tree was in until the audit.

## The four inputs, and three of them are already settled

1. Memory. Dead as an argument here, and it was the paper's headline.

Warton measured the event kernel's per-thread memory at a quarter of the process kernel's. Our
exposure is bounded by construction: `MAX_THREADS` is 256, so kernel stacks (6 pages, 24 KiB each)
total 6.00 MiB, static. An event kernel would spend `MAX_CPUS` (8) stacks, 192 KiB, plus larger
TCBs for the continuations. The saving is 5.81 MiB.

Against what. **No board's memory size is recorded anywhere in this tree**, so a percentage of a
board is an assumption rather than a measurement, and against an assumed 4 GiB board it flatters
the argument by more than an order of magnitude. The number that is measured is the machine this
kernel runs on: all three QEMU runners use `-m 256M`. So:

| denominator | share |
|---|---|
| a 4 GiB board (assumed, no board recorded) | 0.14% |
| **256 MiB, what the suite actually runs on** | **2.27%** |

The conclusion survives, and it is worth being precise about why, because the honest denominator
removes the easy version of the argument. 2.27% is not a rounding error and should not be waved
away as one. It is still not a reason to change kernel models: Warton's result came from
resource-starved embedded systems, ours is a deliberate emulator setting rather than a hardware
limit, and 5.81 MiB buys nothing if the performance case (input 3, the live one) does not hold. The
memory argument is dead here because it is **not decisive**, not because it is too small to see.

And "here" is doing work in that sentence, which calef named on 2026-09-04, asking whether
today's decisions foreclose hardware this project cannot reach yet: *"These things change over time.
I just want to ensure that because we cannot today we don't make decisions that block it in the
future."*

**This input is closed conditionally, on a scale, and the condition is not a detail.** The paragraph
above already says the denominator is a deliberate emulator setting rather than a hardware limit and
that Warton's result came from resource-starved embedded systems. Read together, those say the
memory argument is dead *at the scale this project runs at* and would revive at a smaller one. At
256 MiB the static kernel-stack reservation is 2.27%; the arithmetic is linear, so on a 64 MiB
machine it is 9.1%, and on 32 MiB it is 18%, which nobody would wave away.

A 2026-09-13 caveat on how this input reads Warton's paper is in
[`warton-caveat.md`](0096-process-kernel-or-event-kernel/warton-caveat.md).

What would revive it, stated so a future reader does not have to re-derive it. A target where
`MAX_THREADS * STACK_SLOT_SPAN` is a material fraction of RAM. notes/target-hardware.md's
requirements do not exclude such a machine: an application-class SoC with an MMU, 32 KB of L1i and a
few tens of megabytes clears every one of them, and the form factors that shape belongs to (a watch,
a sensor, an appliance) are exactly where an open MMU-class device does not exist today.

This is not an argument for changing kernel models. It is a note that input 1 is answered for the
machines this project owns and is *unanswered* for a class it has not excluded, so a future reader
finding "the memory argument is dead" should not read it as "the memory argument cannot come back".
Input 3, performance, remains the live one, and milestone 168's instrument was built on 2026-09-04
to produce its number.

A larger reservation sits behind this one and is worth naming, since a reader who checks the
stacks will find it: `kmem::KERNEL_OBJ_PAGES` is 2048 pages, and its carve is eager
(`memory_region::create` takes the whole thing on the first kernel-object need and never returns
it). That is 8.00 MiB, 3.1% of the 256 MiB machine, and the kernel stacks above are a subset of
it rather than additional to it.

2. Shrinking the stacks instead. Closed off, and checked rather than assumed.

The obvious cheap alternative is to keep the model and take part of the memory win by making stacks
smaller. It is not available. Milestone 84's high-water instrument measured thread stacks at 11,352
bytes on aarch64 and 11,672 on riscv64, across roughly 420 stacks over the full suite. At 4 pages
(16 KiB) that is 69 to 71%, and stacks are 6 pages because milestone 124 (a thread is born where it
lives) found `spawn_on` carrying a 4,592-byte frame that could step past a one-page guard. There is
no slack to reclaim.

3. Performance. The live argument, and we cannot currently measure it.

Warton found the event kernel generally within 1% on micro-benchmarks and 20% better on a
multi-tasking workload (AIM7). Read that pairing carefully, because it is the uncomfortable part:
every instrument this project owns is a micro-benchmark. `ipc_rtt`, `ipc_rtt_el0`, the icount
tripwire and milestone 132's footprint gate would all show approximately nothing. The one number that
would move is the one we have no way to produce.

**Correction, 2026-09-13: the 20% was never a number this project could reproduce, and the premise
behind milestone 168 does not hold.** The retrospective names only *"the Pistachio process kernel vs
an event-based (single-stack) kernel with continuations on an ARMv5 processor"* (section 4.1, page
1:16) and never says what userland AIM7 ran under. Warton's own thesis does (*Single Kernel Stack
L4*, BE thesis, UNSW, November 2005, section 5.4): *"The AIM7 benchmark was modified slightly so that
it could run on Wombat."* Wombat is the paravirtualised ARM Linux, so the 20% is a delta between
two microkernels measured through a hosted Linux's syscall path, where milestone 168's instrument
runs native tasks on this kernel's own primitives. It is also a ratio between two kernel models,
and this tree has one, so the instrument produces one arm and no ratio.

Three more facts the retrospective compresses away, each of which this section had been reasoning
without. Warton's run disabled AIM7's filesystem jobs (the ramdisk was too small) and its network
jobs (Wombat had no `GetHost`), so two of the three categories `crates/job_mix` records as missing
were absent from the cited run too. It was *"2 clients with the normal workload file"*, run in two
user tasks, so there was no task-count sweep. And Warton doubted the result himself: he expected
the single-stack kernel to perform similarly or worse, called the outcome something to treat *"with
skepticism until it can be satisfactorily explained"*, and records that the cache simulation which
would have explained it was never run for lack of time.

What this section should take from it is the mechanism, not the number. The only explanation
Warton offered is reduced kernel cache and TLB footprint. Whether per-thread kernel stacks displace
enough cache to cost throughput on this kernel is local, open and measurable: it is what
`app_displacement` measures in isolation, what milestone 134's E1 found bending 8 to 11% by 64 to 96
threads, and what a knee in milestone 168's jobs-per-minute curve would confirm under load. That is a
real input to this decision. Matching somebody else's 20% was never available. See
`notes/job-mix.md`, which carries the quotations, both URLs and the read date.

Update, 2026-09-04: half of that is now false, and the half that moved is the one this section
said would not. Milestone 134's E1 (IPC round trip against thread count) ran on radon, six boots
on a `single_hart` card. It is still a micro-benchmark, so the paragraph above stands as written
about AIM7. But it is a micro-benchmark with a thread-count axis on a 32 KB L1i, and it did not
show approximately nothing:

| threads | ns per round trip | vs 2 threads |
|---|---|---|
| 2 | 1152 | 1.00x |
| 4 | 1189 | 1.03x |
| 8 | 1377 | 1.20x |
| 16 | 1938 | **1.68x** |
| 32 | 1953 | 1.69x |
| 64 | 1966 | 1.71x |
| 96 | 1975 | 1.71x |

A 68% rise with the bend between 8 and 16 threads, then flat to 96 (the last four points span
1.2%). That is the shape of a per-thread working set crossing a cache and then saturating, which is
what a process kernel's per-thread kernel stacks would produce and what an event kernel's shared
continuation would not. The same sweep on patagonia rose 8 to 11% across the whole range with no
identifiable knee, which is the large-cache muting E1's own note predicted.

Two things stop this from answering the section, and both should be said before anyone quotes
it.

The workload is not in the regime. E2's thread census found the customer path running 4 to 8
threads, which is on the flat part *below* the knee, where the effect is 3 to 20% rather than 68%.
Input 3's re-open trigger was written as *"a customer-path workload starts creating threads in the
hundreds"*, and the measurement now says the trigger should have been in the low tens. That is
a correction to the trigger, not a reason to pull it.

And the mechanism is inferred, not observed. A knee at 16 threads on a 32 KB cache is
consistent with kernel stacks displacing each other, and it is also consistent with several other
things (the scheduler's own state, the archive's heap layout, the rendezvous structures). Nothing
in this tree reads a cache-miss counter on any architecture, so the attribution is an argument from
shape. Milestone 134's tier B is what would settle it.

So: the deciding number still does not exist, and the instrument is closer to producing one than
this section assumed. The honest movement is that the performance input went from unmeasurable to
partially measured, in the direction that favors the event kernel, on an axis the workload does
not currently use.

4. Verification. The paper's reason does not transfer; a weaker version might.

Their stated argument is specific: an event kernel avoiding in-kernel page-fault exceptions
"preserves the semantics of the C language", and staying inside C's semantics reduces verification
complexity. We are not verifying C, so that argument is simply not ours. Milestone 193 (put
`kernel/src` within reach of the prover) proves two syscall properties in `kernel/src`, and
`kernel/src/arch/` is unreachable, but the conclusion is about C's semantics rather than about
coverage.

The version that may transfer is an inference and is marked as one: a continuation is explicit,
data-shaped state, where a kernel stack is implicit, control-shaped state, and explicit
data-shaped state is what a model checker can reason about. If the verification frontier ever moves
into the kernel, the event kernel is the shape that would let it. Nobody has demonstrated that here,
and it should not be quoted as though somebody had.

On dismissing the whole thing as an embedded-era artifact, which input 1 invites: the authors
close that door themselves. The choice "was driven initially by the realities of resource-starved
embedded systems and later the needs of verification", but "the approach's benefits are not
restricted to those contexts, and we believe it is generally the best approach on modern
hardware."

## The decoupling that makes this decision less urgent than it looks

Direct process switch does not require an event kernel. Traditional L4 had direct process switch
*and* per-thread kernel stacks; the event kernel arrived years later with L4-embedded. So the cluster
notes/l4-lessons.md identified is separable after all:

- Row 14, direct process switch, is available to us *as a process kernel*, and it is the change
  §95 is already deciding about and milestone 132 already measured the ground for.
- Row 11, the kernel model, is the expensive commitment underneath it.

That matters because it means the cheap win is not gated on the expensive decision. We can take the
fastpath work without settling this, and we should not let the larger question hold the smaller one.

## The question that actually settles it

CLAUDE.md's test for an argument that sounds like design: would I still choose this if both options
were the same amount of work?

Answered honestly: no, I would not choose the process kernel at equal cost. The paper recommends
the event kernel for modern hardware in its own voice, the direct-switch and Benno-scheduling wins
fall out of it rather than being bolted on, and the explicit-state property points the right way for a
verification project. So the case for staying as we are is an effort argument, and per §92 it has
to say so in those words rather than dressing itself as architecture.

What makes the effort argument *legitimate* here rather than merely convenient is input 3. We would
be spending a pervasive kernel rewrite against a benefit we cannot measure, on the word of a
twenty-year-old result from a different kernel on ARMv5. This project's standard is measure, do not
argue, and adopting an event kernel today would be arguing.

## Recommendation

**Do not decide it yet, and make the thing that blocks the decision explicit so it stops being
invisible.** Three steps, in order, none of which is the rewrite:

1. Take the cheap half separately. Pursue direct process switch under §95 as a process kernel.
   It is independently justified, and it retires the part of the gap that is not expensive.
2. Build the instrument that could decide it. The blocker is that a multi-tasking workload is the
   only place the difference appears, and we have none. That wants a roadmap block of its own, and it
   is useful well beyond this question: milestone 25's cross-OS comparison has the same hole, and
   milestone 127's TX1 is where such a number would finally mean something.
3. Re-open this section when that number exists, or sooner if a customer-path workload starts
   creating threads in the hundreds, which is the condition under which input 1 stops being dead.

If calef says no to all of it, the outcome is that the tree keeps a process kernel and now says so
on purpose, with the reason and the trigger written down. That alone is worth more than the status quo
it replaces, which was an unexamined default.
