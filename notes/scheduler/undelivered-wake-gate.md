# The undelivered-wake gate: the misreading, the companions and the loom model

An appendix to [notes/scheduler.md](../scheduler.md), moved there on 2026-10-11 (UTC) under §212
(a prose budget). The main page says what the gate does. This is why it was built, why the failure it was built
against was misread, and how the protocol is model-checked.

The failure it was built against was misread, and that correction is the important part of this
section. It was built from the VisionFive 2's boot 8 (notes/visionfive2.md, fourth bench stop),
read at the time as the boot thread, parked in `ipc_receive`, taking a `wake:0x0` on a boot where no
sender to its endpoint existed. The boot-8 lane's fifth bench stop (2026-08-15) re-read those
dumps and overturned it: the wake was the worker's real send, and the dump state read as a
stranded receiver was the terminal state of a completed tour. So the gate has never fired on a
field failure, and `refuse:` has never appeared on a board ring.

It stays anyway, and the reason is not sunk cost. The transition it forbids would complete a
rendezvous off a stale mailbox while the TCB stayed linked on its endpoint queue. The loom
harness below proves that as a property of the protocol rather than as a story about one boot. What
changes is the claim: this is hardening against a reachable state, not a repair of a field failure,
and anything that cites it as the latter is now wrong.

The fifth stop's full write-up is notes/visionfive2.md's fifth bench stop.

Two companions from the same boot are kept for the same hardening reason. The first is `ipc_reply`,
the one wake site addressed by a tid rather than through an endpoint pop. It delivers only to a
thread whose `wait_on` says it awaits a reply. Anything else would clobber a parked receiver's
mailbox and double-enqueue its one intrusive link. The second is that `schedule()` refuses to switch
into its own current thread (the shape a spuriously queued current produces). That would restore an
already-consumed context and time-travel the thread onto a reused stack. Guarded by
`a_wake_without_delivery_cannot_complete_a_parked_receive` and
`a_reply_to_a_thread_parked_as_a_receiver_is_dropped`, which inject through the real `wake` path.

The protocol is a crate now, and loom searches it (2026-08-14, the retrofit the fourth bench
stop's audit asked for). The whole block/wake state machine lives in `crates/thread_wake_handshake`. That is `state`,
`on_cpu`, `wake_pending`, `wait_on`, `ipc_served`, `ipc_aborted`, and the gate, deferral and
finish-switch transitions over them. It is embedded in `Thread` and called by `sched.rs` rather
than mirrored, so the model-checked code and the shipped code are the same code. Each of the
protocol's three hazards (wake-before-switch-out, the steal edge of the same window, and the
undelivered wake) is a loom harness that holds with the current semantics and a `#[should_panic]`
reconstruction that fails with the historical ones. Two of the three are races observed on the
machine; the third is the undelivered wake, which the section above records as reachable but never
witnessed, so "three recorded races" overstated it and this says hazards instead. See
notes/interleaving.md
for the model's honest limits.
