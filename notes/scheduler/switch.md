# The switch: `schedule`, its decision, and the reaper's far side

An appendix to [notes/scheduler.md](../scheduler.md). It carries the design behind
`kernel/src/sched/switch.rs` and `kernel/src/sched/switch_decision.rs`, which their doc comments
link here, and what the tests in `kernel/src/sched/tests.rs` are built against. The rendezvous
paths that call into the switch are in [rendezvous.md](rendezvous.md).

## Thread 0, and what `init` does first

`sched::init` adopts the context it is already running in as thread 0. The boot thread has no stack
of its own and no saved context; the first switch away from it writes one, because a thread's
context is written by the act of leaving it.

Before any thread exists, `init` shuts the floating-point unit on its core (milestone 447 (a
thread's vector registers are its own)). `crate::fp` marks a thread `live` when it takes the
first-use trap, and a core that starts with the unit open never produces that trap, so every thread
on it would stay `live == false` and two of them would share one register file. The shut lives in
`sched::init` rather than `arch::init` because RISC-V's boot hart never calls `arch::init`: `main`'s
RISC-V path installs `stvec` with `arch::exceptions::init()` and goes straight to `sched::init`, and
OpenSBI hands over a hart with `sstatus.FS` already set. The machine statistics page is published at
the same point for the same reason: threads begin here, so the counters that watch them begin here,
on all three architectures through one function each boot path calls.

The tables are installed empty first, and the boot thread is then inserted into them. Built as a
local and moved into the `Option`, an unoptimised build carries the thread table twice on the boot
stack, so each slot added to `MAX_THREADS` cost about 80 bytes of boot stack rather than the 40 the
table itself is. Measured 2026-08-27 (UTC): at 128 slots `init`'s frame was 43,952 bytes, the
deepest in the kernel, and the suite's boot-stack high-water stood at 54,336 of 65,504 bytes (82%);
a probe raise to 256 slots took it to 64,640 (98%) and failed `stack::report_high_water`'s gate.
Assigning `EMPTY_TABLES`, a `const`, gives the compiler a `.rodata` aggregate to copy from. The
first name a fresh table mints is slot 0, generation 0, so "the boot thread is tid 0" is a property
of the table.

The run queue and inbox are intrusive, so a push from the timer interrupt is two pointer writes and
cannot allocate; nothing is reserved at boot.

The idle thread is built in place on its own TCB page, as `spawn_on` builds every kernel thread. A
by-value `Thread::spawn` held three `Thread`s in one unoptimised frame, 4,368 bytes once the
capability table reached 32 slots, over the 4,096-byte guard page (2026-09-27 UTC). The idle thread
is kept off every ready queue (`cpu::PerCpu::idle`): the pick falls back to it only when nothing
else is runnable, so it never takes a turn from real work.

## Queue tokens and the queue discipline

A thread's queue token (`Thread::own_token`) is on its own TCB exactly when the thread is on no
queue. `take_token` moves it onto a queue, and `hold_token` gives it back when a queue pops or
removes the thread, so every pop ends with the thread holding its token again before anything
decides what happens to it. A missing token means a thread is about to go on a second queue, and
`missing_token` panics in every build, because the alternative is a corrupted intrusive list. It is
one cold function for every site because an `expect` sets up a message and a location inline, and
seven of them were most of what the token added to `script/fastpath-footprint`'s closures.

A raw queue pointer is valid while the thread is queued by the queue discipline: a thread on a run
queue or inbox is `Ready`, a thread on a rendezvous wait queue is `Blocked`, the reaper frees only
`Finished` threads, no thread is two of those at once, and the thread table pins the TCB page.
`thread_control_block_ptr` returns `NonNull` because the pointer is derived from a `&mut Thread`,
which takes null out of the intrusive queue's safety contract (one of the two things CodeQL's
`rust/access-invalid-pointer` alerts pointed at, milestone 45 (triage the CodeQL alerts)). What no
available type can carry for an intrusive structure is that the pointee outlives its time on the
queue; that stays the caller's rule.

## `schedule`

`schedule` is reached from a voluntary `yield_now` and from the timer interrupt's tail, and the two
paths are identical from its first line. Its rules, in the order the code meets them:

1. **Not on an interrupt stack.** A switch parks the running `sp` in the outgoing thread's
   `Context` and resumes it there arbitrarily later. A per-core interrupt stack cannot promise those
   bytes stay the thread's, so a thread parked there would resume on whatever the next interrupt
   wrote. The interrupt path defers its switch to `preempt_if_needed`, one frame outside the
   trampoline. The runtime check is a `debug_assert!`, because it costs an `sp` read and a scan of
   `MAX_CPUS` spans on the hottest path; `script/stack-depth-check` proves the same property
   statically in CI, and the assertion covers the edges a call graph cannot see.
2. **Interrupts masked across the decision and the switch**, so the timer cannot choose again
   between "I chose a thread" and "I am running it". `was_enabled` is a local on the thread's own
   stack, so when a later switch returns to this frame it is still there.
3. **One exit from the locked block.** Every path leaves the labeled block through the same point,
   so the guard drops before interrupts are restored, once, after it. Restoring them inside the
   block while `IPC_TABLES` was held opened a one-instruction window in which a timer could
   re-enter `schedule` and take a lock its core already held.
4. **Orders before the pick.** A thread `DESTROY` marked killed never runs again, so it becomes a
   `Finished` corpse at the top of the decision, where every path below reaps it, including "keep
   current". A killed runaway can be the only thread on its core, and a requeue-time check is then
   never reached and the runaway spins forever. A suspension order (§300 (the paused state, a TCB
   verb, silent)) parks a running thread before the pick, so the pick neither requeues nor keeps
   it. The test is two loads and a compare; the bodies (`carry_out_orders`,
   `finish_killed_current`) are cold and out of line, since a teardown's bytes do not belong in the
   hottest function.
5. **Migration is recorded at the switch.** Every path to a CPU passes through here, whatever moved
   the thread (a wake onto the waker's core, a steal, a placement, a spawn), so `Thread::last_cpu`
   is written here. Only under `soak_test`: shipped everywhere, it put `ipc_fastpath` over
   milestone 132 (the fast path's footprint)'s bound.
6. **The lock is released before `switch_to`.** Held across it, it would belong to a thread that is
   not running, and the next thread to want it would spin forever.
7. **The incoming address space first.** `TTBR0_EL1` is one register shared by everybody, and a
   thread resuming at EL0 in the previous thread's low half runs a stranger's code. The install is a
   no-op, with no TLB flush, when the root is already right, which is every switch between kernel
   threads. Its liveness argument cannot be a borrow, because the lock is released on purpose.
8. **The register file beside the switch, not inside it.** `switch_to` saves what a function call
   may destroy; `crate::fp::hand_over` moves what a thread owns, and has the four cases.
9. **The far side.** The outgoing thread is still standing on its stack until `switch_to` returns
   in the incoming thread, so the incoming one finishes it there: `finish_switch`.

`schedule` also counts the switch through the per-CPU block it already writes (`PerCpu::switches`,
`vmstat`'s `cs`, copied to the statistics page by the tick), so the switch pays one add rather than
a walk to the page.

## The pick and the operands

`choose_next` requeues only a thread still `Running`. A thread that marked itself `Blocked`,
`Finished` or `Dead` must not go back: `schedule` can run from the timer interrupt while a thread is
mid-way through blocking itself, and that one comparison is what keeps it from undoing the block.
`preempt` marks the outgoing thread `Ready` and leaves `on_cpu` set, so it is on a queue and still
on this core until `finish_switch` runs. That is the one legal overlap, and it is why only this core
may pop its own queue (the steal-versus-switch-out rule in `crates/thread_wake_handshake`). A popped
thread carrying a suspension order is parked rather than run, because the pop is the one place a
queued thread reaches a CPU. An empty queue with a blocked current is not a deadlock: the pick falls
back to the idle thread, which waits for the interrupt that will wake someone.

The pop precedes the requeue, so a legal schedule never hands back `current`. If one did, switching
into it would restore a context this thread has already resumed and consumed, and execution would
return to its previous switch-out point on a reused stack. `heal_self_pop` panics in a debug build
and keeps the thread running in a board build, the only coherent state. The undelivered-wake gate's
appendix, [undelivered-wake-gate.md](undelivered-wake-gate.md), has where this guard came from.

`switch_operands` reads everything the switch needs while the lock is held. A kernel thread gets
the empty reserved root, so every low address faults. The incoming thread's current-CPU page is
published here for `last_cpu`'s reason, and on the core that will run it, so writer and reader are
one hardware thread and the ordering argument is `crates/current_cpu_contract`'s short one. It reads
through the TCB page pointer rather than `get_mut`, because each thread's FP register file lives in
the same page past the end of the struct, and a `&mut Thread`'s provenance stops at the struct.

Both functions read the current thread's state themselves and are `#[inline(always)]`. Passing the
state in made LLVM duplicate the run-queue pop per way of producing the `Option<State>` (76 bytes of
`schedule` on aarch64), and with both left to the compiler the icount gate's debug build saw
`yield_switch` and `ctx_switch` rise 1.0 to 1.2% on all three ISAs (both 2026-10-12 UTC). The
release build inlined them either way.

## Per-thread grants, `#[cfg]`-gated rather than carried

Two per-core registers say what the running thread was granted: the cycle counter's user access
(`PMUSERENR_EL0`, riscv64's `scounteren.CY`; milestone 229 (build the cycle-counter grant), §139
(who may read the cycle counter) option 4) and, on x86_64, the TSS I/O bitmap (§152 (the port-range
capability)). Both reads and both installs are `#[cfg]`-gated at the switch site, and neither rides
the switch tuple. The debug build the icount gate measures does not fold a constant away: a
constant-`false` tuple element and an empty install cost `yield_switch` +35.6 and `ctx_switch` +36.0
ticks per switch on aarch64, and a constant `None` port grant cost 13% on the same two rows. With
the cycle-counter feature off, the counter keeps the closed value boot wrote for the life of the
kernel, and that boot write is not behind the feature: closing what is claimed closed is right
whether or not anything can be granted. `kernel/Cargo.toml`'s `cycle_counter_grant` block has the
rest of the measurement. The cycle-counter install needs no `unsafe`, because the register names no
memory: a wrong value opens or closes a counter and hands no thread a stranger's pages.

The port grant is installed under `IPC_TABLES`, not after it drops. It is the one per-core fact a
revocation must take back from a core it is not running on, and `delete_port_range_caps_impl` does
that by NMI while holding the lock. Installing outside the lock left two windows the broadcast could
not close: a core could install a grant the sweep had already cleared, and the NMI could land
mid-install. Both close when every writer of a bitmap holds the lock, at the cost of one compare
inside the critical section. No ring-3 `in` or `out` can run between the install and the switch,
because the core is in the kernel with interrupts masked throughout.

## `finish_switch` and the reaper

The reaper has two halves. `schedule` records the outgoing thread in its core's `switched_from`
before the switch; `finish_switch` runs on the incoming thread after it, when the outgoing thread is
provably off its stack. Dropping a `Thread` unmaps its stack and frees its address space, which must
not happen while a core still stands on it. `finish_switch` is called from two places because a
thread resumes two ways: from `schedule` after `switch_to` returns, and from `thread_entry`, which a
new thread enters without passing `schedule`'s post-switch point. Both run on the core that recorded
`switched_from`. What the switch-out makes legal is `thread_wake_handshake`'s verdict, which loom
searches (notes/interleaving.md): reap, complete a wake deferred mid-switch-out (the waker did not
queue it, so its token is still on the thread), or clear `on_cpu`.

The `Reap` arm is `#[cold]` and out of line because only a finished thread is reaped and no IPC
finishes one; inline, its setup sat in `finish_switch`, which is on both IPC closures, and
`script/fastpath-footprint` can exclude only the callees an arm reaches, not the arm's own bytes.
`reap_switched_out` runs two short critical sections with the expensive part between them:

1. Under the caller's guard, take the kernel stack and the address space out and mark the thread
   `being_reaped`. It stays in the table, `Finished`, so its name resolves and region teardown
   still refuses it.
2. With no lock held, free the stack, then the address space. The stack is six page unmaps, each
   with its TLB obligation: an SBI remote fence on riscv64 that interrupts every other hart and
   waits, an NMI shootdown round on x86_64. Under `IPC_TABLES` every other core would wait behind it
   ([notes/job-mix/null-syscall-under-load.md](../job-mix/null-syscall-under-load.md)). The address
   space could never be dropped under the lock: its teardown is `memory_region::destroy`, whose §13
   (capability revocation) revocation sweep takes `IPC_TABLES` itself. The space leaves the
   address-space registry here, by the name in the thread's copy; the region sweep may have taken it
   already, and `Table::remove` hands it to one of the two (§249 (a running address space stays
   nameable)).
3. Under the lock again, remove the thread. Only now does it stop occupying its region.

Everything the thread owned is gone before step 3, and region teardown (`reclaim_region`) relies on
that order: its `being_reaped` refusal holds the region until step 3, and an owner whose `DESTROY`
succeeds may reuse the memory at once. Nothing else removes a `Finished` thread between steps 1 and
3, because the only other remover is region teardown and `region_reap_verdict` refuses a thread
being reaped. Afterwards, `process::reap_corpses` reaps the members a process's end finished where
they were blocked.

## The floating-point trap takes the lock

`mark_current_fp_live` takes `IPC_TABLES` inside a trap handler, which is ordinary here: every
syscall does it from the same place, and the argument is the same. The trap came from a running
thread, so its core cannot already hold the lock. Kernel code executing an FP instruction while
holding it would break that, and the kernel is built `softfloat` and executes none.

## What the tests in `sched/tests.rs` are built against

- **The preemption test** (`a_thread_that_never_yields_is_preempted_anyway`) is DECISIONS §5
  (preemptive threads with real stacks)'s claim executed. Under async/await, Go before 1.14 or any
  cooperative runtime, a tight loop with no yield, no syscall and no call keeps its CPU, and only a
  timer interrupt landing between two of its instructions can take it back. The spinner and the
  polite thread are pinned to the test's own core, since a spinner on another idle core would never
  need preempting. The spinner reaching a CPU is a precondition and is waited on before the polite
  thread exists; sampled afterwards, `STOP` could beat its first turn (CI on `sifive-u54`,
  2026-08-04 UTC, with no kernel change), and on the physical core under HVF the polite thread could
  run first. The budget is delivered ticks, not wall-clock, because preemption opportunities are
  what the claim counts and a descheduled emulator delivers fewer, never more.
- **The reaper test** waits on each thread's own `ThreadId` and asserts stack reuse directly,
  because a frame count is near-blind to the stack-address leak in its name: [stack reuse and proxy
  detectors](../load-sensitive-assertions/stack-reuse-and-proxy-detectors.md). Every wait in this
  file is bounded by a clock or a tick budget rather than a yield count, for
  [notes/load-sensitive-assertions.md](../load-sensitive-assertions.md)'s reason.
- **The guard-page tests** pin the arithmetic a fatal-fault report rests on.
  [notes/stack/kernel-stack-freed-under-its-owner.md](../stack/kernel-stack-freed-under-its-owner.md)
  has the faults behind them, including why a guard-page fault's address carries no information and
  its slot number does.
- **The interrupt delivery tests** raise a real interrupt per ISA; why RISC-V uses the console
  UART's transmit-empty line is in [notes/interrupts.md](../interrupts.md).

## BUGS

- A corpse whose TCB is outside region R and whose root is inside it still has a narrow window. If
  the reaper takes the space in step 2 and a `DESTROY(R)` on another core runs wholly before the
  drop finishes, the sweep finds no entry, R comes back, and the drop's `revoke::forget_root` can
  land on a root whose page has been handed out since. The sweep taking the space first is the
  common order and is safe. Milestone 765 (a destroyed region cannot free the root a running thread
  walks) is the natural place to count a space that is still being dropped.
