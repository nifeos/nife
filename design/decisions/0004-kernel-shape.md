---
status: DECIDED
raised: 2026-07-13
decided: 2026-11-10
ratified_by: calef
---

# 4. Kernel shape: a microkernel, with two rules that keep it one

The kernel holds only what must run at EL1, plus what a confinement argument needs. Everything else
is a userspace program holding objects the kernel hands out: drivers and their control planes, bus
enumeration, boot services and whatever starts them (§10 (process model: capability-based,
microkernel)). A piece of code earns a place in the kernel by naming which of the two it is.

Two rules keep it that way:

1. **A driver never reaches into a kernel global.** It gets what it needs passed in.
2. **The syscall surface stays narrow and explicit.** It is a boundary, not a habit.
