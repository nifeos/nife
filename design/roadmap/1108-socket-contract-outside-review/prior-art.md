# Prior art

An appendix to [the milestone](../1108-socket-contract-outside-review.md).

How the contract sits against what a reader of other systems already knows. Recent downloads are
the last 90 days at the time of the review, from the crates.io API.

| system | one capability per socket | how a send names its length | what `accept` returns | ephemeral ports | version on the wire |
|---|---|---|---|---|---|
| BSD and Linux sockets | one file descriptor per socket; the descriptor is the authority | an argument of `send(2)`, `sendto(2)`, `write(2)`; the reply is the count accepted, possibly short | a new descriptor and the peer's address | IANA 49152 to 65535 on the BSDs; Linux 32768 to 60999 (unverified); RFC 6056 randomizes the choice | none; `SO_PROTOCOL` and the ABI are fixed |
| Fuchsia `fuchsia.posix.socket` (unverified) | one channel per socket, data over a zircon socket | the length of the write to the data socket | the new socket and the peer address | per stack | FIDL protocols carry versions |
| `smoltcp` 0.14 (2.82M / 10.1M downloads) | a handle into a `SocketSet` | `send_slice(&[u8])` returns the count accepted | a listening socket becomes the connection; the peer is `remote_endpoint()` | the caller's choice | none; it is a library |
| `embedded-nal` 0.9 (449k / 1.54M) and `embedded-nal-async` 0.9 (366k / 1.18M) | a stack-chosen socket handle | `send(&[u8])` returns the count | `accept` returns `(socket, SocketAddr)` | the stack's | none; a trait |
| `embedded-io` 0.7 (80.5M / 181M) | not a socket API; the byte-stream traits the above build on | `write(&[u8])` returns the count | not applicable | not applicable | none |

Where this crate differs: the socket is a kernel capability stamped with a badge, so a closed
socket's copies reach nothing without the stack keeping a free list of descriptors. The inbound
authority is a spawn-time grant word with a proof that the check is exact. The shared page per
socket replaces a copy through the kernel with one through a mapped frame. The ephemeral cursor
follows the boot clock, so a restarted stack starts past its predecessor's ports. What it costs: one
4 KiB page per socket, a single-connection backlog, and no peer address on `ACCEPT` (item 3). It
also costs a send length that travels in the argument word while the layout note says the frame
(item 2). The concrete example is the std platform layer at
`patches/std-nife/overlay/std/src/sys/net/connection/nife.rs` line 496. It calls `CALL(socket,
OPERATION_SEND, chunk)` and never writes `OFF_LEN`. That is what every system in the table does, and
the contract's layout note says it does not.
