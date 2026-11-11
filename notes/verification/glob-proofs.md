# The glob matcher, and the two things that made it tractable

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

An appendix to [notes/verification.md](../verification.md). It holds what proving the glob matcher
taught about the cost of a proof.

Six harnesses are in `crates/glob/src/lib.rs`, the pattern matcher of milestone 47 (navigation and
naming). See [glob.md](../glob.md) for the crate itself, which carries the harness table. The target
is a loop over two byte strings where the pattern is untrusted. So the property that matters is the
one BMC is best at: totality, meaning no panic and no hang on any input.

It also produced two findings worth having next to the calendar's
([calendar-proofs.md](calendar-proofs.md)). Both are about the same thing: the cost of a proof is
the shape of the code, not the size of the claim.

- One loop, not two, and the claim is the same. Finding a bracket expression's closing `]` in one
  loop and testing membership in another, nested inside the match loop Kani already unrolls, took
  one harness to 3.5 GB and twelve minutes before it was killed. Scanning the class once, deciding
  membership as it goes, removes a whole loop from the unrolling. It is less
  work at runtime too. That is rule 1 of DECISIONS §46 (thin primitives or whole subsystems) in one
  edit.
- An unwind bound too high is as expensive as a claim too big. These harnesses were first written
  with `#[kani::unwind(60)]`, picked from a loose algebraic bound. The measured worst case over the
  same domain is 10. A host test now enumerates that domain and pins the number. So the unwind
  bounds are derived from a measurement, rather than from arithmetic on a worst case that cannot
  happen. Every outer iteration charges at least one step. That is what makes the measured step
  count a sound upper bound for the iteration count.

Kani also falsified a *harness* here, rather than the code. With a fully symbolic class body, `[!y]`
is already a negated class. So "`[xy]` and `[!xy]` are complements" is false when `x` is `!`. It came
back in 42 seconds with the counterexample. It is worth recording, because the reflex on a red
harness is to suspect the code.

The cost, stated rather than discovered: about ten minutes of solver time for the six, which puts
`glob` second to `calendar` in this suite. Two thirds of it is the two harnesses that quantify over
a symbolic-length pattern *and* a symbolic-length name at once. That is the calendar's finding again
from a different direction: the expensive thing is not the property, it is the second symbolic
length. Cutting one harness's name bound from three bytes to two took it from 279s to 199s without
weakening it, because that harness's rule is a predicate on the name's first byte.
