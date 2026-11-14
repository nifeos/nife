---
status: PARTIAL
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1102. `network_time_protocol`: what an outside maintainer would fix before it leaves

Raised by a read-only outside review of `crates/network_time_protocol` as a stranger meeting it for
the first time would. It changes nothing; it lists what the move lane and the release lane change.
The number is a placeholder a recording lane fills in.

The crate has no review package under `notes/crate-reviews/` and no row in `notes/crate-reviews.md`,
so this review had no settled package to inherit. The baseline it takes instead, and does not reopen, is what the tree already records. That is the ratified crate name (`network_time_protocol`, the standard's own word and the one exception to the `_contract` rule in `design/naming.md`), the ratified `UNIX_MINIMUM`, and the recorded `MAXIMUM_STRATUM`, `MAXIMUM_ROOT_DISTANCE` and `leap::UNSYNCHRONISED`. It also includes the four design decisions `notes/ntp.md` records. They are unauthenticated NTPv4 only, exactly 48 bytes, a fixed era pivot rather than a nearest-era rule, and `Packet::parse` total with every judgment in `Query::accept`. Two of those baseline names meet a later rule and are
reopened below, as the release-review rule allows: `UNIX_MINIMUM` carries no unit, and
`UNSYNCHRONISED` is a British spelling the cited source does not use.

Reuse: the template `nifeos/crate-workflows` and its scaffold, its naming helpers, Kani, cargo-mutants
and cargo-fuzz. No new tool.

## Goal

A stranger who finds `network_time_protocol` on crates.io can tell in one screen what it is for and
what it refuses to be. The same stranger can build, test and prove it from the README alone. The
repository's ten required checks are green on the first day for the crate's own reasons, and red only
for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1102-network-time-protocol-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the appendix](1102-network-time-protocol-outside-review/prior-art.md).

## The items

Effort: XS is under an hour for a lane, S is half a day, M is a day or two. A door is one-way when
somebody outside the repository acts on it after the release (a published name, a wire format, an
identity, a layout, a dependency). Everything else is two-way and the maintainer decides it. The
maintainers have published nothing here yet, so every public name is two-way today and one-way after
the first publish. Names are the architect's regardless, and the Decisions section lists the ones
the gates refuse.

### P0: red on the template's required checks the day the repository exists, or blocks any release

1. The manifest does not stand alone.
   Finding: `license.workspace = true` and `[lints] workspace = true` inherit a workspace that will
   not be there. No `rust-version`, so `weekly / msrv` has nothing to build. No `description`,
   `readme`, `repository`, `keywords` or `categories`. The `publish = false` comment sends the
   reader to `crates/publish-allowlist.txt`, a file the new repository does not hold. The dependency
   line is item 7.
   Evidence: `crates/network_time_protocol/Cargo.toml` lines 1 to 19. The template's `standalone`
   lists each.
   Change: inline `license = "MIT OR Apache-2.0"`. Inline the lints `clock_contract` already carries
   (`unexpected_cfgs` naming `cfg(kani)`, `unsafe_op_in_unsafe_fn`, `missing_docs`, the clippy set).
   `rust-version = "1.88"` as the sibling crates say (nobody has verified that the crate builds at
   1.88; edition 2024 needs at least 1.85). A one-line description that says "the NTPv4 wire format
   and client-side checks, no socket". `readme`, `keywords` (`no-std`, `ntp`, `time`, `sntp`) and
   `categories` (`no-std`, `date-and-time`, `network-programming`, `parser-implementations`). A
   `publish` comment a stranger can act on, deleted in the release that publishes.
   Door: two-way (the license has a ruling). Effort: S.

2. No README, no license files, no changelog, no stranger record.
   Finding: the crate ships `Cargo.toml`, two source files and seven patches. The template's
   `standalone` fails on the missing README and `LICENSE-MIT` and `LICENSE-APACHE`. The acronym,
   section-order and `EXAMPLES`/`BUGS` gates have nothing to read. `release / gate` refuses while
   `docs/stranger-test.md` reads `Last run: none`. The crate's whole explanation lives in
   `notes/ntp.md`, `notes/ntp/the-era-pivot.md` and `notes/ntp/what-is-proved.md`, which do not move
   with it.
   Evidence: `ls crates/network_time_protocol`. `script/crate-review-packet` says "None. The crate
   has no README; the stranger test cannot pass."
   Change: a README from `scaffold/README-crate.md`. Its summary states the scope in one screen
   (NTPv4 client side, 48 bytes, unauthenticated, no socket or clock, expires 2104). The two
   documentation examples already in `Reject` and `Query` serve as its EXAMPLES. It lists the seven
   properties in a user's words under "What is proven". It explains the exhaustive nanosecond sweep
   and why Kani does not run it. It gives the Kani install line on one line and one real
   falsification command. A BUGS section says what is not there (NTS, MAC, replay state, selection).
   The two notes' content that a user needs (the pivot table, the window, the solver timings) moves
   into `docs/` of the new repository. The license files, the changelog and the stranger record come
   from the scaffold. The maintainers record a stranger run before the first tag.
   Door: two-way. Effort: M.

3. The crate's own name fails the template's contract-crate gate.
   Finding: `contract_names` refuses a package whose last word is `protocol` unless the crate root's
   documentation opens with `(network_time_protocol, <source>)`, the kept-name form. The root opens
   with "**The NTPv4 wire format** (RFC 5905)", so the gate reads it as a mis-spelled contract crate.
   nife's own lint names the crate as its one exception; the template carries no exception list.
   Evidence: `src/lib.rs` line 1; `contract_names: 1 names break the rule`.
   Change: open the root docs with `(network_time_protocol, RFC 5905)` and keep the sentence that
   follows. The ratified name does not change.
   Door: two-way. Effort: XS.

4. `PACKET_LEN` carries no unit.
   Finding: `register_names` rule 4 (a size, length or limit carries its unit) refuses it at a
   ceiling of 0. `design/naming.md` says the same (`MAXIMUM_NAME_BYTES`, never `_LEN`). The name is
   public and used by three callers.
   Evidence: `src/lib.rs` line 30. `fixtures/src/network_time_test_server.rs` lines 118, 155, 157,
   228, 229. `components/src/network_time_client.rs` lines 159, 177.
   Change: `PACKET_BYTES`, with the callers renamed in the same move (Decision 2).
   Door: a public name. Two-way today. Effort: XS, plus the callers.

5. British spellings in names.
   Finding: `british_spelling` refuses the British form of the low-bit randomizer (public). It refuses the test and harness for the British form of the parse-then-serialize identity (the harness name is also the falsification patch's file
   name and the first line of its header). It also refuses the test for the British form of randomizing only the low bits. `Reject::Unsynchronised` and
   `leap::UNSYNCHRONISED` escape that list only because the stem is not in `naming_words.toml`. The
   rule (an `ise` ending for `ize`) covers them, the abbreviations report flags both, and RFC 5905
   itself writes "unsynchronized" (its section 11.1 and Figure 22), so the recorded citation does
   not support the British form. The client's `reject_code` table names the variant.
   Evidence: `src/lib.rs` lines 124, 226, 524, 886, 1101. `src/verification.rs` line 80.
   the falsification patch of that harness under `falsifications/`.
   `components/src/network_time_client.rs` line 319.
   Change: `randomize_low_bits`, `Unsynchronized`, `UNSYNCHRONIZED` (Decision 2). The test and
   harness names `parse_then_serialize_is_the_identity` and
   `randomizing_the_low_bits_touches_only_the_low_bits`, with the patch file renamed and its header
   and `notes/ntp/what-is-proved.md` updated. Any name kept on purpose needs the kept form at the
   item, and none of these mirrors a source.
   Door: two public names, two-way today. The rest two-way. Effort: S.

6. Nine acronyms are not expanded at first use in the doc comments.
   Finding: `prose_acronyms`, ceiling 0, reports `NTPv4`, `RFC`, `NTS`, `MAC`, `UDP`, `NTP`, `IPv4`,
   `DOWN` (emphasis in a test's doc) and `MAXSTRAT` (a source identifier written as prose). The
   template judges Rust doc comments once per crate, so the root's first sentence can carry most of
   the expansions.
   Evidence: `src/lib.rs` lines 1, 4, 5, 19, 375, 947, 1170.
   Change: expand each at its first use ("Network Time Protocol version 4 (NTPv4)", "Request for
   Comments (RFC) 5905", "Network Time Security (NTS)", "message authentication code (MAC)", "User
   Datagram Protocol (UDP)", "Internet Protocol version 4 (IPv4)"). Lower-case "down". Put
   `MAXSTRAT` in a code span. The repository may instead accept `RFC` as a word, with a reason.
   Door: two-way. Effort: XS.

7. The one dependency is a nife contract crate, taken for one constant.
   Finding: `clock_contract` is a path dependency (`../clock_contract`) whose only use is
   `NS_PER_SECOND`, the number 1,000,000,000. The template's `standalone` flags the path. The
   architect-label rule treats any dependency new to the graph as the architect's. And
   `clock_contract`'s own review package proposes renaming that constant to
   `NANOSECONDS_PER_SECOND`, so the dependency would also inherit a rename.
   Evidence: `Cargo.toml` line 19. `src/lib.rs` line 48. `notes/crate-reviews/clock-contract.md`
   line 133.
   Change: Decision 1. The recommendation is a private `NANOSECONDS_PER_SECOND` in the crate and no
   dependency at all.
   Door: one-way (a dependency). Effort: XS.

### P2: later

The P2 items are in [the appendix](1102-network-time-protocol-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1102-network-time-protocol-outside-review/before-the-tag.md).

## Decisions for the architect

Only the doors somebody outside the repository would act on after the release, and the names, which
are the architect's by rule. Each gives the background first, then the recommendation, then prior
art and an example. Nothing here reopens the crate name, the era pivot, the 48-byte rule or the
unauthenticated scope.

### Decision 1. Does the crate keep a dependency on `clock_contract` for one constant?

Background: the only thing the crate imports is `clock_contract::NS_PER_SECOND`, the number
1,000,000,000. The import exists so that all of nife shares one definition of the second. After the
move the line would become a crates.io dependency on `clock_contract`, which is a nife wire contract
with its own release cadence. `clock_contract`'s review package already proposes renaming the
constant under the units rule, so the dependency inherits that break. nife's rule approves any
nifeos crate as a dependency in advance, so the question is not permission but shape.

Recommendation: no dependency. A private `const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000` in
`src/lib.rs`, one line, spelled under the units rule. A wire-format crate that a stranger pulls in
for RFC 5905 should bring nothing else into their `cargo tree`, and a shared definition of a
physical constant protects nobody from drift: the second does not move.

Prior art: `sntpc`'s core has no dependency for this. `ntp-proto` carries many because it also
carries NTS and the algorithms. `core::time::Duration` defines `NANOS_PER_SEC` privately and exports
no constant.

Example: with the dependency, `cargo tree -p network_time_protocol` for a user's embedded project
shows `clock_contract`. That crate's README is about a seqlock-protected clock page in an operating
system they have never heard of. A `cargo update` can break their build when that crate renames a
constant. Without it, the tree shows one crate.

### Decision 2. The public names the template's gates refuse, and the ones a later rule reopens

Background: the template has no baseline; each of its naming checks fails at a ceiling of 0. Three
public names fail today (items 4 and 5) and five more do not meet rules that came after their names.
All are two-way until the first publish and one-way after it. The move cannot land green without the
first three. Ratified or recorded status does not shield a name from a later rule at its release
review.

Recommendation, one line each, in the order a ruling would take them:

| today | recommended | rule | status today |
|---|---|---|---|
| `PACKET_LEN` | `PACKET_BYTES` | a length carries its unit | no block |
| the British form of the low-bit randomizer | `randomize_low_bits` | American spelling; say what is low | no block |
| `Reject::Unsynchronised`, `leap::UNSYNCHRONISED` | `Unsynchronized`, `UNSYNCHRONIZED` | American spelling; RFC 5905 writes "unsynchronized" | recorded (RFC 5905's leap indicator 3) |
| `UNIX_MINIMUM` | `MINIMUM_UNIX_SECONDS` | a limit opens `MINIMUM_` and ends with its unit | ratified; reopened by the later rule |
| `UNIX_LIMIT` (exclusive) | `MAXIMUM_UNIX_SECONDS` (inclusive, one less) | same rule; std's `MAX` is inclusive | provisional |
| `UNIX_DELTA` | `UNIX_EPOCH_DELTA_SECONDS` | a quantity ends with its unit | no block |
| `DEFAULT_POLL` | `DEFAULT_POLL_EXPONENT` | say what the number is (log2 seconds) | provisional |
| `Short` | `ShortFormat` | a generic word; the RFC's term is "NTP short format" | no block |
| `abs_ticks` | drop it (`ticks().unsigned_abs()` is one call) or `unsigned_abs_ticks` | std's spelling | no block |

Changing `UNIX_LIMIT` to an inclusive maximum changes two comparisons (`>=` becomes `>`), the two
Kani harnesses that state the window and three tests. It is still the same window. If the architect
prefers the exclusive form, `UNIX_SECONDS_LIMIT` keeps the unit and the word.

Prior art: Rust's `u32::MAX` and `Duration::MAX` are inclusive maxima. OpenBSD's `ntpd` names the
epoch constant `JAN_1970` (unverified). `ntpd-rs` names it `EPOCH_OFFSET` (unverified). The RFC's
own names are `MAXDISP`, `MAXSTRAT`, `NOSYNC`, which the crate already spells out as
`MAXIMUM_ROOT_DISTANCE`, `MAXIMUM_STRATUM` and the leap constant.

Example: a user reading `Timestamp::from_unix(seconds, nanoseconds)` and its documentation today
meets `UNIX_MINIMUM..UNIX_LIMIT` and has to learn that `LIMIT` is exclusive. With
`MINIMUM_UNIX_SECONDS..=MAXIMUM_UNIX_SECONDS` the range reads the way every std range does.

### Decision 3. Do `Query::new` and the low-bit randomizer leave before the first publish?

Background: `Query::with_nonce` puts 64 random bits on the wire and keeps the true send time
locally, and the crate's documentation explains at length why this is free and strictly stronger.
`Query::new` and the low-bit randomizer are the weaker path for a caller with no random source. No caller
in nife uses them. Once published, removing them is a breaking change.

Recommendation: remove both now. A caller with no random source has no business proposing a wall
clock from an unauthenticated protocol, and the crate's own documentation says as much. Two public
items that exist only to steer callers away are surface without a user. If the architect wants the
weak path for an embedded user with a weak entropy source, keep the low-bit randomizer under the spelled
name and drop `Query::new`. Then the only constructor takes a nonce, and the caller decides what to
put in it.

Prior art: chrony randomizes the transmit timestamp by default (the crate's documentation;
unverified here). `ntpd-rs` generates a random origin for every request (unverified). `sntpc` sends
the caller's timestamp (its documentation does not describe a nonce).

Example: a user who writes `Query::new(now)` because it is the first constructor in the
documentation ships a client whose replies an off-path attacker forges with about 4,096 guesses on
a microsecond clock, as the low-bit randomizer documentation itself computes.

## Done when

The checks are in [the appendix](1102-network-time-protocol-outside-review/done-when.md).

## Ledger

The ledger of each item is in [the appendix](1102-network-time-protocol-outside-review/ledger.md).

## Follow-on

- **Outstanding.** Items 1, 2, 6, 8, 10, 11 and the fuzz half of 9 are the new repository's, and wait on Decision 1. Checked against `crates/network_time_protocol` and the ledger above.
- **Outstanding.** Items 4, 5, 17, 18 and 22 are public names and a public layout, which wait on Decisions 2 and 3.

## BUGS

- The falsification replay checked that each patch fails its harness. It did not check that the
  patch fails at the assertion its header names. Kani reports most of the seven failures through a
  placeholder message rather than the assertion text. So the replay never matched the "expected to
  fail" line of five headers against the output.
- Item 10's timing (a minute unoptimized) is the workspace manifest's figure; this review did not
  rebuild without the override. Item 12's overflow comes from reasoning over the types. Nobody wrote
  a test.
- The prior-art table marks every claim about another implementation's era rule and nonce as
  unverified except `sntpc`'s, which its own documentation states. The crates.io figures are from
  the API at review time and change daily.
- Coverage was not measured. The template's floor is 85 per cent per file. `verification.rs` builds
  only under `cfg(kani)` and should not count, and `lib.rs` is dense with tests, but the figure is
  not known.
- The MSRV claim (`rust-version = "1.88"`) follows the sibling crates and was not built here.
- Whether the move's provenance strip handles a `Name:` paragraph inside a `//!` crate-root block,
  rather than a `///` item block, was not checked against the tool.

## Index row

An outside review of `network_time_protocol` before it leaves. Seven items turn the template's
required checks red on the first day. They are the manifest, the missing README and licenses, the
contract-crate opening form, `PACKET_LEN`, three British spellings, nine unexpanded acronyms, and a
dependency taken for one constant. Eight items land before the first tag. They are nife references
in shipped text, `forbid(unsafe_code)` and a fuzz opt-in, the optimization the exhaustive sweep
needs, the mutation exemptions, an overflow in `Interval`'s accessors, two RFC checks, the caller's
replay duty, and a liveness proof for `accept`. Seven items are for later. Three decisions for the
architect: the dependency, nine names, and whether the low-entropy constructor ships.
