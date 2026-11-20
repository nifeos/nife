---
status: DECIDED
raised: 2026-10-03
decided: 2026-10-03
ratified_by: calef
---

# 245. A `CALL` server tells a Reply from a delegation by a kernel tag in `x4`, read through a typed runtime helper

*Section number and slug provisional until the merge queue lands them. Written 2026-10-03 (UTC) by
the lane for milestone 706 (a `CALL` server can tell a Reply from a delegation) as the maintainer's
delegate. The options with their costs are in the
[milestone's block](../roadmap/0706-a-call-server-can-tell-a-reply-from-a-delegation.md).*

calef, 2026-10-03 (UTC): *"Option 1 with the typed runtime helper."*

## The ruling

### 1. The kernel tags a `CALL`'s Reply

`RECV_CAP` returns `abi::rendezvous::REPLY_DELIVERED` (value `1`, name provisional) in `x4` exactly
when the kernel installed the one-shot Reply it minted for a `CALL`, and `x1` is the slot it landed
in. Two paths write it, both in shared scheduler code: `ipc_call_badged`'s rendezvous arm (the
server was parked) and `ipc_recv_cap`'s collect of a parked caller. Every other delivery leaves `x4`
as before: `0` for a `SEND_CAP` delegation, a plain `SEND`, an interrupt signal and a §26 (the fault
endpoint) death message, and `notification::BOUND` for a bound notification. A `CALL` whose Reply
did not fit in a full table arrives with `x1 == NO_CAP` and `x4 == 0`, since there is nothing to
name. `x4` is written only by the kernel, so no sender can forge the tag. No new system call number and
no new method: this is a value in a register `RECV_CAP` already returns, within §10 (the
capability-based microkernel process model) and §12 (call/reply IPC).

### 2. The runtime types it

`user_mode_runtime::recv_request` reads the five words and returns a `Request` whose `delivered` is
a `Delivered`: `Reply(Reply)`, `Delegation(slot)` or `Nothing`. `Reply` has no public constructor,
and `user_mode_runtime::reply` takes a `Reply` by value, so code built on the runtime cannot answer
through a slot a client delegated, and cannot answer twice. `Delivered::into_reply` deletes a
delegation, so a server that did not ask for one does not keep its slot. `recv_request_bound` is the
same for a thread with a bound notification. All five names are provisional.

### 3. Every `CALL` server in the tree receives through it

They were converted in the same change: 25 files beside the runtime, listed in milestone 706's
block.

## Why

A `CALL` server's only guard was `x1 != NO_CAP`, and a delegation passes it. Answering that slot
runs method `0` of whatever object the client sent: `SEND` on a rendezvous, which parks the server
for the life of the machine if nobody receives there, and every client of the server with it. Not
answering it leaks one of 32 slots per message. Found by the 2026-10-03 security audit's follow-up
(finding 11), severity medium: a denial of service on any `CALL` server by any of its clients, not
an escape.

The tag is the shape this register already took for `notification::BOUND` (milestone 151
(notification objects)), for the same reason: `x0` is the sender's, `x4` is the kernel's. It is one
patch in shared code for all three ISAs, and reversible: a server that never reads it behaves as it
did.

The tag alone is rung three of the ladder: each server has to remember to test it. The typed helper
is what moves it to rung one for every program built on the runtime, because the compiler refuses
`reply` on anything but a `Reply`. That is the half calef added to the recommendation.

## What else was considered

- Option 2, an endpoint that refuses `SEND_CAP`. A rendezvous attribute or a withheld rights
  bit, refused at the `SEND_CAP` arm, the shape milestone 603 (an interrupt's endpoint refuses every
  send) took for `SEND`. The strongest: the wrong state cannot arise for a server that opts in. Not
  taken now because it is a new attribute or rights bit, which is a wire-format change and not
  reversible in the rights word, and it changes every server's creation site. Kept open as the
  rung-one answer for a server that asks for it, and recorded as milestone 706's follow-on.
- Option 3, `reply` refuses a non-Reply. Renumber `abi::reply::REPLY` off `0`, or make the
  Reply arm the only one that answers it. Refused. It renumbers a method every program is
  written against, and it closes only the blocking half: the delegated slot still leaks. The fix
  belongs at the receive, where the server learns what it holds, not at the reply.
- The tag without the helper. Option 1 as first proposed: each server checks `x4` before
  `reply`. Cheaper by one type, and it leaves the check for each server author to remember, which is
  the failure the audit found. Refused by the ruling.

## What it costs, measured

One constant, one helper and two stores in `kernel/src/sched.rs`; the rendezvous store writes a
computed word where it wrote a zero. `script/icount` and `script/fastpath-footprint` in CI are what
price that on the `CALL` round trip.

## What is still open

- The check lives in the server's runtime, not in the kernel. A program that issues `RECV_CAP`
  through its own `invoke` and reads `x1` raw is as exposed as before. Every server in the tree uses
  the runtime; nothing gates a new one that does not. Option 2 is the answer that does not depend on
  the server.
- A `CALL` server that receives with plain `RECV` sees the tag on one arrival order only. When
  the server is parked, `ipc_call_badged` installs the Reply and tags it whichever receive the
  server made; when the caller is parked, a plain `RECV` leaves it blocked and installs nothing (the
  standing advice: serve `CALL` endpoints with `RECV_CAP`). No server in the tree does this.

## Evidence

`system_tests::user::recv_cap_attack_tests::a_call_server_tells_its_reply_from_a_delegation_on_both_arrival_orders`
takes a delegation and a `CALL`, on both arrival orders, with a server that trusts only the tag. Its
replayable falsification zeroes the two stores; replayed on aarch64 on 2026-10-03 it fails at the
predicted assertion: the server deletes the Reply as a delegation and the caller is never answered.
