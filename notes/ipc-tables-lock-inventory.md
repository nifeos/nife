# The `IPC_TABLES` lock: what it protects, and how hot each thing is

*The inventory milestone 17 asks for before anyone partitions anything. Method: every
`IPC_TABLES.lock()` site in `kernel/src/sched.rs` (41 functions as of 2026-08-03), classified by call
path. The classification is by reading, not by measurement; the measurement is exactly what
milestone 88's scaling curve exists to add. The type and the static are `IpcTables`/`IPC_TABLES`,
per DECISIONS §118 (`Scheduler`/`SCHED` rename to `IpcTables`/`IPC_TABLES`), because, as this note
finds below, the hot set is IPC rather than scheduling.*

The scheduler's per-CPU migration (DECISIONS §28) already moved run queues, `current`, and
held-rank out of shared state. What still lives under the one `IPC_TABLES` lock is the thread
table (and with it every thread's CapabilityTable) and the endpoint array. Milestone 17 (multikernel-leaning)'s question is
whether that remainder ever costs enough to justify partitioning it; this note is the denominator
for that question.

## By temperature

| Class | Functions | Why it matters |
|---|---|---|
| **Hot: every IPC, every core** | `ipc_send`, `ipc_receive`, `ipc_call`, `ipc_reply`, `ipc_send_cap`, `ipc_receive_cap`, `irq_notify` | The `call_reply` fast path this project benchmarks and wins on takes `IPC_TABLES` at least once per operation. At 4 harts the hold times are short enough not to show; whether that survives 64 harts is THE milestone 17 question |
| **Hot: every reschedule** | `schedule` (twice), `depart` | Runs on the core's own queue, but takes the global lock to touch the thread table |
| **Warm: per capability operation** | `grant`, `grant_at`, `current_cap`, `delete_current_cap`, `take_ipc_aborted` | CapabilityTables live inside thread-table entries, so a purely thread-local capability lookup pays for the global lock. If partitioning ever happens, this is the piece that partitions for free (a CapabilityTable has exactly one owner) |
| **Cold: lifecycle** | `create_tcb`, `configure_tcb`, `start_tcb`, `tcb_insert_cap`, `spawn_on`, `spawn_with_quota`, `kill_thread`, `reap_supervised`, `reap_region_objects`, `create_endpoint`, `try_create_endpoint_from`, `adopt_address_space`, `adopt_secondary_idle`, `init` | Dozens per boot, not thousands per second. No plausible contention at any scale this project names |
| **Sweeps: revocation** | `delete_frame_caps`, `delete_device_frame_caps_from_others` | Whole-table scans by design (§13 revokes by physical page). Rare, but they hold the lock for a table-length walk, which is a latency spike every other core eats |
| **Observability, test builds** | `thread_count`, `is_thread_present`, `dump_threads`, `user_pc_of`, `corpse_fault_msg`, `endpoint_waiting_senders`, `runnable_non_idle_count`, `is_running`, `current_kernel_stack_top` | Test and diagnostic surface. Never a reason to partition, and several are the same functions milestone 78 (the load-sensitive assertions) rescoped tests away from |

## What this says about milestone 17, without a measurement

Three structural observations survive even before the curve exists:

1. **The hot set is IPC.** If the lock ever shows in data, it shows as IPC throughput flattening
   as harts increase while per-op latency holds. That is precisely what `bench --smp`'s
   `smp_throughput` measures, so the instrument mostly exists; what is missing is a machine with
   enough harts for the curve to bend (milestone 88's Graviton stages, up to 64).
2. **CapabilityTable operations are the free win if anything is.** They are logically per-thread already;
   they share the lock only because CapabilityTables are stored in the table. A partition that moved
   nothing else would still take them off the global lock. Any milestone 17 design should price
   this piece separately from the thread table proper.
3. **The revocation sweeps are the awkward customers.** Revoke-by-physical-page wants to visit
   every CapabilityTable, which is exactly the operation partitioning makes expensive (visit every shard,
   or broadcast). A partitioned design pays here to win on IPC; the trade should be measured, not
   assumed, and §13's semantics are not negotiable to make a scheduler faster.

## The sequencing, recorded

Milestone 17 stays OPTIONAL and gated on evidence: milestone 88 (nife on rented silicon) provides the machine (the
scaling curve at 4/8/16/64 harts is a stated deliverable of its bench stage). Milestone 80 (loom)
provides the method (any design that replaces this lock with messages wants its protocol born
loom-checked; the wake-before-switch-out race is the standing proof that SMP interleavings hide
from this tree's other tools). Until the curve bends, the one lock is the right design, on
purpose.

## What the measurement found (2026-10-04)

The table above classifies by call path, and a call path's temperature is not its hold time.
Measured under QEMU, the job mix at four tasks, the hold time on `IPC_TABLES` is the IPC path's own,
spread across `schedule`, `CALL`, `REPLY` and `current_cap`. `finish_switch` reaps dead threads, and
`start_thread_control_block` (filed as cold) builds a stack, outside the lock, because freeing a
thread's kernel stack is six TLB shootdowns that interrupt every core. The lesson for this table: a cold function that runs
a cross-core operation under the global lock is hot for everyone else.
[`job-mix/null-syscall-under-load.md`](job-mix/null-syscall-under-load.md) has the numbers.

## BUGS

- The temperature classification is by reading call paths, not by counting acquisitions. There is
  a counter, though not a per-acquisition one.
  `--features lock_wait` (kernel/src/lock_wait.rs) counts every *contended* acquisition per lock
  rank and its spin time, in the job mix's timed windows, and reads no clock on an uncontended one. A per-site hold-time table would turn the rows above into numbers; one was built for QEMU
  and not committed, because reading the clock on every acquisition may itself be expensive on
  radon. The contention that matters at scale still needs milestone 88's hardware.
- `kernel/src/sched.rs` line numbers drift; the inventory is by function name on purpose. Re-grep
  before trusting the count of 41.
