# Notes index: Memory

Part of [the notes index](../README.md), which says how to add a line.

- [Physical memory](../physical-memory.md): the frame allocator, bitmap over free list.
- [The MMU](../mmu.md): virtual addresses, page tables, the TLB and page faults.
- [aarch64 page tables](../page-tables.md): the descriptor format the MMU walks, and its traps.
- [The higher-half kernel](../higher-half.md): why the kernel lives in TTBR1, and how it boots.
- [The user address-space map](../address-space-map.md): which band of a process holds what, and where a new page goes.
- [The current-CPU page](../current-cpu-page.md): the per-thread page through which a thread reads its own CPU, and why it sits beside the stack.
- [Tearing down an address space](../teardown.md): how the kernel reclaims a dead address space's frames.
- [Memory regions: the kernel stops allocating](../memory-regions.md): processes spend pages from their own memory capability.
- [Region reaping](../region-reaping.md): what destroying a region does to the threads, rendezvous and other objects in it.
- [A split refused for a full region table](../region-split-on-a-full-table.md): PROPOSED; why a refused split holds its parent forever, and the options.
- [PageFrame capabilities](../frames.md): shared memory a process owns, maps and delegates.
- [Giving up a mapping](../unmap.md): `AddressSpace::UNMAP`, its two provisional semantics, and why it cannot yet reach a running space.
- [`PageFrame::SLICE`](../page-frame-slice.md): a capability naming part of a run, and the file service's window pool it serves.
- [Naming a running address space](../naming-a-running-address-space.md): research for the #1678 fork; what seL4 and Zircon do, and what option A must say.
- [ASIDs: tagged address spaces](../address-space-identifiers.md): per-space TLB tags so context switches flush nothing.
- [The RISC-V TLB shootdown](../riscv-tlb-shootdown.md): cross-hart ASID flush via SBI RFENCE, replacing full flushes.
- [The x86_64 TLB shootdown](../x86-tlb-shootdown.md): cross-core TLB invalidation on x86, done by NMI.
- [The kernel's own budget](../kernel-budget.md): kernel stacks drawn from one fixed boot-carved region.
