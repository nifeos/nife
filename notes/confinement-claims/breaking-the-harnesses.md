# What breaking the claims found: the harnesses (milestone 202)

An appendix to [notes/confinement-claims.md](../confinement-claims.md). It holds what milestone 202
(every confinement test is a ritual until somebody breaks the confinement and watches it fail)
found when each claim's harness was broken on purpose.

## The count

Twenty-five Kani harnesses now carry a recorded patch that turns them red, up from six.
`script/falsifications --sweep` runs all twenty-five in about 30 seconds, and every one goes red.
Three results outweigh the count.

Milestone 305 (the six kernel confinement rows get a falsification a machine can replay) added the
kernel half on 2026-09-16, which milestone 202 could not. Ten kernel `#[test_case]`s now carry a
record, and `--sweep` replays each by booting one architecture. Its results are in
[breaking-the-kernel-tests.md](breaking-the-kernel-tests.md). Read them first if you read one. The
headline is a confinement test that stayed green under a patch that broke the thing it claims, and
had been unable to fail since milestone 41 (dead code: triage the suppressions).

## §31's headline sentence is not what catches a broken confinement

Row 20 is the roadmap's own worked example of §31 (the foreign-language seam). Map `WITNESS_RO`
read/write into the C component, rebuild, run, and the test must go red. It does. It does not go red
on the assertion anybody would name.

The obvious answer is the verdict equality, `assert_eq!(v[2], CONFINED, ...)`, which prints all four
bits including `read-only witness intact`. That assertion never runs. A component that is not
confined does not fault. A component that does not fault produces no death report. And `run_seam`
collects every report before the test inspects any of them, so the run stalls at the collection. The
witness check, which is the sentence §31 leads with, is reached only by an escape that faults
anyway.

And the first run of it failed for the wrong reason, which is the hazard milestone 202's block
names. `run_seam`'s blocking receive had nothing to take. So the break surfaced as a watchdog timeout
at 234 seconds reading `a livelock, not a lost wakeup`. Right answer, useless diagnostic: nothing in
it says the word confinement. `wait_for_report` (provisional name) now bounds that wait at 30 seconds
against a 90-second budget. The second run fails at report 4 of 12, with a sentence about what a
missing death report means.

## A proof can be blind to the predicate it is stated in, twice

`component_plan::a_plan_never_grants_a_right_the_declaration_did_not_ask_for` asserted
`p.caps()[i].1 == reqs.caps[i].direction.rights()`. That is stated *through* `rights()`, so a
`rights()` that adds `GRANT` to everything satisfies it. Only the explicit
`& abi::rights::GRANT == 0` beside it caught the defect. That is the same shape milestone 194 (the
falsification record, its lint, and the sweep that replays it) measured in
`capability::derive_never_widens_rights`, one crate over. It was the argument for keeping an
assertion that looks redundant.

Milestone 307 (which assertion actually fires when a confinement claim is broken) found that this
paragraph had inverted, and the inversion is a better lesson than the original. Milestone 211 (a
harness that states its property through the function under test cannot see that function break)
fixed the harness by writing the expected rights out in literals (`Direction::Serve => READ`,
`Direction::Use => WRITE`) instead of calling `rights()`. `READ` is `1 << 0`, `WRITE` is `1 << 1` and
`GRANT` is `1 << 2`. So that equality now *implies* both lines below it. `& GRANT == 0`, the assertion
this note credits as the only thing catching the defect, became the one that could not fail.

The rescue became the decoration. The note went on describing code that had changed, and nothing
gated the drift. The two implied lines are removed in 307, and the live assertion carries the
sentence. The argument for keeping a redundant-looking assertion survives, with the caveat that which
one is redundant moves when the other is repaired.

## Row 17's harness could not see the real defect

Row 17 was recorded `unfalsifiable` because no line of the crate can alias the driver's table and the
shadow. True, and the wrong defect: the bug a shadow copy prevents is a double fetch, validating
one load and copying a second. A harness that reads a fixed table makes the second load always
equal the first, so the double fetch stays green. Row 17's harness reads through
`ChainMem::racing`, a fresh symbolic value per load, and the double fetch is red. Its sibling harness
still reads a fixed table and stays green under it.
