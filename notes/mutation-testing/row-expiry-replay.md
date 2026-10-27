# Row expiry: a replay over the last 100 merged pull requests

The diff gate can require that a triage row for a survivor on a changed line change in the same pull request (`--base-ref` in `helpers/mutation_diff_gate.py`). The rule exists and stays off. `EXPIRE_ROWS` in `.github/workflows/mutation-gate.yml` holds the switch.

`python3 helpers/mutation_gate_replay.py --table` reproduces the numbers below. It does not run cargo-mutants, because 100 diffs would take hours of runner time. It reduces each merge's diff to the Rust functions it touches. It matches them by crate and function name to the survivors of the census of 2026-10-26. It then asks whether each survivor's triage row is identical at the merge's first parent and at the merge. The script's docstring lists its biases. A survivor that a later test killed, or a constant with no function name, drops out and pushes the count down. Name matching and a rustfmt-shaped function finder push it up.

The last 100 merged pull requests on `origin/main` at 21b3700f2 (#2705 to #2815):

| measure | pull requests |
|---|---|
| touch a Rust function in a crate | 43 |
| touch a function that holds a census survivor | 24 |
| already stopped by the existing rule (a survivor with no live row) | 8 |
| stopped by the row-expiry rule | 20 |

Those 20 would have re-read 117 rows. The median is 7 per pull request and the maximum is 20 (#2763, `designware_mobile_storage`). All 117 are `equivalent` rows: 60 or-versus-xor swaps, 29 comparisons and 28 of other shapes.

Why they stop: 19 of the 20 are naming sweeps (unit symbols, capability, min/max/os and per-driver names). A sweep renames identifiers inside function bodies and leaves the function names alone, so the survivor keys stay. The row was true before and stays true, and a `verified` bump attests to nothing. The twentieth, #2739, rewrote the file allocation table crate, and the rule exists for that case. Five of the 20 stop only on rows of the two noise shapes, which part 4 of the proposal removes through scoped exclusions.

One merge in five would stop, nearly all on mechanical sweeps. The rule works on the rewrite and costs a bulk edit on a rename. Two narrowings are open. A pull request whose diff changes only identifiers could be exempt, though that is hard to detect. Or the rule could switch on after part 4 shrinks the pile. An architect chooses. Until then the workflow leaves the rule off. The census-wide check, the columns and the baselines are on.
