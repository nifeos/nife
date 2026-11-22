# Suspending and resuming a thread

*Name: provisional (lane/48-tcb-suspend-resume, 2026-10-18 UTC).*

[DECISIONS §300 (the paused state, a TCB verb, silent)](../design/decisions/0300-tcb-suspend-resume-the-paused-state.md)
rules what `ThreadControlBlock::SUSPEND` and `RESUME` are: two methods on the TCB capability,
gated on `WRITE`, telling nobody, with `RESUME` continuing at the saved PC. This note records how
the kernel carries that out and the choices the ruling left to the builder. The code is
[`kernel/src/sched/suspension.rs`](../kernel/src/sched/suspension.rs); the contract a caller reads
is on the constants in [`crates/moho_abi/src/thread_control_block.rs`](../crates/moho_abi/src/thread_control_block.rs).

## What a suspended thread is

A suspended thread is `Blocked`, waiting on `Wait::Resume`, on no queue, holding its own queue
token. There is no new run state.

A `CALL` caller waiting for its reply already has this shape: blocked, on no queue, woken by one
named party. Reusing it means every path that ends a blocked thread in place ends a suspended one
with no new case. That covers a region's `DESTROY`, a process's end, and a killed thread caught
mid-park. The handshake's undelivered-wake gate refuses any wake that delivered nothing, so a stray
wake aimed at a suspended thread is refused the same way one aimed at a parked receiver is. Only
`RESUME` serves the wait. A seventh `RunState` would have needed its own arm at each of those
sites, and a site that forgot one would leak a thread that can never be reaped.

The cost: a survey (`moho_abi::survey`) reports a suspended thread as `BLOCKED`. The survey's codes are a
wire two programs agree on, and adding `STOPPED` is a change to that wire with no consumer yet.
The `BUGS` entry below carries it.

## The order and its effect are two steps

`SUSPEND` sets `suspend_requested` on the thread. The thread parks where it next meets the
scheduler, and `schedule` is the only road onto a CPU (`Handshake::switch_in` has one caller), so
it can be caught in two places:

- Leaving a core: `schedule` parks a `Running` current that carries the order, before the
  pick. The pick then neither requeues it nor keeps it.
- About to be switched onto one: The pick parks a popped thread that carries the order and pops
  again. Whatever queued the thread (an IPC wake, a deferred wake completed in `finish_switch`, a
  load-aware wake, a steal, `START`), it ends here, so no queue or placement path needs its own
  check.

`RESUME` clears the order first, so a thread that has not parked yet never will, and then serves a
thread that has. A thread still standing on its old core is deferred by the handshake exactly as a
racing IPC wake is, and that core's `finish_switch` queues it once its context is saved.

## The three hard cases

### Blocked in IPC

The IPC is left alone. The thread stays parked on its rendezvous, a
counterparty may complete it, and the wake that follows puts it on a run queue, where the pick
parks it on the order. `RESUME` then lets it return exactly what it was given.

seL4's `suspend` cancels the IPC instead, and `restart` re-executes the system call. That would
make a suspended receive lose its place in the queue. Worse, it would let a reply-parked `CALL`
caller leave its wait while a `Reply` capability still names it. A caller that leaves its reply
park and enters a second `CALL` is exactly the stale-`Reply` forgery that
[the blocked-thread teardown research](blocked-thread-teardown.md) found and that
`finish_blocked_resident` is built to rule out. Leaving the IPC in place never opens that door,
and it needs no per-architecture PC rewind.

What a stopped job sees, then, is that a message sent to it is taken but not acted on until it is
resumed. For a sender that is the same as a slow receiver.

### Running on another core

`SUSPEND` finds the core whose per-CPU `current` names the thread,
under the lock that `schedule` writes it under, so the answer is exact. It sets that core's
`need_resched` and sends it the reschedule interrupt. Then it waits, yielding, until the thread is
no longer `Running`. When `SUSPEND` returns, the thread is off every core, so a caller that goes on
to read it (the shell printing `Stopped`, one day a debugger reading registers) sees it standing
still. seL4 stalls the remote core for the same reason. The wait also ends if someone else resumes
the thread or it ends, since either leaves nothing to wait for. If the interrupt is lost, the
target core's next timer tick sets the same flag, so the wait is at most one tick longer.

A thread suspending itself takes the same road with no interrupt: it sets its own order and calls
`schedule`, and its `SUSPEND` returns 0 when somebody resumes it. §300 says the kernel never
suspends a thread of its own accord, and this is not that: the order still comes from a holder of
the capability.

### Already dead

A TCB whose thread has exited, faulted, or been doomed by `DESTROY` is `Gone` to
both verbs. One whose thread has been reaped no longer resolves, and its generational name is
`NoSuchSlot` rather than a newer thread in the same slot. An embryo is `WrongObject`: it has never
run, and `START` is its verb. Neither verb turns a corpse back into a thread.

A thread killed while it carries the order dies rather than stopping. `schedule` checks the kill
first. A doomed thread that the pick parks anyway (one killed while queued) is finished in place
by its region's teardown like any parked thread. The system test `suspension_tests` destroys the
region of a suspended thread and checks that it comes back.

## Idempotence

`SUSPEND` on a suspended thread is 0 and changes nothing. So is `RESUME` on a thread that is not
suspended. The orderer observes by acting (§300), and a shell that sends `bg` to a job that is
already running has done nothing wrong.

## BUGS

- **A survey reports a suspended thread as `BLOCKED`.** `ps` and `top` cannot tell a stopped job
  from one waiting on IPC. A `STOPPED` survey code is the fix, and it waits for a consumer, as
  §300's stopped event does.
- **`kill_thread`, the in-kernel test helper, cannot end a suspended thread.** It arms a kill that
  `schedule` spends, and a suspended thread never reaches `schedule`. An IPC-blocked thread has
  the same limit. Real teardown goes through `DESTROY` or a process's end, and both finish a
  parked thread in place.
