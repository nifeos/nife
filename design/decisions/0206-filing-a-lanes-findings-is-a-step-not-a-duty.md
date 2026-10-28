---
status: DECIDED
raised: 2026-09-22
decided: 2026-10-28
ratified_by: calef
---

# 206. Filing a lane's findings is a step, not a duty somebody remembers

Ruled 2026-10-28 (UTC): Option 3: a capture step after every lane reads the report and pull request
body. It emits either nothing or N stub files in `design/roadmap/proposals/`; extraction rather than
judgment, firing without anyone remembering. calef deletes a stub it gets wrong; a finding it catches
is one the architect does not have to prompt.

Raised by calef, 2026-09-22: *"it is the actions that aren't captured as
proposals. That I see and prompt from the summaries. There is still a non-trivial number of those
that are not tracked in the tree at all until I prompt."* *(Section number provisional until the
merge queue lands it.)*

## What is being decided

Whether to build a capture step that reads a finished lane's report and files what it named,
and whether that overturns `AGENTS.md`'s standing refusal to mechanize this at all.

## The failure, stated precisely, because a near neighbor of it is working fine

**This is not the proposals backlog.** `design/roadmap/proposals/` is healthy: 31 files landed in
the two days to 2026-09-22, and the 19 awaiting a number are waiting on triage rather than on
capture. That path works.

The leak is one step upstream. A lane names work it is not doing, in its final report or its
pull request body. Both are read once, by one person, on the day they are written. If the reader
does not act in that moment the finding is gone, and the only thing that recovers it is calef
reading a summary later and prompting. `AGENTS.md` predicts this in terms and then leaves it at rung
four:

> milestone 90 exists only because calef happened to be at his desk the day a report named it

(That is milestone 90 (a guard page under the per-CPU secondary stacks), real kernel work that
existed only because one person read one report on one day.)

And it gets worse as lanes get cheaper, which is why it is worth deciding now rather than when
it hurts. Every lever this project has pulled in the last week (renting open models, gating in CI,
raising the lane ceiling) increases the number of reports produced per hour without increasing the
one person reading them.

## What `AGENTS.md` currently says, and why the reasoning no longer holds

The constitution refuses to gate this, in one sentence:

> **It does not gate**: no check can tell an intention from an observation in prose, and a lint that
> tried would be `git grep -w TODO`'s 82% false-positive rate wearing a different hat.

**That is an argument about false positives, and the costs here are not symmetric.** A false
positive is a junk file in a directory a human already triages: ten seconds to delete. A false
negative is the thing currently being paid for, in the one resource this project calls scarcest.
Optimizing against the cheap error was the mistake, and the 82% figure is the tell: it was quoted as
disqualifying without anyone asking what the other error cost.

The refusal is also aimed at the wrong mechanism. A lint scanning prose for intentions has to be
*right*, because it blocks a merge. A step that files candidates has only to be *useful*, because a
human culls it. Those are different machines and only the first one has the false-positive problem.

## The options

Option 1: leave it at rung four and rely on calef's summary-reading. Honest about what it is.
Costs the architect's attention continuously, and degrades as lane throughput rises.

Option 2: a gate on the pull request body. Require a `## Work I am not doing` section whose
items each name a home (a proposals path, or a file with a `BUGS` heading), with `None.` valid. The
check resolves the references rather than reading prose, so it needs no judgment. Recommended
against: it puts the burden on the lane at the moment it is least able to bear it, and `None.` is
a single word away from defeating the whole thing.

Option 3, recommended: a capture step after every lane. Something reads the report and the pull
request body and emits either nothing or N stub files in `design/roadmap/proposals/`. It is
extraction rather than judgment, which is what §202 (mechanical work goes to a cheaper model, and
the gates are why that is safe) routes to a cheaper model. It fires without anyone remembering,
which is the ladder's rung two. A stub it gets wrong is deleted; a finding it
catches is one calef does not have to.

## What is blocked until this is answered

Nothing is blocked, which is the reason to decide it now rather than under pressure: the cost is
paid continuously and invisibly, in prompts calef should not have to write.

## What would have to be measured first

The size of the leak, which is being measured rather than asserted. Twelve merged pull requests
sampled, each named action checked against `design/roadmap/proposals/` and against `BUGS` sections.
If nearly everything is already tracked, option 1 is correct and this section should be refused on
the evidence.
