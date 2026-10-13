---
status: NOT-STARTED
raised: 2026-09-30
promoted_from: the-graph-counts-the-names-calef-calls
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 701. The graph counts the names calef calls

*Title and slug are drafts.*
## What is being decided

Whether the weekly metrics grow a counted series for public function and method names, the surface
that has been calef's call since 2026-08-23 and that notes/project-metrics.md records as counted by
nothing. calef asked for it on 2026-09-30.

## The proposal

Two series, one mechanism:

1. `public_names`, the stock: total public function and method names per week, split the way the
   lines series splits (kernel crate versus the rest). A discontinuity like milestone 609 (the
   system tests leave the kernel crate) is then annotatable by the 623 (a bullet under the chart
   explains a cliff) machinery rather than argued about.
2. `provisional_names`, the worklist: how many of those names carry a provisional provenance
   block. This is the ratification queue with a number on it, measured weekly instead of
   remembered.

Both derive from one pass. The weekly workflow already produces rustdoc JSON for the doc-coverage
gates. A helper (outside `script/metrics`, which may not run cargo, per the 623 pattern) counts
public fns and methods from it and greps their doc comments for the provisional marker. It appends
one CSV row per week. `script/metrics` renders the chart from the CSV and nothing else.

## What it costs

One helper, one CSV, one chart, one workflow step. The data source already exists and already
runs. No new dependency; no parsing of Rust beyond what rustdoc does better.

## What it cannot see, said in advance

A count reads nothing about a name's quality; a ratified bad name counts as ratified. Provenance
markers can lie the way any prose can. And the stock series cannot attribute a move to a cause by
itself; that is what the 623 week-notes are for.

## Prior art in this tree

The four-kinds table in notes/project-metrics.md counts named things with provenance blocks;
interface-stability counts the flow of public items; this counts the stock. The 623 helper pattern
is the mechanism for anything the weekly workflow must derive outside `script/metrics`.

## Index row

No weekly series counts public function and method names, the surface calef has ruled on since 2026-08-23. Proposed: `public_names` for the stock and `provisional_names` for the ratification queue, charted weekly.
