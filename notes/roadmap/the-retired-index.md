# The roadmap index, and why it is gone

*Name: provisional, minted 2026-10-11 (UTC) by lane/next-ten-longest-notes.*

An appendix to [notes/roadmap.md](../roadmap.md), which summarizes it.

The table was hand-maintained, then generated from the blocks (2026-09-14), and is not built at
all (2026-09-21). The two changes had different reasons and the second did not undo the first:
generating it fixed a merge problem, and retiring it answered a readability one. Five hundred rows of a
five-column layout with paragraph-length cells is not a document anybody reads, and a derived
document nobody reads is work the tree was doing for itself.

Nothing was lost in the retirement, and that is only true because of the migration. Milestone
294 had already moved the two hand-written columns into the blocks, so by the time the table went,
every cell in it was a rendering of text that lives in a milestone's own file. Had the order been
reversed, deleting the table would have deleted 288 hand-written summaries and 288 dates.

What survives is the rendering, not the file. `script/roadmap --index` still assembles the five
columns to stdout, because `script/audits` counts BUILT rows for its cadence trigger and
`script/fatal-risks` reads Built dates for its "as of" check. Both had been moved off the committed
file onto that feed by milestone 443 (lanes wait on each other for three reasons, and none of them
is the work). That is why retiring the file needed no change in either of them.

The rest of this page is the record of the generation, kept because the measurements in it are
the evidence that the retirement cost nothing.

## Why it was generated first

Measured on 2026-09-14, in one session: eight merge conflicts, every one of them in
`design/roadmap/README.md`'s index table and zero anywhere else. They were pull requests #836, #843,
#847, #849, #850, #851, #854 and #855, several more than once. `main` moving even once
re-conflicted every open lane.

It was structural. `script/lint` check 4b requires every lane to touch its own milestone's block.
Every milestone also needed a row in one sorted table. So every milestone in flight collided with
every other milestone in flight, always, in that one file. `AGENTS.md` names
`system_tests/src/user/tests.rs` as the hotspot to fear, measured on 2026-08-16. That is the wrong
file: only some lanes wire a test, and every lane adds a row.

The strongest argument is that `script/roadmap` exists because of this table. Its header records
that milestone status in prose was phrased a dozen ways. It records that a sweep mis-reported
eight milestones, and that *"milestone 69's row said NOT-STARTED while its block said BUILT"*. Milestone 69 (split
`kernel/src/user.rs` by service) was that case. One of the script's checks was that the row and the block
must agree. Generating the row makes that check vacuously
true, which is rung one of `AGENTS.md`'s ladder where the gate was rung two.

## The two columns that had to move, and the measurement that decided it

Three of the five columns were already derivable. The number is the filename's, the status is the
`**Status:` line the script already read, and the title is the H1 it already read. The other two
were not. Both options for avoiding a migration were measured before they were refused.

Deriving the summary from the block's opening paragraph. Refused on evidence. Across 288
milestones the row's summary scored a median similarity of 0.07 against the block's first body
paragraph. Three scored above 0.4, and none above 0.6. An opening paragraph is written to open a
document rather than to summarize one. Taking it would have rewritten what the index said about 288
milestones in a single commit, and those cells are the tree's own account of its history.

Reading the Built date out of the status sentence. Refused on evidence. 129 of 288 blocks
did not state the date anywhere a parser could find it, and the ones that did used at least eight
spellings:

```
**Status: BUILT.**                                    (the date only in the index)
**Status: BUILT, 2026-08-26.**
**Status: BUILT (2026-08-01).**
**Status: BUILT** 2026-09-14.
**Status: BUILT** (2026-08-03).
**Status: BUILT** on 2026-08-18 (PR #320).
**Status: BUILT**, 2026-08-27, as `rmle` rather than ...
**Status: REMOVED 2026-08-30.** Built 2026-08-17, and ...
```

Canonicalising 129 status lines is rewriting prose in 129 blocks to serve a parser, which is the
wrong direction and is not a thing a lane may do.

Generating only the derivable columns and leaving the summary in the table was the third
option, and it solves nothing. A lane would still edit one shared file to add its summary. So the
whole conflict class survives, and the milestone would not have paid for itself.

### Would we still choose this if both options cost the same?

Yes, and it is worth answering out loud because the chosen option is much the most expensive
of the three: it migrated 288 files where deriving from the opening paragraph would have migrated
none. The reason is not effort in either direction. Deriving would have changed what the index says
about 288 milestones, silently, in the direction of text that was never written to be a summary.
Moving text is safe; regenerating it is not.

## The method, and why the proof is reconstruction

The migration moved each cell's text into its block, and the proof was that the table regenerated:
271 of 288 rows byte for byte, and all 17 differences pre-existing drift in the hand-maintained
table. Milestone 294 (the index is generated, not hand-maintained) has the itemized diff. Verify by
reconstruction, not by inspection, is the transferable part, and milestone 596 (the roadmap blocks
get frontmatter too) proved its migration the same way.
