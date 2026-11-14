---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017, 1000
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1108. `socket_contract`: what an outside maintainer would fix before it leaves

Raised by a read-only review of `crates/socket_contract` as a stranger to this tree would meet it,
ahead of its move under step 2 of
[milestone 1017 (the contract crates leave in this order)](1017-the-contract-crates-leave-in-this-order.md).
It changes nothing; it lists what the next lanes change.

The review took its accepted baseline from
[the review package](../../notes/crate-reviews/socket-contract.md). It raises none of the
decisions it puts to the architect. They are the identity values (`INTERFACE_ID` 19, `VERSION` 1), where
`fixture`, `ephemeral` and `polled_wait` live, the table of 55 proposed names, and the clearance
itself. Nothing here reopens them. Where this review reads a later rule differently from the
package, it says so at the item. The bind-time version check belongs to
[milestone 1000 (message contracts carry bind-time version checks)](1000-message-contracts-carry-bind-time-version-checks.md)
and is not repeated here.

Reuse: the template `nifeos/crate-workflows` and its scaffold, Kani, cargo-mutants, the crate's
own tests and falsification patches. No new tool, no new dependency.

## Goal

A stranger who finds `socket_contract` can write a client, or a second network stack, from the
crate's text alone and have it interoperate with the stack nife ships. Today the text and the wire
disagree on where a send's length travels, which fails that goal before anyone discusses a name. After
this block, every operation's documentation states its argument word, its reply words and what the
frame carries in each direction. The repository's gates are green for the crate's own reasons. The
proofs cover the one piece of logic the crate holds beyond the grants.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1108-socket-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1108-socket-contract-outside-review/prior-art.md).

## The items

Effort: S is under half a day for a lane, M a day or two, L more. A door is one-way when somebody
outside the repository acts on it after the release (a published name, a wire format, a layout, an
identity, a dependency); everything else is two-way and the maintainer decides it. The crate has not
released, so its names are two-way today and the package's naming table stands.

### P0: blocks the move

1. The contract's text and its wire disagree on where a send's length travels. DONE (every `OPERATION_` states its argument and reply words; the `OPERATION_SEND` doc test sends with the length as the argument; `LENGTH_OFFSET_BYTES` says only `RECEIVE` writes it; D1 ruled 2026-11-11).
   Finding: the layout note at `crates/socket_contract/src/lib.rs` lines 454 to 459 says the
   `len` field at `LENGTH_OFFSET_BYTES` is "in for SEND*, out for RECEIVE". The documentation of
   `OPERATION_SENDTO` and `OPERATION_SEND` (lines 53 to 60) says "payload in the frame" and
   nothing about the argument word. The stack never reads `LENGTH_OFFSET_BYTES`: `components/src/socket_server.rs`
   lines 172 and 182 pass the `CALL`'s argument as the length, and `socket_operations.rs` lines
   45 and 328 bound that argument by `DATA_MAXIMUM`. The std platform layer and the test client
   both pass the length as the argument and never write `LENGTH_OFFSET_BYTES` (`nife.rs` lines 496 and 856,
   `socket_test_client.rs` line 406). A client written from the crate's text, which writes
   `LENGTH_OFFSET_BYTES` and passes 0, sends nothing and hears that it sent nothing. The only writer of `LENGTH_OFFSET_BYTES`
   is the stack's `RECEIVE` (`socket_operations.rs` line 116), and its only reader is
   `components/src/socket_squatter.rs` line 164, which polls it as a "data arrived" flag.
   Evidence: the lines above.
   Change: state the wire as both sides already speak it. Each `OPERATION_` constant's documentation
   names its argument word. For `SEND` and `SENDTO` it is the payload length in bytes, at most
   `DATA_MAXIMUM`. For `LISTEN` and `BIND_UDP` it is the port. For `RECEIVE`, `CONNECT`, `CLOSE` and
   `ACCEPT` the stack ignores it, so pass 0. The documentation also names the reply. `SEND` replies the
   count of bytes the stack accepted, which may be short. `SENDTO` replies `REP_OK`. `RECEIVE`
   replies the count written to the frame. The layout note says
   `RECEIVE` writes `len` and nobody the contract requires reads it; a test asserts that
   `LENGTH_OFFSET_BYTES`'s documentation and `OPERATION_SEND`'s agree (a doc test that sends from a fake frame
   is enough). The README's "Replies" bullet changes with it. This is item D1 below, since the
   choice of which side carries the length is a wire-format door; the recommendation is to ratify
   the implementation.
   Door: the text is two-way; the wire choice it records is one-way. Effort: S.

2. The manifest's `publish` comment points outside the repository. LEFT: a move step; the manifest line is rewritten at the move.
   Finding: `Cargo.toml` line 11 reads `publish = false     # the default; the allowlist lint holds
   the exception (crates/publish-allowlist.txt)`. That file is nife's and will not be in the new
   repository; the crate is not on the allowlist today. The template's `check / standalone`
   expects either a publishable manifest or the caller's `allow-unpublished` input.
   Evidence: `Cargo.toml` line 11; `crates/publish-allowlist.txt` (no `socket_contract` row);
   the template README's description of `check.yml`.
   Change: at the move, replace the comment with one sentence a stranger can act on ("set to
   `true` only in the release that publishes") and pass `allow-unpublished` in the caller until
   then. Add `repository = "https://github.com/nifeos/socket_contract"` (the scaffold's badges
   assume it). Publishing itself stays held, as the package records.
   Door: two-way. Effort: S.

### P1: before the first release

3. `ACCEPT` carries no peer address, and the tree already lists it as a contract change. DONE (`a_peer_word_round_trips_and_refuses_what_no_stack_writes`, Kani `a_peer_word_round_trips_for_every_host_and_port`; the stack and std read the word; D2 ruled 2026-11-11).
   Finding: `OPERATION_ACCEPT` answers `REP_OK` and the connection's capability. The stack's
   second reply word is always 0 (`socket_server.rs`, every `reply(to, word, 0)`), and the new
   connection has no frame yet to carry an address in. `notes/net.md` line 173 records that an
   accepted stream reports its peer as `0.0.0.0:0`, and
   `notes/bugs-without-milestones/network-stack.md` line 16 holds it as a contract change for the
   architect. Every system in the prior-art table returns the peer with the accepted socket.
   Evidence: `lib.rs` lines 67 to 70; `socket_server.rs` lines 188 to 196;
   `components/src/socket_table.rs` line 125 (`hand_over`).
   Change: item D2 below. If ruled, the crate gains `peer_word(ip, port)` and `word_peer(word)`
   (names provisional) in the `lease` style, documented on `OPERATION_ACCEPT`, with a round-trip
   test and a Kani harness over every address and port. The stack fills the second reply word in
   `hand_over` for an accepted connection, and the std layer reads it for `peer_addr`.
   Door: one-way (a wire format). Effort: M across crate, stack and std; S in the crate.

4. The reply vocabulary leaves two outcomes a client must handle unstated. DONE (the operation, `REP_OK` and `CONNECT_*` docs state the replies; `RECEIVE` 0 is the end of the stream and a short `SEND` count means send the rest).
   Finding: `REP_OK`'s documentation (`lib.rs` line 104) says "Non-negative is success, and
   `RECEIVE` replies its length here instead". `SEND` also replies a length, and a short one when
   smoltcp's buffer is full (`socket_operations.rs` lines 338 to 343: `sent as u64`). `RECEIVE`
   replies 0 when the stream has closed or `recv_slice` fails (`socket_operations.rs` lines 94 to
   97, `unwrap_or(0)`). Every in-tree client treats 0 as end of stream (`socket_test_client.rs` line
   414, `components/src/domain_name_resolver.rs` line 400), yet the contract never says so. `OPERATION_ATTACH_PAGE_FRAME` sent as a `CALL` answers `REP_ERR`
   (`socket_server.rs` line 197), which its documentation does not state.
   Evidence: the lines above.
   Change: one "Replies" section in the crate documentation and the README, listing per operation
   the reply word's meaning, with 0 from `RECEIVE` as end of stream and a short count from `SEND`
   as "send the rest". Lands with item 1.
   Door: two-way (it documents the wire as spoken). Effort: S.

5. The README does not yet meet the template's scaffold. LEFT: README scaffold sections belong to the move lane's staged tree.
   Finding: against `scaffold/README-crate.md` the README lacks several things. It has no badge row
   and no `rustup target add` line before the bare-metal build. It does not say how long `cargo
   kani` takes or its peak memory. It does not say how to run one harness (`cargo kani --harness
   verification::<name> --exact`). It has no worked falsification example with what the failing run
   prints. It does not say what a passing run prints that looks alarming (the future-incompatibility
   warning this review saw). It does not say how to check the minimum Rust version.
   The "Each patch under `falsifications/` ... applying it with `patch -p1` from the repository
   root" sentence is true for nife's paths; `script/crate-move` rewrites the paths, and the
   sentence should name the command and the undo (`git checkout -- src`).
   Evidence: `README.md` lines 55 to 72; the scaffold.
   Change: the scaffold's sections, filled with this crate's figures. The stranger test then has
   a README to fail against.
   Door: two-way. Effort: S.

6. The three surviving mutants need exemptions, or the release gate reads a red verdict. LEFT: `.github/mutants-exempts` exists only in the new repository.
   Finding: the census records 93 mutants, 87 caught, 3 missed, 0 timeouts, 3 unviable, in three
   consecutive weekly runs. The triage classifies all three as equivalent. Two are `|` to `^` in
   `listen_grant` (line 147) and `udp_bind_grant` (line 195), where the operands have no bit in
   common. One is `<` to `<=` in `polled_wait::next` (line 250), where both arms return
   `Sleep(poll_microseconds)` at equality. The review package quotes an older row (84 tested, 80
   caught, 2 missed). The template's `mutation` verdict judges survivors against
   `.github/mutants-exempts`, and the release gate requires that verdict green.
   Evidence: `notes/project-metrics/mutation-census.csv` rows for `socket_contract`;
   `notes/project-metrics/mutation-triage.csv`; the template README's release path.
   Change: at the move, write the three into `.github/mutants-exempts` with the triage's reasons.
   For the two `|` sites, `+` would be a wrong fix (it reads as arithmetic where the word is a
   bit field); the exemption is right. Update the package's figure to the census.
   Door: two-way. Effort: S.

### P2: for later

The P2 items are in [the appendix](1108-socket-contract-outside-review/later-items.md).

## Decisions for the architect

Only one-way doors. The package's five decisions stand and are not repeated; these two are
additions it did not raise.

### D1. Where a send's length travels (ruled 2026-11-11 UTC as recommended)

Background. A client fills the shared frame with a payload and asks the stack to send it. The
stack must learn how many bytes to send. The frame has a `len` field at `LENGTH_OFFSET_BYTES`, and the `CALL`
has an argument word. The crate's text says the field; the stack, the std platform layer and the
test client all use the argument word, and nobody writes the field before a send. The two agree
only by accident of one tree holding both sides. Once the crate leaves, a second
implementation follows the text.

Recommendation: ratify the argument word as the carrier, and make `LENGTH_OFFSET_BYTES` an output of
`RECEIVE` only. The argument arrives with the request atomically and the kernel's
word bounds it, so the stack checks one value against `DATA_MAXIMUM` before it touches the frame. A length in
the frame is a second copy of the same fact that a client could leave stale. The alternative,
the field, would make every in-tree client change for no gain and keep two places where a length
can live.

Prior art: `send(2)` and `write(2)` take the length as an argument and return the count accepted;
`smoltcp::socket::tcp::Socket::send_slice` and `embedded-nal`'s `send` take a slice and return the
count; io_uring's submission entry carries its length beside the buffer address (unverified).

Example: a client writes `b"nife-out!"` at `PAYLOAD_OFFSET_BYTES` and issues `CALL(connection,
OPERATION_SEND, 9)`; the stack replies 9, or fewer if its buffer is short, and the client sends
the rest. It never touches `LENGTH_OFFSET_BYTES`.

### D2. Whether `ACCEPT` tells the client who connected (ruled 2026-11-11 UTC as recommended)

Background. A listener's `ACCEPT` replies with the new connection's capability and nothing else.
The connection has no frame yet, so the peer's address has nowhere to ride, and std reports
`0.0.0.0:0` as the peer of every accepted stream. A server that logs who connected, applies a
per-peer limit, or speaks a protocol that embeds the peer address (FTP, SIP) cannot. The tree
already holds this as a contract change.

Recommendation: carry the peer in the second reply word. Pack it as the IPv4 address big-endian in
bits 16 to 47 and the port in bits 0 to 15 (`peer_word`, `word_peer`, names provisional), with a
harness proving the round trip for every address and port. The word is already on the wire and
always 0; filling it changes no client that ignores it. The alternative, writing the peer into the
connection's frame once attached, delays the fact until a second operation and ties it to a frame
the client may not attach.

Prior art: `accept(2)` returns the peer's `sockaddr` with the new descriptor; `embedded-nal`'s
`TcpFullStack::accept` returns `(socket, SocketAddr)`; Fuchsia's socket `Accept` returns the
address (unverified).

Example: a client calls `CALL(listener, OPERATION_ACCEPT, 0)` and receives `(REP_OK,
0x0a00_0202_c3f1)`: the peer is `10.0.2.2:50161`, and `word_peer` says so.

## Done when

The checks are in [the appendix](1108-socket-contract-outside-review/done-when.md).

## BUGS

- The package's decision 2 moves `fixture` out before the crate leaves. If the ruling goes the other
  way, item 9's `IN_MESSAGE` and `OUT_MESSAGE` and the package's own `nife-in!` and `nife-out!`
  finding become real work; this block does not size it.
- Item 7's second harness depends on CBMC handling 64-bit division in `clock`; the review did not
  try it. If it is slow, bound `milliseconds` to 2^40 (35 years) with `kani::assume`.
- The review ran the crate's tests on the workspace nightly only. The package's 1.88 run covers
  the same code, and no source line has changed since.
- The prior-art rows for Linux's port range and Fuchsia's socket protocol are from memory. The
  crates.io figures and the smoltcp and embedded-nal signatures come from a check.

## Follow-on

What is left: the move steps (items 2, 5, 6), the names (item 9), the inline provisional prose
(item 10), and the parts of items 7 and 8 marked LEFT in the appendix.

- **Milestone 1017.** The crate moves in its order, and the move lane takes items 2, 5, 6 and 10.
- **Outstanding.** Item 7's loop harness and no-reuse arithmetic harness, `Ports::next` returning
  an option, and item 8's single offset type: checked against `crates/socket_contract/src/lib.rs`,
  none exists.
- **Refused.** Nothing here renames; item 9 waits for the naming pass.

## Index row

An outside review of `socket_contract` before its move. The text says a send's length rides in the
frame while every implementation passes it as the argument word, which blocks the move until ruled.
A manifest comment points into nife. Nine more items follow: the peer address `ACCEPT` does
not return, the reply words left unstated, the README against the scaffold, three equivalent mutants
to exempt, a proof for the ephemeral cursor, and one type for offsets and lengths.
