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
| identity values | `INTERFACE_ID` 19 and `VERSION` 1, both provisional; no magic. Row 19 of [milestone 956 (the linked interfaces carry checkable versions)](../../design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md)'s id table names this crate, so the value agrees. No operation carries `VERSION` on the wire: the bind-time check belongs to [the message contracts carry bind-time version checks](../../design/roadmap/1000-message-contracts-carry-bind-time-version-checks.md) |
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
| source copied by path | `xtask/src/farm/generate.rs` writes `src/lib.rs` verbatim into `patches/std-nife` as `netproto.rs`, and `xtask/src/farm/stamp.rs` watches it; the runbook's BUGS has this |
| prose and tables | `helpers/trust_boundary_crates.tsv`, `helpers/interface_stability.py`, `helpers/lint_externalized_crates.py`, `helpers/template_report_baseline.txt`, `packages/contracts.package.toml`, `crates/externalization-list.txt`, the doc comments of `domain_name_resolution_contract` |
| users of `fixture` | `components/src/socket_test_client.rs`, `fixtures`, `jig`, `std_exerciser`, `system_tests` (network and riscv virtio tests); `xtask/src/inbound.rs` spells the same values for the host side |
| users of `ephemeral` and `polled_wait` | the network stack alone: `components/src/socket_server.rs`, `components/src/socket_table.rs`, `components/src/nic_wait.rs`. No crate in nife holds the stack's host-testable logic, so these two modules live here for their host tests |

No test reads a `../../` file, no `include_str!` leaves the crate (the one there reads its own
README), and nife's fuzz workspace has no target for it.

## What was run

This lane ran each check in its own worktree, on this branch.

| check | result |
|---|---|
| `script/crate-standalone socket_contract` | one failure, `names`: 55 items not ratified. Manifest, files, citations, and the build on 1.88 for the host and the three bare-metal targets pass; Kani 3 of 3 prove; all four falsification patches turn their check red |
| `script/crate-move --dry-run` | builds and tests at 1.88, and every patch applies with no offset and no fuzz. Three findings, all known template bugs (the scaffold's mentions of nife, its architect line and its path into nife), and two crate lines: the `fixture` message bytes `nife-in!` and `nife-out!` |
| `cargo test -p socket_contract` | 20 unit tests and 5 doctests pass; one doctest is the README's example |
| `cargo clippy --all-targets`, `cargo doc` with warnings denied | no warning |
| the template's naming checks over `src/lib.rs` | `spelled_out`, `unit_symbols`, `question_shape`, `british_spelling` and `contract_names`: 169 names, 0 refused. `register_names` refuses `OFF_LEN` (a length carries its unit). `abbreviations`, report only: 20 components |
| `script/names socket_contract` | the crate name ratified; three items provisional; one ratified; the rest have no block |
| `script/lint` | passes |

Not run, and so not claimed: the stranger test, `cargo-semver-checks` (a first release has no
baseline) and the template's workflows, which run in the repository.

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
| operations | `OPERATION_ATTACH_PAGE_FRAME`, `OPERATION_OPEN_UDP`, `OPERATION_OPEN_TCP`, `OPERATION_SENDTO`, `OPERATION_RECEIVE`, `OPERATION_CONNECT`, `OPERATION_SEND`, `OPERATION_CLOSE`, `OPERATION_LISTEN`, `OPERATION_ACCEPT`, `OPERATION_BIND_UDP` (1 to 11) |
| badges | `SOCKET_BADGE` (bit 63), `names_a_socket(badge)` |
| replies | `REP_OK`, `REP_ERR` (bit 32), `CONNECT_ESTABLISHED`, `CONNECT_REFUSED`, `LISTEN_GRANTED`, `LISTEN_DENIED`, `LISTEN_IN_USE` |
| port grants | `NO_LISTEN_GRANT`, `listen_grant(lo, hi)`, `grant_allows(grant, port)`, `udp_bind_grant(lo, hi)`, `udp_grant_allows(grant, port)` |
| shared page | `OFF_DST_IP`, `OFF_DST_PORT`, `OFF_LEN`, `OFF_PAYLOAD`, `DATA_MAXIMUM` |
| `stack_slots` | `MINT` (7), `OWN_SPACE` (8) |
| `lease` | `NO_NAMESERVER`, `ipv4_word(octets)`, `word_ipv4(word)` |
| `ephemeral` | `LO`, `HI`, `SPAN`, `PER_SECOND`, `Ports`, `Ports::new`, `Ports::next(milliseconds, held)` |
| `polled_wait` | `Next::{Look, Sleep(u64)}`, `next(now, last_frame, tick, stack_due_microseconds, poll_microseconds)` |
| `fixture` | `LISTEN_PORT`, `DENIED_PORT`, `IN_MSG`, `OUT_MSG`, `ROUNDS`, `ECHO_PEER_IP`, `ECHO_PEER_PORT`, `PACKAGE_PEER_IP`, `PACKAGE_PEER_PORT`, `PACKAGE_PEER_HOST` |

No nife type appears in a signature. What a stranger may find surprising:

- `CONNECT_ESTABLISHED` and `REP_OK` share the value 0, and so do `LISTEN_GRANTED` and
  `NO_LISTEN_GRANT`: a client reads each vocabulary only in reply to its own operation.
- The `LISTEN_` replies also answer `OPERATION_BIND_UDP`, and `NO_LISTEN_GRANT` is the empty word
  for both halves of the grant.
- The documentation refers to a `PageFrame` that the crate does not define; it means the shared
  page.
- Under `cargo kani`, the compile-time `assert!` in `ephemeral` draws a future-incompatibility
  warning from Kani's own `assert` macro. Plain `cargo` builds do not warn.

## Names

The architect ratified the crate name, and `DATA_MAXIMUM` before the rule that a limit opens
`MAXIMUM_` and ends with its unit; a release review reopens that one. `VERSION`, `INTERFACE_ID` and
`polled_wait::Next` are provisional, and the 52 other items, the five modules and both `Next`
variants have no block. A doc comment saying "name provisional" is not a block, and the move does
not strip it.

This lane renames nothing. Every proposal below is provisional, for the architect to rule on. The
crate has not left, so each is a two-way door.

| item now | proposed | rule |
|---|---|---|
| `VERSION`, `INTERFACE_ID` | keep | the names `block_roster_contract` ratified |
| `OPERATION_SENDTO` | `OPERATION_SEND_TO`, alias `sendto` | two words; POSIX's `sendto` kept as the alias |
| `OPERATION_ATTACH_PAGE_FRAME` | `OPERATION_ATTACH_SHARED_PAGE` | `frame` is a word to avoid, and here it also means a network frame (`polled_wait`) |
| the other ten `OPERATION_` constants | keep | `udp` and `tcp` pass the acronym test, as `pci` does; add both to the template's accepted words |
| `SOCKET_BADGE` | `SOCKET_BADGE_BIT` | it is the bit; a socket's badge is that bit with others set |
| `names_a_socket` | keep | a third-person verb phrase |
| `REP_OK`, `REP_ERR` | `REPLY_OK`, `REPLY_ERROR` | `rep` and `err` abbreviate; `REPLY_` is the tree's prefix elsewhere |
| `CONNECT_ESTABLISHED`, `CONNECT_REFUSED` | keep | |
| `LISTEN_GRANTED`, `LISTEN_DENIED`, `LISTEN_IN_USE` | `PORT_CLAIM_GRANTED`, `PORT_CLAIM_DENIED`, `PORT_CLAIM_IN_USE` | they answer a bind as well as a listen; the documentation already calls them the port-claim vocabulary |
| `NO_LISTEN_GRANT` | `NO_PORT_GRANT` | the empty word for both halves |
| `listen_grant`, `udp_bind_grant` | keep; parameters `lo`, `hi` become `start`, `end` | the standard library's words for an inclusive range |
| `grant_allows`, `udp_grant_allows` | `listen_grant_allows`, `udp_bind_grant_allows` | each names the grant its packer makes |
| `OFF_DST_IP`, `OFF_DST_PORT`, `OFF_LEN`, `OFF_PAYLOAD` | `DESTINATION_IP_ADDRESS_OFFSET`, `DESTINATION_PORT_OFFSET`, `PAYLOAD_LENGTH_OFFSET`, `PAYLOAD_OFFSET` | `off`, `dst` and `len` abbreviate; `_OFFSET` as in `CPU_WORD_OFFSET`; an IP address takes `ip` before `_ADDRESS` |
| `DATA_MAXIMUM` | `MAXIMUM_PAYLOAD_BYTES` | a limit opens `MAXIMUM_` and ends with its unit |
| `stack_slots::MINT`, `OWN_SPACE` | `MINTING_ENDPOINT_SLOT`, `OWN_ADDRESS_SPACE_SLOT` | `mint` is a verb and `space` half a term; `_SLOT` as in `ACTIVATION_SLOT` |
| `lease::NO_NAMESERVER` | `NO_NAME_SERVER` | two words |
| `lease::ipv4_word`, `word_ipv4` | `ipv4_to_word`, `word_to_ipv4` | the pair reads as each other's inverse |
| `ephemeral` | `ephemeral_port` | a module takes a noun |
| `LO`, `HI`, `SPAN`, `PER_SECOND` | `LOWEST_PORT`, `HIGHEST_PORT`, `PORT_COUNT`, `PORTS_PER_SECOND` | abbreviations, and a rate names what it counts |
| `Ports`, `Ports::new`, `Ports::next` | `Cursor`; keep `new` and `next` | the documentation calls it the cursor; `next` advances it and returns the next value, as an iterator's does |
| `polled_wait`, `Next::Look` | keep | |
| `Next::Sleep(u64)` | `Next::SleepMicroseconds(u64)` | a quantity carries its unit |
| `polled_wait::next`'s `now`, `last_frame`, `tick` | `now_ticks`, `last_frame_ticks`, `ticks_per_scheduler_tick` | each is in counter ticks; the unit is spelled nowhere in the signature |
| `fixture`'s items | not proposed | decision 2 below would take them out of the crate |

Names that would follow a rename elsewhere: `domain_name_resolution_contract` carries `OFF_NAME`
and `OFF_ADDRESSES` in the same style, and several contracts use `REP_OK` and `REP_ERR`.

## What still blocks the move

| item | owner |
|---|---|
| Ratify `INTERFACE_ID` 19 and `VERSION` 1, which milestone 1017 asks for before a contract leaves. | architect |
| Rule where `fixture`, `ephemeral` and `polled_wait` live. | architect |
| Ratify every remaining name, with the renames ruled. | architect |
| The `std` farm must read the pinned checkout's source; until then the move breaks the platform layer's build. | maintainer |
| Convert the eight path dependencies to `git` and `rev` with `version = "0.1.0"`, and read every file the dry run lists. | move lane |
| Run the stranger test and the template's workflows in the new repository. | move lane |

## Recommendation

Not ready to clear. The crate builds, proves and stands alone. It waits on the identity values, on
the scope of the crate, and on names for 55 items.

## Decisions for the architect

1. Ratify `INTERFACE_ID` 19 and `VERSION` 1 unchanged.
2. Move `fixture` into a crate that stays in nife before this crate leaves.
3. Move `ephemeral` and `polled_wait` into a host-testable crate of the network stack's that stays
   in nife.
4. Ratify the names above, with the proposed renames.
5. Clear the move, with no tag and no publication.
