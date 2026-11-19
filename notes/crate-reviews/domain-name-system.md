# Review package: `domain_name_system`

*Name: provisional, minted by lane 1087-review-package. The `domain_name_system` package of
[crate-reviews.md](../crate-reviews.md), in its own file because the packages together outgrow the
page.*

The architect reads this to clear the crate for its move to its own repository, as part of step 5 of
[milestone 1017 (the contract crates leave in this order)](../../design/roadmap/1017-the-contract-crates-leave-in-this-order.md),
the `process-tools` leaves. It describes the crate as `main` plus this branch leaves it. The review
that raised the work is [milestone 1087 (what an outside maintainer would fix before it
leaves)](../../design/roadmap/1087-domain-name-system-outside-review.md); its Decision 4 rules that
this package comes first and that the move does not wait for the resolver contract.

## In one paragraph

The Domain Name System (DNS) wire format a stub resolver needs, as pure computation. It writes the
bytes of a query for one name's IPv4 addresses, over the User Datagram Protocol (UDP) or the
Transmission Control Protocol (TCP) with its two-byte length prefix. It runs the checks a reply must
pass before an address in it is believed: a response, the right transaction id, the question echoed
back, the name error and server error codes, and a bounded canonical name chain. It reads only
address records owned by the end of that chain, and nothing from the authority and additional
sections. `Reject` lists the nine refusals in the order the checks run. The crate is `no_std`, has no
dependency, forbids `unsafe` and has no socket, timer or policy. Four Kani harnesses prove the
parser's totality, and a fuzz target feeds `Query::accept` arbitrary bytes. Nothing in it points
back into nife except text this package lists. Every public name stands as built (ratified
2026-11-16 UTC).

## Identity

| item | value |
|---|---|
| crate | `domain_name_system`, ratified 2026-10-18 (UTC) (the standard's full name, spelled out; `dns` and `name_resolver` refused) |
| identity values | none: the crate defines no run-time format of its own. RFC 1035 owns the bytes, so `helpers/interface_declarations_check.py` lists the crate as a format nife does not own |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024, 1.88 (the floor that `as_chunks_mut` sets) |
| license | `MIT OR Apache-2.0`, both texts at the crate root, written in the manifest with `description`, `readme`, `keywords` and `categories` |
| `publish` | `false`; milestone 1017 keeps the moved contract crates unpublished under §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit) until their own release milestone, and the publish hold stands |
| dependencies | none |
| size | `src/lib.rs` 1,086 lines, `src/proofs.rs` 165 lines (the Kani harnesses, compiled only under `cfg(kani)`), `tests/replies.rs` 781 lines, three captured replies under `tests/fixtures/` |

## Who in nife depends on it

Each by a path dependency with `version = "0.1.0"`, except the fuzz workspace, which uses a bare
path:

- `components`: the resolver program (`components/src/domain_name_resolver.rs`) uses `Query`,
  `Answer`, `Reject` and `TcpReply`, and `components/src/socket_test_client.rs` uses it too.
- `crates/domain_name_resolution_contract`: its zone type is `domain_name_system::Name`, and it
  asserts that its address limit equals `MAXIMUM_IPV4_ADDRESSES`. This is the crate that pins this
  one as a `git` dependency, so the order of the moves is fixed: this crate first.
- `system_tests` (development only).
- `fuzz`: the target `domain_name_system_reply` links it.

Four other places name the crate without a dependency line. `packages/network.package.toml` lists
it. `xtask/src/farm/copied.rs` mentions it in a comment, because the std source copies only the
resolver contract's `wire.rs`. `helpers/vendored_name_gates.py` holds one test name that carries the
word `cap`, and `.cargo/mutants-baseline.txt` holds the census row (caught 200, missed 3, timeout 4,
unviable 21). `crates/externalization-list.txt` lists the crate, and
`helpers/template_report_baseline.txt` holds four rows for it: abbreviations 29, prose acronyms 13,
README acronyms 4 and register names 1.

What the crate references back in nife: nothing at run time or in a test. `script/crate-move`
reports "mentions nife: none" on the staged copy. The fuzz target and its notes section live in
nife's `fuzz/` workspace and move with the crate.

## What was run

Each check ran on this branch, in a worktree of its own, on 2026-11-19 (UTC) with the toolchain the
check names.

| check | result |
|---|---|
| `cargo test -p domain_name_system` | 43 integration tests and 4 doctests pass; the library unit-test binary holds none |
| `cargo clippy -p domain_name_system --all-targets -- -D warnings` | no warning |
| `cargo doc` with `RUSTDOCFLAGS=-D warnings` | no warning |
| `cargo build --lib` for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat` and `riscv64imac-unknown-none-elf` | all three build |
| the MSRV copy: the crate copied outside the tree, `cargo +1.88 test` and the three target builds | 43 tests and 4 doctests pass, and all three targets build. `cargo +1.85 build` refuses the crate for its `rust-version` of 1.88, as it should |
| `cargo kani` on that copy (Kani 0.68.0) | 4 harnesses verified, 0 failures. Times while other gates ran beside it: the dotted parse 389 seconds, the name decoder 97, the TCP reassembly 48, the header 2 |
| `script/crate-standalone domain_name_system` | clean: the build on 1.88 for the host and three targets, the four harnesses, and each of the four falsification patches fails its own harness |
| `script/crate-move domain_name_system --dry-run --into <scratch>` | stages at template `v8` (`a69ab4829b02`). It copies 15 files, adds 27 scaffold files and strips the `Name:` blocks from 2 files. It regenerates 4 patches, and they apply with no offset. It stages the fuzz target with its 3 corpus inputs. Host tests and the three targets build on 1.88, and the fuzz target builds and runs once over its corpus. The template's cheap checks pass except two findings, listed under "What the move needs" |
| `script/names domain_name_system` and `python3 helpers/name_release.py --draft domain_name_system` | the crate name and every public name ratified; none provisional and none without a block |
| the template's naming helpers over `src/` and `tests/` | `spelled_out`, `unit_symbols`, `british_spelling`, `question_shape` and `contract_names` refuse none of 427 names. `register_names` refuses 9 (see below). `abbreviations` (report only) finds 54 components in three files |
| `python3 helpers/lint_externalized_crates.py` | the crate has 0 hits, below its waiver ceiling of 12 |

Not run, and so not claimed:

- Miri. The active nightly has no `miri` component, and the crate has no `unsafe` to check.
- Mutation. The run is 08:00 to 13:00 UTC and this one ran after it. The recorded row stands:
  caught 200, missed 3, timeout 4, unviable 21, and the three survivors are triaged as equivalent
  mutants. The template's `mutation` workflow has no green full run for the crate on `main` yet,
  because the crate has no repository.
- The stranger test, the template's workflows, a fuzz campaign beyond one pass over the corpus,
  `cargo-semver-checks` (a first release has no baseline) and `cargo package` (the manifest
  sets no `repository` until the move does).

## Public API

| kind | items |
|---|---|
| constants | `PORT` (53), `MAXIMUM_UDP_MESSAGE_BYTES` (512), `MAXIMUM_TCP_MESSAGE_BYTES` (65,535), `MAXIMUM_NAME_BYTES` (255), `MAXIMUM_LABEL_BYTES` (63), `HEADER_BYTES` (12), `MAXIMUM_CHAIN` (8), `MAXIMUM_IPV4_ADDRESSES` (8), `CLASS_IN` |
| header flags | `FLAG_RESPONSE`, `FLAG_TRUNCATED`, `FLAG_RECURSION_DESIRED`, `OPERATION_CODE_MASK`, `RESPONSE_CODE_MASK`, `RESPONSE_CODE_NAME_ERROR` |
| `record_type` | `IPV4_ADDRESS` (1), `CANONICAL_NAME` (5) |
| errors | `Error` (non-exhaustive; nine variants from `Truncated` to `Trailing`), `Result`, `Reject` (nine refusals, `Malformed(Error)` and `ServerError(u8)` among them) |
| parsing | `Header::{parse, write, response_code}`, `Name::{root, as_bytes, from_dotted, is_within}`, `decode_name`, `Question`, `Record`, `Reader::{new, question, record}`, `canonical_name_target` |
| a query | `Query::{new, for_name, id, name, request, request_tcp, accept}`, `Answer::{ipv4_addresses, ttl_seconds, canonical}` |
| TCP framing | `TcpReply::{new, feed, message}` |

No nife type, capability or handoff appears in a public signature.

What a stranger may find surprising:

- `Name` is 256 bytes and `Copy`, so `Query::accept` copies one per record on each of its three
  passes. `Answer` is 296 bytes and `Result<Answer, Reject>` is 300. The README and the root doc say
  so, and the type sizes there match the measured ones of milestone 1087.
- `accept` believes as little as it can and drops the rest: an address for another name, anything
  outside the canonical name chain, and addresses beyond the eighth. A TTL with its high bit set
  reads as zero (RFC 2181 section 8), and the answer's TTL is the smallest on the chain.
- The crate reads no policy. Which names a client may ask about is the caller's, through
  `Name::is_within` and a resolver grant.
- It is IPv4 only, with no EDNS(0), no DNSSEC and no "0x20" case randomization. The BUGS section of
  the README and the root doc list each.

## Proofs

Four Kani harnesses in `src/proofs.rs` each carry a falsification patch in `falsifications/`.
`script/crate-standalone` applied each patch and saw its harness fail. The harnesses prove four
things:

- The name decoder is total and stays in bounds.
- A header round-trips on every value.
- `TcpReply` is total on any split of its input and refuses a reply longer than it announced.
- The dotted parse accepts exactly the valid names.

`Query::accept` is too large for the solver, so a fuzz target covers it. The target asserts three
properties. An accepted address is really in the answer section under the reported name. TCP framing
does not change the bytes. The decoder is total at every offset. No Miri run and no mutation run
backs these claims today (see above).

## Names

Ruled 2026-11-16 (UTC) in #3728: every public name stands as built. The names below came from the
2026-11-16 rulings of milestone 1087.

| item | kind | spelling |
|---|---|---|
| `MAXIMUM_LABEL_BYTES`, `HEADER_BYTES` | constants | the unit on every length; no alias of the old spellings |
| `MAXIMUM_IPV4_ADDRESSES`, `Answer::ipv4_addresses`, `Reject::NoIpv4Address` | constant, method, variant | the kind of address in the name; the resolver contract keeps code 9 |
| `Header::{question_count, answer_count, authority_count, additional_count}`, `response_code` | methods | spelled out, each with the specification's name as `#[doc(alias)]` (`QDCOUNT`, `RCODE` and so on) |
| `FLAG_RESPONSE`, `FLAG_TRUNCATED`, `FLAG_RECURSION_DESIRED`, `OPERATION_CODE_MASK`, `RESPONSE_CODE_MASK`, `RESPONSE_CODE_NAME_ERROR` | constants | spelled out; aliases `QR`, `TC`, `RD`, `OPCODE`, `RCODE` and `NXDOMAIN` |
| `Record::ttl_seconds`, `Answer::ttl_seconds` | field, method | `ttl` stays as a term a reader searches for, with the unit; aliases `TTL` and `time to live` |
| `record_type::CANONICAL_NAME`, `canonical_name_target`, `record_type::IPV4_ADDRESS`, `CLASS_IN` | constants, function | aliases `CNAME`, `A` and `Internet class`; `CLASS_IN` keeps its spelling as a term of art |
| `Question::{record_type, class}`, `Record::{record_type, class, data, data_offset}` | fields | spelled out |

The abbreviations `tcp`, `udp` and `ipv4` stay (ruled 2026-11-16 UTC), and so does `ttl` with its
unit. The `Name:` blocks in the crate source read ratified, and `script/crate-move` strips them from
the moved copy.

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source: it implements RFC 1035 and RFC 7766
from the specifications. The three fixture replies are captures of public DNS answers for
`example.com`, `www.github.com` and `nosuch.invalid`; they carry no license of their own and no
third-party text.

## What the move needs

The move lane runs `script/crate-move domain_name_system` after the clearance. These items are work
for it, or for one lane before it:

| item | owner |
|---|---|
| The README example calls `answer.ttl()`, and the method is `ttl_seconds()`. The README is not compiled by a doctest, so nothing caught it. Fix the example. The README also says "three Kani harnesses" and "about a minute"; there are four, and the four took about nine minutes beside other load. Say four and the measured time. | a lane before the move |
| The template's `passive_voice` check refuses the README: 3 passives in 595 words (lines 8 and 104, "is believed", "are kept" and "are dropped"). The dry run reports it as FAIL. | a lane before the move |
| `script/crate-move` looks for a `fuzz: false` line in the staged weekly caller to turn on, and template `v8`'s weekly caller has none, so the dry run reports a FAIL for the fuzz opt-in. The helper and the template disagree; one of them changes before the move can pass. The crate has no `unsafe`, so the opt-in is allowed. | move lane, with the template |
| `register_names` refuses 9 names, all on the word `address`: the binding `address` in `src/lib.rs` (line 977) and eight names in `tests/replies.rs` (lines 24, 135, 245, 257, 279, 342, 361 and 414). Milestone 1087 item 7 part b leaves them. Qualify each as IPv4, as Decision 3 did for the public names, or write `.github/accepted-words` entries. | a lane before the move |
| Carry the three equivalent mutants (`Name::is_within` `<` to `<=` and two `|` to `^` on disjoint bit ranges) into `.github/mutants-exempts` with their reasons, then dispatch `mutation.yml` once on `main` before the tag. | move lane |
| The `.github/accepted-words` file lists `tcp`, `udp` and `ipv4`; the template's `prose_acronyms` flags 13 acronyms in the README and `src/` for expansion or that list. | move lane |
| Nife side: drop the workspace member. Convert the three path dependencies (`components`, `domain_name_resolution_contract`, `system_tests`) to `git` with `rev` and `version = "0.1.0"`. Delete the fuzz dependency, the target `domain_name_system_reply` and its `script/fuzz` and `notes/fuzzing/targets.md` rows. Refresh `fuzz/Cargo.lock` and delete the mutation baseline row. The dry run lists the other files that name the crate. | move lane, in a nife pull request |
| Remove the crate's `lint_externalized_crates.py` waiver (0 hits against a ceiling of 12) and update its rows in `helpers/template_report_baseline.txt` and `helpers/vendored_name_gates.py`. | move lane |
| Run the stranger test, the template's workflows and the first weekly mutation run in the new repository. | move lane |

## Open questions

None.

## Recommendation

Ready for review.
