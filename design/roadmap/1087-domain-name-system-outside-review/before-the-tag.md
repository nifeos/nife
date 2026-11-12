# P1: before the tag

An appendix to [the milestone](../1087-domain-name-system-outside-review.md).

## P1: before the first tag

5. The fuzz target lives in nife's `fuzz/` workspace, not in the crate, and the crate's doc leans
   on it. Finding: `fuzz/fuzz_targets/domain_name_system_reply.rs` asserts the three properties the
   root doc relies on ("`Query::accept` is total, and that is fuzzed", line 651), seeded by
   `tests/fixtures`. The template's `fuzz.yml` fuzzes only a repository with its own
   `fuzz/Cargo.toml`, and publishes crash inputs only when the caller sets `public-crash-inputs:
   true`, which the ruling allows for a crate with no `unsafe`. This crate has none (`grep unsafe`
   finds nothing), but it does not say so. Evidence: `fuzz/Cargo.toml` lines 68 and 159 to 160;
   `notes/fuzzing/targets.md` section `domain_name_system_reply`; the template README's fuzz
   section. Change: move the target into `crates/domain_name_system/fuzz/` with its own manifest and
   lockfile in the shape of the template's `fixtures/fuzz-clean/fuzz/`, seeding from
   `../tests/fixtures`. Add `#![forbid(unsafe_code)]` to `src/lib.rs` so the template's unsafe scan
   passes by declaration and a future `unsafe` is a deliberate edit. Set `public-crash-inputs: true`
   in the caller. Leave a stub in nife's `fuzz/Cargo.toml` only if `script/fuzz` still lists the
   target, else remove the row. Door: two-way. Effort: S.

6. Two length names break the units rule, one of them ratified before the rule. Finding:
   `design/naming.md` says a length or a limit carries its unit and gives `MAXIMUM_NAME_LEN` as the
   form never to write. `MAXIMUM_LABEL_LEN` (line 47) is that form, ratified in #2797 before the
   rule, and the crate-reviews index says a release review reopens a name ratified before a later
   rule. `HEADER_LEN` (line 50) has the same shape and no ratification. The template's `register_names`
   helper did not flag either here, so a gap in the helper stays unconfirmed. The written rule is
   clear without it. Evidence: lines 47 and 50; `design/naming.md`, "Units". Change:
   `MAXIMUM_LABEL_BYTES` and `HEADER_BYTES`, with the four nife callers that name them updated in
   the same pull request. This is Decision 1. Door: one-way. Effort: XS.

7. Public names a stranger cannot read without the specification, and names the template's gate
   refuses. Finding, part a: `Header::{qdcount, ancount, nscount, arcount}` and `Header::rcode`
   (lines 136 to 143, 179), `Question::{qtype, qclass}` (lines 428 to 430), `Record::{rtype, rdata,
   rdata_off, ttl}` (lines 441 to 449), `FLAG_QR`, `FLAG_TC`, `FLAG_RD`, `RCODE_MASK`,
   `RCODE_NAME_ERROR`, `CLASS_IN` (lines 53 to 76) and `cname_target` (line 518) are the
   specification's abbreviations. The naming page keeps a term a reader searches for, with its
   expansion and a `#[doc(alias)]`, and expands every other term. `rdata_off` holds `off`, an
   abbreviation no rule keeps. None of these carries an alias or opens with its expansion today.
   Finding, part b: `MAXIMUM_ADDRESSES` (line 88, ratified in #2797), `Answer::addresses` (line
   579), the field `addresses` (line 571), `Reject::NoAddress` (line 565), a binding at line 771 and
   seven test names fail the template's `register_names` gate at a ceiling of zero. The gate reads
   an `address` as `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS` unless a qualifier from its list (`ip`,
   `ipv4`, `ipv6`, `mac` and others) stands before it. The crate would arrive red at `check /
   standalone`. The crate is IPv4-only (its BUGS say so) and `addresses()` returns `[[u8; 4]]`, so
   the qualifier is also the honest name once AAAA arrives. Finding, part c: `spelled_out` flags the
   binding `pos` (line
   374) and the test `addresses_past_the_cap_are_dropped_and_the_rest_kept` (`tests/replies.rs` line
   319, `cap`). Finding, part d: `register_names` flags the field `Name::len` (line 197) as a length without a unit. It is private, mirrors the standard library's `len`, and the fix is the
   doc opening the helper asks for, or `len_bytes`. Evidence: the helper outputs quoted under "What
   was checked" and the lines above. `crates/domain_name_resolution_contract/src/lib.rs` line 50
   asserts `MAXIMUM_ADDRESSES` equal to its own, and `src/messages.rs` lines 46 to 58 number every
   `Reject` variant. Change: parts a and b are Decisions 2 and 3. Parts c and d are the
   maintainer's: `position`, `..._past_the_limit_...`, and a doc opening or `len_bytes`. Whatever
   the rulings, every kept specification term gets `#[doc(alias)]` and an opening expansion, and the
   crate's `.github/accepted-words` lists `tcp`, `udp`, `cname` and `ttl` with reasons if the
   abbreviations report names them. Door: one-way for parts a and b; two-way for c and d. Effort: S
   for the rename pull request across the four nife callers.

8. The crate believes a TTL with its high bit set as written.
   Finding: RFC 2181 section 8 says a receiver treats a TTL with the most significant bit set as zero. `accept` takes `record.ttl` into `min` at lines 750 and 770 unchanged, so a
   reply carrying `0xffff_ffff` yields `Answer::ttl()` of 4,294,967,295 (probed), and
   `components/src/domain_name_resolver.rs` line 283 hands that to its clients as how long to
   believe the answer. Nothing caches today, so no current user is wrong; the claim "how long the
   whole answer may be believed" (line 583) is.
   Evidence: lines 750, 770, 583; the probe above.
   Change: one helper, `fn believable_ttl(ttl: u32) -> u32 { if ttl & 0x8000_0000 != 0 { 0 } else
   { ttl } }`, applied at both sites; one test with such a record; one sentence on `Answer::ttl`
   citing the RFC section. The alternative, refusing the reply, loses an answer that is otherwise
   good for the sake of a field the RFC itself says to zero.
   Door: two-way (a behavior fixed before any release). Effort: XS.

9. The three equivalent mutants need a home in the new repository.
   Finding: the census records three survivors (`Name::is_within` `<` to `<=`, two `|` to `^` on
   disjoint bit ranges), each triaged as equivalent with a reason. The template's `mutation`
   workflow goes red on a survivor absent from `.github/mutants-exempts`, and the release gate reads
   that verdict. Whether the four timeouts count against the gate stays unverified.
   Evidence: `notes/project-metrics/mutation-triage.csv` lines 736, 739, 743; the scaffold's
   `mutants-exempts` header.
   Change: carry the three lines and their reasons into `.github/mutants-exempts` at the move, and
   dispatch `mutation.yml` once on `main` before the tag so the gate has a green full run.
   Door: two-way. Effort: XS.
