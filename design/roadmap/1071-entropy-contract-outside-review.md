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
| 6 | open: decision 2 |
| 7 | landed: the `publish` comment, the categories and `documentation`. The `allow-unpublished: true` line in `ci.yml` belongs to the move lane |
| 8 | landed (#3531): `readiness` refuses a repeated first draw under `STEP_FIRST_ALL_SAME`; the name is decision 1 |
| 9 | landed: `reply`, a harness and a falsification patch. The three backends adopt it in their own pull requests |
| 10, 11 | landed: `#[must_use]` on the pure functions; `debug_assert!` on the opcode and step widths |
| 12 | landed (#3531): the readiness harness covers nine-byte draws |
| 13, 14 | confirmed: `script/crate-move --dry-run` against the template's `main` exits 0, so the falsification records and the architect's-name lines need no change |
| 15, 16, 17 | landed: the `INTERFACE_ID` bullet, the prose pass over the crate documentation, three inert lints deleted |

### Open: item 6

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
   Evidence: `components/src/entropy.rs:386-397`, `components/src/jh7110_entropy.rs:268-273`, <!-- line-citation: exception. Reason: pinned -->
   `system_tests/src/user/entropy_tests.rs:205-208`; `src/lib.rs:43-49` (`VERSION`); <!-- line-citation: exception. Reason: pinned -->
   `src/lib.rs:229-231`; `components/src/entropy.rs:269-289`. <!-- line-citation: exception. Reason: pinned -->
   Change: three sentences in the crate documentation and the README's contract list, recording what
   is. A service answers an unknown opcode with `NO_ENTROPY`, and a BUGS line says a client cannot
   tell the two apart. Bits 55:8 stay reserved: a client sends zero and a service ignores them. A
   service puts nothing but zeros past the delivered bytes. The third also softens the
   `copy_delivered_bytes` sentence from "whatever the service had left" to "zero".
   Door: one-way once published, since a service outside this tree acts on them. Today they record
   behavior every service has. See the decisions section. Effort: XS.

## Decisions for the architect

Three one-way doors, each with a recommendation first. Everything else is two-way and the
maintainer decides it. Decision 2 is item 6. Decisions 1 and 3 are names, and each name ships
provisional until a naming pass.

### Decision 1: What to call the step that refuses a repeated first draw

Background. The architect ruled on 2026-11-11 that `readiness` refuses a first draw of at least
`MAXIMUM_BYTES` bytes that repeats one nonzero value, and the crate does so under a new step of
`0x12`. The step is a wire value a bench reader decodes from the report word, so its name is read
beside `STEP_NO_FIRST_BYTES` and `STEP_FIRST_ALL_ZERO` and changes after a release only at a cost.
The name in the tree is a stand-in.

Options.

- `STEP_FIRST_ALL_SAME`, the stand-in. It matches the pattern of `STEP_FIRST_ALL_ZERO`, so the two
  read as a pair. It leaves out "nonzero", which the all-zero step already covers.
- `STEP_FIRST_ALL_EQUAL`. It follows `Itertools::all_equal`, and it names the test the code makes.
  `EQUAL` suggests a comparison with something outside the draw.
- `STEP_FIRST_REPEATED`. It names the failure a bench reader sees. `REPEATED` suggests a period,
  and a stuck source repeats one value.

Recommendation: keep `STEP_FIRST_ALL_SAME`. The pair with `STEP_FIRST_ALL_ZERO` is the point of the
name, and the other two cost a reader a second of doubt each. Renaming later is a one-way door
after a release, which is why the decision comes before it.

Prior art. Linux and the FIPS continuous test describe the failure as consecutive equal values
(unverified wording). SP 800-90B calls it a repetition count. None of them names a bring-up step.

Example. `readiness([0xff; 8])` answers `0xDEAD_0000_0000_0012`. A bench session decodes the low byte
and reads the step name, and the name has to say that the source answered and answered one value.

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

### Decision 3: What to call the refusal type and the reply builder

Background. `fill_with` now returns `Result<(), Refusal>`. `Refusal` has the variants `NoService`
(a reply was not a count) and `ShortReply { asked, delivered }` (a reply carried another count).
The service half has one new function, `reply(delivered: &[u8]) -> (u64, u64)`. All four names are
the lane's, and a published crate keeps them.

Recommendation: keep all four. `Refusal` is the word the tree already uses for a refused call
(`grant_plan::Refusal`, `ShareRefusal`), `NoService` and `ShortReply` name the fix, and
`reply` is the counterpart of `request`. Options are `FillError` (the getrandom and rand_core
habit, `Error`), `Unavailable` and `Short`, and `reply_words`. The tree's word wins over the
habit, and the longer builder name adds nothing the return type does not say.

Prior art. `getrandom::Error` carries one code per cause, and `rand_core::Error` wraps any cause
in a box (both from memory). This crate has two causes and no allocation, so an enum is the shape.

Example. A caller that sees `Err(Refusal::NoService)` fixes a missing grant. One that sees
`Err(Refusal::ShortReply { asked: 8, delivered: 0 })` waits for a dry device.

## Done when

The checks are in [the done-when appendix](1071-entropy-contract-outside-review/done-when.md).

## Follow-on

- **Outstanding.** Item 6 and decisions 1 to 3, which an architect rules. Checked 2026-11-14
  against `crates/entropy_contract/src/lib.rs`: it states none of item 6's three rules, and its
  two provisional names, the step and the refusal type, carry `Name: provisional`.
- **Outstanding.** A stranger's run of the README, which the release gate requires. Checked
  2026-11-14 against `notes/crate-reviews/entropy-contract.md`, which records none.
- **Outstanding.** The three backends adopt `reply` in their own pull requests. Checked 2026-11-14
  against `components/src/entropy.rs` and `components/src/jh7110_entropy.rs`, which still pack the
  word by hand.
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

An outside review of `entropy_contract` before its move. Every item has landed except item 6, three
unwritten service rules, and the three decisions it leaves: the contract's service rules and two
provisional names. No stranger has run the README yet.
