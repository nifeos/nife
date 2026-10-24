# The hung component: the caller a hang strands

*Name: provisional.*

An appendix to [`notes/hung-component.md`](../hung-component.md), whose question 3 it supports.
Milestone 133 (ending a permanently blocked thread, and deciding who may) lets the region holder end
a blocked server. Milestone 254 (a caller stranded by a server that died) wakes the callers of a
server that dies or is ended with `abi::Error::Gone`. A server that is hung and alive, and that
nobody ends, still strands its caller exactly as below.

The path, from the code:

- `sched::ipc_call` parks the caller with `WaitRole::Reply` and says so at the site: *"We are NOT
  queued as a receiver; the Reply capability, which carries our tid, is the only thing that can wake
  us."*
- `sched::ipc_reply` wakes such a thread, addressed by tid rather than through an endpoint's wait
  queue. The other waker is `sched::strand_callers_of` (milestone 254), which runs when the thread
  holding the `Reply` departs or its region is reclaimed.
- The abort machinery for wait queues (`set_ipc_aborted`, and `reap_region_objects`'s endpoint
  sweep calling `drain_waiters`) reaches threads on an endpoint's queues. A caller whose request has
  been taken was popped off that queue at the rendezvous. It is not there.

So a blocked caller ends in one of four ways: its endpoint is destroyed, its server replies, its
server dies, or its server's region is destroyed. A hung server that nobody destroys is none of them.

And the operator cannot answer on the component's behalf. The one-shot `Reply` capability naming
that caller is minted `WRITE` without `GRANT` (§12, and the syscall layer says why: "minted without
`GRANT`, so it could not have been delegated here in the first place"), it lives in the hung
component's capability table, and it is consumed on use. It cannot be delegated to the supervisor,
forged, or reached by revoking anything. What the operator can do is end the component, and the
kernel then answers the caller for it.

The hung test runs that path. The operator `DESTROY`s the hung incumbent's region after the
replacement has started; milestone 133 finishes the incumbent in place without waking it, and the
reclaim's removal phase runs `strand_callers_of`, so the client's swallowed request returns `Gone`.
The client asks again and the replacement answers. `CL_WAS_GONE` is the client's own record of it.
The finishing never wakes the incumbent, which is what keeps its stale `Reply` from forging an
answer to a later call (milestone 133's block has the hazard).

Consequences a design has to carry, not a footnote:

- `Gone` is safe to retry only for a request that may be repeated. The client here asks again
  because this fixture's server swallowed the request before doing anything with it. A server
  ended mid-work gives its caller no such certainty, and `Gone` cannot say how far it got.
- This is not new with hangs. It is true of any server killed mid-`CALL`, including §24's forcible
  `^C` tier applied to a server, and milestone 254 answers that caller the same way.
