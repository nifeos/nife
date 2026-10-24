# `std::net` over the socket contract

*An appendix to [`notes/std.md`](../std.md), which is the page to read. This file holds how
`TcpStream`, `TcpListener` and `UdpSocket` map onto net_stack's socket contract, and the port-reuse
finding. The directory `notes/std/` and this file's stem are provisional names. Naming is calef's.*

The records this file cites by number:

- milestone 27 (Rust `std` on the native ABI)
- §25 (socket identity)
- milestone 64 (enough `std` to run somebody else's crate)
- milestone 55 (Time Machine)


## `std::net` over the socket contract (milestone 27 phase two)

`sys/net/connection/nife.rs` binds std's `TcpStream` and outbound `UdpSocket` to net_stack's socket
contract (DECISIONS §25, notes/net.md, `crates/socket_contract/src/lib.rs`). The PAL is a client of
the frozen contract, nothing more. It holds the `Stack` endpoint (slot 2) and a frame untyped (slot
3). An open is a `CALL` on the `Stack` rendezvous, answered with the socket's own capability (§255
(each socket is its own capability), since 2026-10-07 UTC). The PAL hands that socket a shared
`PageFrame` it minted and mapped (`SEND_CAP`, `OPERATION_ATTACH_PAGE_FRAME`), and then drives the socket
with `CALL`s on its capability. Control words ride the message; bytes sit in the shared frame. The
PAL's socket ids are its own registry's indices and never reach net_stack. This is the exact path the hand-written
`socket_test_client` client walks, reached through std's blocking API instead.

The wire constants are not restated: `netproto.rs` is generated verbatim from
`crates/socket_contract/src/lib.rs` into `sys/pal/nife/netproto.rs` by `std-src`, the same
anti-drift discipline as `abi.rs`. If the contract changes, the PAL's
numbers change with it, because there is one source.

What binds, and how it maps to the contract:

- `TcpStream::{connect, read, write, ...}` -> `OPERATION_OPEN_TCP`, `OPERATION_CONNECT`, `OPERATION_RECEIVE`, `OPERATION_SEND`,
  `OPERATION_CLOSE` (on `Drop`). `read` blocks in net_stack until data arrives (a blocked `RECEIVE`), the
  blocking semantics std's default API wants. A short `read` keeps the segment's tail in a
  per-socket residual buffer, so a stream never drops bytes.
- `UdpSocket::{bind, connect, send, recv, send_to, recv_from}` -> `OPERATION_OPEN_UDP`, `OPERATION_SENDTO`,
  `OPERATION_RECEIVE`. UDP `connect` only fixes a default peer (no contract call, matching Unix). `bind`'s
  local address is validated but not honored: net_stack assigns an ephemeral local port.
- Errors map by meaning, no errno. A refused TCP connect is `ConnectionRefused`; a net_stack timeout
  on `RECEIVE` is `TimedOut`; a datagram larger than the frame is `InvalidInput`; an IPv6 address is
  `Unsupported` (net_stack is IPv4-only). A `CALL` on an empty `Stack` slot (no network granted)
  reads back negative and becomes `Unsupported`, the same answer a program with no net grants gets.

- `TcpListener::{bind, accept}` -> `OPERATION_LISTEN` and `OPERATION_ACCEPT` (milestone 64). This is the inbound
  half, and the reason it reads differently from everything above is that a listening port is an
  authority this program was granted or was not. `net_stack` is spawned with a listen grant, an
  inclusive port range, and refuses `LISTEN` outside it; `NO_LISTEN_GRANT` is the default, so a std
  program on a stack nobody granted ports to is refused every port there is. The three contract
  answers map to three `ErrorKind`s that tell a caller three different things to do. `LISTEN_DENIED`
  is `PermissionDenied` (ask whoever spawned you; no other port will help), `LISTEN_IN_USE` is
  `AddrInUse` (pick another port), and a refused `ACCEPT` is `WouldBlock` (the listener is still
  armed, call again). The `std_net` test boot runs this as its negative control: the same binary
  prints `listen refused` on a stack granted no ports, on aarch64 and riscv64.

  A listener and a connection are two capabilities under two PAL ids, and the listener never gets a
  frame. That is DECISIONS §25 showing through the PAL rather than a choice made here: the shared
  frame is the granted resource and a listener carries no bytes, so it has nothing to grant.
  `OPERATION_ACCEPT` answers with the connection's own capability, which `accept` files under a
  *second* id and hands that id's frame, so the POSIX move of turning a listening descriptor into
  the connection in place is not expressible from this PAL.

  The peer address is `0.0.0.0:0`, and it is a placeholder named as one. `accept` must return a
  `SocketAddr` and the contract's reply carries no peer; reporting the real one is a wire change and
  therefore a fork rather than a PAL decision (notes/net/std-tcp-listener.md carries the two options).

The concurrency model is the contract's: single-threaded, one synchronous exchange at a time. A
program can hold up to six sockets at once (the PAL's `SOCKETS`) and interleave them, but there is only ever
one operation in flight, which is all a single-threaded process can do anyway. For a listener that
means the backlog is one connection deep: the listener re-arms inside `ACCEPT`, so serving
connections one after another works indefinitely, and serving two at once does not.

Capability slots, counted from what the PAL holds (milestone 649 (every client of a network stack
shares its socket numbers)). Each open socket costs one slot for its capability. Each PAL id that
has ever carried bytes keeps one more, for its page's frame, which the next socket at that id is
handed a copy of. So a program holding all six sockets spends twelve of its 64 slots
on the network, and one that has closed them all still spends six.

net_stack assigns ephemeral local ports with a rotating allocator, independently of the socket, and
a socket has no number at all under §255 (each socket is its own capability). So a socket's local
port does not follow its id, and reopening one does not reuse the port it had.

## BUGS

- One operation per socket at a time is assumed, not enforced. The registry is behind a spinlock,
  but a socket's shared frame is not locked: only the running operation touches it between its own
  `CALL`s. That held while a std process had one thread. Under `std::thread` (see
  [threads](threads.md)), two threads using one socket at once collide in its frame:
  - Two writers overwrite each other's payload, because `write` puts the bytes in the frame and
    then `CALL`s.
  - A read and a write trample each other, because `read` `CALL`s and then copies out.
  - Two readers can each take one `RECEIVE` reply and read back each other's segment.
  - Two UDP `send_to` calls share the frame's destination header, so a datagram can go to the
    other thread's address.

  Distinct sockets have distinct frames and do not collide. Only a program that holds the process
  capability, and so can spawn, and that shares one socket through `Arc` or `&` is exposed.
  `try_clone` refuses with `Unsupported`, so the usual split of a reader and a writer on two
  handles fails loudly rather than racing. The fix is a per-socket lock around each frame exchange.
