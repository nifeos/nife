---
status: NOT-STARTED
promoted_from: jh7110-entropy-release-review
raised: 2026-11-09
milestone_dependencies: 818, 1017
decision_dependencies: 247, 305
machine_requirements: riscv64 silicon with the JH7110 TRNG (a VisionFive 2)
specific_machine: radon
needs_person: yes
---
# 1049. `jh7110_entropy` is ready to publish: the release review's findings

Raised by the review lane for milestone 1017's step 0. The
lane read `nifeos/jh7110_entropy` at `98c211e` as an outside maintainer would meet it, ran what it
could on a host, and wrote this block. It changed nothing in any repository.

Reuse: the crate's own Kani harnesses and falsification patches, the shared template
`nifeos/crate-workflows` and its `scaffold/`, cargo-mutants, cargo-semver-checks, and the review
packet of milestone 1017. No new tool.

## Goal

A stranger who adds `jh7110_entropy` to a bare-metal RISC-V kernel gets a register map and a
decision function that are right, cannot be misread into serving bytes the hardware never
generated, and arrive with the metadata, the CI and the README a crates.io crate is expected to
carry. The items below say what stands between the repository as it is and that.

## What was verified firsthand

Every claim in this section was run or read by the lane on a host; nothing here ran on a board.

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
- The template pin `d332215` is on `nifeos/crate-workflows` `main`, 57 commits behind it. Since the
  pin the template moved the full mutation run into its own `mutation.yml`, made
  `check / mutation-diff` a required check on every crate repository's `main`, and made the
  release gate refuse a release without a green full mutation run at the release commit.
- The newest weekly run (`42658233510`) failed in `weekly / mutation`: the verdict step found
  `0 of 1 shards left an outcomes.json` after the shard itself passed in 55 seconds. The weekly
  run before it was green. The defect is in the pinned template's artifact handling, not in the crate.
- Running the current template's helpers over the checkout: the seven naming checks refuse 0 of 162
  names; `standalone` passes; the report-only checks list three README section-order findings, one
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

Each item carries the finding, its evidence, the recommended change, which door it is (a one-way
door is a public API shape, a name, a register layout or the license; everything else is two-way),
and an effort size (XS under an hour, S a few hours, M a day or two). P0 blocks any publish, P1
lands before the first release, P2 can follow it.

### P0

1. **The publishing hold.** A standing ruling holds every crates.io publish from this organization
   until several crates have left and a review has decided whether any of them combine. No work in
   this repository changes that; the block records it so nobody reads a green gate as permission.
   The first version published to crates.io is 0.3.0 (see *The first published version*). No
   release run is held for 0.2.0: all six release runs of the repository are finished, and
   crates.io has no such crate. The tag `v0.2.0` stays as git history. This does not lift the
   hold. One-way (a published version is permanent). Ruled. Effort none.
2. **The manifest refuses to publish, by design.** `Cargo.toml:12` carries `publish = false` and
   `.github/workflows/ci.yml:21-22` passes `allow-unpublished: true`. Both come out in the release
   pull request, and the comment at `Cargo.toml:12` ("release step 7 removes this line") is
   rewritten to say what a reader of this repository can act on: the line comes out when the
   crate publishes. Two-way. Effort XS.
3. **The first publish needs a token.** crates.io binds a trusted publisher only to a crate that
   exists, so the template's release path uses the `crates-io` environment secret
   `CRATES_IO_FIRST_PUBLISH_TOKEN` for a crate it cannot find, and trusted publishing afterwards.
   A person creates that token (scoped to this one name, short expiry), approves the environment,
   then binds the trusted publisher and deletes the token. The token and the `crates-io`
   environment approval wait for the publishing hold to lift. Effort S.
4. **The published tree has not run on the board.** `README.md:102-104` says an earlier version
   under another name produced output on a VisionFive 2 and this exact tree has not. The crate's
   one claim is the board. Before the first publish, one bench session builds a driver on the
   release commit and records the checks listed under *Done when, on hardware*; the README's
   "Hardware evidence" section then names the version that ran. This cannot be done without the
   bench. Two-way. Effort M, bench time. The maintainers may move this to P1 if they choose to
   publish with the caveat as written; the stricter reading is recommended.

### P1

5. **Bump the template pin and adopt what the template now expects.** `ci.yml:17` and the five
   sibling callers pin `d332215`, 57 commits behind. At the current template the full mutation
   run lives in a caller `.github/workflows/mutation.yml` with the repository's own cron line
   (`python3 helpers/mutation_plan.py schedule jh7110_entropy` in a template checkout prints it),
   `check / mutation-diff` is a required check that `script/apply-settings` adds to the ruleset,
   `.github/ci-job-budgets` gains `mutation-diff 30` with its reason, and the release gate
   refuses a release with no green full mutation run at that commit. The red weekly run
   (`42658233510`) is the pinned template's defect and goes away with the bump. Two-way. Effort S.
6. **The decode ignores the output width.** `generation_outcome` (`src/lib.rs:407-421`) answers
   `Ready([u8; 32])` whenever `RAND_RDY` is set on a seeded core, and never looks at
   `STATUS_256_BIT_OUTPUT` (`src/lib.rs:210`), although `status` is in hand and the doc comments
   at `src/lib.rs:69-79` and `189-196` say that in 128-bit mode only `RANDOM_NUMBER_0..3` carry the
   answer. The crate warns in prose and relies on the caller to have written `MODE_REGISTER`; the
   ladder prefers a check the type system or the decoder performs. Ruled: a fourth, refusing
   outcome named `WrongOutputWidth` answers a snapshot taken in 128-bit mode, that is when
   `STATUS_256_BIT_OUTPUT` is clear. It is proven by a harness with a falsification patch, and the
   README's bring-up paragraph points at it. Adding a variant to an enum that is not
   `#[non_exhaustive]` breaks exhaustive matches, so one-way. Effort S.
7. **The decode ignores a generation or reseed in flight.** `STATUS_GENERATING` and
   `STATUS_RESEEDING` (`src/lib.rs:220-230`) exist as constants, and `generation_outcome` does not
   read them. A snapshot with `RAND_RDY` latched while `STAT` reports a generation in flight is
   answered `Ready`, and `RANDOM_NUMBER_0..7` may be mid-rewrite at that instant. Linux's driver
   polls both bits clear before issuing a command, which is why it never meets this; a pure decoder
   can return `NotReady` while either is set, which costs a caller one more poll and nothing else.
   Whether the hardware can show `RAND_RDY` and `RAND_GENERATING` together is a bench question
   (below). Recommended: fold the two bits into the `NotReady` arm, extend
   `neither_bit_set_is_always_not_ready` or add a harness, and add the falsification patch. The
   function's signature does not change; its answers for some inputs do. Two-way. Effort S.
8. **`Pool` can be copied and printed.** `src/lib.rs:470` derives `Clone, Copy, Debug` on the
   struct that holds the 32 bytes. A copy serves the same bytes twice, which is the one property
   `no_byte_is_served_twice_across_refills` (`src/lib.rs:706`) and the README promise; `Debug`
   prints the buffer, so a diagnostic line can carry the bytes into a log. `refill`
   (`src/lib.rs:503-513`) also overwrites up to 31 unserved bytes without zeroing first, and
   nothing zeroes the buffer on drop. Ruled: `Pool` loses `Clone` and `Copy`, gets a hand-written
   `Debug` that prints only `remaining`, and gets a `Drop` that zeroes the buffer. Removing a trait
   implementation is breaking, so one-way. Effort XS.
9. **`Pool`'s names and answer shape at the release review.** `Pool::remaining`
   (`src/lib.rs:497`) returns a count of bytes and the units rule says a length carries its unit;
   `Pool::take`'s parameter `n` (`src/lib.rs:524`) is a byte count with a one-letter name; `take`
   answers a `(u64, u64)` whose order (count, then word) a caller must remember. The earlier
   names predate the units rule, and the release review reopens them. Ratified: `Pool::remaining_bytes`;
   `take`'s parameter `byte_count`; the return becomes the struct
   `TakenBytes { byte_count: u64, word: u64 }`; the private field `buf` becomes `buffer`. One-way
   (names, API). Effort XS.
10. **`Pool` has no proof.** `README.md:52-53` says `Pool` has unit tests and no harness. Its
    invariants are small enough for Kani: for any `n: u64` and any generator that answers or
    refuses, `take` returns a count of at most `min(n, 8)`, serves the generator's bytes in order
    across one refill seam, zeroes what it served, never panics, and answers a short count rather
    than zeros when the generator refuses. The buffer is 32 bytes and the gather loop is bounded by
    8, so the harnesses stay cheap. Each one carries a falsification patch. Two-way. Effort M.
11. **The package ships the repository's plumbing.** `cargo package --list` includes `.github/`
    (workflows, templates, budgets, exemptions), `.githooks/`, `.cargo/mutants.toml`,
    `docs/stranger-test*.md`, `_typos.toml`, `deny.toml`, `rustfmt.toml` and `CODEOWNERS`. A
    user of the crate needs `src/`, `falsifications/` (the release policy ships them), the README,
    the changelog, the two license files, `Cargo.toml` and
    `docs/the-crate-carries-facts-not-source-text.md`. Recommended: an `include` list naming
    those. Two-way. Effort XS.
12. **Manifest metadata for crates.io.** `Cargo.toml:10-11` has `keywords = ["no-std", "kani",
    "driver"]` and no `documentation` field. Recommended: `documentation =
    "https://docs.rs/jh7110_entropy"`, keywords a searcher types (`jh7110`, `trng`, `starfive`,
    `no-std`, `random`; crates.io allows five), and the category `no-std::no-alloc` beside the
    three present (the slug exists). Metadata can change in a later version, so two-way, but the
    first version's page is what a search engine indexes first. Effort XS.
13. **The changelog's order.** `CHANGELOG.md:27-34` has an `## Unreleased` section holding "First
    version" below `## 0.1.1`, while the tags `v0.1.0`, `v0.1.1` and `v0.2.0` exist, and the
    header promises UTC dates that no entry carries. Recommended: the section becomes `## 0.1.0`,
    and either each release line carries its date or the header stops promising one. Two-way.
    Effort XS.

### P2

14. **README shape per the template's report-only checks.** `README.md:113` states the minimum
    Rust version as a bare line where the template's section order wants `## Minimum Rust
    version`; `## EXAMPLES` and `## What is proven` come after sections of the crate's own; the
    crate doc's first sentence (`src/lib.rs:2`) uses `TRNG` before expanding it. The README also
    never says the crate has no dependencies and no `unsafe`, which the stranger run inferred and
    could not confirm from the page. These are report-only today and may become required.
    Two-way. Effort XS.
15. **Two named bits are private.** `INTERRUPT_STATUS_AGE_ALARM` and
    `INTERRUPT_STATUS_REQUEST_ALARM` (`src/lib.rs:270`, `274`) are `pub(crate)`, yet both sit in
    the public `INTERRUPT_STATUS_DOCUMENTED_BITS_MASK` and the mask's doc comment tells a driver to
    report an unexplained residue, which a driver cannot name for bits 2 and 3. The doc comment
    at `src/lib.rs:267-268` argues that a bit with no name leaves a status word undecodable.
    Recommended: make both `pub`. Additive, two-way. Effort XS.
16. **Decode the read-only build registers and the reseed-state field.** NetBSD names
    `FEATURES` bits (`MM_RESET_STATE` 3, `RAND_SEED_AVAIL` 2, `MAX_RAND_LENGTH` 1:0) and `BCONF`
    bits (`AUTO_RESEED_LOOPBACK` 5, `MODE_AFTER_RST` 4, `PRNG_LEN_AFTER_RST` 3, `MAX_PRNG_LEN` 2);
    Linux names the `STAT.LAST_RESEED` field at bit 16, whose value 7 the crate cites in prose
    (`src/lib.rs:394-395`). The crate names the two register offsets (`src/lib.rs:102`, `161`) and
    no field, so a bench dump of them is undecodable with this crate alone, and item 4's width
    question is answered by exactly these fields. `SEED_1..6_REGISTER` are also absent while
    `RANDOM_NUMBER_0..7` are all present. Additive; the new names are provisional and go to the
    name review. Two-way. Effort S.
17. **A rand_core adapter behind a feature. Declined.** A user of the embedded Rust ecosystem may
    reach for `rand_core::TryRng` (0.10) and find nothing here to implement it. There is no
    `rand_core` feature in 0.3.0. nife reaches the interface through `getrandom` at the contract
    layer, and an optional feature nobody uses is API without evidence. Revisit if an outside user
    asks or the contract layer needs the interface. Two-way (additive later). No effort.
18. **Fuzz, or say why not.** The template's `fuzz.yml` caller is absent. The crate parses two
    byte strings (`interrupt_line`, `is_status_okay`), both trivial, and holds no `unsafe`, so
    public crash inputs would be allowed under the opt-in rule. Either add the caller with one
    target over both decoders or record in `BUGS` that the crate has nothing worth fuzzing.
    Two-way. Effort XS.
19. **State why `rust-version` is 1.88.** The source needs edition 2024 (1.85) and
    `Option::is_none_or` (1.82); nothing read asks for 1.88. Either lower to 1.85 and let the
    `msrv` job prove it, or add a comment saying what needs 1.88. Not checked with a 1.85
    toolchain. Two-way. Effort XS.

## Decisions for the architect

No one-way door remains open except the publication steps. Those are the end of the publishing
hold for this crate (item 1) and the first-publish token with the `crates-io` environment approval
(item 3); both wait for the hold to lift. The rulings are recorded in the items: the first
published version (item 1), the width outcome `WrongOutputWidth` (item 6), the `Pool` changes
(item 8), the names (item 9) and the declined `rand_core` feature (item 17).

Everything else is the maintainers' provisional call.

## The first published version

The tag `v0.2.0` exists at `cfdbeb7` and stays as git history, and `main` has no API change since
it. No release run is held for it: all six release runs are finished, and crates.io has no such
crate. No item above can join 0.2.0, because the release workflow refuses a tag that exists, so
any change means a new version. Items 6, 8 and 9 change the public API, and item 7 changes what
the decoder answers, which makes the next version 0.3.0 under the rule that a break bumps the
minor before 1.0. Items 5 and 10 to 19 alone would make 0.2.1.

Ruled: 0.3.0 is the first version published to crates.io. Items 5 to 13 land on `main`, and 0.3.0
is tagged through the release workflow when the hold lifts. Publishing 0.2.0 and breaking it in
the next release would give the registry a permanent version whose `Pool` is `Copy` and whose
decoder cannot refuse a 128-bit answer.

## Prior art

Rows marked *read* were read in the cited tree during this review; rows marked *recalled* are from
memory and not re-read.

| system | what it does that this crate should know | how |
|---|---|---|
| Linux `jh7110-trng.c` (*read*) | Interrupt-driven. `wait_idle` polls `STAT` for `RAND_GENERATING` and `RAND_SEEDING` clear with a 100 ms timeout before any command; a generate waits up to 1 ms (40 µs when the caller will not wait); the interrupt handler acknowledges each `ISTAT` bit by writing it back and issues a reseed on `LFSR_LOCKUP`; init writes `AUTO_AGE` and `AUTO_RQSTS` (default 0), clears `ISTAT`, enables all interrupts, sets `MODE.R256`, and reseeds. The read path clamps to 16 bytes in 128-bit mode and 32 in 256-bit mode. It declares no `quality` to the hwrng core. | items 6, 7; the bench timing checks |
| NetBSD `jh7110_trng.c` (*read*) | Polling. Reads `RAND_RDY` only when `STAT.SEEDED` is set and reseeds otherwise; acknowledges `RAND_RDY` before reading the eight words; `explicit_memset`s the words after handing them to the entropy pool; init masks interrupts and writes `~0` to `ISTAT`; names the `FEATURES` and `BCONF` field bits and the `RQST_LOCKUP` interrupt bit the crate calls the request alarm. Feeds the words to `rnd_add_data_sync` claiming full entropy. | items 8, 16 |
| Linux hwrng core and rng-tools (*recalled*) | The kernel mixes hardware output into its pool through a conditioning function and trusts a per-mille `quality` the driver declares; the FIPS 140-2 continuous tests run in userspace `rngd`, not in the driver. | the health-test question stays open, as the crate says |
| NIST SP 800-90B (*recalled*) | The repetition count test and the adaptive proportion test are the two continuous health tests a noise source is expected to carry; neither the manual nor any cited driver claims one for this block. | the open question in `src/lib.rs:279-281` |
| rand_core 0.10.1 (*read*) | `TryRng` with `type Error: core::error::Error`, `try_next_u32`, `try_next_u64`, `try_fill_bytes`; `Rng` for the infallible case; `CryptoRng` and `TryCryptoRng` as marker traits; `RngCore` and `TryRngCore` kept for compatibility. An embedded driver crate commonly implements `TryRng` behind a feature. | item 17 (declined) |
| getrandom (*read*, README) | A bare-metal system routes `getrandom` to its own source through the opt-in `custom` backend (`getrandom_backend` configuration flag); a library cannot do this for its users. | nothing for the crate to do; the README can say so |
| esp-hal, embassy-stm32, atsamd-hal, nrf-hal (*recalled*) | Each SoC HAL exposes its TRNG as a type implementing rand_core's trait, blocking or fallible; embassy-stm32's RNG recovers from the hardware's seed and clock error bits and does not serve bytes past them. | items 6, 7, 17 |
| embedded-hal 1.0 (*recalled*) | Carries no RNG trait; the ecosystem converged on rand_core. | item 17 |
| Tock `Entropy32` and Zephyr `entropy` (*recalled*) | Both define an OS-side entropy interface that a driver fills; neither is reachable from a Rust crate without the OS. | none |
| `jh71xx-pac`, `jh71xx-hal` (Codeberg, GPL-3.0-only; from milestone 818) | Register maps for the JH7110 family with no TRNG logic, under a license this crate cannot take. | none; this crate stays the only Rust TRNG logic for the chip found |

What a user would miss today: a `TryRng` implementation (item 17, declined), a decoded answer to "what width
is this block in" (items 6, 16), and a measured poll bound (the bench). What is unique: the
decision function is exhausted by Kani with replayable falsifications, the seeded gate and the
lockup precedence are stated as proofs rather than comments, and both device-tree spellings travel
with the register map.

## Done when

On a host, from the repository alone:

- Items 2 (prepared in the release pull request), 5 to 13 are merged, and `cargo semver-checks
  --baseline-rev v0.2.0` reports the break that makes the next version 0.3.0.
- `cargo kani` verifies every harness, the new ones for items 6, 7 and 10 included, and every
  falsification patch under `falsifications/` fails its own harness and no other.
- cargo-mutants reports no survivor outside `.github/mutants-exempts`.
- `cargo package --list` shows only the files item 11 names, and `cargo publish --dry-run` passes
  once `publish = false` is out.
- The template's seven naming checks and `standalone` pass at the bumped pin, and the report-only
  README checks list nothing.

In CI, on `main` at the release commit: `check /` (seven jobs, `mutation-diff` included), `proofs /
kani`, `proofs / falsify` and `mutation / mutation` green; `weekly` green on its next schedule; a
release dry run green.

On hardware, with a VisionFive 2 on the bench; none of these can be done here and none is claimed:

- A driver built on the release commit, after the platform ungates the clocks and releases the
  reset, completes a reseed (`SEED_DONE`) and a generation (`RAND_RDY`) by polling, and the 32
  bytes are not all zero and differ between two generations.
- `STATUS_256_BIT_OUTPUT` reads 1 after `MODE_256_BIT_OUTPUT` is written, and the value it reads
  before that write, with `FEATURES` and `BUILD_CONFIGURATION` dumped, answers what the silicon
  defaults to (item 6's "cannot be assumed either way").
- Whether `RAND_RDY` and `RAND_GENERATING` are ever observed set in one snapshot after a second
  `CONTROL_COMMAND_GENERATE` (item 7).
- `STATUS_SEEDED` and the `LAST_RESEED` field before the first reseed after reset.
- The time from `CONTROL_COMMAND_GENERATE` to `RAND_RDY` and from `CONTROL_COMMAND_RESEED` to
  `SEED_DONE`, so the README can give a measured poll bound instead of none.
- The residue in `INTERRUPT_STATUS_REGISTER` after writing
  `INTERRUPT_STATUS_DOCUMENTED_BITS_MASK`, and whether bits 2 and 3 latch when `AUTO_AGE` or
  `AUTO_RQSTS` is nonzero.
- Whether `LFSR_LOCKUP` is observed in a long run, and that a reseed clears it.
- A capture of at least one mebibyte run through a statistical suite, recorded as a measurement
  beside the README's "quality unmeasured" line, not as a proof.

## BUGS

- Item 4 needs bench time the lane cannot schedule, and the README's "Hardware evidence" section
  stays as written until it happens.
- The prior-art rows marked *recalled* were not re-read; a lane that revisits item 17 reads the
  HAL crates it cites first.

## Index row

The release review of `jh7110_entropy`, the pilot crate: four items block a publish (the hold,
the manifest's refusal, the first-publish token, a run of this tree on the board), nine land
before the first release (the template pin and mutation workflow, a decoder that refuses a
128-bit answer and an in-flight generation, a `Pool` that cannot be copied or printed and has
proofs, package contents, metadata, changelog order), and five follow; the sixth, a `rand_core`
feature, is declined. The first crates.io version is 0.3.0; no 0.2.0 release run is held.
