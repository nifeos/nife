# The hung component: the caller a hang strands

An appendix to [`notes/hung-component.md`](../hung-component.md), whose question 3 it supports.
Milestone 133 (ending a permanently blocked thread, and deciding who may) lets the region holder end
a blocked server. Milestone 254 (a caller stranded by a server that died) wakes the callers of a
server that dies with `abi::Error::Gone`. A server that is hung and alive still strands its caller
exactly as below.

The path, from the code:

- `sched::ipc_call` parks the caller with `WaitRole::Reply` and says so at the site: *"We are NOT
  queued as a receiver; the Reply capability, which carries our tid, is the only thing that can wake
  us."*
- `sched::ipc_reply` is the only site that wakes such a thread, and it is addressed by tid rather
  than through an endpoint's wait queue.
- The abort machinery that produces `Gone` (`set_ipc_aborted`, and `reap_region_objects`'s endpoint
  sweep calling `drain_waiters`) reaches threads on an endpoint's wait queues. A caller whose
  request has been taken was popped off that queue at the rendezvous. It is not there.

So there are three ways a blocked caller ends, and a hung server is none of them: its endpoint is
destroyed (`Gone`), its server replies, or its server replies. `Gone`'s own doc comment is accurate
and the accuracy is the problem: *"the capability names an object that no longer exists."* A hung
server's endpoint exists. A hung server exists.

And the operator cannot answer on the component's behalf. The one-shot `Reply` capability naming
that caller is minted `WRITE` without `GRANT` (§12, and the syscall layer says why: "minted without
`GRANT`, so it could not have been delegated here in the first place"), it lives in the hung
component's capability table, and it is consumed on use. It cannot be delegated to the supervisor,
forged, or reached by revoking anything. Freeing a stranded caller requires the cooperation of the
component whose lack of cooperation is the definition of the hang.

The test demonstrates that inversion rather than asserting it. `swap_protocol::NOTE_RELEASE` is the
operator's reply to the wedged instance, and the instance then uses the reply capability it took to
answer `WEDGE_RELEASED` to the caller it stranded. In the test the wedge is deliberate and
cooperates; a real one does not, and the `CL_WAS_RELEASED` bit exists so a reader can tell which of
the two happened.

Consequences a design has to carry, not a footnote:

- ~~A stranded caller's region was unreclaimable too, so one hang cost two.~~ Untrue since
  2026-09-04 (see the correction at the top).
- This is not new with hangs. It is true of any server killed mid-`CALL`, including §24's forcible
  `^C` tier applied to a server. Nothing in the tree recorded that on 2026-08-17. It was the most
  transferable finding in this note and it wanted its own lane, which milestone 254 became.
