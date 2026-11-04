---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: silicon with a tagged TLB
specific_machine: none
needs_person: yes
---
# 363. ASIDs ship with their payoff asserted rather than measured

Checked on 2026-09-19: nothing in `bench/`
or in notes/address-space-identifiers.md carries a switch-cost number with tagging on against
tagging off, so the payoff is still asserted rather than measured. **The board list in the gate below
has narrowed to one, and the narrowing was measured.** radon's own boot tour
reports `satp.ASID 0 bits measured` (`bench/radon-2026-09-16/bench-134300.log`), which is precisely
the case this file warned would measure no change at all. Xenon cannot show it either:
`kernel/src/arch/x86_64/mmu.rs` records that `CR4.PCIDE` is clear, so an x86_64 address space has
nowhere to put its tag and `ttbr0_value` drops it. argon is the one machine of the three that can
answer this.

Of the second kind: the machines exist. QEMU cannot show this, because it does
not model TLB refill cost. A number from patagonia would be noise wearing a decimal point. It
needs argon, radon or xenon, and it needs somebody at one of them to flash an image and read a
serial console. Waiting does not discharge it.

**In brief.** Address space identifiers landed in July and the context switch stopped flushing the
TLB. No machine has measured what that changed. Put a number on it: the address-space-switch cost
with tagging on and with it off, on real silicon, on at least one of the three boards. Milestone
15's own block calls ASIDs the prerequisite for reasoning about switch cost, and that reasoning has
never had an input.

## Why this matters

The mechanism is in the kernel on every architecture and its justification is an argument. That is
the shape this project explicitly refuses: measure, do not argue. A benchmark this system publishes
about context switching is currently priced against a TLB behavior nobody has characterized. The
honest thing to say about ASIDs today is that they are standard practice, which is a reason to
implement them and not evidence that they helped here.

There is a specific reason the number could be surprising rather than confirmatory. That is why
this is worth a board's time. This kernel bounds concurrent address spaces at 160 and milestone 15
refused the generation-and-rollover scheme because the exhaustion path is unreachable below 256
tags. A system that never recycles a tag has a different TLB pressure profile from Linux, which
recycles constantly, and the win could be larger than the textbook figure or smaller. Either
outcome is a result. Neither is available from QEMU.

It also feeds work that is already tracked. Address-space-switch cost is an input to scheduling and
to IPC pricing, and every comparison this project makes against Linux, macOS or seL4 on those
primitives currently omits the term.

## What a measurement means

Two facts from milestone 15's block bound what a measurement means. The RISC-V half arrived separately in
milestone 58, because `sfence.vma` is local and discharging a tag machine-wide needs an IPI to
every hart through SBI RFENCE, so radon's number is about a different mechanism from argon's.
And `notes/address-space-identifiers.md` records that RISC-V permits `satp.ASID` to be zero bits wide, so a RISC-V
machine that cannot tell tags apart keeps flushing on every switch and would measure no change at
all. Whichever board is used, the report has to say which of these it was.

## Index row

Address space identifiers landed in July, the context switch stopped flushing the TLB, and no
machine has measured what that changed. QEMU cannot show it, since it does not model TLB refill
cost, so the number needs silicon and somebody at a serial console. The gate has narrowed: radon
measures `satp.ASID` at zero bits and keeps flushing on every switch, and x86_64 runs with
`CR4.PCIDE` clear and has nowhere to put a tag. That leaves argon as the only board that can
answer. The number could surprise rather than confirm. This kernel bounds concurrent address spaces
at 160 and never recycles a tag, so its TLB pressure is not Linux's, and every comparison against
Linux, macOS or seL4 on switch cost currently omits the term.
