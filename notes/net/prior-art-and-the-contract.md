# The network stack: prior art, and how the socket contract was decided

*An appendix to [`notes/net.md`](../net.md), which is the page to read. This file holds the prior
art read before the contract, where this design differs from seL4, and the design record behind §25.
It exists to verify or challenge the main page. A reader who only needs to use the socket contract
should not have to open it. The directory `notes/net/` and this file's stem are provisional names;
naming is calef's.*

*Records cited below: milestone 32 (a real filesystem), milestone 47 (navigation and naming),
milestone 94 (the untracked-work sweep) and milestone 107 (the socket contract learns to accept).
Also §10 (process model), §25 (socket identity), §71 (a limitation is promoted when it stops being a
fact) and §76 (what catches a milestone status that is wrong in both places).*

## Prior art, read before the contract

The reuse call (smoltcp, not a hand-built TCP) is settled in the roadmap. What the prior art informs
is the *contract*: how a process asks a userspace stack to open a socket, and how bytes and events
cross the boundary.

seL4 net_stack componentization (and the CAmkES/lwIP and later Rust efforts). The stack is a
component; clients reach it over seL4 endpoints, and bulk data moves through shared dataports
(pre-shared memory regions), not through the IPC message. Control (open, connect, close, "data
ready") travels as small messages on the endpoint; payload lives in the shared region. This is the
cleanest match to what this kernel already has: a `Rendezvous` for control and a delegated `PageFrame`
for the shared buffer. The lesson taken: keep the per-connection data plane in shared frames and the
control plane in three-word messages, exactly the split DECISIONS §10 already chose for IPC.

Fuchsia Netstack3 (Rust, the closest cousin). Sockets are handles (Fuchsia's capabilities);
there is no ambient network, and a component reaches the stack only through a handle routed to it by
the component framework. Netstack3 is a state machine driven by events, which is also smoltcp's
shape. The lesson: a socket is a capability, the stack is named by a capability, and "no ambient
network" is enforced by the same mechanism as everything else (you hold a handle or you do not).
This is the model the roadmap already commits to; Netstack3 is the evidence it works in Rust at
scale.

Plan 9 /net, the counter-design. Everything is a file: a connection is a directory
(`/net/tcp/clone`, then `ctl`, `data`, `status`), you `write` "connect 1.2.3.4!80" into `ctl`, and
`read`/`write` the `data` file. It is elegant and it is the wrong fit here, twice over. It needs a
filesystem-shaped namespace, which milestone 32's FS server deliberately does not provide (a
client holds a directory capability, and open-by-path exists only inside the server). And "everything
a file"
means "everything reachable by path," which is the ambient authority this project inverts. Read as
the road not taken: the capability contract is what /net's `ctl` file would be if designation were
authorization instead of a path lookup.

Synthesis: seL4's endpoint-plus-dataport data plane, Fuchsia's socket-is-a-capability control plane.
Neither is novel here; both are what this kernel's primitives already point at.

### Prior art for the *inbound* half: who may claim a listening port

Read for milestone 107, after the contract above was already built. The outbound question ("how does
a process reach a stack") and the inbound one ("who decides this program may serve port 80") turn out
to have almost disjoint prior art, which is itself worth knowing.

POSIX, Linux and the BSDs: ambient above 1024. Any process binds any free port. Below 1024 needs
`CAP_NET_BIND_SERVICE` on Linux, or historically root, and that is a privilege rather than a
designation: it says "this process may bind low ports", never *which* ones. Linux later added
`net.ipv4.ip_unprivileged_port_start` to move the line, which is the same knob in a different place.
`SO_REUSEPORT` lets several sockets share one port, so even exclusivity is opt-out.

systemd socket activation, and `inetd` before it: the closest mainstream precedent, and it does not
enforce. The service manager binds the port and passes the listening descriptor to the service
(`LISTEN_FDS`); the service never calls `bind`. That is exactly this milestone's shape: whoever
spawns you decides what you may serve. The difference is enforcement. Nothing stops a systemd
service binding port 9999 itself afterwards, because the pre-bound descriptor is a convenience rather
than a boundary. Here `NO_LISTEN_GRANT` means the server *cannot*, because the grant is checked at
the only place a port can be claimed. Read as: the mainstream pattern with the hole closed.

Capsicum (FreeBSD): the same instinct, taken further. After `cap_enter()` a process cannot `bind`
at all and must receive pre-bound sockets. Capsicum's answer to "which ports may this program serve"
is "none, ever, only what it is handed". That is stricter than ours and it is the honest upper bound
on this design.

seL4: silent, and the silence is the finding. The kernel models no ports, no sockets and no TCP;
networking is a userspace component (CAmkES/lwIP, later Rust). Whatever a net server implements *is*
the policy. So there is no seL4 answer to diverge from here, and any claim that "seL4 does X" means
"some unverified seL4 component does X", since seL4's proofs cover the kernel and a port policy is
outside them. The same is true of us: nothing in `crates/socket_contract` is machine-checked, and
this grant is ordinary code.

Plan 9: authority by namespace. You write `announce 80` into `/net/tcp/clone`, and what
constrains you is whether `/net/tcp` is in your namespace at all. Restriction is by what you can
*see* rather than by an explicit grant, which is the counter-design already recorded above.

### Where this differs from seL4, and what it costs

The data plane here is seL4's, deliberately (see the synthesis above). The difference is one level
up, in how a component gets its authority:

seL4 systems wire statically. CAmkES describes the whole system at build time (components,
endpoints, dataports, connections) and generates the glue; seL4 Microkit does the same with protection
domains and channels in a system description. There is no "spawn a net server and hand it port 7778
because somebody just asked". The topology is fixed before the machine boots.

This tree grants at spawn, at run time. `wire_net_server(..., listen_grant(7778, 7778))` is a
decision taken while the system runs, by whoever holds the authority to take it.

What that buys: authority that moves during operation, which is the whole of milestone 47's
caretakers and the shell's grant model. A static system cannot express a user handing a program a
port range it was never declared with.

What it costs, and this is the honest half. A static topology is checkable *as a whole*: a CAmkES
spec can be read to see every channel that will ever exist. Our grant is a `u64` computed at a call
site, so "which components may serve inbound ports?" has no artifact anyone can read. That is a real
loss against seL4, not a tie. It is the same weakness this tree keeps meeting in other forms: a
fact that exists only at a call site (compare §76's roadmap status, and `script/names`' reason for
existing).

### One correction to the claim below

The section on listeners ([the-inbound-half.md](the-inbound-half.md)) says POSIX "conflates" a listener and a connection. That is imprecise and
the sharper version is worth having: POSIX's `accept()` already returns a new descriptor, so the
two are distinct objects there too. What POSIX does is make them the same *type*, with hidden state
deciding which calls are valid on which. The claim that survives is narrower and stronger: here they
are distinct authorities, because a listener carries no frame and therefore structurally cannot
move bytes, where a POSIX listening descriptor is a `read`/`write`-shaped thing that merely fails.

## The socket contract (resolved: DECISIONS §25)

The roadmap sketched "an endpoint plus shared frames per connection; no ambient network." The
socket-identity question below was a genuine design fork, raised rather than built through. The
architect resolved it (DECISIONS §25 on main). A socket is a socket id carried on the one `Stack`
endpoint, and the per-connection shared frame is the real granted resource. Minted-endpoint-per-socket
is deliberately deferred, with the recorded trigger being a socket that must be delegated to a third
process. The rest of the shape below stands as the contract Piece 3 implements. Recommendation first,
then the questions (question 1 now answered).

Recorded-accepted by milestone 94's sweep (2026-08-04): a deferral that already names what would
end it, which is the shape a limitation is supposed to have, so an audit may pass over it. The
trigger is the promotion rule §71 later wrote down, arrived at here first. See
notes/untracked-work-sweep.md.

Recommended shape. A process holds one capability to the stack: a `Stack` endpoint. Everything
is a `CALL` on it or on a per-connection reply channel, control in the three-word message, bytes in
a per-connection shared frame delegated at open time.

- `Stack::open(kind, ...)` where `kind` is TCP or UDP -> a socket capability (a fresh endpoint
  minted by the server, or a small integer socket id carried on the stack endpoint; see open
  question 1). At open, the client delegates (or the server delegates back) a shared `PageFrame` that
  becomes that connection's TX/RX ring, seL4-dataport style.
- `Socket::connect(ip, port)`, `Socket::bind(port)`, `Socket::listen`, `Socket::send(len)`,
  `Socket::recv() -> len`, `Socket::close`. `send`/`recv` carry only a length; the bytes are already
  in the shared frame. "Data ready" is a message on the socket endpoint, the same way an `Interrupt`
  capability delivers an interrupt (WAIT-shaped), so a blocking read is a blocked `RECEIVE`.
- DHCP and the interface config live entirely inside the server; a client never sees "the network,"
  only its own sockets. The server runs smoltcp's DHCP socket at startup and does not expose it.

Why this fits. It is the disk driver's discipline one layer up. The kernel confines the NIC's
DMA (Piece 1), the net server owns the smoltcp state and the NIC driver capability, and a client
gets exactly the sockets it was granted and no interface handle at all. Bytes never cross the
syscall boundary in a message (DECISIONS §10); they cross in a shared frame the two parties both
map.

The questions.

1. Socket identity: DECIDED 2026-07-28 (calef): a socket id on the stack endpoint for phase
   one; minted-endpoint-per-socket is the deliberate later step, tracked in DECISIONS §25. The
   trade as it was put to him. A minted endpoint per socket is the purest capability shape (a
   socket IS an unforgeable object, delegatable on its own). But it spends a kernel object (a
   page) per connection and needs the server to retype untyped per socket. A socket id (small
   integer) on the one stack endpoint is cheap and matches what `std::net`'s PAL wants (a
   file-descriptor-like handle), but "which socket" then rides in a message word rather than
   being the capability itself, which is weaker designation. The shared frame stays the real
   per-connection resource either way, which is what makes the later migration cheap.

2. How does the shared frame's producer/consumer protocol work without a syscall per byte? A
   ring buffer in the shared frame with head/tail indices, the driver pattern, so `send(len)` just
   advances a tail and messages the server. Straightforward, but the exact layout (one frame split
   TX/RX, or two frames) is a contract detail to pin.

3. Blocking vs. poll for the PAL. `std::net` is blocking by default. A blocked `RECEIVE` on the
   socket endpoint gives blocking cleanly. Non-blocking/`poll` is a later PAL concern; phase one can
   be blocking-only and still satisfy the roadmap's "no sockets-API mimicry beyond what the PAL
   needs."

The driver (Piece 2) did not depend on any of this: it needs only Piece 1's confinement, and it is
built.
