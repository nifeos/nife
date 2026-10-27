---
status: PROPOSED
raised: 2026-10-27
milestone_dependencies: 663, 924
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# A mutant that hangs is its own verdict, and a test that hangs fails instead

Raised 2026-10-27 (UTC) by `lane/mutation-gaps-plan`, the third of the three mutation-testing gaps
calef ruled closed that day. A timeout counts as neither caught nor missed, and three instruments
read it three ways. The same day a timeout stopped #2797 at the gate.

This replaces the rule milestone 326's follow-on and `notes/mutation-testing.md`'s caveats call
for, a `--report` rule that compares a timeout with the package's baseline test time. Telling a
deadlock from a slow test stops mattering once every hang has to become a failure. Milestone 663
(bound the host pass) stays separate and is the outer bound this proposal relies on. The companion
proposal `a-survivor-accepted-once-is-not-accepted-forever.md` builds the census-wide check this
one extends to timeouts.

## What a timeout is today

The 2026-10-26 census (run 40311212712) has 258 timeouts across 33 crates. The largest are
`device_tree_blob` 29, `grant_plan` 28, `documentation` 20, `system_log` 16, `designware_ethernet`
14, `swish` 12, `uefi_loader` 12, `machine_discovery` 11 and `capability_plan` 10. Those nine hold
152.

| instrument | what a timeout counts as |
|---|---|
| the census's `killed%` | a kill, on an August hand-check of 96 that nobody has repeated since |
| the weekly inflow check | nothing: it and the committed survivor lists read `missed.txt` only |
| the per-pull-request gate | an untriaged survivor, cleared by an `equivalent` or `gap` row |

Each timeout also costs the census its whole auto timeout, 28 to 52 seconds and sometimes two
minutes. At 45 seconds each, the 258 cost about three runner-hours of the census's 13.7.

## The case that raised it

#2797's gate reported `TIMEOUT crates/credentialer/src/lib.rs: replace + with - in Record::decode`
at its 52-second auto timeout. `decode` reads the Argon2 passes count `t` from four bytes at an
offset. Turning one `+` into `-` reads a salt byte into it, and the decoded cost asks for an
enormous number of passes. `a_record_round_trips_through_its_encoding` checks `back.cost == cost`
and fails at once. `a_decoded_record_still_answers_the_same_way` runs in the same test binary, and
it calls `verify` with that cost before asserting anything about it. That test runs Argon2 for
hours, so the binary times out and the named failure never reaches the verdict. That is the
documented shape: a deadlock in one test hides an assertion failure in another.

It also exposes a real limit. `Cost::new` refuses `t == 0` and bounds memory and lanes, but it puts
no maximum on passes. A stored record can therefore ask `verify` for any number of passes, which is
the denial of service on every login that its own doc comment guards against for lanes. Part 2
below is that fix.

## The decision: a timeout is its own class

A timeout is neither caught nor missed, and the reason is not caution. An equivalent mutant
behaves the same as the original. A mutant that does not terminate where the original does
behaves differently, observably, so no hang is equivalent. Counting it as caught claims that a test
noticed, and #2797 shows a hang can sit beside a real catch or hide a real limit. Counting it as
missed claims nothing noticed, yet milestone 741 found 29 mutants a Kani proof kills where the tests
only hang.

So the ledger and every instrument carry `hang` as its own outcome. An `equivalent` row never clears
a hang. A hang clears in one of two ways only. A change makes the mutant fail a named test, or a
`hang` row says why nothing can bound it and names its home. How the published rate reads this
class is calef's question on the pull request that carries this proposal.

## What a test that hangs has to do

In order, strongest first, which is AGENTS.md's ladder applied to a hang:

1. The code bounds its own work. A value read from input that sizes work has a maximum, like
   passes in `Cost::new`. A loop that waits on a device or a condition has an iteration cap or a
   deadline and returns an error at it. The mutant then produces an error a test asserts.
2. The test asserts the cheap, decisive fact before the expensive work that depends on it. In
   #2797's case, that means `assert_eq!(back.cost, cost)` before `verify`.
3. A test that must block runs its body on a worker with a deadline, and the deadline expiring is
   a named assertion failure. Five crates already carry their own copy of this; part 1 decides
   whether one shared test helper replaces them, and names it provisionally if it does.
4. Milestone 663's wall-clock bound on the host pass is the net under the other three. It turns a
   spinning test into a failed run rather than a lost one.

A `hang` row is for what none of these reaches: a doctest that calls a blocking API, as in
`jh7110_entropy`'s `Pool::take`.

## What done means

Measured on the first census after the last part lands:

- Every census timeout key has a row, and the weekly job fails on one that does not.
- The census's timeout count is at most its `hang` row count, and each `hang` row names a home.
- The nine crates above carry no timeout a rung-one, two or three change can turn into a failure.
- `Cost::new` refuses passes above a stated maximum, and the #2797 mutant fails a named test in
  well under a second.

## How the work splits

1. The mechanism. The survivor lists carry an outcome column, so the weekly job compares a timeout
   key from one census to the next. The inflow check and the census-wide check read `timeout.txt`
   beside `missed.txt`. The gate stops letting an `equivalent` row clear a timeout. `script/mutation
   --report` prints hangs as their own column. One lane, with selftest cases in `script/lint` for
   each rule.
2. `credentialer`'s passes bound and the test ordering, one small lane, which also clears #2797's
   survivor at its root.
3. One lane per crate, in the order above. Each runs only the timed-out functions with
   `script/mutation -p <crate>` and a function filter. A full per-crate run pays every timeout's
   clock, and `device_tree_blob`'s 29 alone are about 20 minutes of waiting. Each pull request stays
   inside one crate, so the gate's 20-minute cap is not at risk. Converting a hang is also what
   keeps that cap safe, since each hang on a changed line costs the gate its full timeout.
4. The remaining 24 crates hold 106 timeouts, in batches of a day's work each.

## What holds the gain

- The weekly job fails on a timeout with no row.
- The gate treats a timeout as untriaged unless a `hang` row clears it.
- A `hang` row without a home fails `script/lint`, like a `gap` row.
- Milestone 663's bound fails a host pass that spins.

## Cost

Four parts. Part 1 is one lane of `helpers/` and workflow work. Part 2 is a morning. Part 3 is
nine crate lanes, and part 4 a few more. The census gets cheaper as hangs convert, by up to the
three runner-hours above. The one standing cost is the rule itself: a new test that can hang
needs a bound before it merges, because the gate already refuses the timeout.

Reuse: the deadline-worker pattern five crates already carry, milestone 663's process-group
bound for the host pass, and `helpers/mutation_inflow.py` and `helpers/mutation_diff_gate.py` for
every rule. cargo-nextest's terminating slow-test timeout was considered and lost, for 663's
reasons: it is a dependency, and it does not run doctests, the case that hangs.

## Names

The `hang` outcome and disposition, and the survivor list's outcome column. Name: provisional,
minted by this proposal on 2026-10-27 (UTC). An architect names things.

## Index row

258 census timeouts count as kills in the published rate, as nothing in the inflow check and as
survivors at the gate. Proposed: a hang is its own outcome that only a bound or a reasoned `hang`
row clears. A test that can hang bounds the code, asserts first or runs under a deadline, with
`credentialer`'s unbounded passes as the first fix.
