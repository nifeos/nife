---
status: DECIDED
raised: 2026-10-26
decided: 2026-10-26
ratified_by: calef
---

# 310. Read a note against its code, and send a crate's documentation with the crate

Ruled 2026-10-26 (UTC), and in force tree-wide. An agreement between two programs that a note meets
falls under [§311 (a contract is code, not prose)](0311-a-contract-is-code-not-prose.md), which this
section defers to.

## The rule

1. Each notes area gets one reading against its code. The reader sorts every passage into one of
   four piles:
   - gone: it retells the code. The rewrite deletes it and names the item that holds the steps.
   - moves: it is about one crate alone, such as a hardware quirk, an invariant, a source read
     or why the crate takes the shape it has. It goes into that crate's README, its rustdoc or a
     `docs/` folder inside the crate, and cites nothing a stranger cannot open.
   - stays: it is about nife's use of a crate, or about several crates together. Reasons,
     refused alternatives, measurements, `BUGS` entries and orientation stay.
   - contract: it states an agreement between two programs. §311 says where that goes.
2. A note does not retell code or contracts. It names the function, type or contract item and keeps
   the reason the code cannot give: why this order, why this bound, what the alternative cost.
3. Where a note and its code disagree, the reading fixes the note. Where the code may be the wrong
   side, it files a `BUGS` entry where a reader meets the feature, and the note claims no more than
   the code shows.
4. Before a crate leaves nife, a lane lists every note and every decision in nife that mentions it,
   and sorts each passage the same way. The crate does not leave until the lane has finished its
   moves pile.
5. A reviewer reads the rewrite passage by passage against the code before it merges. A reading
   that has not had that review is not finished.

The aim is accuracy and a correct sort. Fewer words and fewer passives are not aims, and a reading
may leave an area longer than it found it.

## Scope

Every area under `notes/`, its appendix directories included, each read once. The crate step covers
every crate that leaves the tree under [milestone 857 (crates released on their
own)](../roadmap/0857-crates-released-on-their-own.md), and reaches `design/` as well as `notes/`.
The reading covers the code an area describes: the crate, and each program on the other side of
what it states.

## The evidence

Two pilots tried the rule before it took effect for the whole tree.

`notes/system-initializer` (#2778), read against `crates/system_initializer`: 50 passages. The
reading found 9 claims the code contradicts, every one in narration or a summary of narration. The
reading and its review found 4 places where the code may be the wrong side, and each became a `BUGS`
entry. The review then read every row again and found a third of the narration cuts wrong: 7 of 21
removed something a reader needs, and the review put each one back. Most were contracts that
`grant_plan` holds, such as the spawn request's order and the configuration page's slot. The rest
were facts a reader acts on, such as a condition on a grant. The review also found 2 more
contradictions and 1 row that was no contradiction. Words in the area rose from 8,963 to 9,326.

`notes/e1000e` (#2782), a crate on the release track, read against `crates/e1000e` and both
sides of its spawn handoff. In the area itself: 23 passages stay, 7 go, 9 move, 10 contradict the
code and 2 state contracts. Across the other 208 mentions of the crate in `notes/` and `design/`:
190 stay, 12 contradict the code, 4 state contracts and 2 move. The reading fixed 14
contradictions in notes; the review found 2 of them, one in a line that never names the crate. The
notes area shed 542 words, and about 360 moved into the crate's README and rustdoc.
`script/crate-standalone` reported the same failures before the moves and after, so the moved text
cited nothing a stranger cannot open. The sort turned up 7 contracts between programs that no
shared code states whole, which §311 routes.

What the two show together: a reading finds real contradictions and real defects, which a
newcomer would otherwise inherit. A rewrite with no second reading would have shipped a third of
its cuts wrong, because the passages that look most like narration are often contracts.

## How it relates to other sections

- [§213 (writing standards)](0213-writing-standards.md) still governs every sentence a rewrite
  writes. Its rule 4, state a finding once, is the same rule this section applies between a note
  and the code: the code states the steps, and the note does not state them twice.
- [§267 (a comment states the constraint as it is
  now)](0267-a-comment-states-the-constraint-as-it-is-now.md) holds comments to their
  constraint and sends design to the note. This section holds the note to what the code cannot
  say. The two meet at the item a note names and its doc comment.
- [§277 (cite a heading or a symbol, never a line
  number)](0277-cite-a-heading-or-a-symbol-never-a-line-number.md) is how a note names the item
  that replaces a gone passage.

## Enforcement

Today, review: each reading's pull request carries its ledger of passages, and the reviewer asks of
every row whether the code shows what the note claims and whether the sort is right.

This section proposes two checks, and the tree has neither yet:

- a lint that every function, type or item a note names exists in the tree, so a rename cannot
  leave a pointer to nothing;
- a step in `script/crate-standalone` that refuses a crate whose moves pile is not finished.
