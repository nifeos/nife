# The network stack: what the outbound gates prove, and three fixes they forced

*An appendix to [`notes/net.md`](../net.md), which is the page to read. This file holds what phase
B's gates prove, the ephemeral-port fix, the RX-poll lost wakeup, and the UDP gate's move off the
host resolver. It exists to verify or challenge the main page. A reader who only needs to use the
socket contract should not have to open it. The directory `notes/net/` and this file's stem are
provisional names; naming is calef's.*

*Records cited below: milestone 27 (Rust `std` on the native ABI) and milestone 107 (the socket
contract learns to accept).*

## What the gate proves, and what it does not

Both tests are deterministic and zero-host-setup, run over both the mmio and the PCI-behind-IOMMU
transports, on aarch64 and riscv64:

- UDP, `a_client_resolves_dns_through_the_socket_contract`. A real DNS A-query for `example.com`
  to slirp's built-in resolver (10.0.2.3:53); the client verifies the reply is a response (QR bit) to
  its own transaction id. Proves UDP send and receive through the whole path (client, net_stack, smoltcp,
  confined NIC). It relies on the test host being able to resolve DNS, which slirp forwards; a host
  with no resolver would make this time out.
- TCP, `a_client_echoes_over_tcp_through_the_socket_contract`. A full round trip against a slirp
  `guestfwd` echo peer: the runners add `guestfwd=tcp:10.0.2.9:7777-cmd:/bin/cat` to each NIC's
  `-netdev user`, so a guest connection to 10.0.2.9:7777 is piped to a fresh `/bin/cat`. The client
  does OPEN_TCP, CONNECT (the three-way handshake completes against a real peer), SEND a payload,
  RECEIVE the echo and check it byte for byte, then CLOSE (the FIN). No host port is bound and nothing
  outlives QEMU, so the whole round trip, handshake through bidirectional data to teardown, is in the
  committed gate with zero host setup. Verified against QEMU 11.0.2.

Not proven by the gate, when this was written: inbound connections. A `LISTEN`/`accept` verb
plus a QEMU `hostfwd` (host port -> guest) is the way to test the guest accepting a connection, and
that is future work; the contract has no listen verb yet. The concurrency model on the [main page](../net.md) is the other
limit: one synchronous exchange at a time, no overlapping connections.

Milestone 107 built exactly that, and the paragraph above is kept as the record of the gap
rather than edited away. See "The inbound half" on the [main page](../net.md) and in [the-inbound-half.md](the-inbound-half.md) for the verbs, the two design questions they
raised, and what the gate proves now. The concurrency sentence still stands, with one qualification
the accept work earned. A single listener now serves connections one after another without dropping
the next, which is not the same as serving two at once.

This binds milestone 27's `std::net` PAL, replacing its `Unsupported`. Scope discipline held: TCP,
UDP, DHCP, no sockets-API mimicry beyond what the PAL needs.

## Ephemeral ports must be independent of the socket id (a fix the PAL found)

A local port derived from the socket id (`LOCAL_PORT_BASE + sid`) is wrong, and the `std::net` PAL
showed why. A program that opens a TCP socket, closes it, and opens another reuses the same socket
id, so it would reuse the exact local port. Reconnecting to the same peer on an identical 4-tuple
whose slirp flow has not yet cleared makes the SYN go unanswered, and net_stack stalls in its
bounded connect poll forever. Bisection confirms it: a fresh id connects, a reused id hangs.

So net_stack does what any real stack does: it allocates ephemeral local ports from a private range
with a rotating allocator independent of the socket id (`socket_contract::ephemeral`).
Each open advances the cursor, so a just-closed
connection's port is not handed out again until the whole range has cycled. A port a live socket
still holds is skipped outright. Socket-id reuse is then safe: the reopened socket gets a new local
port, a new 4-tuple, and a new slirp flow.

The same stall crosses stacks, and it is slirp's `TIME_WAIT`, measured by capturing the e1000e's
slirp side with `-object filter-dump` (2026-10-13 UTC). slirp closes first in these exchanges and
holds the 4-tuple for 10 s. A SYN reusing it is recycled only when its sequence number is above the
old connection's last one; smoltcp's are random. In the capture every reused SYN above the old
sequence was answered at once, and every one below went unanswered, retransmits included, until the
15 s bound failed the `CONNECT`. Starting every stack at one port failed 9 of 10 filtered name
resolver runs on riscv64 and aarch64. Milestone 801 (packages over the internet) started each stack
at a counter-derived point instead, which made an overlap rare but not impossible: CI hit it on
#1964 (2026-10-12, `thead-c906`, the `std_resolve` lookup) and in run 38357922842 (2026-10-13,
riscv64, `loop.nife.test`). The cursor now follows the boot clock, so a later stack starts past an
earlier one's ports. `socket_contract::ephemeral` has the bound and its BUGS.

The regression is `a_reopened_socket_id_connects_again_over_tcp` (both ISAs): open a TCP socket,
connect to the guestfwd echo peer, close, reopen the *same* id, and connect again; the client reports
OK only if both connects complete. Before the fix the second connect hangs the way the PAL saw.

## The RX poll must honor smoltcp's timers, not just the interrupt (a riscv-SMP lost-wakeup)

`std_net` hung on riscv under the 4-hart boot, watchdog-killed with every core idle and every thread
blocked. Yet the same test passed on aarch64 and the lighter DHCP and TCP-echo tests passed on
riscv in the same run. The shape said lost wakeup; the cause was subtler than a dropped interrupt.

The old server loop blocked on the NIC interrupt between polls: `poll; if done break; WAIT; ack`. That
is fine until an exchange depends on one of smoltcp's *own* timers. smoltcp drives TCP retransmits,
delayed ACKs, and DNS timeouts from its clock, which only advances when we call `poll`. If a segment's
ACK is dropped, the peer goes quiet waiting for our retransmit. Our retransmit is a timer event
that only fires on the next `poll`, which we are not doing because we are blocked on an interrupt the
peer will never send. net_stack waits for the peer, the peer waits for net_stack, both idle. Instrumenting the
PLIC at the hang showed the truth: no source pending, the net source still enabled. Not a masked
line, not a lost IRQ; the device was simply idle, because both ends were waiting on the same stalled
timer. aarch64 happened never to drop a segment (different servicing latency), so it never armed the
retransmit path. The riscv SMP scatter, which moves the driver and its wakes across harts, was slow
enough to drop one and expose the hole. It was not the IRQ affinity: forcing every source back to the
boot hart's PLIC context still hung, which ruled the PLIC out.

The fix (`wait_for_nic`, `components/src/net_stack.rs`) asks smoltcp when it next needs to run. With no timer
pending (`poll_delay` is `None`), it blocks on the interrupt, the common case, 0% CPU until a frame
arrives. It is correct because with nothing of our own outstanding we are purely waiting on the peer,
whose retransmit will wake us. With a timer pending, it does not block: it yields and lets the
loop re-`poll`, so the timer fires and the retransmit goes out. The busy interval is confined to the
short retransmit window rather than the whole exchange. Both the DHCP bring-up loop and the
service loop use it. `std_net` then completes on both ISAs at the 4-hart/4-core boot.

The honest caveat: yielding across a retransmit window spins a hart until the timer is due (bounded by
the exchange, and by a 15 s per-call backstop well under the 60 s watchdog). The clean version is a
*timed* wait, a `WAIT` that returns on either the interrupt or a deadline, so the server sleeps
through the backoff instead of spinning. That is a small kernel-surface addition (an `Irq::WAIT`
timeout, or a timer notification) and is left as the follow-up. The yield-poll is correct and needs no
new syscall.

## The wait ends at the caller's deadline

`wait_for_nic` (`components/src/nic_wait.rs`) now ends at the earliest of three things: a frame,
smoltcp's next timer, and the caller's deadline. It sleeps on a timer bound to the server thread
(slots 5 and 6, milestone 106 (a wait that ends on either the interrupt or the deadline)), so the
hart is idle through a retransmit backoff. It blocks on the interrupt alone only when there is
neither a timer nor a deadline.

The deadline is what makes `service_until`'s 15 s bound a bound (2026-10-13 UTC). A listener with
nobody connecting, or a connection whose peer has gone silent, leaves smoltcp no timer. Without the
deadline the wait blocked until a frame that might never come. The inbound prober holding a
connection to nobody (milestone 514 (the HVF std listener hangs in some full runs)) sends nothing,
so the std listener's `ACCEPT` never expired and its retries never ran. A filtered run of the
hand-written listener test, which starts no prober, hung the same way every time.
`an_accept_nobody_answers_ends_at_the_stacks_bound` holds the bound on riscv and aarch64.

A polled NIC (the `e1000e` server, and radon's Ethernet port) holds no interrupt. Its wait,
`poll_without_interrupt`, sleeps at most `e1000e_transport::POLL_MS`, so the caller's bound holds
there too.

## The UDP gate must not depend on the host's resolver (a testing-hygiene defect, fixed)

The UDP socket-contract test queried `10.0.2.3:53` and called it "slirp's built-in resolver". That
description was wrong, and the error was not cosmetic: 10.0.2.3 is not a resolver. libslirp
implements no DNS server. It NATs anything sent to its guest-visible nameserver address to the
*host's* configured nameserver, which it looks up with `get_dns_addr_libresolv`. So every run of that
"zero host setup" gate sent a real query out of the machine to whatever resolver the developer's
laptop happened to be using, and passed only if that resolver answered in time.

It was measured, not argued. A temporary instrument in the client sent the same `example.com` query
40 times in one boot and reported what came back:

- 1 of 40 queries got no answer (2.5%), with a 15 s wait per query, so it is loss and not slowness.
- The answer's `ANCOUNT` was 2 and its first A record was `0x6814179a` = `104.20.23.154`, byte for
  byte what `dig @192.168.8.1 example.com` returned on the host at that moment. libslirp carries no
  zone data for `example.com` and cannot invent Cloudflare's rotating addresses, so this is direct
  proof that the host's resolver answered the guest's query.
- The same host resolver, probed directly with `dig +tries=1 +time=2`, dropped 1 of 30.

Two DNS queries ran per suite (mmio and PCIe), so a suite failed a few percent of the time from
nothing but a dropped packet on somebody's LAN. That matches the roughly one-in-three seen across a
handful of runs once network conditions were worse. UDP has no retransmit of its own and the client
sent exactly once, so a single lost datagram was a failed gate.

The fix keeps the coverage and removes the dependency. The gating UDP test now talks to slirp's
own TFTP server (`tftp=` on the netdev, at the gateway `10.0.2.2:69`), which libslirp answers
itself. The client sends a read request for a fixture the runners plant and asserts the reply is
`DATA`, block 1, with the fixture's exact bytes. This is the UDP twin of the guestfwd `/bin/cat` echo
peer the TCP gate already used. QEMU provides the service, nothing leaves the emulator, and no packet
can be dropped by a third party.

What the gate proves now: a client holding only a `Stack` endpoint and a shared frame can open a UDP
socket by id. It can send a datagram to an address of its choosing, and read the reply back through the same
frame. It does so over both the mmio and PCIe transports, on both ISAs. That is the whole client-to-net_stack-to-
smoltcp-to-confined-NIC path, which is what the test was ever really for.

What it no longer proves, deliberately: that DNS resolution works, or that the guest can reach
anything outside the emulator. That case did not get deleted; it became non-gating. The client
still sends a real query (now with three attempts, which is ordinary resolver behavior rather than a
widened timeout) and reports a distinct `NO_ANSWER` when the host never replies. The kernel test
prints and skips. A reply that arrives but is *not* a valid answer to our transaction still fails the
suite, because that would be our defect rather than the network's. So a broken host resolver, or an
offline laptop, now skips a check instead of failing a build, and a broken socket contract still fails
loudly.

The PCIe DNS variant is gone, not lost: UDP over the PCIe transport is now covered by the TFTP gate's
PCIe twin, deterministically.
