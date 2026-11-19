# P1: before the first release

An appendix to [the milestone](../1108-socket-contract-outside-review.md).

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
