---
status: BUILT
raised: 2026-08-31
built: 2026-09-01
---
# 211. A harness that states its property through the function under test cannot see that function break

Swept 2026-09-01: 146 harnesses read, 11 measured blind, all 11 rewritten, and each carries a
machine-replayable record of the defect its old phrasing could not see. The result and its method
are in notes/falsification.md, under "The sweep for self-referential harnesses".

It has no gate and cannot have one: no check distinguishes "asserts through the function under
test" from "legitimately asserts agreement", so the sweep's output is eleven falsification patches
rather than a lint. After it, `script/falsifications` reads 35 of 145 replayable, over the wider
walk of milestone 212 (`script/falsifications` walks `crates/` only, so the ratio it prints is not
the tree's).

## What the sweep found

The number, for `design/fatal-risks/README.md` risk 2: 11 of 146. Not "11 harnesses that look
suspicious": for each of the eleven there is a patch in the tree, and the pre-211 phrasing was run
against that patch and observed to stay **green** while the rewritten harness goes red. Both
directions, every time, because a claim about a proof is not evidence about a proof.

The eleven span seven crates and are listed with their defects in notes/falsification.md. Three
kinds, and the third is the one nothing predicted:

- The guard's own predicate. `capability_table::derive_never_widens_rights` (`is_subset_of`),
  `dma_validator::an_accepted_descriptor_is_confined` (`is_indirect`),
  `ntp_proto::accepting_is_total_and_a_sample_is_coherent` (`is_negative`, `is_zero`). The harness
  asserts the thing the code refuses on, through the call it refuses with.
- The value's own producer. `component_plan`'s three (`Direction::rights`, `PageKind::mode`,
  `str_eq`), `credential_proto::no_request_word_makes_the_parse_read_outside_the_page` (`id_len`),
  `jh7110_entropy::ready_requires_rand_rdy_and_carries_the_words_untouched` (`assemble`). The harness
  checks a field against the function that filled it.
- An encoder round-tripped through its own decoder. `the_leaf_keeps_address_and_permissions_apart`,
  once per ISA. `leaf_entry` and `entry_pa` never call each other, so no call-graph test finds
  this one, and it is the most dangerous of the three because the artifact is a hardware format.
  A shift wrong in both directions leaves the round trip perfect and the MMU reading the wrong
  page. The rewrite states the bit positions the architecture assigns, as literals rather than
  through the crate's own `ADDR_MASK` or `PPN_SHIFT`, since a harness citing those constants moves
  with a defect in them.

**Three more harnesses were rewritten and recorded as *not* findings**, which is the discipline
the whole exercise turns on. `filesystem_proto`'s two attenuation proofs and
`capability_table::a_deleted_capability_stays_deleted` have the shape and are not blind: the sweep tried
to exhibit a defect their original phrasing misses and could not. They were made more direct
anyway, and their comments say so. Writing "fixed a blind spot" over a hole nobody found is the
same one-step-from-evidence failure §134 exists to end.

The mechanical narrowing was weaker than its hit rate. 23 candidates out of 146 and all eleven
findings among them. Three of those eleven were flagged by a bug in the extractor rather than
by a real call-graph edge, and a correct implementation of the test would have missed the whole
encoder/decoder family. Every harness was read.

In brief. Milestone 194's lane found it in `capability`, and `notes/falsification.md` states the
generalization rather than the instance:

> `derive_never_widens_rights` states its property *through the predicate it is testing*. It asserts
> `derived.rights.is_subset_of(src_rights)` while `derive` guards on
> `rights.is_subset_of(src.rights)`, so a consistently wrong `is_subset_of` satisfies both sides and
> the harness cannot see the break.
>
> -- notes/falsification.md

Milestone 202's lane then found the same shape in `component_plan`'s proof, independently.

Two instances is a class. This milestone is the sweep that asks how many more there are.

## Why it is worth a sweep rather than two fixes

The defect is invisible to everything this project has. The harness passes. `kani::cover!` passes,
because the input set is not empty. Mutation testing does not reach `cfg(kani)` code at all. And
until DECISIONS §134 (a harness carries a machine-replayable falsification record) landed, nothing
even asked the question: it was found only because somebody broke the code and watched what stayed
green.

It is also the most dangerous shape a proof can have, because it is *load-bearing in one direction*.
`derive_never_widens_rights` genuinely catches the guard being dropped, which is what its recorded
falsification does. It is blind only to the comparison the guard calls. So it is neither a good
harness nor a useless one, and a reader seeing it green learns something true and something false at
once.

## The sweep

For every harness, ask: does the assertion reach the same function the code under test uses to
decide? Where it does, the harness proves the two agree, not that either is right.

Mechanical narrowing is possible and should be tried before reading 145 harnesses by hand: a harness
whose assertion calls a function that also appears in the guarded path is the candidate set, and it
is smaller than the whole. **The judgment is not mechanical**, though: agreeing with itself is
sometimes the property you want, and a sweep that flags every such harness as defective would be
wrong.

Each finding is either a rewritten harness or a recorded reason it is fine. A rewritten one
states the property independently, in terms the implementation does not get to choose, the way
milestone 193's kernel harnesses state run arithmetic in `u128` so the harness cannot repeat the
implementation back at itself.

## BUGS

- The rewrite is harder than the finding, and this block does not pretend otherwise. Stating a
  property without using the vocabulary the code uses is exactly where specification work is
  expensive, and for some properties there may be no independent statement worth having.
- It cannot be a gate. No check can tell "asserts through the function under test" from
  "legitimately asserts agreement", so this is a sweep with a worklist, not a lint.
- The sweep's own output can rot. A harness found fine today can become self-referential when the
  code it covers is refactored, and nothing will say so. §134's falsification records are the closest
  mechanism, and they only catch it if somebody re-falsifies after the refactor. The eleven findings
  are now protected by exactly that (each carries a patch the weekly sweep replays); the 135 cleared
  harnesses carry no artifact at all, so a refactor into the cleared shape goes unnoticed.
- **Eleven is a floor, not a count.** Each finding is a defect somebody thought of. "No blind spot
  demonstrated" means nobody has broken it the right way yet, which is why the three harnesses this
  milestone rewrote without a finding are recorded as exactly that rather than as clean.
- Five harnesses outside `crates/` were read by hand and are outside the machinery.
  `script/falsifications` walks `crates/` only (milestone 212 fixes that), so the two in
  `kernel/src/system_call.rs`, the two in `components/src/printenv.rs` and the one in
  `vendor/redoxfs/src/node.rs` were swept as prose and could not be swept as patches. All five are
  fine; the kernel pair are in fact the tree's model of the good shape, stating run arithmetic in
  `u128`.

## Follow-on

- **Milestone 212.** `script/falsifications` walks `crates/` only, so five harnesses (two in
  `kernel/src/system_call.rs`, two in `components/src/printenv.rs`, one in `vendor/redoxfs/src/node.rs`) were
  swept as prose and could not be swept as replayable patches. 212 landed first and widened the
  denominator under this lane.
- **Refused.** A lint for this defect. No check distinguishes "asserts through the function under
  test" from "legitimately asserts agreement", and agreeing with itself is sometimes the property
  you want, so a gate that flagged every such harness would be wrong more often than right. The
  output is eleven falsification patches and a worklist instead.
  This refusal is milestone 470 (design/roadmap/0470-a-gate-on-harness-quality.md), which carries it
  with the condition that would change it.
- **Recorded.** `design/roadmap/0211-self-referential-harnesses.md`: the rewrite is harder than the
  finding. Stating a property without the vocabulary the code uses is where specification work is
  expensive, and for some properties there may be no independent statement worth having.
- **Recorded.** `design/roadmap/0211-self-referential-harnesses.md`: the sweep's own output can rot,
  since a harness found fine today can become self-referential under a refactor and nothing will say
  so. The eleven findings are protected by §134's replayed patches; the 135 cleared harnesses carry
  no artifact at all.
- **Recorded.** `design/roadmap/0211-self-referential-harnesses.md`: eleven is a floor rather than a
  count. Each finding is a defect somebody thought of, and "no blind spot demonstrated" only means
  nobody has broken it the right way yet.

## Index row

Scopes `design/fatal-risks/README.md` risk 2, are the proofs load-bearing. 146 harnesses read, 11
measured blind, and measured is the word: for each one the pre-211 phrasing was run against a
defect in the function it stated its property through and observed to stay green, while the
rewrite goes red. All 11 rewritten, each carrying that patch as its §134 record; `script/falsifications` went 25 of 141 replayable to 33. Three kinds: the guard's own predicate,
the value's own producer, and an encoder round-tripped through its own decoder, which no
call-graph test can find. Three further harnesses were rewritten and recorded as **not** findings,
because no defect their old phrasing misses could be exhibited. Method and the per-harness table
in notes/falsification.md.
