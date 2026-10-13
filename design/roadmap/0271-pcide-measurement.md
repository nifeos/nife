---
status: NOT-STARTED
raised: 2026-09-10
milestone_dependencies: none
decision_dependencies: none
machine_requirements: x86_64 silicon with PCID
specific_machine: none
needs_person: yes
---
# 271. Measure `CR4.PCIDE`, and settle the ASID-tagging skip it leaves on x86_64

Outstanding in milestone 161 (the x86_64 kernel port), and referenced but not answered by
milestone 186 (derive the architecture list), whose only claim on it is that the bench-tooling
caller for the x86 baseline is one of its eleven silent gaps.

In the second sense `notes/roadmap.md` names: xenon is here and this
needs an evening at it. PCID's whole benefit is a TLB that survives a `mov cr3`, and the only
instrument this tree points at x86_64 counts instructions. QEMU's softmmu TLB is not tagged, so it
charges for the added gate and credits nothing for the removed flush, which is not a prediction: it
is what milestone 335 (the `ctx_switch` number on real RISC-V silicon) already measured on the
identical experiment one ISA over, where icount came back +1.2% and the block records it as
"the measurement reading backwards". The same arithmetic makes this the x86_64 twin of that block
under DECISIONS §19, and it carries the same gate for the same reason.

The token was `DECISION` from 2026-09-10 to 2026-09-19, and it is corrected here rather than
quietly because it confused this milestone's output with its gate. Whether to turn `CR4.PCIDE`
on is genuinely calef's, and it is what step 3 below exists to bring him; a gate says what stops a
lane from starting, and nothing calef could say today would let this start any sooner. That is
milestone 421's correction read in the other direction: 421 carried `NONE` while owing a decision,
and this carried `DECISION` while owing a measurement. No `design/decisions/` section is minted,
because the section is the deliverable and writing it before the number exists would be the
asserting this block was filed to stop (per the seven-questions rule, *"what does each option cost,
measured rather than asserted"*).

## What is skipping, and why it is not a bug

`kernel/src/user/tests.rs:508` skips its ASID-tagging assertion on x86_64, because the kernel runs
with `CR4.PCIDE` clear: PCID is `CR3[11:0]`, and with `PCIDE` clear those bits are reserved-zero,
so `arch::x86_64::mmu::ttbr0_value` drops the tag and every `mov cr3` flushes the whole TLB. The test
would pass, but for the wrong reason (nothing was cached, not because tagging works), so it skips
rather than gives a false positive. `kernel/src/arch/x86_64/mmu.rs`'s own `BUGS` records the same
gap: both `PCIDE` and `CR4.PGE` are off with nothing measured against them.

This is a real architectural divergence under §19, not a scope note: aarch64 and riscv64 both tag
address spaces on every context switch today, and x86_64 does not.

## What this needs

1. Turn `CR4.PCIDE` on (and, since `script/icount` already only takes the other two
   architectures per the same `BUGS` entry, `CR4.PGE` alongside it if the two are entangled) in a
   throwaway or feature-gated build.
2. **Measure the cost**, the same way milestone 134's register measures everything else: a context-
   switch benchmark with and without PCID tagging, on the same machine, same methodology as E1.
   `script/icount` should take a third leg if the instrument allows it.
3. **Bring the number to calef** as a DECISIONS section, priced rather than argued, with the recorded
   ASID test as the thing that flips from skip to pass if the answer is yes.

## BUGS

- Nothing here prices the risk side. PCID reuse across processes needs INVPCID or careful
  reuse-of-tag bookkeeping; this block does not yet name what breaks if tags are recycled wrong,
  which the measurement work should surface rather than assume away.

## Follow-on

- **Milestone 186.** Once this lands, 186's own list of eleven gaps loses the "bench tooling item 3
  has no caller" entry for the x86 leg, since the measurement this milestone produces is that caller.

## Index row

calef, 2026-09-10. Outstanding since milestone 161, checked 2026-09-03, never measured.
