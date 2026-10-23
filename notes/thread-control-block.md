# The thread control block (TCB)

*(Written during milestone 14 phase B, when "where do TCBs live" became a decision.)*

*Renamed 2026-08-24 (DECISIONS §113, milestone 158): the kernel capability object this note
describes, "a thread under construction", was spelled `Tcb` (and its pointer wrapper `TcbPtr`) in
code until this rename; both are now `ThreadControlBlock` and `ThreadControlBlockPointer`. The
companion thread-id type `Tid` is now `ThreadId`. This note keeps saying "TCB" throughout, on
purpose. That acronym is what the type is now spelled out in full, so the informal usage and the
type name finally agree. The acronym-collision section below (Thread Control Block vs Trusted
Computing Base) is unaffected by the rename either way.*

## What it is

The kernel's bookkeeping record for one thread: our `Thread` struct in `kernel/src/thread.rs`.
Milestone 1 defined a thread as "a stack plus a set of register values"; the TCB is where the
kernel keeps everything needed to *manage* that thread while it is not running. Ours holds the id (a
generational name, notes/generational-names.md), the state, and the `context` pointer naming where
the registers are saved on the thread's own stack. It holds ownership of that stack, the address
space, the capability table (`CapabilityTable`), the IPC mailbox, the intrusive queue link
(notes/intrusive-queues.md), and the `on_cpu`/`wake_pending` switch-out flags. "Spawn allocates
a TCB" means allocating this struct. seL4 uses the same name for the same object.

## The acronym collision, so nobody trips on it

**TCB also means Trusted Computing Base** (DECISIONS §14: "a small, machine-checked trusted
core"), which is unrelated. Both senses appear in this project's documents. In milestone 14 and
scheduler contexts, TCB is the thread struct; in §14 and verification contexts, it is the
trusted core. Expand the term when there is any doubt.

## Where TCBs live (the phase B.2 decision)

Decided at B.2: a static pool, a MAX_THREADS-sized array in BSS, a ThreadId's slot bits naming
its storage directly. That pool was always a scaffold: the B.2 note said "the pool upgrades to
retype-backed storage behind the table when init lands."

Milestone 19c.2 landed that upgrade, and the static pool is gone. Every `Thread` is now
**page-resident**: it lives at the start of one page, and the generational table stores a
pointer to it rather than indexing a BSS array. Kernel threads' TCB pages come from the kernel's
own budget (`kmem`, notes/kernel-budget.md); a user process (19c.3) retypes its TCB page from
its own memory region by the same mechanism, the page merely coming from a different budget. A page's
address never changes (direct-mapped, its region pinned), which supplies the pinning the `Box`
and then the pool both provided. The win over the pool: the kernel reserves no per-thread memory
it was not handed, the last corner of milestone 14's no-open-ended-spending thesis. Why this is
now worth doing when B.2 said retype earned nothing: B.2's premise was "the kernel is the only
payer," and 19c is exactly when that expires (the progenitor becomes a payer).

On the page-granularity worry the pool decision leaned on: a TCB is sub-page, so page-residency
"wastes" most of a page. That was a real reason to prefer the pool *while the kernel paid*. It is
not one now, because the page is paid by whoever owns the thread (a user thread's by its
creator's memory region, a kernel thread's by `kmem`, both bounded budgets). Sub-page packing stays
declined for the same reason it was for endpoints (19a). One memory rule for the whole object
family, packing a later placement optimization behind the table, not a slab rebuilt in the
milestone that deleted the slab.

## What shares the TCB page

A `Thread` sits at the start of its page, and the page's idle space holds two more things: the
thread's FP/SIMD register file, then its capability table behind its own lock. The saved
general-purpose state is one stack pointer in the struct. The vector registers are four times its
size, and they live in the page beside it. Compile-time assertions in `kernel/src/thread.rs` are what keep
all three inside 4,096 bytes; growing `Thread` past the point where they fit stops the build.

The capability table lives outside the struct for soundness, not layout. The running thread reads
its own table with `IPC_TABLES` not held. Meanwhile another core holding `IPC_TABLES` may hold a
`&mut Thread` for the same thread: a revocation sweep's `iter_mut`, or an IPC writing its mailbox.
A `&mut` asserts that nothing else touches any byte it covers. A lock inside the struct would be
read by one core under another core's exclusive reference. No reference to the `Thread` covers the
bytes past its end, which is the same provenance rule the FP register file follows. Its own lock at
`rank::CAPABILITY_TABLE` keeps the running thread's lookups off the global lock. The order is
written at the rank in `kernel/src/sync.rs`.

## Fields that are there for a measured reason

- `last_cpu` records the core a thread last ran on, to count migrations. The scheduler's own
  `PlaceRemote` trace event cannot. A rendezvous wake makes its peer ready on the waker's core, so
  it is local by construction. On 2026-09-01 (UTC) a four-core IPC soak doing 65,000 round trips a
  second reported `remote` frozen at 23 while threads moved between cores continuously. The write
  sits in `schedule()`'s switch. Shipping it cost 5.7% of `ipc_fastpath`'s footprint on aarch64
  (5,788 to 6,120 bytes), over the bound `script/fastpath-footprint` holds. riscv64 and x86_64 grew
  4.7% and 4.6%. So it exists only in a `soak_test` build.
- `placement` records where a thread was placed when it started, written once at creation. A
  supervisor in the kernel gets the same fact as a return value from the spawn. A userspace
  supervisor did not do the spawning, so the field gives it a path out through a survey. One store
  per creation is free where a store per switch was not.
- `port_range_grant` makes the TSS I/O bitmap lazy. An always-write cost about 2,682 ns a switch.
  Rewriting only when the incoming thread's grant differs costs one compare on nearly every switch,
  since one process ever holds a port capability.
- `cycle_counter_grant` costs `sched::schedule` 192 bytes for an instrument nothing can request yet,
  so the whole mechanism is a measurement build, like the soak.
- `own_token` holds the thread's queue token while no queue does. It deviates from ruling C's
  letter, which put the running thread's token in a per-core slot. Round 10's report of milestone
  139 (drive the unsafe count down) puts that deviation to calef.
