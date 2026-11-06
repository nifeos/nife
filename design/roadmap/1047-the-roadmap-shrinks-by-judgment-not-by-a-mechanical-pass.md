---
status: NOT-STARTED
promoted_from: the-roadmap-shrinks-by-judgment-not-by-a-mechanical-pass
raised: 2026-11-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1047. The roadmap shrinks by judgment, not by a mechanical pass

Raised by lane/record-follow-on-items.

Reuse: §298 (a document states what is in force now)'s list of what stays, the prose ratchet and
`script/roadmap --check`. No new tool.

## Background

Two mechanical batches trimmed roadmap blocks to §298: #3213 (43 blocks, about 5,800 words) and
#3220 (17 blocks, about 1,600 words). They cut filing narration, provisional-number asides, quoted
follow-on text, resolved `BUGS` entries and index rows that restated a whole block. Together they
removed about 7,400 of the roadmap's 1.48 million words, under 1 percent. An earlier estimate put
the reachable cut at 15 to 35 percent; that range assumes rewriting blocks, which a mechanical pass
does not do. The mechanical batches stop here.

## The question

Is a judged rewrite worth its cost, and where does it start? Recommendation: measure first. Count
words per block by status. Then rewrite only the BUILT blocks a reader still meets, those that code,
notes and decisions cite, one block per change. Each rewrite keeps §298's list: refused options with
their reasons, prior-art tables, measurements with their dates and methods, caveats, counted-claim
markers and index rows. The alternative, leaving the roadmap as it is, costs nothing
now and keeps every block a reader may need.

## Done when

- This block records the per-block word count by status, and an architect has chosen whether a judged
  rewrite proceeds and in what order.

## Index row

Two mechanical passes cut under one percent of the roadmap. Proposed: measure words per block by status, then rewrite by judgment only the built blocks a reader still meets.
