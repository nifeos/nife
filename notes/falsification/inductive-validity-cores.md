# Inductive Validity Cores: checked first, and they are not available

*Name: provisional. Named after the main page's own heading.*

An appendix to [`notes/falsification.md`](../falsification.md), which states the conclusion.

§134 named IVCs as the cheaper family and said the implementing milestone should look before
building anything, because a yes would make most of the section unnecessary. The answer is no, and
the reason is structural rather than a feature Kani has not got round to.

An IVC is the minimal set of model elements a proof actually needed. It falls out of the
unsatisfiable induction step of an inductive proof, which is why the technique belongs to
k-induction and IC3 model checkers over Lustre models: JKind, Kind2, Sally. Kani is a bounded
model checker. It unrolls loops to a depth, compiles the whole thing to one SAT formula, and hands
it to CaDiCaL. A bounded proof has no induction step, so there is nothing for a core to be minimal
with respect to.

What was actually looked at, on 2026-08-31, against Kani 0.67.0 and the CBMC 6.8.0 it ships:

| looked at | what it gives | why it is not an IVC |
|---|---|---|
| `cargo kani --coverage` | source-based **reachability** coverage: which regions symbolic execution touched | catches some vacuity (an unreachable region), no tautology at all. Kani's own RFC-0011 calls it reachability analysis and not proof-theoretic core extraction |
| `cbmc --slice-formula` | drops assignments unrelated to the property | a solver optimization that reports nothing to the user |
| `cbmc --cover`, `--symex-coverage-report` | test-suite generation against a coverage criterion | again reachability, and about generating tests rather than explaining a proof |
| anything unsat-core shaped in either tool | nothing. `cbmc --help` has no such flag; `cargo kani --help` has no such flag | |

So §134's option C is the affordable option that exists, and this is it.
