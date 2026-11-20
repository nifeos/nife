---
status: DECIDED
raised: 2026-10-09
decided: 2026-10-09
ratified_by: calef
---

# 269. How threads share a process: the seven thread forks

Raised 2026-10-09 (UTC) by a maintainer session, which took calef through
[milestone 812 (`std::thread::spawn` runs real threads in one address space)](../roadmap/0812-threads-in-one-address-space.md)'s
seven forks one at a time. Four of them (1, 2, 3 and 6) are system call surface under §10 (the
capability-based microkernel process model), so 812 owes this section before any code. *(Section
number provisional until the merge queue lands it.)*

Three of the rulings differ from what 812's block recommended (forks 4, 5 and 6), and two widen
it (forks 2 and 7). Each says so.

## The rulings

1. A second thread joins by a `BIND` right, which lets `CONFIGURE` bind without consuming.
   calef: *"Why would I pick anything but a?"* Adding a thread is running code inside the space,
   so it is an authority that can be withheld and narrowed on delegation. See the open consequence
   below: fork 5 moves where the right lives.
2. Futex-shaped wait and wake, with an ABI ready for shared futexes. Methods on the space
   capability take a virtual address in the caller's own space, a flags word and a size field.
   `PRIVATE`, keyed by (space, address), is built now. `SHARED`, keyed by (memory object, offset),
   is reserved and refused until a cross-process user needs it. The size field admits 32 bits
   only for now. calef chose this over the block's private-only form after asking what is most
   compatible with the long term. Linux added `FUTEX_PRIVATE_FLAG` (2.6.22) and futex2's size field
   after the fact; reserving both now makes the shared key purely additive.
3. Sibling threads share one capability table, and the table becomes its own object. Today it
   is embedded in each thread's page (`kernel/src/thread.rs`, `CAPABILITY_TABLE_OFFSET`). Each TCB
   holds a pointer to it, as seL4's TCBs do. A table per thread was refused: `std`'s `Send` and
   `Sync` handles move as bytes with no hook at the crossing, and separate tables inside one
   address space are not a boundary, since a sibling can write the other's stack.
4. One thread-pointer ABI on all three architectures, set by the kernel. The initial value
   arrives at `CONFIGURE` (Linux's `CLONE_SETTLS`), the kernel saves and restores it per thread,
   and a TCB method changes it later (seL4's `SetTLSBase`). A user write to the register is not
   part of the contract. On `x86_64`, `CR4.FSGSBASE` stays off, so a user-chosen `GS` base never
   reaches the kernel entry, whose NMI exit window has no paranoid path yet (`trap.s`). calef:
   *"I am hung up on parity."* That ruled out the split first recommended (user-writable on
   `aarch64` and `riscv64`, kernel-set on `x86_64`) and the block's (a). What is left
   asymmetric is below the ABI: the hardware lets an `aarch64` or `riscv64` program write its own
   register. A scope note records it, the way §139 (who may read the cycle counter, and by what authority) records `rdtsc`.
5. The capability-table object of fork 3 is the process. It holds the table, a reference to
   the address space, the set of threads, the exit status and the quota. Destroying it ends every
   thread in it atomically: `exit()` destroys the caller's own process, and a supervisor's kill
   destroys the one it names. The block recommended that `std` destroy its siblings through the
   TCB capabilities it holds, which misses threads `std` did not create and races a concurrent
   spawn. Making the address space the group was also considered and refused: it is LinuxThreads'
   shape, which NPTL replaced with a thread group in 2003, and memory can be shared for reasons
   other than being one process.
6. The current-CPU page becomes a page per thread. Each thread gets its own read-only page,
   mapped at thread creation and unmapped with its stack at exit. Its address arrives at
   `CONFIGURE` and the thread keeps it in TLS. calef: *"I'm leaning b, but want to understand how
   often thousands of threads comes up in practice."* The block's slot-per-thread page was
   refused: it needs a slot allocator and a false-sharing measurement, and its density matters
   only for thread-per-connection servers with thousands of idle threads, which nothing on the
   roadmap is. Moving to slots later changes how the address is computed, not the ABI.
7. `available_parallelism()` reads an allowance from the thread's page. A read-only field on
   fork 6's page, defined as the cores this process may use, not the cores the machine has. It
   holds the online count today, counted from the online set and not from `CPU_ID_BOUND`, and a
   later spawner grant or affinity budget lowers it without changing its meaning. Linux's `std`
   (cgroup quotas) and Java (JDK 10's container support) both had to retrofit that meaning.
   Milestone 527 (the `SURVEY` selector, and a thread's placement) was refused for this: it is a supervisor's view of other
   threads behind `ENUMERATE`, and a thread observing itself uses a page, per calef's 2026-09-21
   ruling quoted in `crates/current_cpu_contract`. 527 itself is unchanged; its callers (`ps`,
   `top`) observe other threads, which is what `ENUMERATE` is for.

## An open consequence, not ruled

Fork 1 put `BIND` on the address-space capability. Fork 5 makes a thread join a process, which
holds the space. The right probably belongs on the process object instead. Fork 1's argument
carries over unchanged, but where the right lives is calef's, and 812's lane brings it back
before the code that adds the right.

*Answered 2026-10-10 (UTC) on #1892: the right lives on a process capability. §276 (the thread
system calls as built) records the ruling and the surface.*

## What this changes in milestone 812

Work items 1 to 4 now build a process object as well as a shared table, and items 11 and 16
follow forks 6 and 7. Item 14 (the per-thread user registers) follows fork 4. The exit test is
unchanged.
