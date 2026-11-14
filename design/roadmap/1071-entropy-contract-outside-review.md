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

Landed items are not repeated here; their text is in the pull requests named below. Open ones keep
their text.

| item | state |
|---|---|
| 1, 2, 3 | landed (#3512): the README states the proofs, `request` clamps, the passive-voice gate passes |
| 4 | landed: badge row, the install lines, the measured Kani time and memory, one harness, one falsification patch with its undo, the alarming output and the minimum-version check. No stranger has run it yet |
| 5 | landed: `fill_with` returns `Result<(), Refusal>`; `user_mode_runtime::fill` maps it with `.ok()` |
| 6 | landed: decision 2 states the three rules in the crate documentation and README, and the three service loops keep them |
| 7 | landed: the `publish` comment, the categories and `documentation`. The `allow-unpublished: true` line in `ci.yml` belongs to the move lane |
| 8 | landed (#3531): `readiness` refuses a repeated first draw under `STEP_FIRST_ALL_SAME`; the name is decision 1 |
| 9 | landed: `reply`, a harness and a falsification patch. The three backends adopt it in their own pull requests |
| 10, 11 | landed: `#[must_use]` on the pure functions; `debug_assert!` on the opcode and step widths |
| 12 | landed (#3531): the readiness harness covers nine-byte draws |
| 13, 14 | confirmed: `script/crate-move --dry-run` against the template's `main` exits 0, so the falsification records and the architect's-name lines need no change |
| 15, 16, 17 | landed: the `INTERFACE_ID` bullet, the prose pass over the crate documentation, three inert lints deleted |

### Item 6, landed

6. Three rules the services keep are written in the contract: an unknown opcode is answered
   `NO_ENTROPY`; a request word with any of bits 55:8 set is answered `NO_ENTROPY`; the reply's
   byte word holds zeros past the delivered count. The reasoning is in decision 2.

## Decisions

Decisions 1 and 3 are names, and each name stays provisional until a naming pass. Decision 2 is
the architect's. Item 6 is decision 2.

### Decision 1: What to call the step that refuses a repeated first draw

Ruled by the maintainer, provisional: the readiness step `0x12` keeps the name
`STEP_FIRST_ALL_SAME`. It pairs with `STEP_FIRST_ALL_ZERO`. `STEP_FIRST_ALL_EQUAL` and
`STEP_FIRST_REPEATED` were the other options.

### Decision 2: What the contract says a service does

Ruled by the architect, 2026-11-14 (UTC). The contract states what a service does:

- An unknown operation is answered `NO_ENTROPY`. The crate's `BUGS` says a client cannot tell that
  from a dry source. The alternative, a reply code for "unknown operation", would widen the
  reply's first word past "a count or not a count".
- A request word with any of bits 55:8 set is answered `NO_ENTROPY`, and a client sends zero
  there. This is the restrictive form, the same shape as the clock-and-reset wire rule (section
  320 (the jh7110 clock-and-reset wire)): a later version can give those bits a meaning without an
  old service misreading them. It means `VERSION`'s promise that an optional field owes no bump no
  longer holds: a new field is a new operation or a bump.
- The reply's byte word holds zeros past the delivered count.

The crate's `request_is_well_formed` says whether a request word is well formed, so the three
service loops share one check; the name is provisional. Prior art: Linux `getrandom(2)` rejects
unknown flags with `EINVAL`, and virtio requires zero in the fields it does not define (both
unverified wording). This crate has no error channel, so the nearest honest answer is
`NO_ENTROPY`. Example: a client sends `request(1, 8) | (1 << 20)` and reads `NO_ENTROPY`.

### Decision 3: What to call the refusal type and the reply builder

Ruled by the maintainer, provisional: `Refusal`, `NoService`, `ShortReply` and `reply` keep their
names. `Refusal` is the tree's word for a refused call.

## Done when

The checks are in [the done-when appendix](1071-entropy-contract-outside-review/done-when.md).

## Follow-on

- **Outstanding.** A stranger's run of the README, which the release gate requires. Checked
  2026-11-14 against `notes/crate-reviews/entropy-contract.md`, which records none.
- **Outstanding.** The three service loops adopt `reply`: the instruction backend does; the virtio
  backend and the `jh7110_entropy` crate's `take` build the word by hand and already zero the tail.
- **Outstanding.** The move itself, with the `allow-unpublished: true` line in its `ci.yml`. The
  dry run exits 0 against the template's `main`.

## BUGS

- The dry run was made against the template's `main`, which is newer than its newest release. At
  the release the move pins, the passive-voice and prose gates may not run, and a lane repeats the
  run there.
- The prior-art rows for Linux, virtio and `x86_init_rdrand` are from memory and marked unverified
  where the wording matters; a lane confirms the sources before a decision cites them.
- Item 8's false-refusal probability assumes a uniform source. A source with its output biased
  toward one byte value is exactly the source the rule means to refuse, so the figure bounds from below how often
  the rule wrongly refuses a good source. It says nothing about a poor one.
- Nobody ran a mutation census; the figures are the tree's records at the pre-rename names. Items
  2, 5, 8 and 9 add branches that the next census judges.

## Index row

An outside review of `entropy_contract` before its move. Every item and the three decisions have
landed; two names stay provisional. No stranger has run the README yet.
