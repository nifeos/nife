# A thread's vector registers are its own: the appendices to milestone 447

[Milestone 447 (a thread's vector registers are its own)](../0447-vector-registers-are-a-thread-s-own.md)
carries the rule and its four cases, why the save is eager, and the three architectures'
mechanisms. It also carries what it found, the cost in headline figures, the proof, the `BUGS` and
the follow-on. This directory carries the working its prose budget has no room for.

- [`riscv.md`](riscv.md): why the RISC-V first-use handler retries rather than decodes, why it
  sets `sstatus` in the trap frame, and how the boot path that skips `arch::init` was found.
- [`costs.md`](costs.md): the fastpath-footprint, benchmark and icount tables, the TCB-page memory
  argument, and the stack-frame gate's numbers for the inline shape.
- [`proof.md`](proof.md): what each of the five tests catches, the falsification, and why the
  tests are kernel threads rather than user programs.
- [`target-flip.md`](target-flip.md): what flipping the targets to hard float would take, buy and
  cost, as priced on 2026-09-20. The flip itself is milestone 534 (the soft-float targets could now
  be flipped).


*Name: provisional. The directory follows §212 (a prose budget)'s siting default. calef names
directories.*

- `riscv` Name: provisional. It holds the parent's two RISC-V subsections and "What it found". calef
  names files.
- `costs` Name: provisional. It holds the parent's "What it costs, measured". calef names files.
- `proof` Name: provisional. It holds the parent's "The proof, and it fails against a kernel without
  this". calef names files.
- `target-flip` Name: provisional. It holds the parent's "The proposal". calef names files.
