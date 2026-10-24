# The socket contract crate: its protocol, its waits and its ports

*An appendix to [`notes/net.md`](../net.md), which is the page to read. It carries the design
behind `crates/socket_contract`, whose doc comments link here. The contract's decision is
[§255 (each socket is its own capability)](../../design/decisions/0255-each-socket-is-its-own-capability.md).*

## One capability per socket

A process holds a front door, a `Stack` endpoint that can only make sockets, and one capability per
open socket. `OPEN_UDP`, `OPEN_TCP`, `BIND_UDP` and `LISTEN` are `CALL`s on the front door. Each is
answered by `REPLY_CAPABILITY` with the socket's capability, which the client takes with
`user_mode_runtime::call_receiving`. `ATTACH_PAGE_FRAME` is a `SEND_CAP` on a socket. Every other
operation is a `CALL` on a socket: an opcode, one argument and one reply word. `ACCEPT` answers with
the connection's capability.

The stack names a socket only by the kernel-stamped badge it arrived on. It mints each badge with
`SOCKET_BADGE` set, from a counter that never rewinds, and drops it at `CLOSE`. A request whose badge
has the bit names a socket, and any other badge is a front door. So a closed socket's copies reach
nothing: the badge is in the socket range and no longer in the stack's table. Passing a socket is
passing its capability.

Each socket's capability is minted with `BADGE` from the stack's own endpoint, held a second time at
`stack_slots::MINT` with `WRITE | GRANT`. It is not minted from the `READ` copy the stack serves on,
because a socket minted from that could take other clients' requests.

## The frame

The frame is one data region reused per operation, not a TX/RX ring. Each `CALL` is one synchronous
exchange, so a request and its reply never coexist in it. Its header is a destination address, a
destination port and a payload length, and the payload fills the rest of the 4 KiB frame.

A UDP responder must see who asked: mDNS turns on the querier's port (RFC 6762 section 6.7). So a
UDP `RECEIVE` reply carries the source address and port in the header's destination fields, which
are otherwise dead on a reply. A TCP reply leaves them alone.

## Listeners, connections and grants

A listener and a connection are two authorities. A listener receives connections on one exclusive
port and carries no frame, since no bytes cross it. A connection speaks with one peer. Claiming a
port, by `LISTEN` or a fixed `BIND_UDP`, is a claim on a shared namespace, so it is granted and never
assumed. The stack is spawned with a listen grant and a UDP bind grant packed into one word, and it
refuses a port outside them with `LISTEN_DENIED`. [the-inbound-half.md](the-inbound-half.md) has
who grants a port and why.

`LISTEN` has its own small vocabulary for the same reason `CONNECT` has one. A client that cannot
tell "never granted this port" from "somebody else holds it" cannot report anything useful. The two
call for opposite responses: ask for authority, or pick another port.

## Waiting on a NIC with no interrupt

`polled_wait` decides whether `net_stack` may sleep while it waits on a NIC that raises no interrupt:
the `e1000e`, and radon's `DesignWare` port. A timed wait ends at the first scheduler tick at or after
its deadline. The tick is 100 Hz, as the kernel publishes it on the current-CPU page
(`user_mode_runtime::tick_hz`). So the 1 ms poll sleep the polled NICs ask for lasts up to 10 ms.

On a stream, the next segment arrives one round trip after the stack's acknowledgment, and on a LAN
that is well under a tick. A stack that sleeps once nothing is waiting adds up to a tick to every
round trip. A window's worth of data per tick then becomes the rate
([notes/designware-ethernet/the-receive-rate.md](../designware-ethernet/the-receive-rate.md) has the
measurements).

So for one tick after the last frame the stack keeps looking at the ring, yielding the core between
looks. It sleeps only once a whole tick has passed with nothing. A tick is the bound because it is
what a sleep costs: looking longer can never deliver a frame sooner than sleeping would. The cost is
a busy core while a stream flows. Each look yields, so another thread on that core still runs, but an
idle core does not idle. An interrupt for the NIC, or a timed wait finer than the tick, would remove
it, and neither exists for these NICs.

## Ephemeral ports and the boot clock

A peer that closed first holds the 4-tuple in `TIME_WAIT`: 10 s in QEMU's slirp, 60 s on Linux and
the BSDs. It recycles the tuple only for a SYN whose sequence number is above the old one's.
smoltcp's sequence numbers are random, so a reused 4-tuple's SYNs go unanswered half the time, past
`net_stack`'s 15 s bound. One stack reuses a port only after the range cycles. A fresh stack must
not start on ports an earlier stack used moments ago.

`ephemeral::Ports` walks the private range (RFC 6335). Each allocation takes the later of its cursor
and a clock that advances `PER_SECOND` ports a second. The clock is the boot counter, shared by every
stack, so a later stack starts past an earlier one's ports. RFC 6528 applies the same idea to
sequence numbers, which smoltcp does not offer. `PER_SECOND` is 256, which makes one clock step about
4 ms and keeps the wrap at 64 s, above the 60 s `TIME_WAIT` of Linux and the BSDs. The capture, and
the CI failures it explained, are in [the-outbound-gates.md](the-outbound-gates.md).

The module's BUGS, at length:

- The bound is a rate, not a proof. A stack that opens `n` sockets faster than the clock, and is
  replaced within `n / PER_SECOND` seconds or within one clock step of its last open, can hand its
  successor a port it used. This has not been measured across the suite.
- The range wraps every `SPAN / PER_SECOND` seconds, 64 s, so a port recurs after 64 s even when
  idle. A peer holding `TIME_WAIT` longer, such as Windows at its 240 s default, can refuse it.
- The clock is boot-relative. A rebooted machine's first ports land where the last boot's did at the
  same uptime, and a peer still holding one drops the SYN half the time. That takes a reboot inside
  the peer's 60 s, and a connection in the same 4 ms step. Closing it needs a per-boot offset that
  all of a boot's stacks share, and nothing hands one to `net_stack` today.
- The sequence-number half stays smoltcp's, so a reused 4-tuple still fails half the time. That is
  the generator of milestone 783 (the network stack seeds its random generator from the clock). A
  monotonic initial sequence number needs a change to smoltcp.

## The inbound fixture, and the copy kept out of it

`fixture` holds the inbound gate's shared facts, because four programs must agree on them and three
are separate binaries: `socket_test_client`, `std_exerciser` in its own workspace, and the kernel
test that decides which ports each stack is granted. It is `filesystem_fixture`'s discipline and
rule 7's reason.

The host's copy is deliberately not here. `xtask`'s prober spells `IN_MSG` and `OUT_MSG` again as its
own literals, as the pinned [MS-NLMP] vectors beside the credential tests are spelled twice. The
gate's claim is that two independently written sides agree, and a shared constant would let one edit
move both. The port is spelled a third time in `helpers/qemu-runner-*.sh`, where a `hostfwd` names
the guest side, because a shell script cannot read a Rust crate. A drift in either is loud: the
prober reports "the guest served 0 of N".

The echo peer and the package source live in the fixture for rule 7 too. Two binaries dial each:
the kernel harness's `socket_test_client` and the prompt's `network_echo_client` (milestone 590
(the booted system starts its network stack)) for the echo peer, and `socket_test_client` and
`jig install` (milestone 198 (a package manager) rung 3a) for the package source. The package source
is reachable only inside a QEMU runner, which is [notes/packages.md](../packages.md)'s BUGS rather
than a property of the address.

## The lease report

`net_stack` sends its spawner a three-word lease report once DHCP completes (milestone 384 (in a
capability system the resolver is a grant)). Word 0 is the leased IPv4 address, as milestone 30
(the network stack as a confined component) set it. §248 (the name resolver is its own confined
program) gave the nameserver DHCP hands out a consumer, the resolver's spawner. So the first DNS
server rides in word 1, and a spawner that ignores word 1 is unaffected, which is why the report
needs no second message.
