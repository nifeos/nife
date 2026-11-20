# What Warton ran AIM7 on, and why the 20% is not a target

An appendix to [notes/job-mix.md](../job-mix.md), which quotes the retrospective's sentence and
summarizes this page. It holds the provenance of the 20% that sentence cites, read from the
sources on 2026-09-04 and 2026-09-13.

## Two things about the citation that are worth knowing before quoting it

The 20% is the retrospective's summary of Warton, not Warton's own claim. His thesis abstract
(read the same day from
<https://trustworthy.systems/publications/theses_public/05/Warton:be.abstract>) says only that
"significant memory savings can be achieved without degrading the kernels performance" and that
"preliminary results show improvement in the kernels performance due to the single stack
architecure, however more experiments are required to verify this result." Both are real citations; they are not the same strength of
claim, and this tree has carried a fabricated block quote for twelve days, so the provenance
is written down rather than remembered.

Since 2026-09-13 the thesis body has been read too, and it is harder on the number than the
abstract is. Warton expected the opposite result. He called his own 20% something to treat "with
skepticism until it can be satisfactorily explained", and never ran the cache simulation that would
have explained it. The sections below have the quotations.

The result is from 2005, on ARMv5, on Pistachio. Nothing about it transfers to this kernel by
assumption. That is the whole reason this milestone exists rather than a rewrite.

Checked on 2026-09-13, because AGENTS.md's fourth question is *is the premise true?* and nobody
had asked what the 20% was measured on.

## The retrospective does not say, and that is worth knowing before quoting it

Read from <https://trustworthy.systems/publications/nicta_full_text/8988.pdf> (629,888 bytes, 30
pages). "AIM7" occurs exactly once in the paper, in the sentence this page quotes above. The
whole setup it gives is section 4.1, page 1:16: *"the Pistachio process kernel vs an event-based
(single-stack) kernel with continuations on an ARMv5 processor."* Two arms and an ISA. The
software above the kernel is not named, and "Wombat" appears in the paper only as a bibliography
entry cited from section 5.1 (Virtualisation), unconnected to Warton's measurement.

## Warton's own thesis says, and the answer is Wombat

<https://trustworthy.systems/publications/theses_public/05/Warton%3Abe.pdf>, read the same day
(361,145 bytes, 43 pages). Matthew Warton, *Single Kernel Stack L4*, BE thesis, UNSW, submitted
2 November 2005, supervisor Gernot Heiser. Section 5.4:

> The AIM7 benchmark is a measures system performance by simulating workload on a multiuser system.
> The AIM7 benchmark was modified slightly so that it could run on Wombat. The modifications included
> disabling the network operation simulations because Wombat does not support the GetHost function,
> and disabling the file system operation simulations, because Wombat runs from a ram disk, and the
> ram disk is not large enough to support the benchmarks.

(`pypdf` drops this PDF's `fi` ligature, so "modified", "file", "benefits" and "conflict" arrive
from the extractor with the `fi` missing. They are restored in every quotation on this page, since
the ligature is an artifact of reading the document rather than of writing it. Nothing else in any
quotation here is altered.)

Wombat is the paravirtualised ARM Linux, the same one the retrospective cites at Leslie, van
Schaik and Heiser 2005. So the 20% is a delta between two microkernels measured through a hosted
Linux's system call path. Four things follow, and each one costs this instrument something different.

1. The premise fails. `crates/job_mix` runs native tasks on nife's own primitives. Warton
measured Linux processes whose every system call became an exception IPC to a user-level Linux
server. Those are different experiments, and no fidelity work on the mix turns one into the other.

2. The 20% is a ratio, and this tree has one kernel. Section 6's table, five runs:

| | single stack | multi stack | variable stack |
|---|---|---|---|
| average time | 157.69 | 197.22 | 157.78 |
| standard deviation | 0.015 | 0.344 | 0.019 |

(197.22 − 157.69) / 197.22 is 20.0%, which is where the retrospective's figure comes from. It is a
paired measurement: neither column means anything alone. nife has no event kernel to be the
other column, so the mix produces one arm and no ratio, whatever jobs it contains.

3. Warton's AIM7 run had already disabled two of the three categories `crates/job_mix`'s `BUGS`
apologizes for. Section 5.4 turned off the filesystem jobs (the ramdisk was too small) and the
network jobs (Wombat had no `GetHost`). The crate records a missing disk-file category as a fidelity
gap against AIM7; the AIM7 run being cited did not have one either.

4. It was two tasks, and there was no sweep. Section 5.4: *"The precise benchmark used was 2
clients with the normal workload file, with the disk and network tests removed"*, and section 6:
*"This workload was run in two user tasks."* This page lists a task-count sweep as one of AIM7's
four methodological properties and keeps it out to 32 tasks. The run the 20% comes from swept
nothing. The sweep is a good idea on its own merits and it is not a reproduction of Warton.

## What Warton thought of his own number

This is the part the retrospective compresses away entirely, and it is the reason the figure should
never be quoted flat. He expected the opposite result (section 5.2: *"the single stack kernel is
expected to perform similarly to or worse than the multi stack kernel"*), and the micro-benchmarks
delivered it. Section 7.3, on the AIM7 result:

> As with any experimental result, this needs to be treated with scepticism until it can be
> satisfactorily explained. Because the benchmark is not very stable and crashes on some runs, I
> initially doubted the results.

He ruled out the timer by re-running against a wall clock. Then:

> If the result is accurate, it must be due to reductions in the cache and TLB footprint of the
> single stack kernel. I did not expect that this reduction would make such a massive difference in
> performance [...] To determine the validity of this result a simulation of the cache impact of
> the kernels must be performed. There was not enough time to complete this simulation in the
> course of this thesis, due to external events.

The validating simulation was never run, and section 7.4 asks for another macro-benchmark for
the same reason. So here is the chain this project has been reasoning from. A BE thesis reports an
unexplained 20% its author flagged as needing skepticism, on a modified AIM7, on two tasks, on a
hosted Linux. A retrospective eleven years later summarizes it as a flat *"20% performance
advantage of the event kernel on a multitasking workload (AIM7)"*. And this tree built an instrument
to chase it. Every step is a real citation. The compression happened at the second one.

## The hardware, for completeness

Section 5.2: a littlechips LN2410SBC single-board computer with *"a Samsung S3C2410 arm processor
clocked at 200 MHz, a 32 kilobyte, 64 way associative cache, and 64 megabytes of ram."* The thesis
never says "ARMv5"; that is the retrospective's own gloss on the SoC. Note the cache: Warton
expected *"not many caching benefits [...] due to the single kernel stacks reduction of conflict
misses"* precisely because 64-way associativity makes conflict misses rare, which is what made the
result surprising to him.

## What survives, and it is the reason to keep the instrument

The mechanism, not the number. The only explanation Warton offered for his 20% is kernel cache
and TLB footprint, which is exactly the quantity `kernel/src/bench.rs`'s `app_displacement` and
milestone 134's E1 measure. Whether per-thread kernel stacks displace enough cache to cost
throughput on this kernel is a real, open, local question, and a knee in jobs-per-minute against
task count answers it. That is a genuine input to
`design/decisions/0096-process-kernel-or-event-kernel.md`. A reproduction of 20% was never available
and is not what this instrument was ever going to deliver.

## One gloss in §96 that the paper contradicts in the same sentence

§96 (process kernel or event kernel, and how to decide it) twice discounts the memory argument with the clause *"Warton's result came from resource-starved
embedded systems"*. The paper's own sentence, section 4.1, page 1:17, is:

> While this decision was driven initially by the realities of resource-starved embedded systems and
> later the needs of verification, the approach's benefits are not restricted to those contexts, and
> we believe it is generally the best approach on modern hardware.

The first half is where §96's clause comes from and it is fair: the move was driven by
resource-starved embedded realities. The second half rejects the inference §96 draws from it, in the
same sentence, and §96 does not record that it exists. The authors are not neutral on this and the
tree should quote them rather than paraphrase them into agreement.
