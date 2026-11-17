---
status: PROPOSED
raised: 2026-11-17
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# `script/catch-up` reads the milestone blocks

Raised by the documentation audit of 2026-11-17 (UTC),
[the newcomer path](../../audit-reports/2026-11-17-the-newcomer-path.md), finding 1. Severity: high
for the newcomer path, because the README's Try it block offers this script as the way to learn
what changed, and it answers "nothing" with exit 0.

Reuse: `parse_blocks` in `script/catch-up` itself, and `helpers/roadmap_block.status_and_built`,
which it already uses. No new dependency.

## The problem

`parse_roadmap` in `script/catch-up` tries `design/roadmap/README.md` first and returns whatever
rows it finds there. That file still exists, but it is prose and holds no `| N |` row, so the
function returns an empty table and never reaches `parse_blocks`, the reader written for the block
files. The same happens at both ends of the window, so every milestone section prints as empty.

Run on `main` at `a4faa4614` with the default window (since 2026-11-10), the script prints
`MILESTONES THAT CHANGED STATUS (0)` and `MILESTONES NEWLY MINTED (0)`. A count over the block
files with `helpers/roadmap_block.status_and_built`, for the same two commits, gives 26 status
changes and 60 new blocks. A window that reaches back before the generated index was retired (for example
`--since 2026-09-01`) instead reports 208 `MILESTONE ROWS THAT DISAPPEARED`, because the old end
still has the table and the new end has none.

## The work

- `parse_roadmap` takes the index table only when it yields at least one row, and otherwise falls
  back to `parse_blocks`. A window whose old end predates the block files keeps the table read for
  that end, so "newly minted" stays exact across the change of format.
- A window that crosses the format change compares numbers and statuses, not the source each came
  from, so the retirement of the index does not read as 208 removals.
- A host test runs the script over two fixed commits of this repository's history, one window
  inside the block era and one across the change, and checks the counts it prints. The test fails
  when `parse_roadmap` returns an empty table for a tree that holds blocks.
