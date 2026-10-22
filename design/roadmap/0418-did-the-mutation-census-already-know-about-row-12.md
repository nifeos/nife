---
status: NOT-STARTED
raised: 2026-09-16
promoted_from: did-the-mutation-census-already-know-about-row-12
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 418. Did the mutation census already know about row 12, and if so who read it

Promoted from the proposal
`did-the-mutation-census-already-know-about-row-12`, filed 2026-09-16 by milestone 307, which broke
`paging::x86_64::no_vtd_entry_ever_sets_a_reserved_bit` and found it could not fail. *(Number
provisional until the merge queue lands it.)*

This is a measurement a lane can take with tools that exist, and it wants a lane
rather than an hour because the answer is not guessable from the tree.

Premise re-checked 2026-09-19, and the title's own question is answerable by reading, which
changes the shape of the work rather than removing it. Three findings, none of which needed a
sweep:

- `VTD_ADDR_MASK` is a `const` initialized from a literal (`crates/paging/src/x86_64.rs:197`),
  so there is no function body to replace and no operator to flip. `cargo mutants` rewrites one
  function at a time, which `notes/mutation-testing.md` states in its own opening lines. So this
  block's central sentence, that a mutation of `VTD_ADDR_MASK` is precisely what `script/mutation`
  exists to try, **is false**. Neither branch of the dichotomy below is the answer: the two
  instruments do not overlap on this constant at all.
- **The 2026-08-03 baseline could not have known either way.**
  `crates/paging/src/x86_64.rs` was added on 2026-09-14 (merge of pull request #855), six weeks
  after the baseline ran. The `paging` triage in `notes/mutation-testing.md` that classifies every
  `|` to `^` in `table_entry`/`leaf_entry` as equivalent says "on both formats", and both formats
  there are aarch64 and Sv39; the VT-d encoder is a third and was not in that run.
- The 2026-09-14 census's `paging` survivors are among the 771 nobody has read, which is
  milestone 326's whole subject. So the first branch below, a census that surfaced something and
  nobody acted, is a general condition of that census rather than a finding about row 12.

What survives the check, and it is the deliverable this block was really asking for, is the
cross-instrument comparison. Run the census over the four crates carrying rows 1 to 18 and compare
its survivor list against milestone 307 (which assertion actually fires)'s per-row verdicts. A census asks whether a change
to the code is caught by something and 307 asked whether a named assertion can catch anything. That
work is real and unstarted. What should not be carried into it is the expectation that row 12 is its
exhibit.

## In brief

Milestone 307 found that row 12's proof stated all three of its assertions, and its `kani::assume`,
through `VTD_ADDR_MASK`. So widening that constant left the harness SUCCESSFUL, 0 of 45 failed
while every VT-d leaf carried a hardware-reserved bit.

A mutation of `VTD_ADDR_MASK` is precisely what `script/mutation` exists to try, and
`crates/paging` is a host crate already inside risk 3's census. So there are exactly two possibilities
and they call for different work:

- The census already surfaced this as a survivor and nobody acted on it. Then the finding is not
  about the harness at all. It is that a report nothing forces anyone to read is rung four, which is
  the failure `AGENTS.md` names more often than any other, and the fix is about the census's output
  rather than about `paging`.
- **The census did not surface it.** Then there is a gap in what the census mutates or in how it
  judges a survivor, in the crate holding three of the table's 26 claims, and that gap is worth its
  own work.

Either answer is worth more than the assumption, and the assumption a reader would make from
`design/fatal-risks/README.md` today is the second.

## What it would take

Run `script/mutation` against `crates/paging`, `crates/dma_validator`, `crates/capability_plan` and
`crates/capability`, which between them carry rows 1 to 18, and compare its survivor list against the
per-row verdicts milestone 307 recorded in `notes/confinement-claims.md`. The two instruments measure
different things and the comparison is the deliverable: a census asks whether a change to the code is
caught by *something*, and 307 asked whether a named assertion can catch *anything*. Where they
disagree, the disagreement is the finding.

`AGENTS.md`'s memory ceiling applies: a mutation sweep is never run beside lanes.

## What is blocked until it is answered

Nothing is blocked. What is at stake is whether `design/fatal-risks/README.md`'s risk 3 and risk 7 are
measuring overlapping ground or adjacent ground, which both entries currently assume without either
having checked.

## Index row

Milestone 307 found that `paging::x86_64::no_vtd_entry_ever_sets_a_reserved_bit` stated all three of
its assertions and its `kani::assume` through `VTD_ADDR_MASK`. So widening that constant left the
harness successful with 0 of 45 failed while every VT-d leaf carried a hardware-reserved bit. This
block asked whether the mutation census had already surfaced it, which would make the finding about
a report nobody is obliged to read rather than about the harness. Re-checked 2026-09-19, the
question resolves by reading: `VTD_ADDR_MASK` is a `const` literal and `cargo mutants` rewrites
functions, so the census has nothing to mutate there, and the file holding it postdates the baseline
run by six weeks. What remains is the comparison the block asks for between two instruments that
measure different things, the census over the crates carrying rows 1 to 18 against milestone 307's
per-row verdicts, with the disagreements as the deliverable.
