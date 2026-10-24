# The glob matcher: how the proofs were made tractable, and what enumeration proves instead

*Name: provisional.*

An appendix to [`notes/glob.md`](../glob.md), whose table lists the six Kani harnesses. This records
why the domain is three bytes, what had to change to reach it, what it costs, the defect Kani found
in a harness, and the host-test enumeration that carries the length-independent claims.

## Three bytes, measured rather than chosen

The road to three bytes is the interesting part. Kani unrolls loops. So the match loop enters the formula once per possible iteration and the cost grows
with the name length rather than staying flat. Four bytes each was tried and abandoned twice, at 3 GB
and past ten minutes without finishing. What three bytes buys is every byte *value*: 2^48
pattern/name pairs per harness. That reaches a closed class, an escape, a trailing backslash, an
unterminated bracket and two stars. What it does not buy is quantification over length.

Two things had to change to get there, and both are DECISIONS §46 (thin primitives or whole
subsystems) rule 1 in practice: restructure the code, do not weaken the claim.

- The bracket expression is scanned once, not twice. Finding the closing `]` in one loop and
  testing membership in another would give Kani two loops to unroll, both nested inside the match
  loop it is already unrolling. A single pass that decides membership as it goes removes a whole
  loop from the unrolling. It is also less work at runtime, which is the usual shape of these.
- The unwind bounds are measured, not guessed. Every outer iteration adds at least one to the
  step count. So the worst step count over a domain is a safe upper bound for the iteration count
  over it. `the_worst_case_over_the_proof_domain_is_what_the_unwind_bounds_are_set_from` enumerates
  the harnesses' own domain and pins the answer: 10 steps at three bytes, 17 at four. The unwind
  is 11. Guessing 20 instead cost nothing in correctness and a great deal in solver time. An
  unwind bound too high grows the formula for iterations that cannot happen.

## The cost

The honest cost is recorded, since a gate people skip is worse than none. The six harnesses take
about ten minutes of solver time. That makes this the largest single entry in `script/verify` after
`calendar`'s seven. Two thirds of it is the two harnesses that quantify over a symbolic-length
pattern and a symbolic-length name (199s and 186s). The other four are 92, 57, 53 and 1. The
lever that worked was cutting the dot rule's name bound from three bytes to two, which is sound
because that rule is a predicate on the name's first byte. The lever that did not work, measured and
recorded so nobody re-tries it, was restating a restriction as `kani::assume` instead of an early
`return`: 181s became 186s, which is noise.

## A defect Kani found in a harness

Kani found a real defect in a harness, which is worth recording because it was not in the code.
The negation-complement property failed in 42 seconds with a counterexample, and the counterexample
was right. With the class body fully symbolic, `[!y]` is not "the class of `!` and `y`", it is
*already* a negated class, so `[!!y]` is its complement rather than its double. The assumption that
excludes `!` and `^` at the head of the body is about what the two spellings *are*, not a weakening
of the claim.

## What enumeration proves instead

The length-independent claims are the host tests' job, and that is the `network_time_protocol`
lesson applied (see [ntp.md](../ntp.md)). A model checker is the tool for domains too big to
enumerate, not a better tool for domains that are not. Two places it decided the design here:

- Equivalence with exhaustive search. The property that would settle "is the greedy
  single-backtrack loop actually correct" is "it agrees with a naive matcher that tries everything".
  A solver is bad at that, because the reference is recursive and has to be unwound. Enumeration is
  perfect at it. It takes every pattern of length 0..=5 over an alphabet holding one representative of each
  syntactic role (`a`, `b`, `*`, `?`, `[`, `]`, `!`, `-`, `\`) against every name of length 0..=3
  over `a`, `b`, `-`. That is 66,430 patterns times 40 names, 2,657,200 comparisons, in 0.3 seconds. It is
  *complete* over that domain rather than bounded.

  The alphabet is the part that took thought. It is not "some characters". It is one representative
  of each thing the decoder can do, so the enumeration reaches unterminated brackets, `]`-first
  classes, `[a-]`, `[!ab]`, escaped metacharacters and trailing backslashes without anyone having
  thought to write them down. Length five rather than four because five is the shortest pattern that
  reaches a full range (`[a-b]`) and a full negated class (`[!ab]`).

  The reference shares `decode` with the real matcher on purpose. What the
  cross-check compares is therefore the search strategy and nothing else: if the two disagree, the
  greedy loop is wrong rather than the syntax being read two ways.

- The blowup itself. `cost_bound(3, 3)` is 285, and no matcher at all is slow enough to exceed
  that on three bytes. So the Kani harness is not where exponential backtracking would be caught.
  What it catches is an error in the *accounting*, which is what would otherwise let the big test
  pass while measuring the wrong thing. The blowup evidence is the host test. It runs
  `a*a*a*a*a*a*a*a*a*a*b` against 100,000 bytes of `a` and asserts the measured step count is under
  the bound computed before the match started. The naive reference in the same file would not finish
  that input before the heat death of anything. Overclaiming this in the harness comment and then
  working out the arithmetic is how the distinction got written down.

Both directions of the cross-check are pinned as non-vacuous. 15,956 of the 2,657,200 pairs match,
asserted, because an agreement test between two matchers that both said "no" to everything would
pass.
