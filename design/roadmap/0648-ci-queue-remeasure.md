---
status: NOT-STARTED
raised: 2026-09-24
promoted_from: ci-queue-remeasure
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 648. Re-measure CI queue waits by runner label, a week after the arm64 split

*Title and slug are drafts.*
Raised by milestone 587 (most CI jobs do not need an arm64 host),
which moved 15 of 18 jobs to `ubuntu-24.04` on one afternoon's evidence. It named this check as
what decides whether that evidence generalized. Name provisional, this file's alone.

Due on or after 2026-10-01; before then there is not a week of data.

## What to do

Re-run milestone 587's measurement over the week's `ci.yml` and `verify.yml` jobs. Measure created
to started per job, grouped by hour and by runner label, the same shape as that block's table.

- If the four arm64 jobs (`build + test`, `cpu matrix`, `re-falsify`, `prove the kernel on
  aarch64`) still wait a median over ten minutes in busy hours, the next lever is milestone 587's
  option D. That splits the x86_64 guest legs out of `build + test` into an x86_64 job, about ten
  minutes off the arm64 job.
- If the x86_64 jobs have started waiting the way arm64 did, the premise that x86_64 supply is
  looser rested on one afternoon. Revisit milestone 587's split rather than extending it.
- Otherwise record the numbers in milestone 587's block and retire this proposal.

## Index row

A week after milestone 587 moved most CI jobs to x86_64, measure whether the arm64 queue that remains, or the x86_64 one it moved to, is now the wait.
