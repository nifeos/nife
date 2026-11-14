---
status: NOT-STARTED
promoted_from: measure-the-reach-of-the-glob-and-calendar-proofs
raised: 2026-10-04
milestone_dependencies: 741
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 782. Measure the reach of the glob proofs

Raised by the risk-2 reach study (`lane/kani-reach`, milestone 741 (does a standing proof notice a
regression)) on 2026-10-04 (UTC). Title and slug are drafts.

The study measured 189 harnesses and skipped glob's 6 on cost. With `--reached-only` they were
estimated at 53 Kani CPU-hours, against 29 for the other packages combined. The estimate came
within 6% of elf's measured cost. glob's proofs are the slowest in `script/verify`'s table
(902 s), so they are also where a weak proof costs the most to keep.

## Done when

- glob is measured by `.github/workflows/kani-reach.yml` at `max-parallel: 4`, a few
  shards at a time so the shared runners stay free (`package:shards:only` runs one shard of n).
- Its rows are added to `notes/kani-reach-2026-10-04.md` and its CSV, or to a dated successor.

## Index row

The reach study skipped glob's 6 harnesses on cost, and they are the slowest proofs in the verify table. Measuring them at bounded parallelism completes the study and shows where the most expensive weak proofs would be.
