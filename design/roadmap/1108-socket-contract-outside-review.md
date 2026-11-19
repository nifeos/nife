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

## Finding status

The ids P0-1 to P2-11 are this review's items; PKG-1 to PKG-5 are the decisions of the review
package, `notes/crate-reviews/socket-contract.md`.

- P0-1, text and wire disagree on where a send's length travels: Done,
  [#3517](https://github.com/nifeos/nife/pull/3517) (D1).
- P0-2, the `publish` comment points outside: Open. The comment and `repository` are in place; the
  new repository's caller passes `allow-unpublished` at the move.
- P1-3, `ACCEPT` carries no peer: Done, [#3526](https://github.com/nifeos/nife/pull/3526), its
  proof in [#3580](https://github.com/nifeos/nife/pull/3580) (D2).
- P1-4, the reply vocabulary: Done, [#3517](https://github.com/nifeos/nife/pull/3517).
- P1-5, the README against the scaffold: Open. The badges, the `rustup target add` line, the Kani
  time and peak, the one-harness command, a falsify example and the undo remain, then a stranger
  run; a move step.
- P1-6, three equivalent mutants: Open. `.github/mutants-exempts` exists only in the new
  repository; the move exempts them with the triage's reasons.
- P2-7, the ephemeral cursor unproven: Done, [#3580](https://github.com/nifeos/nife/pull/3580) and
  [#3610](https://github.com/nifeos/nife/pull/3610).
- P2-8, offsets and lengths of mixed types: Open. `FRAME_BYTES` and the byte order are in place;
  the offsets stay `u64` while `DATA_MAXIMUM` is `usize`, and one type waits on a change to
  `user_mode_runtime`'s window API that no milestone holds.
- P2-9, names: Open. `Ports::next`, `polled_wait::next` and, if `fixture` stays, `IN_MSG` and
  `OUT_MSG` keep their spellings; two-way before the first tag, so the maintainer rules them
  provisionally. `tcp`, `udp` and `ipv4` stay abbreviated (ruled 2026-11-16 UTC).
- P2-10, inline "provisional" prose: Open. Seven doc comments in `src/lib.rs` still say it; the
  move removes them with the `Name:` blocks.
- P2-11, vocabulary undefined: Done, [#3580](https://github.com/nifeos/nife/pull/3580).
- D1, a send's length in the argument word: Done,
  [#3517](https://github.com/nifeos/nife/pull/3517) (ruled 2026-11-11 UTC).
- D2, the `ACCEPT` peer in the second reply word: Done,
  [#3526](https://github.com/nifeos/nife/pull/3526) (ruled 2026-11-11 UTC).
- PKG-1, `INTERFACE_ID` 19 and `VERSION` 1: Open, and an open decision below.
- PKG-2, `fixture` moves to a crate that stays in nife: Open. `pub mod fixture` is still in the
  crate; a two-way door the maintainer rules provisionally, then a lane moves it.
- PKG-3, `ephemeral` and `polled_wait` move to the stack's own crate: Open. Both modules are still
  in the crate; the same two-way ruling and move as PKG-2.
- PKG-4, the package's names table: Open. Two-way before the first tag, ruled by the maintainer
  with P2-9. The table's four `_OFFSET` rows give way to the family's `<FIELD>_OFFSET_BYTES`
  (ruled 2026-11-14 UTC), which the crate already carries.
- PKG-5, clearing the move with no tag and no publish: Open, and an open decision below.

## Open decisions

### The socket contract's identity: interface 19, version 1

Every contract crate in this tree carries two numbers that a program checks when it links to the
other side: an interface number that says which contract this is, and a version that says which
revision of it. The socket contract is the agreement between the network stack and every program
that opens a socket, including the standard library's platform layer, which copies this crate's
source verbatim. Its numbers today are `INTERFACE_ID` 19, the row milestone 956 (the linked
interfaces carry checkable versions) gives it, and `VERSION` 1. Both are provisional. No operation
sends `VERSION` on the wire; the bind-time check that will read it belongs to milestone 1000
(message contracts carry bind-time version checks).

The package records no magic for the socket frame, as `entropy_contract` has none. Milestone 1017
asks for a frozen identity before a contract leaves.

Recommendation: A.

- A. Freeze `INTERFACE_ID` 19 and `VERSION` 1, with no magic.
- B. Freeze the same two numbers and add a magic word at the head of each frame.
- C. Leave them provisional until milestone 1000 builds the bind-time check.

Door: one-way. A second network stack, or a client built against the crate in its own repository,
bakes both numbers in; changing either after that breaks every program built before.

Prior art: Fuchsia gives each FIDL protocol a fixed name and each method an ordinal, and a peer
that disagrees fails the call; Android's Binder sends an interface descriptor string with each
transaction. nife differs by checking the pair once at bind time instead of on each message. In
this tree, `entropy_contract` froze 12 and 1 with no magic, and `clock_contract` 8 and 1 with one.
Example: a client linked against version 1 that meets a stack reporting version 2 refuses at bind
time instead of misreading a reply word.

Blocks: the move, and with it any tag and the first publish.

### Clearing the move of `socket_contract`

Clearing a crate's review package is what approves its repository, here
`nifeos/socket_contract`. The package says the crate builds, proves and stands alone, and it
recommends against clearing yet. It waits on the identity above, on where `fixture`, `ephemeral`
and `polled_wait` live (PKG-2 and PKG-3), and on its names (P2-9 and PKG-4). The package also lists
a maintainer step: the `std` farm must read the pinned checkout's source, or the move breaks the
platform layer's build.

Recommendation: A.

- A. Clear once the architect freezes the identity and PKG-2, PKG-3, P2-9 and PKG-4 have
  provisional rulings and have landed, with no tag and no publish.
- B. Clear on the identity alone, and let the scope moves and renames land in the new repository.
- C. Hold the clearance until the first publish is near, and move nothing before then.

Door: one-way. A public repository is visible and forkable the moment it exists, and the eight
path dependencies in nife switch to it.

Prior art: smoltcp keeps its socket types and the stack in one crate, which is the shape C keeps;
`embedded-nal` publishes the client-side traits alone, apart from any stack, which is closer to
A. nife differs by keeping one contract crate that both sides link, with the stack's private logic
outside it. In this tree, `block_roster_contract` and `entropy_contract` each moved after a cleared
package.

Blocks: the move, and with it any tag and the first publish.

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

2. The manifest's `publish` comment points outside the repository. PARTIAL: the comment and `repository` are in place. LEFT: the caller's `allow-unpublished`, at the move.
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

The P1 items are in [the appendix](1108-socket-contract-outside-review/before-the-release.md).

### P2: for later

The P2 items are in [the appendix](1108-socket-contract-outside-review/later-items.md).

## Decisions for the architect

Only one-way doors, both ruled. The package's decisions 1 and 5 are under "Open decisions" above;
its decisions 2, 3 and 4 are two-way doors the maintainer rules provisionally. These two are
additions the package did not raise.

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

What is left is under "Finding status" above.

- **Milestone 1017.** The crate moves in its order, and the move lane takes items 2, 5, 6 and 10.
- **Outstanding.** Item 8's single offset type, which waits on the window API's offset type.
  Item 7's loop proof is bounded at four candidates, as the appendix says.
- **Outstanding.** Item 9's names and the package's decisions 2 to 4, which the maintainer rules
  provisionally, checked against `crates/socket_contract/src/lib.rs`.
- **Outstanding.** The identity values and the clearance, under "Open decisions" above.

## Index row

An outside review of `socket_contract` before its move. The text says a send's length rides in the
frame while every implementation passes it as the argument word, which blocks the move until ruled.
A manifest comment points into nife. Nine more items follow: the peer address `ACCEPT` does
not return, the reply words left unstated, the README against the scaffold, three equivalent mutants
to exempt, a proof for the ephemeral cursor, and one type for offsets and lengths.
