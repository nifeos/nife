# The second sweep, 2026-09-03: the 22 `PARTIAL` blocks

*Name: provisional, minted 2026-10-11 (UTC) by lane/final-longest-notes, after the parent's "The
second sweep, 2026-09-03: `PARTIAL`". calef names files.*

The evidence behind [`notes/follow-on-work.md`](../follow-on-work.md)'s section "The second sweep,
2026-09-03: `PARTIAL`, and why it is the harder half". The parent keeps the argument, the result and
the eighth word, `Outstanding.`; this file keeps the case that minted milestone 252 (nobody re-reads
it), the three shapes the stale claims took, and the stale sentences found outside the roadmap.

## The case that minted the milestone

Milestone 16 (real hardware) listed three things as remaining, and two were finished. The on-board
test-suite exit is the UART marker plus SBI SRST that every board run is judged by. The DTB-driven
UART IRQ printed `source 32` on all nine boots of the 2026-09-03 series. The block said otherwise
for weeks, and calef asking what was left is what caught it.

## How the 22 were read

Six lanes read all 22 blocks in full, about 8,500 lines. Each checked every claim against the tree
rather than against the block's own prose. 215 dispositions were written, and 56 of them record a
claim the tree disproved. Every one of the 22 blocks carried at least one.

The expectation going in was a long tail: a few rotten blocks and a majority that were fine. There
is no such majority. Three of the blocks the lanes singled out as unusually well maintained each
carried stale items too.

## Three shapes recurred, and none of them is carelessness

- A block that accretes lane reports keeps its old questions in the present tense. Milestone 139
  (drive) is eight rounds appended in order. Its "what is still open" section opens with a sentence
  its own `BUGS` refutes 140 lines later. Every round was honest when written.
- A block written before a decision landed keeps citing the fork. Milestone 142 (text) waited on a
  font and a palette that DECISIONS §104 (the rich-text font) chose on 2026-08-20. That was seven
  days before the block was last edited, and it cites §104 nowhere.
- A block's premise can be deleted out from under it. Milestone 152's "what was built" section
  cites files the SMB removal deleted on 2026-08-30.

## The wrong sentences outside the roadmap

Fixing a block repeatedly left a rustdoc, a module header or a note still saying the old thing,
where no gate reads it. Ten of those are listed in
`design/roadmap/0333-claims-the-sweep-found-false-outside-the-roadmap.md`. Among them are a rustdoc
that says the address-space object does not consult `ENUMERATE` yet, and a decision file that quotes
a roadmap block saying nobody is building the decision. Six comments cite a type deleted four days
earlier. This is the same rot milestone 247 (follow-on work) found through `Recorded.` path
citations, one layer out, and nothing mechanical would have caught any of it.
