# The network stack as a confined component (milestone 30)

Milestone 30 is three pieces in order: multi-queue transport confinement, a userspace virtio-net
driver behind it, and a TCP/IP stack (smoltcp) in a net server speaking a capability-shaped socket
contract.

Pieces 1 and 2, and phase A of piece 3, are built on both ISAs. The kernel confines the NIC's
DMA to N queues, a userspace driver runs it, and smoltcp takes a DHCP lease over it. Their records
are in [the-driver-and-dhcp.md](net/the-driver-and-dhcp.md). What a client uses is the socket
contract below, decided in §25 (socket identity) and built as phase B.

## Piece 3 phase B: the client-facing socket contract (built, both ISAs)

The §25 contract, so a process other than net_stack can open sockets. net_stack, after DHCP, serves requests
on a `Stack` endpoint; a client holds `WRITE` on it plus its own untyped budget. Files:
`crates/socket_contract/src/lib.rs` (the wire format), the serve loop in `components/src/net_stack.rs`, and the client in
`components/src/socket_test_client.rs` (a module of the net_stack binary, dispatched by the entry role, see the archive
note below).

### A socket is a capability

Since 2026-10-07 (UTC), milestone 649 (every client of a network stack shares its socket numbers)
and §255 (each socket is its own capability). The client's `Stack` capability is a **front door**:
it opens sockets and does nothing else. Each open is answered by `REPLY_CAPABILITY` with the new
socket's own capability, a copy of the stack's endpoint that net_stack badged for that socket, and
every later call is made on it. net_stack names the socket by the badge the kernel stamps on each
request, so a client can reach exactly the sockets it holds capabilities for, and can hand one to
another program by sending the capability. `CLOSE` drops the badge from net_stack's table, so every
copy reaches nothing afterwards. The per-connection shared frame is still the granted resource for
bytes, delegated to the socket's capability by `SEND_CAP` and mapped by net_stack at its table
entry's VA. Before milestone 649 a socket was a small integer carried in the request word, shared
by every client of the stack; §25 (socket identity) had named minted endpoints as the later step.

### Operations

`ATTACH_FRAME` is a `SEND_CAP` on a socket's capability (it carries the frame). The rest are
`CALL`s (which mint the reply cap net_stack answers on), the opcode alone in the request word.
`OPEN_UDP` / `OPEN_TCP` are on the front door, and `SENDTO(len)` and `RECEIVE() -> len` serve UDP
(destination and payload in the shared frame). `CONNECT` / `SEND(len)` / `RECEIVE()` for TCP;
`CLOSE`. A blocking `RECEIVE` is net_stack driving the
smoltcp poll loop (WAIT on the NIC interrupt) until the socket has data, then replying, the disk
driver's discipline one layer up.

### Frame layout, pinned

One data region reused per operation, NOT a split TX/RX ring: the
phase-one contract is one *synchronous* exchange per `CALL` (the client blocks in the CALL while
net_stack drives the network), so a request's payload and its reply never coexist. A split ring becomes
necessary only with asynchronous or streaming sockets, deferred with the concurrency model.

### Concurrency model, phase one

net_stack is single-threaded, with one synchronous exchange per request. net_stack
blocks on the `Stack` endpoint between requests and drives the network inside handling one. This
suits the `std::net` PAL's blocking calls; concurrent connections and listening sockets want either
userspace threads (the TCBs of milestone 19c (run a real workload)) or a select-like wait, the phase-two extension.

### One binary, one archive entry

The client rides in the net_stack binary (a nonzero entry role runs
it) rather than a separate binary, because the nifefs archive directory held at most 15 files at
the time and the initrd was already near that ceiling. (The ceiling is `nifefs::MAX_FILES`, 76
since 2026-08-01; see [nifefs.md](nifefs.md). The decision stands on its own merits, but the
pressure behind it is gone.) A subtlety worth recording: net_stack reports its DHCP
lease with a *blocking* `send`, so the spawn service drains that report before returning, or net_stack
never reaches its serve loop and the client's first request hangs. That was the one real bug in
bring-up, caught by a watchdog hang.


## The inbound half: the guest can be connected to (milestone 107)

Everything above is the guest as a client. The TCP gate connects out to a slirp `guestfwd` peer, the
UDP gate sends a request, DHCP is a client protocol. nife could reach the network and could not
be reached, and the contract had no listen verb to fix that with.

Milestone 107 (the socket contract learns to accept) adds `LISTEN` and `ACCEPT` (`crates/socket_contract`, opcodes 9 and 10, **names
provisional**). It also adds the smoltcp side in `components/src/net_stack.rs`, and a gate in which a **host process
connects into the guest** and gets an answer the guest composed. Two design questions came with the
verbs, and neither was copied from POSIX.

### EXAMPLES: serving a port, end to end

#### Spawn side

Whoever wires the pair decides the inbound authority, and the client never asks for
it. `socket_contract::listen_grant(lo, hi)` packs an inclusive range into the one word `arg2` carries:

```rust
// A stack whose client may listen on 7778 and nothing else.
let report = virtio_service::start_net_stack(
    image,
    NET_TEST_TCP_ACCEPT,                                    // which exchange the client drives
    false,                                                  // mmio, not PCIe
    socket_contract::listen_grant(NET_LISTEN_PORT, NET_LISTEN_PORT),
)?;

// A stack that serves nobody inbound, which is every other net test in the tree.
let report = virtio_service::start_net_stack(image, NET_TEST_TCP_ECHO, false,
                                             socket_contract::NO_LISTEN_GRANT)?;
```

#### Client side

Bind the port, keeping the listener's capability, then accept on it. Each accepted connection
comes back as a capability of its own, and that is what gets the frame. The listener never does:

```rust
// 1. Bind. No frame is attached anywhere, because a listener carries no bytes.
let listener = match call_receiving(STACK, OPERATION_LISTEN, 7778) {
    (LISTEN_GRANTED, _, Some(listener)) => listener,
    (LISTEN_DENIED, ..) => /* this stack was never granted 7778: ask the spawner, not again */,
    (LISTEN_IN_USE, ..) => /* somebody already holds it: pick another port */,
    _ => unreachable!(),
};

// 2. Accept, use, close, repeat. The listener re-arms inside ACCEPT, so this loop keeps working.
loop {
    let (REP_OK, _, Some(conn)) = call_receiving(listener, OPERATION_ACCEPT, 0) else { break };
    send_capability(conn, frame, rights::READ | rights::WRITE, OPERATION_ATTACH_PAGE_FRAME);
    let (len, _) = call(conn, OPERATION_RECEIVE, 0);
    // ... read the request out of the frame at FRAME_VA + OFF_PAYLOAD, write the answer back ...
    let _ = call(conn, OPERATION_SEND, reply_len);
    let _ = call(conn, OPERATION_CLOSE, 0);   // the listener is untouched by this
    cap_delete(conn);
}
```

`components/src/socket_test_client.rs::tcp_accept_inbound` is that sequence with the assertions in it.

#### Running the gate

It is part of the ordinary suite and needs no host setup; xtask picks a free
loopback port, hands it to the runner as `NIFE_HOSTFWD_PORT`, and runs the prober thread itself:

```console
$ script/test                                  # both ISAs, inbound gate included
$ cargo xtask test --arch aarch64              # just the aarch64 leg
```

A plain `cargo xtask run` sets no `NIFE_HOSTFWD_PORT`, so nothing binds a port on your machine
outside a test run.

## The appendices

Everything else is in [`notes/net/`](net/README.md), under §212 (a prose budget). None of it is
needed to use the contract.

| appendix | what it holds |
|---|---|
| [the-driver-and-dhcp.md](net/the-driver-and-dhcp.md) | Multi-queue confinement, the virtio-net driver and its DHCP proof, the smoltcp pin, and smoltcp's first lease. |
| [prior-art-and-the-contract.md](net/prior-art-and-the-contract.md) | seL4, Fuchsia, Plan 9, POSIX, systemd and Capsicum, read before the contract; where this differs from seL4; and the §25 design record. |
| [the-outbound-gates.md](net/the-outbound-gates.md) | What the UDP and TCP gates prove, the ephemeral-port fix, the riscv lost wakeup, and why the UDP gate stopped using the host's resolver. |
| [the-inbound-half.md](net/the-inbound-half.md) | Why a listener is not a connection, who grants a port, what `ACCEPT` re-arming buys, and the host-to-guest gate. |
| [the-inbound-check.md](net/the-inbound-check.md) | The inbound check's intermittent red, as it was investigated before the cause was found. |
| [std-tcp-listener.md](net/std-tcp-listener.md) | `std::net::TcpListener` on this contract, and why the prober requires three rounds of four. |
| [memory-and-reclamation.md](net/memory-and-reclamation.md) | How net servers ran the aarch64 test boot out of memory, and what reclaiming them returned. |
| [the-test-clients.md](net/the-test-clients.md) | What the client roles in the `net_stack` binary prove per spawn, and the hostile squatter's five steps. |

## BUGS

- The DMA region is one 4 KiB page, so the MTU is 576 and a full 1514-byte frame does not fit. See
  [the-driver-and-dhcp.md](net/the-driver-and-dhcp.md).
- `net_stack` handles one synchronous exchange at a time. A listener serves connections one after
  another, never two at once, and its backlog is one connection deep. See
  [the-inbound-half.md](net/the-inbound-half.md).
- While one of smoltcp's timers is pending, `net_stack` yields and polls instead of sleeping, so it
  spins a hart until the timer is due. The clean fix is a timed wait. See
  [the-outbound-gates.md](net/the-outbound-gates.md).
- A listen grant belongs to the `Stack` endpoint, not the client, because an endpoint carries no
  sender identity. Two clients sharing one endpoint share its grant. See
  [the-inbound-half.md](net/the-inbound-half.md).
- An accepted stream reports its peer as `0.0.0.0:0`, because `OPERATION_ACCEPT`'s reply carries no peer.
  Both fixes change what two programs agree on, so both are calef's. See
  [std-tcp-listener.md](net/std-tcp-listener.md).
- `virtio::MAX_DEVICES` never reuses a slot, and a dead net service's DMA page and shadow ring are
  not returned. See [memory-and-reclamation.md](net/memory-and-reclamation.md).
- No artifact says which components may serve inbound ports, where a static seL4 system description
  would. Nothing in `crates/socket_contract` is machine-checked. See
  [prior-art-and-the-contract.md](net/prior-art-and-the-contract.md).

### BUGS: the check fails intermittently, and the mechanism is still not identified

*Identified 2026-09-24: `read-failed` is `EINTR`, a signal on the held read, and the prober now reads
again instead of dropping the connection. The errno, the base rate and the fix are in
[the inbound EINTR appendix](load-sensitive-assertions/inbound-eintr.md). The history is kept as it was
written, in [the-inbound-check.md](net/the-inbound-check.md).*

### What is still not proven, and what is deliberately out of scope

- **`std::net::TcpListener` is bound** (milestone 64 (enough `std` to run somebody else's crate)),
  as predicted above. `bind` is `OPERATION_LISTEN` on a client-allocated socket id, and `accept` is
  `OPERATION_ACCEPT` into another one the PAL allocates and attaches a frame to. The std client's
  stack is spawned with a real listen grant. See "The
  std client is a server too" in [std-tcp-listener.md](net/std-tcp-listener.md), and notes/std.md.
- **Only the mmio transport carries the inbound gate.** A PCIe twin would need a second host port,
  and the transport is orthogonal to the accept path, which the outbound gates already prove over
  both buses. This is the same reasoning that retired the PCIe DNS variant.
- **Inbound UDP is built, and it is a grant of its own** (the mDNS stack half of milestone 55 (Time
  Machine: SMB3 with Apple's extensions, and mDNS)). `BIND_UDP` claims a fixed UDP port the way
  `LISTEN` claims a TCP one. It is checked against a UDP bind grant the spawn site packs into the
  high half of the same spawn word the listen grant rides in (`socket_contract::udp_bind_grant`; the
  halves are independent
  authorities, and the zero word still grants nothing anywhere). It answers with `LISTEN`'s own
  vocabulary because the three outcomes are properties of claiming a port, not of TCP. A UDP
  `RECEIVE` reply carries the datagram's source endpoint in the frame's dst
  fields (dead space on a reply). A responder must see who asked, and RFC 6762 section 6.7 turns on the
  querier's source port. The TFTP gate consumes it by ACKing to the DATA packet's reported
  source, which is what TFTP's TID scheme wanted all along. The stack joins no multicast group:
  milestone 298 (retire the multicast DNS responder and its two crates) removed the join, smoltcp's
  `multicast` feature and the runners' injection hub with the responder, so nothing proves
  multicast. The UDP bind grant and the `RECEIVE` source endpoint are proved by the accept test and
  the TFTP gate. What QEMU could and could not prove about multicast is in notes/mdns.md.
