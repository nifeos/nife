# The network stack: listening, accepting, and the inbound gate

*An appendix to [`notes/net.md`](../net.md), which is the page to read. This file holds the two
design questions `LISTEN` and `ACCEPT` raised, the concurrency they ship with, and the host-to-guest
gate. It exists to verify or challenge the main page. A reader who only needs to use the socket
contract should not have to open it. The directory `notes/net/` and this file's stem are provisional
names; naming is calef's.*

*Records cited below: milestone 32 (a real filesystem), milestone 54 (a network file service a Mac
can mount), milestone 55 (Time Machine), milestone 64 (enough `std` to run somebody else's crate),
§25 (socket identity) and §50 (namespace composition).*

## A listener is not a connection, because they are not the same authority

(Prior art and one correction to the wording below: see "Prior art for the *inbound* half" in [prior-art-and-the-contract.md](prior-art-and-the-contract.md).)

POSIX makes a listening socket and an accepted one both file descriptors; the only difference is
which calls happen to work on each. That conflation is why "give this program port 80" and "give this
program this connection" are the same kind of grant there, and it is not a shape this tree should
inherit.

Here they are two objects:

- A listener names a port. It is exclusive (two programs cannot both hold 80) and it carries no
  shared frame at all, because no bytes ever cross on it.
- A connection names a peer. It is the object `CONNECT` already produced, reached from the other
  direction, and the shared frame is its real granted resource exactly as §25 says.

The frame is what makes this more than a naming preference. §25 already decided that the per-socket
shared frame is the granted thing and the socket id is bookkeeping. A listener has no frame, so under
that decision it has nothing to grant, and the split falls out rather than being imposed. This is the
tree's existing habit of splitting authority by what a holder can *do* rather than by what it names,
which is `PageFrame` versus `DeviceFrame` and `WRITE` versus `GRANT` on one object.

So `ACCEPT` carries the socket id to install the connection at (`CALL(req(OPERATION_ACCEPT, lsid),
target_sid)`), the client must have attached a frame there first, and `target_sid == lsid` is
refused. The contract will not let a listener become a connection in place. The listener keeps its
id, its port, and its authority.

The client-side proof costs nothing and is worth having: `TEST_TCP_LISTEN_GRANT` in
`components/src/socket_test_client.rs` attaches no frame anywhere and still listens, binds and collides.

## Who binds the port: the spawn service grants a range, the client does not ask

Outbound needs no permission beyond the `Stack` capability, and that is not laxity. An ephemeral
local port is allocated by `net_stack`'s own rotating allocator and contended by nobody, so there is
nothing to arbitrate. Inbound is different in kind. A listening port is a claim on an exclusive
name in a shared namespace. That is the same property that makes a directory a capability here
rather than a path (milestone 32's FS server) and the same reason `bind` (§50) is a grant.

So the port is not the client's to pick. `wire_net_server` spawns `net_stack` with a listen
grant, an inclusive range packed by `socket_contract::listen_grant` and carried in the spawn's `arg2`;
`LISTEN` outside it replies `LISTEN_DENIED`. Every outbound test passes `NO_LISTEN_GRANT`, which is
also the default, so a net server that was never told which ports it may serve refuses all of
them. Inbound authority is granted, never assumed. `LISTEN_DENIED` is deliberately a different
reply from `LISTEN_IN_USE`, because "you were not granted this" and "somebody else has it" call for
opposite responses from a client.

The honest limit, and it is the interesting one. The grant's granularity is the `Stack` endpoint,
not the client, because an endpoint carries no sender identity. `net_stack` cannot tell two callers
apart, so two clients sharing an endpoint share its grant. In this tree each stack endpoint has
exactly one client today, so nothing is wrong yet. But a real multi-client net server needs a
minted endpoint per client and the grant then rides on that. That is the same deferred step §25
already recorded for socket identity, arriving from a second direction. That is itself the finding:
the thing that would make a socket an unforgeable object is the same thing that would make a port
grant per-client. If that step is ever taken, it should be taken once, for both.

## The concurrency model, and what "accept" had to mean to be worth shipping

The roadmap block warned that adding `LISTEN` to a stack handling one exchange at a time produces a
server that accepts a connection and then cannot accept the next one. smoltcp has no accept queue: a
`tcp::Socket` in `Listen` state *becomes* the established connection, and there is no separate
listener object to accept from again.

The answer is that `ACCEPT` re-arms. It hands the established socket to the target id and
immediately creates a fresh `tcp::Socket` parked in `Listen` on the same port under the listener's
id, before returning. Two smoltcp sockets then share a local port, one established and one listening.
That is safe because smoltcp's dispatch refuses to hand an ACK-bearing segment to a `Listen`-state
socket and refuses a segment from the wrong peer to an established one.

What that buys, honestly stated:

- A listener serves connections one after another, indefinitely, and does not go deaf after one.
- A handshake that arrives while the client is busy on an earlier connection completes underneath
  it, because `net_stack` drives the poll loop inside every blocking operation. It is waiting when
  the client next calls `ACCEPT`.
- The backlog is one connection deep. A second peer arriving in the window between a handshake
  completing and `ACCEPT` re-arming gets a RST rather than a wait.
- Two connections cannot be served *simultaneously*, because the client blocks in one `CALL` at a
  time. That is the phase-one limit recorded on the [main page](../net.md), unchanged.

For milestone 54's file service that is the difference between usable and not: a Mac mounting a share
opens connections in sequence. For milestone 55's SMB3, which uses several at once, the remaining
step is real concurrency (userspace threads, or a select-shaped wait), not another verb.

## The gate: a host process connects to the guest, twice

`hostfwd` is `guestfwd`'s mirror, so this costs no host setup: QEMU listens on a loopback port and
forwards connections to the guest's `10.0.2.15:7778`. The runners add it to the mmio NIC only,
and only when `NIFE_HOSTFWD_PORT` is set, which is the test flow and nothing else. That flag is
the one thing here that binds a port on the developer's machine, so it stays off a plain
`cargo xtask run` and off the benchmark boot, both of which share the runner.

The port is chosen by xtask, from the OS (`free_loopback_port`), rather than fixed. A fixed port
collides every time two lanes run the suite at once on one machine. An asked-for one collides only if
it is taken between the ask and QEMU's bind, and that failure is loud (QEMU refuses to start).

The host side is `InboundProber`, a thread beside the scanout referee and there for the same reason.
Nothing inside the guest can open a connection to the guest from outside it. It connects, sends
`nife-in!`, and requires `nife-out!` back. The two strings differ on purpose. An echo would pass
even if the guest were only reflecting our bytes, and the claim is that the guest *composed* an
answer to a connection it did not make. It retries for the whole run, because nothing on the host
knows when the accept test starts. A connection that lands while another net test holds the NIC finds
no listener and is reset by smoltcp, which costs nothing.

It requires three of the four connections the guest offers, and the gap is deliberate. Two
listeners serve two rounds each, neither can supply more than two, so a host that collected three
collected at least one from each. The second round of each pair is the load-bearing one. A listener
that accepts once and goes deaf would pass a one-connection gate, and is exactly what a file server
cannot use. (It required two, from one listener, until milestone 64 added the `std::net` half; see
the BUGS history in [the-inbound-check.md](the-inbound-check.md) for why the floor is three and not four.)

A host prober must never abandon a connection because it is slow. The first green run
taught by failing in the most instructive way available. The guest passed and the prober reported
serving *zero*. A `connect` to a `hostfwd` port succeeds the moment QEMU accepts the host side. slirp
only then starts the guest side, and nothing answers that SYN until the guest is inside an operation
that polls smoltcp. Dropping the connection on a read timeout does not take back the payload
already written. slirp keeps the guest-side connection, completes the handshake whenever the guest
next polls, and delivers those bytes to a socket whose host end has gone. One retry every 100 ms for
a whole boot builds a queue of those. The guest cheerfully served both of its rounds from
abandoned connections while the prober timed out on its own live one.

The fix is a rule rather than a longer timeout. A timeout is not a reason to give up (keep
reading the same connection until it answers, dies, or the run ends). A hard error is, and
a cheap one, because a RST means the guest had no listener and consumed nothing. The general shape is
worth remembering for any host-side actor: an abandoned request may still be executed, so "I gave up
waiting" and "it did not happen" are different claims.

Both halves assert. The guest reports `OK` only if both rounds arrived with the right bytes and its
answers were sent (`a_host_process_connects_to_the_guest_and_is_answered`, both ISAs). The prober
fails the leg if what came back was not the guest's answer. The guest's half covers "somebody
connected"; the host's covers "and got the right answer", which the guest cannot know.

The grant half rides in the same exchange, ahead of the first connection. 8080 is refused as a
matter of authority, 7778 binds, and asking for 7778 again on a second socket id reports
`LISTEN_IN_USE`. No frame is attached until all of that has passed, which is the two-object claim
proved by construction rather than by assertion.

## What else rides the inbound gate's spawn

`a_host_process_connects_to_the_guest_and_is_answered` is one test with stage codes, not several,
and the codes stand in for the names the others would have had. When it was written, a second net
server cost a region nothing reclaimed, and the boot had none to spare (`virtio::MAX_DEVICES`). A net
service has been reclaimable since 2026-08-16 (notes/frames.md), and splitting the test is a change
of its own, with its own argument.

The UDP bind grant (milestone 55's stack half) rides the same spawn for the same memory reason. The
lane that built it measured it: a twelfth net server died as `Unmappable(OutOfPageFrames)` in an
unrelated later test. After the accept rounds the client binds a port outside `udp_bind_grant`, which
is refused, then the granted one, which binds and is exclusive. The spawn's word carries both
grants, so the composed packing is what the machine exercises.

`LISTEN_DENIED` is a distinct answer from `LISTEN_IN_USE` because the two call for opposite
responses from a client: no other port helps the first, and retrying helps the second.

Nothing in the suite checks that a datagram addressed to a joined multicast group is accepted, or
that a multicast `SENDTO` reaches the wire. The multicast DNS responder carried both checks, and it is
gone (notes/mdns.md). The source endpoint a UDP `RECEIVE` reports is still proved, by the TFTP
exchange (stage codes 0xE046 and 0xE047).
