# The number that decides it: proof effort

An appendix to [`notes/verus.md`](../verus.md). It holds the proof-effort figures from both
Atmosphere papers, their comparison table and its inconsistency, the OOPSLA examples, seL4's cost,
Atmosphere's syscall figures second-hand, and the extrapolation to this kernel.

All figures are quoted, with the paper each came from. The two papers disagree, because the second
revises the first.

| | KISV '23 | SOSP '25 |
|---|---|---|
| executable code | not stated | **6K lines** |
| proof code | not stated | **20.1K lines** (14.3K specification + 5.8K hints), of which 2.9K spec is the top-level abstract system-call specification |
| proof-to-code ratio | **7.5:1** | **3.32:1** |
| verification wall clock | *"Verus takes roughly 30 minutes to finish the proof and spends up to 850 seconds at most on a function"* (runtime checks off) | *"completes verification in less than 20 seconds on a modern laptop"* |
| total effort | *"less than 2.5 person-years, with only 1.5 years spent on verification"* | *"less than one and a half physical years and an effort of roughly two and a half person-years. But only one and a half person-years was spent on the development of the verified parts (we used the second person-year on unverified parts such as the boot and initialization infrastructure, user-level device drivers, application benchmarks, and the build environment)"* |

The 30-minutes-to-20-seconds swing over two years is the single most encouraging number in either
paper, and SOSP '25 draws the conclusion itself: *"it takes less time to finish verification than
compiling the kernel, which enables a truly interactive development cycle with a verifier as opposed
to traditional recompile, reboot, and run tests approach."*

Their own comparison table (KISV '23, Figure 2), for context on what that 3.32:1 is beating:

| system | language | spec language | proof-to-code |
|---|---|---|---|
| seL4 | C+Asm | Isabelle/HOL | 20:1 |
| CertiKOS | C+Asm | Coq | 14.9:1 |
| SeKVM | C+Asm | Coq | 6.9:1 |
| Ironclad | Dafny | Dafny | 4.8:1 |
| NrOS | Rust | Verus (Rust eDSL) | 10:1 |
| Atmosphere | Rust | Verus (Rust eDSL) | 7.5:1 (3.32:1 by SOSP '25) |

That table disagrees with its own paper's prose, which says Atmosphere's 7.5:1 beats *"SeL4 and
CertiKOS, which have proof-to-code ratio of 19:1 and 20:1, respectively"*, while the table gives 20:1
and 14.9:1. Both appear in KISV '23 and it does not reconcile them. Flagged rather than resolved.

A second Verus data point, at a much smaller scale. The OOPSLA paper's Table 1 gives per-example
spec, proof and exec lines with wall-clock verification times:

| example | spec + proof | exec | ratio | seconds |
|---|---|---|---|---|
| a verified `RwLock` | 200 + 80 | 145 | 1.9:1 | 4.44 |
| an XOR doubly-linked list | 116 + 118 | 151 | 1.6:1 | 5.03 |
| a FIFO queue | 220 + 119 | 138 | 2.5:1 | 4.58 |
 Those are
library-sized rather than kernel-sized, and the ratios are *better* than Atmosphere's, which is what
you would expect from a smaller unit with a simpler specification. They are the closest thing here to
a price for one subsystem.

And seL4's own cost, as SOSP '25 states it: *"development of the first formally verified microkernel,
seL4, required 11 person-years (an additional 9 person-years were needed for the development of
formal language frameworks, proof tools, and libraries)"*. KISV '23 puts the same thing as *"200,000
lines of proof code in the Isabelle/HOL theorem prover for 8,700 lines of C and required 22
person-years"*.

## Performance, second-hand

This lane could not read the evaluation section. `notes/redleaf/crossing-costs.md` (branch
`maintainer/redleaf-comparison`, 2026-09-20) reports SOSP '25 Table 3 from the full text, on CloudLab
c220g5 (two Intel Xeon Silver 4114 10-core at 2.20 GHz): call/reply 1,058 cycles for Atmosphere
against seL4's 1,026, and map-a-page 1,984 against 2,650. Treated here as second-hand and
attributed; that note is the record for it.

## What it would cost this kernel

The cost, in this project's units. Atmosphere verified 6K executable lines in 1.5
person-years at 3.32 lines of proof per line of code. `kernel/src` is 40,953 code lines,
which is 6.8x Atmosphere's kernel. A linear extrapolation gives roughly 136,000 lines of proof
and ten person-years, and linear is the optimistic reading for three reasons the papers state
plainly:

- Atmosphere was designed for its proof and nife was not. Flat permission storage (*"push it to
  the extreme arguing that it is a key design choice critical for the scalability of the proof"*),
  manual memory management chosen so that memory is reasoned about explicitly, and a big lock:
  *"Atmosphere is a multiprocessor system, but to simplify verification we rely on a big-lock
  synchronization, i.e., all interrupts and system calls execute in the microkernel under one global
  lock and with further interrupts disabled."* This tree runs SMP with fine-grained locking and
  DECISIONS §4 (kernel shape: monolithic, deferred, with two cheap rules) rule 4, assume weak memory
  ordering, as a tenet. Atmosphere's design bought its ratio.
- One architecture. The SOSP '25 TCB list is x86_64 throughout. nife has three, and DECISIONS
  §19 (architectural parity is a tenet; the targets are aarch64, riscv64, and x86_64) makes that a
  gate rather than a choice. Verus
  would have the same one-architecture problem Kani has, for the same `cfg` reason.
- Concurrency is named as future work by the paper itself: *"I/O intensive shared device drivers
  will require proof about concurrent code, which we leave as future work."* Five of the eighteen
  defects in `notes/proof-retrospective.md`'s corpus are concurrency. And Verus's concurrency model
  is sequentially consistent, which [`what-verus-is.md`](what-verus-is.md) argues is the assumption DECISIONS §4's rule 4
  exists to keep this project from acquiring. So the defect class the prover misses most is the class
  where adopting Verus would buy the least and cost the most in false confidence.
