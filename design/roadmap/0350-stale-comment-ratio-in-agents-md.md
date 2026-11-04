---
status: SUPERSEDED
raised: 2026-09-03
superseded_by: §177
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 350. The comment ratio AGENTS.md quotes is wrong, and it climbs every week

AGENTS.md quotes no counts and points at `notes/project-metrics.md` (§177 (whether AGENTS.md quotes
measured numbers at all)), so the sentence this block would have corrected does not exist. The rest
of this block is the record as filed. Premise checked 2026-09-19, and the number had drifted
further. `AGENTS.md` line 111 still reads "`kernel/src`
measures 40% of them". `notes/project-metrics/weekly.csv`'s latest row (2026W38, 2026-09-14) gives
`kernel_code_lines` 36,157 and `kernel_comment_lines` 31,309, which is 46.4%; 2026W36 was 46.0%.
So the figure a reader meets in the front-door file is six and a half points low and still
climbing.

Nothing is left to start: the ruling answered the question and the edit it asked for was calef's
commit. What follows is the gate as it read before 2026-09-24. `AGENTS.md` is calef's file and the
correction is a one-line edit only he makes.
The measurement is already done and stored, so nothing else is owed: what is missing is the edit and
a decision about whether a hand-copied number belongs in that file at all.

**In brief.** `AGENTS.md` tells a reader that `kernel/src` "measures 40% of them" when discussing
comments in the line count. Milestone 234's weekly series measures that ratio directly: 39.3%
eight weeks ago and 45.3% now, climbing every week. The figure was true when it was written and is
not true today. The work is to correct it, and to decide whether the sentence should quote a number
at all or point at `notes/project-metrics/weekly.csv`, which cannot go stale.

## Why this matters

This is the project's front-door file. A newcomer reads it before they read anything else, and
AGENTS.md's own third principle is that a stranger must be able to succeed on the tree alone. A
wrong number there is the exact defect that principle names: *"a newcomer who hits a limitation the
docs named will trust the docs. One who hits a limitation the docs hid will not trust anything
again."*

It is also drifting rather than merely stale. Correcting 40% to 45.3% buys a few weeks and then the
same bullet is true again, which is why the second half of this is a question and not just an edit.
A pointer at the series is rung one, since the series recomputes itself; a hand-copied percentage is
rung four and has now demonstrated its failure mode.

## Index row

`AGENTS.md` told a reader that `kernel/src` "measures 40% of them" for comments in the line count.
Milestone 234's weekly series measures the ratio directly: 39.3% eight weeks before the proposal,
45.3% when it was written, 46.4% on 2026-09-14. The figure is drifting and not merely stale, so
correcting it buys a few weeks. A pointer at `notes/project-metrics/weekly.csv` is rung one because
the series recomputes itself, and a hand-copied percentage is rung four. `AGENTS.md` is calef's
file, so the edit is his.
