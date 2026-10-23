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

## The thread table (`kernel/src/sched/thread_table.rs`)

`Threads` is a generational table of `ThreadControlBlockPointer`s, one per page-resident TCB. The
table stores the pointer; the generational name is what everything else carries. A pointer may
move between cores because the `Thread` it names is touched only under `IPC_TABLES`, and its queue
link only under the intrusive discipline (notes/intrusive-queues.md).

**One page, three things.** The `Thread` sits at the start of its page, the FP register file past
it, and the capability table past that. `get` and `get_mut` narrow the page pointer to a reference
over `size_of::<Thread>()` bytes, so the register file and the table are reached from the stored
page pointer (`Threads::pointer`, `thread::capability_table_of`), never from a `&Thread`, which
would reach outside its own provenance. The two inserts initialize the register file and the table
in place, because the register file's `live` flag must be written before anything reads it
(`thread::init_fp_state`); that is a fact about a place, so it is not in a `Thread` constructor.
`insert_at_in_place` hands `build` the destination rather than returning a `Thread`, because a
`Thread` is large and a debug build copies it at every hop.

**Every thread's queue token is minted once, at insert.** `mint_token` is the only place in the
kernel a token is made. From there it moves onto a queue at a push and back onto
`Thread::own_token` at a pop or removal (`hold_token`), and nowhere else. `Unqueued::new`'s three
obligations are discharged there: the `Thread` lives until `Threads::remove`, which is reached only
for a `Finished` thread off its core (on no queue, since a running thread holds its own token) or for
one `finish_blocked_resident` has unlinked; the name is a moment old, so no other token for the page
exists; and every queue holding a `Thread` is behind `IPC_TABLES` or an inbox lock.

**Removal order.** `remove` reads whether the page came from `kmem` before dropping the `Thread`,
drops it in place (its kernel stack, address space and quota token), kills the name, and only then
recycles a `kmem` page. A user TCB's page belongs to its region and is reclaimed only when the region
is destroyed.

### CPU time, per slot

`CPU_TICKS` holds each thread's scheduled ticks in an array beside the table, keyed by slot, which
`abi::survey::record::CPU_TIME` reads (§150 (how a thread's CPU time reaches userspace)). It is not a
`u64` on `Thread` because the increment happens in `on_tick`, in interrupt context, where a core
knows its running thread only as a tid. Reaching the `Thread` would take `IPC_TABLES`, a lock wait at
every tick on every core; `try_lock` would drop samples exactly when the machine is busiest. The
slot is the tid's low 32 bits, so the array needs no lock, no lookup and no ordering beyond relaxed,
and `cpu::NO_TID` (`u64::MAX`) falls out of range at no cost. It is 2 KiB of `.bss` at `MAX_THREADS`.

It is not on `cpu::PerCpu` either. The counter is per thread, and growing `PerCpu` from 128 to 136
bytes cost riscv64's IPC fastpath 5.4%, because `PERCPU[id]` stopped indexing with a shift;
`kernel/src/cpu.rs` const-asserts that size.

The race: a core's tick increments only the slot of the thread it runs, so the write is
uncontended. A survey reading a slot another core is incrementing gets a value that was true a moment
ago, never a torn one, since the unit is an aligned `u64`. The counter is zeroed when a slot is
filled, not emptied, so a corpse keeps the time it earned until its supervisor reaps it.

### The peak

`PEAK_THREADS` is the most threads alive at once on this boot, so `MAX_THREADS` is a measured number:
by the time a spawn is refused the table is full and every earlier number is gone. It is one
high-water mark rather than a per-test ledger, because what fills the table is the services earlier
tests left running on purpose (notes/frames.md's "held" list), not one test's spend. Both inserts
update it under `IPC_TABLES` with `fetch_max`.
