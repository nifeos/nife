---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1087. `domain_name_system`: what an outside maintainer would fix before it leaves

Raised by a read-only outside review of `crates/domain_name_system`, read the way a maintainer
meeting it on crates.io would read it. The review changed nothing; this block lists what the next
lanes change, in the order the move needs them.

The accepted baseline is the review package, [domain-name-system.md](../../notes/crate-reviews/domain-name-system.md)
in `notes/crate-reviews/`. Four things are settled.
The crate name is ratified, with `dns` and `name_resolver` refused. Pull request #2797 ratified
five constants (`MAXIMUM_UDP_MESSAGE_BYTES`, `MAXIMUM_NAME_BYTES`, `MAXIMUM_LABEL_LEN`,
`MAXIMUM_CHAIN`, `MAXIMUM_ADDRESSES`). Section 248 (the resolver is its own confined program) and
§252 (one zone per client badge) shape the callers and not this crate. Milestone 1017 (the contract
crates leave in this order) places this crate in step 5 with the `process-tools` leaves. This block reopens
none of those except where a later rule contradicts a ratified name, and says so where it does
(items 6 and 7).

Reuse: the template `nifeos/crate-workflows` and its scaffold, `script/crate-standalone`,
`script/crate-review-packet`, `helpers/name_release.py`, Kani, cargo-mutants, cargo-fuzz. No new
tool.

## Finding status

Items 1 to 4 are under [P0](#p0-blocks-the-move), 5 to 9 in [the before-the-tag
appendix](1087-domain-name-system-outside-review/before-the-tag.md), and 10 to 16 in [the later
items](1087-domain-name-system-outside-review/later-items.md).

- 1, unratified public names: Done, [#3728](https://github.com/nifeos/nife/pull/3728), which
  ratified every public name as built.
- 2, the manifest does not stand alone: Done, [#3658](https://github.com/nifeos/nife/pull/3658).
- 3, the files a stranger needs are absent: Open. [#3658](https://github.com/nifeos/nife/pull/3658)
  added the README and both license files. The scaffold files, the caller workflows,
  `docs/stranger-test.md` and the stranger run come from `script/crate-move` at the move.
- 4, shipped text points into nife: Done, [#3608](https://github.com/nifeos/nife/pull/3608).
- 5, the fuzz target lives in nife: Open. [#3658](https://github.com/nifeos/nife/pull/3658) added
  `#![forbid(unsafe_code)]`. The move stages the target in the crate's own `fuzz/`, and the weekly
  caller passes `fuzz: true`.
- 6, two length names break the units rule: Done, [#3608](https://github.com/nifeos/nife/pull/3608).
- 7, names that need the specification, and the `address` gate: Open. Parts a, c and d landed in
  [#3608](https://github.com/nifeos/nife/pull/3608) and
  [#3668](https://github.com/nifeos/nife/pull/3668). Part b leaves a private binding `address`
  (`src/lib.rs` line 977) and eight test names in `tests/replies.rs` that `register_names` refuses;
  Decision 3 qualifies them as IPv4. The crate's `.github/accepted-words` adds `tcp` and `udp` at
  the move, and `prose_acronyms` flags 13 acronyms in the README and `src/` for expansion or that
  list.
- 8, a TTL with its high bit set: Done, [#3559](https://github.com/nifeos/nife/pull/3559).
- 9, the three equivalent mutants: Open. The move commit writes their `.github/mutants-exempts`
  lines, and `mutation / mutation` has no green full run on `main` yet.
- 10 to 16, `Display` and `FromStr`, type sizes, an exhaustive `Error`, documentation drift,
  host-name syntax, re-capturable fixtures and the TCP buffer: Done,
  [#3608](https://github.com/nifeos/nife/pull/3608).
- Decision 1, `MAXIMUM_LABEL_BYTES` and `HEADER_BYTES`: Done,
  [#3608](https://github.com/nifeos/nife/pull/3608), ratified by
  [#3728](https://github.com/nifeos/nife/pull/3728).
- Decision 2, the specification's abbreviations: Done,
  [#3668](https://github.com/nifeos/nife/pull/3668), ratified by
  [#3728](https://github.com/nifeos/nife/pull/3728).
- Decision 3, the IPv4 address names: Done, [#3608](https://github.com/nifeos/nife/pull/3608),
  ratified by [#3728](https://github.com/nifeos/nife/pull/3728).
- F-package, the review package: Done, [the package](../../notes/crate-reviews/domain-name-system.md).
- F-clearance, review clearance: Open. [Decision 4](#decision-4-write-the-package-now-and-clear-the-move-from-it)
  says the architect clears it from the package.

## Open decisions

None.

## Goal

A stranger who finds `domain_name_system` on its own can tell in one screen that it is the DNS wire
format a stub resolver needs, as pure computation. It covers a query for one name's IPv4 addresses,
the checks a reply must pass, and TCP framing. The same stranger can build, test, prove and fuzz it
from the README alone, meets no name that needs this tree to decode, and finds the repository's
gates green for the crate's own reasons. Nothing in the crate's shipped text points back into nife.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1087-domain-name-system-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1087-domain-name-system-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the release (a public name, a published behavior, a
wire layout, a dependency). Everything else is two-way and the maintainer decides it. Line numbers
are `crates/domain_name_system/src/lib.rs` unless a path says otherwise.

### P0: blocks the move

1. DONE. Every public name carries a `Name:` block, and all are ratified as they stand (2026-11-16
   UTC): `helpers/name_release.py --draft domain_name_system` lists none as provisional or
   without a block, and `script/crate-standalone domain_name_system` passes its names step. The
   ratification pull request's body is that table, which is the one record of these names, since the
   move commit strips the blocks. Door: each name is one-way once published; the block itself is
   two-way.

2. DONE. The manifest does not stand alone. Finding: the manifest inherits `license` and `lints` from the nife workspace, there is no `rust-version`, no `description`, `repository`, `readme`, `keywords` or
   `categories`, and the `publish = false` line's comment names `crates/publish-allowlist.txt`, a
   file the new repository will not have. The header comment cites milestone 384 (nothing here resolves a hostname, and in a capability system the resolver is a grant) and
   `notes/name-resolution.md`. Evidence: `crates/domain_name_system/Cargo.toml` lines 4 to 17;
   `crate-standalone` findings `inherits license`, `inherits lints`, `no rust-version`, `2 milestone
   citation line(s)`. Change: inline `license = "MIT OR Apache-2.0"`. Inline the `[lints]` table the
   template's `standalone.py` requires (`unsafe_op_in_unsafe_fn`, `missing_docs`, `unexpected_cfgs`
   with `cfg(kani)` in its check-cfg list, and the clippy rows the workspace carries). `rust-version
   = "1.88"` (the floor `as_chunks_mut` sets; confirm with `cargo +1.88 test` before the tag). A
   one-sentence `description`. `repository`, `readme`, `categories = ["no-std", "no-std::no-alloc",
   "network-programming", "parser-implementations"]`, `keywords = ["dns", "stub-resolver", "no-std",
   "rfc1035", "rfc7766"]`. Replace the `publish` comment with one a stranger can act on, and the
   header comment with the crate's one-line purpose. Door: two-way (publishing itself has a ruling elsewhere, not reopened here). Effort: XS.

3. The files a stranger needs are absent: README, `LICENSE-MIT`, `LICENSE-APACHE`, and the
   scaffold's `CHANGELOG.md`, `_typos.toml`, `deny.toml`, `rustfmt.toml`, `docs/stranger-test.md`
   and caller workflows. Finding: the crate directory holds `Cargo.toml`, `src/`, `tests/` and
   `falsifications/` only. The template's `check / standalone` refuses a crate with no README, and
   its `helpers/readme_sections.py` wants `## EXAMPLES` and `## BUGS`, which the crate's root doc
   comment already has in substance (lines 1 to 24 and the four doctests). Evidence:
   `crate-standalone` findings `no README`, `no LICENSE-MIT`, `no LICENSE-APACHE`; `cargo package
   --list` ships ten files and no license text. Change: copy `scaffold/` at the move as its README
   says. Write the README from `scaffold/README-crate.md`. It carries the badge row, the
   `Query::accept` doctest as the example, the three harnesses by name and the Kani install line on
   one line. It also gives the measured proof cost (about 65 seconds for the three harnesses here,
   memory not measured). It adds one falsification run with its printed `VERIFICATION:- FAILED`, the
   BUGS list from the root doc, the minimum Rust version and the type sizes measured above. Record
   the first stranger run in `docs/stranger-test.md`. Door: two-way. Effort: S.
   DONE for the README and both license files, which now sit in the crate directory. The scaffold's
   other files, the caller workflows and `docs/stranger-test.md` come from `script/crate-move` at
   the move, and the stranger run follows the move (step 12 of `notes/crate-move.md`).

4. Shipped text points into nife. DONE. Finding: line 4 sends the reader to `notes/name-resolution.md`.
   Line 7 names `fuzz/fuzz_targets/domain_name_system_reply.rs`, a path in nife (item 5 moves the
   target). Lines 262 to 263 cite milestone 384 and say nothing applies `is_within`, which
   `crates/domain_name_resolution_contract/src/grants.rs` line 180 does. Lines 369 to 371 cite
   `crates/multicast_dns_protocol`, a commit hash and milestone 298 (retire the multicast DNS responder and its two crates, which advertised a goal that is gone), which a stranger cannot open.
   The move strips the crate-name block at lines 21 to 24, so it needs no edit for it.
   Evidence: `crate-standalone` findings `5 path into the nife tree line(s)`, `1 provenance block
   line(s)`; the lines above. Change: line 4 becomes "which program sends these bytes, and which
   names a client may ask about, are the caller's policy". Line 7 names the crate's own `fuzz/`
   directory. Lines 262 to 263 say what `is_within` is for (a resolver grant scoped to a zone)
   without the milestone. Lines 369 to 371 keep the one useful fact, that the engine is separate so
   Kani can prove the loop at a small output size, and drop the lineage. `script/crate-standalone`
   must report zero citations. Door: two-way. Effort: XS.

### P2: later

The P2 items are in [the appendix](1087-domain-name-system-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1087-domain-name-system-outside-review/before-the-tag.md).

## Decisions for the architect

Each is a rule in force. The ratification of every public name as built, 2026-11-16 (UTC), covers
the first three. Decision 4 is ruled 2026-11-19 (UTC).

### Decision 1. `MAXIMUM_LABEL_BYTES` and `HEADER_BYTES`

A DNS label is at most 63 bytes and a message header 12. A length or a limit carries its unit, as
`MAXIMUM_NAME_BYTES` and `MAXIMUM_UDP_MESSAGE_BYTES` already did, so the names are
`MAXIMUM_LABEL_BYTES` and `HEADER_BYTES`, with no alias of the old spellings. A caller writes `[0u8;
MAXIMUM_LABEL_BYTES]` and knows the unit from the name. Prior art: `hickory-proto` names its limit
`MAX_LABEL_LENGTH` (unverified); nife's rule is stricter.

### Decision 2. The specification's abbreviations in public names

A term a reader searches for keeps its spelling, with the expansion in its doc and a
`#[doc(alias)]`; any other term expands. People say "question count", not "qdcount", and they do say
"TTL" and "class IN". The names in force:

- `Header::{question_count, answer_count, authority_count, additional_count}` (aliases `QDCOUNT` and
  so on) and `Header::response_code()` (alias `RCODE`).
- `FLAG_RESPONSE`, `FLAG_TRUNCATED` and `FLAG_RECURSION_DESIRED` (aliases `QR`, `TC`, `RD`);
  `RESPONSE_CODE_MASK`, `RESPONSE_CODE_NAME_ERROR` (alias `NXDOMAIN` too) and `OPERATION_CODE_MASK`
  (alias `OPCODE`).
- `Question::{record_type, class}` and `Record::{record_type, class, ttl_seconds, data,
  data_offset}`. `ttl` stays an abbreviation and carries the unit: `Record::ttl_seconds` and
  `Answer::ttl_seconds()`, with aliases `TTL` and `time to live`.
- `record_type::CANONICAL_NAME` and `canonical_name_target` (aliases `CNAME` and `canonical name`),
  and `record_type::IPV4_ADDRESS` (aliases `A` and `address record`).
- `CLASS_IN` keeps its spelling as a term of art, with alias `Internet class`.

Prior art: `hickory-proto` spells the counts out and keeps `ttl` (unverified); the `domain` crate
keeps `qdcount()` (unverified). Example: the resolver's check reads `header.question_count != 1`,
and a reader who has never opened RFC 1035 knows what refused the reply.

### Decision 3. `MAXIMUM_IPV4_ADDRESSES`, `Answer::ipv4_addresses` and `Reject::NoIpv4Address`

The crate returns IPv4 addresses only, and the template's `register_names` gate reads a bare
`address` as a memory address. So the names say which kind: `MAXIMUM_IPV4_ADDRESSES`,
`Answer::ipv4_addresses()` and its field, `Reject::NoIpv4Address`, and the test names to match, with
no template exemption. `domain_name_resolution_contract` asserts the same limit, and `reject_code`
keeps code 9. Prior art: `std::net::Ipv4Addr` and smoltcp's `IpAddress` name the kind where a reader
looks first. Example: `answer.ipv4_addresses()` says what `&[[u8; 4]]` holds, and an
`ipv6_addresses()` later needs no rename.

### Decision 4. Write the package now and clear the move from it

The move follows `notes/crate-move.md`: a review package, the architect's review clearance, then
the repository. Milestone 1017 (the contract crates leave in this order) places this crate in step
5, before `domain_name_resolution_contract`, which pins it as a `git` dependency. The package is
written now, from the crate as it stands, and the architect's clearance is brought from it. The
move does not wait for the resolver contract, whose names are ruled. Creating a public repository
is an outward step, so the clearance itself stays the architect's. Prior art:
`file_allocation_table` and `e1000e` each cleared from a written package before their repository
existed. Example: the architect clears the package, and `script/crate-move domain_name_system`
creates `nifeos/domain_name_system` with the three mutant exemptions and the fuzz target staged.

## Done when

The checks are in [the appendix](1087-domain-name-system-outside-review/done-when.md).

## Follow-on

- **Outstanding.** The open lines under [Finding status](#finding-status): items 3, 5, 7 and 9,
  and the review clearance.

## BUGS

- The template's `abbreviations` helper judged one name when run here. That is almost certainly a
  harness problem on the reviewing side, so the abbreviations report (`tcp`, `udp`
  and any other) stays unverified until the crate runs the template's `check` workflow.
- The crate builds and tests at Rust 1.88 in the move's dry run (`script/crate-move --dry-run`).
- The `register_names` helper did not flag `MAXIMUM_LABEL_LEN` or `HEADER_LEN`, while the written
  rule does. Whether the helper reads a `_LEN` suffix by design is a template question this block
  raises and does not answer.
- Whether the four timed-out mutants count against the template's mutation gate stays unverified.
- The prior-art rows for `hickory-proto`, `domain`, `simple-dns` and the C libraries are from
  memory and the other-implementations note, not from reading those sources here. Each carries a mark.

## Index row

An outside review of `domain_name_system` before it leaves. Four items block the move (48 unratified
names, a manifest and files that do not stand alone, text pointing into nife). Five land before the
first tag (the fuzz target moving with the crate, two length names under the units rule, the
specification's abbreviations and the `address` gate, a TTL the RFC says to zero, the three
equivalent mutants). Seven more are for later. The architect has ratified the three name decisions,
and the review clearance goes to the architect.
