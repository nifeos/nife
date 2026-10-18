# The overflows of 2026-08-14

*An appendix to [`notes/stack.md`](../stack.md), which explains the stack and is the page to read.
This file holds the account of two real kernel stack overflows found by CI on 2026-08-14. It moved
here on 2026-09-25 (UTC) under §212 (a prose budget). The move then edited it only to meet §213
(writing standards), splitting long sentences and dropping bold, with the meaning unchanged. The
five appendices were one file until then, in the order the main page's table lists them, so "above",
"below" and "this file" in them mean that whole sequence. The directory `notes/stack/` and this
file's stem are provisional names; naming is calef's.*


Two real kernel stack overflows in one day, on both architectures, found by CI. Written up from the
ground up, because the mechanism is worth understanding before the incident is, and because the
question "how did we introduce it" turned out to have an uncomfortable answer.

## The shape of a kernel thread stack

Every kernel thread gets four pages, 16 KiB, decided when the thread is created, and that is all
it will ever have. `thread::STACK_PAGES` is 4, and the figure came from what Linux uses for its own
arm64 kernel threads. It was never measured for this tree, which turns out to matter. *(This
section describes 2026-08-14. The next day's overflow, below, raised `STACK_PAGES` to 6, so the
worked addresses here are the old five-page stride.)*

Directly beneath it sits one unmapped page:

```
0xffffffd0001fe000  +--------------+
                    |  GUARD PAGE  |   unmapped: any access faults
0xffffffd0001ff000  +--------------+  <- stack bottom
                    |              |
                    |   16 KiB     |   frames pile downward from the top
                    |   of stack   |
0xffffffd000203000  +--------------+  <- stack top, where sp starts
```

The per-thread stride is therefore five pages (`0x5000`), guard page first. That arithmetic is how a
faulting address gets attributed to a stack: subtract `thread::STACK_AREA`, divide by `0x5000` for
the slot, and a remainder under `0x1000` means the guard page.

Why fixed size at all. In userspace a stack grows on demand: run off the end and the kernel maps
more. In a kernel there is nobody underneath to do that. The size is the size.

What the guard page buys, exactly. Without it, overflow is silent: thread A writes below its own
stack, lands in thread B's, and B crashes later doing something unrelated, arbitrarily far from the
cause. With it, the CPU faults on the first access and the crash is precise. The guard does not
prevent overflow. It converts an invisible failure into a legible one, which is the whole of its
value.

## Why an overflow is intermittent

Depth is a property of the path, not of the binary. A test that spawns a program, faults it, reaps it
and respawns stacks far more frames than one that reads a file. Test ordering, interrupt timing and
which core picked up the work all shift it. So the same kernel image overflows on one run and not
the next: milestone 108's branch went four green and one red on a byte-identical binary.

That is also why "re-run it and see" is the wrong instinct. A four-in-five pass rate looks like flake
and is actually a margin that has already run out.

## How it was introduced, which is the uncomfortable part

No single commit introduced it. There is no bad change to point at, and the milestone that was
held for hours on suspicion of causing it had a largest new frame of 128 bytes.

The margin was spent gradually, by ordinary code:

- `sched::reap_region_objects` carried a 6816-byte frame, of which 6144 was three arrays sized to
  their table maxima. `let mut doomed_eps = [0u64; MAX_ENDPOINTS]` is 4096 bytes of stack, and it
  reads as a bound rather than as an allocation. `MAX_ENDPOINTS` is 512 because that is a sensible
  ceiling on live endpoints; nothing about that number was ever a claim about stack.
- `sched::spawn_on` carries 4592 bytes, because a `Thread` travels by value into the thread table
  and a debug build copies it at each step rather than eliding. It is generic over the spawned
  closure, so every service gets its own instantiation: ten of them, all over the guard page.
- Milestone 84 had already measured the peak at 11672 of 16384 bytes, 71%, and written it in a
  table in notes/stack-high-water.md. 4712 bytes of headroom reads as comfortable. Nobody put that
  number next to a 6816-byte frame, and the two had never appeared on the same page.

Put them together and the arithmetic was impossible: one frame wanted 2104 bytes more than all the
headroom there was. It needed the right call chain on the right run to expose it, and when it did,
the change that happened to be in flight got the blame.

Nothing caught it because nothing was looking. A 6816-byte frame compiles without a warning. The
compiler will hand you a frame larger than the entire stack it will run on, because it has no idea
how big that stack is. No gate in the tree measured frame sizes until this day.

## The two faults, and what each taught

aarch64, on milestone 108's branch. `FAR_EL1` was `0xffff0010001b3000`, exactly the guard page of
thread stack slot 87, during the supervision and reap tests. Cause: `reap_region_objects`. Fixed by
rescanning for one endpoint at a time instead of collecting them all first, which took the frame from
6816 to 2560 bytes.

A wrong turn worth keeping: the first decode read `FAR` through `phys_to_virt` and concluded the
pointer was corrupted, because the physical half looked like `0x1b3000` with a stray bit 36. Bit 36
is `STACK_AREA` itself, placed 64 GiB up precisely so a stack address can never collide with the
virtual name of a physical one. A high-half address is not automatically a physmap address, and
masking off `KERNEL_VA_BASE` is not a decode until you know which region you are in.

riscv64, on the `thead-c906` CPU model, hours later. This one the kernel diagnosed itself, because
milestone 78's `stack::warn_if_guard_page` had merged in between:

```
*** KERNEL STACK OVERFLOW ***
0xffffffd0001fe008 is in THREAD stack slot 102's guard page (thread.rs).
```

The address is the lesson. It is 4088 bytes below the stack bottom, on a 4096-byte guard page.
Eight more bytes and there would have been no fault at all, just a corrupted neighbor.

## The rule that came out of it

A frame larger than the guard page defeats the guard page. One page is 4096 bytes; a function
whose frame exceeds that can move `sp` from inside the stack to below the guard in a single step,
touching nothing in between. No access lands in the guard, so nothing faults, and the write goes into
the neighboring thread's stack. The mechanism that makes overflow legible is bypassed entirely.

`script/stack-frame-check` gates exactly this, at 4096 rather than at any fraction of the stack, and
the first version of that gate got it wrong by picking a third of the stack instead. Ten `spawn_on`
instantiations sit over the line today, held at their current size by a ratchet until milestone 124
restructures them.

Growing the stack does not fix this shape, which is the counterintuitive part. `STACK_PAGES` 4 to
8 buys headroom, but the guard page stays one page, so an oversized frame still steps over it.
Growing the stack moves the overflow further away while leaving it silent when it finally arrives.
Shrinking the frame is what restores the fault.

## What was still open, and what closed it

Both of these were open on 2026-08-14 and both are answered below by the guard-page faults of
2026-08-16 and the walker they finally forced into the tree.

- Per-function frames are not call chains. `-Z emit-stack-sizes` says what one function costs, not
  which functions stack on top of each other, so it cannot produce a worst-case depth. The watermark
  in notes/stack-high-water.md is the other half. Neither is sufficient alone, and a call-graph
  walker would close the gap. Nothing in this tree had one until `script/stack-depth-check`.
- The riscv64 overflow is not proven fixed. The aarch64 cause was found and fixed; the riscv64
  fault is a different chain on a different slot, and milestone 124 is the prime suspect rather than
  a demonstrated cause. §19 parity says a fix that works on one architecture and silently not the
  other is the bug.

## What this file's numbers cite

Glossed here because the split made this a file of its own, and `script/citations` asks each
file to say once what a number cites. Each gloss is the record's own title.

- §19 (Architectural parity is a tenet; the targets are aarch64, riscv64, and x86_64)
- milestone 78 (The load-sensitive assertions, and the three that measure the wrong thing)
- milestone 84 (Stack high-water: measure kernel stack depth)
- milestone 108 (The drivers move onto frame capabilities)
- milestone 124 (A thread is born where it lives: the spawn path's copies)
