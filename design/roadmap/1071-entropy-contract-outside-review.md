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
decisions it settles. Those are the crate name, the identity (`INTERFACE_ID` 12, `VERSION` 1, no
magic), the thirteen names ratified on #3480 and the six earlier ones, `#![forbid(unsafe_code)]`,
the four harnesses, the dual license, `publish = false` through the soak, and the move itself.
No later rule in [design/naming.md](../naming.md) contradicts any ratified name. Every quantity ends
in its unit, the rules spell out every abbreviation they list, and `fill_with` takes std's shape.

Reuse: the template `nifeos/crate-workflows` and its scaffold, Kani, cargo-mutants, the crate's own
harnesses. No new tool.

## Goal

A stranger who finds `entropy_contract` can tell in one screen what the two words on the wire mean.
The same stranger can follow the README alone to a build, the tests, the proofs and one
falsification, and meets no sentence in the README or the documentation that the code contradicts. The request builder does
what its documentation says for every byte count. The repository's `check` and `proofs` runs are
green on the first push for the crate's own reasons.

## What was checked, and how

The tools run, the sources read and what was not run are in [the appendix](1071-entropy-contract-outside-review/what-was-checked.md).

## Prior art

The comparison with nearby work is in [the prior art appendix](1071-entropy-contract-outside-review/prior-art.md).

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
   Evidence: `README.md` `## What is proven`; `src/lib.rs:487-593`; `falsifications/`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   Change: one line per property in a user's words. A request word reads back as built. A copy
   writes only the delivered bytes and nothing past them. A fill succeeds exactly when every reply
   is full. Readiness is `READY` exactly when a first byte is nonzero. Each line carries the bound
   its proof runs to (a slice of up to 17 bytes, a buffer of 12, a draw of 4), and the section says
   "every harness carries a falsification patch".
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
   (`fill_with`, the standard library overlay, `system_initializer`, the kernel's test client), so
   no program breaks today; a stranger following the README does. The Kani harness proves the
   truncating behavior, `(byte_count & 0xff).min(MAXIMUM_BYTES)`, which is why it did not catch the
   gap between the code and its documentation.
   Evidence: `src/lib.rs:69-71` (the builder), `59-61` and `81-83` (the claims), `248-258` and <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `README.md` `## EXAMPLES` (the examples), `src/lib.rs:502-505` (the harness); verified by a throwaway <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   test.
   Change: `request` clamps, `(operation << OPERATION_SHIFT) | byte_count.min(MAXIMUM_BYTES)`, so
   the wire carries at most eight in bits 7:0 and `requested_bytes` keeps its clamp for a request
   another builder made. The harness asserts `byte_count.min(MAXIMUM_BYTES)`, and a new patch that
   forgets the clamp replaces the one that forgot the mask. The two examples keep
   their unclamped remainder, since the builder now makes that correct, and a unit test asks for
   256 and reads eight back.
   Door: two-way. The wire format does not change; a builder's output for counts above 255 does.
   Effort: XS.

### P1: before the first release

3. The README fails the template's passive-voice gate.
   Finding: `passive_voice.py` counts 6 passive constructions in 536 words and allows 1; `check /
   passive voice and history phrases` runs it over `README*` and fails the job. The six read "is
   transformed", "be given", "be read", "is answered", "are covered" and "is read". The dry run `script/crate-move --dry-run` runs the same check on the staged tree
   and refuses to finish (inferred from the runbook; the dry run was not run).
   Evidence: `README.md` (the six phrases above); `crate-workflows/.github/workflows/check.yml:254-266`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   Change: rewrite five of the six in the active voice, in nife before the move. Item 1 rewrites
   the sixth ("are covered") anyway.
   Door: two-way. Effort: XS.

4. The README has no badge row and no path from a clone to a proof.
   Finding: `readme_badges.py` reports three findings; `check / README badge row` is a required
   check and a finding fails it. `## Building and testing` gives `cargo test` and one bare-metal
   build line, and nothing else the scaffold's `README-crate.md` asks for. Missing are the
   `rustup target add` line before the first bare-metal build and the Kani install line
   (`cargo install --locked kani-verifier --version <.kani-version>` then `cargo kani setup`, with
   the note that setup needs the network). Also missing are how long `cargo kani` takes and its
   peak memory. The README also omits how to run one harness (`cargo kani --harness
   verification::<name> --exact`), how to apply one falsification patch and undo it, what a passing
   run prints that looks alarming, and how to check the minimum Rust version. The stranger test
   reads the README alone; the release gate refuses while no stranger run is on record.
   Evidence: `README.md` `## Building and testing`; `crate-workflows/scaffold/README-crate.md`;
   `crate-workflows/.github/workflows/check.yml:336-347`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   Change: in nife before the move, add the badge row under the title with the repository's name.
   Add the two subsections `### Running the proofs` and `### Running one falsification patch` with
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
   Evidence: `src/lib.rs:272-293`; `src/lib.rs:189-196`; <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `patches/std-nife/overlay/std/src/sys/random/nife.rs:67-82`; <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `crates/user_mode_runtime/src/entropy.rs:17-30`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
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
   test pins that. The contract says nothing, so a client cannot tell a wrong opcode from a dry
   device, and the README's "fail, do not retry" advice sends it to the wrong fix. (b) Bits 55:8 of
   the request word carry no defined meaning: `request` zeros them and `requested_bytes` masks them,
   but no sentence says a service ignores them. `VERSION`'s promise that an addition owes no bump
   holds only if receivers ignore what they do not know. (c) The documentation says the second
   reply word's bytes past the count hold "whatever the service had left in the register". The
   virtio pool in fact zeros them and hands out no byte twice, and the instruction backend returns
   a fresh draw, but the contract leaves a service free to leak the next client's bytes.
   Evidence: `components/src/entropy.rs:386-397`, `components/src/jh7110_entropy.rs:268-273`, <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `system_tests/src/user/entropy_tests.rs:205-208`; `src/lib.rs:43-49` (`VERSION`); <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `src/lib.rs:229-231`; `components/src/entropy.rs:269-289`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   Change: three sentences in the crate documentation and the README's contract list, recording what
   is. A service answers an unknown opcode with `NO_ENTROPY`, and a BUGS line says a client cannot
   tell the two apart. Bits 55:8 stay reserved: a client sends zero and a service ignores them. A
   service puts nothing but zeros past the delivered bytes. The third also softens the
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
   Evidence: `Cargo.toml:9-11`; `crate-workflows/helpers/standalone.py:197-203`; <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   `nifeos/jh7110_entropy` `Cargo.toml` and `.github/workflows/ci.yml` (read through the GitHub API).
   Change: now, a self-contained comment ("set to `true` only in the release that publishes");
   `categories = ["no-std", "no-std::no-alloc", "os"]`; `documentation =
   "https://docs.rs/entropy_contract"` beside the `repository` the move writes. At the move, the
   `ci.yml` caller passes `allow-unpublished: true`. At the release, delete the line.
   Door: two-way. Effort: XS.

8. Readiness passes a source stuck on any nonzero value.
   Finding: `readiness` is `READY` when any byte of the first draw is nonzero, so a draw of eight
   `0xff` bytes, or of any repeated byte, is `READY`. The rule targets a gated clock that
   reads zeros, and it does that. A source that latches a nonzero value is the other known
   bring-up failure, and it passes. The Linux `x86_init_rdrand` check and the FIPS 140-2 continuous
   test exist for it, and a processor erratum of this shape is on record (details unverified).
   Decision 137 puts continuous health tests in the driver, which would catch it later; the tree's
   entropy note records that no such test exists yet. The bring-up word is the one place a bench
   reader looks.
   Evidence: `src/lib.rs:165-178`; `readiness([0xff; 8]) == READY`, verified by a throwaway test; <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
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

The P2 items are in [the appendix](1071-entropy-contract-outside-review/later-items.md).

## Decisions for the architect

Two one-way doors, each with a recommendation first. Everything else above is two-way and the
maintainer decides it; the three new names (`Refusal`, `STEP_FIRST_ALL_SAME`, `reply`) ship
provisional and wait for a naming pass.

### Decision 1: Whether readiness refuses a first draw of one repeated nonzero byte

Background. At bring-up a service draws its first bytes and sends one word on its readiness
endpoint: `READY`, or `BRING_UP_FAILED | step`. Today the only thing the draw must satisfy is that
one byte is nonzero. That catches a gated clock, which reads zeros. It passes a source that answers
the same nonzero value every time, which is the other way a hardware generator fails at start, and
the one `x86_init_rdrand` and the FIPS continuous test target. Decision 137 puts continuous
health tests in the driver, where they will catch a stuck source once they exist; none exists yet.
The step constants are wire values a bench reader decodes from the report word, so adding one
after a release is a one-way door. Before it, this tree alone links the crate.

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

### Decision 2: Whether the three service-side rules of item 6 become part of the contract

Background. The contract crate states what a client sends and reads. Three things a service does
are not in it: it answers an opcode it does not know with `NO_ENTROPY`; it ignores bits 55:8 of the
request word; it puts zeros past the delivered bytes of the second reply word. Both services in the
tree do all three, and a system test pins the first. Once published, a service written outside this
tree reads the contract and may do otherwise unless the contract says.

Recommendation: write all three down as the contract, in the crate documentation and the README, as
item 6 states them. They record behavior every service already has, so nothing changes in the tree.
The alternative is a reply code for "unknown operation". It widens the reply's first word past "a
count or not a count", the property the whole error story rests on, and it is a larger change than
a one-operation protocol needs.

Prior art. Linux `getrandom(2)` rejects unknown flags with `EINVAL` rather than answering nothing;
virtio reserves and requires zero in the fields it does not define (unverified wording). This crate
has no error channel for a service to say "unknown", so the nearest honest answer is the one the
services give, written down.

Example. A client built against a later version that adds a second opcode talks to a version-1
service: it sends `request(2, 8)` and reads `NO_ENTROPY` back, which the contract will now tell it
may mean "unknown operation" as well as "dry".

## Done when

The checks are in [the done-when appendix](1071-entropy-contract-outside-review/done-when.md).

## BUGS

- Items 3, 4, 8, 13 and 14 describe what the template's gates would report, from the helpers run by
  hand at the template's head, not from the dry run or a workflow run. A lane checks each against
  the staged tree before acting.
- The prior-art rows for Linux, virtio and `x86_init_rdrand` are from memory and marked unverified
  where the wording matters; a lane confirms the sources before the decision text cites them.
- Item 8's false-refusal probability assumes a uniform source. A source with its output biased
  toward one byte value is exactly the source the rule means to refuse, so the figure bounds from below how often
  the rule wrongly refuses a good source. It says nothing about a poor one.
- Nobody ran a mutation census; the figures are the tree's records at the pre-rename names. Items
  2, 5 and 8 add branches that the next census judges.

## Index row

An outside review of `entropy_contract` before its move. Two items block any publish: a README that
denies its own proofs and a request builder that truncates where it says it clamps. Six land before
the first release. They are two template gates the README fails, the README's path to a proof, an error type
for `fill_with`, three service-side rules the contract leaves unsaid, the manifest's comment and
categories, and a bring-up rule that passes a stuck source. Nine follow later.
