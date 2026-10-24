# Live replacement: the operator, role by role

*An appendix to [`notes/live-replacement.md`](../live-replacement.md), which is the page to read and
holds the five steps of a swap and why they run in that order. This file holds the design inside
`components/src/swapper.rs`, the operator, one system per entry role.*

The operator is an unprivileged process. The kernel does not know a swap is happening, has no
notion of a component, and gained no method for one. It supplies what it already supplied.
There are endpoints that name no peer (§12 (call/reply IPC: a one-shot reply capability)), a death
message (§26 (the fault endpoint)) and a reap that needs no construction authority (§32). There is
revocation: §13 (capability revocation and untyped reclamation), §16, and the device take-back of
§41. Everything else is code in the operator.

## The roles

- `ROLE_DIRECT` is the default rung. The stable name a client holds is the endpoint object itself,
  and the swap changes who is parked in `RECEIVE_CAP` on it. No process sits in the data path, so
  the steady state costs exactly what an unbrokered call costs.
- `ROLE_QUEUED` is the opt-in rung. A `broker` stands between producer and backend, so the producer
  never blocks on an absent consumer. It costs one extra hop, priced by the `broker_rtt` benchmark,
  and is chosen per channel rather than imposed on every IPC.
- `ROLE_UNWARNED` and `ROLE_LATE_WARNING` are the queued system with the dependent never warned, or
  warned only after the swap. That is what a supervisor is left with when a dependent will not
  answer, and it costs latency, not loss ([notes/non-cooperative-fallback.md](../non-cooperative-fallback.md)).
- `ROLE_HUNG` runs the direct system against an incumbent that stops answering without dying, and
  `ROLE_HUNG_WITHOUT_DEVICE` is the same run with no device, for `x86_64`
  ([notes/hung-component.md](../hung-component.md)).
- `ROLE_HANDOFF` carries a tally across the swap on a handoff page, and tries the swap twice
  ([notes/state-handoff.md](../state-handoff.md)).
- `ROLE_DEPUTY` is the confused-deputy channel: one incumbent holding the device and the log, and a
  client that drives it to write past the log into the device page. There is no swap; the question
  is what a client can make a server do on its behalf.

## Routing, and the control that must fail

Each child is wired from a routing table: which of the operator's own objects answers to which
name the component uses. No rights, addresses or slot order are spelled in the operator; each
component's declaration supplies them through `capability_plan`. A component and its client get two
tables rather than one wide one, because the two bounds are independent. The manifest bounds what a
component may ask for, and the routing table bounds what the operator offers. A client that could
be routed the device would be one manifest edit away from holding it.

Every channel first asks for a plan it must be refused, before anything is built. The direct
channel asks for the broker, whose `requests` and `backend` it routes neither of. The queued channel
routes no device, so it asks for the console. The handoff channel asks the client's table, which
routes no handoff page, for the stateful contract. Wiring a stateful component without one would be
a kill-and-replace that reads like a swap while the tally went to zero. A refusal is a value, with
nothing built, mapped or started, which is what makes a manifest a request rather than an
instruction. Both the direct and queued roles report one, because a mechanism that only worked
against the component it was written for would not be a mechanism.

On the queued channel the producer's declaration is `CLIENT`, the one the direct channel's client
uses, and its `service` resolves to the broker's front endpoint. One declaration takes two
routings, and the program cannot tell which it got. That makes a component's peer substitutable,
not only the component.

## When a swap waits, and when it does not

The replacement is built before anyone is talking, endowed with everything except the device and
not configured, so it cannot race the incumbent for requests. The swap then lands at a point the
incumbent chooses: it notes the operator after serving `SWAP_TRIGGER` requests, so the swap falls
in a live conversation rather than at a moment picked by counting yields.

Before the drain, the operator asks the dependency graph who depends on the target among the live
instances. On the direct channel the answer is a real empty set, because nothing declares `console`
in its `depends_on`. `CLIENT` is a pure consumer and never needs telling, by the sender-queue
argument of §41 (the endpoint is the broker, and a device is revoked by taking it back)
(`capability_plan::Requirements::depends_on`'s doc comment). On the queued channel the broker's
manifest declares `depends_on: &["backend"]`, and the two-instance registry checked against
`capability_plan::dependents` decides whether the broker is warned. The operator never waits for a
warning to be seen (§231 (a swap's warning to a dependent is advisory, and the supervisor never
waits for it)).

## The attacker after the direct swap

Once the honest system is done, the direct channel starts an attacker with exactly the client's
capabilities, wired from the client's own declaration and routing table. It tries to become the
server on the endpoint it is a client of. A plant, wired the same way, parks one message on the
endpoint and exits when anyone takes it.

The replacement is retired first, and that order is the point. A `RECEIVE_CAP` the kernel fails to
refuse takes from the queue or blocks. With a live server receiving, the queue is always empty, so
a break surfaced as a watchdog hang rather than a report (measured 2026-09-16 (UTC), milestone 305
(the six kernel confinement rows get a falsification a machine can replay)). Once the replacement
has answered `QUIESCE` it never receives on the endpoint again, so the plant's message is the only
thing in the queue. A let-open `RECEIVE_CAP` returns it, the attacker reports a word nothing honest
produces, and the test fails at its own assertion.

No handshake orders the plant and the attacker, and none would help. If the attacker has already
parked as a receiver, the plant's send delivers to it directly, so every interleaving ends with the
message taken by whoever the kernel's gate allowed. The operator then un-parks the plant with a
bounded receive, using the timer and bound notification its spawner granted. There are three cases:

- A kernel that refused the attacker leaves the plant parked, and the receive returns its marker.
- A kernel that let the attacker through leaves the queue empty, and the bound ends the wait, so the
  run fails at the test's assertion rather than at a watchdog.
- On an honest kernel the bound can fire before the plant has sent, since spawn order is not run
  order. The plant then never exits and the reap would wait forever. With a refusal reported, the
  operator bails on its own stage number: an infrastructure failure that names itself.

The run ends with every region back in the operator's budget, and the test asserts on that.

## State handoff

The handoff role differs from the direct one in five ways, each a sentence of §209 (state handoff
is an opaque blob over a granted frame, and it is optional):

- It has no device. The state is the point, and a device would put it behind the same `x86_64` gap
  as the other systems (`hot_swap_contract::probe_device`'s x86 arm), so it runs on all three
  architectures.
- One frame is routed to every instance under the handoff role at build time. The operator never
  maps it and never reads it; the bytes are the component's.
- The replacements' `control` is a different endpoint from the incumbent's. Both are built from one
  declaration whose role name is the component's while the object is the operator's, so a resume
  meant for the incumbent cannot reach a replacement.
- The incumbent is retired only after the replacement says it absorbed the state. Before that,
  nothing has committed.
- A replacement that refuses leaves nothing behind. It never received a request; it exits, and the
  reap returns its region, so the new grant is revoked by the machinery of §16 (object revocation:
  reclaim the objects a process built) with no rollback of its own. The incumbent is told to resume,
  holding the state it never lost.

## The hung incumbent

The hung channel's wiring is the direct channel's, declarations, objects and rights alike. What
differs is one start argument, and a start argument carries no authority. The incumbent's region
capability is kept, unlike every other child's, because it is the authority that ends a child that
will never cooperate (milestone 133 (ending a permanently blocked thread, and deciding who may)).
The replacement and the stand-in that carries the bounded quiesce are both built ahead, since a hung
component gives the operator no drain to hide a build behind.

What the supervisor can see is two negative results. `abi::rendezvous::SURVEY` (milestone 126 (the
`procps` package)) shows every member `BLOCKED` and none `DEAD`, which is also what a healthy idle
system looks like. `Rendezvous::REAP` on every member is refused `StillAlive`, and that is the
supervisor's whole vocabulary over its domain since §32 (a supervisor may collect a corpse without
being able to build one). Asking about every member rather than the suspect is easier, because a
survey returns tids and nothing that says which is which, and it is the stronger claim. The verdict
comes from the clock, not from this view.

The stand-in goes first. It is parked on the stable endpoint's sender queue holding a quiesce, and
left there it would drain the replacement the moment it started. Destroying its region finishes it
in place and unlinks it from the queue, so the request is never delivered.

Recovery needs no authority the operator did not already hold, although §32 says a supervisor that
must restart a hung child "still needs the stronger right". For restarting the service that is not
so. The device comes back with `PageFrame::REVOKE`, `GRANT`-gated take-back (§41) that asks the
holder for nothing. There is nothing to drain, because a component that is not receiving has already
done what `OPERATION_QUIESCE` exists for. The replacement parks in `RECEIVE_CAP` and picks up whatever
queued behind the silence, because the kernel's sender queue is the buffer.

Reclaiming the hung incumbent is where §32's stronger right was right, and the right is the kept
region capability. `DESTROY` finishes the `Blocked` incumbent without waking it
(`sched::finish_blocked_resident`). Freeing it answers every caller it held a reply capability for
with `abi::Error::Gone` (milestone 254 (a caller stranded by a server that died),
`sched::strand_callers_of`), so the client whose request it swallowed asks again and is served by
the replacement. The destroy comes after `STARTED`, so that second attempt finds a server. The
incumbent sends no death message, since it was finished rather than killed, and the revoke has no
fault receipt on this channel, since the holder never runs again.
