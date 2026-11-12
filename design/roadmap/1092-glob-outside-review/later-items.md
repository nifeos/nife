# P2: later

An appendix to [the milestone](../1092-glob-outside-review.md).

## P2: after the first tag

#### P2-1. `has_magic` and `literal` have no cost bound, and the doc says the work is bounded

The module doc says "the work is bounded by `cost_bound`" (`src/lib.rs` lines 8 to 9). Only `match_steps` is.
`has_magic` and `literal` decode each `[` by scanning to the closing `]` or the end, so a pattern of
n unterminated brackets costs about n²/2 byte reads in either function, with no counter. Harmless at
shell-word lengths, and still a claim the doc overstates. Change: say which function the bound
covers, and give the two scanners their own one-line bound in the doc. Two-way. S.

#### P2-2. `.*` matches `.` and `..`

`matches(b".*", b"..")` is `true` and the test at `src/lib.rs` line 506 pins it. That is historical shell
behavior, and bash 5.2 changed its default (`globskipdots`) so that `.*` no longer yields the two
entries; whether any nife enumeration ever offers `.` or `..` to the matcher stays unverified. Under this
crate's own rule (the default grants less), the matcher is not the place to decide it: the layer that
enumerates is. Change: the README states that `.` and `..` are ordinary names here and the caller's
enumeration decides whether to offer them; a note in `grant_plan` if its enumeration can yield
them. Two-way. S.

#### P2-3. `match_steps` returns a tuple and calls its own value unstable

`match_steps` returns `(bool, usize)` and its doc says the count "is not stable across changes to
this crate". A published function whose second value is explicitly unstable is a compatibility
statement no tool checks. Change: a named return (`Outcome { matched, steps }`) whose doc states the
one promise the crate makes about `steps`, that it is at most `cost_bound` of the two lengths; or
keep the tuple and state the promise in the README. Two-way before the first release; one-way
after. S.

#### P2-4. No fuzz target, for a crate that qualifies for the weekly opt-in

The crate has no `unsafe` and takes two untrusted byte strings, which is what the weekly fuzz run
is for, and `fuzz/Cargo.toml` names no target for it. The naive matcher that the exhaustive test
uses is the oracle a target wants. Change: `fuzz/fuzz_targets/glob.rs` over an arbitrary split of
the input into pattern and name, asserting no panic, `steps <= cost_bound`, agreement with the
naive matcher under both dot rules, and `has_magic` false implying `literal` round-trips. The naive
matcher moves behind `#[cfg(any(test, fuzzing))]` or into the target. M.

#### P2-5. Two harnesses quantify over one dot rule where both are cheap

`the_cost_never_exceeds_the_bound` and `no_magic_means_the_pattern_is_its_own_only_match` fix
`Dot::Ordinary`. Under `Dot::Special` the first is trivially stronger (a refused name costs zero
steps) and the second is the claim the shell's grant planner relies on with the default rule. Both
can take a symbolic `Dot` as `matching_is_total` does, at the cost of a second path through the
loop. The first also redefines its own `SHORT` bound instead of calling `any_lengths()`
(`src/lib.rs` lines 931 to 937). Two-way. S, plus solver time to re-measure.

#### P2-6. The recorded solver times no longer describe a run

`notes/glob/proofs.md` gives the four cheaper harnesses as 92, 57, 53 and 1 seconds and the total as
about ten minutes. One run here took the negated-class harness to 190 seconds under Kani 0.68.0 on
another host, and the star harness to 2. The figures are environment facts, not crate facts. Change:
`docs/proofs.md` keeps the reasoning (why three bytes, why one loop, why the unwind is 11) and drops
the per-harness seconds, or states them as one measurement with its Kani version. Two-way. S.

#### P2-7. `Elem` is an abbreviation

The private enum `Elem` (`src/lib.rs` line 257) is "element" with a decoder. The naming page's first
failure mode is an abbreviation that needs one; `spelled_out` does not list this word, so nothing
refuses it. Change: `Element`. Two-way. S.

#### P2-8. No benchmark behind the "polynomial" claim

Every comparable crate publishes a number; this one proves an inequality and asserts
`steps < 40 * NAME_LEN` in a test. A `benches/` with the three adversarial shapes (`a*a*...*b`,
`*???...?b`, `[[[[...`) and one ordinary pattern gives the README a throughput figure and catches a
regression the step count cannot see. M.
