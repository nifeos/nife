---
status: BUILT
raised: 2026-08-30
built: 2026-08-30
---
# 191. Did the proofs catch the bugs? A retrospective of every real defect against the harness that should have found it

Built. This is an experiment from the fatal-risk sweep (design/fatal-risks/README.md), recorded
there as `RUN, 2026-08-30. AMBER`; `notes/proof-retrospective.md` is the study.

In brief. DECISIONS §14 (a verified-Rust capability microkernel) promises a verified core. There are 112+ Kani
harnesses and `notes/verification.md` explains how they work. Nothing in this tree asks whether
they caught anything. This milestone answers that against the only evidence that cannot be
arranged: the project's own record of real defects.

## The question, stated so it can come back red

For every real bug this project has found, could a proof have caught it, and did one exist?

A red result is a finding that the harnesses cluster on pure algorithmic properties while every
defect that actually cost time was a concurrency, hardware-contract, or integration bug that no
proof was ever positioned to see. That would mean the verification claim is real but narrow, and
narrow in the direction that does not matter, which is a serious wound to half the thesis.

A green result is at least as valuable and is the more likely one: proofs caught a class of thing
early enough that it never became a bug, which is invisible in a defect list precisely because it
worked. That asymmetry is the hard part of this milestone, and the method below is what keeps it
from being a study that can only confirm.

## The method, because the shape decides whether the answer means anything

One pass, four fixed questions per defect, the same discipline `notes/arch-audit.md` used and
milestone 187 (read the x86_64 arch tree through the lens the first arch audit used) is repeating:

1. What was the defect, stated as a property that was false? Not "the board hung", but the
   invariant that did not hold.
2. Was that property provable at all? Some are not: a property about what real silicon does with
   a store buffer is not a property of our source.
3. Did a harness exist that covered it? If yes and it passed anyway, that is the most
   interesting outcome on this list and deserves its own writeup.
4. What would it have cost to have one? This is the question that turns the study into a
   worklist rather than a scoreboard.

And one pass in the other direction, which is what makes the result honest: walk the harnesses
and ask which ones constrain something that could plausibly have gone wrong, and which prove a
property that could not have been false. A harness over an algorithm nobody could have written
incorrectly is a passing check that buys nothing, and counting it is how a verification claim
inflates.

## The corpus, which already exists and is unusually good

This project writes its failures down, so the defect history is real rather than reconstructed:

- The load-sensitive assertions (notes/load-sensitive-assertions.md), including the run that
  went red on a clean kernel.
- The arch audit's own bug class (notes/arch-audit.md): state staged in single-copy hardware
  registers across more than one instruction while an exception can land in the middle.
- The FS-server stack bug, the PLIC hart lottery, the timer drift (notes/instruction-clock.md),
  the two-core crash (milestone 161's lane), the `std-src` toolchain race, and the record-level
  re-run that corrected stale counts (notes/fs-server.md).
- The nine misrecorded roadmap statuses and the fabricated block quote that survived twelve days
  of gates. These are not code defects and they belong in the study anyway, because they are the
  same question one level out: what did the mechanisms fail to see?

## Why this is first on the fatal-risk list

It costs an afternoon, needs no hardware, blocks nothing, and aims at half the thesis. Every other
experiment on that list needs a lane, a board, or both. **A cheap test that can return red is worth
more than an expensive one that probably will not**, and this is the cheapest one available.

## BUGS

- A retrospective cannot prove a counterfactual. "A proof would have caught this" is a judgment,
  and the study should mark each one as such rather than presenting it as measurement.
- Survivorship runs both ways and the second pass only partly fixes it. Bugs that proofs
  prevented never entered the record, and bugs nobody has found yet are not in the corpus either.
- It has no gate and produces no artifact the build checks. Its output is a note and, probably, a
  worklist of harnesses worth writing; nothing stops that worklist from going the way milestone 94's
  inventory went.

## Follow-on

- **Done.** `notes/proof-retrospective.md`, pull request #589, merged 2026-08-30.
- **Milestone 193.** Put `kernel/src` within reach of the prover. This study's red half is one line
  of `script/verify`'s own header (`cargo kani -p <crate>` never compiles the kernel), so the
  worklist's first item is the gate that closes it.
- **Milestone 197.** `user/` and `xtask` are out of reach of the prover, which is the second half of
  the same finding and which half-refuted its own premise when it ran.
- **Recorded.** The study's own limits are in `notes/proof-retrospective.md`'s `BUGS`: ten of the
  eighteen corpus entries are single-sourced, the reverse pass is a reading rather than a
  measurement, and the counts were taken at base `f1f138a8` and will drift.

## Index row

From the fatal-risk sweep (design/fatal-risks/README.md), where it ranks first: an afternoon, no
hardware, aimed at half of DECISIONS §14's thesis. 112+ Kani harnesses exist and nothing asks
whether they caught anything. Four fixed questions per defect over the tree's own recorded failures,
plus a second pass over the harnesses asking which prove a property that could plausibly have been
false, which is what stops a study that can only confirm.
