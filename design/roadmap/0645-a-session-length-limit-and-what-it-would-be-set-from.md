---
status: NOT-STARTED
raised: 2026-09-24
promoted_from: a-session-length-limit-and-what-it-would-be-set-from
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 645. A session length limit, and what it would be set from

*Title and slug are drafts.*
Raised by calef on 2026-09-24, after a measurement across this
project's session records showed that 98% of every token spent is a cache read and 0.1% is
output. A turn costs roughly the size of its context, not the size of its thought. He asked the
question this file exists to answer: *"How do we set a checkpoint to re-evaluate that will not be
forgotten?"*

The threshold is an architect's, and it is deliberately not being chosen yet.
`AGENTS.md`'s *measure first, then decide* rules that a threshold set before the data exists is no
better than one chosen under attachment, and on the day this was raised there was one measurement,
which is not a distribution. The trigger is enough weeks of per-turn context to show a
distribution, judged by reading the panel, not a week count: a count would be the same
threshold-in-ignorance the tenet refuses, one level up. The panel is
[`notes/project-metrics.md`](../../notes/project-metrics.md), section *What a turn costs*, and it
arrives in front of whoever reads that page each week, which is the whole answer to calef's question.

## What is being deferred

Whether to impose a session length limit on the agent harness, and at what threshold. Today
there is none, and a long session's context grows until auto-compaction cuts it.

## What the numbers say so far

Per ISO week, from `notes/project-metrics/context-per-turn.csv`, columns `lane_turns`,
`lane_cache_read_share_pct`, `lane_context_per_turn_mean` and `lane_context_per_turn_peak`:

- Cache-read share of all tokens: 95.6% to 98.6% across six captured weeks.
- Mean context carried per turn: 230,100 to 358,804 tokens.
- Largest single request each week: 933,378 to 999,863, against a 1M window. Sessions run to the
  wall every week.
- One session measured at roughly 233 million tokens against 5 million for the twenty lanes it
  dispatched (notes/what-a-session-carries.md), which is the shape this would act on.

**This is not a claim that context is waste.** Carrying it is what makes a long session coherent and
it is why the method in `AGENTS.md` principle 2 works. The finding is where the bill goes.

## What the decision would choose between

Three levers, and they are not equivalent:

1. A `UserPromptSubmit` hook that warns past a threshold. Advisory. It costs nothing when wrong
   and it relies on a person acting on a warning, which is rung four of the ladder and is the rung
   this project's recorded failures live on.
2. `autoCompactEnabled: false`. Converts a slow bleed into a hard stop. It is the strongest of
   the three and it would also end sessions mid-task, which is a cost paid by whatever lane was
   holding uncommitted work at the time. `AGENTS.md` already names uncommitted work in a lane
   worktree as the one thing no part of this system protects.
3. `autoCompactWindow`. Moves where compaction fires without removing it. The middle option, and
   the one whose effect on cost nothing here has measured.

Doing nothing stays available and is the current state.

## What would have to be true to answer it

- Enough weeks that the mean per turn has a shape rather than a level: is it rising, flat, or set
  by the model's window rather than by how anyone works?
- Some evidence on what a compaction actually costs in re-read tokens, which none of these columns
  can see.
- A statement of what a limit is for. Cash is under a thousand dollars all in against eleven
  person-weeks (*What this project costs* on the same page), so a limit justified by money is
  arguing about a rounding error. A limit justified by a session degrading as it lengthens is a
  different claim and would need a different measurement.

## The instrument's own fragility

The session records are **not in git**, live on one laptop, and nothing promises to keep them;
`script/effort`'s header already records 2026W29 through 2026W33 as unrecoverable. A per-turn context
history has the same deadline, and with more than one contributor it measures one machine. So the
trigger above is a race: the distribution either accumulates before the records rotate, or this
decision is made on whatever survived. `script/effort --snapshot` under `launchd` is what makes
forgetting survivable, and it is the reason to answer this while the capture is running rather than
after it stops.

## Name

**Provisional.** `a-session-length-limit-and-what-it-would-be-set-from` is a slug, and the column
names it proposes (`lane_turns`, `lane_cache_read_share_pct`, `lane_context_per_turn_mean`,
`lane_context_per_turn_peak`) and the panel file (`context-per-turn.svg`) are provisional too. They
follow the `lane_` prefix `lane_tokens` and `lane_wall_clock_hours` already established for
harness-measured columns. no architect has ratified any of them.

## Index row

98% of every token spent is a cache read, so a turn costs the size of its context. Proposed: a session length limit, with the threshold deliberately not chosen until enough weeks of per-turn context show a distribution.
