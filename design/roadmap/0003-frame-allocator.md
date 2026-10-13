---
status: BUILT
raised: 2026-07-13
built: 2026-07-13
---
# 3. Hand out physical memory, and detect a smashed stack

Built in `ba47a27` (2026-07-13), written up in `097ef52`, with the initrd reservation and the
bitmap's placement proven in `bad2e28`.

The kernel reads its memory map from the device tree instead of a constant, and hands out 4 KiB
frames. The commit records the two decisions that still describe the allocator: the bitmap is
carved by hand out of the memory it is about to manage ("the allocator's first act is to allocate
itself"), and everything starts marked USED, because a bitmap that defaulted to free would hand
out the MMIO hole and give you the UART's control registers as scratch space.

The milestone also blew the kernel stack, took an embarrassingly long time to notice, and left
two permanent marks: the stack canary (checked after every test, in the panic handler, and in the
fault handler) and notes/stack.md. The canary is recorded in the commit as a mitigation rather
than a fix; the real fix, a guard page below the stack, waited for milestone 4's MMU.

One known hole was recorded rather than papered over: the allocator's lock was a formality on one
core with no interrupts, and the commit demanded a written-down locking discipline before
milestone 5. That arrived as `6974f6c` (IrqSafeMutex, handlers do not allocate) and was later
enforced in `e85802d` rather than merely written down.

## Follow-on

- **Milestone 4.** The guard page below the kernel stack. The stack canary this milestone added is
  recorded in its own commit as a mitigation rather than a fix, and the real fix needs the MMU that
  milestone 4 turns on.
- **Milestone 5.** The written-down locking discipline the commit demanded before a timer interrupt
  could land between any two instructions. It arrived as `IrqSafeMutex` and DECISIONS §9, and
  milestone 5 is where it stopped being a hypothesis.

## Index row

where RAM actually comes from, and the allocator allocates itself first
