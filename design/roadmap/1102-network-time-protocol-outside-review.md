---
status: BUILT
built: 2026-11-22
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

## Finding status

- P0-1, the manifest does not stand alone: At the move. It still inherits `license` and `[lints]`
  from the workspace and points at nife's publish allowlist; the move inlines both.
- P0-2, README, licenses, changelog and a stranger record: At the move. The move writes them, and a
  stranger runs the README before the first tag.
- P0-3, the contract-crate gate: Done, [#3841](https://github.com/nifeos/nife/pull/3841). The crate-root documentation opens with the kept-name
  form in code spans, ``(`network_time_protocol`, `RFC 5905`)``, which the template's
  `contract_names` and clippy's `doc_markdown` both accept.
- P0-4, `PACKET_LEN`: Done, [#3804](https://github.com/nifeos/nife/pull/3804), by Decision 2. The constant is `PACKET_BYTES`.
- P0-5, British spellings in names: Done, [#3804](https://github.com/nifeos/nife/pull/3804), by Decision 2. The low-bit randomizer is gone, the
  `Reject` variant for leap indicator 3 is `Unsynchronized` and the harness is
  `parse_then_serialize_is_the_identity`.
- P0-6, acronyms at first use: Done, [#3841](https://github.com/nifeos/nife/pull/3841). The crate-root documentation expands each acronym where
  it first appears, and the template's `prose_acronyms` reports none. Each RFC citation is a code
  span, as `nifeos/domain_name_system` writes them, because that gate drops "for" as a small word
  and so never reads "Request for Comments" as an expansion of RFC.
- P0-7, the dependency on `clock_contract`: Done, [#3804](https://github.com/nifeos/nife/pull/3804), by Decision 1. The crate keeps a private
  `NANOSECONDS_PER_SECOND` and has no dependencies.
- P1-8, shipped text that points back into nife: At the move.
- P1-9, safe Rust: At the move. `#![forbid(unsafe_code)]` landed in [#3567](https://github.com/nifeos/nife/pull/3567); the new repository's fuzz
  opt-in and target remain.
- P1-10, the exhaustive sweep and the workspace profile: At the move, in the new manifest. At
  `opt-level = 2` line coverage of `src/lib.rs` is 97 per cent, over the template's floor of 85,
  so the full sweep can stay in the default test.
- P1-11, surviving mutants: At the move, in the new repository's exemptions file.
- P1-12, the totality of `Interval`: Done, [#3841](https://github.com/nifeos/nife/pull/3841). Both accessors saturate, and the Kani harness
  `the_interval_conversions_are_total` proves them free of overflow and sign-preserving over every
  `i128` tick count, with a falsification patch. Narrowing `from_ticks` to `i64` is a further
  two-way door before the first publish, which the maintainer may rule; the done-when check does
  not need it.
- P1-13, two checks the RFC's reference implementation makes: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P1-14, the caller's replay duty: At the move. The `Query::accept` documentation states it
  ([#3567](https://github.com/nifeos/nife/pull/3567)); the README's BUGS line is written with the README.
- P1-15, a proof that `accept` ever accepts: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-16, `Timestamp::fraction`: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-17, the low-entropy items: Done, [#3804](https://github.com/nifeos/nife/pull/3804), by Decision 3. `Query::new` and the low-bit randomizer
  no longer exist; `Query::with_nonce` is the only constructor.
- P2-18, `Short`'s public field: Done, [#3804](https://github.com/nifeos/nife/pull/3804), by Decision 2. `ShortFormat` keeps its field private,
  with `from_bits` and `bits`.
- P2-19, the `dest` parameter: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-20, drifted notes: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-21, the `Reject` order claim: Done, [#3567](https://github.com/nifeos/nife/pull/3567).
- P2-22, `abs_ticks`: Done, [#3804](https://github.com/nifeos/nife/pull/3804), by Decision 2. `abs_ticks` no longer exists.
- Decision 1, `clock_contract`: Done, [#3804](https://github.com/nifeos/nife/pull/3804), ruled by the maintainer, provisional, 2026-11-14 (UTC).
- Decision 2, the public names: Done, [#3804](https://github.com/nifeos/nife/pull/3804), ruled by the maintainer, provisional, 2026-11-14 (UTC),
  with the `UNIX_*` constants, `DEFAULT_POLL_EXPONENT` and `ShortFormat` renamed, and ratified
  with `PORT` kept.
- Decision 3, the low-entropy items: Done, [#3804](https://github.com/nifeos/nife/pull/3804), ruled by the maintainer, provisional, 2026-11-14
  (UTC).

## Decisions recorded

### Ratify the names now the rename lane has landed

Background. A crate's public names are the interface every user compiles against. The move to
its own repository strips the `Name:` blocks that mark a name provisional, so each name must read
ratified first. Decision 2, a maintainer's provisional ruling, settles nine spellings: units and
bounds spelled into the constants (`PACKET_BYTES`, `MINIMUM_UNIX_SECONDS`, an inclusive
`MAXIMUM_UNIX_SECONDS`), American spellings, `ShortFormat`, and `abs_ticks` gone. Ratification is
the architect's by the naming rule, and a lane has applied the renames; the names stay provisional.

Ruling 2026-11-20 UTC: A. `PORT` stays, not `UDP_PORT`.

- A. Ratify against the renamed source, from the review package.
- B. Ratify Decision 2's table now, before the renames.
- C. Ratify with changes the architect names; the rename lane applies them.

Door: one-way at the first publish. A published public name is a breaking change to rename.

Prior art. Rust's std settles a name at stabilization. `ntpd-rs` names its epoch constant `EPOCH_OFFSET` and OpenBSD's `ntpd`
names it `JAN_1970` (both recalled). nife differs in spelling the unit and the bound into the name.
Concrete example: a user writes `Timestamp::from_unix(seconds, nanoseconds)` and reads the window
as `MINIMUM_UNIX_SECONDS..=MAXIMUM_UNIX_SECONDS`, the way every std range reads.

Blocks: the move.

### Clear the review package and approve the repository

Background. A crate leaves this tree for its own `nifeos` repository only when the architect
clears its review package, and that clearance approves the repository. `network_time_protocol` is a
pure RFC 5905 wire format with no nife vocabulary, so it meets the bar for leaving. The package
is `notes/crate-reviews/network-time-protocol.md`.

Ruling 2026-11-20 UTC: A. The architect clears the package and approves `nifeos/network_time_protocol`.
No tag, no publish.

- A. The architect clears the package, and that clearance creates the repository.
- B. Clear on this outside review alone.
- C. Keep the crate in nife.

Door: one-way. Creating a public repository and moving the crate are outward steps.

Prior art. `sntpc` and `ntp-proto` each live in their own repository and publish on their own
cadence. nife differs in reviewing a whole crate once, before its first publish.
Concrete example: the architect cleared `generational_table` on 2026-11-08 (UTC), and its
repository, `nifeos/generational_table`, dates from that day.

Blocks: the move.

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
   Change: `PACKET_BYTES`, with the callers renamed in the same move, as Decision 2 decides,
   provisionally.
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
   Change: `Unsynchronized` and `UNSYNCHRONIZED`, as Decision 2 decides, provisionally; the
   low-bit randomizer leaves under Decision 3. The test and
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
   Change: a private `NANOSECONDS_PER_SECOND` in the crate and no dependency at all, as Decision 1
   decides, provisionally.
   Door: two-way; it removes a dependency. Effort: XS.

### P2: later

The P2 items are in [the appendix](1102-network-time-protocol-outside-review/later-items.md).

### P1: before the tag

The P1 items are in [the appendix](1102-network-time-protocol-outside-review/before-the-tag.md).

## Decisions for the architect

The maintainer ruled all three, provisionally, on 2026-11-14 (UTC), as two-way doors before the
first publish; the architect may revisit them. Nothing here reopens the crate name, the era pivot,
the 48-byte rule or the unauthenticated scope. Ratifying the names is under "Decisions recorded"
above. The case behind each ruling (background, recommendation, prior art and an example) is in
[the appendix](1102-network-time-protocol-outside-review/decisions.md).

### Decision 1. Does the crate keep a dependency on `clock_contract` for one constant?

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first
publish, which the architect may revisit. Removing a dependency on an own crate needs no
dependency ruling. The crate drops `clock_contract`, its only `[dependencies]` entry, and defines a
private `NANOSECONDS_PER_SECOND` in `src/lib.rs`, as `core::time` defines its own privately.
Item 7 has landed.

### Decision 2. The public names the template's gates refuse, and the ones a later rule reopens

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first
publish, which the architect may revisit. The table in
[the appendix](1102-network-time-protocol-outside-review/decisions.md) is the rule, with two rows
settled elsewhere. `MAXIMUM_UNIX_SECONDS` is inclusive. `abs_ticks` goes, since no caller outside the
crate uses it. The low-bit randomizer leaves under Decision 3, so its row renames nothing; its
test still takes the American spelling. No old spelling stays as an alias. Items 4, 5, 18 and 22
landed.

### Decision 3. Do `Query::new` and the low-bit randomizer leave before the first publish?

Decided by the maintainer on 2026-11-14 (UTC), provisional: a two-way door before the first
publish, which the architect may revisit. Both leave: the stricter surface, since adding a
constructor later is compatible and removing one is not. `Query::new` (`src/lib.rs` line 616) and
the low-bit randomizer (line 226) have no caller outside the crate. Item 17 has landed.

## Follow-on

- **Recorded.** Every finding is Done or At the move, so the milestone is built. The findings
  marked At the move are the move lane's, checked against `crates/network_time_protocol` and its
  manifest. The names are ratified and the review package is cleared.

## Done when

The checks are in [the appendix](1102-network-time-protocol-outside-review/done-when.md).

## BUGS

- The falsification replay checked that each patch fails its harness. It did not check that the
  patch fails at the assertion its header names. Kani reports most of the seven failures through a
  placeholder message rather than the assertion text. So the replay never matched the "expected to
  fail" line of five headers against the output.
- The prior-art table marks every claim about another implementation's era rule and nonce as
  unverified except `sntpc`'s, which its own documentation states. The crates.io figures are from
  the API at review time and change daily.

## Index row

An outside review of `network_time_protocol` before it leaves. Seven items turn the template's
required checks red on the first day. They are the manifest, the missing README and licenses, the
contract-crate opening form, `PACKET_LEN`, three British spellings, nine unexpanded acronyms, and a
dependency taken for one constant. Eight items land before the first tag. They are nife references
in shipped text, `forbid(unsafe_code)` and a fuzz opt-in, the optimization the exhaustive sweep
needs, the mutation exemptions, an overflow in `Interval`'s accessors, two RFC checks, the caller's
replay duty, and a liveness proof for `accept`. Seven items are for later. The maintainer ruled three decisions provisionally (the dependency,
nine names, and whether the low-entropy constructor ships), and a lane built them and wrote the
review package. The crate-root opening form, the acronyms and a proof over `Interval`'s accessors
have landed; the architect cleared the package on 2026-11-20 UTC.
