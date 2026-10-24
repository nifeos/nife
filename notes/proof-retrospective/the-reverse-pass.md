# What the proofs did catch, and the reverse pass

*Name: provisional, minted 2026-10-11 (UTC) by lane/sixth-ten-longest-notes, after the parent's own
heading.*

The corpus in [`notes/proof-retrospective.md`](../proof-retrospective.md) is a list of escapes. This
is the other half, and it is the reason that note's finding is amber. Then comes the reverse pass,
the question that keeps a study of proofs from being able only to confirm.

## What the proofs did catch

Two real defects, both found at harness-writing time.

- `dtb::be32` and `be64`. The original `at + 4` was a bare add on a `usize` taken straight out of
  an untrusted blob. Trying to state `be32_is_total` made it fail, and the readers were hardened to a
  checked add returning `Truncated`. This is a panic on the kernel's boot path, reachable from
  firmware, in the first parser that runs on both ISAs. Real, and found by the attempt to prove.
- `pci::intx_irq`. The pin-0 case underflowed and panicked in debug builds. Hardened with
  saturating arithmetic while writing `intx_irq_is_total_and_bounded`. The comment records the honest
  scope: every caller checks `pin == 0` first, so this is defense in depth rather than a live hole.

One proof obligation closed, marked honestly as not a bug. `paging::domain::grant_pages` gained a
wrap refusal because `an_enumerated_page_lies_inside_the_grant` cannot compute the grant's limit
without it. The comment says out loud that the first draft of itself overstated this: reaching a
wrapped address needs a `size` within a page of `2^64` and the frame allocator would run dry tens of
orders of magnitude earlier. What the check buys is totality, not a fix. That paragraph is the model
for how a proof-driven change should be described.

One specification falsified by the prover. In `crates/glob`, Kani came back in 42 seconds with a
counterexample to the *harness*, not the code: with a fully symbolic class body, `[!y]` is already a
negated class, so "`[xy]` and `[!xy]` are complements" is false when `x` is `!`. `notes/verification.md`
keeps it because *"the reflex on a red harness is to suspect the code."* A prover that corrects the
person writing the specification is doing something no test can.

And the counterfactual that cannot be measured. Nothing that a shipped harness states has
afterwards turned out to be false. The capability model's twelve harnesses, the DMA validator's seven,
the untyped-region no-double-free pair, the address arithmetic, the IOMMU domain's page set: all of
these underwrite security claims that would otherwise rest on argument, and none has ever needed
correcting. That is either a strong result or an unfalsifiable one, and the reverse pass below is the
only thing that tells them apart.

## The reverse pass

Which harnesses prove a property that could plausibly have been false?

### Harnesses that could not have failed

Judgment, harness by harness, on the ones that look weakest.

`capability::subset_is_reflexive` is the clearest case. It proves `a.is_subset_of(a)` for every
`a`, and `is_subset_of` is `self.0 & !other.0 == 0`. Reflexivity is then `a & !a == 0`, a tautology of
the two operators. Work through the plausible ways to get `is_subset_of` wrong and none of them
break it: reversing the operands passes, writing it as `self.0 & other.0 == self.0` passes, using the
wrong mask constant passes. There is no implementation error this harness distinguishes from the
correct one. It is documented as *"the reflexive base case of the derivation order"*, which is a fair
description of what it is for and not a claim that it constrains anything.

Its siblings are not in this category. `subset_is_transitive` is the property that licenses a flat
check instead of a derivation-tree walk, and it is exactly the sort of thing a hand-rolled rights
check gets wrong. `from_bits_cannot_forge_a_right` quantifies over an attacker-controlled syscall
register. `derive_never_widens_rights` runs over the real `CapabilityTable::derive`.

Restated-implementation harnesses. `be32_reads_big_endian_when_in_bounds` asserts that an
in-bounds read equals `bytes[at..at+4]` most significant byte first, which is the function's body
written a second way. `align4_rounds_up_to_a_multiple_of_four` is one arithmetic identity. Neither is
worthless: a byte-order flip is a real error, and the assertion is not literally the same
expression. But both are much closer to a unit test that Kani happens to execute than to a proof of
something anyone could have got wrong at scale. Their siblings `be32_is_total` and `be64_is_total`
are where the value is, and they earned it by failing.

The parity family. Twenty-six of the 145 harnesses live in `crates/paging`, and six properties
appear three times each, once per ISA (`aarch64.rs`, `sv39.rs`, `x86_64.rs`). That is not padding: Sv39
has three levels where the other two have four, and the leaf codecs are genuinely different formats,
so `index_is_always_in_bounds` is a different theorem in each file and rule 5 requires all three. It
does mean the raw harness count over-represents distinct properties by about twelve, and anyone
quoting 145 as "145 things proved" is quoting 133 at best.

The pattern behind all three. The harnesses that could not have failed are the ones stated over a
single operator or a single expression. The ones that could are stated over a *composition*: a table,
a walk, a chain, a round trip, a second implementation. `gpt::crc32_matches_its_bitwise_definition` is
the shape done right, a table-driven implementation checked against a bitwise one, and
`overlap_is_exactly_sharing_a_block` says in its own comment that it is *"stated as the **definition**
(the intersection is non-empty) rather than as the implementation, so the proof is not the code
compared with itself."* That sentence is the whole reverse pass in one line, and the tree already
knew it.

### What actually separates a load-bearing harness from a decorative one

Not the property. The falsification, and the tree's own rule already says so:
*"A harness that cannot be made to fail is not evidence."*

The harnesses with a recorded falsification are the ones this study can vouch for without judgment.
Milestone 35's whole set was broken on purpose before being believed, and one falsification corrected
a claim in the code (soundness rested on `grant_pages` flooring, not on the partial-page guard the
comment pointed at). The calendar's two central properties were broken and both harnesses caught it in
under half a minute. `glob`'s complement claim was falsified by the prover itself. `memory_regions`'
loom side carries a named falsification witness.

That is a minority of the 145. No gate requires a falsification record, no naming convention marks
one, and there is no way to enumerate them except by reading. That is rung four behaving like rung
four.

The second separator is non-vacuity. An assumption or a bound can silently empty a harness's input
set and a vacuous harness reports `SUCCESSFUL`; `kani::cover!` is the one check that catches it. There
are 19 `cover!` sites, in four crates (calendar 7, paging 5, dma_validator 5, glob 2). The other
twenty harness crates have none. A harness that constrains its inputs with `kani::assume` and
carries no `cover!` is one whose input set nobody has confirmed is non-empty. There are many of
them: `jh7110_trng`'s three all assume, `timetable`'s five all assume, `network_time_protocol` and
`credential_protocol` assume throughout.

Not a claim that any of them is vacuous. A claim that nothing in this tree would say so if one
were, which is the same shape as every other finding here.
