# What the machine already has, which reframes the question

*Name: provisional. Named after the parent's own heading.*

[`notes/blocked-thread-teardown.md`](../blocked-thread-teardown.md) summarizes this finding: the mechanism to end a blocked thread was nearly free, and the
authority is the whole problem. This is the reading of the kernel behind it, as it stood on
2026-08-17. The parent's opening paragraphs say what
milestone 133 (ending a permanently blocked thread) found wrong in it.

The most useful finding in this lane is not from the prior art. It is that the mechanism is nearly
free and the authority is the whole problem, and that is the opposite of how the question reads
from outside.

## Every blocked thread is in exactly one of three places, and the kernel knows which

`thread::WaitRole` is a closed enumeration of three, written by the same `SCHED`-held statement that
writes `Blocked`:

| `WaitRole` | Where the TCB is linked | Reached by |
|---|---|---|
| `Sender` | the endpoint's sender `Fifo` | `Endpoint::receive`, `drain_waiters`, `remove_sender` |
| `Receiver` | the endpoint's receiver `Fifo` | `Endpoint::send`, `Endpoint::signal`, `drain_waiters` |
| `Reply` | no queue at all | `sched::ipc_reply`, addressed by tid |

There is no fourth. An `Irq::WAIT` is `sched::ipc_receive` on a routed endpoint, so it is a `Receiver`.
`handshake.wait_on` carries `(EpId, WaitRole)` and `endpoint_of` resolves the `EpId`, so from a bare
`Tid` the kernel can say exactly which queue, if any, holds that thread.

## The abort-and-resume pair already exists, and is already used

```rust
set_ipc_aborted(sched, tid);   // handshake.abort(): opens the undelivered-wake gate
wake(sched, tid);              // Blocked -> Ready, onto a run queue
```

Those two lines are what `reap_region_objects` runs against every waiter it drains from a doomed
endpoint. The syscall layer then reads the flag through `take_ipc_aborted` and hands userspace
`abi::Error::Gone`. The abort path is complete; it is only ever entered from an endpoint's wait
queue, which is the whole of `Error::Gone`'s reach.

## The one primitive that is missing is twelve lines

`Endpoint::remove_sender` exists, and it exists for exactly this shape of problem: a corpse
parked on its supervisor's sender queue must be unlinked before its TCB is freed. Its doc calls it
"the one operation an intrusive `Fifo` deliberately does not offer (arbitrary remove)". There is no
`remove_receiver`, and it is the same drain-and-repush over the other `Fifo`.

For `WaitRole::Reply` no unlink is needed at all, because the thread is on no queue. Mechanically the
case the hung-component note frames as hardest is the easiest one.

## And the precedent for reaching outside the region is already settled

The objection that suggests itself is that ending a thread blocked on somebody else's endpoint means
mutating an endpoint the destroyer does not own. The removal phase of `reap_region_objects` does that, deliberately and with its reasoning written down: a corpse parked on its *supervisor's*
endpoint, which "is not in this region and the endpoint sweep above did not touch it", is unlinked
with `remove_sender` before its TCB is freed. Endpoint wait queues are kernel state under `SCHED`,
not the endpoint owner's property, and the tree has decided that once.

So: the kernel could end any blocked thread today, in about thirty lines, with no new syscall.
Every proposal below differs in who may ask and in what the victim and its peers observe, not in
whether it can be done.
