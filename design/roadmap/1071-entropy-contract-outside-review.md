---
status: NOT-STARTED
raised: 2026-11-11
milestone_dependencies: 1017
decision_dependencies: 137
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1071. `entropy_contract`: what an outside maintainer would fix before 0.1.0

Raised by a read-only review of `crates/entropy_contract` as a stranger to this tree would meet it,
made before the crate's move to `nifeos/entropy_contract`. It changes nothing; it lists what the
next lanes change, in nife before the move where the runbook puts a finding against the crate
itself, and in the new repository after it.

The review took its accepted baseline from
[the review package](../../notes/crate-reviews/entropy-contract.md) and raises none of the
decisions it settles: the crate name, the identity (`INTERFACE_ID` 12, `VERSION` 1, no magic), the
thirteen names ratified on #3480 and the six earlier ones, `#![forbid(unsafe_code)]`, the four
harnesses, the dual license, `publish = false` through the soak, and the move itself.
No later rule in [design/naming.md](../naming.md) contradicts any ratified name: every quantity ends
in its unit, every abbreviation the rules list is spelled out, and `fill_with` is std's shape.

Reuse: the template `nifeos/crate-workflows` and its scaffold, Kani, cargo-mutants, the crate's own
harnesses. No new tool.

## Goal

A stranger who finds `entropy_contract` can tell in one screen what the two words on the wire mean,
can follow the README alone to a build, the tests, the proofs and one falsification, and meets no
sentence in the README or the documentation that the code contradicts. The request builder does
what its documentation says for every byte count. The repository's `check` and `proofs` runs are
green on the first push for the crate's own reasons.

## What was checked, and how

Verified firsthand, on a fresh shallow clone of `nifeos/nife` at `cf1328af` (the head of `main`
after #3480 merged), with the crate copied out of the workspace into a directory of its own:

- `cargo test` on stable and on 1.88, the manifest's `rust-version`: 15 unit tests and 4
  documentation tests pass, one of them the README's example.
- `cargo clippy --all-targets -- -D warnings` and `cargo doc` with `-D warnings -D
  rustdoc::broken_intra_doc_links -D rustdoc::invalid_html_tags`: clean.
- The library builds for `x86_64-unknown-none`, `aarch64-unknown-none-softfloat` and
  `riscv64imac-unknown-none-elf` on the tree's pinned nightly, and for `x86_64-unknown-none` on
  1.88.
- Kani 0.68.0 (CBMC 6.11.0): `Complete - 4 successfully verified harnesses, 0 failures, 4 total.`
- `cargo package --list`: 12 files, the four falsification patches among them; Cargo warns that the
  manifest has no `documentation`, `homepage` or `repository`.
- The template's helpers at its head (`2e651d8`), run over the crate: `spelled_out`,
  `unit_symbols`, `register_names`, `question_shape` and `abbreviations` report nothing;
  `readme_sections`, `readme_acronyms`, `inflated_wording`, `prose_figures`, `history_phrases`,
  `file_length`, `function_length`, `comment_blocks` and `orphan_modules` report nothing.
  `passive_voice` fails the README (item 3). `readme_badges` reports no badge row (item 4).
  `prose_caps` passes the README and fails `src/lib.rs`, which the template does not gate (item 16).
  `falsify.py --check` reports every harness as "replayable with no patch path" (item 13).
  `standalone.py` reports `publish = false`, 20 provenance lines and 2 lines naming the architect
  (items 8 and 14).
- Every caller in the tree (`kernel`, `components`, `crates/user_mode_runtime`,
  `crates/system_initializer`, `fixtures`, `system_tests`, the standard library overlay at
  `patches/std-nife/overlay/std/src/sys/random/nife.rs`), read for how it builds requests, reads
  replies and composes them.
- The recorded mutation figures: 40 mutants, 37 caught, 3 survivors, all three triaged as
  equivalent in `notes/project-metrics/mutation-triage.csv`, with reasons that hold on reading.
- The arithmetic claims of items 2, 11 and 17, by a throwaway test against the crate (not kept).
- Download figures for the prior-art crates, from the crates.io API.

Inferred from reading, not run: the dry run `script/crate-move --dry-run` and the template's
workflows on a staged tree (items 3, 4, 8, 13 and 14 say what those would report, from the
helpers run by hand). No mutation census was run; the figures above are the tree's records.

Not run: the stranger test, `cargo semver-checks` (no previous release), Miri, a fuzz run (the
crate has no target), `script/crate-standalone` and `script/names` (the package ran both over the
same names, and no public name has changed since).

## Prior art

How the contract sits against what a reader already knows. Recent downloads are the last 90 days
at the time of the review.

| system | request | reply | short answer | "no source" | health check at start |
|---|---|---|---|---|---|
| `getrandom` 0.4 (791.8M recent / 2.57G all) | `fill(&mut [u8]) -> Result<(), Error>` | the buffer | never: loops inside | `Error` with a code | none |
| `rand_core` 0.10 (617.4M / 2.17G) | `fill_bytes`, `try_fill_bytes -> Result<(), Error>` | the buffer | never | `Error` | none |
| Linux `getrandom(2)` | buffer and length | byte count | up to 256 bytes never short once the pool is ready (unverified wording) | `errno` | the pool blocks until seeded |
| OpenBSD `getentropy(2)` | buffer up to 256 bytes | fills all | never | `EIO` above 256 bytes | none |
| Zephyr `entropy_get_entropy` | buffer and length | `0` or `-errno` | never | `-errno` | none |
| virtio-rng | a buffer in a queue | bytes written | the device may fill fewer (unverified wording) | no device | none |
| Linux `x86_init_rdrand` | eight `RDRAND` draws at boot | | | | refuses when consecutive draws repeat and disables the instruction (unverified detail) |
| FIPS 140-2 4.9.2, SP 800-90B 4.4.1 | | | | | compare each block with the last; a repeat is a failure |

Where this crate differs: the unit is one `CALL` of one request word and two reply words, so a
reply carries at most eight bytes and every caller loops. A count and a failure are told apart by
magnitude alone, not by an error type, which the kernel's small negative error numbers make
possible. The caller supplies the `CALL`, so the crate has no dependency and no system call. It
carries a bring-up rule no listed system states: a first draw of all zeros is a dead device. It has
no error enumeration; `fill_with` answers `Option<()>`. What it costs: a hidden truncation in the
request builder (item 2), two failures folded into one `None` (item 5), and a bring-up rule that
passes a source stuck on any nonzero value (item 17), the failure `x86_init_rdrand` was written
for.

## The items

Effort: XS is under an hour for a lane, S half a day, M a day or two, L longer. A door is one-way
when somebody outside the repository acts on it after a release (a public name, a published
behavior, a wire value, a layout, a dependency); everything else is two-way and the maintainer
decides it. Line numbers are in `crates/entropy_contract/` at `cf1328af`.

### P0: blocks any publish

1. The README denies the crate's proofs.
   Finding: `## What is proven` reads "Nothing is machine-checked: there are no Kani harnesses."
   `src/lib.rs` carries four harnesses with four falsification patches, and the review package
   describes all four. #3480 added the harnesses and left this section as it was.
   Evidence: `README.md:93-96`; `src/lib.rs:487-593`; `falsifications/`.
   Change: one line per property in a user's words (a request word reads back as built, a copy
   writes only the delivered bytes and nothing past them, a fill succeeds exactly when every reply
   is full, readiness is `READY` exactly when a first byte is nonzero), with the bounds each proof
   runs to (a slice of up to 17 bytes, a buffer of 12, a draw of 4) and the words "every harness
   carries a falsification patch".
   Door: two-way. Effort: XS.

2. `request` truncates a byte count instead of clamping it, and the README's example trips on it.
   Finding: `request(operation, byte_count)` keeps `byte_count & 0xff`, so a request for 256 bytes
   is a request for zero, a request for 257 is a request for one, and a request for 512 is a request
   for zero. `requested_bytes` clamps only what survives the mask. The documentation of `GET`
   ("clamped to `MAXIMUM_BYTES` by `requested_bytes`, so a client that asks for more is answered
   rather than refused"), of `requested_bytes` ("a request for 500 bytes is a request for eight")
   and the README ("asking for more than `MAXIMUM_BYTES` is answered with `MAXIMUM_BYTES`") all say
   otherwise. The README's example and the `copy_delivered_bytes` example pass the unclamped
   remainder, `(key.len() - filled) as u64`; with a 256-byte key, the first trip asks for zero
   bytes, the service answers zero, and the example's own `assert_ne!` panics with "a zero count is
   `NO_ENTROPY`", blaming the device. Every caller in the tree clamps before it calls `request`
   (`fill_with`, the standard library overlay, `system_initializer`, the kernel's test client), so no
   program is affected today; a stranger following the README is. The Kani harness proves the
   truncating behavior, `(byte_count & 0xff).min(MAXIMUM_BYTES)`, which is why it did not catch the
   gap between the code and its documentation.
   Evidence: `src/lib.rs:69-71` (the builder), `59-61` and `81-83` (the claims), `248-258` and
   `README.md:79-86` (the examples), `src/lib.rs:502-505` (the harness); verified by a throwaway
   test.
   Change: `request` clamps, `(operation << OPERATION_SHIFT) | byte_count.min(MAXIMUM_BYTES)`, so
   the wire carries at most eight in bits 7:0 and `requested_bytes` keeps its clamp for a request
   another builder made. The harness asserts `byte_count.min(MAXIMUM_BYTES)`, and the patch that
   falsifies it (the mask forgotten) is replaced by one that forgets the clamp. The two examples keep
   their unclamped remainder, since the builder now makes that correct, and a unit test asks for
   256 and reads eight back.
   Door: two-way. The wire format does not change; a builder's output for counts above 255 does.
   Effort: XS.

### P1: before the first release

3. The README fails the template's passive-voice gate.
   Finding: `passive_voice.py` counts 6 passive constructions in 536 words and allows 1; `check /
   passive voice and history phrases` runs it over `README*` and fails the job. The six: "is
   transformed" (line 8), "be given" (18), "be read" (21), "is answered" (23), "are covered" (62),
   "is read" (74). The dry run `script/crate-move --dry-run` runs the same check on the staged tree
   and refuses to finish (inferred from the runbook; the dry run was not run).
   Evidence: `README.md:8, 18, 21, 23, 62, 74`; `crate-workflows/.github/workflows/check.yml:254-266`.
   Change: rewrite five of the six in the active voice, in nife before the move. Item 1 rewrites
   line 62 anyway.
   Door: two-way. Effort: XS.

4. The README has no badge row and no path from a clone to a proof.
   Finding: `readme_badges.py` reports three findings; `check / README badge row` is a required
   check and a finding fails it. `## Building and testing` gives `cargo test` and one bare-metal
   build line, and nothing the scaffold's `README-crate.md` asks for: `rustup target add` before
   the first bare-metal build, the Kani install line (`cargo install --locked kani-verifier
   --version <.kani-version>` then `cargo kani setup`, with the note that setup needs the network),
   how long `cargo kani` takes and its peak memory, how to run one harness (`cargo kani --harness
   verification::<name> --exact`), how to apply one falsification patch and undo it, what a passing
   run prints that looks alarming, and how to check the minimum Rust version. The stranger test
   reads the README alone; the release gate refuses while no stranger run is on record.
   Evidence: `README.md:98-103`; `crate-workflows/scaffold/README-crate.md`;
   `crate-workflows/.github/workflows/check.yml:336-347`.
   Change: in nife before the move, add the badge row under the title with the repository's name,
   and the two subsections `### Running the proofs` and `### Running one falsification patch` with
   the crate's own commands and one real patch as the example (the readiness patch is the
   shortest). Measure the Kani time and memory with `/usr/bin/time -v cargo kani` and write the
   numbers.
   Door: two-way. Effort: S.

5. `fill_with` folds two failures the contract takes care to tell apart.
   Finding: `delivered_bytes` separates "no service answered" (a word above `MAXIMUM_BYTES`) from
   "the service answered with nothing" (`NO_ENTROPY`), and its documentation calls that the property
   the whole error story rests on. `fill_with` returns `Option<()>` and answers `None` for both,
   and for a short reply besides. The standard library overlay distinguishes the first two with two
   different panic messages because they call for different fixes (a missing grant against a dry
   device); `user_mode_runtime::fill` passes the `None` on and its callers cannot. A stranger from
   `getrandom` or `rand_core` expects a `Result` with an error that says which.
   Evidence: `src/lib.rs:272-293`; `src/lib.rs:189-196`;
   `patches/std-nife/overlay/std/src/sys/random/nife.rs:67-82`;
   `crates/user_mode_runtime/src/entropy.rs:17-30`.
   Change: `fill_with(out, call) -> Result<(), Refusal>` with a small `enum Refusal { NoService,
   ShortReply { asked: usize, delivered: usize } }` (both names provisional; the type is the crate's
   first), `#[non_exhaustive]` so a third refusal is not a break. `user_mode_runtime::fill` maps it
   or exposes it. The fill harness asserts `Err(NoService)` exactly when a reply was not a count and
   `Err(ShortReply)` exactly when one was short. The crate gains no dependency: the enum derives
   `Debug, Clone, Copy, PartialEq, Eq` only.
   Door: two-way now; a return type is one-way after the first publish, which is why this comes
   before it. Effort: S.

6. Three rules the services keep are not written in the contract.
   Finding: (a) Both services answer an opcode other than `GET` with `NO_ENTROPY`, and a system
   test pins that; the contract says nothing, so a client cannot tell a wrong opcode from a dry
   device and the README's "fail, do not retry" advice sends it to the wrong fix. (b) Bits 55:8 of
   the request word are undefined: `request` zeros them and `requested_bytes` masks them, but no
   sentence says a service ignores them, and `VERSION`'s promise that an addition owes no bump holds
   only if receivers ignore what they do not know. (c) The second reply word's bytes past the count
   are, per the documentation, "whatever the service had left in the register"; the virtio pool in
   fact zeros them and hands out no byte twice, and the instruction backend returns a fresh draw,
   but the contract leaves a service free to leak the next client's bytes.
   Evidence: `components/src/entropy.rs:386-397`, `components/src/jh7110_entropy.rs:268-273`,
   `system_tests/src/user/entropy_tests.rs:205-208`; `src/lib.rs:43-49` (`VERSION`);
   `src/lib.rs:229-231`; `components/src/entropy.rs:269-289`.
   Change: three sentences in the crate documentation and the README's contract list, recording what
   is: a service answers an unknown opcode with `NO_ENTROPY` (and a BUGS line that the two are
   indistinguishable to a client); bits 55:8 are reserved, a client sends zero and a service ignores
   them; a service puts nothing but zeros past the delivered bytes. The third also softens the
   `copy_delivered_bytes` sentence from "whatever the service had left" to "zero".
   Door: one-way once published, since a service outside this tree acts on them; today they record
   behavior every service already has. See the decisions section. Effort: XS.

7. The manifest's `publish` comment, categories and metadata.
   Finding: the `publish = false` line's comment sends a reader to `crates/publish-allowlist.txt`, a
   file the new repository does not have (the review package already lists this for the move lane).
   The template's `standalone` refuses `publish = false` unless the caller passes
   `allow-unpublished: true`, which the precedent repository `nifeos/jh7110_entropy` does. The
   `categories` list carries `cryptography`, which the crate is not (it holds no primitive and
   makes no claim about the bytes), and lacks `no-std::no-alloc`, which it is. `cargo package` warns
   that `documentation`, `homepage` and `repository` are absent; the move sets `repository`.
   Evidence: `Cargo.toml:9-11`; `crate-workflows/helpers/standalone.py:197-203`;
   `nifeos/jh7110_entropy` `Cargo.toml` and `.github/workflows/ci.yml` (read through the GitHub API).
   Change: now, a self-contained comment ("set to `true` only in the release that publishes");
   `categories = ["no-std", "no-std::no-alloc", "os"]`; `documentation =
   "https://docs.rs/entropy_contract"` beside the `repository` the move writes. At the move, the
   `ci.yml` caller passes `allow-unpublished: true`. At the release, delete the line.
   Door: two-way. Effort: XS.

8. Readiness passes a source stuck on any nonzero value.
   Finding: `readiness` is `READY` when any byte of the first draw is nonzero, so a draw of eight
   `0xff` bytes, or of any repeated byte, is `READY`. The rule was written for a gated clock that
   reads zeros, and it does that. A source that latches a nonzero value is the other known
   bring-up failure (the Linux `x86_init_rdrand` check and the FIPS 140-2 continuous test exist
   for it, and a processor erratum of this shape is on record; details unverified), and it passes.
   Decision 137 puts continuous health tests in the driver, which would catch it later; the tree's
   entropy note records that no such test exists yet. The bring-up word is the one place a bench
   reader looks.
   Evidence: `src/lib.rs:165-178`; `readiness([0xff; 8]) == READY`, verified by a throwaway test;
   `notes/entropy.md` BUGS ("No health test"); `design/decisions/0137-trng-health-tests.md`.
   Change: `readiness` names a third shared step when the draw holds at least `MAXIMUM_BYTES` bytes
   and every byte equals the first (all zero keeps `STEP_FIRST_ALL_ZERO`): `STEP_FIRST_ALL_SAME`
   (provisional), `0x12`. The harness widens to eight bytes and asserts the three steps and `READY`
   are exhaustive and exclusive; a patch that checks only the first two bytes falsifies it. The
   README's contract list and BUGS say so, and the BUGS probability line gains the new case (2^-56
   for an eight-byte draw from a uniform source).
   Door: one-way after a release, since the step is a wire value in the bring-up report. See the
   decisions section. Effort: S.

### P2: can follow later

9. A reply builder for the service side.
   Finding: the crate's claim is that the service, its clients and their tests share one
   definition; the client half does, and the three backends compose `(count, word)` by hand: the
   virtio pool packs bytes into the word by shifts, the instruction backend returns `(requested,
   u64::from_le_bytes(draw))`, the JH7110 pool packs its own. Item 6 (c) becomes enforceable only
   when one function makes the word.
   Evidence: `components/src/entropy.rs:269-289, 519-525`; `components/src/jh7110_entropy.rs:268-273`.
   Change: `reply(delivered: &[u8]) -> (u64, u64)` (provisional) clamps to `MAXIMUM_BYTES`, packs
   little-endian and zeros the rest, with a harness: `copy_delivered_bytes` of a `reply` yields the
   input, and the undelivered bytes read zero. The backends adopt it in their own pull requests.
   Door: two-way (an addition). Effort: S.

10. `#[must_use]` on the pure functions.
    Finding: only `bring_up_failure` carries it. `request`, `operation`, `requested_bytes` and
    `delivered_bytes` do not (clippy's `must_use_candidate`, run by hand, names lines 69, 77, 86 and
    218), and neither does `copy_delivered_bytes`, whose ignored count is the bug the function's
    documentation warns about.
    Evidence: `src/lib.rs:69, 77, 86, 218, 266`.
    Change: add the attribute to the five. Effort: XS. Door: two-way.

11. `request` and `bring_up_failure` accept values that do not fit their fields.
    Finding: `request(0x1ff, 1)` yields an opcode that reads back as `0xff`; `bring_up_failure(0x1_0000)`
    puts the step outside the low byte (both verified). The harness assumes the opcode fits, and
    the documentation says a step is `0x01..=0x11`.
    Evidence: `src/lib.rs:69-71, 135-137`.
    Change: `debug_assert!(operation <= 0xff)` and `debug_assert!(step <= 0xff)` (both legal in
    `const fn`), or masks; the documentation says which. Effort: XS. Door: two-way.

12. The readiness proof's bound is below every draw a backend makes.
    Finding: the harness proves draws of up to four bytes; the backends draw 8, 32 and 256, which the
    host tests cover for the all-zero case only.
    Evidence: `src/lib.rs:579-583` and `420-437`.
    Change: raise the bound to `MAXIMUM_BYTES` (unwind 10); item 8 needs that anyway. Effort: XS.

13. The `Falsification:` lines sit at the foot of each harness's comment.
    Finding: `falsify.py --check` reports all four as "replayable with no patch path" because the
    template reads the line at the head of the comment. The runbook says the dry run moves the lines.
    Evidence: `src/lib.rs:494, 512, 538, 577`; `notes/crate-move.md` (the dry run).
    Change: none, if the dry run does what the runbook says; the move lane confirms `proofs /
    falsify` is green on the first push. Effort: none. Door: two-way.

14. Two lines of the crate documentation name the architect.
    Finding: `standalone.py` reports two "architect's name" lines, the crate-level `Name:` block.
    `strip_name_blocks` removes a `//!` `Name:` line and its continuation lines, so the move deletes
    both (verified by reading `helpers/crate_release.py:180-206`).
    Evidence: `src/lib.rs:17-18`.
    Change: none before the move; the move lane confirms the staged `src/lib.rs` carries neither
    line. Effort: none.

15. The README does not name `INTERFACE_ID`.
    Finding: the contract list names `VERSION` (1) but not `INTERFACE_ID` (12), though every program
    declares the pair and the crate documentation says so.
    Evidence: `README.md:66-67`; `src/lib.rs:51-57`.
    Change: one bullet. Effort: XS.

16. The library's prose is over the tree's own caps.
    Finding: `prose_caps.py` over `src/lib.rs`: 3,278 words against a cap of 3,000, a median
    sentence of 21 against 20, a longest of 164 against 40, and 11.6 bold spans per 1,000 words
    against 4. The template gates the README and CHANGELOG only, so this is red nowhere; it is what
    docs.rs shows a stranger.
    Evidence: `src/lib.rs`, the whole file.
    Change: a prose pass that shortens the long sentences and drops most bold. Effort: S.

17. Lints the crate cannot trip.
    Finding: `clippy::undocumented_unsafe_blocks`, `cast_ptr_alignment` and `ptr_as_ptr` under
    `#![forbid(unsafe_code)]` do nothing. The template requires `unsafe_op_in_unsafe_fn` and
    `missing_docs`, which stay.
    Evidence: `Cargo.toml:18-30`; `crate-workflows/helpers/standalone.py:62`.
    Change: delete the three. Effort: XS.

## Decisions for the architect

Two one-way doors, each with a recommendation first. Everything else above is two-way and the
maintainer decides it; the three new names (`Refusal`, `STEP_FIRST_ALL_SAME`, `reply`) ship
provisional and wait for a naming pass.

### 1. Whether readiness refuses a first draw of one repeated nonzero byte

Background. At bring-up a service draws its first bytes and sends one word on its readiness
endpoint: `READY`, or `BRING_UP_FAILED | step`. Today the only thing the draw must satisfy is that
one byte is nonzero. That catches a gated clock, which reads zeros. It passes a source that answers
the same nonzero value every time, which is the other way a hardware generator fails at start, and
the one `x86_init_rdrand` and the FIPS continuous test were written for. Decision 137 puts
continuous health tests in the driver, where a stuck source will be caught once those tests exist;
none exists yet. The step constants are wire values a bench reader decodes from the report word, so
adding one after a release is a one-way door; before it, the crate is linked by this tree alone.

Recommendation: add the rule now. `readiness` answers `bring_up_failure(STEP_FIRST_ALL_SAME)` when
the draw holds at least `MAXIMUM_BYTES` bytes and every byte equals the first and is nonzero; the
step takes `0x12`, next after the two shared steps. For a uniform source the false refusal has
probability 255 in 2^64 for an eight-byte draw, which is below the all-zero rule's 2^-64 only by a
factor of 255 and still the safe way round. Refusing more at bring-up is the stricter option, and
relaxing a rule later is compatible where tightening one is not.

Prior art. Linux reads `RDRAND` eight times at boot and disables the instruction when consecutive
values repeat (unverified detail). FIPS 140-2 section 4.9.2 compares each block with the last.
SP 800-90B section 4.4.1 counts repeats of one sample. None of the listed user-space interfaces
checks anything at start.

Example. `readiness([0xff; 8])` is `READY` today. Under the rule it is
`bring_up_failure(STEP_FIRST_ALL_SAME)`, and a report word of `0xDEAD_0000_0000_0012` tells a bench
session the source answered, and answered the same thing eight times.

### 2. Whether the three service-side rules of item 6 become part of the contract

Background. The contract crate states what a client sends and reads. Three things a service does
are not in it: it answers an opcode it does not know with `NO_ENTROPY`; it ignores bits 55:8 of the
request word; it puts zeros past the delivered bytes of the second reply word. Both services in the
tree do all three, and a system test pins the first. Once published, a service written outside this
tree reads the contract and may do otherwise unless the contract says.

Recommendation: write all three down as the contract, in the crate documentation and the README, as
item 6 states them. They record behavior every service already has, so nothing changes in the tree;
the alternative, a reply code for "unknown operation", widens the reply's first word past "a count
or not a count", the property the whole error story rests on, and is a larger change than a
one-operation protocol needs.

Prior art. Linux `getrandom(2)` rejects unknown flags with `EINVAL` rather than answering nothing;
virtio reserves and requires zero in the fields it does not define (unverified wording). This crate
has no error channel for a service to say "unknown", so the nearest honest answer is the one the
services give, written down.

Example. A client built against a later version that adds a second opcode talks to a version-1
service: it sends `request(2, 8)` and reads `NO_ENTROPY` back, which the contract will now tell it
may mean "unknown operation" as well as "dry".

## Done when

- `## What is proven` names the four properties, their bounds and the falsification patches, and
  no sentence of the README or the crate documentation is contradicted by the code.
- `requested_bytes(request(GET, 256))` is `MAXIMUM_BYTES`, the harness asserts the clamp, and the
  README's example runs unchanged over a 256-byte buffer.
- `passive_voice.py`, `readme_badges.py` and `standalone.py --allow-unpublished` from the pinned
  template report nothing on the staged tree, and `script/crate-move --dry-run` exits 0.
- The README takes a stranger from a clone to `cargo test`, the three bare-metal builds, `cargo
  kani`, one harness and one falsification, and a stranger run is on record with no open finding
  against it.
- `fill_with` returns `Result<(), Refusal>` (or the ratified name), the fill harness tells the two
  refusals apart, and `user_mode_runtime::fill` compiles against it.
- The contract documentation states the unknown-opcode answer, the reserved bits and the zeroed
  tail, and `readiness` carries the architect's ruling on a repeated draw with a harness and a
  patch.
- `Cargo.toml` names no path outside the repository, carries `no-std::no-alloc` and not
  `cryptography`, and the five pure functions carry `#[must_use]`.
- `check` and `proofs` are green on `main` in the new repository for the crate's own reasons.

## BUGS

- Items 3, 4, 8, 13 and 14 describe what the template's gates would report, from the helpers run by
  hand at the template's head, not from the dry run or a workflow run. A lane checks each against
  the staged tree before acting.
- The prior-art rows for Linux, virtio and `x86_init_rdrand` are from memory and marked unverified
  where the wording matters; a lane confirms the sources before the decision text cites them.
- Item 8's false-refusal probability assumes a uniform source. A source whose output is biased
  toward one byte value is exactly the source the rule is meant to refuse, so the figure is a lower
  bound on how often a good source is wrongly refused and says nothing about a poor one.
- No mutation census was run; the figures are the tree's records at the pre-rename names. Items 2,
  5 and 8 add branches that the next census judges.

## Index row

An outside review of `entropy_contract` before its move: two items that block any publish (a README
that denies its own proofs and a request builder that truncates where it says it clamps), six to
land before the first release (two template gates the README fails, the README's path to a proof,
an error type for `fill_with`, three service-side rules the contract leaves unsaid, the manifest's
comment and categories, and a bring-up rule that passes a stuck source), and nine for later.
