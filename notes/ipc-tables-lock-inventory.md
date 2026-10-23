# The `IPC_TABLES` lock: what it protects, and how hot each thing is

*The inventory milestone 17 asks for before anyone partitions anything. Method: every
`IPC_TABLES.lock()` site in `kernel/src/sched.rs` (41 functions as of 2026-08-03), classified by call
path. The classification is by reading, not by measurement; the measurement is exactly what
milestone 88's scaling curve exists to add. The type and the static are `IpcTables`/`IPC_TABLES`,
per DECISIONS §118 (`Scheduler`/`SCHED` rename to `IpcTables`/`IPC_TABLES`), because, as this note
finds below, the hot set is IPC rather than scheduling.*

Run queues, `current` and held rank are per-CPU (DECISIONS §11 (SMP: per-CPU run queues) and
§28 (SMP placement)). Each thread's CapabilityTable has its own lock, from
milestone 761 (capability lookup off the global lock). What lives under the one `IPC_TABLES` lock
is the thread table, the rendezvous table, the notification and timer tables, the futex buckets and the
process table (`kernel/src/sched/ipc_tables.rs`). Milestone 17 (multikernel-leaning)'s question is
whether that remainder ever costs enough to justify partitioning it; this note is the denominator
for that question.

## By temperature

| Class | Functions | Why it matters |
|---|---|---|
| **Hot: every IPC, every core** | `ipc_send`, `ipc_receive`, `ipc_call`, `ipc_reply`, `ipc_send_cap`, `ipc_receive_cap`, `irq_notify` | The `call_reply` fast path this project benchmarks and wins on takes `IPC_TABLES` at least once per operation. At 4 cores on radon this is most of what is left of the lock's wait ([below](#what-the-measurement-found-2026-10-04)); whether partitioning it pays past 4 is THE milestone 17 question |
| **Hot: every reschedule** | `schedule` (twice), `depart` | Runs on the core's own queue, but takes the global lock to touch the thread table |
| **Warm: per capability operation** | `delete_current_cap`, `take_ipc_aborted` | `current_cap`, `grant` and `grant_at` take only the running thread's own CapabilityTable lock (milestone 761, finding 2 below). These two still take `IPC_TABLES` |
| **Cold: lifecycle** | `create_tcb`, `configure_tcb`, `start_tcb`, `tcb_insert_cap`, `spawn_on`, `spawn_with_quota`, `kill_thread`, `reap_supervised`, `reap_region_objects`, `create_endpoint`, `try_create_endpoint_from`, `adopt_address_space`, `adopt_secondary_idle`, `init` | Dozens per boot, not thousands per second. No plausible contention at any scale this project names |
| **Sweeps: revocation** | `delete_frame_caps`, `delete_device_frame_caps_from_others` | Whole-table scans by design (§13 revokes by physical page). Rare, but they hold the lock for a table-length walk, which is a latency spike every other core eats |
| **Observability, test builds** | `thread_count`, `is_thread_present`, `dump_threads`, `user_pc_of`, `corpse_fault_msg`, `endpoint_waiting_senders`, `runnable_non_idle_count`, `is_running`, `current_kernel_stack_top` | Test and diagnostic surface. Never a reason to partition, and several are the same functions milestone 78 (the load-sensitive assertions) rescoped tests away from |

## What this says about milestone 17, without a measurement

Three structural observations survive even before the curve exists:

1. **The hot set is IPC.** If the lock ever shows in data, it shows as IPC throughput flattening
   as harts increase while per-op latency holds. That is precisely what `bench --smp`'s
   `smp_throughput` measures, so the instrument mostly exists; what is missing is a machine with
   enough harts for the curve to bend (milestone 88's Graviton stages, up to 64).
2. **CapabilityTable operations partition for free.** They are logically per-thread, and milestone
   761 gives each table its own lock, so the running thread's lookups contend with nobody. On
   radon at four tasks that halved the `IPC_TABLES` wait ([radon-2026-10-05.md](job-mix/radon-2026-10-05.md)).
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

## What the tables hold, and the limits that size them

`kernel/src/sched/ipc_tables.rs` defines the tables and their limits. Each object registry
(rendezvous, notifications, timers, processes) maps a generational name to the physical address of
the page the object lives at the start of. The page is retyped from some region that stays pinned
while the object lives. The name is what a capability carries, so a stale one fails safely.

### The lock's rank

The rank sits above the allocators. The reaper (`finish_switch`) drops a dead `Thread` while
holding `IPC_TABLES`, and that drop frees: the kernel stack's pages go back through the kernel MMU
lock, and its address range to its free list. Freeing takes the locks allocating does. Nothing
under the lock allocates from a heap. A TCB page comes from a region or from `kmem`'s fixed pool,
and the queues are intrusive, so a queue operation is a few pointer writes. §9's
no-allocation-in-IRQ rule holds by construction.

### `MAX_THREADS`

`MAX_THREADS` is 256, a documented limit of the image, so a spawn past it fails cleanly. The boot
keeps many services alive for its whole life (notes/frames.md's "held" list), so every milestone
that adds one spends some of it. `PEAK_THREADS` and `kernel::testing`'s closing `threads:` line print
the demand on every run; when the limit was set the peaks were 130 (aarch64), 129 (riscv64) and 57
(`x86_64`). 256 is about twice the tightest, because the failure is an unrelated test refusing a
spawn far from its cause. What scales with it, each checked at 256:

- the boot stack, about 20 bytes a slot, which is why `init` installs `EMPTY_TABLES` from `.rodata`
  rather than building the tables on the boot stack, under `stack::report_high_water`'s gate;
- `kmem::KERNEL_OBJ_PAGES`, 7 pages per live thread (`thread::STACK_PAGES` and the TCB page);
- `ps::MAX_ROWS`, const-asserted `>= MAX_THREADS` in `kernel::user::survey_tests`, which sizes
  stack-resident row arrays in `ps` and `pgrep` inside `system_initializer::CHILD_STACK_PAGES`;
- `revoke::MAX_SPACES` and `thread::FreeAddressSpace`, derived from it.

Nothing on the `spawn_el0` benchmark scales with it, because the revocation scans are bounded by
live occupancy ([notes/benchmarks.md](benchmarks.md)). No `[u64; MAX_THREADS]` scratch array sits
on a stack ([notes/region-reaping.md](region-reaping.md)).

### `MAX_RENDEZVOUS`, and the kernel's chunks

`MAX_RENDEZVOUS` is 512 live at once. Tearing down a region removes its rendezvous and frees their
slots, so it bounds concurrency, not creations. `PEAK_RENDEZVOUS` and `KERNEL_CHUNK_RENDEZVOUS`
report the peak and how many of those sat on the kernel's own chunks, updated at the one insert
under the lock. Measured 2026-10-19 (UTC) on the `system_tests` image:

| architecture | peak | spare | on kernel chunks |
|---|---|---|---|
| aarch64 | **269** | 243 | 188 |
| riscv64 | **260** | 252 | 179 |
| `x86_64` | **189** | 323 | 133 |

A rendezvous on a kernel chunk lives for the whole boot, so every test that wires a service with
`create_rendezvous` spends the registry until shutdown. On aarch64 the largest spenders are the
compositor and display services, the disk service, and the credential, block-driver, clock and login
tests. The fix is to retype the endpoint from the run's own region
(`design/roadmap/0670-test-helpers-give-their-clients-a-region-to-retype-from.md`). It reports and
does not gate.

`create_rendezvous` retypes the kernel's own rendezvous from a chunk of `KERNEL_EP_CHUNK_PAGES` (32
pages). The chunk is carved lazily and replaced when it fills, so the kernel's supply is not a
compile-time guess. A fixed ceiling grew with the test suite, and the union of parallel branches'
boots crossed it at merge, where no branch could see it coming. A normal boot carves one chunk. A
filled chunk's handle is forgotten; its pages stay pinned and its endpoints live. No path tears a
kernel chunk down, and if one should, the handle becomes an array. `MAX_KERNEL_EP_CHUNKS` is derived
so chunks can host `MAX_RENDEZVOUS` one-page endpoints, which makes the registry, never the page
supply, the limit that refuses.

### `FUTEX_BUCKETS`

`FUTEX_BUCKETS` is 64. A wake walks one bucket, so that is four waiters a bucket on average with the
machine full, at 24 bytes a bucket. The futex queues are under this lock for the reason every wait
queue is: the check of the word, the park and a waker's take must be one order.

## BUGS

- The temperature classification is by reading call paths, not by counting acquisitions. There is
  a counter, though not a per-acquisition one.
  `--features lock_wait` (kernel/src/lock_wait.rs) counts every *contended* acquisition per lock
  rank and its spin time, in the job mix's timed windows, and reads no clock on an uncontended one. A per-site hold-time table would turn the rows above into numbers; one was built for QEMU
  and not committed, because reading the clock on every acquisition may itself be expensive on
  radon. The contention that matters at scale still needs milestone 88's hardware.
- `kernel/src/sched.rs` line numbers drift; the inventory is by function name on purpose. Re-grep
  before trusting the count of 41.
