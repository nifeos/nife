---
status: OPTIONAL
raised: 2026-07-23
milestone_dependencies: 88
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 17. Multikernel-leaning scheduler (research, optional)

In brief. Partition or replicate what still sits under the one `IPC_TABLES` lock.

Why it matters. Optional; not on the thesis path.

Deliverable. Per-core state, with message-passing where `IPC_TABLES` now serializes every core. That
lock (rank 60, `kernel/src/sched/ipc_tables.rs`) guards the thread table, the rendezvous table and
the kernel's rendezvous chunks, the notification and timer tables, the futex buckets and the process
table.

What is per-core already. The SMP work of §11 (SMP: per-CPU run queues) and §28 (SMP placement)
gives each core its own run queue, `current` and held rank. A thread crosses cores through the
target's inbox (rank 59) and a reschedule interrupt from `arch::interrupt_controller::send_reschedule`. That
interrupt is a GIC SGI on aarch64, an SBI IPI on riscv64 and a local APIC IPI on x86_64. No core
locks another core's run queue. Each thread's capability table has its own lock, from milestone 761
(capability lookup off the global lock), so the running thread's lookups never take `IPC_TABLES`.

What the evidence says.
[notes/ipc-tables-lock-inventory.md](../../notes/ipc-tables-lock-inventory.md) classifies every
acquisition by call path. Its three findings are the shape any design here starts from:

- The hot set is IPC.
- Capability tables partition for free. Milestone 761 took exactly that piece.
- The §13 (capability revocation and untyped reclamation) sweeps are what partitioning makes
  expensive, because a sweep visits every thread. Milestone 183 (a physical-range index for
  capability holders) removes the scan.

The lock shows at four cores. On radon's job mix at four tasks with 761 in place, an instrumented
boot waits about 1.25 M ticks on `IPC_TABLES` across about 320,000 contended acquisitions. Those
waits belong to the inventory's hot rows: the IPC primitives, `schedule()` and `take_ipc_aborted`
([notes/job-mix/radon-2026-10-05.md](../../notes/job-mix/radon-2026-10-05.md)). Milestone 787 (the
IPC primitives look each thread up once) shortens those critical sections without partitioning
anything, and it comes first.

The gate. Whether the remainder justifies partitioning is a question for the `smp_throughput` curve
past four cores, which milestone 88 (nife on rented silicon) owes at its bench stage. The kernel
seats at most eight cores (`cpu::MAX_CPUS`, which is `current_cpu_contract::CPU_ID_BOUND`), so that
curve also needs the bound raised. A protocol that replaces the lock with messages is born
loom-checked under `script/interleaving-check`, the tool from milestone 80 (loom: the hand-rolled
atomic protocols, model-checked), as `crates/thread_wake_handshake` and `crates/work_steal_slot`
are. Until the curve bends, the answer is the one lock, on purpose. Barrelfish's multikernel (the
machine as a distributed system, message-passing between cores) is the research direction for NUMA
and P/E asymmetry, not a commitment.

## Index row

optional; not on the thesis path
