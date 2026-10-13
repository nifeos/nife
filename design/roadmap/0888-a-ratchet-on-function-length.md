---
status: BUILT
raised: 2026-10-09
built: 2026-10-12
promoted_from: a-ratchet-on-function-length
milestone_dependencies: none
decision_dependencies: 266, 283
machine_requirements: none
specific_machine: none
needs_person: no
---
# 888. A ratchet on function length, beside the one on file length

*(The title and slug are provisional.)*

One of five lint milestones. Its numbers are in [notes/lint-census-2026-10-09.md](../../notes/lint-census-2026-10-09.md).
It reuses the lint ratchet specified in
[the panic proposal](0896-panics-in-the-kernel-and-the-parsers-are-counted-and-fall.md).

Reuse: clippy's `too_many_lines`, and `helpers/file_length_ratchet.py`'s rules from milestone 841
(a ratchet on Rust file length). Nothing is written but the row key below.

## The rulings, 2026-10-09 (UTC)

calef ruled on two of this proposal's questions on 2026-10-09 (UTC).

- Open question 1, the threshold: 100 code lines. Every function over it today is baselined.
- Open question 4: `kernel_main` is split by boot phase. Done criterion 4 records it.

## The problem

§266 (a Rust source file stays under 2,000 lines) bounds a file so a reader can load it in one
read. It names a per-function limit as "a different problem, and not ruled out", from memory and
unmeasured. A long function costs the same reader the same way at a smaller scale: one body to hold
in mind, with no seam to stop at.

Measured over the note's non-test scope with `too_many_lines` at a threshold of 0, so clippy
reports every function. Its unit is code lines; blank and comment lines are not counted.

| | functions | p50 | p90 | p95 | p99 | max | over 100 | over 150 | over 200 |
|---|---|---|---|---|---|---|---|---|---|
| in scope | 5,064 | 5 | 26 | 41 | 89 | 977 | 39 | 14 | 10 |
| kernel | 1,619 | 5 | 30 | 45 | 86 | 977 | 8 | 3 | 3 |

The long tail is a handful of boot and plan functions:

| function | file | code lines |
|---|---|---|
| `kernel_main` | `kernel/src/lib.rs` | 977 |
| `boot` | `crates/system_initializer/src/lib.rs` | 847 |
| `spawn_service` | `crates/system_initializer/src/lib.rs` | 545 |
| `Prog::manifest` | `crates/grant_plan/src/lib.rs` | 488 |
| `boot_progenitor` | `kernel/src/user.rs` | 404 |
| `invoke` | `kernel/src/syscall.rs` | 222 |

## Complexity, measured and refused

The request also asked for `cognitive_complexity`. Clippy's own documentation on this toolchain
refuses it: "We used to think it measured how hard a method is to understand", and it is left in
`restriction` "so as to not mislead users into using this lint as a measurement tool". It names
`excessive_nesting` and `too_many_lines` instead. For the record, its distribution here is p50 1,
p99 10 and max 80, with 9 functions over its default of 25.

`excessive_nesting` was run on the host and aarch64 configurations. At a threshold of 4 it finds
243 blocks in 56 files; at 6, 10 blocks in 4 files. It measures something length does not, and it
is open question 3 rather than part of the recommendation.

## The mechanism, and its rung

Rung two, in the file-length ratchet's shape, through the lint ratchet. One difference: a row is
keyed by file and function name, not by file, and its number is the function's code lines. The
rules are the file-length ratchet's. A function not on the list fails over the threshold. A listed
function may not grow against the merge base or past its row. `--bank` lowers a row and nothing
raises one. A renamed function moves its row.

The threshold is 100, clippy's default (ruled 2026-10-09 UTC). The tree's p99 is 89, so 100 leaves 99% of
functions untouched, and the baseline starts at 39 rows, 8 of them in the kernel. 150 would list 14.

§266's warning carries over. A line ceiling rewards deleting comments, and `too_many_lines` does
not count comments, so that incentive is absent here. The remaining one is splitting a function at
an arbitrary line. The failure message says the remedy is a seam, as the file-length ratchet's
does.

## Done when

1. `too_many_lines` runs in the lint ratchet at 100 code lines, with a selftest.
2. The baseline holds every function over 100 today, keyed by file and name, 39 rows at the census.
3. Its dashboard row reads the tree, as milestone 841's row does, so the long tail is visible
   without opening the baseline.
4. `kernel_main` is split by boot phase, one function per phase, and its row is banked down or
   removed. No recorded exception at the site is needed.

5. A chart drives the long functions out, modeled on the file-size chart (calef, 2026-10-11 UTC).
   `script/metrics` records the share of in-scope code lines inside functions over 100, with the
   count and the summed excess, backfilled weekly, and plots the share with a 10-week trailing mean.

Burning the rest of the list down is not this milestone. Each other long function is its own split,
as file splits became their own blocks after milestone 841.

## Where it stands, 2026-10-11 (UTC)

Criteria 1, 2, 3 and 5 are built. Criterion 4 was held, and calef ruled on 2026-10-12 (UTC) that
milestone 876 completes it. It did, on 2026-10-12 (UTC): `kernel_main` is 59 code lines in
`kernel/src/boot.rs`, and its row is gone from the baseline.

- The lint ratchet is `helpers/lint_ratchet.py`, built to the panic proposal's specification so the
  other lint proposals register a lint rather than write a gate. `script/lint` runs one more clippy
  pass per census configuration at `-W`, in about 20 seconds. One lint is registered.
- The row value is `helpers/function_length.py`'s text replica of `too_many_lines`, which
  `script/metrics` needs because it may not build. At a threshold of 0 it matched clippy on all
  5,181 functions both saw. The gate fails on any difference.
- The baseline, `design/lint-baseline.tsv`, holds 46 rows, not 39. The text measure reaches 7 long
  functions in kernel build modes and the UEFI loader that no census clippy pass compiles.
- The chart is `notes/project-metrics/function-length.svg`, backfilled from 2026W29 with no build.
  At 2026W41 the share is 9.1%: 46 functions, 4,566 code lines past the limit. The definition is
  [the register's appendix](../../notes/register-of-measures/function-length.md).
- Every name here is provisional: both helpers, the baseline, the measure, its columns and the chart.

**Criterion 4 was held on §266, and milestone 876 landed it.** The split was built and passes clippy
on all three targets, on the branch `lane/888-kernel-main-split-held`. `kernel_main` falls from 977
code lines to 59, across 22 functions, the largest 90 (`tour_el0`). But `kernel/src/lib.rs` grows
from 2,984 lines to 3,048, and its §266 row is 2,984. The phases need signatures, calls and `cfg`
lines that the file has no room for. Milestone 876 (the kernel crate root is split along its seams)
makes room by moving the two architecture boots into child modules, and its cut and names are
calef's. The phases are the contents those modules would take, under the names 876 proposes. calef's
ruling of 2026-10-12 (UTC): this milestone lands the ratchet and the chart, and 876's lane starts
from the held branch.

## Open questions for an architect

1. Ruled 2026-10-09 (UTC): 100, with the 39 functions over it baselined. The options were 100,
   150 (14 rows) and 200 (10).
2. The unit. §266 counts physical lines and this counts code lines, so a 120-line function with 30
   lines of comment passes at 100. Code lines is the recommendation, since it removes the comment
   incentive; it does mean the two ratchets disagree on what a line is.
3. `excessive_nesting` at a threshold of 6 (10 blocks), as a second count in the same ratchet?
4. Ruled 2026-10-09 (UTC): `kernel_main` is split by boot phase (done criterion 4). The question
   was whether to split it or keep it as one boot sequence under a recorded exception.

Questions 2 and 3, with the panic proposal's question 5, were ruled "A" by calef on 2026-10-12
(UTC): code lines, no nesting measure, and one baseline file. §283 (a function is held to 100 code
lines by a lint ratchet) records them.

## Where it sits in the ranking

The customer path is vacant, so the tie breaks toward the fatal risks, and this serves none of
them directly. It serves the third principle: a newcomer reading `kernel_main` meets 977 code
lines with no seam. It ranks fifth of the five.

## Follow-on

- **Done.** Criterion 4, the `kernel_main` split, landed with milestone 876 (the kernel crate root
  is split along its seams), built from `lane/888-kernel-main-split-held` (calef, 2026-10-12 UTC).
  `kernel_main` is 59 code lines in `kernel/src/boot.rs`; its row is removed from
  `design/lint-baseline.tsv`, and `kernel/src/lib.rs` is an index with no §266 row.
- **Recorded.** The unit, the nesting lint and one baseline file were ruled "A" by calef on
  2026-10-12 (UTC), as recommended: code lines, no nesting measure, one file. §283 (a function is
  held to 100 code lines by a lint ratchet) records it,
  `design/decisions/0283-a-function-is-held-to-100-code-lines-by-a-lint-ratchet.md`.

## Index row

Clippy's `too_many_lines` at 100 code lines, held by a lint ratchet whose baseline only falls, one
row per function. It builds the shared lint ratchet the panic proposal specified, so the other lint
proposals register a lint instead of writing a gate. A chart beside it tracks the share of code
lines in long functions, so the splits that drive the list down can be seen working.
