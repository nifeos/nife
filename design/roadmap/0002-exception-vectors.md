---
status: BUILT
raised: 2026-07-13
built: 2026-07-13
---
# 2. Exception vectors, and a fault that tells you what it was

Built in `543d390` (2026-07-13), the first commit to carry a milestone title.

Before it, a bad memory access killed the machine in silence. After it, a fault prints the
exception class, the faulting address, the instruction that did it, and the whole register file,
then stops. The commit records the reason to do it carefully: on aarch64 a fault, an interrupt,
and a syscall are the same mechanism, so this is also the plumbing for milestone 5 (the timer)
and milestone 7 (`svc`), and those became "mostly a matter of adding arms to a match."

The test the commit singles out is `registers_survive_an_exception`: the TrapFrame layout is a
contract with assembly the compiler cannot check, and a wrong offset would scramble a register
while returning happily to the right address. It also verified the harness can actually fail, by
injecting a bad assertion and confirming a nonzero exit.

Where plan and outcome differ, which is the reason the backfill records outcomes: the original
row (`b7f10e7`) promised an "EL2 -> EL1 drop" alongside the vectors. No commit contains one, and
the revised table in `491f23d` quietly dropped it: QEMU's `virt` machine enters a flat Image at
EL1, so there was nothing to drop from. The boot-protocol work that day was `d87d24a`, booting as
a flat arm64 Image and getting the device tree.

## Follow-on

- **Refused.** The EL2 to EL1 drop the original row promised alongside the vectors. QEMU's `virt`
  machine enters a flat Image at EL1, so there was nothing to drop from, and building the mechanism
  with no caller would have been guessing at what a later board needs.
  This refusal is milestone 449 (design/roadmap/0449-the-el2-to-el1-drop.md), which carries it with
  the condition that would change it.

## Index row

faults, interrupts, and syscalls are one mechanism; this is the plumbing for all three
