---
experiment_status: RUN
experiment_run: 2026-09-17
---
# 9. The HAL is a fiction, and an architecture costs a restructure rather than a port, and so does the next machine

*Risk 9 of [the nine](README.md). The status vocabulary, the rule an entry meets and the running order are there.*

The claim, calef's, 2026-08-30: *"another proof/disproof of the nife thesis is actual functioning on
the three silicons. If we can't get it to run on one, that would also likely kill the effort."*

Sharpened, because the ISA count is not the fatal part. What would be fatal is what a failure would
reveal: that adding an architecture requires changing the kernel rather than adding a directory under
`arch/`. That is what DECISIONS §4 (kernel shape)'s rule 1 and 
§19 (architectural parity is a tenet) claim it does not.

Widened 2026-09-23 from architectures to machines, and the ruling is calef's: *"A nife that runs on
one cloud platform but not another is also its own form of risk."* So it reads at two grains now, an
architecture and an implementation, a particular machine of one. Both words are provisional. The
implementation grain is the earlier warning, and the only one that can be bought.

GREEN.
Milestone 87 (the x86_64 bare-metal machine) reached `nife self-test: 5 of 5 passed` on xenon's own
firmware, so nife runs on bare metal on two of its three declared architectures. aarch64 has run
only under HVF on patagonia, which is virtualization (`notes/bench-runbook.md`). Everything it needed
lives under `kernel/src/arch/x86_64/`, and its one defect was fixed inside `arch/x86_64/mmu.rs`. The
cost was measured rather than merely passed: 42 compiler errors, every one "this `arch::` name does
not exist yet", with `crates/paging` unchanged.
[`notes/x86-port.md`](../../notes/x86-port.md): *"That is the whole diff above `arch/`. A new ISA was a
new directory."*

**The experiment for the widened grain, which has not been run:** a second machine of an architecture
nife already boots, riding on milestone 225 (run the soak on radon, argon and xenon). Ruled
2026-10-06 (calef): *"We can do the Scaleway first."* Milestone 89 (Scaleway EM-RV1) runs first,
a second riscv64 beside radon, and argon (the TX1) follows when it arrives. No difference is a
result too. History: on 2026-10-05 calef ruled *"Both, argon first"*, because argon closes the
larger gap. On 2026-10-06 argon turned out not to be in hand: the seller shipped a TK1 against the
TX1 order, it is going back, and the TX1 has no date (`notes/bench-runbook.md`).

Three caveats. The verdict is one machine per architecture, and for aarch64 not even that, since
argon has never booted nife. So those 42 errors price a third *architecture* and say nothing about a
second *machine*. The failure this grain fears is silence. The appendix has a worked instance, closed
on 2026-09-23 by milestone 186 (derive the architecture list, and close what it does not reach): five
functions that compiled, shipped and did nothing on the architecture nobody had run them on. And
parity multiplies every other risk here. If the project ever needs to buy time, dropping to two
architectures is the largest single lever available, and it should be a decision rather than a drift.
Correction, 2026-10-03: the closure above was not complete on 2026-09-23. Milestone 186 finished that
day's open item, so `script/stack-depth-check` now gates x86_64 from a list it reads out of the
toolchain pin, and found no offender. It does not change the verdict: the risk is about restructure
cost and a third architecture's gates, and the silent-gap class stays open wherever a new gate spells
its own list.
[Appendix](the-hal-and-the-next-machine.md).
