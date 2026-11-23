# The guard-page faults of 2026-08-16, which were not overflows

*An appendix to `nifeos/nife:notes/stack.md`, which explains the stack and is the page to read.
This file holds the account of the 2026-08-16 guard-page faults, which were not overflows. It moved
here on 2026-09-25 (UTC) under §212 (a prose budget). The move then edited it only to meet §213
(writing standards), splitting long sentences and dropping bold, with the meaning unchanged. The
five appendices were one file until then, in the order the main page's table lists them, so "above",
"below" and "this file" in them mean that whole sequence. The directory `notes/stack/` and this
file's stem are provisional names; naming is calef's.*


Two more guard-page faults, one per architecture, in the same test
(`user::supervision_tests::a_faulting_child_reports_to_its_supervisor_and_is_reaped_then_respawned`),
intermittent on both. They read exactly like the 2026-08-14 pair above and they are a different
thing, and the reason the difference took a day to see is that the kernel's own report asserted
the wrong half of it.

## What the two faults said

```
*** KERNEL STACK OVERFLOW ***
0xffff0010001b3000 is in THREAD stack slot 87's guard page (thread.rs).
bottom 0xffff0010001b4000, so sp went 4096 bytes past it, on a 16384-byte stack.
ESR_EL1 0x0000000096000047   FAR_EL1 0xffff0010001b3000     (aarch64, run 31920141776)

0xffffffd0001fe008 is in THREAD stack slot 102's guard page (thread.rs).
bottom 0xffffffd0001ff000, so sp went 4088 bytes past it, on a 16384-byte stack.
scause=0xf (code 15)                                        (riscv64, PR #213's cpu matrix, rv64)
```

## The fact that settles it, and the tree was already holding every piece of it

There are two addresses, not six faults. Every recorded guard-page fault on this project lands
on one of exactly two, and they were written down in three different files over five days without
anyone putting them side by side:

| when | where recorded | address | slot |
|---|---|---|---|
| 2026-08-11ish, two `cpu matrix` runs | `sched.rs`, `guard_page_at`'s doc | `0xffffffd0001fe000` | riscv64 slot 102 |
| 2026-08-13, one run in five | notes/frames.md, milestone 108's BUGS | `0xffff0010001b3000` | aarch64 slot 87 |
| 2026-08-14 | this file, "the overflows of 2026-08-14" | `0xffffffd0001fe008` | riscv64 slot 102 |
| 2026-08-16, merge queue | above | `0xffff0010001b3000` | aarch64 slot 87 |
| 2026-08-16, PR #213 `cpu matrix` | above | `0xffffffd0001fe008` | riscv64 slot 102 |

Same slot per architecture, every time, over five days, across milestone 124's restructuring of the
entire spawn path (the worst `spawn_on` instantiation went from 4592 bytes to 1040) and across
#157's fix to `reap_region_objects`. The addresses did not move by one byte.

A depth-driven overflow cannot do that. Depth is a property of which calls ran and when an
interrupt landed, which this note says in its own words two sections up ("the same kernel image
overflows on one run and not the next"). An overflow's faulting address wanders with the chain that
produced it. These do not wander at all. That is a fixed computation landing on a fixed address, and
the intermittency is in whether the path runs, not in where it ends up.

And one register in the 2026-08-13 dump argues the same thing, directly under the sentence that
concluded the opposite. notes/frames.md records `x8 = 0xffff0010001b7a90` and reads it correctly
as slot 87's own stack, "1392 bytes below its top", and then concludes "a 16 KiB kernel stack ran
out". A stack with 1392 bytes used has not run out; it has used 8% of itself. The two sentences are
adjacent and only one of them can be true.

That argument is wrong, twice over, and this file already contained both refutations (2026-08-17).
The paragraph above it is a near-verbatim duplicate. The claim they share is answered by "The
repeated fault address is a signature, not a coincidence" forty lines earlier in the 2026-08-15
section. Any fault that reaches the exception vector's own frame store cascades down to the same
slot base. The walk terminates at the first `sp` whose whole frame clears the guard and the stores
run upward from `sp` in 16-byte steps. So a wandering `sp` produces a fixed *reported* address, and
address stability says nothing about the writer. What it does say is which slot, and that turned out
to be the real signal. Slot 87 survived `STACK_PAGES` going 4 to 6, which changed the address and
not the index, so what repeats is a position in the allocation sequence. See "a kernel stack freed
under its owner" at the end of this file. The verdict here (not depth) was right; the reasoning was
not, and the reasoning is what a reader would have carried forward.

That also revises the 2026-08-14 entry above. Its aarch64 fault was attributed to
`sched::reap_region_objects`'s 6816-byte frame and closed by #157; the same address came back after
that fix and after milestone 124's. Either the attribution was wrong, or there were two faults at
one address and only one of them was fixed. The frame *was* real and shrinking it *was* right (a
frame larger than the guard page defeats the guard page regardless), so nothing about that work is
wasted. But it did not close this.

## The arithmetic that says it is not depth

The deepest chain a kernel thread stack can carry is 13792 bytes on aarch64 and 13344 on
riscv64, measured by `script/stack-depth-check` over the same test binary CI builds:

| | aarch64 | riscv64 |
|---|---|---|
| longest chain from `thread_entry` (a kernel thread's own work) | 9456 | 9168 |
| trap frame the vector builds | 272 | 288 |
| handler chain that can nest on kernel code (no system call or user-fault arm) | 4064 | 3888 |
| **worst total on a 16384-byte stack** | **13792** | **13344** |
| measured high water, milestone 84's watermark, 31 runs on this machine | 9536 to 10600 | 9344 |

(The aarch64 handler row read 3984 before this work and 4064 after, because the `sp` line added to
`warn_if_guard_page` is itself on the fault path. Eighty bytes to make the report say what it
measures is worth it, and the instrument noticing its own cost is the sort of thing that says it is
measuring the right binary.)

Two things make that close to a bound. The call graph is acyclic: no recursion, so the longest path
is the worst case for everything the graph contains. And no frame over the 4096-byte guard page is
reachable from a thread-stack entry point at all on either ISA. So milestone 124's fix does cover
every path that reaches a thread, and the frame-jumps-the-guard hazard is genuinely closed there.

The third thing is where the measurement lands, and it took two corrections to state properly.
A first draft said the walker and the watermark agreed to the byte on aarch64 at 9536, which came
from a script still mis-parsing RISC-V local labels. A second said the watermark ran 80 to 176 bytes
*above* the walker's `thread_entry` chain, and blamed the walker's blind spot for assembly frames.
Then 31 aarch64 runs produced 9536 twenty-nine times, 9640 once, and 10600 once, which is 1144
bytes above that chain and too much for `switch_to`'s 96 bytes to explain.

The second draft was comparing the wrong two numbers. The watermark measures whatever was on the
stack, nested traps included; the `thread_entry` chain is only the thread's own work, with no trap
on top. The comparison that means something is against the composed row: a timer interrupt landing
near the deepest point costs a 272-byte trap frame plus a handler. That is exactly the shape of a
1144-byte excursion, and the walker's own model of that is the 13792 in the table. So:

    thread_entry chain   9456    the thread's own work, no trap
    measured watermark   9536 to 10600    what actually happened, traps included
    composed worst       13792   the bound, chain + trap frame + nestable handler

The measurement sits between them, which is where it should sit, and the bound is not contradicted
by anything measured. It is still a lower bound in principle, because indirect calls and assembly
frames are invisible to the walker. The honest reason to trust it here is the size of the gap it is
being used across rather than its precision: 10600 measured against a 20480 the fault would require.

The riscv64 watermark is one run on this machine, not the 11672 in notes/stack-high-water.md's
table. That number predates milestone 124, which took the worst `spawn_on` instantiation from
4592 bytes to 1040, so it describes a kernel that no longer exists.

A fault at a slot's guard-page base needs `sp` at 20480 bytes into a 16384-byte stack. That is
6768 bytes deeper than the modeled worst case and 9880 deeper than the deepest thing 31 runs of the
suite ever measured. The walker's imprecision is measured in hundreds of bytes and the gap is
measured in thousands, which is the only reason the imprecision does not matter here.

## What the report was actually entitled to say

`stack::warn_if_guard_page` derived every line from the faulting address and then wrote "so sp
went N bytes past it", which is a claim about the stack pointer that the function never read.
The two agree only when the fault is `sp` walking off the end.

And the addresses point away from that reading. Both landed at guard-page offset 0 and 8, the
far end of the guard, 4096 and 4088 bytes below their stack's bottom. A gradual overflow arrives at
the *near* end, within a few hundred bytes of the bottom. Offset 0 is also, exactly, one word past
the top of the stack in the slot below: the slots are contiguous and each slot's guard page is its
first page, so `slot N guard base == slot N-1 stack top`. Six faults, two ISAs, every one within
eight bytes of that boundary.

The handler argues `sp` was mapped, too. On aarch64 the vector's `SAVE_CONTEXT` builds a 272-byte
frame at the live `SP_EL1` before any Rust runs; if `sp` had been inside the unmapped guard, that
store would have faulted again. It printed a full register dump instead. RISC-V's `trap.s` stays on
the interrupted `sp` for an S-mode trap and has the same property. (That argument is not airtight,
and the next subsection is why: aarch64 recovers from a nested vector fault by walking `sp` down
until the frame fits. It is refuted on other grounds.)

So the report now prints `sp` beside the faulting address in the same units and lets the reader
compare, rather than asserting the answer. `kernel/src/stack.rs`, with
`sched::tests::a_slots_guard_page_begins_where_the_slot_below_it_ends` pinning the geometry the
comparison rests on.

### A model that fits the offsets exactly, and why it is still wrong

Worth writing down because it is the reading a careful person reaches next, and because refuting it
costs an hour the second time.

The two fault offsets are the two ISAs' first trap-frame stores. aarch64's `SAVE_CONTEXT` opens
`sub sp, sp, #272` then `stp x0, x1, [sp, #16 * 0]`, a store at sp + 0. RISC-V's `trap_entry`
opens `addi sp, sp, -288` then `sd x1, 1*8(sp)`, a store at sp + 8. The faults are at guard base
+ 0 and + 8. So: if `sp` were exactly a slot's guard base at trap entry, each ISA's first
store lands precisely where its fault did.

That model even survives the double-fault objection. aarch64 has no double fault; a store fault in
`SAVE_CONTEXT` re-enters the same vector with `sp` another 272 lower. After one step `sp` is inside
the previous slot's mapped stack, so the frame builds, `exception_dispatch` runs, and `FAR_EL1`
still holds the first failing address. The register dump would be the original context's (nothing
before the store touches `x0`..`x30`), and `SPSR_EL1` would read EL1h with all of `DAIF` set, which
is exactly the `0x3c5` in the aarch64 dump.

`ELR_EL1` refutes it. Under that model the reported `ELR` is the PC of the *previous* level's
faulting instruction, which is the `stp` inside the vector table. The dump reads
`0xffff00004013a228`. Two local builds of this tree, with different metadata hashes, both place
`exception_vectors` at `0xffff0000400b4000` and agree instruction for instruction around it. So the
assembly's position is stable across builds and the faulting instruction was ordinary Rust roughly
550 KB further into `.text`. The failing CI build is a different commit and its layout is not
knowable from here, but it is main plus a 59-line markdown pull request, and that does not move half
a megabyte of code.

That paragraph is wrong, and it is the single sentence that sent this reopening after a stray
store for a day (2026-08-17). It reads a CI `ELR` against a local build's
`exception_vectors`, in a file whose own 2026-08-15 section had already measured CI's vector base at
`0xffff000040130000`, half a megabyte from the local one. Corrected: `0xffff00004013a228` is
`exception_vectors + 0x228` in that build, the tenth `stp` of the same-EL synchronous entry stub, and
the walk is exactly what happened. `ELR` does not refute the model; in all three aarch64 dumps it
confirms it, and the offsets pin `sp` to the window the arithmetic predicts. See the last section of
this file, "a kernel stack freed under its owner". The rest of this subsection is right and is the
model that turned out to be the answer.

## What it therefore is, and what is still open

Not settled. What is settled is what it is *not*: not thread-stack depth, not a frame larger
than the guard page, and not the class milestone 124 closed. What remains, in order of what the
addresses support:

- A store one or two words past the top of a kernel stack. It would come from a pointer that treats
  a stack top as inclusive rather than exclusive, or from a stale pointer into a slot whose
  `KernelStack` has been dropped and whose address range went back to `FREE_STACK_VAS`. Every
  in-tree computation from a `KernelStack` was read for this and each is exclusive at the top
  (`paint`/`high_water` over `[bottom, top)`, `spawn_into`'s closure slot and `Context`,
  `arm_for_start`, `enter_frame`'s `top - 272`, `user_pc`). So if this is the shape, the pointer is
  not one of those or it is being used after its stack died.
- A stray store through a corrupted pointer that happens to name a slot base. Six faults across
  five days landing on two addresses argues against a random one, and argues for a computation with
  a fixed input.

And the search space is small, which is the encouraging part. Only three places in the kernel can
name a slot *base* at all. They are `KernelStack::new` (which holds it as `base` and `guard`), its
`Drop` (which pushes `self.guard` back to `FREE_STACK_VAS`), and the arithmetic in `stack.rs` that
turns an address into a slot. `grep` for `STACK_AREA`, `STACK_SLOT_SPAN`, `NEXT_STACK_VA` and
`FREE_STACK_VAS` finds nothing else outside tests. Whatever stores there either derives the address
from one of those, or does not know it is a stack address at all.

The two slot numbers are themselves a clue nobody has spent. Why 87 on aarch64 and 102 on riscv64,
every time? A slot index is `(va - STACK_AREA) / 0x5000`, so a repeatable index means a repeatable
*count* of stacks handed out before the offending one. Whatever computes the faulting address is
reached at the same point in the same suite on each run. That is another way of saying this is
deterministic in the sequence of allocations and not in the timing. The intermittency then has to
come from something else deciding whether that path runs at all, not from where it lands.

It did not reproduce here, and that is a result rather than a gap in the effort. 31 full-suite
aarch64 runs under host load on a 4-CPU Linux box, `-smp 4` under TCG, every one green, plus a green
riscv64 leg. The thread watermark read 9536 twenty-nine times, 9640 once, and 10600 once, which is
worth recording on its own. The depth is a near-constant with occasional excursions of the size a
nested trap frame explains. It is nothing like a stack that intermittently runs 4096 bytes past its
own bottom. The reproduction cost is the honest blocker: CI hit it perhaps two runs in six, on a
runner this machine does not resemble. A hunt harness that restores the fixture images and stands in
for xtask's two host actors got the cycle to about 32 seconds, and 32 seconds times zero failures is
still zero information about the cause.

The next occurrence should be legible without another investigation, which is what the `sp` line
buys. If it prints a slot *below* the faulting address, the store-past-the-top reading is confirmed
and the search narrows to pointers derived from a stack top. If it prints the same slot, the depth
reading comes back and the walker's bound is wrong somewhere, which would mean an indirect call it
cannot see.

## What this file's numbers cite

Glossed here because the split made this a file of its own, and `script/citations` asks each
file to say once what a number cites. Each gloss is the record's own title.

- milestone 84 (Stack high-water: measure kernel stack depth)
- milestone 108 (The drivers move onto frame capabilities)
- milestone 124 (A thread is born where it lives: the spawn path's copies)
