# The hung component: what the test shows, and why the wedge is silent

*Name: provisional.*

An appendix to [`notes/hung-component.md`](../hung-component.md). It records the six results of
`swapper`'s `ROLE_HUNG` case and why its wedge takes the shape it does.

## The six results

`a_supervisor_notices_a_hung_component_and_reclaims_it` runs on all three ISAs: `ROLE_HUNG` where
there is a device page to lend, `ROLE_HUNG_WITHOUT_DEVICE` on `x86_64`, where there is not. The
incumbent swallows one request and stops answering. Six results:

1. The operator notices on its own. Its `OPERATION_QUIESCE` goes unanswered within its bound
   (milestone 106 (a wait that ends on either the interrupt or the deadline)'s timed wait), the
   drain step is asserted absent, and the verdict precedes everything the operator does about it.
   How the bound was chosen is in [what-counts-as-hung.md](what-counts-as-hung.md).
2. The domain does not report a hang. `SURVEY` reports every member `BLOCKED` and none `DEAD`,
   which is byte for byte what a healthy idle system reads as: `abi::survey::BLOCKED` is the state of
   a server parked in `RECEIVE_CAP`, and that is every healthy server between requests. The verdict
   in result 1 had to come from the clock because this view cannot give it.
3. The supervisor's reap vocabulary is refused. `Rendezvous::REAP` is asked about *every* member of
   the domain and answers `StillAlive` every time. It is side-effect free by construction:
   `reap_supervised` decides `StillAlive` before it looks up a region.
4. The service is restored with no new authority, per the four-step table in the parent.
5. The incumbent's region comes back while the incumbent is still hung. The budget's live-children
   count falls by exactly one instance across the `DESTROY`, and the incumbent leaves no death
   message, because a thread finished in place (milestone 133 (ending a permanently blocked thread,
   and deciding who may)) never runs again.
6. The caller it stranded sees `abi::Error::Gone` (milestone 254 (a caller stranded by a server that
   died)) and asks the replacement again. The client's `CL_WAS_GONE` bit is its own testimony, and
   both witnesses put the version change at the swallowed request.

Only result 1 involves a clock, and it is not asserted on: against a hung incumbent any bound is
eventually reached. The bound decides whether a *healthy* incumbent would be convicted, which is a
measurement, not a test.

## Why the wedge is silent, and when it fires

The wedged instance takes request `WEDGE_SEQ`, sends the ordinary swap trigger (`NOTE_SWAP_NOW`, the
three words a healthy incumbent sends after `SWAP_TRIGGER` requests), and parks in `RECEIVE` on its
control endpoint, which nothing in the hung system sends to. It keeps the caller's `Reply` and never
writes the log page for that request. Three properties fall out:

- The operator learns nothing a healthy run would not tell it. The trigger carries no hint, and the
  operator's next act, the bounded quiesce, is the same whatever the incumbent is doing.
- It is the commonest real hang: blocked awaiting a peer that will not speak.
- The trigger is sent at the swallowed request, not earlier, for determinism. The quiesce must
  reach the endpoint after that request was taken; sent earlier, the quiesce could overtake the
  client's next call in the FIFO, a healthy-looking incumbent would answer it, and the run would test
  the healthy path. That timing is the fixture's, and the operator never uses it.

The wedge fires on the identity of one request (`WEDGE_SEQ`), not after a count of them. A count
would depend on how far the conversation had got when the operator was ready, which is a race; a
request identity happens in the same place on every architecture under any scheduling.

The members' `BLOCKED` states are established long before the survey: the incumbent parks a few
instructions after its trigger, and the survey runs a quiesce bound later. That is a margin, not a
proof, and the survey assertion says what a failure there would mean.

## Why the quiesce has a stand-in

A `CALL` takes no deadline. A bound notification ends a `RECEIVE` (milestone 106), and nothing ends
a caller parked on a sender queue or awaiting a reply. So the operator starts a
`swap_boundary_witness` instance as `ROLE_QUIESCER` to make the one call, and waits with a deadline
on the answer it forwards. When the deadline wins, the stand-in is still queued on the stable
endpoint holding a quiesce request, and the operator destroys its region before starting the
replacement, which would otherwise receive that request and drain itself.
