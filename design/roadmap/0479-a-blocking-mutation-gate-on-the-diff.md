---
status: REFUSED
raised: 2026-09-20
refused_by: 438, 448
---
# 479. A blocking `--in-diff` mutation gate

Refused by milestone 438 (design/roadmap/0438-a-mutation-gate-on-the-diff.md), which records it in
its Follow-on. Milestone 448 (design/roadmap/0448-a-refusal-gets-a-number.md) gives a refusal that
names work a number, a status and a condition that would change it.

## The refusal, in its own words

From '438. Would a diff-scoped mutation check have caught the 55, and what would it cost', under `##
Follow-on`:

> A blocking `--in-diff` gate, per this block's recommendation and the ruling that ends it at
> measurement 1. Nothing is switched on and no workflow, `script/ci-build` row or ruleset entry
> was touched.
>
> -- design/roadmap/0438-a-mutation-gate-on-the-diff.md

## Why it is here rather than only there

Cost was not the objection: four seconds on a documentation change is cheaper than several checks
already required. The objection is evidence. The case the gate was built on is not a case it would
have caught, and the experiment that would settle it runs against pull requests this clone does not
contain. The block's own rule ends the work at measurement 1, and nothing was switched on: no
workflow, no `script/ci-build` row, no ruleset entry.

## Revisit

- **Condition.** A second measurement, on the experiment measurement 1 could not run. The block is
  explicit that the mechanism is not refuted and the story is, so what would change this is evidence
  about pull requests as they land rather than about the diff in hand.

  MET on 2026-09-21, and this is the first refusal in the tree whose condition has come true.
  Milestone 517 (what fraction of survivor growth arrives on lines a pull request touched) ran
  exactly that experiment across the six weeks between the baseline and the census: 629 of 771
  survivors sit on lines a merged pull request wrote, against 142 on older lines of which one
  is a genuine regression. Adoption would have meant 62 of 761 pull requests (8.1%) carrying
  untriaged survivors, median 6 each.

  calef ruled on 2026-09-21: it stays refused, with the condition met and the measurement in
  hand. The evidential objection this condition named is answered; the other two that milestone 438
  (would a diff-scoped mutation check have caught the 55) raised are not, and they are what the
  refusal now rests on.

- **Condition.** *(Restated 2026-09-21, for what is actually missing now that the evidence question
  is settled.)* Two things, either of which alone leaves this gate worth less than it costs.

  **The corpus does not contain the kernel.** `kernel/**` and `components/**` generate no mutants,
  so a kernel change passes this gate by construction, and it would be strictest exactly where the
  tree's risk is smallest. Milestone 517 (what fraction of survivor growth arrives on lines a pull
  request touched) priced closing that at roughly 500 runner-hours per census against 52 minutes
  today, so a kernel *census* is refused on arithmetic. A kernel *diff-scoped* check is minutes per
  pull request and is blocked on scaffolding, because `cargo test -p kernel` cannot run from a
  cargo-mutants build directory: `xtask/src/suite.rs` builds the initrd, the images and the disks
  before the boot.

  And it has no rule for an artifact of the exclusion mechanism. In milestone 438 (would a
  diff-scoped mutation check have caught the 55)'s own sample, two of six survivors were artifacts
  rather than missing tests, on the pull request that added machine-checked proofs to that crate.
  A gate whose first act is to block a proof lane teaches the wrong thing about proofs, and
  nothing in the mechanism tells the two cases apart today.

  **What does not reopen it: more evidence that survivors arrive on touched lines.** That is settled
  at 629 against one, and `design/fatal-risks/README.md`'s risk 3 carries a green condition built on it
  which was deliberately written so the verdict does not depend on this gate existing. The triage it
  asks for is measurable after the merge, weekly, without blocking anybody. What this gate would
  change is when the triage happens, not whether.

## Index row

A gate built, measured and then deliberately not switched on, with the ruling recorded at the
measurement that ended it. The condition is a further measurement rather than a decision, and the
block already says which experiment it would be.
