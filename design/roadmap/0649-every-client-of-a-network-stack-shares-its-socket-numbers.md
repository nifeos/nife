---
status: BUILT
built: 2026-10-07
raised: 2026-09-24
promoted_from: every-client-of-a-network-stack-shares-its-socket-numbers
milestone_dependencies: none
decision_dependencies: 255
machine_requirements: none
specific_machine: none
needs_person: no
---
# 649. Every client of a network stack shares its socket numbers

*Title and slug are drafts.*
Raised by milestone 590 (the booted system starts its network
stack), whose number is provisional, which gave programs at the prompt a `WRITE` view of one
`net_stack` endpoint and had to say in `caps` what that view does not narrow.

The fix changes `crates/socket_protocol`, which two programs agree on, so it is
a wire format and calef's call.

## The finding

`socket_protocol::req(op, sid)` names a socket by `sid`, an integer below `MAX_SOCKETS` (6), and
`net_stack` keeps one table of them for every caller of its endpoint. Nothing ties a `sid` to the
client that opened it. So two programs holding the network capability at once can each send, read
and close the other's sockets, and one that attaches a page to a `sid` another is using redirects
that socket's bytes into its own page. Under the kernel harness this never mattered, because every
stack had one client. At the prompt it is the difference between "may reach the network" and "may
reach every connection any other program on this machine has open", and `caps` now prints the
second sentence because it is the true one.

## Options

1. A badge per client. The progenitor mints each declaring child its own badged view of the
   stack's endpoint, and `net_stack` keys its socket table by badge. Needs endpoint badges, which is
   a question about §10 (process model: capability-based, microkernel) before it is one about
   sockets.
2. An endpoint per socket. `OPEN` answers with a fresh endpoint capability for that socket, the
   `fs_subtree_caretaker` shape; the `sid` disappears from the wire. The larger change and the one
   that matches how this tree already narrows files.
3. A stack per client. One `net_stack` per declaring job. No wire change, but a NIC can back one
   stack, so this only works behind a multiplexer that does not exist.

Recommendation, from reading rather than measurement: option 2, because it is the shape the tree
already uses for the analogous file case (a narrowed endpoint per grant), and a wire change is owed
either way. What each costs has not been measured.

Option 1 is now buildable without its own §10 fork. calef ruled the analogous filesystem
question on 2026-09-26 (milestone 599 (a frame per filesystem client channel), a frame per filesystem client channel) in favor of badged
endpoint capabilities, which is exactly the "endpoint badges" option 1 says it needs. Milestone 599
builds the badge machinery on the shared `INVOKE` surface (a `BADGE` method to mint a badged
endpoint, and the badge as a fourth `RECEIVE_CAP` return value). So option 1's prerequisite is being
built, and its cost here is `net_stack` keying its socket table by the badge the kernel already
delivers, with no `socket_protocol` change. Whether to take option 1 (reuse the badge) or option 2
(an endpoint per socket) is still open and still a wire decision, but the badge no longer has to be
argued for from scratch.

## Ruled and built (2026-10-07 UTC)

calef ruled the shape on 2026-10-07 (UTC), recorded on PR #1798 at 15:04Z: each socket is its own
capability. The stack mints a badged endpoint per socket on open, and every later call is made on it.
The stack names the socket by the kernel-stamped badge, so `socket_protocol` drops the `sid`. Close
unbinds the badge, and a socket moves to another program by passing the capability. It refuses
this block's option 1 as written ((badge, sid) keying) and its option 3.

A reply cannot carry the minted capability through a plain `REPLY`, and `SEND_CAP` would park the
one-threaded stack on any client that never receives, so calef ruled (2026-10-07, PR #1817): option
A, a new Reply method that carries one capability. §255 (each socket is its
own capability), number provisional, records both rulings, the method's semantics and the contract.

Reuse: none exists to take. The contract and the stack are this tree's own; the method follows
seL4's reply-carries-a-capability and Fuchsia's `zx_channel_call`, read for §255 and not code.

## Result

Built on aarch64, riscv64 and x86_64 (PR #1817).

- The kernel method. `abi::reply::REPLY_CAPABILITY` (method 1 on a Reply; name ratified 2026-10-07 UTC, number provisional): answers a
  `CALL` and files a `GRANT`-checked copy of one capability in the caller's table, whose `CALL`
  returns its slot in `x2`. Every `CALL` now returns `x2`, `NO_CAP` after a plain `REPLY`.
  `a_reply_carries_a_capability_the_caller_can_use` proves it at EL0, with a replayable
  falsification; the seeded syscall driver's shadow model checks `x2` after every `REPLY`.
- The contract and the stack. `socket_protocol` loses `req`, `req_sid` and `MAX_SOCKETS`, and
  gains `SOCKET_BADGE` and `stack_slots`. `net_stack` keys sixteen sockets by badge, mints each from a
  `WRITE | GRANT` copy of its own endpoint (slot 7), and unmaps a closed socket's page through its
  own address space (slot 8), which every spawner now grants. The progenitor's spawn gives it both
  (a `placed` entry naming `supervision_protocol::CHILDS_OWN_SPACE`, provisional), and so do the kernel's test
  wirings (`user::run_with_own_space`, provisional).
- Every client. `socket_test_client`, `name_resolver`, `network_time_client`, the progenitor's
  package fetch, `network_echo_client`, `name_resolver_test_client`, `network_time_test_server`,
  `unreachable_network_witness`, and the `std::net` PAL.
- The property. `system_tests/src/user/net_confinement_tests.rs` runs in the default suite on all
  three ISAs, and each test has a replayable falsification confirmed red on aarch64. The first is
  `a_squatter_at_a_shared_stack_endpoint_cannot_capture_the_clients_traffic`, milestone 800 (a
  non-Anthropic model attacks the confinement claim)'s pinned test rewritten. Ten ways a second
  client might reach a held socket are all refused, and the honest exchange is untouched. The
  second is `a_socket_moves_by_its_capability_and_a_closed_one_reaches_nothing`.
- Slot pressure, counted. A client spends one of its 64 slots per open socket. The `std::net`
  PAL also keeps one frame per id that has carried bytes, so a std program holding its six sockets
  spends twelve. The stack holds a minted copy only between minting and answering.
- The window-server audit calef ruled on #1798: `system_log`, the file service and
  `name_resolver` already read a client's window from its badge; `net_stack` was the one that did
  not. §255 records it.

The exit criteria from the review of #1798 (finding 8) are met. The test runs in the default suite
with no opt-in skip. Each test starts its own stack and gives every region back, so the aarch64
suite gains no held region. A replayable record that brings the shared namespace back turns it red.

## Follow-on

- **Recorded.** One client can take all sixteen sockets; a per-client quota wants a badged front door per client.
  Recorded in `net_stack`'s BUGS.
- **Decision.** Whether a `CALL` caller may refuse a carried capability is open in
  `design/decisions/0255-each-socket-is-its-own-capability.md`, and nothing blocks on it.
- **Recorded.** Risk 7's criterion (c) restarted with milestone 800's fourth pass; it needs a fresh clean
  non-Anthropic pass, which this does not supply.

## Index row

Built: each socket is its own capability (§255), handed over by a new `REPLY_CAPABILITY`, so a network
program reaches only the sockets it holds.
