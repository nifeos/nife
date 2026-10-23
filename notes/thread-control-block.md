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
table stores the pointer, and the generational name is what everything else carries. A pointer may
move between cores. The `Thread` it names is touched only under `IPC_TABLES`, and its queue link
only under the intrusive discipline (notes/intrusive-queues.md).

### One page, three things

The `Thread` sits at the start of its page. The FP register file comes after it, and the capability
table after that. `get` and `get_mut` narrow the page pointer to a reference over
`size_of::<Thread>()` bytes. So the register file and the table are reached from the stored page
pointer (`Threads::pointer`, `thread::capability_table_of`), never from a `&Thread`. A pointer
derived from the reference would reach outside its own provenance.

The two inserts initialize the register file and the table in place. The register file's `live` flag
must be written before anything reads it (`thread::init_fp_state`). That is a fact about a place, so
it is not in a `Thread` constructor. `insert_at_in_place` hands `build` the destination rather than
taking a returned `Thread`, because a `Thread` is large and a debug build copies it at every hop.

### Queue tokens

Every thread's queue token is minted once, at insert. `mint_token` is the only place in the kernel
a token is made. From there it moves onto a queue at a push, and back onto `Thread::own_token` at a
pop or removal (`hold_token`). It goes nowhere else. `mint_token` discharges `Unqueued::new`'s three
obligations:

- The `Thread` lives until `Threads::remove`. That is reached only for a `Finished` thread off its
  core, which is on no queue because a running thread holds its own token, or for one that
  `finish_blocked_resident` has unlinked.
- The name is a moment old, so no other token for the page exists.
- Every queue holding a `Thread` is behind `IPC_TABLES` or an inbox lock.

### Removal order

`remove` first reads whether the page came from `kmem`. Then it drops the `Thread` in place, which
frees its kernel stack, address space and quota token. Then it kills the name. Only then does a
`kmem` page go home. A user TCB's page belongs to its region and is reclaimed when the region is
destroyed.

### CPU time, per slot

`CPU_TICKS` holds each thread's scheduled ticks in an array beside the table, keyed by slot.
`abi::survey::record::CPU_TIME` reads it (§150 (how does a thread's CPU time reach userspace?)).

It is not a `u64` on `Thread`. The increment happens in `on_tick`, in interrupt context, where a core
knows its running thread only as a tid. Reaching the `Thread` would take `IPC_TABLES`, a lock wait
at every tick on every core. `try_lock` would drop samples exactly when the machine is busiest. The
slot is the tid's low 32 bits, so the array needs no lock, no lookup and no ordering beyond relaxed.
`cpu::NO_TID` (`u64::MAX`) falls out of range at no cost. The array is 2 KiB of `.bss`.

It is not on `cpu::PerCpu` either. The counter is per thread. Growing `PerCpu` from 128 to 136 bytes
cost riscv64's IPC fastpath 5.4%, because `PERCPU[id]` stopped indexing with a shift, and
`kernel/src/cpu.rs` const-asserts that size.

A core's tick increments only the slot of the thread it runs, so the write is uncontended. A survey
reading a slot another core is incrementing gets a value that was true a moment ago. It is never
torn, since the unit is an aligned `u64`. The counter is zeroed when a slot is filled, not emptied,
so a corpse keeps the time it earned until its supervisor reaps it.

### The peak

`PEAK_THREADS` is the most threads alive at once on this boot, which makes `MAX_THREADS` a measured
number. By the time a spawn is refused, the table is full and every earlier number is gone. It is one
high-water mark rather than a per-test ledger. What fills the table is the services earlier tests
left running on purpose (notes/frames.md's "held" list), not one test's spend. Both inserts update
it under `IPC_TABLES` with `fetch_max`.

### The running thread, per core

`kernel/src/sched/current.rs` keeps two things per core. The running thread's id is in
`cpu::PerCpu::current`. A pointer to its capability table is in `CURRENT_CAPABILITIES`, so a
capability lookup takes the table's own lock and never `IPC_TABLES`. `set_current` is the only
writer of either, on the owning core. So a lookup never finds one thread's name beside another's
table.

The id is read through `arch::read_current_thread`, because the read must be indivisible from a
preemptible caller. A kernel thread migrated between finding its per-CPU block and reading the field
would get another thread's name. `adopt_address_space` would then bind a new space to that thread.
`smp::tests::a_migrated_kernel_thread_reads_its_own_name` fails on all three architectures without
it.

`CURRENT_CAPABILITIES` is its own array, because `PerCpu` is held to 128 bytes. Each entry fills a
cache line. Every core writes its entry on every switch and reads it on every syscall, and a shared
line would trade the lock for line traffic between the same cores
(notes/job-mix/null-syscall-under-load.md's BUGS has that shape).

The pointer is valid without a lock, because it names the page of the thread running on this core,
and a running thread's page is not recycled. `Threads::remove` is reached only from
`reap_switched_out`, for a thread its successor has switched off, and from `reap_region_objects`.
That one's verdict refuses a `Ready` or `Running` thread and one a core still stands on. `schedule`
replaces the pointer before the switch that makes the old thread reapable.

`current_thread_id` and `set_current` are on every IPC and must land in `.text.hot` beside their
callers. Whether LLVM inlines them moves with the codegen unit and the build host. So they are
forced inline where that does not cost the band: `current_thread_id` on riscv64, and `set_current`
on riscv64 and `x86_64`. On aarch64, forcing either grows `syscall_entry` past its band. There
`set_current` carries a `.text.hot` section instead. Inlined, the section is unused; out of line, the
copy lands beside its caller.
