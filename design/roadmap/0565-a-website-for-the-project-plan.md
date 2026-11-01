---
status: NOT-STARTED
raised: 2026-09-21
promoted_from: a-website-for-the-project-plan
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 565. A website for the project plan

The number is **provisional**: the integrator mints it at merge. Promoted from the proposal `a-website-for-the-project-plan` on 2026-09-22, filed 2026-09-21. Written by the lane that retired the roadmap index, out of calef's
own sentence on the day he retired it: *"Nothing at all. You'll have to read the directory for now.
Eventually we'll build a website and the project plan will be captured there."* The first half is
done; this is the tracked form of the second, because `AGENTS.md` says identified work leaves a lane
as a proposal or a recorded limitation and "eventually" is neither.

What a website is for here has not been decided, and everything about the shape
follows from that answer rather than from anything a lane can look up.

## What the directory does not do

The retirement cost nothing a reader had, because the table was generated and unreadable. It did not
add anything either, and three of the things a stranger wants are still answered only by running a
command in a checkout:

- An ordering. `script/roadmap --ready` says what a lane could start today; nothing says what
  this project intends to do next, or why that rather than the other thirty-eight ready blocks.
- A shape. 522 milestones in one flat directory have no visible structure. The journeys
  (`design/journeys/`) are the closest thing the tree has to one and nothing points at them from
  outside.
- **A reader who is not in a checkout at all.** Every query here is a script, and every script
  assumes a clone, a Python, and a terminal. `AGENTS.md`'s third principle is that a newcomer must
  be able to succeed without asking anyone; a person who has not cloned the repository yet cannot
  run the thing that would tell them whether to.

## What this tree does not have today

There is no publishing story of any kind, and that is the honest size of the work. Nothing
renders markdown for a browser, nothing is deployed anywhere, the project's domain (nifeos.org,
registered 2026-10-03) has no web address set, and no gate would notice a published page going stale. `crates/documentation` renders
markdown to a terminal, for `man`, which is the opposite end of the problem.

So a website is not a page: it is a renderer, a place to put the output, a trigger that runs it, and
a rule about what may be published. The last is the one this tree already has opinions about, and it
is why the gate above is `DECISION` rather than `NONE`: *move fast on what can be undone* names
facts that leave the machine as the irreversible category. Every number on a published page is
one. This repository has published a wrong claim to itself more than once and corrected it the same
week; a stranger quoting one off a website cannot be reached.

## Deliberately not designed here

No static-site generator is named, no hosting is chosen, and no page structure is drawn. Each of
those is cheap once the question above is answered and misleading before it, and the shape of the
answer decides them: "a reader's front door" wants something quite different from "the project plan,
published".

## What is blocked until it is answered

Nothing. The directory works, the queries work, and no milestone waits on this. It is written down
so that the sentence that named it does not become the fifth thing this project loses by saying it
out loud instead of writing it down.

## Index row

Retiring the generated roadmap index cost a reader nothing and added nothing. Three of the things a stranger wants are still answered only by running a command inside a checkout. Two are what this project intends to do next and why, and what shape 522 flat milestone files have. The third is anything at all for a person who has not cloned the repository. `AGENTS.md`'s third principle is that a newcomer must be able to succeed without asking anyone, and there is no publishing story of any kind here yet, which is the honest size of the work.
