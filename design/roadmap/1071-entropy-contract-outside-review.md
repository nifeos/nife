---
status: PARTIAL
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

Reuse: the template `nifeos/crate-workflows`, Kani, cargo-mutants and the crate's own harnesses.

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

## Finding status

One line per finding, in the review's order. Pull requests are in nifeos/nife unless named
otherwise; `entropy_contract#N` is nifeos/entropy_contract. Line numbers in the review are in
`crates/entropy_contract/` at `cf1328af`.

- P0-1, the README denies the crate's proofs: Done, [#3512](https://github.com/nifeos/nife/pull/3512).
  `## What is proven` names every harness and its falsification patch.
- P0-2, `request` truncates a byte count instead of clamping it: Done,
  [#3512](https://github.com/nifeos/nife/pull/3512). `request` clamps to `MAXIMUM_BYTES` and the
  harness asserts the clamp.
- P1-3, the README fails the passive-voice gate: Done, [#3512](https://github.com/nifeos/nife/pull/3512).
- P1-4, no badge row and no path from a clone to a proof: Done,
  [#3558](https://github.com/nifeos/nife/pull/3558),
  [entropy_contract#10](https://github.com/nifeos/entropy_contract/pull/10). The README carries the
  badge row, the install lines, the Kani install's cost, the measured proof time and memory, one
  harness, one falsification patch with its undo, the alarming output and the minimum-version check.
- P1-5, `fill_with` folds two failures together: Done, [#3558](https://github.com/nifeos/nife/pull/3558).
  `fill_with` returns `Result<(), Refusal>`; `user_mode_runtime::fill` maps it with `.ok()`.
- P1-6, three rules the services keep are unwritten: Done (decision 2, ruled 2026-11-14 UTC),
  [#3573](https://github.com/nifeos/nife/pull/3573),
  [#3616](https://github.com/nifeos/nife/pull/3616). The crate documentation and README state them,
  and the three service loops check `request_is_well_formed`.
- P1-7, the manifest's `publish` comment, categories and metadata: Done,
  [#3558](https://github.com/nifeos/nife/pull/3558),
  [#3627](https://github.com/nifeos/nife/pull/3627). The repository's `ci.yml` passes
  `allow-unpublished: true`.
- P1-8, readiness passes a source stuck on a nonzero value: Done (ruled 2026-11-11 UTC),
  [#3531](https://github.com/nifeos/nife/pull/3531). `readiness` refuses it under
  `STEP_FIRST_ALL_SAME`, with a harness and a patch.
- P2-9, a reply builder for the service side: Done, [#3558](https://github.com/nifeos/nife/pull/3558),
  [#3616](https://github.com/nifeos/nife/pull/3616). The three service loops build their replies
  with `reply`.
- P2-10, `#[must_use]` on the pure functions: Done, [#3558](https://github.com/nifeos/nife/pull/3558).
- P2-11, `request` and `bring_up_failure` accept values past their fields: Done (decision 4),
  [entropy_contract#5](https://github.com/nifeos/entropy_contract/pull/5). Both return `Err` in
  every build, with a harness and a patch.
- P2-12, the readiness proof's bound is below every draw: Done,
  [#3531](https://github.com/nifeos/nife/pull/3531).
- P2-13, the `Falsification:` lines sit at the foot of each comment: Done,
  [#3627](https://github.com/nifeos/nife/pull/3627). The move placed them, and `proofs` is green on
  the repository's `main`.
- P2-14, two lines of the crate documentation name the architect: Done,
  [#3627](https://github.com/nifeos/nife/pull/3627). The move stripped the `Name:` block.
- P2-15, the README does not name `INTERFACE_ID`: Done, [#3558](https://github.com/nifeos/nife/pull/3558).
- P2-16, the library's prose is over the caps: Done, [#3558](https://github.com/nifeos/nife/pull/3558).
- P2-17, lints the crate cannot trip: Done, [#3558](https://github.com/nifeos/nife/pull/3558).
- Decision 1, the name `STEP_FIRST_ALL_SAME`: Done (maintainer, provisional),
  [#3531](https://github.com/nifeos/nife/pull/3531).
- Decision 2, what the contract says a service does: Done (ruled 2026-11-14 UTC),
  [#3573](https://github.com/nifeos/nife/pull/3573).
- Decision 3, the names `Refusal`, `NoService`, `ShortReply` and `reply`: Done (maintainer,
  provisional), [#3558](https://github.com/nifeos/nife/pull/3558).
- Decision 4, a release build refuses an out-of-range opcode or step: Done (maintainer,
  provisional), [entropy_contract#5](https://github.com/nifeos/entropy_contract/pull/5).
- The move: Done, [#3627](https://github.com/nifeos/nife/pull/3627).
- The stranger run: Done, [entropy_contract#7](https://github.com/nifeos/entropy_contract/pull/7),
  [entropy_contract#10](https://github.com/nifeos/entropy_contract/pull/10).
- The pre-release tag `v0.1.0-pre.1`: Waiting on the maintainer's tag (ruled 2026-11-19 UTC). The
  release workflow's dry run on nifeos/entropy_contract@5c9f8eb comes first, then the tag on that
  commit.
- nife's pins: Waiting on the tag. Every `entropy_contract` pin in nife is at
  nifeos/entropy_contract@7e800fd; a nife pull request moves them to the tagged commit and cites it.

## Decisions

Decisions 1, 3 and 4 are the maintainer's, two-way, and stay provisional until the release review
rechecks every name. Decision 2 is the architect's. Item 6 is decision 2.

### Decision 1: What to call the step that refuses a repeated first draw

Ruled by the maintainer, provisional; landed in #3531. The readiness step `0x12` keeps the name
`STEP_FIRST_ALL_SAME`. It pairs with `STEP_FIRST_ALL_ZERO`. `STEP_FIRST_ALL_EQUAL` and
`STEP_FIRST_REPEATED` were the other options. The architect ruled on 2026-11-11 (UTC) that the
step exists: `readiness` refuses a first draw of eight or more bytes that are all one value.

### Decision 2: What the contract says a service does

Ruled by the architect, 2026-11-14 (UTC); landed in #3573 and #3616. The contract states what a
service does:

- A service answers an unknown operation with `NO_ENTROPY`. The crate's `BUGS` says a client cannot tell that
  from a dry source. The alternative, a reply code for "unknown operation", would widen the
  reply's first word past "a count or not a count".
- A service answers a request word with any of bits 55:8 set with `NO_ENTROPY`, and a client
  sends zero there. This is the restrictive form, the same shape as the clock-and-reset wire rule (section
  320 (the jh7110 clock-and-reset wire)): a later version can give those bits a meaning without an
  old service misreading them. It means `VERSION`'s promise that an optional field owes no bump no
  longer holds: a new field is a new operation or a bump.
- The reply's byte word holds zeros past the delivered count.

The crate's `request_is_well_formed` says whether a request word is well formed, so the three
service loops share one check; the name is provisional. Prior art: Linux `getrandom(2)` rejects
an unknown flag with `EINVAL` (`drivers/char/random.c`), the restrictive form. Virtio takes the
other form: a driver ignores and never accepts a reserved feature bit, and a device ignores the
reserved flags and fields of an indirect descriptor (the virtio specification's feature-bit and
packed-virtqueue sections). This crate has no error channel, so the nearest honest answer is
`NO_ENTROPY`. Example: a client sends `request(1, 8) | (1 << 20)` and reads `NO_ENTROPY`.

### Decision 3: What to call the refusal type and the reply builder

Ruled by the maintainer, provisional; landed in #3558. `Refusal`, `NoService`, `ShortReply` and
`reply` keep their names. `Refusal` is the tree's word for a refused call.

### Decision 4: Whether a release build refuses an out-of-range opcode or step

Ruled by the maintainer, provisional; landed in nifeos/entropy_contract#5. `request` and
`bring_up_failure` return a `Result` and refuse a value past one byte in every build, where they
had panicked in a debug build and accepted it in a release build. The restrictive form, since a
later release can loosen it and cannot tighten it. The error types `OperationOutOfRange` and
`StepOutOfRange` carry provisional names. nife's callers already use the `Result` form.

### The pre-release tag

Ruled by the architect, 2026-11-19 (UTC): the repository takes the tag `v0.1.0-pre.1` once the
release workflow's dry run is green on `main`; nothing publishes. The manifest reads `0.1.0-pre.1`
(nifeos/entropy_contract#8), and `main` at nifeos/entropy_contract@5c9f8eb carries every finding's
fix. The dry run and the tag are the maintainer's, and the pin bump follows the tag.

## Open decisions

None. Every finding and decision has landed or has a ruling; nothing in this milestone waits on a
question to the architect. The first publication waits for the publishing hold, which lifts only
when every outside-review milestone has delivered, and nobody asks it crate by crate.

## Done when

The checks are in [the done-when appendix](1071-entropy-contract-outside-review/done-when.md).
Every check holds on the repository's `main`.

## Where this stands

Every finding, the four decisions, the move and the stranger run are Done (see the finding status).
`check` and `proofs` are green on nifeos/entropy_contract@5c9f8eb, and `mutation` and `weekly` on
the commit before it. Two steps remain, in order: the tag `v0.1.0-pre.1` on `main` after a green
release dry run, then a nife pull request that moves every `entropy_contract` pin from
nifeos/entropy_contract@7e800fd to the tagged commit. Publishing stays under the publishing hold.

## Follow-on

- **Done.** The move, the stranger run and the JH7110 service loop's `reply`: see the finding status.
- **Outstanding.** The tag `v0.1.0-pre.1`, which has its ruling: nifeos/entropy_contract lists no
  tag (checked through the GitHub API). Then the pin bump: every `entropy_contract` pin in nife
  reads nifeos/entropy_contract@7e800fd (checked with `grep` over the manifests).

## BUGS

- Item 8's false-refusal probability assumes a uniform source. A source with its output biased
  toward one byte value is exactly the source the rule means to refuse, so the figure bounds from below how often
  the rule wrongly refuses a good source. It says nothing about a poor one.

## Index row

An outside review of `entropy_contract` before its move. Every finding and the four decisions have
landed, the crate has moved and the repository holds its own stranger run. The pre-release tag has
its ruling, then the pin bump follows. The maintainer's names stay provisional until the release
review.
