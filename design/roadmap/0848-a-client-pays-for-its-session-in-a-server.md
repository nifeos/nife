---
status: NOT-STARTED
raised: 2026-10-06
promoted_from: a-client-pays-for-its-session-in-a-server
milestone_dependencies: 851
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 848. A client pays for its session in a server

*(The title and slug are provisional.)*

calef asked for this on 2026-10-06 (UTC), ruling fork 6 of milestone 851 (a running program acquires
more memory as it needs it): "Approve Fork 6 A with both follow-ons". Fork 6 kept boot servers on
fixed budgets,
because a server that grew from a shared pool would let one person's traffic spend another's
memory. This is the first follow-on: a server whose load grows with its clients is paid by those
clients. calef ruled all four forks on #1786 the same day.

It depends on milestone 851's `memory_broker` and its account tree (forks 1 and 5 there, both
ruled). Every
name here is provisional: the donation, its verbs, and the fixture.

## Why, and its honest rank

A server with a fixed budget has a fixed number of clients. `net_stack` today holds a 128-page
budget, a 96-page heap for smoltcp, and `socket_protocol::MAX_SOCKETS` = 6. Each client already
pays for the frame its bytes cross in, minted from its own `NET_MEMORY_REGION_SLOT`. But the
socket's smoltcp buffers (`SOCK_BUF`, 2 KiB each way) and the page tables that map the client's
frame come from the server's budget (`components/src/net_stack.rs`). So the seventh connection is
refused, and a web server is out of reach.

Raising the server's budget moves the ceiling without removing it, and a connection flood then
spends it. Growing the server from the pool, as fork 6 refused, lets the flood spend the machine.
The client paying is the shape where a flood drains the flooders.

On principle 1, this is behind milestone 851's first slice and behind milestones 801 (packages over
the
internet) and 802 (the trivial install). It is on the path to any network service a customer runs,
since a server that holds six connections is not one. It moves no fatal-risk verdict by itself.
It strengthens risk 7 (the confinement claim is false): it closes a resource channel between two
clients of one server.

## What the tree has, read on 2026-10-06 (UTC)

- Milestone 851's accounts form a tree: pool, then session, then program. A grant is charged to every
  account above it. An account is a badged endpoint, and a program's spawner opens and closes it.
- A region's `DESTROY` cuts the page tables it paid for and everything mapped beneath them
  (`abi::page_frame::MAP`). A server mapping pages a client can revoke can lose them while running.
- A server dies on a fault, delivered to its supervisor (§26 (the fault endpoint)). There is no
  recoverable fault.
- `socket_protocol` opens a socket with a message that carries no capability today.

Reuse: the broker and its accounts are milestone 851's. Genode's session quota donation lends the shape
and no code.

## The forks

Four, all ruled.

### Fork 1. What the client hands the server

Ruled A, calef, 2026-10-06 (UTC): "Yes".

| option | what it is | verdict |
|---|---|---|
| A | A donation: a child of the client's account whose capability goes to the server. The server draws on it; the charge stays on the client's side of the tree | ruled |
| B | A region capability the client splits and sends | refused |
| C | The server bills the client after the fact | refused |

B puts memory the client can destroy under a running server. Revoking it, or the client dying,
cuts pages the server is using, and the server faults and dies with every other client's session.
C needs the server to be trusted to bill, and does nothing to stop a flood before it lands.

A is Genode's shape (from memory): a client transfers quota to the server when it opens a session,
and the quota comes back when the session closes. Under A the server holds the only capability
that spends the donation. The client cannot pull pages out from under it.

What this needs from milestone 851's broker, which its first slice should build in so this is not a
protocol change later:

- An account holder can open a child of its own account with a ceiling, not only a spawner. Today
  the text gives that verb to spawners.
- A child's capability can be delegated by `SEND_CAP` like any endpoint, and whoever holds it spends
  from it. The broker attributes the charge by the tree, not by who holds the capability.
- A donated child is closed by its holder. Its parent's close does not destroy it (fork 2).

So the answer to "can part of an account be delegated?" is yes, as a child account. Milestone 851
was already armed, so calef ruled that these three points land in its milestone and decision text at
promotion, recorded in a comment on #1777. Its promotion wrote them into 851's first slice.

### Fork 2. When the client dies first

Ruled A, calef, 2026-10-06 (UTC): "Yes".

| option | what it is | verdict |
|---|---|---|
| A | The donation outlives the client until the server closes the session. Its charge moves up to the client's session account | ruled |
| B | The broker destroys the donation when the client's account closes | refused |

B is fork 1 B's crash by another route. Under A the server learns of the death as it does today,
from a dead channel, and closes the session; the charge returns then. A server that never closes
leaks only its clients' charge, which the owner's revoke on the server reclaims.

### Fork 3. How small a session can be

Ruled A, calef, 2026-10-06 (UTC): "Yes", on two conditions. The lag is bounded and gated: opening
and closing 10,000 sessions must return the server's charge to baseline within one region, 1 MiB.
His motive is Linux memcg's "zombie cgroup" problem, where shared slab pages kept dead groups
charged until per-object charging, around 5.9 (from memory; the building lane owes a read). And
C is the recorded fallback if that test cannot pass.

Milestone 851's fork 7 bounds regions by memory because every region is at least 1 MiB. A TCP session
needs a few KiB. One region per session would cost 1 MiB per connection.

| option | what it is | verdict |
|---|---|---|
| A | The server commits from its own regions in 1 MiB steps and packs many sessions into each. The broker moves only the charge: each session's bytes are charged to its donation | ruled |
| B | One region per session, at least 1 MiB | refused |
| C | Page-sized regions, each charging its table slot to the account, Genode's cap quota | fallback, if A's lag test fails |

Under A, the broker keeps a charge ledger per donation, not a region per donation. A server
records "this session now holds N pages", the broker checks N against the donation's ceiling and
moves N pages of charge from the server's account to the donation. Physical pages return to the
pool when a server region empties, which can lag the charge. That lag is held on the server's own
account, bounded by one region of slack per server. B is simple and costs 1,000 MiB for 1,000
connections. C breaks milestone 851 fork 7's 1 MiB floor on regions, so it must charge each region's table
slot to the account to keep a flood from spending the region table.

A is about elegance, not effort: B is less work.

### Fork 4. Which servers first

Ruled, calef, 2026-10-06 (UTC): "Approve".

| option | verdict |
|---|---|
| `net_stack`, since its six-socket ceiling is the one a customer would meet | ruled, first slice |
| the file server, the system log, the compositor | follow-ons, each its own measurement |

`net_stack`'s own budget stays fixed for its own state. Only per-socket memory moves to donations.
`MAX_SOCKETS` becomes a table sized by memory, not a constant.

## How others do it

From memory, unchecked; the building lane owes a read of each.

- Genode: a client donates RAM quota with a session request, the server spends it on that session,
  and closing the session returns it. A server that runs out asks the client to upgrade.
- Genode cap quota: capabilities are a second quota, donated like RAM. Servers keep a heap per
  session, so closing one frees exactly its memory. Fork 3 C.
- KeyKOS, EROS and Coyotos: a client passes a space bank to a server, which allocates the
  session's storage from it. Destroying the bank reclaims all of it.
- Linux memcg: kernel memory and socket buffers are charged to the cgroup of the process that
  caused them. Charging whole slab pages left dead cgroups charged ("zombies") until per-object
  charging, around 5.9. `somaxconn` and per-socket limits bound a flood.

## The first slice

1. Milestone 851's broker gains a holder-opened child account, delegable, closed by its holder.
2. The broker's charge ledger: a server moves charge between its account and a donation.
3. `socket_protocol`'s open carries a donation capability. A socket without one is refused by name.
4. `net_stack` keeps its fixed budget for itself and draws per-socket memory against donations.

### Exit criteria a stranger could check

One `cargo xtask` gate that `script/test` runs, exiting 0 under QEMU on aarch64, riscv64 and
x86_64:

1. More than six. A client opens 64 TCP sockets to a loopback listener and moves bytes on each.
2. The lag is bounded. A client opens and closes 10,000 sessions. The server's charge returns to
   its baseline within one region, 1 MiB. If this cannot pass, fork 3 C replaces A.
3. A flood drains the flooder. A client opens sockets until its donation ceiling refuses. A second
   client then opens and uses a socket, and `net_stack`'s own budget figure is unchanged.
4. Death returns the charge. Killing the flooding client returns its charge to its session account
   once `net_stack` closes its sessions, with `net_stack` still serving the second client.
5. No revocation reaches the server. The client deletes its copy of everything it sent, and
   `net_stack` keeps running.
6. A `design/decisions/` section records the donation verbs, minted by the integrator.

## BUGS

- A server that never closes a dead client's session holds that client's charge until the owner
  revokes the server.

## Index row

A server's per-session memory is paid by the clients that spend it: a donation account per session, a charge ledger in the broker, and `net_stack` drawing per-socket memory against donations. `net_stack` is the first slice, since its six-socket ceiling is the one a customer would meet. Waits on milestone 851 (a running program acquires more memory as it needs it).
