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

## The items

Effort: XS is under an hour, S half a day, M a day or two, L longer. A door is one-way
when somebody outside the repository acts on it after a release (a public name, a published
behavior, a wire value, a layout, a dependency); everything else is two-way and the maintainer
decides it. Line numbers are in `crates/entropy_contract/` at `cf1328af`.

### The ledger

Every item's text is in the pull request that landed it; open ones keep their text here. Pull
requests are in nifeos/nife unless named otherwise.

| item | status |
|---|---|
| 1, 2, 3 | Done (#3512): the README states the proofs, `request` clamps, the passive-voice gate passes |
| 4 | Done (#3558): badge row, the install lines, the measured Kani time and memory, one harness, one falsification patch with its undo, the alarming output and the minimum-version check. The Kani install's own time, download size and platforms: Open in nifeos/entropy_contract#10, ready for the merge |
| 5 | Done (#3558): `fill_with` returns `Result<(), Refusal>`; `user_mode_runtime::fill` maps it with `.ok()` |
| 6 | Done (#3573): decision 2's three rules are in the crate documentation and README; the three service loops keep them (#3616) |
| 7 | Done (#3558, #3627): the `publish` comment, the categories, `documentation`, and the `allow-unpublished: true` line in the repository's `ci.yml` |
| 8 | Done (#3531): `readiness` refuses a repeated first draw under `STEP_FIRST_ALL_SAME`; the name is decision 1 |
| 9 | Done (#3558, #3616): `reply`, a harness and a falsification patch; the three service loops build their replies with it |
| 10 | Done (#3558): `#[must_use]` on the pure functions |
| 11 | Done (nifeos/entropy_contract#5): `request` and `bring_up_failure` refuse an out-of-range opcode or step in every build, returning `OperationOutOfRange` or `StepOutOfRange`, with a harness and a patch; decision 4 |
| 12 | Done (#3531): the readiness harness covers nine-byte draws |
| 13, 14 | Done: `script/crate-move --dry-run` against the template's `main` exits 0, so the falsification records and the architect's-name lines need no change |
| 15, 16, 17 | Done (#3558): the `INTERFACE_ID` bullet, the prose pass over the crate documentation, three inert lints deleted |

### Item 6, landed

6. The contract states three rules the services keep: a service answers an unknown opcode with
   `NO_ENTROPY`; it answers a request word with any of bits 55:8 set with `NO_ENTROPY`; the reply's
   byte word holds zeros past the delivered count. The reasoning is in decision 2.

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

Ruled by the architect, 2026-11-19 (UTC): a lane tags the repository `v0.1.0-pre.1` once the
release workflow's dry run is green on `main`; nothing publishes. Not built yet: the manifest already reads
`0.1.0-pre.1` (nifeos/entropy_contract#8), and the dry run and the tag wait for
nifeos/entropy_contract#10 to merge so the tag carries it. The pin bump below follows the tag.

## Open decisions

None. Every finding and decision has landed, has a ruling or sits under a hold; nothing in this
milestone waits on a question to the architect. The first publication waits for the publishing
hold, which lifts only when every outside-review milestone has delivered, and nobody asks it crate
by crate.

## Done when

The checks are in [the done-when appendix](1071-entropy-contract-outside-review/done-when.md).
Every check holds on the repository's `main` except the stranger-run clause, which holds once
nifeos/entropy_contract#10 answers the run's one open gap.

## Where this stands

Done: every item and the four decisions (see the ledger), and the move (#3627). In the
repository: the stranger run, with `Last run:` in `docs/stranger-test.md`
(nifeos/entropy_contract#7); the template bump to crate-workflows v8 (nifeos/entropy_contract#6);
and a full mutation run, green on `main` with the two equal clamp mutants exempt
(nifeos/entropy_contract#9). `check`, `proofs`,
`mutation` and `weekly` are green on the repository's `main`. The `jh7110_entropy` crate's
`Pool::take_into` serves bytes into the caller's slice (nifeos/jh7110_entropy#29), and nife's
JH7110 service loop passes them through `reply`.

Open, in order:

- nifeos/entropy_contract#10: the Kani install's time, download size and platforms in the README.
  Ready; waits for the maintainer's merge.
- The tag `v0.1.0-pre.1`: ruled; a lane runs the release dry run on `main` after #10 merges and
  tags that commit.
- nife's pin: every `entropy_contract` pin in nife is at nifeos/entropy_contract@7e800fd, behind
  `main` by #6 to #10. A nife pull request moves them to the tagged commit and cites it.
- Publishing: held by the publishing hold.

## Follow-on

- **Done.** The repository's own stranger test (runbook step 12), from a clone of
  `nifeos/entropy_contract` (nifeos/entropy_contract#7). Its one open gap is #10 above.
- **Done.** `jh7110_entropy` hands the bytes out through `Pool::take_into`
  (nifeos/jh7110_entropy#29), and the JH7110 service loop builds the reply with `reply`.
- **Done.** The move: the repository `nifeos/entropy_contract` holds the crate with the
  `allow-unpublished: true` line in its `ci.yml`, and nife pins it by commit.
- **Outstanding.** The pre-release tag, which has its ruling, then the pin bump to the tagged
  commit.

## BUGS

- Item 8's false-refusal probability assumes a uniform source. A source with its output biased
  toward one byte value is exactly the source the rule means to refuse, so the figure bounds from below how often
  the rule wrongly refuses a good source. It says nothing about a poor one.

## Index row

An outside review of `entropy_contract` before its move. Every item and the four decisions have
landed, the crate has moved and the repository holds its own stranger run. The pre-release tag has
its ruling and waits for one README pull request, then the pin bump. The maintainer's names stay provisional
until the release review.
