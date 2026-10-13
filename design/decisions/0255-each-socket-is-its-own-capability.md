---
status: DECIDED
raised: 2026-10-07
decided: 2026-10-07
ratified_by: calef
---

# 255. Each socket is its own capability, and a reply carries it

*Raised for milestone 649 (every client of a network stack shares its socket numbers).*

Two rulings, both calef's, both 2026-10-07 (UTC). The first is the shape: each socket is its own
capability. Building it found that a reply could not carry a capability, which was a fork on the
syscall surface. The second ruling took option A, a new Reply method that carries one. This section
records both, the method's semantics (§10 (process model: capability-based, microkernel) requires
them written here), and the `socket_protocol` contract that two programs now agree on.

## The rulings

The shape, recorded by the maintainer session on PR #1798 at 15:04Z, superseding calef's
per-caller-windows ruling of about 03:50Z the same day:

> milestone 649's fix makes each socket its own capability. On open, the network stack mints a
> badged endpoint for that one socket (milestone 606's badge model); every later call (send,
> receive, attach a page, close) is made on that capability; the stack identifies the socket by the
> kernel-stamped badge, never by a client-supplied `sid`, so `socket_protocol` drops the `sid`
> argument; close unbinds the badge; a socket can be handed to another program by passing the
> capability. This is §27's directory-capability model applied to sockets.

calef's words: *"Naming socks as capabilities seems right. Can we do that?"*, then *"Yes"*.

The delivery fork, recorded on PR #1817 at 15:23Z:

> §255 delivery fork ruled (calef, 2026-10-07 UTC): option A. Add a new Reply method
> (provisionally `REPLY_CAP`) that carries one capability, so the stack hands each socket capability
> to its client in the `CALL` reply and delivery never blocks the stack.

## `REPLY_CAPABILITY`: a reply that carries one capability

`invoke(reply, REPLY_CAPABILITY, r0, r1, capability_slot)`, method 1 on a Reply capability. Name
ratified by calef on 2026-10-07 (UTC), recorded on #1817, to match `SEND_CAPABILITY`,
`RECEIVE_CAPABILITY` and `NO_CAPABILITY` in design/naming/capability-worklist.md; the method number
is provisional. The runtime pair is `reply_capability` and `call_receiving`.
`abi::reply::REPLY_CAPABILITY` carries the same contract at the definition.

- What it does. It answers the caller as `REPLY` does, and files a copy of the server's
  `cap_slot` in the caller's table. The caller's `CALL` returns the new slot in `x2`.
- Authority. `WRITE` on the Reply, as `REPLY` takes. `GRANT` on the carried capability, as
  `SEND_CAP` requires of a delegation. The copy keeps the rights it has in the server's table,
  because the method has no word left for a rights mask, so a server narrows before it answers.
- A refusal consumes nothing. An empty `cap_slot` is `NoSuchSlot`, and a capability without
  `GRANT` is `NotPermitted`. Either way nothing reaches the caller, the caller still waits, and
  the Reply is still the server's to answer with.
- A full table drops the copy, not the answer. The caller gets `r0`, `r1` and `NO_CAP` in
  `x2`. That is what a server already sees when a `CALL`'s Reply did not fit its table.
- A caller that has stopped waiting gets nothing, and no copy is made, exactly as for `REPLY`.
- Every `CALL` now returns `x2`. After a plain `REPLY` it is `NO_CAP`, so a caller never
  reads slot 0 out of a reply that carried nothing. Before this, `x2` came back holding whatever
  the caller had left there.
- No opt-in on the caller's side. A caller asked for a reply by calling, and can delete what
  it is given; seL4 gates reply transfer on the server's `Grant` alone. §246 (a plain `RECEIVE`
  never takes a capability) refused filling the table of a receiver that never asked, which a
  `CALL` caller is not. Whether a caller should be able to refuse stays open; nothing blocks on it.
- Cost on the round trip. The method is out of line in `kernel/src/sched.rs`
  (`ipc_reply_capability`), so plain `REPLY` does not reach it. What a plain round trip pays is one
  constant store in `ipc_reply` (`NO_CAP` where it stored a zero) and one register write in the
  `CALL` arm. The CI footprint and icount gates price it on every ISA.
- Proof. `a_reply_carries_a_capability_the_caller_can_use` in `system_tests`, on aarch64,
  riscv64 and x86_64: the carried capability works in the caller's hands, both refusals leave the
  Reply usable, and a plain `REPLY`'s `x2` is `NO_CAP`. The seeded syscall driver's shadow model
  now checks `x2` after every `REPLY`.

## The `socket_protocol` contract

- The request word is the opcode alone. `req`, `req_sid` and the client-visible
  `MAX_SOCKETS` are gone. A socket is named by the badge on the capability a request arrives on,
  which the kernel writes (`x3` of `RECEIVE_CAP`, §230 (badged endpoint capabilities)) and no
  client can choose.
- The front door makes sockets, and nothing else. The capability a client is spawned with
  accepts `OPEN_UDP`, `OPEN_TCP`, `BIND_UDP` and `LISTEN`. Each answers with the new socket's
  capability carried by `REPLY_CAPABILITY`. Any other operation on the front door is `REP_ERR`.
- A socket's capability does the rest. It accepts `ATTACH_PAGE_FRAME` (still a `SEND_CAP`),
  `SENDTO`, `RECEIVE`, `CONNECT`, `SEND` and `CLOSE`. A listener's accepts `ACCEPT` and `CLOSE`.
  `ACCEPT` answers with the connection's capability, so its `target_sid` argument is gone, and
  the client attaches a page to the connection afterwards.
- Socket badges are their own range. The stack mints them with the top bit set
  (`socket_protocol::SOCKET_BADGE`) from a counter it never rewinds. A request whose badge has that
  bit names a socket; any other badge is a front door. So a spawner may badge front doors for its
  own reasons, and a socket's capability never acts as one.
- Close unbinds. `CLOSE` drops the badge from the stack's table and unmaps the socket's page.
  Any copy of that capability, held by anyone, then reaches nothing: every request on it is
  `REP_ERR`, an `ATTACH` on it is dropped, and it cannot open a socket, because its badge is in the
  socket range. That is the fail-closed reading §230's milestone 726 (an unknown badge fails closed
  in subtree_scope) amendment gave `subtree_scope`.
- Passing a socket is passing its capability. The stack mints from a `WRITE | GRANT` copy of
  its own endpoint, so a socket holder may `SEND_CAP` its socket on and may not receive on the
  stack's endpoint. `BADGE` refuses an already-badged source, so `GRANT` does not let a holder
  re-badge its socket into another.
- What the stack is spawned with grows by two. Slot 7 is that `WRITE | GRANT` copy of its own
  endpoint. Slot 8 is its own address space, `WRITE`, so `CLOSE` can `UNMAP` the socket's page;
  without it a table entry reused by another client would still hold the last client's page.
- The table is the stack's, not the wire's. Sixteen sockets live at once across every client,
  a constant inside `net_stack`. A client that holds all sixteen starves the rest; that is recorded
  in `net_stack`'s BUGS.

Slot pressure, counted from what each client holds: a client spends one slot of its 64 per open
socket, and nothing per page it attaches, because `SEND_CAP` sends a copy and the client keeps its
own. The `std::net` PAL also keeps one frame per id that has carried bytes, so a std program holding
its six sockets spends twelve slots. The stack spends one slot per socket for the instant between minting and answering.

## Refused

- B, a client-supplied return endpoint. calef: refused "because any client could stall the
  stack for good". The stack is one thread and `SEND_CAP` blocks until the receiver takes it, so a
  client that opens and never receives parks every client's network.
- C, a pool minted at spawn by the progenitor. calef: refused "because it fixes the number of
  sockets at startup". It also takes minting away from the stack, and a stale copy reaches nothing
  only if a closed badge is retired for good.
- A non-blocking `SEND_CAP`. A kernel change too, and a single-threaded client is still in its
  `CALL` when the stack would send, so the send finds nobody parked.
- The client minting its own badges. `GRANT` on an unbadged copy of the stack's endpoint mints
  any badge, so it would name any socket.
- `(badge, sid)` keying, the 03:50Z ruling read across the whole table. It closes the
  cross-client escape, but a socket is still a number its holder chose: it cannot be passed on, and
  closing it revokes nothing a copy could hold.
- Per-caller frame windows only, the 03:50Z ruling as worded. It closes the frame capture and
  leaves sending to, reading and closing another client's socket open.
- A stack per client (option 3 in milestone 649's block): a NIC backs one stack, so it needs a
  multiplexer that does not exist.

## Prior art

- seL4 lets a reply carry capabilities, gated on the `Grant` right of the endpoint capability the
  server received on (seL4 reference manual; seL4 PRs #874 and #945 tightened the MCS variant).
  Read for this section.
- Fuchsia's `zx_channel_call` reads its reply as `zx_channel_read` does, handles included. Read for
  this section. A socket there is a `zx_socket` handle passed over a channel. (Recalled.)
- Genode: a client's connection to a service is a session capability the server returns, and every
  later RPC is on it. (Recalled.)
- Capsicum: in capability mode a socket is a file descriptor with rights, moved between processes
  by `SCM_RIGHTS`. (Recalled.)
- In this tree: §27 (the filesystem service)'s directory capability, bound to a badge by
  milestone 606 (a directory walk costs what it does on Linux), and §252 (a resolver grant is one
  zone per client badge)'s resolver grant. In both, authority is a capability the server recognizes
  by its badge, never an argument the client fills in.

## Servers that keep windows for many clients

Also ruled on #1798 the same day, and recorded there by the GLM lane: per-caller scoping becomes a
written rule for every server that keeps windows for many clients, and milestone 649's lane audits
the rest, `system_log`'s reader windows first.

- `system_log`: scoped already. It answers `OPERATION_READ` with the window registered for the
  reader's badge (`crates/system_log/src/lib.rs`, `Log::handle`), and only badge 0, the spawner,
  can register one. A reader names no window, so it cannot name another's.
- The file service and `name_resolver` read a request's window from its badge (§230, §252), so a
  client names no window either.
- `net_stack` was the one that did not, and this section is its fix.

The rule itself has its own section since PR #1798's merge: §256 (a server that keeps windows for
many clients scopes each by the caller's badge), which also holds the rest of the audit.
