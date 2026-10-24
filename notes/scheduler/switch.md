# The switch: `schedule`, its decision, and the reaper's far side

*Name: provisional. After the source file it explains.*

An appendix to [notes/scheduler.md](../scheduler.md). It carries the design behind
`kernel/src/sched/switch.rs` and `kernel/src/sched/switch_decision.rs`, whose doc comments link
here. It also says what the tests in `kernel/src/sched/tests.rs` are built against. The rendezvous
paths that call into the switch are in [rendezvous.md](rendezvous.md).

## Thread 0, and what `init` does first

`sched::init` adopts the context it is already running in as thread 0. The boot thread has no stack
of its own and no saved context. The first switch away from it writes one. A thread's context is
written by the act of leaving it.

Before any thread exists, `init` shuts the floating-point unit on its core (milestone 447 (a
thread's vector registers are its own)). `crate::fp` marks a thread `live` when it takes the
first-use trap. A core that starts with the unit open never takes that trap. Every thread on it
would then stay `live == false`, and two of them would share one register file. The shut lives in
`sched::init` rather than `arch::init`, because RISC-V's boot hart never calls `arch::init`. `main`'s
RISC-V path installs `stvec` itself and goes straight to `sched::init`. OpenSBI hands over a hart
with `sstatus.FS` already set. The machine statistics page is published at the same point, for the
same reason: threads begin here, so the counters that watch them begin here too.

The tables are installed empty first, and the boot thread is then inserted into them. Built as a
local and moved into the `Option`, the thread table sits twice on the boot stack in an unoptimised
build. Each slot added to `MAX_THREADS` then cost about 80 bytes of boot stack, not 40. Measured
2026-08-27 (UTC): at 128 slots, `init`'s frame was 43,952 bytes, the deepest in the kernel. The
suite's boot-stack high-water stood at 54,336 of 65,504 bytes (82%). A probe raise to 256 slots took
it to 64,640 (98%) and failed `stack::report_high_water`'s gate. Assigning `EMPTY_TABLES`, a
`const`, gives the compiler a `.rodata` aggregate to copy from instead. The first name a fresh table
mints is slot 0, generation 0, so the boot thread is tid 0 by construction.

The run queue and inbox are intrusive. A push from the timer interrupt is two pointer writes and
cannot allocate, so nothing is reserved at boot.

The idle thread is built in place on its own TCB page, as `spawn_on` builds every kernel thread. A
by-value `Thread::spawn` held three `Thread`s in one unoptimised frame. That was 4,368 bytes once the
capability table reached 32 slots, over the 4,096-byte guard page (2026-09-27 UTC). The idle thread
stays off every ready queue (`cpu::PerCpu::idle`). The pick falls back to it only when nothing else
can run, so it never takes a turn from real work.

## Queue tokens and the queue discipline

A thread's queue token (`Thread::own_token`) is on its own TCB exactly when the thread is on no
queue. `take_token` moves it onto a queue. `hold_token` gives it back when a queue pops the thread.
So every pop ends with the thread holding its token again, before anything decides its fate. A
missing token means a thread is about to go on a second queue. `missing_token` panics in every
build, because the alternative is a corrupted intrusive list. It is one cold function for every
site. An `expect` would set up a message and a location inline, and seven of them were most of what
the token added to `script/fastpath-footprint`'s closures.

A raw queue pointer stays valid while its thread is queued, by the queue discipline. A thread on a
run queue or inbox is `Ready`. A thread on a rendezvous wait queue is `Blocked`. The reaper frees
only `Finished` threads, and no thread is two of these at once. The thread table pins the TCB page.
`thread_control_block_ptr` returns `NonNull`, because the pointer comes from a `&mut Thread`. That
takes null out of the queue's safety contract, one of the two things CodeQL's
`rust/access-invalid-pointer` alerts pointed at (milestone 45 (triage the CodeQL alerts)). No
available type can say that the pointee outlives its time on the queue. That stays the caller's rule.

## `schedule`

`schedule` is reached from a voluntary `yield_now` and from the timer interrupt's tail. The two
paths are identical from its first line. Its rules, in the order the code meets them:

1. **Not on an interrupt stack.** A switch parks the running `sp` in the outgoing thread's
   `Context` and resumes it there later. A per-core interrupt stack cannot promise those bytes stay
   the thread's. The interrupt path defers its switch to `preempt_if_needed`, one frame outside the
   trampoline. The runtime check is a `debug_assert!`, since it scans `MAX_CPUS` spans on the
   hottest path. `script/stack-depth-check` proves the same property statically in CI. The
   assertion covers the edges a call graph cannot see.
2. **Interrupts masked across the decision and the switch.** Otherwise the timer could choose again
   between choosing a thread and running it. `was_enabled` is a local on the thread's own stack, so
   it is still there when a later switch returns to this frame.
3. **One exit from the locked block.** Every path leaves the labeled block through the same point.
   The guard drops first, and interrupts are restored once, after it. Restoring them inside the
   block opened a one-instruction window. A timer could then re-enter `schedule` and take a lock its
   core already held.
4. **Orders before the pick.** A thread `DESTROY` marked killed never runs again. It becomes a
   `Finished` corpse at the top of the decision, where every path below reaps it, "keep current"
   included. A killed runaway can be the only thread on its core, and a requeue-time check would
   never reach it. A suspension order (§300 (the paused state, a TCB verb, silent)) parks a running
   thread before the pick. The test is two loads and a compare. The bodies, `carry_out_orders` and
   `finish_killed_current`, are cold and out of line.
5. **Migration is recorded at the switch.** Every path to a CPU passes through here, whatever moved
   the thread. So `Thread::last_cpu` is written here, and only under `soak_test`. Shipped everywhere,
   it put `ipc_fastpath` over milestone 132 (the fast path's footprint)'s bound.
6. **The lock is released before `switch_to`.** Held across it, the lock would belong to a thread
   that is not running. The next thread to want it would spin forever.
7. **The incoming address space first.** `TTBR0_EL1` is one register shared by everybody. A thread
   resuming at EL0 in the previous thread's low half runs a stranger's code. The install is a no-op
   when the root is already right, as it is between kernel threads. Its liveness argument cannot be
   a borrow, because the lock is released on purpose.
8. **The register file beside the switch, not inside it.** `switch_to` saves what a function call
   may destroy. `crate::fp::hand_over` moves what a thread owns, and has the four cases.
9. **The far side.** The outgoing thread stands on its stack until `switch_to` returns in the
   incoming thread. So the incoming one finishes it there, in `finish_switch`.

`schedule` also counts the switch through the per-CPU block it already writes (`PerCpu::switches`).
That is `vmstat`'s `cs`, copied to the statistics page by the tick. The switch pays one add rather
than a walk to the page.

## The pick and the operands

`choose_next` requeues only a thread still `Running`. A thread that marked itself `Blocked`,
`Finished` or `Dead` must not go back. `schedule` can run from the timer interrupt while a thread is
mid-way through blocking itself. That one comparison keeps it from undoing the block. `preempt`
marks the outgoing thread `Ready` and leaves `on_cpu` set. It is then on a queue and still on this
core until `finish_switch` runs. That is the one legal overlap, and it is why only this core may pop
its own queue (the steal-versus-switch-out rule in `crates/thread_wake_handshake`). A popped thread
with a suspension order is parked rather than run, because the pop is where a queued thread reaches
a CPU. An empty queue with a blocked current is not a deadlock. The pick falls back to the idle
thread, which waits for the interrupt that will wake someone.

The pop precedes the requeue, so a legal schedule never hands back `current`. If one did, the switch
would restore a context this thread has already consumed. Execution would return to its previous
switch-out point on a reused stack. `heal_self_pop` panics in a debug build. A board build keeps the
thread running, the only coherent state. [undelivered-wake-gate.md](undelivered-wake-gate.md) has
where this guard came from.

`switch_operands` reads everything the switch needs while the lock is held. A kernel thread gets the
empty reserved root, so every low address faults. The incoming thread's current-CPU page is
published here, for `last_cpu`'s reason. It is published on the core that will run the thread, so
writer and reader are one hardware thread (`crates/current_cpu_contract`). It reads through the TCB
page pointer rather than `get_mut`. Each thread's FP register file lives past the end of the struct
in the same page, and a `&mut Thread`'s provenance stops at the struct.

Both functions read the current thread's state themselves and are `#[inline(always)]`. Passing the
state in made LLVM duplicate the run-queue pop, 76 bytes of `schedule` on aarch64. With both left to
the compiler, the icount gate's debug build saw `yield_switch` and `ctx_switch` rise 1.0 to 1.2% on
all three ISAs. Both were measured 2026-10-12 (UTC). The release build inlined them either way.

## Per-thread grants, `#[cfg]`-gated rather than carried

Two per-core registers say what the running thread was granted. One is the cycle counter's user
access: `PMUSERENR_EL0`, or riscv64's `scounteren.CY` (milestone 229 (build the cycle-counter
grant), §139 (who may read the cycle counter) option 4). The other, on x86_64, is the TSS I/O bitmap
(§152 (the port-range capability)). Both reads and both installs are `#[cfg]`-gated at the switch
site. Neither rides the switch tuple.

The debug build the icount gate measures does not fold a constant away. A constant-`false` tuple
element and an empty install cost `yield_switch` +35.6 and `ctx_switch` +36.0 ticks per switch on
aarch64. A constant `None` port grant cost 13% on the same two rows. With the feature off, the
counter keeps the closed value boot wrote. That boot write is not behind the feature, because
closing what is claimed closed is right either way. `kernel/Cargo.toml`'s `cycle_counter_grant`
block has the rest of the measurement. The install needs no `unsafe`, since the register names no
memory. A wrong value opens or closes a counter, and hands no thread a stranger's pages.

The port grant is installed under `IPC_TABLES`, not after it drops. It is the one per-core fact a
revocation must take back from a core it is not running on. `delete_port_range_caps_impl` does that
by NMI while holding the lock. Installing outside the lock left two windows the broadcast could not
close. A core could install a grant the sweep had already cleared, and the NMI could land
mid-install. Both close when every writer of a bitmap holds the lock. The cost is one compare inside
the critical section. No ring-3 `in` or `out` can run before the switch, because interrupts stay
masked.

## `finish_switch` and the reaper

The reaper has two halves. `schedule` records the outgoing thread in its core's `switched_from`
before the switch. `finish_switch` runs on the incoming thread after it, when the outgoing thread is
off its stack. Dropping a `Thread` unmaps its stack and frees its address space. That must not
happen while a core still stands on it.

`finish_switch` is called from two places, because a thread resumes two ways. One is `schedule`,
after `switch_to` returns. The other is `thread_entry`, which a new thread enters without passing
`schedule`'s post-switch point. Both run on the core that recorded `switched_from`. What the
switch-out makes legal is `thread_wake_handshake`'s verdict, which loom searches
(notes/interleaving.md). It reaps, completes a wake deferred mid-switch-out, or clears `on_cpu`. A
deferred wake's token is still on the thread, because the waker did not queue it.

The `Reap` arm is `#[cold]` and out of line, because no IPC finishes a thread. Inline, its setup sat
in `finish_switch`, which is on both IPC closures. `script/fastpath-footprint` can exclude only the
callees an arm reaches, not the arm's own bytes. `reap_switched_out` runs two short critical
sections with the expensive part between them:

1. Under the caller's guard, take the kernel stack and the address space out, and mark the thread
   `being_reaped`. It stays in the table, `Finished`. Its name resolves, and region teardown still
   refuses it.
2. With no lock held, free the stack, then the address space. The stack is six page unmaps, each
   with its TLB obligation. On riscv64 that is an SBI remote fence that interrupts every other hart
   and waits. On x86_64 it is an NMI shootdown round. Under `IPC_TABLES`, every other core would
   wait behind it ([notes/job-mix/null-syscall-under-load.md](../job-mix/null-syscall-under-load.md)).
   The address space can never be dropped under the lock. Its teardown is `memory_region::destroy`,
   whose §13 (capability revocation) sweep takes `IPC_TABLES` itself. The space leaves the
   address-space registry here, by the name in the thread's copy. The region sweep may have taken it
   already, and `Table::remove` hands it to one of the two (§249 (a running address space stays
   nameable)).
3. Under the lock again, remove the thread. Only now does it stop occupying its region.

Everything the thread owned is gone before step 3. Region teardown (`reclaim_region`) relies on that
order. Its `being_reaped` refusal holds the region until step 3, and an owner whose `DESTROY`
succeeds may reuse the memory at once. Nothing else removes a `Finished` thread between steps 1 and
3. The only other remover is region teardown, and `region_reap_verdict` refuses a thread being
reaped. Afterwards, `process::reap_corpses` reaps the members a process's end finished where they
were blocked.

## The floating-point trap takes the lock

`mark_current_fp_live` takes `IPC_TABLES` inside a trap handler, which is ordinary here. Every
syscall does it from the same place, for the same reason. The trap came from a running thread, so its
core cannot already hold the lock. Kernel code executing an FP instruction under the lock would break
that. The kernel is built `softfloat` and executes none.

## What the tests in `sched/tests.rs` are built against

- The preemption test (`a_thread_that_never_yields_is_preempted_anyway`) executes DECISIONS §5
  (preemptive threads with real stacks). Under any cooperative runtime, a tight loop with no yield,
  no syscall and no call keeps its CPU. Only a timer interrupt between two of its instructions can
  take it back. Both threads are pinned to the test's core, since a spinner on another idle core
  would never need preempting. The spinner reaching a CPU is a precondition, so it is waited on
  before the polite thread exists. Sampled afterwards, `STOP` could beat its first turn. That failed
  CI on `sifive-u54` on 2026-08-04 (UTC) with no kernel change. The budget is delivered ticks, not
  wall-clock. A descheduled emulator delivers fewer preemption opportunities, never more.
- The reaper test waits on each thread's own `ThreadId`. It asserts stack reuse directly, because a
  frame count is near-blind to the leak in its name
  ([stack reuse and proxy detectors](../load-sensitive-assertions/stack-reuse-and-proxy-detectors.md)).
  Every wait in the file is bounded by a clock or a tick budget, never a yield count
  ([notes/load-sensitive-assertions.md](../load-sensitive-assertions.md)).
- The guard-page tests pin the arithmetic a fatal-fault report rests on. The faults behind them are
  in [notes/stack/kernel-stack-freed-under-its-owner.md](../stack/kernel-stack-freed-under-its-owner.md),
  including why a guard-page fault's address carries no information and its slot number does.
- The interrupt delivery tests raise a real interrupt per ISA. Why RISC-V uses the console UART's
  transmit-empty line is in [notes/interrupts.md](../interrupts.md).

## BUGS

- A corpse whose TCB is outside region R and whose root is inside it has a narrow window. Suppose
  the reaper takes the space in step 2, and a `DESTROY(R)` on another core runs wholly before the
  drop finishes. The sweep finds no entry and R comes back. The drop's `revoke::forget_root` can
  then land on a root whose page has been handed out since. The sweep taking the space first is the
  common order, and it is safe. Milestone 765 (a destroyed region cannot free the root a running
  thread walks) is the natural place to count a space that is still being dropped.
