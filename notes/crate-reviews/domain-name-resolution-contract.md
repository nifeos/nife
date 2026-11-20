# Review package: `domain_name_resolution_contract`

*Name: provisional, minted by lane 1086-review-package. The `domain_name_resolution_contract`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the packages together
outgrow the page.*

The architect reads this to ratify the crate's names and to give the go to create its repository,
as part of step 6 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md).
It describes the crate as `main` leaves it, after the four rulings of
[milestone 1086 (`domain_name_resolution_contract`: what an outside maintainer would fix before it leaves)](../../design/roadmap/1086-domain-name-resolution-contract-outside-review.md)
merged. This package ratifies nothing and clears nothing. The architect ratifies the names below,
and the go creates the repository with `publish = false`, no tag and no publish.

## In one paragraph

The contract between a confined name resolver, the program that spawns it, and the clients that ask
it for IPv4 addresses. A client attaches one writable page, writes a host name into it and `CALL`s
`OPERATION_RESOLVE`. The resolver judges the name against a grant table before it sends anything,
and replies with an `Outcome` word and the answer's time to live. The spawner grants each client
badge a zone in eight-byte control messages through the unbadged capability, and the resolver
answers each control `CALL` with the same packed word, so a refused grant is a failed spawn. The
spawner starts the resolver with three words: the name server, its port and the transport (UDP or
TCP). The crate also holds the grant table itself, because what a grant means is half of the
contract and a host test can reach it. It is `no_std`, has no `unsafe`, and depends on one crate,
`domain_name_system`, whose `Name` a zone is. Two Kani harnesses prove the grant table, each with a
falsification patch. One file, `src/wire.rs`, names nothing outside itself, because the standard
library's platform layer copies it verbatim. One module, `fixture`, is nife's boot-test data and
leaves the crate before the move (item 2 of milestone 1086).

## Identity

| item | value |
|---|---|
| crate | `domain_name_resolution_contract`, ratified 2026-10-18 ("to pair with the program `domain_name_resolver`") |
| identity values | frozen 2026-11-14: `INTERFACE_ID` 11 and `VERSION` 1. Version 1 includes the control reply word. The 2026-11-19 ruling adds `network_step::UNKNOWN_TRANSPORT` (5) inside version 1, with no bump |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 |
| license | `MIT OR Apache-2.0` in the manifest; the two license texts are not yet at the crate root (the move adds them) |
| `publish` | `false`; milestone 1017 keeps the moved contract crates unpublished under §305 (git dependencies on `nifeos/*` only, pinned to a landed commit) until their own release milestone |
| dependencies | `domain_name_system` 0.1.0, a path dependency until that crate leaves (step 5) |
| size | 1,649 lines in `src/`: about 900 of library (`wire` 213, `grants` 327, `control` 161, `endowment` 46, `messages` 60, `lib` 97), 514 of tests, 134 of Kani harnesses, 97 of `fixture`; a 144-line README and two falsification patches |

## Who in nife depends on it

Nine manifests name it by path: the root workspace, `components`, `crates/approval_file`,
`crates/capability_manifest_note`, `crates/installed_programs`, `crates/system_initializer`,
`fixtures`, `fuzz`, `std_exerciser`, and `system_tests` (development only).
Source files that use it: `components/src/domain_name_resolver.rs` (the resolver program),
`crates/system_initializer` (`children.rs`, `image_request.rs`, `layout.rs`: the spawner's half),
`crates/installed_programs` (`approval.rs`, `generation.rs`, `grants.rs`: which zones a program
may ask for), `crates/approval_file`, `crates/capability_manifest_note`,
`fixtures/src/name_resolver_test_client.rs`, `system_tests/src/user/name_resolver_tests.rs`,
`std_exerciser/src/bin/std_resolve.rs` and `fuzz/fuzz_targets/capability_manifest_note_decode.rs`.
`crates/package_index` cites it in a doc comment only.

Two more readers reach it outside the dependency lines:

- The standard library. `cargo xtask std-src` copies `src/wire.rs` verbatim to the platform layer
  (`xtask/src/farm/copied.rs` names the package; the overlay's `sys/pal/nife/mod.rs` says so). The
  `wire` module therefore names nothing outside itself, and a compile-time assertion in `lib.rs`
  checks its address bound against `domain_name_system`'s.
- The tooling. `script/verify` carries the crate's two Kani harnesses with the `-Z stubbing` flag
  and splits the heavier one from its shard; `helpers/verify-stub-selftest.sh` and
  `script/falsifications` name it. `packages/network.package.toml` lists it as an interface,
  `crates/externalization-list.txt` and `helpers/trust_boundary_crates.tsv` list it,
  `.cargo/mutants-baseline.txt` holds its census row, and `helpers/template_report_baseline.txt`
  holds four report-only rows.

What references back into nife from the crate: `script/crate-move --dry-run` finds three paths into
the nife tree, two milestone citations and twelve mentions of nife, almost all in `src/fixture.rs`.
They leave with the fixture (item 2) or fall to the move lane's rewrite (below).

## What was run

All runs are on this branch at the commit below `main`'s tip of 2026-11-19, after the four rulings
merged (pull request 3785). They are one run each.

| check | result |
|---|---|
| `cargo test -p domain_name_resolution_contract` | 30 unit tests and 2 doctests pass |
| `cargo clippy -p domain_name_resolution_contract --all-targets -- -D warnings` | no warning |
| `cargo doc` with `RUSTDOCFLAGS="-D warnings"` | no warning |
| bare-metal builds, `--lib`, for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat` and `riscv64imac-unknown-none-elf` | all three build |
| MSRV copy outside the tree (`src/` and the manifest of this crate and of `domain_name_system`, in a scratch directory, `cargo +1.88.0 test`) | 30 unit tests and 2 doctests pass on 1.88.0 |
| `cargo +nightly miri test -p domain_name_resolution_contract` | 30 unit tests and 2 doctests pass under Miri; the crate has no `unsafe`, so this checks the safe code's arithmetic and indexing at run time |
| Kani, `cargo kani -Z stubbing -p domain_name_resolution_contract` | both harnesses verify: `the_one_message_zone_parses_or_is_refused` in 300 s and `the_grant_table_grants_only_what_it_said` in 1231 s, 25 minutes 40 seconds of wall time in all |
| the two falsification patches, each applied to the tree and its own harness run | each patch makes its harness fail, as it should: `the_grant_table_grants_only_what_it_said` fails in 13 minutes and `the_one_message_zone_parses_or_is_refused` in 5 minutes; the tree was restored after each |
| `script/crate-standalone domain_name_resolution_contract` | exits 1 with 4 failures, all the move's work: `names` (72 names not ratified), `manifest` (the path dependency leaves the crate), and `build` and `proofs`, both skipped because the dependency is outside the copy. Its Kani step therefore did not run, which is why the two rows above ran by hand |
| `script/crate-move --dry-run --into <scratch directory> domain_name_resolution_contract` | stages at template `v8` (`a69ab4829b02`); exits 1 with 3 findings for the move lane. Passing: the copy of 18 tracked files, the mutation cron, the scaffold of 27 files, the rewrite (`Name:` blocks removed from 8 files, 2 falsification patches regenerated for `-p1`, both apply with no offset and no fuzz), `standalone`, history phrases, file length, comment blocks, function length, spelled-out words, unit symbols, orphan modules, markdown links, falsification records and mentions of nife (none). Failing: the build (the dependency is outside the staged copy), prose caps and passive voice in `README.md` |
| `python3 helpers/lint_externalized_crates.py` | no failure; the crate has 1 hit against its waiver ceiling of 1, the ratified crate-name `Name:` block, which the move strips |
| `python3 helpers/name_release.py --draft domain_name_resolution_contract` | 78 names: 6 ratified, 31 provisional, 41 with no block |
| `script/names domain_name_resolution_contract` | the crate name is ratified |
| `script/lint --quick` | passes on this branch |

The standalone failures are all expected before the move. None is a defect in the contract.

Not run, and so not claimed:

- Mutation. The census row in `.cargo/mutants-baseline.txt` (112 caught, 6 missed, 0 timeouts, 17
  unviable) predates the renames and the four rulings. A new census runs between 08:00 and 13:00
  UTC, and this lane ran at 21:52 UTC. The move carries the six missed mutants into
  `.github/mutants-exempts` and the first weekly run in the new repository settles them.
- The stranger test (`script/stranger-test`), which needs the staged repository.
- `cargo-semver-checks`, because a first release has no baseline.
- The template's workflows, which run in the repository.
- A fuzz target of the crate alone, which item 10 of milestone 1086 puts in the new repository.

## Public API

| kind | items |
|---|---|
| identity | `INTERFACE_ID` (11), `VERSION` (1) |
| limits and layout | `MAXIMUM_NAME_TEXT_BYTES` (253), `MAXIMUM_NAME_FIELD_BYTES` (254), `MAXIMUM_IPV4_ADDRESSES` (8), `NAME_OFFSET_BYTES` (0), `IPV4_ADDRESSES_OFFSET_BYTES` (0x100), `MAXIMUM_GRANTS` (8) |
| the client's words | `OPERATION_ATTACH_SHARED_PAGE` (1), `OPERATION_RESOLVE` (2), `request(operation)`, `operation(first_word)` |
| the reply | `Outcome { status, detail, count }`, `Outcome::of`, `word`, `from_word`; `status` (`OK` 0 to `BAD_REQUEST` 10); `network_step` (`NO_SOCKET_PAGE` 1, `OPEN` 2, `SEND` 3, `CONNECT` 4, `UNKNOWN_TRANSPORT` 5) |
| the spawner's words | `OPERATION_GRANT` (0x10), `OPERATION_FORGET` (0x11), `control_word`, `control_fields`, `grant_messages`, `forget_message`, `reject_code`, `ROOT_ZONE`, `is_canonical_zone` |
| the control reply | `refusal` (`UNKNOWN_OPERATION` 1 to `NO_SUCH_GRANT` 7), `control_answer`, `Control::reply`, `Refusal::code`, `Refusal::from_code` |
| the grant table | `Grants` (`new`, `control`, `check`, `granted`), `Control`, `Refusal`, `Denied` |
| `endowment` | `TRANSPORT_UDP` (0), `TRANSPORT_TCP` (1), `is_known_transport`, `server`, `server_octets`, `port_and_transport`, `port_and_transport_fields`, `OWN_SPACE_SLOT` (4) |
| `fixture` (leaves, item 2) | `ZONE`, `SERVER`, `WITH_NETWORK`, `ECHO_OK`, `ECHO_SKIPPED`, `ECHO_NO_ADDRESS`, `ECHO_FAILED`, `Case`, `CASES` |

No nife type, capability or handoff appears in a public signature. The one nife-specific number is
`OWN_SPACE_SLOT`, the capability slot the spawner and the test harness place the resolver's own
address space in.

What a stranger may find surprising:

- Statuses, steps, refusals and operations are bare constants, not enums. They are words on a page
  and in a message, so a reader keeps a value a newer peer added. `Refusal` and `Denied` are enums
  because only the grant table produces them.
- `Outcome::from_word` returns `None` for a word the resolver cannot send: a failed `CALL`'s
  negative error, an undefined status, or a count above `MAXIMUM_IPV4_ADDRESSES`.
- A refused grant leaves the badge with its window and no zone, and a re-grant withdraws the old
  zone at its first piece, so a half-sent zone is never in force.
- `endowment::is_known_transport` and `network_step::UNKNOWN_TRANSPORT` carry the ruling that the
  resolver refuses a start with any transport other than UDP or TCP, and answers every resolve with
  `NETWORK` and step 5. The resolver program is the other half; this crate defines the words.

## Proofs

Two Kani harnesses in `src/verification.rs`, each with a falsification patch under
`falsifications/`. `the_grant_table_grants_only_what_it_said` proves that any short run of control
messages never panics and that a badge is in force only after a message said `Granted`, in a window
that never moves. `the_one_message_zone_parses_or_is_refused` proves the bookkeeping of a one-message
grant, with `core::str::from_utf8` and `Name::from_dotted` stubbed (the parse is proved in the name
crate). The stubs need `-Z stubbing` on the Kani command line. A repository that runs a plain
`cargo kani` sees a compile error, so the move lane sets the flag in the template's proof step, as
`script/verify` does here. Unit tests cover the wire words, the control words, the table and the
refusals.

## Names

Milestone 1086's decision 1 ruled on 2026-11-19 (UTC), option A: the provisional names stand as
built. Its table renames nine of them. The architect ratifies the rest below with the ratification
pull request, whose body carries `python3 helpers/name_release.py --draft
domain_name_resolution_contract`. Each row has a proposal. "Ratify" means ratify as built.

The 30 `Name:` blocks that still read provisional:

| item | kind | what the block says | proposal |
|---|---|---|---|
| `MAXIMUM_NAME_TEXT_BYTES` | constant | the limit opens `MAXIMUM_` and ends with its unit | ratify (ruling 1) |
| `MAXIMUM_NAME_FIELD_BYTES` | constant | minted 2026-11-16 for the one bound on a resolve's name | ratify; it pairs with the text bound by the trailing dot |
| `MAXIMUM_IPV4_ADDRESSES` | constant | says which kind of address, as `domain_name_system` does | ratify (ruling 1) |
| `IPV4_ADDRESSES_OFFSET_BYTES` | constant | the family-wide `<FIELD>_OFFSET_BYTES` shape | ratify (ruling 1) |
| `MAXIMUM_GRANTS` | constant | a count names the things, the limit word first | ratify (ruling 1) |
| `network_step` | module | minted 2026-10-13 | ratify; says what it is, the step of the socket contract that refused |
| `ROOT_ZONE` | constant | minted 2026-11-16 | ratify |
| `is_canonical_zone` | function | minted 2026-10-19 | ratify; "canonical" is the one spelling a zone may take |
| `OPERATION_GRANT`, `OPERATION_FORGET` | constants | origin not recorded; `system_log_contract` has its own `OPERATION_FORGET` | ratify; each contract has its own operation space, and the full path disambiguates |
| `control_word`, `control_fields` | functions | origin not recorded | ratify |
| `refusal` | module | the reasons a control message was refused | ratify; see open question 1 |
| `UNKNOWN_OPERATION`, `BADGE_ZERO`, `OUT_OF_ORDER`, `TOO_LONG`, `BAD_ZONE`, `FULL`, `NO_SUCH_GRANT` | constants | one per `Refusal` variant | ratify; each matches its `Refusal` variant by name |
| `Control::reply`, `Refusal::code`, `Refusal::from_code` | methods | provisional | ratify; `from_code` and `code` match the standard library's habit of naming a conversion by its source |
| `control_answer` | function | what the resolver answered a control `CALL` | ratify |
| `forget_message` | function | minted 2026-10-12 | ratify; pairs with `grant_messages` |
| `is_known_transport` | function | minted 2026-11-19 | ratify (ruling 4); pairs with `state::is_known` of the clock contract |
| `OWN_SPACE_SLOT` | constant | after `socket_contract::stack_slots::OWN_SPACE` | ratify for now; see open question 2 |
| the modules `control`, `grants`, `messages`, `verification` | private modules | minted by earlier lanes | ratify; none is public, and the move strips their blocks |

Already ratified, with the block current: the crate name, `INTERFACE_ID`, `VERSION` and
`NAME_OFFSET_BYTES`. Ratified by ruling and still reading provisional or unblocked in the source:
`OPERATION_ATTACH_SHARED_PAGE` and `network_step::UNKNOWN_TRANSPORT` (their blocks already say
ratified 2026-11-19), and the parameter and field names the table of ruling 1 renames
(`first_word`, `second_word`, `text_bytes_received`, `text_bytes_total`, `operation`).

The 50 items with no `Name:` block need one before the crate leaves. `script/crate-standalone`
refuses a public item with no block, and a ratification pull request writes them. They are the
`wire` items that no earlier lane touched (`status`, `Outcome`, `OPERATION_RESOLVE`, `request`,
`operation`), `endowment`'s transports and start-word functions, `grants`' types and methods
(`Grants`, `Control`, `Refusal`, `Denied`, `check`, `granted`), `grant_messages`, `reject_code`, and
the 10 items of `fixture`. The proposal for the first three groups is to ratify as built. The
`fixture` names are not for ratification: the module leaves the crate.

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair. The
move adds `LICENSE-MIT` and `LICENSE-APACHE` at the crate root, which `script/crate-standalone`
requires. The RFC numbers in the documentation (1035, 7766) cite public standards and copy no text.

## What the move lane does

The move lane works from `script/crate-move --dry-run` and runs after `domain_name_system` has left
(step 5), because the crate depends on it.

| item | owner |
|---|---|
| Move `fixture` into a crate that stays in nife (item 2 of milestone 1086). It clears the three paths into the nife tree, the twelve mentions of nife and 14 hits of the externalization waiver. The `socket_contract` package puts the same question and shares the answer. | a lane before the move |
| Write a `Name:` block on each public item that has none and rewrite the 30 provisional blocks to ratified, then let the move strip them. | ratification lane |
| Replace the path dependency on `domain_name_system` with a `git` dependency pinned to a landed commit, with `version = "0.1.0"`. | move lane |
| Add `LICENSE-MIT` and `LICENSE-APACHE`. | move lane |
| Clear the prose findings: prose caps, 13 passive-voice lines, and the acronyms `prose_acronyms` flags (RFC, TTL, NXDOMAIN, GB, OK). | move lane |
| Remove the milestone citations (`src/lib.rs`) the template refuses, and the mentions of nife the fixture leaves behind. | move lane |
| Set `-Z stubbing` where the template runs Kani, so the stubbed harness compiles. | move lane |
| Carry the six missed mutants into `.github/mutants-exempts` after the first weekly census. | move lane |
| In nife, drop the workspace member, convert the nine dependency lines to `git` and `rev`, delete the census row in `.cargo/mutants-baseline.txt`, and read the files the dry run lists (the resolver program, the spawner's three files, the installed-programs files, `xtask/src/farm/copied.rs` for the std copy, `script/falsifications`, `script/verify`, `helpers/lint_externalized_crates.py`, `helpers/template_report_baseline.txt`, `helpers/trust_boundary_crates.tsv`, `helpers/verify-stub-selftest.sh` and `packages/network.package.toml`). | the nife-side pull request, citing `nifeos/domain_name_resolution_contract@<sha>` |
| Run the stranger test and the first weekly mutation census in the new repository. | move lane |

`xtask std-src` must read the pinned checkout once the crate leaves, as it does for the other
contract crates.

## Dependents and order

It moves after `domain_name_system` (step 5), in step 6 of milestone 1017 beside
`machine_statistics_contract`, `grant_plan` and `capability_manifest_note`. `capability_manifest_note`
and `installed_programs` use it, so the dependents' pins follow the tag. Nothing in nife blocks
the move except the items above, and the move does not wait for the socket contract (ruling 6).

## Open questions

1. Whether `refusal`, `reject_code` and `status::REFUSED` read as three words for three things.
   `refusal` is the spawner-facing control reasons, `reject_code` encodes `domain_name_system`'s
   `Reject`, and `REFUSED` is a status the resolver sends. Recommendation: keep all three. Each says
   what it is, and the documentation names the difference. The maintainer can decide this.
2. Whether `OWN_SPACE_SLOT` belongs in a contract crate. It is a capability slot number that the
   spawner and the test harness both place, and the crate cites the socket contract's equivalent.
   Recommendation: keep it for now; a slot layout shared by two programs is code both use, not
   prose (§311 (a contract is code, not prose)). The maintainer can decide this.

## Recommendation

Ready for review. The architect ratifies the names above, and the go creates
`nifeos/domain_name_resolution_contract` with `publish = false`, no tag and no publish. The move
itself waits for `domain_name_system` to leave and for the fixture to leave the crate.

Ready for review.
