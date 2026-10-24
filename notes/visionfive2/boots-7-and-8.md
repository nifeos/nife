# Boots 7 and 8: the fourth bench stop

*Name: ratified 2026-10-18 (calef, "Approve all").*

An appendix to [notes/visionfive2.md](../visionfive2.md). It keeps the fourth bench stop
(2026-08-14) as written, including the conclusion the fifth stop overturned on 2026-08-15. Read it
with [boots-9-and-10.md](boots-9-and-10.md) in hand.

## Boot 7's impossible pair

This covers boot 7's impossible pair, what the audit ruled out, and what boot 8 would say. Boot 7
carried the online-set sweep and the new cross-hart `fence.i`. It hung in a shape none of the
previous stops produced, stable across five thread dumps over ten seconds. `init`, the only user
thread, was `Blocked` with its saved user pc at 0x00400188, a plain store loop in the builder's
memset. The boot thread was `Running`, `on_cpu`, as core 2's current the whole time. Two endpoints
each held one parked receiver, with no senders and no pending signals. The syscall count was frozen
at 20.

## What the dump could honestly claim

This was established by reading the dump's locking rather than assuming it. `state`, `on_cpu`,
`wake_pending` and the endpoint counts are one consistent snapshot: every writer holds SCHED and the
dump holds SCHED. The pc column is the trap frame at the thread's stack top, which trap entry writes
without the lock. So it is a racing read for a thread on a cpu. It is trustworthy for a parked one.
The frame write happened-before the state write on the thread's own core, and the dump's lock
acquire synchronizes with that core's release. So init's memset pc is evidence, not a dump
artifact. The dump now says this about itself (the `pc*` marker and the honesty comment in
`sched::dump_threads`).

## The audit

And it is evidence of a state no legal transition sequence produces. The audit walked every write
of `State::Blocked` in the tree. There are five sites, all under SCHED, all applied to the executing
core's own current thread. A user thread reaches any of them only through its own `ecall`. The
syscall path advances the frame's `sepc` past the `ecall`. So a legitimately blocked user thread's
dumped pc is its syscall site, never a memset store. A timer preemption leaves `Ready`, and nothing
blocks a `Ready` thread in absentia.

The wake-before-switch-out family was read against this state, and it holds. A preempted thread's
context is saved before any core can pop it: run queues are single-owner, and interrupts are masked
from the requeue through `finish_switch`. A deferred wake (`wake_pending`) completes on the thread's
own core after the context is real. The one lock-free cross-core protocol, the steal slot, is
loom-checked in `crates/steal_request`.

The block/wake protocol itself had no loom coverage when this was written. It is lock-based.
Modeling it means extracting SCHED plus the run queues plus the inbox into a host-checkable crate,
which is a milestone of its own, not a bench-night patch. (Since done, 2026-08-14:
`crates/wake_handshake` extracts the handshake with SCHED as a loom mutex. Each of this protocol's
recorded races is a harness plus a failing reconstruction; see notes/interleaving.md.)

The riscv64 `tp` plumbing, the prior art for exactly this smell, was re-audited and reads correct.
Trap entry reloads `tp` from the per-hart stash. An S-mode return keeps the live `tp`. `switch_to`
never carries one. The stash is per-hart, written once.

## Three mechanisms survive the audit

Boot 8's serial log now discriminates them (the instrumentation commit on this branch):

1. A `Blocked` byte written outside the block paths. That is a stray write into the TCB, or a block
   applied to the wrong thread through a wrong per-cpu resolution. `Thread::wait_on` (endpoint and
   sender/receiver/reply role) is written in the same SCHED-held statement as `Blocked`, and is
   printed per thread. `Blocked` beside `wait=-` at boot 8 is corruption. `wait=ep/role` means the
   block path ran, and names the endpoint it ran against.
2. A hart wedged where no trap can land. The boot thread sat `Running` as core 2's current for ten
   seconds, with SCHED demonstrably free (the dumps kept printing). That means core 2 reached no
   scheduler entry for ten seconds: an S-mode spin with interrupts masked, or an SBI call that
   never returned. Boot 7 was the first boot to carry `sbi_remote_fence_i`, issued for every
   executable-page map. It went into vendor OpenSBI, the same firmware whose HSM fell over on hart 0
   (the second stop). A hart parked in M-mode takes no delegated S-interrupts. So it freezes with
   its last `current` on display and, until now, nothing in the dump to say so. The per-core
   `ticks` column is the discriminator: a wedged core's tick count holds still between dumps. The
   `steal_req` column shows the same wedge from a thief's side, as a claimed slot that is never
   served.
3. An intrusive-link double-enqueue. One `Thread::next` link serves run queues, inboxes and
   endpoint queues, so a double-enqueue corrupts two structures silently. No path that produces one
   was found, but the class cannot be ruled out from the end state alone. The per-cpu event ring is
   what will show the path if the state machine took an illegal step. It holds the last 16
   scheduler events each core performed: switch, block, wake, deferred wake, remote place, steal
   serve and inbox drain. The dump prints it.

The third stop's parked-inbox dump line is now a debug assertion in the placement path, per the
audit lane's handoff. It is loud in every QEMU test build and compiled out of the release board
image, where the dump line remains the field diagnostic.

## Why QEMU was not expected to reproduce it

This was said before the runs rather than after. TCG's emulated memory model is far stronger than
the U74's. Guest accesses execute in the host's program order, and MTTCG serializes cross-vCPU
visibility through host atomics. QEMU `virt`'s online set is contiguous from zero. Its firmware is
mainline OpenSBI. So all three candidate mechanisms are structurally hidden there. A green QEMU
suite says the instrument is safe to fly, not that boot 7 cannot recur.

It was attempted anyway, as it should be. The full riscv64 suite passed at `-smp 4` unloaded, on
the sifive-u54 model, and again with the host starved by six busy loops. That suite includes the
steal/migration hammers: the cpu-bound batch, the migrated-`tp` waves and the cross-hart ASID
shootdown. There was no reproduction. That is the expected null result, recorded so nobody mistakes
it for evidence of health.

## Boot 8: the instrument worked, and the transition was made impossible

*(Overturned 2026-08-15: the fifth stop re-read these same dumps. The "undelivered" wake was the
worker's real send, and the state read as fabricated is the terminal state of a completed tour. The
paragraphs are kept as written because the reasoning is the record. Read them with the fifth stop's
correction in hand.)*

The dump discriminated the candidates exactly as designed. Every core's tick count climbed normally
across ten seconds of dumps, so no hart was wedged in M-mode. Candidate 2 is out, and the
`sbi_remote_fence_i` suspicion with it. The wait column was populated on every blocked thread, so
there was no bare corrupted state byte. Candidate 1's simplest form is out.

The boot hart's event ring showed the path itself instead. First came `block:0x0/0`, the boot
thread parking in `ipc_receive` on the report endpoint. Later came `wake:0x0`. Then
`steal:0x100000005/2`: the diag watcher handed to core 2, which is the core the dumps then printed
from. Then `switch:0x0`, and then nothing for ten seconds. Throughout, the boot thread sat `Running`
as that core's current with `wait=-`, and the report endpoint's receiver queue was empty. A receiver
was woken with nothing delivered.

The receive tail (`sched::ipc_receive`) read the mailbox unconditionally after `schedule()`
returned. So an undelivered wake completed a rendezvous that never happened. It read a mailbox
holding whatever it last held, with the TCB's endpoint linkage in whatever state the spurious waker
left it. That is the strand. The receive neither completes with a message nor re-parks, because the
code had no way to notice the difference.

## The wake's issuer is not established

The census says that plainly. Every `wake()` caller in the tree delivers something first. The four
rendezvous sites stage a mailbox. `irq_notify` counts a signal. `deliver_death` stages a death
message. `ipc_reply` stages a reply. The revocation drain flags an abort.

On the wedged boot none of them was reachable. The syscall counter was frozen, so no user thread
was sending. The boot tour parks in this demo *before* the UART-driver step. So no IRQ was routed
to any endpoint and no reply capability had ever been minted. And nothing was being revoked. The
ring proved the transition happened without any legal path having produced it.

So the fix closes the transition, not a caller. `wake()` and `wake_load_aware` now refuse to make
a waiting thread Ready unless the waker delivered. Delivery is `Thread::ipc_served`, set in the
same SCHED critical section that stages the message or signal, or `ipc_aborted`. A refused wake is
recorded on the ring as `refuse:tid`. `ipc_reply` is the one wake site addressed by tid rather than
through an endpoint pop. It additionally refuses any thread not parked awaiting a reply.

And `schedule()` refuses to switch into its own current thread, the pop-yourself shape a spuriously
queued current produces. Doing so restores an already-consumed context. Execution time-travels to
its previous switch-out point on a reused stack and spins there forever, off every instrument. That
is precisely the silence boot 8's ring recorded after `switch:0x0`.

Boot 9 therefore either completes the demo, or its dump carries `refuse:` events. Those name the
core that issued the spurious wake and the thread it aimed at, which is the culprit's address. It
was proven red-then-green in QEMU by `a_wake_without_delivery_cannot_complete_a_parked_receive` and
`a_reply_to_a_thread_parked_as_a_receiver_is_dropped` (sched.rs). Both inject through the real wake
path rather than by poking state.

## Two rows recorded as a finding of their own

*(Overturned 2026-08-15, fifth stop: both rows are real, legitimate, parked-by-design waiters of the
tour's UART-driver step, which had already run. The census this section rests on was correct about
the park point and wrong about which moment the dump was showing.)*

The dump showed init (tid 0x400000004) `Blocked` as a *Receiver* on ep 0x1, with its saved user pc
in the builder's memset loop. It showed a gen-2 kernel thread in slot 6 `Blocked` as a Receiver on
ep 0x2. Both read as legitimate parked waiters, and neither survives the code.
`components/src/builder.rs`, the program init runs on this boot, issues no receive of any kind. Its
only verbs are `invoke` (retype/map/configure/start), `send` and `exit`.

And at the point this boot parks, exactly one endpoint exists. That is the report endpoint, created
at `user.rs`'s `riscv_initrd_demo`, which the registry names 0x0. The UART demo that creates the
next two runs later in the tour and was never reached. No reachable path, the builder's retypes
included, creates an endpoint in between. So ep 0x1 and ep 0x2, and the two receivers parked on
them, are kernel state no code that ran can have written.

The instrument's own honesty note said `wait=ep/role` means "the block path ran". Boot 8 is the
counterexample: it means the field holds those bytes, and corruption can produce that. Candidate 3's
class is therefore still open: structure corruption, whether from a stray write, the U74's memory
model meeting a latent race, or the vendor firmware. It has a narrower fingerprint now. It
fabricates *coherent-looking* waiter state, not garbage. The gate does not fix that and does not
claim to. It makes the scheduler refuse to act on one consequence of it. The `refuse:` ring events
are the tripwire that will show where it fires from.
