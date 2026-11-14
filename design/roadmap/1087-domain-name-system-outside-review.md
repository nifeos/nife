---
status: NOT-STARTED
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

The accepted baseline is thinner than for the other crates on journey 5: `notes/crate-reviews.md`
lists no package for this crate, and `notes/crate-reviews/` holds none. Four things are settled.
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

1. 48 public names carry no `Name:` block, so `script/crate-standalone` fails at step 0. Finding:
   `helpers/name_release.py --draft domain_name_system` lists the crate name and five constants as
   ratified. Every other public item has no block (`PORT`, `HEADER_LEN`, the flag and mask
   constants, `record_type::{A, CNAME}`, `CLASS_IN`, `Error`, `Result`, `Header`, `Name`,
   `Question`, `Record`, `Reader`, `Query`, `Reject`, `Answer`, `TcpReply`, `decode_name`,
   `cname_target` and every method). The move commit strips the blocks, and nife keeps no other
   record, so the ratification pull request is the only record these names will ever have. Evidence:
   `crate-standalone` output, `FAIL names: 48 name(s) in the crate not ratified, first PORT`.
   Change: one pull request whose body is the `--draft` table, after the architect rules items 6 and 7, so the
   table carries the final spellings. The names this review recommends ratified as they stand are
   every one not named in items 6 and 7. Door: each name is one-way once published; the block itself
   is two-way. Effort: S for the lane, plus the architect's reading.

2. The manifest does not stand alone. Finding: the manifest inherits `license` and `lints` from the nife workspace, there is no `rust-version`, no `description`, `repository`, `readme`, `keywords` or
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

4. Shipped text points into nife. Finding: line 4 sends the reader to `notes/name-resolution.md`.
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

Each is one decision, with the background first and the recommendation before the alternatives.
All three are decided, provisionally. The review recommends ratifying the remaining 40-odd names as they stand, and they need only the pull request of item 1.

### Decision 1. `MAXIMUM_LABEL_LEN` and `HEADER_LEN`

Background. A DNS label is at most 63 bytes, and a message header is 12 bytes. The crate names these
`MAXIMUM_LABEL_LEN` and `HEADER_LEN`. The units rule says a length or a limit carries its unit, with
`MAXIMUM_NAME_BYTES` as the right form and `MAXIMUM_NAME_LEN` as the wrong one. The crate already
follows it for `MAXIMUM_NAME_BYTES` and `MAXIMUM_UDP_MESSAGE_BYTES`. `MAXIMUM_LABEL_LEN` got its ratification before the rule, and the review index says a later rule reopens such a name at release
review. Four nife manifests depend on the crate; the names appear inside the crate and in
`components/`.

Recommendation: `MAXIMUM_LABEL_BYTES` and `HEADER_BYTES`. It matches the two sibling constants, the
rule's own example, and what the template's gate would ask of a new name.

Alternative: keep both as ratified, on the ground that a label length in DNS is a count of bytes by
definition. The rule's example rejects exactly this argument for `MAXIMUM_NAME_LEN`.

Prior art: `hickory-proto` names its limit `MAX_LABEL_LENGTH` (unverified); `smoltcp` has no
constant for it. The nife rule is stricter than both, deliberately.

Concrete example: a caller sizing a label buffer writes `[0u8; MAXIMUM_LABEL_BYTES]` and knows the
unit from the name, as it already does with `[0u8; MAXIMUM_UDP_MESSAGE_BYTES]`.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The recommendation holds: `MAXIMUM_LABEL_BYTES` and
`HEADER_BYTES`, with no alias of the old spellings. Item 6 is the work.

### Decision 2. The specification's abbreviations in public field and constant names

Background. RFC 1035 names the header counts `QDCOUNT`, `ANCOUNT`, `NSCOUNT`, `ARCOUNT`, the flag
bits `QR`, `TC`, `RD`, the response code `RCODE`, the class `IN`, the record fields `TYPE`, `CLASS`,
`TTL`, `RDLENGTH`, `RDATA`, and the record types `A` and `CNAME`. The crate carries most of these as
lower-case field names (`qdcount`, `rcode`, `qtype`, `qclass`, `rtype`, `rdata`, `rdata_off`, `ttl`)
and as constants (`FLAG_QR`, `FLAG_TC`, `FLAG_RD`, `RCODE_MASK`, `RCODE_NAME_ERROR`, `CLASS_IN`,
`record_type::A`, `record_type::CNAME`), plus `cname_target`. The naming page's rule: a
specification's term a reader searches for keeps its spelling, with the expansion in the doc and a
`#[doc(alias)]`. Any other term expands. The acronym test asks whether people speak the expansion. Nobody
says "qdcount" or "ancount" aloud; people say "question count" and "answer count". People do say
"TTL", "CNAME", "an A record" and "class IN". `rdata_off` holds `off`, which no rule keeps.

Recommendation, one list, ruled as one decision:

- `Header::{question_count, answer_count, authority_count, additional_count}`, each with
  `#[doc(alias = "QDCOUNT")]` and so on; `Header::response_code()` with alias `RCODE`.
- `FLAG_RESPONSE` (alias `QR`), `FLAG_TRUNCATED` (alias `TC`), `FLAG_RECURSION_DESIRED` (alias
  `RD`). `RESPONSE_CODE_MASK` and `RESPONSE_CODE_NAME_ERROR` (alias `RCODE`, and `NXDOMAIN` on the
  second). `OPCODE_MASK` stays, since "opcode" is a spoken word.
- `Question::{record_type, class}` and `Record::{record_type, class, ttl, data, data_offset}`,
  with the `Question` fields losing their `q` and `data_offset` spelled out.
- Keep `ttl`, `record_type::A`, `record_type::CNAME`, `CLASS_IN` and `cname_target`: each is the
  term a reader searches for, and each gets the expansion in its doc opening and an alias
  (`time to live`, `address record`, `canonical name record`, `Internet class`).

Alternative A: keep every specification spelling and add the aliases. This is what the `domain`
crate does (`qdcount()`, `rcode()`, `tc()`; unverified) and what a reader with the RFC open expects.
It is also what the acronym test refuses for the counts.

Alternative B: spell out everything, `ttl` and `cname` included (`time_to_live`,
`canonical_name_target`, `CLASS_INTERNET`, `record_type::ADDRESS`). Consistent, and it costs the
reader the four terms every DNS text uses on its first page.

Prior art: `hickory-proto`'s `Header` spells the counts out (`query_count`, `answer_count`,
`name_server_count`, `additional_count`) and keeps `ttl` on records (unverified, from memory of its
API). `smoltcp`'s `dns.rs` keeps the RFC field names in its internal `field` module and exposes none
of them. Both sides exist in the ecosystem; the recommendation takes the one the nife rule already
picked for `network_time_protocol` and `non_volatile_memory_express`.

Concrete example: a log line in the resolver today reads `h.qdcount != 1`. After the ruling it reads
`header.question_count != 1`, and a reader who has never opened RFC 1035 knows what refused the
reply. This decision leaves `domain_name_resolution_contract::reject_code` alone; Decision 3
touches it.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The recommendation's list holds as written, with the
specification's spelling as `#[doc(alias)]` on each expanded name. `ttl`, `record_type::A`,
`record_type::CNAME`, `CLASS_IN` and `cname_target` keep their spelling as terms of art, each with
its expansion in the doc opening and an alias. Item 7 part a is the work.

### Decision 3. `MAXIMUM_ADDRESSES`, `Answer::addresses` and `Reject::NoAddress`

Background. The crate returns IPv4 addresses as `[[u8; 4]]`, caps them at `MAXIMUM_ADDRESSES`
(ratified), and refuses a reply with none as `Reject::NoAddress`. The template's `register_names`
gate runs in every crate repository at a ceiling of zero. It reads a bare `address` as a memory
address and demands `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS` unless a qualifier from its list
(`ip`, `ipv4`, `ipv6`, `mac`, `network`, `socket` and others) stands before it. It flags 15 names
here, ratified and unratified alike, and the crate would arrive red. The crate is IPv4-only by its
own BUGS list, and when AAAA arrives `addresses()` returning `[u8; 4]` will need a sibling anyway.
`domain_name_resolution_contract` asserts its own `MAXIMUM_ADDRESSES` equals this one (its `lib.rs`
line 50) and numbers `Reject::NoAddress` as code 9 (`messages.rs` line 57).

Recommendation: `MAXIMUM_IPV4_ADDRESSES`, `Answer::ipv4_addresses()`, the field to match,
`Reject::NoIpv4Address`, and the seven test names qualified the same way. The names say what the
bytes are, the gate passes without an exemption, and an `ipv6_addresses()` later needs no rename of
this one. The contract crate's assertion and `reject_code` arm follow in the same pull request; the
code number 9 does not change.

Alternative A: widen the gate in `crate-workflows` so `address` passes in a crate that declares a
network vocabulary. That is a template change for one crate's convenience, and the first future
`e1000e` or `designware_ethernet` reader would then meet a bare `address` and wonder which kind.

Alternative B: keep the names and list the 15 in a baseline the gate tolerates. The template has
no such baseline for `register_names` by design.

Prior art: `std::net::Ipv4Addr` and `SocketAddr` qualify in the type, not the field.
`hickory-proto`'s `A` record type is the IPv4 record and its `lookup_ip` returns `IpAddr`
(unverified). `smoltcp`'s DNS socket returns `IpAddress`. All three say which kind of address in the
name a reader sees first.

Concrete example: today `answer.addresses()` returns `&[[u8; 4]]`, and the type is the only place
the reader learns these are IPv4. After the ruling `answer.ipv4_addresses()` says it, and the
template's gate reports zero findings on the crate.

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first tag,
which the architect may revisit. The recommendation holds: `MAXIMUM_IPV4_ADDRESSES`,
`Answer::ipv4_addresses()` and its field, `Reject::NoIpv4Address` and the test names to match,
with no template exemption. The contract crate's assertion and `reject_code` arm follow, and the
code number 9 does not change. Item 7 part b is the work, and milestone 1086 (the resolver contract's outside review) decision 1 takes
`MAXIMUM_IPV4_ADDRESSES` to match.

## Done when

The checks are in [the appendix](1087-domain-name-system-outside-review/done-when.md).

## BUGS

- No review package exists for this crate in `notes/crate-reviews/`, so this block had to assemble
  its own baseline from the ratification record, the roadmap and the resolver notes. The crate still needs a package before the architect's reading; this block is not one.
- The template's `abbreviations` helper judged one name when run here. That is almost certainly a
  harness problem on the reviewing side, so the abbreviations report (`tcp`, `udp`, `cname`, `ttl`
  and any other) stays unverified until the crate runs the template's `check` workflow.
- The build at Rust 1.88 was not run; `as_chunks_mut` is the only item known to set that floor.
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
equivalent mutants). Seven more are for later. Three decisions for the architect, all names.
