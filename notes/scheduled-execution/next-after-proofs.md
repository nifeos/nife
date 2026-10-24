# `next_after`'s proofs: a measurement and a gap

*Name: provisional, minted 2026-10-11 (UTC) by lane/final-longest-notes, for what the parent said
about `next_after`'s Kani harnesses. calef names files.*

An appendix of [scheduled execution](../scheduled-execution.md), for its section "The
arithmetic, and the decision inside it", under §212 (a prose budget). The
parent keeps the three properties and why Kani holds them. This file keeps the two things about
`crates/timetable/src/proofs.rs` worth knowing before anyone edits `next_after`.

## A measurement: one operator, three orders of magnitude

The function reaches its answer by snapping `now` back onto the beat
(`now - (now - prev) % period`). It does not count the periods that went by
(`first + (gap / period + 1) * period`). Both are correct, and the host tests do not tell them
apart. But the counting form contains a 64-by-64 multiplication, which is an enormous thing to hand
a SAT solver. CBMC was still grinding on one harness after ten minutes. The snapping form proves in
about a second.

That is a three-orders-of-magnitude difference bought by removing one operator. It is the kind of
thing the verification path notices and a test suite never would.

## A gap: phase preservation is tested, not proved

This is stated because a reader would otherwise assume it is covered. Phase preservation is the
one law that is host-tested rather than machine-checked.

Every way of writing the congruence needs a modulo of a *computed* value on top of the one inside
`next_after`. A second 64-bit modulo is where CBMC stops finishing. The direct spelling, the
cheaper one, and the cheaper one bounded to `1 << 32` all failed to return. A harness bounded far
enough down to finish would read as proved while covering a range no schedule lives in, which is
worse than shipping none.

`next_after_is_strictly_in_the_future_and_keeps_its_phase` samples the law up to `u64::MAX / 4`
instead. The implementation's shape is the real defense: the phase is not computed and then
preserved, it is the only thing that expression can produce.
