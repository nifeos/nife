# The block/wake handshake: its races, its states and the loom model

An appendix to [notes/scheduler.md](../scheduler.md). It carries the design behind
`crates/thread_wake_handshake`, whose doc comments link here. The undelivered-wake gate's own
account is in [undelivered-wake-gate.md](undelivered-wake-gate.md).

## Why the handshake is a crate

The handshake is the per-thread state machine that decides when a blocked thread may become
runnable. It lives in a host crate so that loom can explore every interleaving of it. The kernel
calls these transitions rather than mirroring them, because a proof over a copy proves the copy.
`Thread` embeds a `Handshake`, and the kernel re-exports `RunState` as `thread::State`.

The payload type `W` says what a thread waits on. The kernel uses `(RendezvousId, WaitRole)` and
the tests use small scalars. The handshake only asks whether a payload is present.

## The window, and the three rules

Every field is written under `IPC_TABLES`, and the concurrency is in the gaps between critical
sections. A thread that marks itself `Blocked` keeps running until its core's `switch_to` saves
its context. A waker on another core can take `IPC_TABLES` inside that window. Each race out of the
window is a rule:

- Wake before switch-out ([notes/intrusive-queues.md](../intrusive-queues.md)). Queueing a thread
  that is still on its CPU lets two cores run one thread. `try_wake` finding `on_cpu` set parks the
  wake in `wake_pending`, and `finish_switch` completes it once the context is saved.
- The undelivered wake, which would complete a parked `RECEIVE` off a stale mailbox. `try_wake`
  makes a waiting thread `Ready` only if the waker delivered (`ipc_served`) or aborted the wait
  (`ipc_aborted`). The gate is hardening. It has never fired on a field failure, and the loom
  harness proves that the transition it forbids is unsafe.
- A steal racing a switch-out. Only the owning core pops its run queue, after `finish_switch`, so a
  thief never gets a thread mid-save. `switch_in` asserts that `on_cpu` is clear as the tripwire.

Every path onto a CPU passes `switch_in`. A thread arriving there with `on_cpu` still set means some
queue handed out a thread whose context is still being saved. That is two cores in one thread,
whichever queue it came from. Debug builds fail loudly. The board build proceeds, because by the
time this is observable the coherent recovery is the switch already underway.

## The states and their overlaps

Only `Ready`, `Running` and `Blocked` take part in the handshake. `Embryo`, `Finished` and `Dead`
are entered by kernel-side transitions (retype, exit, fault), and the handshake only reads them. A
wake ignores them, and `finish_switch` hands a `Finished` predecessor back to the kernel to reap.

The idle thread lives outside the ready queue. It is never marked `Ready` when a core switches away
from it, so it reaches `switch_in` still saying `Running` from its last turn.

`preempt` leaves `on_cpu` set until the successor's `finish_switch`. The thread is then in a queue
and still standing on its core. That is the one legal overlap, and it is why the single-owner queue
discipline is load-bearing.

A wake that raced the switch-out completes in `finish_switch`, and the caller queues the thread on
that core. One wake without load-aware placement, in the rare case of an IRQ inside the window, is
the accepted cost. Teaching this path a placement policy was judged not worth it.

`park` writes `wait_on` in the same statement as `state = Blocked`, and a wake that makes the
thread `Ready` clears it. The coupling is diagnostic as well as protocol. A `Blocked` beside a
`None` is a state byte no block path wrote (notes/visionfive2.md, fourth bench stop).

`serve` runs in the same critical section that staged a delivery. A wake whose critical section did
not call it is the wake the gate refuses. No such wake has been observed on hardware, and the loom
module holds the reconstruction that produces one.

## What stays kernel-side

The queues themselves stay in the kernel; the verdicts say when to push. So do the single-threaded
writes of `state` outside the protocol: spawn, `depart`, `START` and the killed-thread conversion.
The self-switch and reply-role checks read state the handshake cannot see, and stay too.

## The loom model

`script/interleaving-check` runs it. `IPC_TABLES` is a `loom::sync::Mutex`, and the run queue and
migration inbox are booleans under it. The thread's saved context is a `loom::cell::UnsafeCell`
written outside the lock, exactly where `switch_to` writes the real one. Each loom thread is a core
running its critical sections in the kernel's program order. The cell turns "the resumer sees a
saved context" from an argument into a checked ordering fact.

The model cannot check that the kernel keeps the discipline it encodes: every site under
`IPC_TABLES`, and single-owner queues. That is established by reading `sched.rs`
([notes/interleaving.md](../interleaving.md)).

Every harness runs to exhaustion, with no `LOOM_MAX_PREEMPTIONS` and no `max_branches` trimming.
That is affordable because each model has three or four threads with two or three critical sections
each. This is the protocol's real shape: one waker, one victim core and one thief are the whole cast
of every recorded race. Each harness that must reach a branch marks it with a real atomic that
survives across executions, so a search that never reaches the branch fails.

The reconstructions of the old semantics are `#[should_panic]`, so each is required to fail rather
than assumed to:

- a wake that respects the gate but skips the switch-out deferral, which switches into a thread
  whose core has not saved its context;
- a thief that pops the victim's queue directly, breaking the single-owner rule;
- a wake that keeps the deferral but has no gate, which resumes a receiver with nothing delivered.

`resume` restates `switch_in`'s tripwires as real asserts. `script/interleaving-check` compiles with
`--release`, where a `debug_assert` would leave the reconstructions nothing to fire.
