# The six questions, answered for this fork

*Name: provisional. After the parent's "The six questions". calef names files.*

An appendix of [reviewing the work of one model](../model-attribution-review.md). The parent keeps a
one-line answer to each; this file keeps the answers whole. "Finding 6" is in
[reconnaissance.md](reconnaissance.md), and "above" means the parent.

## The six questions

*A fork reaches calef with its questions already answered.*

1. What else was considered, and why did each lose? An unblinded read of all 180 Sonnet diffs (the
   request as first framed) loses because it cannot produce a negative result. A metrics-only answer
   with no reading loses because finding 6 shows three metrics giving three orderings. A review
   scoped to "commits with the Sonnet trailer" loses to pull-request attribution, which recovers 62
   more commits for free. Tier 3 loses to tier 4 on the only axis that matters, which is whether the
   confound is removable.
2. What does this tree already do in the analogous case? It runs audits as a mechanism:
   `script/audits`, `design/audit-reports/README.md`, seven audits on record, each a named lens with
   a cadence row and findings dispositioned as fixed, minted, or accepted. It blinds a reader by
   spawning a separate `claude` process whose working directory is the tree's parent, and it probes
   the isolation before trusting it (`script/stranger-test`). It tracks unratified names in
   `script/names` rather than in a study. Tier 1 uses all three rather than growing twins of them.
3. What is the prior art outside the tree? The line-level attribution in finding 6 is the SZZ algorithm
   (Śliwerski, Zimmermann and Zeller, *When Do Changes Induce Fixes?*, MSR 2005). Its known failure
   modes are the ones observed here: keyword-matched fix commits over-select, and refactoring
   commits absorb blame for lines they only moved. The rename-campaign check in finding 6 is the standard
   mitigation for the second. Cited as a method, not quoted; nothing here is a block quote from
   memory.
4. Is the premise true? Partly. "Sonnet wrote 181 commits" is close (180 trailered, 242 by pull
   request). "Sonnet's commits can be compared against other models' commits in this tree" is
   **false**, and that is the finding: there is no overlap in time, and the comparison everyone
   would naturally draw is between two different weeks.
5. What does each option cost, measured rather than asserted? Priced above in lanes, hours and
   tokens, from measured diff sizes (46,223 total, 38,459 code, 649 on the irreversible surface)
   rather than from estimates.
6. How reversible is it, and who has already acted on it? The review itself is fully reversible;
   nothing it produces changes the tree. Its *conclusion* is not. A written verdict that a model is
   worse is a fact that leaves the machine, and this project's own tenet puts that in the
   irreversible category alongside a published benchmark. That is the reason for the rubric, the
   control and the stated null: a wrong verdict here cannot be recalled by deleting a note.
