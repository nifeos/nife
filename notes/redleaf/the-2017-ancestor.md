# The ancestor: the 2017 case for writing a kernel in Rust

*Name: provisional. After the parent's "The ancestor: the 2017 case".*

An appendix to [`notes/redleaf.md`](../redleaf.md). It holds the reading of Levy et al. (APSys
'17), the three questions that paper left open and whether RedLeaf closed them, and what this tree
can and cannot say in the paper's terms. Every quotation is from the sources in
[`sources.md`](sources.md).

This is the argument RedLeaf inherits, and it is worth reading first because it is narrower and
therefore clearer. Levy et al. state it in one sentence: *"This paper takes a more extreme
approach: entirely throw away hardware protection within the kernel and, instead, write the kernel
in a memory-safe programming language."*

It positions against the obvious predecessors and rejects them on the runtime rather than the idea:
*"Both Spin and Singularity, however, use garbage-collected languages, which pose many problems for
kernels. Garbage collection complicates memory placement and layout, creates timing non-determinism
from background locks and introduces stop-the-world intervals that pause the entire OS."* A second
objection follows, and it is the one that sets up the whole contribution: *"Furthermore, both Spin
and Singularity depend on a large, unsafe code base: the Spin kernel and Singularity's runtime,
respectively."*

So the contribution is a claim about how little must be trusted, and it is made as two
enumerations rather than an argument. Four Rust-library abstractions that use `unsafe`: bounds
checks, iterator optimizations, compiler intrinsics and primitive casts, and `Cell`.
Six pieces of kernel code: context switches, memory-mapped I/O and structures, the memory
allocator, userspace buffers, interrupt/exception handlers, and `TakeCell`. Their own
framing: *"Together, they constitute the complete set of unsafe code in the kernel and are
surprisingly small, primarily consisting of mechanisms that any language or kernel needs to
provide."*

`TakeCell` is the one thing they had to invent, and the reason is a cost: *"Cell, unfortunately, is
only a partial solution. It imposes the significant cost of requiring memory copies, which is an
unacceptable overhead for complex or large kernel data structures."* `TakeCell` passes code in
through a closure instead of copying data out, its `map(f)` is a no-op when a reference already
exists (so it is *"a form of mutual exclusion ... unlike a mutex, it skips the operation instead of
blocking"*), and they show the generated ARM assembly to support the claim *"Once compiled, TakeCell
is just as fast as unchecked C code."*

One line carries the paper's whole quantitative claim, and it is quoted here in full because
truncating it changes what it says. *"The kernel's trusted computing base includes the Rust core
library as well as under 1000 lines out of over 6000 lines of kernel code."*

Two things about that sentence. It says *under* 1000, not "around" 1000. And the trusted computing
base it names is *not* 1000 lines: it is 1000 lines plus the whole of `libcore`, by the
sentence's own construction. The famous ratio is the *kernel-authored* part of the TCB, and a reader
who quotes "1000 of 6000" as the TCB size is dropping the larger and less auditable half.

The paper never says "Tock." The word does not appear in it; it says *"our kernel"* and *"a
kernel for low-power uniprocessors."* This is Tock's founding argument in substance, not in name.

## The three things the 2017 paper left open, and whether RedLeaf closed them

The paper is unusually clear about its own limits, which makes this answerable from the two documents
rather than by opinion.

Multiprocessor: RedLeaf implements it and never evaluated it, so the open question is still open.
Levy: *"we did not evaluate our design in a multi-processor setting"*, and its multicore subsection is explicit that
*"this paper has examined using Rust in a single-threaded setting."* Their ask is specific: *"Future
work should explore how to avoid growing the trusted computing base in service of concurrency."*
RedLeaf claims the capability (*"RedLeaf provides typical features of a modern kernel: multi-core
support, memory management, dynamic loading of kernel extensions, POSIX-like user processes, and
fast device drivers"*) but every published number in it is single-core by construction: *"In all
our tests, we pin the application thread to a single CPU core"*, *"All tests are limited to a single
CPU core"*, *"on all setups, we restrict execution to one CPU core."* And it says nothing at all
about what concurrency did to its trusted base, which is the half Levy actually asked about. So:
not answered.

Page tables: RedLeaf dissolves the question rather than answering it. Levy asks for future work
on *"data structures on disk or in hardware (e.g. the page table)"*, meaning: can a safe language
model a structure whose layout and semantics are fixed by something other than the compiler? RedLeaf
never has to, because it does not use hardware address spaces for isolation at all. That is a
legitimate move and it has a visible price, stated by RedLeaf itself: *"We do not support the full
semantics of the fork() system call as we do not rely on address spaces and hence cannot virtualize
and clone the address space of the domain."* The question Levy posed is untouched, and it is a
question this tree answers head-on. `crates/paging` is where the page-table math lives precisely so
a model checker can reach it. Milestone 193 (put `kernel/src` within reach of the prover) is about
getting the prover to the rest.

Size of the trusted base: RedLeaf regressed from a number to a list. This is the finding worth
carrying. Levy's whole contribution is a *measured* TCB ("under 1000 lines out of over 6000"), and
the RedLeaf paper contains no such figure anywhere. It enumerates instead: *"RedLeaf's TCB
includes the microkernel, a small set of trusted RedLeaf crates required to implement hardware
interfaces and low-level abstractions, device crates that provide a safe interface to hardware
resources, e.g., access to DMA buffers, etc., the RedLeaf IDL compiler, and the RedLeaf trusted
compilation environment."* That list is strictly larger in kind than Tock's: it adds an IDL
compiler and a trusted build environment, neither of which Tock needs, and both of which the
same authors later called *"incomplete and error prone"* (PLOS '21). So the trajectory from 2017 to
2020 is that the bet got bigger and the accounting for it got weaker, and nobody quantified the
difference.

## What this tree can and cannot say in their terms

It cannot say it in their terms, and the reason is architectural rather than a gap in the
tooling. `notes/trusted-base.md` is the worked version: the three definitions of "trusted", the
re-derived numbers, their six unsafe categories run against this tree file by file, and the
`TakeCell` question answered from the code. The short form:

- In Tock and in RedLeaf the *safe* part of the kernel is *not* trusted for isolation, so counting
  unsafe lines is a genuine TCB measurement.
- In nife the *whole* kernel is trusted for isolation whether or not it is safe, because it is the
  thing that programs the MMU and mints capabilities. The unsafe census measures proof burden, not
  TCB size.
- The tree's own numbers, from `script/metrics --table` on 2026-09-20: 39,892 kernel code lines,
  577 `unsafe` blocks inside the kernel (314 of them in `arch/`), and an unsafe density of 77
  per 10,000 non-arch code lines against a ceiling of 88. The headline `unsafe_outside_arch` of 824
  is misleading for this purpose, because 561 of those blocks are in userspace, outside the
  trusted base entirely.
- The apples-to-apples comparison is with seL4, which draws the boundary in the same place:
  *"of the order of ten thousand lines of source code (10 kSLOC)"* (the seL4 Foundation whitepaper).

And the meta-gap is the one worth closing. `notes/thread-control-block.md` is about the *Thread* Control Block,
and its own acronym-collision section says the Trusted Computing Base sense *"is unrelated"*. No
metric column tracks the trusted core's size, so the split above is a snapshot with nothing watching
it. Proposed as `design/roadmap/0535-the-trusted-core-has-no-size.md` rather than fixed here.
