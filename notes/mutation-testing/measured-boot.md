# `measured_boot`: five survivors proved equivalent

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

This appendix of [notes/mutation-testing.md](../mutation-testing.md) holds the 2026-09-03 re-run of
`measured_boot`, and the algebra proving its five survivors equivalent.

## 2026-09-03: `measured_boot` re-run, and its five survivors proved equivalent rather than argued

Milestone 246 (measured boot's refusal path is tested by nothing, and one mutant turns it off) moved
measured boot's load-or-refuse decision into this crate. So the crate was re-run whole: 151
mutants, 133 caught, 5 missed, 4 timeouts, 9 unviable.

### The new function is one mutant, and it is caught

`verdict` decides whether unmeasured code may run. On a function returning a struct, `cargo
mutants`' only operator replaces the body with `Default::default()`. Without a `Default` impl that
does not compile, so the first run scored it unviable and the tool said nothing about the line.

`Verdict` now derives `Default`. That value is both the fail-safe and exactly the dangerous wrong
answer here (an absence where there was a refusal). `cargo mutants --in-diff` over that lane's diff
went from *1 unviable, 0 tested* to *1 caught*. A crate can score 100% on a function the tool never
mutated: **an unviable mutant is a hole in the measurement, not a pass.**

### The five missed are provably equivalent

This moves the crate's row in [the baseline ledger](baseline-ledger.md) from "argued" to proved. An
equivalence verdict reached by reading is wrong about ten percent of the time (see
[scope and honest caveats](../mutation-testing.md#scope-and-honest-caveats)). These five are
algebraic identities, not readings:

- `Sha256::compress`, `ch = (e & f) ^ ((!e) & g)`, `^` to `|`. The two operands are disjoint by
  construction (`e` and `!e` cannot both be set in a bit position). `^` and `|` agree wherever the
  operands never both hold.
- `Sha256::compress`, `maj = (a & b) ^ (a & c) ^ (b & c)`, `^` to `|`, twice. Count the set inputs
  in one bit position. At 0 or 1 every pair-AND is 0, at 2 exactly one is 1, at 3 all three are 1.
  XOR and OR agree on all four counts (0, 0, 1, 1), so both spellings of `maj` compute the
  bit-majority.
- `parse_hex`, `(hi << 4) | lo`, `|` to `^`. `lo` is a nibble and `hi << 4` has its low four bits
  clear, so the operands are disjoint and the two operators agree.
- `Sha256::update`, `bytes.len() < want` to `<=`. In the equal case both arms yield `want`.

No test can kill any of the five, so nothing here is deferred and nothing is a gap. The four
timeouts are `update`'s loop-control mutants. They hang rather than lie, the same family recorded in
[the documentation appendix](documentation.md).
