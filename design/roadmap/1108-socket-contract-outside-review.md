---
status: NOT-STARTED
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
[the review package](../../notes/crate-reviews/socket-contract.md) and raises none of the
decisions it puts to the architect: the identity values (`INTERFACE_ID` 19, `VERSION` 1), where
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

1. The contract's text and its wire disagree on where a send's length travels.
   Finding: the layout note at `crates/socket_contract/src/lib.rs` lines 454 to 459 says the
   `len` field at `OFF_LEN` is "in for SEND*, out for RECEIVE". The documentation of
   `OPERATION_SENDTO` and `OPERATION_SEND` (lines 53 to 60) says "payload in the frame" and
   nothing about the argument word. The stack never reads `OFF_LEN`: `components/src/socket_server.rs`
   lines 172 and 182 pass the `CALL`'s argument as the length, and `socket_operations.rs` lines
   45 and 328 bound that argument by `DATA_MAXIMUM`. The std platform layer and the test client
   both pass the length as the argument and never write `OFF_LEN` (`nife.rs` lines 496 and 856,
   `socket_test_client.rs` line 406). A client written from the crate's text, which writes
   `OFF_LEN` and passes 0, sends nothing and hears that it sent nothing. The only writer of `OFF_LEN`
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
   `OFF_LEN`'s documentation and `OPERATION_SEND`'s agree (a doc test that sends from a fake frame
   is enough). The README's "Replies" bullet changes with it. This is item D1 below, since the
   choice of which side carries the length is a wire-format door; the recommendation is to ratify
   the implementation.
   Door: the text is two-way; the wire choice it records is one-way. Effort: S.

2. The manifest's `publish` comment points outside the repository.
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

3. `ACCEPT` carries no peer address, and the tree already lists it as a contract change.
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

4. The reply vocabulary leaves two outcomes a client must handle unstated.
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

5. The README does not yet meet the template's scaffold.
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

6. The three surviving mutants need exemptions, or the release gate reads a red verdict.
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

7. The one piece of logic beyond the grants has no proof.
   Finding: the three harnesses prove the grant packing and checks, which is the authority. The
   frame layout, the badge predicate and the lease round trip are tests of fixed values, which is
   enough for constants. `ephemeral::Ports::next` (lines 331 to 341) is a loop with a clock, a
   modulus and a fallback, proven by nothing. Its central claim is that a stack started a clock tick
   after another hands out none of its ports. A sampled test covers that claim over five start times
   and 1,997 gaps (lines 388 to 401). The fallback at line 340 returns a port the `held` predicate refused
   when every port is in use, silently.
   Evidence: the lines above; `README.md` "What is proven".
   Change: two bounded harnesses. One: for any cursor position, any `milliseconds` and any
   `held` answer, the port returned lies in `LO..=HI` and the cursor never moves backwards. Two:
   for any `last` and any `gap` of at least `1000 / PER_SECOND` rounded up, `clock(last + gap)`
   exceeds the position a cursor reaches after `n` opens `g` apart when `g * PER_SECOND >= 1000`,
   which is the no-reuse claim as arithmetic rather than samples. Each with a falsification patch
   (the existing ephemeral patch falsifies the test and can falsify the harness too). Document
   the fallback, or make `next` return `Option<u16>` and let the stack answer `REP_ERR`; the
   second is the honest shape and costs one `match` in `socket_table.rs`.
   Door: two-way (an unreleased signature). Effort: M.

8. Offsets, lengths and byte orders mix types and go unstated.
   Finding: `OFF_*` are `u64` and `DATA_MAXIMUM` is `usize`, so every caller casts
   (`DATA_MAXIMUM as u64` at five sites in `components/`, `OFF_PAYLOAD as usize` in the crate's
   own line 471). `DATA_MAXIMUM = 4096 - OFF_PAYLOAD` hardcodes the page size with no constant for
   it; `user_mode_runtime` brings its own `PAGE`. The text gives the byte order for `dst_port` alone
   ("little-endian", line 456). It gives none for `len`. The std layer comments that it matches
   (`nife.rs` line 219). The lease word is big-endian in the low 32 bits, so one contract carries
   two byte orders without saying it has two.
   Evidence: `lib.rs` lines 454 to 471 and 534 to 537; `socket_frame.rs` line 35.
   Change: one type for offsets and lengths (`usize`, as `core::ptr::add` and slices take). Add a
   `FRAME_BYTES` constant (name provisional) of 4096 that `DATA_MAXIMUM` derives from and the client
   runtime can import. Add one sentence to the layout note: every multi-byte frame field is
   little-endian, and a `lease` word carries the address big-endian in its low 32 bits because that
   is the way an address reads.
   Door: two-way before the release; the types become one-way after. Effort: S.

9. Names where this review reads a later rule differently from the package.
   Finding: the package proposes keeping `Ports::next` and `polled_wait::next`. The rule that a
   method takes std's spelling asks for std's meaning too; `Iterator::next` takes no argument and
   returns `Option`, while `Ports::next(milliseconds, held)` takes two and returns a `u16`, and
   `polled_wait::next(now, last_frame, tick, ...)` is a pure decision. `fixture::IN_MSG` and
   `OUT_MSG` abbreviate `message`; the package proposes nothing for `fixture` because its decision
   2 moves the module. `udp`, `tcp` and `ip` appear in public names and are in neither the
   template's `[spelled_out]` nor its `[accepted]` list, so the gate neither refuses nor records
   them.
   Evidence: `lib.rs` lines 239, 331, 488, 490; `vendor/crate-workflows/helpers/naming_words.toml`
   `[accepted]`; the package's names table.
   Change: proposals for the architect's naming pass, all provisional. Rename to `Ports::allocate`
   (it acts and reports, like `claim`), and `polled_wait::decide` or `wait_decision`. Use
   `IN_MESSAGE` and `OUT_MESSAGE` if `fixture` stays long enough to matter. Add `udp`, `tcp` and
   `ip` to the template's `[accepted]` with their reasons ("nobody says User Datagram Protocol in a
   name"), so the rule has a record rather than passing by silence.
   Door: two-way. Effort: S, in the package's rename lane.

10. Inline "name provisional" prose survives the move.
    Finding: the move strips `Name:` blocks. The crate also carries the words inside ordinary
    sentences: "Names provisional." (line 65), "Name provisional." (lines 74, 84), "Names
    provisional." (line 440), "The module's name is provisional." (lines 213, 292), and
    "provisional, as is the name" (line 516). A stranger reads them in rustdoc with no idea whose
    ruling is pending. Line 22 names the decider inside the crate docs.
    Evidence: the lines above.
    Change: move each into the item's `Name:` block, which the move strips, or delete it at the
    move. Line 22 becomes a `Name:` block in the recorded form.
    Door: two-way. Effort: S.

11. The documentation refers to things the crate does not define.
    Finding: `PageFrame` (line 451) is a kernel type; `SEND_CAPABILITY`, `CALL`, `WRITE | GRANT`,
    `BADGE`, `UNMAP` (lines 46, 442 to 448) are kernel operations and rights; "the kernel-stamped"
    badge assumes the reader knows the kernel stamps badges. The package lists `PageFrame`. A
    reader of the crate alone has no link to follow.
    Evidence: the lines above.
    Change: one "Vocabulary" paragraph in the crate documentation. It says, in a sentence each, what
    a capability, a badge, a `CALL` and the shared frame are in the system this contract assumes. It
    also says which crate (`abi`) names the operations, without a path into nife.
    Door: two-way. Effort: S.

## Decisions for the architect

Only one-way doors. The package's five decisions stand and are not repeated; these two are
additions it did not raise.

### D1. Where a send's length travels

Background. A client fills the shared frame with a payload and asks the stack to send it. The
stack must learn how many bytes to send. The frame has a `len` field at `OFF_LEN`, and the `CALL`
has an argument word. The crate's text says the field; the stack, the std platform layer and the
test client all use the argument word, and nobody writes the field before a send. The two agree
only by accident of one tree holding both sides. Once the crate leaves, a second
implementation follows the text.

Recommendation: ratify the argument word as the carrier, and make `OFF_LEN` an output of
`RECEIVE` only. The argument arrives with the request atomically and the kernel's
word bounds it, so the stack checks one value against `DATA_MAXIMUM` before it touches the frame. A length in
the frame is a second copy of the same fact that a client could leave stale. The alternative,
the field, would make every in-tree client change for no gain and keep two places where a length
can live.

Prior art: `send(2)` and `write(2)` take the length as an argument and return the count accepted;
`smoltcp::socket::tcp::Socket::send_slice` and `embedded-nal`'s `send` take a slice and return the
count; io_uring's submission entry carries its length beside the buffer address (unverified).

Example: a client writes `b"nife-out!"` at `OFF_PAYLOAD` and issues `CALL(connection,
OPERATION_SEND, 9)`; the stack replies 9, or fewer if its buffer is short, and the client sends
the rest. It never touches `OFF_LEN`.

### D2. Whether `ACCEPT` tells the client who connected

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

## Index row

An outside review of `socket_contract` before its move. The text says a send's length rides in the
frame while every implementation passes it as the argument word, which blocks the move until ruled.
A manifest comment points into nife. Nine more items follow: the peer address `ACCEPT` does
not return, the reply words left unstated, the README against the scaffold, three equivalent mutants
to exempt, a proof for the ephemeral cursor, and one type for offsets and lengths.
