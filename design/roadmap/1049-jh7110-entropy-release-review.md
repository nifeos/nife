---
status: NOT-STARTED
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

The lane ran or read every claim in this section on a host; nothing here ran on a board.

- `cargo build`, `cargo test` (18 unit tests, 2 doctests), `cargo clippy --all-targets -D warnings`,
  `cargo doc` with warnings denied and `cargo fmt --check` pass on stable 1.99.1 and on 1.88, the
  manifest's `rust-version`.
- Kani 0.68.0 verifies all four harnesses. Each of the four falsification patches applies with
  `git apply -p1` and makes its own harness fail.
- cargo-mutants: 79 mutants, 76 caught, 1 unviable, 2 missed, and the 2 missed are the two
  `1 << 0` constants `.github/mutants-exempts` exempts with a reason.
- cargo-semver-checks reports no API change between the tag `v0.2.0` and `main`.
- `cargo package` builds a 37,612-byte archive; `cargo publish --dry-run` was not run because the
  manifest refuses it (`publish = false`).
- The 20 register offsets, 4 command values and 20 bit constants match Linux's
  `drivers/char/hw_random/jh7110-trng.c` and NetBSD's `sys/arch/riscv/starfive/jh7110_trng.c`,
  read from their current trees. NetBSD also names field bits in `FEATURES` and `BCONF` that the
  crate does not.
- The source holds no `unsafe`, no dependency, no contraction in prose, and no mention of the
  parent project beyond the organization and App names the workflows need.
- The template pin `d332215` sits on `nifeos/crate-workflows` `main`, 57 commits behind it. Since the pin the template has moved the full mutation run into its own `mutation.yml`. It has made
  `check / mutation-diff` a required check on every crate repository's `main`. It has also taught the release gate to refuse a release without a green full mutation run at the release commit.
- The newest weekly run (`42658233510`) failed in `weekly / mutation`: the verdict step found
  `0 of 1 shards left an outcomes.json` after the shard itself passed in 55 seconds. The weekly
  run before it was green. The defect is in the pinned template's artifact handling, not in the crate.
- The current template's helpers, run over the checkout, give this result. The seven naming checks refuse 0 of 162 names, and `standalone` passes. The report-only checks list three README section-order findings, one
  acronym (`TRNG` in the crate doc's first sentence, before its expansion) and 20 name components
  that are not dictionary words (`reseed`, `unseeded`, `buf`, and test-name components).
- crates.io has no `jh7110_entropy`. rand_core's current release is 0.10.1; its traits are
  `TryRng`, `Rng`, `CryptoRng` and `TryCryptoRng`, with `RngCore` and `TryRngCore` kept as
  compatibility traits. getrandom's README documents an opt-in `custom` backend selected by the
  `getrandom_backend` configuration flag.
- The GitHub team `nifeos/crate-maintainers` that `CODEOWNERS` names exists.

Not run: a comparison against the JH7110 Technical Reference Manual (no copy in hand), the vendor
Linux and U-Boot trees, Miri, fuzzing, a build on a 1.85 toolchain, the template's release dry run,
and anything on hardware.

## The items

Each item carries the finding, its evidence and the recommended change. It also says which door it is and gives an effort size. A one-way door is a public API shape, a name, a register layout or the license; everything else is two-way. Sizes: XS under an hour, S a few hours, M a day or two. P0 blocks any publish, P1
lands before the first release, P2 can follow it.

### P0

#### Item 1: The publishing hold

A standing ruling holds every crates.io publish from this organization
until several crates have left and a review has decided whether any of them combine. No work in
this repository changes that; the block records it so nobody reads a green gate as permission.
The first version published to crates.io is 0.3.0 (see *The first published version*). The repository has no pending 0.2.0 release run: all six of its release runs have finished, and crates.io has no such crate. The tag `v0.2.0` stays as git history. This does not lift the
hold. One-way (a published version is permanent). Ruled. Effort none.

#### Item 2: The manifest refuses to publish, by design

`Cargo.toml` carries `publish = false` and
`.github/workflows/ci.yml` passes `allow-unpublished: true`. Both come out in the release
pull request, and the comment beside `publish = false` ("release step 7 removes this line") is
rewritten to say what a reader of this repository can act on: the line comes out when the
crate publishes. Two-way. Effort XS.

#### Item 3: The first publish needs a token

crates.io binds a trusted publisher only to a crate that
exists, so the template's release path uses the `crates-io` environment secret
`CRATES_IO_FIRST_PUBLISH_TOKEN` for a crate it cannot find, and trusted publishing afterwards.
A person creates that token (scoped to this one name, short expiry), approves the environment,
then binds the trusted publisher and deletes the token. The token and the `crates-io`
environment approval wait for the publishing hold to lift. Effort S.

#### Item 4: The published tree ran on the board once, at one commit

The bench session ran on radon (a VisionFive 2) on 2026-11-09 (UTC) against
`nifeos/jh7110_entropy` commit `ee0bde8feb9f78a67da011c928f9a2a697f3bbc1`, with a payload built from nife
`9a1c55963b`, loaded over the network after a cold power-on.
The checker printed PASS; the table is in
[bench/radon-2026-11-09/](../../bench/radon-2026-11-09/README-1049.md), and the crate's README
"Hardware evidence" section records the run for that commit only.

- Passed: the polled reseed and two generations, 256-bit output after `MODE_256_BIT_OUTPUT`, and
  1,048,576 driver-path generations with no timeout, wrong width or all-zero output.
- Recorded, not judged: the first `STAT` read showed `SEEDED` = 1 and `LAST_RESEED` = 4, the
  state after the bootloader and not shown to be silicon reset. `MODE` (`0x08`), `FEATURES` (`0x0d`)
  and `BCONF` (`0x3d`) were read before any `MODE` write, so "`BCONF` bit 3 set means 256-bit" holds
  only if nothing wrote `MODE` since reset. `STAT` bits 19 to 23 never set. Timings are in the table.
- Open: no `LFSR_LOCKUP` appeared, so whether a reseed clears it is unanswered.
- Statistics: one 1,310,720-byte capture, 2 of 524 FIPS 140-2 blocks failed (bar at most 3), 7.999854
  bits per byte. It shows no gross defect and proves nothing about
  unpredictability. No outside suite ran.

The console log (SHA-256 `540996ad...960e`, kept by the maintainers) holds two boots, and the judged
run is the second. The capture's SHA-256 is `97fc5325...9263`.
Two-way. Effort M, bench time.

### P1

#### Item 5: Bump the template pin and adopt what the template now expects

`ci.yml` and the five
sibling callers pin `d332215`, 57 commits behind. At the current template the full mutation run lives in a caller `.github/workflows/mutation.yml`. That file carries the repository's own cron line, which `python3 helpers/mutation_plan.py schedule jh7110_entropy` prints in a template checkout. `check / mutation-diff` is a required check that `script/apply-settings` adds to the ruleset,
`.github/ci-job-budgets` gains `mutation-diff 30` with its reason, and the release gate
refuses a release with no green full mutation run at that commit. The red weekly run
(`42658233510`) is the pinned template's defect and goes away with the bump. Two-way. Effort S.

#### Item 6: The decode ignores the output width

`generation_outcome` answers
`Ready([u8; 32])` whenever `RAND_RDY` shows on a seeded core. It never looks at `STATUS_256_BIT_OUTPUT`, although `status` is in hand, and the doc comments
on the mode and status constants say that in 128-bit mode only `RANDOM_NUMBER_0..3` carry the
answer. The crate warns in prose and relies on the caller to have written `MODE_REGISTER`; the
ladder prefers a check the type system or the decoder performs. Ruled: a fourth, refusing
outcome named `WrongOutputWidth` answers a snapshot taken in 128-bit mode, which means `STATUS_256_BIT_OUTPUT` reads zero. A harness with a falsification patch proves it, and the
README's bring-up paragraph points at it. Adding a variant to an enum that is not
`#[non_exhaustive]` breaks exhaustive matches, so one-way. Effort S.

#### Item 7: The decode ignores a generation or reseed in flight

`STATUS_GENERATING` and
`STATUS_RESEEDING` exist as constants, and `generation_outcome` does not
read them. `generation_outcome` answers `Ready` for a snapshot with `RAND_RDY` latched while `STAT` reports a generation in flight, and `RANDOM_NUMBER_0..7` may be mid-rewrite at that instant. Linux's driver
polls both bits clear before issuing a command, which is why it never meets this; a pure decoder
can return `NotReady` while either bit shows, which costs a caller one more poll and nothing else.
The hardware can show `RAND_RDY` and `RAND_GENERATING` together: the bench run saw both in 8,775
snapshots after a second `GENERATE` over a latched `RAND_RDY` (item 4). Recommended: fold the two bits into the `NotReady` arm, extend
`neither_bit_set_is_always_not_ready` or add a harness, and add the falsification patch. The
function's signature does not change; its answers for some inputs do. Two-way. Effort S.

#### Item 8: `Pool` can be copied and printed

`Pool` holds the 32 bytes and derives `Clone, Copy, Debug`. A copy serves the same bytes twice, which is the one property
`no_byte_is_served_twice_across_refills` and the README promise; `Debug`
prints the buffer, so a diagnostic line can carry the bytes into a log. `Pool::refill` also overwrites up to 31 unserved bytes without zeroing first, and
nothing zeroes the buffer on drop. Ruled: `Pool` loses `Clone` and `Copy`, gets a hand-written
`Debug` that prints only `remaining`, and gets a `Drop` that zeroes the buffer. Removing a trait
implementation is breaking, so one-way. Effort XS.

#### Item 9: `Pool`'s names and answer shape at the release review

`Pool::remaining`
returns a count of bytes and the units rule says a length carries its unit;
`Pool::take`'s parameter `n` is a byte count with a one-letter name; `take`
answers a `(u64, u64)` whose order (count, then word) a caller must remember. The earlier
names predate the units rule, and the release review reopens them. Ratified: `Pool::remaining_bytes`;
`take`'s parameter `byte_count`; the return becomes the struct
`TakenBytes { byte_count: u64, word: u64 }`; the private field `buf` becomes `buffer`. One-way
(names, API). Effort XS.

#### Item 10: `Pool` has no proof

The README says `Pool` has unit tests and no harness. Its invariants are small enough for Kani. For any `n: u64` and any generator that answers or refuses, `take` returns a count of at most `min(n, 8)`, serves the generator's bytes in order
across one refill seam, zeroes what it served and never panics. When the generator refuses, it answers a short count rather than zeros. The buffer is 32 bytes and the gather loop runs at most 8 times, so the harnesses stay cheap. Each one carries a falsification patch. Two-way. Effort M.

#### Item 11: The package ships the repository's plumbing

`cargo package --list` includes `.github/`
(workflows, templates, budgets, exemptions), `.githooks/`, `.cargo/mutants.toml`,
`docs/stranger-test*.md`, `_typos.toml`, `deny.toml`, `rustfmt.toml` and `CODEOWNERS`. A
user of the crate needs `src/`, `falsifications/` (the release policy ships them), the README,
the changelog, the two license files, `Cargo.toml` and
`docs/the-crate-carries-facts-not-source-text.md`. Recommended: an `include` list naming
those. Two-way. Effort XS.

#### Item 12: Manifest metadata for crates.io

`Cargo.toml` has `keywords = ["no-std", "kani",
"driver"]` and no `documentation` field. Recommended: `documentation =
"https://docs.rs/jh7110_entropy"`, keywords a searcher types (`jh7110`, `trng`, `starfive`,
`no-std`, `random`; crates.io allows five), and the category `no-std::no-alloc` beside the
three present (the slug exists). Metadata can change in a later version, so two-way, but the
first version's page is what a search engine indexes first. Effort XS.

#### Item 13: The changelog's order

`CHANGELOG.md` has an `## Unreleased` section holding "First
version" below `## 0.1.1`, while the tags `v0.1.0`, `v0.1.1` and `v0.2.0` exist, and the
header promises UTC dates that no entry carries. Recommended: the section becomes `## 0.1.0`,
and either each release line carries its date or the header stops promising one. Two-way.
Effort XS.

### P2

#### Item 14: README shape per the template's report-only checks

The README states the minimum
Rust version as a bare line where the template's section order wants `## Minimum Rust
version`; `## EXAMPLES` and `## What is proven` come after sections of the crate's own; the
crate doc's first sentence uses `TRNG` before expanding it. The README also
never says the crate has no dependencies and no `unsafe`, which the stranger run inferred and
could not confirm from the page. These are report-only today and may become required.
Two-way. Effort XS.

#### Item 15: Two named bits are private

`INTERRUPT_STATUS_AGE_ALARM` and
`INTERRUPT_STATUS_REQUEST_ALARM` are `pub(crate)`, yet both sit in
the public `INTERRUPT_STATUS_DOCUMENTED_BITS_MASK` and the mask's doc comment tells a driver to
report an unexplained residue, which a driver cannot name for bits 2 and 3. The doc comment
on the mask argues that a bit with no name leaves a status word undecodable.
Recommended: make both `pub`. Additive, two-way. Effort XS.

#### Item 16: Decode the read-only build registers and the reseed-state field

NetBSD names
`FEATURES` bits (`MM_RESET_STATE` 3, `RAND_SEED_AVAIL` 2, `MAX_RAND_LENGTH` 1:0) and `BCONF`
bits (`AUTO_RESEED_LOOPBACK` 5, `MODE_AFTER_RST` 4, `PRNG_LEN_AFTER_RST` 3, `MAX_PRNG_LEN` 2);
Linux names the `STAT.LAST_RESEED` field at bit 16, whose value 7 the crate cites in prose. The crate names the two register offsets and
no field, so a bench dump of them is undecodable with this crate alone, and exactly these fields answer item 4's width question. `SEED_1..6_REGISTER` are also absent while
`RANDOM_NUMBER_0..7` are all present. Additive; the new names are provisional and go to the
name review. Two-way. Effort S.

#### Item 17: A rand_core adapter behind a feature. Declined

A user of the embedded Rust ecosystem may
reach for `rand_core::TryRng` (0.10) and find nothing here to implement it. There is no
`rand_core` feature in 0.3.0. nife reaches the interface through `getrandom` at the contract
layer, and an optional feature nobody uses is API without evidence. Revisit if an outside user
asks or the contract layer needs the interface. Two-way (additive later). No effort.

#### Item 18: Fuzz, or say why not

The template's `fuzz.yml` caller is absent. The crate parses two
byte strings (`interrupt_line`, `is_status_okay`), both trivial, and holds no `unsafe`, so
public crash inputs fit the opt-in rule. Either add the caller with one
target over both decoders or record in `BUGS` that the crate has nothing worth fuzzing.
Two-way. Effort XS.

#### Item 19: State why `rust-version` is 1.88

The source needs edition 2024 (1.85) and
`Option::is_none_or` (1.82); nothing read asks for 1.88. Either lower to 1.85 and let the
`msrv` job prove it, or add a comment saying what needs 1.88. Not checked with a 1.85
toolchain. Two-way. Effort XS.

## Decisions for the architect

No one-way door remains open except the publication steps. Those are the end of the publishing
hold for this crate (item 1) and the first-publish token with the `crates-io` environment approval
(item 3); both wait for the hold to lift. The items record the rulings: the first
published version (item 1), the width outcome `WrongOutputWidth` (item 6), the `Pool` changes
(item 8), the names (item 9) and the declined `rand_core` feature (item 17).

Everything else is the maintainers' provisional call.

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

- Item 4's bench session ran at crate commit `ee0bde8`. The release commit is a later one, and
  the README's "Hardware evidence" section names `ee0bde8` only, so a release commit that changes
  `src/` needs another session before the section can name it.
- The prior-art rows marked *recalled* were not re-read; a lane that revisits item 17 reads the
  HAL crates it cites first.

## Index row

The release review of `jh7110_entropy`, the pilot crate. Four items block a publish: the hold, the manifest's refusal, the first-publish token and a run of this tree on the board. Nine land before the first release. They cover the template pin and mutation workflow, a decoder that refuses a 128-bit answer and an in-flight generation, a `Pool` with no copy, no print of its bytes and proofs, package contents, metadata and changelog order. Five follow, and the sixth, a `rand_core` feature, is declined. The first crates.io version is 0.3.0, and no 0.2.0 release run is pending.
