---
status: NOT-STARTED
promoted_from: kernel-comments-in-the-linux-style
raised: 2026-10-15
milestone_dependencies: 860
decision_dependencies: 267
machine_requirements: none
specific_machine: none
needs_person: no
---
# 929. Comments in the Linux style across first-party Rust

This proposes applying Linux's commenting convention to all first-party Rust, `kernel/` first. Its
scope is ruled; the rule is not in force. It amends §267 (a comment states the constraint as it is
now) and the "Comments" section of `AGENTS.md`, so nothing here binds until calef rules on the rule.

## Linux's convention

`Documentation/process/coding-style.rst` and `scripts/checkpatch.pl` hold it:

1. A comment says what a function does and why, at its head. A comment inside a function body is
   discouraged; the function should be small and its working obvious.
2. A public function gets a kernel-doc header: a one-line summary, then `@arg:`, `Context:` (what
   locks and what context) and `Return:`. The published documentation is extracted from these.
3. Design lives in `Documentation/`, not in source.
4. History, credit and bug references go in commit trailers (`Fixes:`, `Link:`), never in comments.
5. An invariant is code where it can be: `lockdep_assert_held()`, `might_sleep()`, `BUILD_BUG_ON()`.
6. Two comments are demanded rather than discouraged: one on every memory barrier, naming what it
   pairs with.

This tree already has 4 (§267's second rule and its 2026-10-09 amendment), 5 where a lane has done
it, and 6 in its `// SAFETY:` discipline. Rule 2's place is taken by rustdoc. What it does not have
is rules 1 and 3, and the density they produce.

## The gap, measured

`notes/comment-cost-2026-10-07.md` measured comment lines per code line with one lexer across trees:
this kernel 0.92, Linux's `kernel/` 0.33, seL4 0.15, Redox's kernel 0.09 and Rust's `core` 1.22.

The same lexer (`notes/comment-cost-2026-10-07/rustlex.py`) counted whole-line comments in every
tracked `.rs` file outside `vendor/` on 2026-10-21 (UTC), at `2b0f59ac2`. "In body" is a proxy: a
plain `//` line indented 8 or more, other than `// SAFETY:`. The block columns use
`helpers/comment_block_ratchet.py`'s block definition and are "blocks / lines over the cap".

| directory | code | doc (`///`, `//!`) | plain (`//`) | comment per code | in body | over 20 | over 10 |
|---|---|---|---|---|---|---|---|
| `kernel` | 43,904 | 28,226 | 8,768 | 0.84 | 2,550 | 284 / 2,570 | 938 / 8,305 |
| `crates` | 105,390 | 50,051 | 7,972 | 0.55 | 3,402 | 483 / 4,876 | 1,124 / 12,557 |
| `components` | 17,766 | 7,518 | 3,577 | 0.62 | 1,296 | 94 / 1,063 | 198 / 2,447 |
| `system_tests` | 28,191 | 10,243 | 2,779 | 0.46 | 446 | 108 / 747 | 347 / 2,858 |
| `xtask` | 18,043 | 5,074 | 3,203 | 0.46 | 872 | 58 / 482 | 163 / 1,507 |
| the other 18 directories | 28,714 | 10,700 | 3,823 | 0.50 | 1,199 | 137 / 1,324 | 308 / 3,460 |

Two things hold in every directory. The bulk is item documentation, not narrative inside bodies:
in `kernel/` doc comments are 28,226 lines against 8,768 plain, so a rule about function bodies
alone would move at most the plain lines and leave `kernel/` near 0.65. The long doc block is the
real target, and it is Linux's rule 3: a design essay on an item belongs in the note for its
subsystem. And the cap in force, 40 lines, is met everywhere: no directory has a block over it.

## The rule proposed, for all first-party Rust

The rule is one rule. The stage order below is about effort and the merge queue, and exempts no
directory.

1. An item's doc comment is in the kernel-doc shape. It gives a summary sentence, then the
   arguments, what it holds and returns or refuses, with a link to its note. A design argument goes
   in the note. A `Context:` line (what locks it takes, whether it may run in interrupt context)
   appears only on code that holds locks or runs in interrupt context.
2. A comment inside a function body states a constraint the code cannot show: an ordering, a
   `SAFETY` argument, a hardware quirk. Narrative, worked examples and "how this works" move to the
   item's doc comment or the note.
3. An invariant a comment states as "the caller must..." becomes an assertion or a type where one
   fits, as `NameCopy` did for milestone 825 (a hostile client races the file server's name window).

Everything else stays: §267's rules, `SAFETY` comments, and `missing_docs`. The newcomer test in
`AGENTS.md` is met by the note a doc comment links to, which is where Linux meets it too.

The stages, in order:

1. `kernel/`.
2. `crates/`, then `components/`.
3. `system_tests/` and `xtask/`, then every other non-`vendor/` directory that holds Rust.

## The mechanism

- `helpers/comment_block_ratchet.py` takes a cap per directory instead of one cap for the tree. In
  stage order a directory's cap steps from 40 to 20 and then to 10, each step a `--bank` with the
  over-cap blocks listed and held. A directory not yet started stays at 40.
- A second count in the same helper: plain comment lines inside function bodies, held per directory
  by a ceiling that only falls, like `helpers/line_citations.py`. A directory's ceiling starts when
  its stage starts. `// SAFETY:` lines are not counted.
- Both are charted in `notes/project-metrics.md` with the existing chart code, one series per
  directory.

Reuse: `helpers/comment_block_ratchet.py`'s block definition, baseline and `--bank`, taken; the
lexer in `notes/comment-cost-2026-10-07/rustlex.py`, taken for finding function bodies;
`script/metrics`' ratchet chart, taken. What is written is the in-body count and the per-directory
cap.

## Cost

Lines judged are the lines over a 10-line cap plus the in-body proxy. Each is cut, moved to a note,
or turned into code. At 500 to 1,000 lines a reviewable pull request (assumed, not measured), at
most two in the queue:

| stage | blocks over 10 | lines over 10 | in body | lines judged | paydown pull requests |
|---|---|---|---|---|---|
| 1 `kernel/` | 938 | 8,305 | 2,550 | 10,855 | 11 to 22 |
| 2 `crates/`, `components/` | 1,322 | 15,004 | 4,698 | 19,702 | 20 to 40 |
| 3 `system_tests/`, `xtask/`, the rest | 818 | 7,825 | 2,517 | 10,342 | 11 to 21 |
| total | 3,078 | 31,134 | 9,765 | 40,899 | 42 to 83 |

- CI: a paydown pull request runs the full suite unless the classifier has a comment-only class.
  `helpers/prose_only.py` skips the heavy jobs only for Markdown under `notes/`, `design/`,
  `briefs/` or the root that no code reads. A `.rs` file is never prose to it, so a diff that
  changes only comments runs everything. Its comment stripping decides only whether a file
  *reads* a note, not whether a changed file is prose. `ci.yml` alone took 119 to 129
  runner-minutes over 26 jobs in three merge-group runs on 2026-10-22 (UTC), of which the gate,
  `lint` and `rustfmt` were 7; `verify.yml`'s Kani shards come on top. A comment-only class
  (#2546) would leave a paydown pull request the gate, `rustfmt`, `lint` (clippy and rustdoc) and
  the host tests of the crates it touches.
- The return: comments are 57% of the bytes in this tree's Rust, and every agent that opens a file
  reads all of them. The sweep moves `kernel/` from 0.84 comment lines per code line to about 0.6,
  and the essays it moves are read only by an agent that follows the link.
- The in-body proxy over-counts where a plain comment is indented inside a nested `impl` or `mod`,
  and the real ceiling is set by the lexer when the helper's count is written.

The end state is not Linux's 0.33, and this proposal does not aim at it. Reaching it would mean
cutting the item documentation the newcomer test rests on, not moving it.

## Done means

- §267 is amended in calef's words, and `AGENTS.md`'s "Comments" section says the same.
- Every directory's cap is 10, every block over it is gone or listed, and each directory has an
  in-body ceiling, all in `script/lint` with selftests.
- The counts are on `notes/project-metrics.md`, per directory.

*Name: provisional. "Kernel comments in the Linux style" no longer fits the scope; "Comments in the
Linux style across first-party Rust" says it. The file keeps its number-and-slug name because
`notes/comment-cost-2026-10-07.md` cites it.*

## Index row

Linux comments at a function's head and keeps design in `Documentation/`; this tree's comment density runs from 0.46 to 0.84 per code line against Linux's 0.33. The proposal measures the gap and the cost of a Linux-style rule for all first-party Rust, `kernel/` first, and the rule is not in force until calef rules.
