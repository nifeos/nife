---
status: PARTIAL
raised: 2026-11-09
milestone_dependencies: 818, 1017
decision_dependencies: 247, 305
machine_requirements: riscv64 silicon with the JH7110 TRNG (a VisionFive 2)
specific_machine: radon (the JH7110 TRNG exists only on a VisionFive 2)
needs_person: yes
---
# 1049. `jh7110_entropy` is ready to publish: the release review's findings

Raised by the review lane for step 0 of milestone 1017 (the contract crates leave in this order). The
lane read `nifeos/jh7110_entropy` at `98c211e` as an outside maintainer would meet it, ran what it
could on a host, and wrote this block. It changed nothing in any repository.

Reuse: the crate's own Kani harnesses and falsification patches, the shared template
`nifeos/crate-workflows` and its `scaffold/`, cargo-mutants, cargo-semver-checks, and the review
packet of milestone 1017. No new tool.

## Goal

A stranger who adds `jh7110_entropy` to a bare-metal RISC-V kernel gets a register map and a
decision function that are right and cannot be misread into serving bytes the hardware never
generated. They arrive with the metadata, the CI and the README a crates.io crate
should carry. The items below say what stands between the repository as it is and that.

## What was verified firsthand

What the review ran or read on a host, and what it did not run, is in the
[evidence appendix](1049-jh7110-entropy-release-review/review-evidence.md).

## The items

Each item carries the finding, its evidence and the recommended change. It also says which door it is and gives an effort size. A one-way door is a public API shape, a name, a register layout or the license; everything else is two-way. Sizes: XS under an hour, S a few hours, M a day or two. P0 blocks any publish, P1
lands before the first release, P2 can follow it.

### P0

#### Item 1: The publishing hold

Status: Ruled, held. 0.3.0 is the first published version; it waits on the publishing hold, which lasts until every outside review milestone is delivered.

A standing ruling holds every crates.io publish from this organization
until several crates have left and a review has decided whether any of them combine. No work in
this repository changes that; the block records it so nobody reads a green gate as permission.
The first version published to crates.io is 0.3.0 (see *The first published version*). The repository has no pending 0.2.0 release run: all six of its release runs have finished, and crates.io has no such crate. The tag `v0.2.0` stays as git history. This does not lift the
hold. One-way (a published version is permanent). Ruled. Effort none.

#### Item 2: The manifest refuses to publish, by design

Status: Ruled, held. The comment beside `publish = false` already says the line comes out in the release pull request; both lines come out there, after the hold.

`Cargo.toml` carries `publish = false` and
`.github/workflows/ci.yml` passes `allow-unpublished: true`. Both come out in the release
pull request. The comment beside `publish = false` ("release step 7 removes this line") is
rewritten to say what a reader of this repository can act on: the line comes out when the
crate publishes. Two-way. Effort XS.

#### Item 3: The first publish needs a token

Status: Held. A person's step after the hold lifts; no question is open until then.

crates.io binds a trusted publisher only to a crate that
exists, so the template's release path uses the `crates-io` environment secret
`CRATES_IO_FIRST_PUBLISH_TOKEN` for a crate it cannot find, and trusted publishing afterwards.
A person creates that token (scoped to this one name, short expiry), approves the environment,
then binds the trusted publisher and deletes the token. The token and the `crates-io`
environment approval wait for the publishing hold to lift. Effort S.

#### Item 4: The published tree ran on the board twice, at one commit

Status: Done. Landed in `nifeos/jh7110_entropy#22` and `#23` (`ee0bde8`) and `#25` (`v0.3.0-pre.1`, `497d3ec`, nife #3701). Two bench questions stay open, under *What remains*.

The finding as raised is in the [evidence appendix](1049-jh7110-entropy-release-review/review-evidence.md).

### P1

#### Item 5: Bump the template pin and adopt what the template now expects

Status: Done. Landed in `nifeos/jh7110_entropy#26`, `#27`, `#30` and `#33`. The ruleset on `main` requires `check / mutation-diff` and `proofs / loom`.

The finding as raised is in the [evidence appendix](1049-jh7110-entropy-release-review/review-evidence.md).

#### Item 6: The decode ignores the output width

Status: Done. Landed in `nifeos/jh7110_entropy#19` (harness `a_128_bit_answer_is_never_served`).

`generation_outcome` answers
`Ready([u8; 32])` whenever `RAND_RDY` shows on a seeded core. It never looks at `STATUS_256_BIT_OUTPUT`, although `status` is in hand, and the doc comments
on the mode and status constants say that in 128-bit mode only `RANDOM_NUMBER_0..3` carry the
answer. The crate warns in prose and relies on the caller to have written `MODE_REGISTER`; the
ladder prefers a check the type system or the decoder performs. Ruled: a fourth, refusing
outcome named `WrongOutputWidth` answers a snapshot taken in 128-bit mode, which means `STATUS_256_BIT_OUTPUT` reads zero. A harness with a falsification patch proves it, and the
README's bring-up paragraph points at it. Adding a variant to an enum that is not
`#[non_exhaustive]` breaks exhaustive matches, so one-way. Effort S.

#### Item 7: The decode ignores a generation or reseed in flight

Status: Done. Landed in `nifeos/jh7110_entropy#19` (harness `work_in_flight_is_always_not_ready`).

The finding as raised is in the [evidence appendix](1049-jh7110-entropy-release-review/review-evidence.md).

#### Item 8: `Pool` can be copied and printed

Status: Done. Landed in `nifeos/jh7110_entropy#19`.

`Pool` holds the 32 bytes and derives `Clone, Copy, Debug`. A copy serves the same bytes twice, which is the one property
`no_byte_is_served_twice_across_refills` and the README promise; `Debug`
prints the buffer, so a diagnostic line can carry the bytes into a log. `Pool::refill` also overwrites up to 31 unserved bytes without zeroing first, and
nothing zeroes the buffer on drop. Ruled: `Pool` loses `Clone` and `Copy`, gets a hand-written
`Debug` that prints only `remaining`, and gets a `Drop` that zeroes the buffer. Removing a trait
implementation is breaking, so one-way. Effort XS.

#### Item 9: `Pool`'s names and answer shape at the release review

Status: Done. Landed in `nifeos/jh7110_entropy#19`, with the ratified names.

`Pool::remaining`
returns a count of bytes and the units rule says a length carries its unit.
`Pool::take`'s parameter `n` is a byte count with a one-letter name, and `take`
answers a `(u64, u64)` whose order (count, then word) a caller must remember. The earlier
names predate the units rule, and the release review reopens them. Ratified: `Pool::remaining_bytes`;
`take`'s parameter `byte_count`; the return becomes the struct
`TakenBytes { byte_count: u64, word: u64 }`; the private field `buf` becomes `buffer`. One-way
(names, API). Effort XS.

#### Item 10: `Pool` has no proof

Status: Done. Landed in `nifeos/jh7110_entropy#19`; `nifeos/jh7110_entropy#29` moved both harnesses onto `take_into`.

The finding as raised is in the [evidence appendix](1049-jh7110-entropy-release-review/review-evidence.md).

#### Item 11: The package ships the repository's plumbing

Status: Done. Landed in `nifeos/jh7110_entropy#20`.

`cargo package --list` includes `.github/`
(workflows, templates, budgets, exemptions), `.githooks/`, `.cargo/mutants.toml`,
`docs/stranger-test*.md`, `_typos.toml`, `deny.toml`, `rustfmt.toml` and `CODEOWNERS`. A
user of the crate needs `src/`, `falsifications/` (the release policy ships them), the README,
the changelog, the two license files, `Cargo.toml` and
`docs/the-crate-carries-facts-not-source-text.md`. Recommended: an `include` list naming
those. Two-way. Effort XS.

#### Item 12: Manifest metadata for crates.io

Status: Done. Landed in `nifeos/jh7110_entropy#20`.

`Cargo.toml` has `keywords = ["no-std", "kani",
"driver"]` and no `documentation` field. Recommended: `documentation =
"https://docs.rs/jh7110_entropy"`, keywords a searcher types (`jh7110`, `trng`, `starfive`,
`no-std`, `random`; crates.io allows five), and the category `no-std::no-alloc` beside the
three present (the slug exists). Metadata can change in a later version, so two-way, but the
first version's page is what a search engine indexes first. Effort XS.

#### Item 13: The changelog's order

Status: Done. Landed in `nifeos/jh7110_entropy#19`; the header promises no dates.

`CHANGELOG.md` has an `## Unreleased` section holding "First
version" below `## 0.1.1`, while the tags `v0.1.0`, `v0.1.1` and `v0.2.0` exist, and the
header promises UTC dates that no entry carries. Recommended: the section becomes `## 0.1.0`,
and either each release line carries its date or the header stops promising one. Two-way.
Effort XS.

### P2

#### Item 14: README shape per the template's report-only checks

Status: Done. Landed in `nifeos/jh7110_entropy#20`; the report-only README checks list nothing on `main`.

The README states the minimum
Rust version as a bare line where the template's section order wants `## Minimum Rust
version`. `## EXAMPLES` and `## What is proven` come after sections of the crate's own; the
crate doc's first sentence uses `TRNG` before expanding it. The README also
never says the crate has no dependencies and no `unsafe`, which the stranger run inferred and
could not confirm from the page. These are report-only today and may become required.
Two-way. Effort XS.

#### Item 15: Two named bits are private

Status: Done. Landed in `nifeos/jh7110_entropy#21`.

`INTERRUPT_STATUS_AGE_ALARM` and
`INTERRUPT_STATUS_REQUEST_ALARM` are `pub(crate)`, yet both sit in
the public `INTERRUPT_STATUS_DOCUMENTED_BITS_MASK` and the mask's doc comment tells a driver to
report an unexplained residue, which a driver cannot name for bits 2 and 3. The doc comment
on the mask argues that a bit with no name leaves a status word undecodable.
Recommended: make both `pub`. Additive, two-way. Effort XS.

#### Item 16: Decode the read-only build registers and the reseed-state field

Status: Done, names open. Landed in `nifeos/jh7110_entropy#21`; the new names wait on *Open decisions*.

NetBSD names
`FEATURES` bits (`MM_RESET_STATE` 3, `RAND_SEED_AVAIL` 2, `MAX_RAND_LENGTH` 1:0) and `BCONF`
bits (`AUTO_RESEED_LOOPBACK` 5, `MODE_AFTER_RST` 4, `PRNG_LEN_AFTER_RST` 3, `MAX_PRNG_LEN` 2);
Linux names the `STAT.LAST_RESEED` field at bit 16, whose value 7 the crate cites in prose. The crate names the two register offsets and
no field, so a bench dump of them is undecodable with this crate alone, and exactly these fields answer item 4's width question. `SEED_1..6_REGISTER` are also absent while
`RANDOM_NUMBER_0..7` are all present. Additive; the new names are provisional and go to the
name review. Two-way. Effort S.

#### Item 17: A rand_core adapter behind a feature. Declined

Status: Declined.

A user of the embedded Rust ecosystem may
reach for `rand_core::TryRng` (0.10) and find nothing here to implement it. There is no
`rand_core` feature in 0.3.0. nife reaches the interface through `getrandom` at the contract
layer, and an optional feature nobody uses is API without evidence. Revisit if an outside user
asks or the contract layer needs the interface. Two-way (additive later). No effort.

#### Item 18: Fuzz, or say why not

Status: Done. Landed in `nifeos/jh7110_entropy#21`; the README's `BUGS` says why there is no fuzz target.

The template's `fuzz.yml` caller is absent. The crate parses two
byte strings (`interrupt_line`, `is_status_okay`), both trivial, and holds no `unsafe`, so
public crash inputs fit the opt-in rule. Either add the caller with one
target over both decoders or record in `BUGS` that the crate has nothing worth fuzzing.
Two-way. Effort XS.

#### Item 19: State why `rust-version` is 1.88

Status: Done. Landed in `nifeos/jh7110_entropy#27`, which lowers `rust-version` to 1.85.

The source needs edition 2024 (1.85) and
`Option::is_none_or` (1.82); nothing read asks for 1.88. Either lower to 1.85 and let the
`msrv` job prove it, or add a comment saying what needs 1.88. Not checked with a 1.85
toolchain. Two-way. Effort XS.

## Open decisions

The rulings this block records stand: the first published version (item 1), the width outcome
`WrongOutputWidth` (item 6), the `Pool` changes (item 8), the names of item 9 and the declined
`rand_core` feature (item 17). The publication steps (items 1 to 3) wait on the publishing hold and
ask nothing until it lifts. The maintainers rule the parameter name `out` of `Pool::take_into`
provisionally: a parameter name is not part of a Rust API, so changing it later breaks no caller.
One decision is open.

### Ratify the public names added since the last ratification

The crate gained twelve public names after the four ratified groups. Item 16 added eleven constants
that decode the read-only build registers and the reseed-state field of the status register, and
the `take_into` method arrived with a caller's request to fill a byte slice. Each shipped as
provisional, in the pre-release `v0.3.0-pre.1` that the kernel pins by commit. `SEED_1_REGISTER` to
`SEED_6_REGISTER` follow the ratified `SEED_n_REGISTER` pattern and are not asked again.

It matters now because a crates.io version is permanent. Once 0.3.0 publishes, a rename costs a
0.4.0 and every user's edit; before it, a rename costs a new pre-release and one pin bump in the
kernel. The publishing hold means nothing publishes soon, so the answer is not urgent, but it is the
last thing between this repository and a release pull request.

The constraint is the naming rules: names are spelled out, a length or limit carries its unit, and
the specification's mnemonics stay as `#[doc(alias)]`. The specification's `MAX_RAND_LENGTH`,
`PRNG_LEN_AFTER_RST` and `MAX_PRNG_LEN` are widths with no unit, and the units check refuses
`LENGTH` without one, so the crate spells them `WIDEST_OUTPUT` and `OUTPUT_WIDTH`, which matches the
ratified `MODE_256_BIT_OUTPUT` and `STATUS_256_BIT_OUTPUT`. The seven naming checks of the template
refuse none of the twelve.

The names, with the source mnemonic each decodes; the crate carries the mnemonic as an alias on the
first constant of each field:

| name | source mnemonic |
|---|---|
| `STATUS_LAST_RESEED_MASK`, `STATUS_LAST_RESEED_SHIFT` | Linux `STARFIVE_STAT_LAST_RESEED(x)` (bit 16) |
| `STATUS_LAST_RESEED_UNSEEDED` (the value 7) | none; the crate's prose cited it |
| `FEATURES_MISSION_MODE_RESET_STATE` | NetBSD `FEATURES_MM_RESET_STATE` |
| `FEATURES_RANDOM_SEED_AVAILABLE` | NetBSD `FEATURES_RAND_SEED_AVAIL` |
| `FEATURES_WIDEST_OUTPUT_MASK`, `FEATURES_WIDEST_OUTPUT_256_BIT` | NetBSD `FEATURES_MAX_RAND_LENGTH` |
| `BUILD_CONFIGURATION_AUTOMATIC_RESEED_LOOPBACK` | NetBSD `BCONF_AUTO_RESEED_LOOPBACK` |
| `BUILD_CONFIGURATION_MISSION_MODE_AFTER_RESET` | NetBSD `BCONF_MODE_AFTER_RST` |
| `BUILD_CONFIGURATION_OUTPUT_WIDTH_AFTER_RESET` | NetBSD `BCONF_PRNG_LEN_AFTER_RST` |
| `BUILD_CONFIGURATION_WIDEST_OUTPUT_256_BIT` | NetBSD `BCONF_MAX_PRNG_LEN` |
| `Pool::take_into` | none |

Prior art, read for this brief: NetBSD's `sys/arch/riscv/starfive/jh7110_trng.c` (trunk) names the
`FEATURES` and `BCONF` bits above and describes bit 2 of `FEATURES` as "ring oscillator" present or
not; Linux's `drivers/char/hw_random/jh7110-trng.c` (mainline) names only the `LAST_RESEED` field.
For a method that fills a caller's slice and may stop short, the standard library's `Read::read`
answers the count it wrote, as `take_into` does. rand_core 0.10's `fill_bytes(dst)` and getrandom
0.4's `fill(dest)` fill the whole slice or fail, so a `fill` name would promise more than
`take_into` gives. In the tree, `components/src/timetable.rs` has a private `read_into(..., out)`
of the same shape, and the `_MASK` and `_SHIFT` suffixes appear in `boot_slot`,
`byte_sink_contract`, `clock_contract` and `credential_contract`.

Recommended: A.

- A. Ratify all twelve names as built.
- B. Ratify them except `take_into`, which takes a name you give; this
  costs one rename, a `v0.3.0-pre.2` tag and a kernel pin bump.
- C. Rename any of the eleven constants you name; the same cost as B.

The door is one-way at the first publish and two-way until then. Blocked until it is answered: the
release pull request for 0.3.0, which also waits on the publishing hold.

## The first published version

The tag `v0.2.0` exists at `cfdbeb7` and stays as git history, and `main` has no API change since
it. It has no pending release run: all six release runs have finished, and crates.io has no such crate. No item above can join 0.2.0, because the release workflow refuses a tag that exists, so
any change means a new version. Items 6, 8 and 9 change the public API, and item 7 changes what
the decoder answers, which makes the next version 0.3.0 under the rule that a break bumps the
minor before 1.0. Items 5 and 10 to 19 alone would make 0.2.1.

Ruled: 0.3.0 is the first version published to crates.io. Items 5 to 13 land on `main`, and the release workflow tags 0.3.0 when the hold lifts. Publishing 0.2.0 and breaking it in
the next release would give the registry a permanent version whose `Pool` is `Copy` and whose
decoder cannot refuse a 128-bit answer.

## Prior art

How other systems solve what the items touch, where this crate differs, and which
trees the review read, is in the [prior art appendix](1049-jh7110-entropy-release-review/prior-art.md).

## Done when

The checks on a host, in CI and on hardware are in the [done-when appendix](1049-jh7110-entropy-release-review/done-when.md).
The review claims none of the hardware checks.

## BUGS

- The prior-art rows marked *recalled* were not re-read; a lane that revisits item 17 reads the
  HAL crates it cites first.

## Follow-on

Each item above carries its status. What is still open:

- **Outstanding.** Items 1 to 3, the publication steps: publish 0.3.0, remove `publish = false` and
  `allow-unpublished: true` in the release pull request, then pass `cargo publish --dry-run`, and
  create the first-publish token. All wait on the publishing hold; checked on `main`, where the
  manifest still refuses to publish.
- **Outstanding.** The twelve provisional public names, under *Open decisions*; checked against
  `src/lib.rs` on `main`.
- **Outstanding.** The README's bring-up paragraph gives the measured poll times, a done-when check
  the README contradicted; carried by `nifeos/jh7110_entropy#34`.
- **Outstanding.** `weekly` green. Its last scheduled run failed only `settings drift`, which asked
  for the `proofs / loom` check that the ruleset on `main` now requires; a dispatched run on `main`
  checks it.
- **Outstanding.** Bench: whether a reseed clears `LFSR_LOCKUP`, since no lockup appeared in either
  board run.
- **Outstanding.** Bench: `STATUS_SEEDED`, `LAST_RESEED` and `BCONF` bit 3 at the silicon's reset
  state rather than after the bootloader. That read also settles which value of
  `BUILD_CONFIGURATION_OUTPUT_WIDTH_AFTER_RESET` means 256 bits.
- **Done.** The other checks of the [done-when appendix](1049-jh7110-entropy-release-review/done-when.md)
  hold on `main`, as its closing paragraph lists.

## Index row

The release review of `jh7110_entropy`, the pilot crate. Four items block a publish: the hold, the manifest's refusal, the first-publish token and a run of this tree on the board. Nine land before the first release. They cover the template pin and mutation workflow, a decoder that refuses a 128-bit answer and an in-flight generation, a `Pool` with no copy, no print of its bytes and proofs, package contents, metadata and changelog order. Five follow, and the sixth, a `rand_core` feature, is declined. The first crates.io version is 0.3.0, and no 0.2.0 release run is pending.
