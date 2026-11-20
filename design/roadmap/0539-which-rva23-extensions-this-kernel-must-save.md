---
status: NOT-STARTED
raised: 2026-09-20
promoted_from: which-rva23-extensions-this-kernel-must-save
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 539. Which RVA23 extensions this kernel must save, and which it only has to admit exist

*(Number provisional until the merge queue lands it.)* Promoted from the proposal `which-rva23-extensions-this-kernel-must-save`, filed 2026-09-20, on calef's instruction of 2026-09-20 to give every proposal on `main` a number. The text below is the proposal's own, unedited except for this paragraph: the argument is its author's and promotion is not the moment to improve it. Filed by `maintainer/riscv-summit-research` from the RISC-V Summit
Europe 2026 reading (`notes/riscv-summit-2026.md`). *(Number and slug provisional until the merge
queue lands it.)*

The profile is ratified, QEMU can be told to be an RVA23 machine, and the audit is a
reading of `kernel/src/arch/riscv64/context.rs` against a published list. Nothing waits on calef, on
hardware, or on another milestone.

## The idea that makes this cheap, and it is somebody else's

Guodong Xu and Charlie Jenkins, *RVA23 Profile Support in Linux Kernel: From Extension Definitions
to Userspace Export* (https://riscv-europe.org/summit/2026/presentations#P-B7EASJ, abstract only, no
slides read). Their classification, quoted:

> a data-backed classification by whether an extension adds architectural state the OS must save and
> restore [...] showing roughly two-thirds are stateless and only need to be discoverable, not
> implemented.

*(The elision replaces a dash, not any words: this tree's lint forbids the character and the
sentence is otherwise verbatim.)*

That is the sentence worth stealing. It converts "support RVA23", which sounds like a year, into two
much smaller questions: **what state must a context switch carry**, and **what must a program be
able to ask about**. Their own numbers are for Linux and are quoted as claims, not adopted:
kernel coverage *"stuck near 68% for a year"* until *"Linux v7.0 reached 100%"*.

RVA23 itself is ratified, checked away from the talk: announced 2024-10-22
(https://riscv.org/blog/risc-v-announces-ratification-of-the-rva23-profile-standard/), specification
at https://docs.riscv.org/reference/rva23/v1.0/index.html.

## Why this tree needs it

**This kernel has never asked the question.** `kernel/src/arch/riscv64/isa.rs` probes what firmware
offers and prints it, and the context switch saves what it was written to save. Nowhere does the
tree state which RVA23 mandatory extensions carry architectural state, which of those this kernel
preserves across a switch, and which it silently does not. That is exactly the shape
DECISIONS §19 (architectural parity is a tenet) calls the bug. *A feature works on one ISA and silently not
another*, except here it is a feature that works on one core and silently not after a
preemption.

It is also the near half of the Server Platform work. Server Platform 1.0's content slide mandates
RVA23S64, so the profile is not optional on the machine the other proposal in this directory is
about (`a-riscv64-host-that-hands-us-acpi`).

And there is a live consumer. Fatal risk 1's experiment ran unmodified `ripgrep` on all three
architectures; the next stranger's binary may be built for RVA23 by a distribution that, as Jon
Taylor put it, *"moved to requiring RVA23 with the release of Ubuntu 25.10"*
(https://riscv-europe.org/summit/2026/presentations#P-BTUW3M). A program compiled for the profile
that runs correctly until it is preempted is the worst failure shape available.

## What the work is

1. Take the mandatory list from the ratified profile (not from the talk, not from memory) and
   sort each entry into *adds architectural state* or *stateless, discoverable only*.
2. Audit the riscv64 context switch against the first pile, and write the answer down beside the
   code. Where something is not saved, that is a `BUGS` entry naming the extension, in the FreeBSD
   posture this tree already uses.
3. Decide what discovery looks like here, and this is the part that may hand a fork back. Linux
   answers through `hwprobe` and `/proc/cpuinfo`; nife has neither, and a capability system arguably
   should answer through something a program is *granted* rather than something it reads. That is a
   system-call-surface question if it grows one, so the milestone should stop and write a proposal
   rather than invent an interface.
4. Run the tour under `qemu-system-riscv64` configured for the profile and record the
   transcript, so the claim is a run rather than a reading.

## The second finding this should carry, because it has no other home

Asanović's State of the Union introduced optimization guidance options, a category that did not
exist before: slides 6 and 7, `Oilsm` and `Ovlt`, described as guidance that *"Software should
assume"* something about performance rather than functionality, because *"Current ISA strings encode
functionality, not performance"*, and *"Intended to be mandatory for future RVA profiles, but will
first appear as development options"* (https://riscv-europe.org/summit/2026/presentations#P-N9KRDZ).

That is a CLAIM about an unratified future profile and nothing should be built for it. It
belongs in this milestone's block as one paragraph, because the audit above is the place a reader
will next ask "and what about the O-options", and the answer should be waiting there rather than
rediscovered.

## Index row

Guodong Xu and Charlie Jenkins, *RVA23 Profile Support in Linux Kernel: From Extension Definitions
to Userspace Export* (https://riscv-europe.org/summit/2026/presentations#P-B7EASJ, abstract only, no
slides read).
