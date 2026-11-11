# The network stack: `std::net::TcpListener` on the socket contract

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

*An appendix to [`notes/net.md`](../net.md), which is the page to read. This file holds milestone
64's binding of `TcpListener` and why the inbound prober requires three rounds of four. It exists to
verify or challenge the main page. A reader who only needs to use the socket contract should not
have to open it. The directory `notes/net/` and this file's stem are provisional names; naming is
calef's.*

*Records cited below: milestone 55 (Time Machine) and milestone 64 (enough `std` to run somebody
else's crate).*

## The std client is a server too: `TcpListener` on this contract (milestone 64)

Everything in [the-inbound-half.md](the-inbound-half.md) is `socket_test_client`, which is hand-written and speaks the wire directly. The
same inbound path is now reachable from ordinary Rust: `std::net::TcpListener::bind`, `accept`,
`read_exact`, `write_all`, in a program that names no capability and no socket id
(`patches/std-nife/overlay/std/src/sys/net/connection/nife.rs`). That is the difference between a
crate compiling and a server running, and it is what milestone 55's Samba-shaped workload stands on.

The binding was mechanical, exactly as this note predicted, and the interesting part is what it
did to the capability graph rather than to the code. `start_net_std` used to pass
`NO_LISTEN_GRANT` with a comment saying a grant would be authority nothing could spend. It now takes
the grant as a parameter, so the caller decides, and the two callers decide differently:

| the stack a `std_exerciser` is spawned over | `TcpListener::bind` | the pinned transcript |
|---|---|---|
| `NO_LISTEN_GRANT` | `PermissionDenied` | `std net on nife` / `listen refused` / `udp ok` / `tcp echo ok` |
| `listen_grant(7778, 7778)` | granted | `std net on nife` / `listen ok` / `denied refused` / `in use refused` / `served 2` |

Both are compared byte for byte on both ISAs, which is what makes the first row a negative
control rather than a test that happens not to run. A change that widened the grant check would
turn `listen refused` into `listen ok` and fail in a transcript diff. It costs that boot nothing,
because that test already existed and already spawned a stack with no grant.

Three answers, three `ErrorKind`s, and the mapping is the contract's own vocabulary rather than
one invented in the PAL. `LISTEN_DENIED` becomes `PermissionDenied`, which is the one error here a
caller cannot fix by trying elsewhere: no other port helps, because the answer was about authority.
`LISTEN_IN_USE` becomes `AddrInUse`, which is the retryable one. A refused `ACCEPT` (nobody arrived
inside the server's bounded wait) becomes `WouldBlock`, because the listener is still armed and
calling again is the right response.

What the granted run does not do is the outbound half, and that is a cost decision made in the
open rather than an oversight. A net test spends minutes in `net_stack`'s userspace smoltcp poll, so
a boot is the expensive unit in this suite. The outbound transcript is already proven by the run that
is refused the port, so the granted run serves and stops.

One thing the contract cannot tell the PAL, and it is now a fork rather than a gap.
`std::net::TcpListener::accept` must return a `SocketAddr`, and `OPERATION_ACCEPT`'s reply carries no peer,
so what comes back is `0.0.0.0:0` and `peer_addr()` on an accepted stream reports the same. A server
that logs its peers logs zeros. Two ways to fix it, both changes to what two programs agree on and
therefore neither taken here. One is a second reply word (`reply` already carries two and `OPERATION_ACCEPT`
sends zero in the second). The other is the frame's dead `dst` fields, which is exactly the move a UDP
`RECEIVE` already makes with the datagram's source and would cost no format change at all. The second
is cheaper and has the precedent; both are calef's.

The host prober now sees two listening windows over one boot, and what it requires changed shape
because of it. The guest offers four rounds (two per program) and the prober passes at three.
That is a provable claim rather than a fudge: neither program can supply more than two, so three
collected means at least one from each, which is exactly the half no in-guest assertion can make.
The *re-arm* claim is not the host's to confirm any more; it is asserted inside each guest test,
where it is actually checkable, and a guest cannot fake a second `accept` returning the right bytes.

Requiring all four was tried first and measured red. On CI run 32195227733 the riscv64 leg
reported "the guest served 3 of 4" while all 279 guest tests passed, on a machine the script's
own load instrument called not oversubscribed. So the guest served four and the host collected
three: one answer went somewhere the prober was not reading. Five local riscv boots did not
reproduce it, which puts it near one in six on that runner class.

The mechanism is not identified, and that is stated rather than guessed at. The candidate this
note already describes is a connection the prober abandoned that slirp still delivered ("an
abandoned request may still be executed"), which would let the guest serve a round nobody was
collecting. The other is a teardown race on the very last round, since that test's program exits and
its `net_stack` is reclaimed immediately afterwards. Neither is confirmed. What is confirmed is that
this is the intermittent red notes/net.md already records misleading three separate milestones, so
the gate was made robust to losing one round rather than left claiming something it cannot deliver
six runs out of six. Identifying it wants a lane of its own.
