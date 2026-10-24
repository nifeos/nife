# The network stack: its test clients and the hostile one

*An appendix to [`notes/net.md`](../net.md), which is the page to read. It holds the design behind
the client roles that ride in the `net_stack` binary: `components/src/socket_test_client.rs` and
`components/src/socket_squatter.rs`. The file's stem is a provisional name; naming is calef's.*

## Why one spawn proves several things

Every `net_stack` a test starts holds its untyped region and a virtio slot until reboot, because
nothing reaps a net server ([memory-and-reclamation.md](memory-and-reclamation.md) has the
numbers). The aarch64 test boot has room for few of them. So several client roles prove two or
three claims in one spawn, and distinct stage codes stand in for separate test names:

- `TEST_TCP_ACCEPT` runs the TCP listen grant's refusals, two inbound connections, and then the UDP
  bind grant's refusals. A second net server for the UDP half was measured: a spawn is about 154
  frames nothing reclaims, and the eleventh died as `Unmappable(OutOfPageFrames)` in an unrelated
  later test.
- `TEST_NAME_RESOLUTION` runs the gating exchange against the runners' name server and the
  non-gating one against the host's resolver, and reports both words.
- `TEST_HTTP_PACKAGE` fetches the genuine package and then the tampered copy, and reports both
  verdicts.

Because the TCP and UDP grants share one spawn word, the UDP half also exercises the composed word
rather than one half of it alone.

## The inbound gate

The listener and the connection are two objects, and the client keeps them apart. The grant half
runs with no shared frame anywhere: the frame is attached only after the listener is bound, and only
to the connection. That ordering makes the split visible, since a listener carries no bytes. Two
rounds are served on one listener. The second is the load-bearing one, because only a second accept
proves the re-arm. Why the split exists is in [the-inbound-half.md](the-inbound-half.md).

A round sits through up to five of `net_stack`'s 15 s accept waits (`ACCEPT_TRIES`), 75 s inside the
test's 90 s budget. One wait is not enough. The host prober opens its first connection at boot,
long before the listener exists, and slirp retransmits the SYN with exponential backoff. By the time
the guest listens, the gaps between retransmits are 16 and then 32 seconds, so a single 15 s window
can fall inside one gap and see nothing. On the HVF leg, measured 2026-10-12 (UTC), the first round
was answered 10.3 s into its window from a connection held 31 s. `std_exerciser` re-arms for the
same reason. Milestone 514 (the HVF std listener hangs in some full runs) has the investigation.

`TEST_TCP_ACCEPT_UNHEARD` accepts once on a NIC no host process can reach. The stack's wait must end
by its own 15 s bound with `REP_ERR`. If `nic_wait` did not take the caller's deadline, every retry
built on that bound would retry nothing, and a listener whose peer went silent would hang the suite
into the 60 s watchdog.

## The UDP gates

The TFTP round trip is the gating UDP test because libslirp answers TFTP itself (`tftp=` on the
netdev), so neither the request nor the reply leaves the emulator. It also checks the source
endpoint a `RECEIVE` reports. The port is not pinned to 69: section 4 of RFC 1350 has the server answer from a
transfer-id port of its choosing, so asserting 69 would pin a libslirp detail. The ACK goes to that
reported port, which ends the transfer; without it the server retransmits its DATA at a closed
socket while the next test brings the NIC up.

`UDP_GRANTED_PORT` is 5354 rather than 5353 because 5353 is multicast DNS's port, held by a second
client of this stack until that responder was retired ([notes/mdns.md](../mdns.md)). Nothing in the
tree proves multicast receive or multicast send now. The source endpoint on `RECEIVE` is still
proved, by the TFTP test.

The real-DNS query retries three times (`DNS_ATTEMPTS`). UDP has no retransmit, and a single query
to a real resolver was lost about 2.5% of the time ([the-outbound-gates.md](the-outbound-gates.md)).
Three attempts is ordinary resolver behavior, not a widened timeout.

## Reopening a socket

`TEST_TCP_REOPEN` connects, closes, opens again and connects again. If `net_stack` gave a socket a
local port tied to its place in the table, the reopen would reuse the exact port. A second connect
on a 4-tuple whose slirp flow had not cleared would then stall the stack's bounded poll. The
rotating allocator gives the reopen a fresh port. The second socket also lands in the entry the
first left, with a new badge, so the test exercises an entry's page being taken back and attached
again.

## A one-shot client exits

Every client role ends by reporting and calling `exit`, never by spinning. A spinning client that
nobody reaps stays on a run queue, and enough of them across the socket tests starve a later test
on core 0, because nothing balances threads across cores.

## The squatter

`socket_squatter` holds what any stack client holds: the report endpoint, `WRITE` on the stack's
front door, and an untyped budget. While an honest TFTP client holds a socket open, it tries every
way it has of reaching that socket, in five steps:

1. It attaches its own page through the front door. When a socket was a small number every client
   shared, this landed on the victim's socket.
2. It asks the front door for every operation that names a socket (`SENDTO`, `SEND`, `CONNECT`,
   `ACCEPT`, `CLOSE`, `RECEIVE`). Each must be refused, because the front door names none.
   `RECEIVE` goes last: on an open socket it waits out the stack's bounded wait, so asking it first
   would turn a refusal the test can name into a report that never came.
3. It tries to mint a socket's capability with `BADGE`, which its front door has no `GRANT` for.
4. It opens a socket of its own and tries a kernel `RECEIVE` and `RECEIVE_CAP` on the capability,
   re-badges it, closes it, and tries the closed one.
5. It arms a valid TFTP read request in its page, reports which attempts were refused, and watches
   the page for the victim's traffic, bounded.

Step 4's kernel probes cover a capture by IPC rather than by a page. The capability the stack mints
for a socket is a copy of its own serve endpoint carrying the socket's badge
(`socket_contract::stack_slots::MINT`), and the mint is `WRITE | GRANT` with no `READ`. With `READ`, a
receive on it would dequeue every other client's request to the stack. The kernel refuses a receive
on a `READ`-less endpoint at once (`RECEIVE` and `RECEIVE_CAP` in `kernel/src/syscall.rs`). A
permissive kernel would block there instead, which the test reads as the squatter never arming.
[confinement-outsider-pass-5.md](../confinement-outsider-pass-5.md) records the pass that asked.

`system_tests/src/user/net_confinement_tests.rs` asserts that every attempt was refused, that the
honest exchange went through untouched, and that nothing of it reached the squatter's page.
