# Notes index: The kernel

Threads, capabilities, IPC, and how authority ends.

Part of [the notes index](../README.md), which says how to add a line.

- [Userspace](../userspace.md): the three hardware walls that make userspace.
- [The console driver leaves the kernel](../userspace-drivers.md): the console as a userspace process reached by IPC.
- [The native ABI](../abi.md): the syscall convention, object surface and entry contract.
- [Threads, the context switch, and preemption](../threads.md): threads, the fifteen-instruction switch, and preemption.
- [The scheduler: placement, stealing, wakes](../scheduler.md): per-core run queues, placement, stealing and wakes.
- [The TCB](../tcb.md): the Thread Control Block, and where it lives.
- [Generational names](../generational-names.md): thread ids that fail safely after slot reuse.
- [Intrusive queues](../intrusive-queues.md): run queues whose link lives inside the thread.
- [Locking](../locking.md): spinlocks with interrupts, and the orderings that avoid deadlock.
- [Deadlock](../deadlock.md): the four Coffman conditions and how to break one.
- [Memory ordering, and the fences with no partner](../memory-ordering.md): every fence and ordered atomic, adjudicated.
- [The IPC_TABLES lock inventory](../ipc-tables-lock-inventory.md): what the one remaining IPC lock protects, by heat.
- [Capabilities, and why the kernel has no `open()`](../capabilities.md): capabilities, the confused deputy, first syscalls and IPC.
- [Who does IPC name?](../ipc-naming.md): IPC names a rendezvous, never the peer.
- [Futex wait and wake](../futex.md): parking a thread on a word of its own space, and why the check and the park are one step. Name provisional.
- [The thread pointer, and the one asymmetry below its ABI](../thread-pointer.md): each thread's own `TPIDR_EL0`, `tp` or `FS` base, set by the kernel.
- [Notification objects](../notification-objects.md): a doorbell bound to a thread, and why the receive tag lives in `w4`.
- [How authority moves, narrows, and ends](../capability-lifecycle.md): how capabilities are copied, narrowed and revoked.
- [Delegating a capability](../delegation.md): passing a narrowed capability between processes over IPC.
- [The progenitor's capability-table peak, slot by slot](../capability-peak-trace.md): which capabilities hold the 31 of 32, and what each option to lower it saves. Name provisional.
- [Object revocation: tearing a process back down](../object-revocation.md): reclaiming the kernel objects a process built.
- [Ending a permanently blocked thread](../blocked-thread-teardown.md): research and proposals for ending a blocked thread.
- [Supervision: a thread's death becomes a message](../supervision.md): the fault endpoint, and reaping without building.
- [Per-process resource quotas](../quotas.md): a live-children cap on spawners, kept but unused.
- [What a timed wait costs](../timed-wait.md): pricing a deadline on a blocked thread.
- [The timer](../timer.md): the object that ends a wait at a deadline.
- [Can a userspace process hold a timer?](../timer-capability.md).
- [Trusted init: measuring the boot program, and then everything the progenitor loads](../trusted-init.md).
- [The progenitor, and loading a program from userspace](../progenitor-and-loading.md).
- [Auditing the hand-written arch assembly](../arch-audit.md): a by-hand audit of the least-verified TCB code.
- [The L4 lessons, audited against this kernel](../l4-lessons.md): the kernel checked against L4's twenty-year retrospective.
- [How the log service drains the kernel's ring](../kernel-ring-drain.md): milestone 342 (the kernel and the `console` server drive one UART)'s proposal, a read-only frame plus a notification against a read method on a new object, with the seven questions answered.
