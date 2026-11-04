---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 348. The x86_64 kernel is linked at one physical address, so firmware picks the address and we hope

Premise re-checked 2026-09-19 and it holds. `kernel/link-x86_64.ld` still sets `PHYS_START = 0x02000000` (32 MiB) and still carries
the paragraph headed "PHYS_START WAS 1 MiB AND IS NOW 32 MiB, and the reason is firmware rather than
specification"; the image is still linked at one physical address, and `.boot` still carries the
absolute self-references that have to become position-independent first.

A lane can start today. OVMF under QEMU reproduces the whole problem, which is how
milestone 195 found it, and nothing here waits on the OptiPlex.

**In brief.** `PHYS_START` for the x86_64 kernel is 32 MiB rather than 1 MiB so the image clears
the low memory OVMF reserves for itself. That buys a larger gap between us and the firmware; it does
not make the image safe at any other address. Making it physically relocatable means the loader can
place it wherever the memory map actually has room. The blocker is `.boot`: the 32-bit
trampoline's self-references are absolute, and a 32-bit instruction stream cannot name a 64-bit
address. So those references have to become position-independent before the link address can stop
mattering.

## Why this matters

The claim the current arrangement rests on is that 32 MiB is free on every machine that boots this
kernel, and nothing verifies it. Milestone 195's own `BUGS` says so: none of the UEFI path is proved
on a Dell, OVMF is not a vendor firmware, and whether the OptiPlex leaves 32 MiB free is a question
for the bench. So the failure mode is a machine-specific boot failure discovered by a person
standing at xenon, at the point in the loop where debugging costs the most. A relocatable image
turns "does this firmware happen to leave our address free" into "the loader read the memory map",
which is a question the code can answer on any machine.

It also removes a number that will be tuned again. 1 MiB became 32 MiB once already, in response to
one firmware. A third firmware with different reservations produces a third constant, and each one
is only known to be wrong after a boot fails.

## Index row

`PHYS_START` for the x86_64 kernel is 32 MiB rather than 1 MiB so the image clears the low memory
OVMF reserves. That buys a larger gap from the firmware and does not make the image safe at any
other address. Nothing verifies that 32 MiB is free on every machine that boots this kernel, and
milestone 195 (finish the UEFI boot path: the suite)'s `BUGS` says none of the UEFI path is proved on a Dell. The failure mode is a
machine-specific boot failure found by a person at xenon, where debugging costs the most. A
relocatable image turns "does this firmware happen to leave our address free" into "the loader read
the memory map", and it ends the tuning of a constant per firmware. The blocker is `.boot`, whose
32-bit trampoline cannot name a 64-bit address. OVMF under QEMU reproduces the whole problem, so
nothing waits on the OptiPlex.
