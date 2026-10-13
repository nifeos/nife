---
status: DECIDED
raised: 2026-09-19
decided: 2026-09-24
ratified_by: calef
---

# 177. Whether AGENTS.md quotes measured numbers at all

This section is milestone 350 (the comment ratio AGENTS.md quotes is wrong, and it climbs every
week)'s `DECISION` gate. Milestone 234 (the project's own numbers, one row per ISO week)'s series
produced the evidence.

## The ruling

calef, 2026-09-24 (UTC): drop the counts and point at the generated metrics. AGENTS.md now
reads *"A number here changes at the pace of a decision, not at the pace of a commit (calef,
2026-09-24). Counts live in `notes/project-metrics.md`, generated weekly so they cannot rot"*
(AGENTS.md lines 56 and 57). What stays is one order-of-magnitude sentence carrying the date it was
written, which moves only when a decision moves it.

That is option B, with two differences from the table below. The pointer is at
`notes/project-metrics.md`, the generated page, rather than at `weekly.csv`. And the four figures
the series did not carry were deleted rather than first added to the series, so D's precondition was
not taken. The evidence is commit `e23df4a4d` ("docs: AGENTS.md's numbers change at the pace of a
decision"), merged in PR #1198 on 2026-09-24 at 15:05 UTC, whose message records that calef ratified
the rule that day. Recorded here 2026-09-26 by the decisions-hygiene lane, which invented no part of
it.

What it answers downstream. Milestone 350 (the comment ratio AGENTS.md quotes is wrong) is
`SUPERSEDED` by this: the sentence it would have corrected no longer exists.

## What is being decided

AGENTS.md's second principle rests on a paragraph of measurements, and every one of them is a
hand-copied number that drifts. The decision is whether that paragraph quotes numbers, points at
the series that recomputes them, or does both with a date attached.

Milestone 350 poses it as one bullet (the `40%` comment ratio). It is the whole paragraph.

## The tree as it stands, read rather than recalled

AGENTS.md lines 106 to 111, measured 2026-08-30 and unchanged since. Against
`notes/project-metrics/weekly.csv`'s latest row (`2026W38`, 2026-09-14) and against the tree on
2026-09-19:

| figure in AGENTS.md | as written | measured now | in the weekly series? |
|---|---|---|---|
| days since the first commit | 49 | **69** (2026-07-12 to 2026-09-19) | no |
| milestones built | 103 | **167** | yes |
| milestones total | 193 | **290** | yes |
| crates | 65 | **68** (`crates/*/`, 2026-09-19) | no |
| user programs | 69 | **unrecoverable, see below** | no |
| lines of Rust | ~194,000 | **204,921** (code plus comments, all four columns) | yes |
| Kani proof harnesses | 145 | **151** | yes |
| commits | 3,099 | **not countable here**, a lane's worktree has shallow history | no |
| `kernel/src` comment ratio | 40% | **46.4%** (31,309 of 67,466) | yes, derivable |

Eight figures, and the series recomputes four of them. That is the finding that shapes this
decision, and it is not in milestone 350's block: "point at the series" is rung one for half the
sentence and answers nothing for the other half, because the series carries no crate count, no
program count, no commit count and no elapsed days.

One figure cannot be corrected at all, which is the sharper version of the same problem. "69
user programs" does not say what it counted, and the tree offers no single answer: `components` and
`fixtures` declare 47 and 40 `[[bin]]` targets, 87 together. A hand-copied number whose method
is not recorded cannot be re-derived by the next person, only re-guessed, which is a worse failure
than staleness.

## Why it matters, in the file's own terms

This is the project's front-door file, and AGENTS.md's third principle is that a stranger must be
able to succeed on the tree alone. The same section already prices its own numbers honestly (*"size
and rate, not quality"*), which makes the drift worse rather than better: a paragraph careful enough
to caveat itself reads as one whose figures were checked.

## The options

| | shape | cost |
|---|---|---|
| **A** | **Correct the numbers in place.** | One edit, and the same bullet is wrong again in a few weeks. It has already demonstrated the failure twice. Rung four. |
| **B** | **Replace the figures with a pointer at `notes/project-metrics/weekly.csv`.** | Rung one for the four figures the series carries, and it deletes the other four rather than fixing them. It also costs the paragraph its punch: a reader meeting a link instead of a number does not feel the claim, and the claim is what the principle is for. |
| **C** | **Keep a dated snapshot and add the pointer**, in the shape the paragraph already half uses (*"Measured on 2026-08-30"*). | One edit now and a habit afterwards, which is rung four wearing a date. Honest about being stale rather than silently stale, and that is a real improvement: a reader can tell a six-week-old number from a current one. |
| **D** | **Extend the series to carry the four missing figures, then take B.** | A lane's change to milestone 234's generator, needing no ruling. Afterwards the whole paragraph recomputes and the question stops recurring. |

Recommendation: D, then B, and this one is offered rather than withheld because the fork is
reversible: it is a paragraph in a file, and getting it wrong costs an edit.

The argument is that A and C both keep the tree in the state that produced this section, and B alone
is not available until the series can answer for all eight. D is the only option that puts the
numbers on the rung AGENTS.md's own ladder recommends, and the generator change is small and is not
an architect's.

Question 7, answered out loud: yes, this would still be the recommendation if all four options
cost the same, because the argument is about which rung the figures sit on rather than about effort.

## What is blocked until this is answered

Milestone 350. The one-line edit is an architect's either way, since a developer may not edit AGENTS.md,
and the series extension under D is a lane's and could start today.

## What this does not decide

Nothing about the caveats around the paragraph, which are correct as written and are the reason the
paragraph is worth keeping at all.
