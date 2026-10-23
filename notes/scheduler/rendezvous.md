# Rendezvous: send, receive, call and reply, and freeing stranded callers

An appendix to [notes/scheduler.md](../scheduler.md). It carries the design behind
`kernel/src/sched/rendezvous.rs`, which its doc comments link here, and what the tests in
`kernel/src/sched/rendezvous/tests.rs` are built against. The switch every blocking path ends in is
in [switch.md](switch.md); names and call/reply semantics are in
[notes/ipc-naming.md](../ipc-naming.md) and DECISIONS §12 (call/reply IPC).

## Creating one

A rendezvous lives at the start of one page retyped from a region and pinned there, named by a
fresh generational name in the registry. `try_create_rendezvous_from` checks the registry's capacity
before the retype, under the same hold of `IPC_TABLES`, so a full registry spends no page; the two
failures are kept apart because they call for opposite responses (carve more memory and retry, or
give up). The page arrives zeroed and an all-zero `Rendezvous` is valid, but it is written
explicitly anyway, because "happens to be valid" stops being true silently.

`create_rendezvous` serves the kernel's own services from lazily carved chunks that grow on demand,
so nothing depends on a guess at the suite's eventual size. A chunk that fills is forgotten, not
freed: its pages stay pinned and its endpoints stay live. Those chunks are never freed, so it is
`pub(crate)`, and the `system_tests` crate, which depends on the kernel, cannot call it. Every
endpoint a test made there cost the boot a page for good, and the frame ledger moved in steps of 32
charged to whichever later test crossed a chunk boundary. A test carves from a region it owns with
`create_rendezvous_from` and reclaims it, or hands the region to a `user::holding::Holding`.

## The two halves of a rendezvous

A send that finds a receiver waiting drops the message into the receiver's mailbox and wakes it,
and nobody blocks. A send that finds nobody parks the message in the sender's own mailbox, joins the
sender queue, marks itself `Blocked` and calls `schedule`; a later receiver reaches into that
mailbox and wakes it. A receive mirrors this. Each path blocks outside the lock, and only after the
thread has recorded itself as blocked, so a timer-driven `schedule` in the gap does the right thing
either way. Every wake a path makes passes the undelivered-wake gate
([undelivered-wake-gate.md](undelivered-wake-gate.md)) by calling `serve` first.

The mailbox is five words wide for every message. An ordinary message is three words, word 3 is the
badge on the endpoint capability the sender invoked (0 when unbadged), and word 4 is zero; only a
fault or exit message (DECISIONS §26 (the fault endpoint)) fills the top two otherwise. One width
means the fault path reuses the rendezvous machinery rather than growing a parallel one. A plain
`SEND` carries its badge because §242 (a system log) stamps every record from the writer's badge,
and a log writer speaks the byte sink, a plain `SEND`; without the word, the badge a spawner minted
would be dropped on the path it was minted for. The badge is written by the same store that once
wrote a zero there, so it costs no instruction.

A bound notification (milestone 151 (notification objects), DECISIONS §101 (notification objects))
ends a receive before it can block: a signal counted while the thread was elsewhere is taken first,
ahead of a queued sender, because it arrived earlier. The `is_some` test on the thread already in
hand is the one load and one branch §101 priced onto the path. It is an arm of the same decision
rather than an early return, because a `return` inside the lock hold gave the function a second copy
of the unlock path and cost `ipc_receive` 138 bytes on riscv64 and 174 on x86_64.

## A plain `RECEIVE` never takes a capability

§246 (a plain `RECEIVE` never takes a capability) holds on every arrival order (calef's ruling A,
2026-10-04 UTC):

- A `SEND_CAP` that meets a parked plain `RECEIVE` delivers the data word and drops the copy. The
  sender keeps its own, because `SEND_CAP` narrows a copy and never moves the source.
- A plain `RECEIVE` that collects a parked `SEND_CAP` sender clears the sender's `outgoing_cap`.
  Left in place, the delegation would ride the sender's next plain `SEND` on a different rendezvous
  and reach whoever `RECEIVE_CAP`s there: a delegation made to one endpoint, delivered to another.
  `set_ipc_aborted` closes the same hazard on the teardown path.
- A `CALL` that meets a plain `RECEIVE`, in either order, delivers its words with no Reply and is
  answered `Gone`, since no Reply exists for anyone to send (`call_meets_plain_receive`,
  `collected_without_serving`). These paths are `#[cold]`: every `CALL` server in the tree receives
  with `RECEIVE_CAP`, so the fastpath pays one branch and none of their bytes.

`RECEIVE_CAP`'s `x1` is `NO_CAP` unless a capability was installed for this delivery (milestone
634). A plain `SEND` that reached a parked `RECEIVE_CAP` dropped its three words straight into the
mailbox, so `m[1]` is a word the sender chose, and returning it would hand a sender a slot number
where a `CALL` server reads a reply slot. `x4` is `REPLY_DELIVERED` only on the two paths a `CALL`
reaches a receiver (milestone 706 (a `CALL` server can tell a Reply from a delegation), §245 (a
kernel tag in `x4`)), so a `SEND_CAP` delegation, which reaches the same `x1`, never carries it.
`receiving_cap` is cleared when a `RECEIVE_CAP` resumes rather than when a plain `RECEIVE` parks,
which keeps the store off `ipc_send_receive`'s closure; there it took x86_64's closure past
`script/fastpath-footprint`'s 5% band.

Filing a delivered capability (`deliver_capability`) takes the receiving table's own lock beneath
`IPC_TABLES` (rank 60, then 57). It is one out-of-line copy for its three callers, because inlined
at each site that lock's mask, rank check and release grew both fastpath closures past their band on
aarch64 and riscv64.

## A death message's label

A supervisor receiving a death message gets the dead thread's label (milestone 105 (the two forks),
§148 (a supervisor restarts by asking), ruling R3) in argument register 5 of its saved user frame:
`x5`, `a5` or `r9`. It sits beside the mailbox, not in it, because word 3 already carries the fault
address and word 4 is reserved for §26.4's resume protocol; a six-word mailbox would make every
delivery store one more word, and here only a death pays. Only a plain `RECEIVE` gets it, since
`RECEIVE_CAP` lays out its result differently and §26 names `RECEIVE` as the death message's
receive. A kernel thread has no user frame, and the bytes at its stack top are its own stack, so it
gets nothing. An ordinary message writes nothing to the register, so a supervisor that wants 0 to
mean "not stamped by the kernel" zeroes it before the receive (`user_mode_runtime::receive_fault`
does), and a child cannot forge a label: the badge it sends with arrives in word 3.

## Call and reply

At the rendezvous the kernel mints a one-shot `Reply` capability naming the caller and hands it to
the server through `RECEIVE_CAP`; the caller then blocks, reachable only through that capability. A
caller that meets no server parks its words and rides the Reply in `outgoing_cap`, so the eventual
`RECEIVE_CAP` hands it over and, seeing a Reply, leaves the caller blocked. A full server table
drops the Reply, and the caller then waits until it is torn down. `ipc_call` forwards to
`ipc_call_badged` with badge 0 and is inlined, so the unbadged fastpath `script/icount` measures
gains no frame. `fastpath_pad`'s perturbation is on the `CALL` side as well as the `SEND` side
because the `CALL`/reply closure is the larger one and the shape real services run.

`ipc_reply` touches only a thread whose `wait_on` says it awaits a reply. A Reply names a tid, not a
wait state, and this is the one wake site addressed by tid rather than through a wait queue.
Delivered to a thread parked as an ordinary receiver, it would clobber that thread's mailbox and
wake it while its TCB is still linked on the wait queue, a double enqueue on the one intrusive link.

`ipc_reply_capability` (§255 (each socket is its own capability)) reads, checks and files its
capability under one hold, the `Delegation` rule. The copy keeps the source's rights, because the
method has no word left for a mask; a server narrows before it answers (the network stack mints from
a `WRITE | GRANT` copy of its own endpoint). The server learns whether the copy landed, so it can
undo what the capability was for.

## Freeing callers a server can no longer answer

A `CALL` caller whose request was taken left every wait queue at the rendezvous: it is linked on
nothing, and only `ipc_reply` wakes it. `abi::Error::Gone` reaches wait queues, so without more a
caller stranded by a server that died stayed blocked for the life of the machine, and was itself a
region nobody could reclaim. QNX Neutrino has not allowed that since the 1990s ("if the server
thread fails, exits, or disappears, the client thread becomes READY, with `MsgSend()` indicating an
error"). Proposal C of [notes/blocked-thread-teardown.md](../blocked-thread-teardown.md) is the
design; milestone 254 (a caller stranded by a server that died) built it.

`strand_reply_caller` sweeps capabilities before it wakes. `cap::reply_cap` mints
`Object::Reply(tid)` with a generational thread name and no call identity, and `ipc_reply`'s guard
checks the wait role while discarding the rendezvous. That guard is sound only while nothing can
leave a reply park and enter a second `CALL` with an unconsumed Reply still naming it, and waking a
stranded caller is exactly such a path. A hung server holding the stale capability would otherwise
forge an answer to a later, unrelated conversation; L4Re documents the identical hazard as a
consequence of its finite receive timeouts. So every `Object::Reply(caller)` in the machine is
deleted first, which is seL4's `cteDeleteOne(callerCap)` reached from `cancelIPC`. seL4's other
answer, never waking the victim (`ThreadState_Inactive`), is not available, because waking it is the
point. `delete_reply_caps_naming` also clears `outgoing_cap`, the half a second copy would forget: a
caller that met no server rides its Reply there, and a live Reply in a hand-off slot is the same
forgery one step earlier. `finish_blocked_resident` runs the same sweep on a caller it ends and
never wakes.

There are three triggers. `strand_callers_of` is the server side, for a thread that can no longer
answer anybody: `depart`, the forcible-teardown conversion at the top of `schedule` (an armed kill
never reaches `depart`), and `reap_region_objects`'s removal phase. It reads the table once into an
array of victims and then acts. Re-resolving the thread on every slot, because `strand_reply_caller`
deletes out of the same table, cost 24 generational lookups per departing thread at 24 slots, about
830 icount ticks on every `spawn_reap` iteration; the array (512 bytes at 64 slots) is in this
`#[inline(never)]` function's own frame, never in `reap_region_objects`'s, and an empty table
returns at once. `strand_callers_awaiting` is the rendezvous side, for a rendezvous being torn down,
which is Zircon's answer (a closed channel fails the call in flight) reached without touching the
server. `drain_waiters` cannot do it, since a reply park is on no queue; `wait_on` records `(ep,
Reply)`, so a scan of the thread table finds it. It rescans rather than lists, because its bound is
`MAX_THREADS`, a kilobyte that grows with the thread ceiling, on the chain through
`reap_region_objects`, the deepest frame in the kernel
([notes/stack-high-water.md](../stack-high-water.md)). The scan ends on the abort flag
`strand_reply_caller` sets, not on `wait_on`: a wake deferred behind `on_cpu` leaves the park in
place until that core's `finish_switch`, and a predicate on `wait_on` alone would spin.

All three are `#[cold]` as a claim, not a hint. One call site is the top of `schedule`, and inlined
there the stranding cost 1,363 bytes of x86_64 IPC fastpath, a 20% growth, for code that runs when
something is being torn down.

## What the tests are built against

- The waits are on the property, never a yield count: a waiter is on another core since §28 (SMP
  placement), and on the physical core under HVF a yield returns in nanoseconds
  ([notes/load-sensitive-assertions.md](../load-sensitive-assertions.md)).
- The reply-park tests assert the sweep as well as the wake. A version asserting only the wake
  would pass while the kernel was worse than before it, because a woken caller with a live Reply
  still naming it is the forgery above.
- `a_wake_without_delivery_cannot_complete_a_parked_receive` and
  `a_reply_to_a_thread_parked_as_a_receiver_is_dropped` inject through the real wake path; the
  gate they guard is [undelivered-wake-gate.md](undelivered-wake-gate.md).

## BUGS

- A server that is alive and simply never replies blocks its caller forever. A `CALL` has no
  deadline (milestone 106 (a wait that ends on the interrupt or the deadline)'s fork, and L4's
  answer), and nothing ends a server's thread from outside (milestone 133 (ending a permanently
  blocked thread)'s). Only a server that stops being able to answer frees its callers.
- A Reply names a thread, not a call, so what stops a stale one answering a later `CALL` is that
  `strand_reply_caller` deletes it, not that `ipc_reply`'s guard can tell the conversations apart. A
  future path reaching `ipc_reply` without presenting a capability would reopen that. The structural
  fix is a call identity in the payload: [milestone 371 (a reply capability that names a
  call)](../../design/roadmap/0371-a-reply-capability-that-names-a-call.md).
