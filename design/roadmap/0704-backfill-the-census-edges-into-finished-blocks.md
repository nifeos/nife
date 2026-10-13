---
status: NOT-STARTED
raised: 2026-10-02
promoted_from: backfill-the-census-edges-into-finished-blocks
milestone_dependencies: 596
decision_dependencies: 207
machine_requirements: none
specific_machine: none
needs_person: no
---
# 704. Backfill the census's 128 prerequisite edges into BUILT and PARTIAL blocks

*Title and slug are drafts.*
Raised 2026-10-02 by the maintainer, amending §207 (the roadmap is a graph, and the block says so in
fields a script can walk) on pull request #1480 after calef ruled that the backfill starts with the
finished work. The slug and title are provisional.

## The gap, measured 2026-10-02

Milestone 596 (the roadmap blocks get frontmatter too) gave every PARTIAL, NOT-STARTED and PROPOSED
block the five dependency fields, and left the finished work for later. Its own follow-on says so.
Read from frontmatter on `main` that day:

| | |
|---|---|
| BUILT blocks carrying the fields | 7 of 279 |
| Blocks naming any milestone dependency | 38, one of them BUILT |
| Census prerequisite edges present in a field | 4 of 128 |
| Blocks `script/roadmap --unmodelled` lists | 319 of 612 |

The 128 edges in `notes/dependency-census/dependencies.tsv` come from 89 source blocks: 46 BUILT
and 14 PARTIAL, carrying 61 and 26 of the edges. The rest are NOT-STARTED (24 blocks, 35 edges) and
a handful of OPTIONAL, RECORDED and REMOVED. So the graph a script walks today is mostly forward
guesses, and the edges hindsight already found sit in a TSV nothing reads.

## The work

Write each census prerequisite into its source block's `milestone_dependencies`, BUILT and PARTIAL
first, and give every BUILT block the five fields as it goes (`none` where the census found
nothing). The NOT-STARTED edges come along cheaply once the tooling exists.

Do not copy the TSV blind. The census note records about 80% precision on a twenty-row sample,
every miss a negation (*"nothing gated this after milestone N"*), and §207 records 18 rows whose
direction is impossible because the source merged before the target. Each edge is read in its
evidence sentence before it is written. The rows refused, and why, belong in the census note's
`BUGS` so the next reader does not redo them.

## Done when

`script/roadmap --unmodelled` lists no BUILT or PARTIAL block, and the count of census edges present
in a field is reported against the 128 with each refusal accounted for. Re-take the table above
from the merged tree and put it in the block.

## Index row

Only 4 of the dependency census's 128 prerequisite edges are present in a block field, and 7 of 279 BUILT blocks carry the five fields. Proposed: backfill the edges into BUILT and PARTIAL blocks, starting with the finished work, as calef ruled.
