---
status: NOT-STARTED
raised: 2026-09-19
promoted_from: colour-the-kernel-stacks-and-take-e1-again
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 506. Color the kernel stacks and take E1 again, to find out what E1's knee is made of

*(Number provisional until the merge queue lands it.)* Promoted from the
proposal `colour-the-kernel-stacks-and-take-e1-again`, filed 2026-09-19, on calef's instruction of
2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own, unedited
except for this paragraph: the argument is its author's and promotion is not the moment to improve
it. Written by milestone 134 (the register of measures)'s per-IPC stack-depth lane, from the measurement that retired E1's
estimated input (notes/stack-high-water.md, "Per-IPC depth").

The build is a feature on patagonia; the reading needs radon, which is booked by
milestone 134's and 168's evenings anyway (notes/footprint-perturbation.md, "The next radon
evening"), so this can ride on one of them if it is built first.

In brief. E1 found IPC latency on radon rising 68% between 2 and 16 threads, bending between 8
and 16, then flat to 96. Its prediction was a capacity story: each thread's kernel stack is a
different set of lines, and enough of them overflow a 32 KB L1d. The per-IPC depth is now measured,
and at **about 600 bytes per thread per round trip** in the release kernel, stacks alone would not
overflow radon's L1d until about 54 threads. Capacity does not explain the knee.

A set conflict does, on arithmetic, and so does a competitor. Every kernel stack's top is a page
boundary, so every thread's hot stack lines sit at the same page offsets. radon's U74 L1 D-cache is
32 KiB, 4-way, virtually indexed, 64-byte lines (SiFive U74-MC Core Complex Manual 21G3.02.00, read
2026-09-19): 8 KiB ways, set index from VA bits 6 to 12. Stack slots are seven pages apart, so bit 12
of a stack's top alternates with the slot, giving two colors of four ways: at most eight threads'
hot stack lines can be resident together, which puts a knee at 8. But every thread's TCB is also on
its own page, so hot TCB fields alias the same way and predict the same knee.

**The experiment separates them.** A feature build that starts each thread's stack a per-slot color
below its top (slot index times about 640 bytes, modulo a page, so ten-line windows stop sharing
sets). Then E1 on a `board,bench,single_hart` card, interleaved against an uncolored build like E3.

| E1 with coloured stacks | reading | what it does to §96 |
|---|---|---|
| knee moves well right of 16 | the stacks caused it | a process kernel buys the effect back with coloring, so this axis stops arguing for an event kernel |
| knee stays at 8 to 16 | the TCBs, or another page-aligned per-thread object, caused it | an event kernel's single stack would not remove it either; color the TCBs next |
| both builds move together | neither; something shared, like the rendezvous objects | the mechanism is still open, and M7's event counters are the next instrument |

What it costs. One feature, one constant, one line where a thread's initial `sp` is computed,
and a color's worth of each stack (up to about 4 KiB of a 24 KiB stack, which `script/stack-depth-check`
must still pass). No system call surface, no dependency, no default-build change.

**What it does not do.** It is not a proposal to color stacks by default. That is a decision to make
after the reading, with the reading.

## Index row

E1 found IPC latency on radon rising 68% between 2 and 16 threads, bending between 8 and 16, then
flat to 96.
