# Rendezvous: send, receive, call and reply, and freeing stranded callers

*Name: provisional. After the source file it explains.*

An appendix to [notes/scheduler.md](../scheduler.md). It carries the design behind
`kernel/src/sched/rendezvous.rs`, whose doc comments link here. It also says what the tests in
`kernel/src/sched/rendezvous/tests.rs` are built against. The switch every blocking path ends in is
in [switch.md](switch.md). Names and call/reply semantics are in
[notes/ipc-naming.md](../ipc-naming.md) and DECISIONS §12 (call/reply IPC).

## Creating one

A rendezvous lives at the start of one page, retyped from a region and pinned there. A fresh
generational name in the registry names it. `try_create_rendezvous_from` checks the registry's
capacity before the retype, under the same hold of `IPC_TABLES`, so a full registry spends no page.
The two failures stay apart because they call for opposite responses: carve more memory and retry,
or give up. The page arrives zeroed, and an all-zero `Rendezvous` is valid. It is still written
explicitly, because "happens to be valid" stops being true silently.

`create_rendezvous` serves the kernel's own services. It carves chunks lazily and grows them on
demand, so nothing depends on a guess at the suite's eventual size. A chunk that fills is forgotten,
not freed: its pages stay pinned and its endpoints stay live. Because those chunks are never freed,
the function is `pub(crate)`, and the `system_tests` crate cannot call it. A test endpoint made there
cost the boot a page for good. The frame ledger then moved in steps of 32, charged to whichever
later test crossed a chunk boundary. A test carves from a region it owns with
`create_rendezvous_from` and reclaims it, or hands the region to a `user::holding::Holding`.

## The two halves of a rendezvous

A send that finds a receiver waiting drops the message into the receiver's mailbox and wakes it.
Nobody blocks. A send that finds nobody parks the message in the sender's own mailbox and joins the
sender queue. It marks itself `Blocked` and calls `schedule`. A later receiver reaches into that
mailbox and wakes it. A receive mirrors this. Each path blocks outside the lock, and only after the
thread has recorded itself as blocked. A timer-driven `schedule` in the gap then does the right
thing either way. Every wake calls `serve` first, which passes the undelivered-wake gate
([undelivered-wake-gate.md](undelivered-wake-gate.md)).

The mailbox is five words wide for every message. An ordinary message fills three. Word 3 is the
badge on the endpoint capability the sender invoked, 0 when unbadged, and word 4 is zero. Only a
fault or exit message (DECISIONS §26 (the fault endpoint)) fills the top two otherwise. One width
lets the fault path reuse the rendezvous machinery. A plain `SEND` carries its badge because §242 (a
system log) stamps every record from the writer's badge. A log writer speaks the byte sink, which is
a plain `SEND`. Without the word, the badge a spawner minted would be dropped on the path it was
minted for. The same store that wrote a zero there writes the badge, so it costs no instruction.

A bound notification (milestone 151 (notification objects), DECISIONS §101 (notification objects))
ends a receive before it can block. A signal counted while the thread was elsewhere is taken first,
ahead of a queued sender, because it arrived earlier. The `is_some` test on the thread in hand is
the one load and one branch §101 priced onto the path. It is an arm of the same decision, not an
early return. A `return` inside the lock hold gave the function a second copy of the unlock path. It
cost `ipc_receive` 138 bytes on riscv64 and 174 on x86_64.

## A plain `RECEIVE` never takes a capability

§246 (a plain `RECEIVE` never takes a capability) holds on every arrival order (calef's ruling A,
2026-10-04 UTC):

- A `SEND_CAP` that meets a parked plain `RECEIVE` delivers the data word and drops the copy. The
  sender keeps its own, because `SEND_CAP` narrows a copy and never moves the source.
- A plain `RECEIVE` that collects a parked `SEND_CAP` sender clears the sender's `outgoing_cap`.
  Left in place, the delegation would ride the sender's next plain `SEND` on another rendezvous. It
  would reach whoever `RECEIVE_CAP`s there: a delegation made to one endpoint, delivered to another.
  `set_ipc_aborted` closes the same hazard on the teardown path.
- A `CALL` that meets a plain `RECEIVE`, in either order, delivers its words with no Reply. It is
  answered `Gone`, since no Reply exists for anyone to send (`call_meets_plain_receive`,
  `collected_without_serving`). These paths are `#[cold]`. Every `CALL` server in the tree receives
  with `RECEIVE_CAP`, so the fastpath pays one branch and none of their bytes.

`RECEIVE_CAP`'s `x1` is `NO_CAP` unless a capability was installed for this delivery (milestone 634
(a plain `SEND` received by `RECEIVE_CAP` never hands over a sender-chosen slot)). A plain `SEND`
that reached a parked `RECEIVE_CAP` dropped its three words straight into the mailbox. So `m[1]` is
a word the sender chose. Returned as `x1`, it would hand a sender a slot number where a `CALL`
server reads a reply slot. `x4` is `REPLY_DELIVERED` only on the two paths a `CALL` reaches a
receiver (milestone 706 (a `CALL` server can tell a Reply from a delegation), §245 (a kernel tag in
`x4`)). A `SEND_CAP` delegation reaches the same `x1` and never carries it. `receiving_cap` is
cleared when a `RECEIVE_CAP` resumes, not when a plain `RECEIVE` parks. That keeps the store off
`ipc_send_receive`'s closure, where it took x86_64 past `script/fastpath-footprint`'s 5% band.

Filing a delivered capability (`deliver_capability`) takes the receiving table's own lock beneath
`IPC_TABLES`, rank 60 then 57. It is one out-of-line copy for its three callers. Inlined at each
site, the lock's mask, rank check and release grew both fastpath closures past their band on
aarch64 and riscv64.

## A death message's label

A supervisor receiving a death message gets the dead thread's label in argument register 5 of its
saved user frame: `x5`, `a5` or `r9` (milestone 105 (the two forks), §148 (a supervisor restarts by
asking), ruling R3). It sits beside the mailbox, not in it. Word 3 already carries the fault
address, and word 4 is reserved for §26.4's resume protocol. A six-word mailbox would make every
delivery store one more word. Here, only a death pays.

Only a plain `RECEIVE` gets the label. `RECEIVE_CAP` lays out its result differently, and §26 names
`RECEIVE` as the death message's receive. A kernel thread has no user frame, so it gets nothing; the
bytes at its stack top are its own stack. An ordinary message writes nothing to the register. A
supervisor that wants 0 to mean "not stamped by the kernel" zeroes it before the receive, as
`user_mode_runtime::receive_fault` does. And a child cannot forge a label, because the badge it
sends with arrives in word 3.

## Call and reply

At the rendezvous the kernel mints a one-shot `Reply` capability naming the caller. It hands the
Reply to the server through `RECEIVE_CAP`. The caller then blocks, reachable only through that
capability. A caller that meets no server parks its words and rides the Reply in `outgoing_cap`. The
eventual `RECEIVE_CAP` hands it over and, seeing a Reply, leaves the caller blocked. A full server
table drops the Reply, and the caller then waits until it is torn down. `ipc_call` forwards to
`ipc_call_badged` with badge 0 and is inlined, so the unbadged fastpath `script/icount` measures
gains no frame. `fastpath_pad`'s perturbation sits on the `CALL` side too. The `CALL`/reply closure
is the larger one, and the shape real services run.

`ipc_reply` touches only a thread whose `wait_on` says it awaits a reply. A Reply names a tid, not a
wait state. This is the one wake site addressed by tid rather than through a wait queue. Delivered
to a thread parked as an ordinary receiver, it would clobber that thread's mailbox. It would wake
the thread while its TCB is still linked on the wait queue, a double enqueue on one intrusive link.

`ipc_reply_capability` (§255 (each socket is its own capability)) reads, checks and files its
capability under one hold, the `Delegation` rule. The copy keeps the source's rights, because the
method has no word left for a mask. A server narrows before it answers. The network stack mints
from a `WRITE | GRANT` copy of its own endpoint. The server learns whether the copy landed, so it
can undo what the capability was for.

## Freeing callers a server can no longer answer

A `CALL` caller whose request was taken left every wait queue at the rendezvous. It is linked on
nothing, and only `ipc_reply` wakes it. `abi::Error::Gone` reaches wait queues and nothing else. So
without more, a caller stranded by a dead server stayed blocked for the life of the machine. It was
also a region nobody could reclaim. QNX Neutrino has not allowed that since the 1990s: "if the
server thread fails, exits, or disappears, the client thread becomes READY, with `MsgSend()`
indicating an error". Proposal C of [notes/blocked-thread-teardown.md](../blocked-thread-teardown.md)
is the design that milestone 254 (a caller stranded by a server that died) built.

`strand_reply_caller` sweeps capabilities before it wakes. `cap::reply_cap` mints
`Object::Reply(tid)` with a generational thread name and no call identity. `ipc_reply`'s guard
checks the wait role and ignores the rendezvous. That guard is sound only while no thread can leave
a reply park and enter a second `CALL` with an unconsumed Reply still naming it. Waking a stranded
caller is exactly such a path. A hung server holding the stale capability could forge an answer to a
later, unrelated conversation. L4Re documents the same hazard as a consequence of its finite receive
timeouts.

So every `Object::Reply(caller)` in the machine is deleted first. That is seL4's
`cteDeleteOne(callerCap)`, reached from `cancelIPC`. seL4's other answer, never waking the victim
(`ThreadState_Inactive`), is not available, because waking it is the point.
`delete_reply_caps_naming` also clears `outgoing_cap`, the half a second copy would forget. A caller
that met no server rides its Reply there, and a live Reply in a hand-off slot is the same forgery
one step earlier. `finish_blocked_resident` runs the same sweep on a caller it ends and never wakes.

There are three triggers on the server side, all through `strand_callers_of`:

- `depart`, for a thread that exited or faulted.
- The forcible-teardown conversion at the top of `schedule`, since an armed kill never reaches
  `depart`.
- `reap_region_objects`'s removal phase.

`strand_callers_of` reads the table once into an array of victims, and then acts. Re-resolving the
thread on every slot cost 24 generational lookups per departing thread at 24 slots. That was about
830 icount ticks on every `spawn_reap` iteration. The array, 512 bytes at 64 slots, is in this
`#[inline(never)]` function's own frame, never in `reap_region_objects`'s. An empty table returns
at once.

`strand_callers_awaiting` is the rendezvous side, for a rendezvous being torn down. It is Zircon's
answer, a closed channel failing the call in flight, reached without touching the server.
`drain_waiters` cannot do it, since a reply park is on no queue. `wait_on` records `(ep, Reply)`, so
a scan of the thread table finds it. It rescans rather than lists, because its bound is
`MAX_THREADS`, a kilobyte that grows with the thread ceiling. A list would sit on the chain through
`reap_region_objects`, the deepest frame in the kernel ([notes/stack-high-water.md](../stack-high-water.md)).
The scan ends on the abort flag `strand_reply_caller` sets, not on `wait_on`. A wake deferred behind
`on_cpu` leaves the park in place until that core's `finish_switch`. A predicate on `wait_on` alone
would spin.

All of these are `#[cold]`, as a claim rather than a hint. One call site is the top of `schedule`.
Inlined there, the stranding cost 1,363 bytes of x86_64 IPC fastpath, a 20% growth, for code that
runs only during teardown.

## What the tests are built against

- The waits are on the property, never a yield count. A waiter is on another core since §28 (SMP
  placement). On the physical core under HVF a yield returns in nanoseconds
  ([notes/load-sensitive-assertions.md](../load-sensitive-assertions.md)).
- The reply-park tests assert the sweep as well as the wake. A version asserting only the wake
  would pass while the kernel was worse. A woken caller with a live Reply still naming it is the
  forgery above.
- `a_wake_without_delivery_cannot_complete_a_parked_receive` and
  `a_reply_to_a_thread_parked_as_a_receiver_is_dropped` inject through the real wake path. The gate
  they guard is [undelivered-wake-gate.md](undelivered-wake-gate.md).

## BUGS

- A server that is alive and simply never replies blocks its caller forever. A `CALL` has no
  deadline; that is milestone 106 (a wait that ends on the interrupt or the deadline)'s fork, and
  L4's answer. Nothing ends a server's thread from outside either (milestone 133 (ending a
  permanently blocked thread)). Only a server that stops being able to answer frees its callers.
- A Reply names a thread, not a call. What stops a stale one answering a later `CALL` is that
  `strand_reply_caller` deletes it. `ipc_reply`'s guard cannot tell the conversations apart. A
  future path reaching `ipc_reply` without presenting a capability would reopen that. The structural
  fix is a call identity in the payload:
  [milestone 371 (a reply capability that names a call)](../../design/roadmap/0371-a-reply-capability-that-names-a-call.md).
