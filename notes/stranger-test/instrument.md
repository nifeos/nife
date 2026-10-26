# Stranger test instrument history

*An appendix to [the stranger test](../stranger-test.md), which holds the current state. This page
is the record for how the harness, the rubric and the cadence came to be as they are.*

## The harness, built 2026-08-18

Before `script/stranger-test` existed, four runs each rebuilt the harness by hand. Each got the
isolation wrong somewhere the others had not:

- Run 2's stranger was a subagent and had `AGENTS.md` in context at turn zero.
- Run 3's log files sat in the stranger's own working directory, under names that told it which run
  it was.
- Run 4's `pkill` shim was right because somebody remembered, not because anything held it.

That was rung four of CLAUDE.md's ladder holding up the milestone that exists to move things off
rung four, as every run from run 1 on found. The main page lists what the
harness holds; each item there is something a person would otherwise have to remember. Telling the
stranger it is measured is run 4's finding acted on, not a softening. The `nife-dev` link is
restored because a stranger's `script/test` takes it like any other lane, and unlike a lane its tree
is disposable.

The harness reads the rubric out of the note rather than from a copy in the script. So the note's
table stays the only rubric, and an amendment is asked at the next debrief without anyone editing
the harness. The first draft of that extraction matched every `| M<n> |` row in the note. That swept
the *scoring* tables of runs 3 and 4 into the stranger's own question list. The `--smoke` run caught
it, in the reply, and that is the argument for `--smoke` existing.

What the harness cannot do is in its own `BUGS` section, where a reader meets the tool. Two of those
bound what any future run can claim, so the note repeats them. The harness cannot make the operator
un-read `AGENTS.md`. And it cannot stop the tree leaking that a test exists, which is now also a
`script/` entry point and a row in `notes/scripts.md`'s table. The response to the second is
disclosure, not concealment.

The harness withholds these appendices along with the main page.

## Amendments, 2026-08-18, forced by run 3 and applied before run 4

The rubric says that a stranger falling down somewhere it does not ask about means the rubric needs
amending. Run 3 went further and falsified two rows. Run 3's lane recorded the corrections and left
the table as written. Run 4's lane applied them to the table itself on 2026-08-18, before its run
started. A rubric that says one thing in its table and another four paragraphs below is two rubrics,
and the next run would grade against whichever it read.

### M8's premise was stale

It asked for "the three provenance states", and there are four. §89 (`provisional` becomes the
fourth provenance state) landed `provisional` on 2026-08-16, ten days after the rubric was written.
`notes/adding-a-program.md` stated three and then corrected itself to four in the same section. The
stranger answered four and said the question was out of date. That is the better answer, and against
the table as written it would have scored "wrong". A rubric that predates a decision grades against
a tree that no longer exists. The amended row states no count, because the count is the part that
went stale, and a row that counts something will go stale again. It asks what states exist and
points at the gate that enumerates them.

### M1 quoted a phrase the tree does not use

"Designation is authorization" is object-capability vocabulary from outside this project. The
stranger looked for it, did not find it, and flagged that it was importing the phrase rather than
reading it. In the tree's own words the idea is `swish`'s banner, *"naming a resource in a command
IS granting it"*, and `grant_plan`'s `Refusal::NoSuchProgram`. A rubric row that quotes a phrase the
tree never wrote tests whether the stranger already knew the field, which is the opposite of its
purpose. The amended row asks the question without importing anyone's vocabulary. A stranger can
answer it from `crates/abi`, from `notes/capabilities.md`, or from `swish`'s banner.

## The cadence: monthly, decided 2026-08-18

calef decided on 2026-08-18 that the stranger test runs monthly. That closed the one thing five
runs each named as the reason milestone 117 (the stranger test) could not move. The milestone's own
sentence is *fix what the run finds, then run it again*. Until this decision nothing scheduled the
"again": the test ran when somebody thought of it, which is rung four of CLAUDE.md's ladder holding
up the milestone that exists to move things off rung four. Four runs said so in their handoffs, and
the fifth said it was the only remaining decision.

The date of the last run comes from the `### Run <n>, <date>:` headings on the main page, which
every run already writes. Nothing is maintained for the tripwire's benefit, and that is what stops a
schedule and a record from disagreeing. A second copy of the date, in a script or a workflow, would
be a fact kept in two places, and this tree has watched that fail often enough to name it. The audit
cadences sit in the audit index for the same reason, per §74 (audits run on change, not on the
calendar).

### What a run costs

One stranger session plus one lane: roughly 200k lane tokens on top of the stranger's own, and about
half an hour of wall clock, with the score and the write-up after that.

Run 5's `$10.56` needs its caveat, and it has already been quoted without one. It is the CLI's
`total_cost_usd` for the stranger process alone. It excludes the lane that pre-registered the run,
watched it, debriefed it, scored it and wrote it up, which is the larger half of the cost. A monthly
cadence claims that a stranger's questions are worth about that much once a month. That is an
architect's claim, and the runs support it: five runs produced roughly seven findings each,
including two defects nobody in the tree could see.

### Why the calendar, and when to revisit it

The value of a run decays with the tree, not with the clock. That argues for a count-based trigger
of the kind §74 chose for audits, and against 30 days being the right unit forever. It is not built
that way here because an audit's triggers are countable (milestones built, components, ABI
constants, packages) and this one's are not. What ages a stranger run is how much of the
*documentation* has moved. The only count for that is `script/audits --worklist`, a heuristic that
never reads a sentence. So the calendar does the whole job here, where for audits it is a backstop.
Revisit it the first time a run comes back saying nothing had changed.

Run 6 showed that the notification has no reader yet. That
gap is milestone 495 (a cadence that says "due" reaches nobody).
