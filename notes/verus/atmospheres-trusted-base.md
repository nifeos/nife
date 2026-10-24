# Atmosphere's trusted base, and where it draws the hardware boundary

*Name: provisional, for the trusted computing base it quotes.*

An appendix to [`notes/verus.md`](../verus.md), under "Does Verus reach `asm!` and MMIO?". It holds
SOSP '25's own list of the trusted assembly and Rust beside its verified kernel, and the rest of that
paper's trusted computing base.

This is the finding that answers the brief's question. SOSP '25 Section 5, its Trusted Computing Base section, item 8,
verbatim:

> Sequences of assembly and trusted Rust code. The fragments of assembly code (172 lines) implement
> entry and exit trampolines for system call and interrupt handlers. Trusted Rust code (total of
> around 3,000 lines) implementing hardware interface to configure IOMMU (465 lines), setup execution
> environment of the kernel (321 lines), initialize interrupt handling (293 lines), advanced
> programmable interrupt controller (287 lines), interrupt descriptor table (179 lines), initialize
> task state segment (TSS) and global descriptor table (172 lines), fast system call entry via
> sysenter (123 lines), initialize per-CPU data structures and application processes, etc.

Read that list against this tree's `arch/x86_64/`. IOMMU configuration, interrupt setup, the
APIC, the IDT, the GDT, the TSS, the syscall entry: it is the same file list, and in Atmosphere it is
trusted, not verified. A 6K-line verified kernel carries 3,172 lines of hand-trusted hardware
code beside it, and that trusted code is more than half the size of the verified part.

The same section is candid about the rest of the TCB: the Verus frontend, Z3 (*"any unsoundness in it
will compromise the soundness of verification"*), the Rust compiler and toolchain, the `core`
library, specifications of `core` routines, and *"Axioms missing in Verus... These axioms can be
verified in the future"* (around 700 lines of spec). KISV '23 adds one more that matters to a kernel:
*"Verus cannot guarantee the absence of stack overflows since it does not model the hardware and
relies on Rust to correctly abstract details of the machine executing the code."*
