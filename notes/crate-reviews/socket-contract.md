# Review package: `socket_contract`

*Name: provisional, minted by lane lane/review-prep-socket-contract. The `socket_contract` package
of [crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow
the page.*

The architect reads this to clear the crate for its move to its own repository, as part of step 2
of [milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as `main` plus this lane's pull request leaves it.

## In one paragraph

The wire contract between a network stack and its clients, where each socket is its own
capability. A client holds a front door that only makes sockets and one capability per open
socket. The contract holds eleven opcodes, the reply words, a 4 KiB shared page per socket and a
spawn word that grants inbound ports. It is
`no_std`, has no dependency and no `unsafe`, and three Kani harnesses prove the port grants. Besides
the contract it carries three things only one side uses: the network stack's ephemeral-port cursor
(`ephemeral`), its polled-wait policy (`polled_wait`), and the fixed addresses and messages of one
test harness (`fixture`). Nothing in the crate points back into nife, but nife reads its source
file by path to build the standard library's platform layer.

## Identity

| item | value |
|---|---|
| crate | `socket_contract`, ratified |
| identity values | `INTERFACE_ID` 19 and `VERSION` 1, frozen with no magic word (ruled 2026-11-19 UTC). Row 19 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table names this crate, so the value agrees. No operation carries `VERSION` on the wire: the bind-time check belongs to [the message contracts carry bind-time version checks](../../design/roadmap/1000-message-contracts-carry-bind-time-version-checks.md) |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0`, both texts at the crate root; nothing adapted |
| `publish` | `false`; milestone 1017 keeps moved contract crates unpublished until their own release milestone |
| size | one `src/lib.rs` of 874 lines: about 440 of library, 380 of tests, 50 of Kani harnesses |
| package | `cargo package` packs 12 files, 63.2 KiB (20.2 KiB compressed), the four patches included |
| mutation baseline | 80 caught, 2 missed, 0 timeouts, 2 unviable |

## Who in nife depends on it

| kind | where |
|---|---|
| path dependency | `components`, `kernel`, `fixtures`, `crates/system_initializer`, `crates/user_mode_runtime`, `jig`, `std_exerciser`; `system_tests` as a development dependency. `jig` and `std_exerciser` carry no `version` key |
| source copied into `std` | `cargo xtask std-src` writes `src/lib.rs` verbatim into the patched `std` as `netproto.rs` and stamps it, from the directory `cargo metadata` gives for the package (`xtask/src/farm/copied.rs`), so the move changes nothing there |
| prose and tables | `helpers/trust_boundary_crates.tsv`, `helpers/interface_stability.py`, `helpers/lint_externalized_crates.py`, `helpers/template_report_baseline.txt`, `packages/contracts.package.toml`, `crates/externalization-list.txt`, the doc comments of `domain_name_resolution_contract` |
| users of the moved `fixture` | now `socket_fixture` (see below): `components/src/socket_test_client.rs`, `fixtures`, `jig`, `std_exerciser`, `system_tests` (network and riscv virtio tests); `xtask/src/inbound.rs` spells the same values for the host side |
| users of the moved `ephemeral_port` and `polled_wait` | the network stack alone, which now reads them from `network_stack`: `components/src/socket_server.rs`, `components/src/socket_table.rs`, `components/src/nic_wait.rs` |

No test reads a `../../` file, no `include_str!` leaves the crate (the one there reads its own
README), and nife's fuzz workspace has no target for it.

## What was run

The first table is what the review lane ran before the scope moves and renames; it counts the crate as it was then. The second table is what the lane `1108-renames-scope` ran on the crate as it is now.

| check | result |
|---|---|
| `script/crate-standalone socket_contract` | one failure, `names`: 55 items not ratified. Manifest, files, citations, and the build on 1.88 for the host and the three bare-metal targets pass; Kani 3 of 3 prove; all four falsification patches turn their check red |
| `script/crate-move --dry-run` | builds and tests at 1.88, and every patch applies with no offset and no fuzz. Three findings, all known template bugs (the scaffold's mentions of nife, its architect line and its path into nife), and two crate lines: the `fixture` message bytes `nife-in!` and `nife-out!` |
| `cargo test -p socket_contract` | 20 unit tests and 5 doctests pass; one doctest is the README's example |
| `cargo clippy --all-targets`, `cargo doc` with warnings denied | no warning |
| the template's naming checks over `src/lib.rs` | `spelled_out`, `unit_symbols`, `question_shape`, `british_spelling` and `contract_names`: 169 names, 0 refused. `register_names` refuses `LENGTH_OFFSET_BYTES` (a length carries its unit). `abbreviations`, report only: 20 components |
| `script/names socket_contract` | the crate name ratified; three items provisional; one ratified; the rest have no block |
| `script/lint` | passes |

Not run, and so not claimed: the stranger test, `cargo-semver-checks` (a first release has no
baseline) and the template's workflows, which run in the repository.

The lane `1108-renames-scope` moved three modules out and applied the names table below. Draft
pull request [#3811](https://github.com/nifeos/nife/pull/3811) carries it. The maintainer ruled the
moves and the renames provisionally and as two-way doors on 2026-11-19 UTC; the architect ratified the names table as built on 2026-11-20 UTC.

| check | result |
|---|---|
| `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo doc` with warnings denied | `socket_contract`, `network_stack`, `socket_fixture`, `xtask` and the other dependents on the host: no failure, no warning |
| `cargo check` for the bare-metal programs | `components`, `fixtures`, `system_tests` and `kernel` on `aarch64-unknown-none-softfloat`: pass. The RISC-V target is not installed on this machine, so the lane checked only what its own jobs run |
| `script/falsifications --check` | every patch applies, after the moved and renamed ones were re-cut |
| Kani, Miri, `script/crate-standalone`, `script/crate-move --dry-run`, the QEMU gates, `script/lint` | see the pull request's checks; this table is updated when each result is in |

### What this lane changed in the crate

- The text no longer names nife, except the two message bytes above. The text expands eight acronyms
  at first use, and a test comment no longer names a machine.
- `VERSION`'s documentation described a bind-time call that the crate does not define. It now
  says no operation carries the version, and BUGS says so.
- The ephemeral-port patch applied 14 lines off; it applies exactly now.
- Three Kani harnesses, each with a patch: a listen grant admits exactly its range and no UDP port;
  a bind grant the same; the halves of a composed word answer independently.
- `script/crate-standalone` ran a host test's patch through Kani and reported a compile error. It
  now replays such a patch with `cargo test` and requires the test to fail.

## Public API

| kind | items |
|---|---|
| identity | `VERSION`, `INTERFACE_ID` |
| operations | `OPERATION_ATTACH_SHARED_PAGE`, `OPERATION_OPEN_UDP`, `OPERATION_OPEN_TCP`, `OPERATION_SEND_TO` (alias `sendto`), `OPERATION_RECEIVE`, `OPERATION_CONNECT`, `OPERATION_SEND`, `OPERATION_CLOSE`, `OPERATION_LISTEN`, `OPERATION_ACCEPT`, `OPERATION_BIND_UDP` (1 to 11) |
| badges | `SOCKET_BADGE_BIT` (bit 63), `names_a_socket(badge)` |
| replies | `REPLY_OK`, `REPLY_ERROR` (bit 32), `CONNECT_ESTABLISHED`, `CONNECT_REFUSED`, `PORT_CLAIM_GRANTED`, `PORT_CLAIM_DENIED`, `PORT_CLAIM_IN_USE` |
| port grants | `NO_PORT_GRANT`, `listen_grant(start, end)`, `listen_grant_allows(grant, port)`, `udp_bind_grant(start, end)`, `udp_bind_grant_allows(grant, port)` |
| shared page | `DESTINATION_IPV4_ADDRESS_OFFSET_BYTES`, `DESTINATION_PORT_OFFSET_BYTES`, `PAYLOAD_LENGTH_OFFSET_BYTES`, `PAYLOAD_OFFSET_BYTES`, `MAXIMUM_PAYLOAD_BYTES` |
| `stack_slots` | `MINTING_ENDPOINT_SLOT` (7), `OWN_ADDRESS_SPACE_SLOT` (8) |
| `lease` | `NO_NAME_SERVER`, `ipv4_to_word(octets)`, `word_to_ipv4(word)` |

Three modules left the crate. `ephemeral_port` and `polled_wait` are in `network_stack`
(`crates/network_stack`, the stack's own host-testable crate, `publish = false`):

| module | items |
|---|---|
| `ephemeral_port` | `LOWEST_PORT`, `HIGHEST_PORT`, `PORT_COUNT`, `PORTS_PER_SECOND`, `Cursor`, `Cursor::new`, `Cursor::next(milliseconds, held)` |
| `polled_wait` | `Next::{Look, SleepMicroseconds(u64)}`, `next(now_ticks, last_frame_ticks, ticks_per_scheduler_tick, stack_due_microseconds, poll_microseconds)` |

`fixture` is now the crate `socket_fixture` (`crates/socket_fixture`, `publish = false`, a
development dependency on `socket_contract`): `LISTEN_PORT`, `DENIED_PORT`, `INBOUND_MESSAGE`,
`OUTBOUND_MESSAGE`, `ROUNDS`, `ECHO_PEER_ADDRESS`, `ECHO_PEER_PORT`, `PACKAGE_PEER_ADDRESS`,
`PACKAGE_PEER_PORT`, `PACKAGE_PEER_HOST`. The three crate names are provisional; the architect ratified the items (2026-11-20 UTC).

No nife type appears in a signature. What a stranger may find surprising:

- `CONNECT_ESTABLISHED` and `REPLY_OK` share the value 0, and so do `PORT_CLAIM_GRANTED` and
  `NO_PORT_GRANT`: a client reads each vocabulary only in reply to its own operation.
- The `PORT_CLAIM_` replies answer `OPERATION_BIND_UDP` as well as `OPERATION_LISTEN`, and `NO_PORT_GRANT` is the empty word
  for both halves of the grant.
- The documentation refers to a `PageFrame` that the crate does not define; it means the shared
  page.
- Under `cargo kani`, the compile-time `assert!` in `ephemeral_port` draws a future-incompatibility
  warning from Kani's own `assert` macro. Plain `cargo` builds do not warn.

## Rulings

- A send's length travels in the `CALL` argument word, checked against `MAXIMUM_PAYLOAD_BYTES`, and
  `PAYLOAD_LENGTH_OFFSET_BYTES` is an output of `RECEIVE` only (the architect, 2026-11-11 UTC). The crate's layout
  note, every `OPERATION_` constant's documentation, the README and the client runtime's frame
  accessor state it, and a documentation test on `OPERATION_SEND` sends with the length in the
  argument. 
- `ACCEPT`'s second reply word carries the peer: the IPv4 address in bits 47 to 16 and the port in
  bits 15 to 0, packed by `peer_word` and read by `word_peer` (the architect ratified both names 2026-11-20 UTC), the architect's
  ruling of 2026-11-11 UTC. The frame's `dst` fields are not used for it, and the word stays the
  IPv4 answer; another family would need a second route.

## Names

The architect ratified the crate name on 2026-08-01 and every item in the table below, as built, on
2026-11-20 UTC. Each item's `Name:` block reads ratified; the move strips the blocks. The three
crate names (`socket_contract` is ratified; `network_stack` and `socket_fixture` are provisional)
and the `Cursor` and `Next` types' own spelling follow the table. A release review reopens the
names ratified before a later rule, such as `MAXIMUM_PAYLOAD_BYTES`, which replaced a name ratified
before the rule that a limit opens `MAXIMUM_` and ends with its unit.

| item before | built | rule |
|---|---|---|
| `VERSION`, `INTERFACE_ID` | keep | the names `block_roster_contract` ratified |
| `OPERATION_SENDTO` | `OPERATION_SEND_TO`, with `#[doc(alias = "sendto")]` | two words; POSIX's `sendto` kept as the alias |
| `OPERATION_ATTACH_SHARED_PAGE` | keep (ruled 2026-11-19 UTC) | both contracts spell the one act the same way, and here `frame` also means a network frame (`polled_wait`) |
| the other ten `OPERATION_` constants | keep | `udp` and `tcp` pass the acronym test, as `pci` does; add both to the template's accepted words |
| `SOCKET_BADGE` | `SOCKET_BADGE_BIT` | it is the bit; a socket's badge is that bit with others set |
| `names_a_socket` | keep | a third-person verb phrase |
| `REP_OK`, `REP_ERR` | `REPLY_OK`, `REPLY_ERROR` | `rep` and `err` abbreviate; `REPLY_` is the tree's prefix elsewhere |
| `CONNECT_ESTABLISHED`, `CONNECT_REFUSED` | keep | |
| `LISTEN_GRANTED`, `LISTEN_DENIED`, `LISTEN_IN_USE` | `PORT_CLAIM_GRANTED`, `PORT_CLAIM_DENIED`, `PORT_CLAIM_IN_USE` | they answer a bind as well as a listen; the documentation already calls them the port-claim vocabulary |
| `NO_LISTEN_GRANT` | `NO_PORT_GRANT` | the empty word for both halves |
| `listen_grant`, `udp_bind_grant` | keep; parameters `lo`, `hi` become `start`, `end` | the standard library's words for an inclusive range |
| `grant_allows`, `udp_grant_allows` | `listen_grant_allows`, `udp_bind_grant_allows` | each names the grant its packer makes |
| `DESTINATION_ADDRESS_OFFSET_BYTES`, `DESTINATION_PORT_OFFSET_BYTES`, `LENGTH_OFFSET_BYTES`, `PAYLOAD_OFFSET_BYTES` | `DESTINATION_IPV4_ADDRESS_OFFSET_BYTES`, `DESTINATION_PORT_OFFSET_BYTES`, `PAYLOAD_LENGTH_OFFSET_BYTES`, `PAYLOAD_OFFSET_BYTES` | **Differs from the proposal.** The proposal dropped `_BYTES`; the family-wide `<FIELD>_OFFSET_BYTES` spelling (ruled 2026-11-14 UTC) governs, so the lane kept it and renamed only the stems. The address stem is `IPV4_ADDRESS`, since `ip` abbreviates and the field holds an IPv4 address (ruled 2026-11-20 UTC) |
| `DATA_MAXIMUM` | `MAXIMUM_PAYLOAD_BYTES` | a limit opens `MAXIMUM_` and ends with its unit |
| `stack_slots::MINT`, `OWN_SPACE` | `MINTING_ENDPOINT_SLOT`, `OWN_ADDRESS_SPACE_SLOT` | `mint` is a verb and `space` half a term; `_SLOT` as in `ACTIVATION_SLOT` |
| `lease::NO_NAMESERVER` | `NO_NAME_SERVER` | two words |
| `lease::ipv4_word`, `word_ipv4` | `ipv4_to_word`, `word_to_ipv4` | the pair reads as each other's inverse |
| `ephemeral` | `ephemeral_port`, in `network_stack` | a module takes a noun |
| `LO`, `HI`, `SPAN`, `PER_SECOND` | `LOWEST_PORT`, `HIGHEST_PORT`, `PORT_COUNT`, `PORTS_PER_SECOND` | abbreviations, and a rate names what it counts |
| `Ports`, `Ports::new`, `Ports::next` | `Cursor`; `new` and `next` kept | the documentation calls it the cursor; `next` advances it and returns the next value, as an iterator's does |
| `polled_wait`, `Next::Look` | keep | |
| `Next::Sleep(u64)` | `Next::SleepMicroseconds(u64)` | a quantity carries its unit |
| `polled_wait::next`'s `now`, `last_frame`, `tick` | `now_ticks`, `last_frame_ticks`, `ticks_per_scheduler_tick` | each is in counter ticks; the unit is spelled nowhere in the signature |
| `fixture`'s items | moved to `socket_fixture`; `IN_MSG`, `OUT_MSG`, `ECHO_PEER_IP`, `PACKAGE_PEER_IP` became `INBOUND_MESSAGE`, `OUTBOUND_MESSAGE`, `ECHO_PEER_ADDRESS`, `PACKAGE_PEER_ADDRESS` | **Not in the table.** The new crate's report gates refuse `msg` and `ip` as abbreviations and a new crate starts at zero findings, so the four stems were spelled out; the rest keep their names |

Names that would follow a rename elsewhere: `domain_name_resolution_contract` carries `NAME_OFFSET_BYTES`
and `ADDRESSES_OFFSET_BYTES` in the same style, and no other crate defines `REP_OK` or `REP_ERR`; only notes and design files mention them.

## What still blocks the move

| item | owner |
|---|---|
| Add `tcp` and `udp` to the template's accepted words (another repository; not done here). | maintainer |
| The `std` farm must read the pinned checkout's source; until then the move breaks the platform layer's build. | maintainer |
| Convert the eight path dependencies to `git` and `rev` with `version = "0.1.0"`, and read every file the dry run lists. | move lane |
| Run the stranger test and the template's workflows in the new repository. | move lane |

## Recommendation

Not ready to clear. The crate builds, proves and stands alone, its names carry the architect's ratification, and its scope has a
provisional answer: `fixture`, `ephemeral_port` and `polled_wait` live in crates that stay in nife. It still
waits on the clearance itself.

## Decisions for the architect

1. Ratify the names above. Answered: ratified as built (2026-11-20 UTC).
2. Clear the move, with no tag and no publication.

The scope decisions (where `fixture`, `ephemeral_port` and `polled_wait` live) are built on the
maintainer's provisional ruling of 2026-11-19 UTC, so they no longer wait for the architect.
