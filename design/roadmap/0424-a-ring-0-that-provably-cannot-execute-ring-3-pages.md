---
status: NOT-STARTED
raised: 2026-09-17
promoted_from: a-ring-0-that-provably-cannot-execute-ring-3-pages
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 424. A ring 0 that provably cannot execute ring-3 pages, and SMAP with a number

Promoted from the proposal
`a-ring-0-that-provably-cannot-execute-ring-3-pages`, filed 2026-09-17 from milestone 313's security
audit (findings 3 and 6). *(Number provisional until the merge queue lands it.)*

Nothing has to land first; the SMAP half owes a system-call-path measurement before it
ships, and that measurement is part of the work rather than a precondition on starting it.

**Premise re-checked 2026-09-19 and still true, both halves.** `CR4.SMEP` is set and says so at boot
(`kernel/src/arch/x86_64/mod.rs`). There is still no test anywhere that makes ring 0 fetch from a
user page. `CR4.SMAP` and `PSTATE.PAN` are both still off. Both `x86_64/mmu.rs` and
`aarch64/mmu.rs` still say so at
`permit_kernel_access_to_user_pages`, which is therefore still the function that permits nothing.
The kernel still has no fault-recovery path for its own faults. Nothing in `kernel/` matches an
expected-fault slot, which is the expensive half this block prices.

## What is being proposed

Two halves, one lane, because they need the same new thing.

1. A kernel test that ring 0 cannot fetch from a user page, on all three architectures, with a
   falsification record each. On aarch64 the defect is a user page without `PXN`; on riscv64 there
   is no defect to write (the hardware refuses unconditionally, and the test documents that); on
   `x86_64` the defect is not setting `CR4.SMEP`.
2. **`CR4.SMAP` on `x86_64` and `PSTATE.PAN` on aarch64**, so a kernel bug that strays into the low
   half faults instead of succeeding quietly, with the system-call-path cost measured first, which is
   what `mmu::permit_kernel_access_to_user_pages`'s `BUGS` has asked for since it was written.

## Why

Milestone 313's audit found the claim "the kernel cannot execute a confined component's code"
stated nowhere, true on aarch64 and riscv64 by the encoders and the hardware, and false on `x86_64`
because nothing set SMEP. The bit is set now and a boot line says so, and that is rung three: a
setting with a transcript, not a claim with a test. A test would need ring 0 to take a page fault
on purpose and recover, and this kernel has no fault-recovery path for its own faults. That is also
the thing a SMAP test needs. Building that path once serves both.

## What it costs, honestly

The recovery path is the expensive half and it is a design question. A per-CPU "expected fault"
slot the trap handler consults before it panics is the small shape, and it has to be written so
that nothing but a test can arm it. SMAP itself is `stac`/`clac` around every kernel access to
user memory, of which there are few by design (the shared-page contracts carry no lengths and the
system call surface moves words in registers). So the number may be small, but it is a number this tree
asks for before turning a protection on.

## Index row

Milestone 313's audit found the claim "the kernel cannot execute a confined component's code"
stated nowhere, true on aarch64 and riscv64 by the encoders and the hardware, and false on x86_64
because nothing set SMEP. The bit is set now and a boot line says so. That is a setting with a
transcript rather than a claim with a test. Two halves go in one lane because they need the same new
thing. A kernel test on all three architectures that ring 0 cannot fetch from a user page, each with
a falsification record. Each with the defect written where there is one (a user page without
`PXN` on aarch64, nothing on riscv64 where the hardware refuses unconditionally, an unset
`CR4.SMEP` on x86_64). And `CR4.SMAP` and `PSTATE.PAN`, so a kernel bug that strays into the
low half faults instead of succeeding quietly. The expensive half is the fault-recovery path both
need. A per-CPU expected-fault slot the trap handler consults before it panics, written so that
nothing but a test can arm it. SMAP owes its system-call-path cost as a number before it ships. That is
what `permit_kernel_access_to_user_pages`'s `BUGS` has asked for since it was written.
