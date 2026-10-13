---
status: DECIDED
raised: 2026-09-23
decided: 2026-09-23
ratified_by: calef
---

# 210. A correction of error, and its action items are decisions, proposals or milestones

calef, 2026-09-23: this tree runs corrections of error, and an action item
is a decision, a proposal, or a milestone. Nothing else counts. *(Section number provisional until
the merge queue lands it: two other decision files, numbered 208 and 209, are open on
unmerged branches tonight, so 210 is the first free number as seen from `main` and may not be the
number this gets.)*

**What is decided and what is not**, so a reader does not take the whole file as settled. The ruling
in the line above is calef's and holds. The trigger, the file layout, and the gate extension in the
sections below are this lane's proposals inside a decided section, and are marked where they appear.

## The ruling

calef spent his career deploying Amazon's Correction of Error (COE) process, at IMDb, Amazon,
Backcountry.com and Varsity Tutors/Nerdy. On 2026-09-23 he ruled that this tree should run COEs
rather than keep an index of scars, and that action items should be decisions, proposals, or
milestones.

`notes/corrections.md` is the thing being replaced, and it is worth being precise about what is
wrong with it, because it is a good page. calef asked for it on 2026-09-21 and it does what it says:
three entries, each naming the note that carries the full account, kept because the corrections were
the most instructive part. What it has no place for is a timeline, a root cause, or an
action item. It is an index of scars. A scar is a record that something healed; it is not a
record of what was changed so it would not happen again, and nothing in the page's shape asks for
one.

**Everything that defines what a COE is belongs here, not in a COE.** calef, 2026-09-23, on the
first one written: *"Lets instead set a standard and this is the standard. The definition of the
standard is not part of this event."* A record that opens by explaining the form, the path, the
blameless rule, or its own place in a sequence is spending the reader's first screen on this section
rather than on what happened. So: a COE states its event and nothing else. It does not say it is
blameless (every one is, and the rule is two sections down), does not say it is the first or the
fourth, does not note that its times are UTC, and does not caption a table the reader can see. The
one form-level citation it carries is the gloss the citations ratchet requires on this section's
number.

**"Write it down" is not an action item.** It is what produced the problem. Every case in the next
section but one was written down, correctly, in the right file, by someone who understood it, and
each recurred anyway.

## What a COE is

Amazon's COE is a short document written after a failure, with a fixed shape: what happened, a
timeline with times attached, the impact in the terms the affected party would use, a root
cause reached by asking 5 Whys rather than stopping at the first plausible answer, and action
items with owners and dates. It is blameless, and this tree requires it rather than
recommending it: no COE names a person or a lane as a cause, and no COE says that it is blameless,
because a property every record has is not news in any of them. The rule exists because
a document that can end in "someone was careless" will end there, and that ending has no mechanism in
it. Aiming at mechanisms rather than people is what makes the fifth Why reach something anyone can
build.

This tree already holds most of that, in its own vocabulary and without the name. `AGENTS.md`'s
ladder is the root-cause discipline written as a ranking: make the wrong state unrepresentable, then
a gate that fails loudly, then a record at the thing itself, then a note. Its line **"somebody will
notice is not a mechanism. It is rung zero and it belongs on no list"** is a COE reviewer's rejection
of a soft action item, already written in this tree's voice. What is missing is the document that
convenes those and the requirement that its actions resolve to something.

## Why the action-item clause is the load-bearing part

Four cases, all verified against this tree while writing this section, each a correction whose action
item was "write it down", each of which recurred.

A retracted reading reached thirteen records before anyone swept it. On 2026-08-14 radon (the
RISC-V VisionFive 2) was read as having produced a receiver woken with nothing delivered on three
harts. On 2026-08-15 `notes/visionfive2.md`'s fifth bench stop retracted it: the dumps were the
terminal state of a completed boot tour, identified five independent ways. The retraction was
written down, in the right file, the next day. It never propagated. Pull request #1135 swept eleven
files on 2026-09-23 and #1136 took the two out of that lane's reach, thirteen in all, and one of them
was §203 (capacity is rented rather than bought), written on 2026-09-20, five weeks after the
retraction, using the retracted reading as radon's justification. The gap between the correction and
the sweep is thirty-nine days.

§116 (live component state handoff is declined, for want of a customer) outlived its premise by one
day. calef declined it on 2026-08-23 because no component with meaningful live state was built or
being built. `redoxfs_server` enters the tree on 2026-08-24, in commit `0c0ef2367`. The premise
was false within twenty-four hours of the decision that rested on it. Nobody revisited for a month;
it is reopened tonight on a separate branch. The decision said "revisit when a customer needs it",
which is a condition with no observer, and an unobserved condition is rung zero.

A load-sensitive flake was written into `notes/load-sensitive-assertions.md` twice and left there
both times. The swapper-budget surplus was recorded once, and on 2026-09-21 commit `0c872ac47`
added a second observation (296 pages of 224, where the first was 277) whose own message reads: *"It
recurs, the surplus differs each time, and it is still not established whether the frames come from a
neighbor's teardown or from a leak. Not this lane's to chase; it is a data point on a finding
somebody else already wrote down."* That sentence is honest and correct under the rules as they
stand. No lane that hits the flake is the lane that caused it, so nobody owns it, and the note grows
sightings instead of a fix.

**`script/metrics`' own `BUGS` section records a backfill that was run, verified, and never
committed.** The entry on the trust-boundary crate table ends *"None of the ten weeks on record hit
it as of 2026-09-20 (checked by running `--backfill` and reading the column back)."* The check ran.
The code was committed. The output was not: `notes/project-metrics/weekly.csv` today has the nine
`unsafe_trust_*` columns filled for `2026W39` and empty for 2026W29 through 2026W38, ten weeks of
a series with a hole in it. (The count behind this ruling was given as nine; the file says ten. The
correction is recorded here rather than silently used.) The work was done and the artifact was
thrown away, and the record of doing it is in the BUGS section of the script that did it.

The shape is the same four times. Somebody understood the problem, wrote a true sentence in a
sensible place, and stopped. The writing was the whole action. A COE that permits that as an action
item is `notes/corrections.md` with a timeline stapled on.

## The mechanism, which is what makes this checkable

*(Proposed, not decided. The design is this lane's; an architect rules on it.)*

A COE's action items use the same disposition vocabulary that `## Follow-on` rows already use.
`script/roadmap --check` carries that vocabulary and already validates it. The openers are
`**None.**`, `**Milestone N.**`, `**Done.**`, `**Recorded.**`, `**Refused.**`, `**Decision.**`,
`**Proposed.**`, `**Outstanding.**`, and each is resolved rather than read: `**Milestone N.**` must
name a block that exists under `design/roadmap/` and must not be the block it is written in;
`**Decision.**` must name a file under `design/decisions/` in backticks that exists; `**Proposed.**`
must name a file under `design/roadmap/proposals/` in backticks that exists; `**Recorded.**` has its
backticked paths resolved against a `PATHISH` pattern derived from `git ls-tree` rather than a typed
list.

A COE whose action items resolve to nothing has produced nothing, and a gate can say that. That
is the entire claim, and it is the reason to reuse this check rather than invent a form.

What to extend, specifically. In `script/roadmap`, the loop that walks a block's `## Follow-on`
bullets against the `DISPOSITION` regex and dispatches per `kind`. Factor that loop into a function
over (bullets, a filename, a context) and call it twice: once for a roadmap block's `## Follow-on`,
once for a COE record's `## Action items`. The status-coupled cases stay behind the context, because
they are roadmap facts rather than vocabulary facts: `**Outstanding.**` is PARTIAL-only and does not
apply to a COE, and the PARTIAL-versus-`**None.**` contradiction has no analogue.

One divergence, and it is the ruling. `**None.**` is legal on a milestone block and is
deliberately the cheapest thing to write there. It is **not** legal in a COE, because a correction
that identified nothing to change is a correction that has not finished. A COE with no action items
is the document this section exists to prevent.

Why reuse beats a second implementation, and this tree has already paid for the answer twice.
`script/metrics`' `name_surfaces` and `script/names`' `SURFACES` enumerate the same thing in two
places; milestone 175 (`components/` for services, `fixtures/` for test and benchmark programs)
moved the programs on
2026-09-13, `script/names` was told and `script/metrics` was not, and `names_total` showed a collapse
from 204 to 133 that never happened until 2026-09-19. One level in, `script/roadmap`'s own path list,
typed out by hand, drifted the same way, still naming `user/` nineteen days after that split,
and it was fixed by deriving the list from `git ls-tree` instead. A second copy of a
vocabulary that eight openers wide and growing (`Done.`, `Proposed.` and `Outstanding.` were all
added after the first draft) would drift on its next addition, and the drift is invisible: a COE
bullet the second implementation does not recognize is not read as an action item at all, so it
passes in silence, which is the failure this whole section is about.

## What triggers a COE

*(Proposed, not decided.)*

Amazon's trigger is customer impact, and it does not transfer: this project has no customers, which
`AGENTS.md` states plainly as of 2026-08-30. A substitute has to fire often enough to matter and
rarely enough to be read. Two, and they are both events rather than conditions.

Trigger 1: a correction that had to propagate to more than one record. One record is a fix. Two
or more means the fact had already spread before it was corrected, which is the failure with the
worst half-life in this tree, and the radon sweep is what it costs: thirty-nine days and thirteen
files. It is countable without judgment, because the act of sweeping is the act of counting, and the
number is in the sweep's own commit. The COE is written by whoever ran the sweep, while they still
have the list.

Trigger 2: a gate passed something it should have caught. This project's safety argument is its
gates. A gate that reported clean over a real defect is a falsified claim about a mechanism, and it
is the one class of failure where "write it down" is never the right answer, because the gate is
already a rung-two artifact and the fix belongs at rung two or higher. The tree has instances on
record: `script/roadmap`'s IN-PROGRESS check spent a day unable to fail in CI because
`actions/checkout` clones to depth 1, and `script/citations` exists at all because two gates both
reported clean on 23 comments that cited a milestone number when they meant the decision of the same
number, since both existed.

Rejected: work lost or nearly lost. It is the most severe thing that happens here, and
`AGENTS.md` says so (disk is the only pressure that destroys rather than delays). It is the wrong
trigger anyway, because its root cause has already been adjudicated several times over and reaches
the same fifth Why each time: a command that reads as lane-local touches state shared across the
whole `.git` or the whole machine. `git stash`, `git reset --soft origin/main`, the `nife-dev`
symlink and `git checkout <file>` are four instances of one finding. A trigger that fires on a
repeat produces a second document with the first document's root cause in it. The right response to
a repeat is to move the existing action item up a rung, and that is an action item on the
original COE rather than a new one.

Rejected: a decision whose premise expired. §116 is the cleanest evidence in this file and it
still cannot be a trigger, which is worth stating because the two are easy to confuse. Expiry is a
condition, not an event: no moment announces that `redoxfs_server` has just falsified a decision
made yesterday. Something has to be watching, and "something is watching" is exactly rung zero. So
expiry belongs as an action item on some other COE (a decision deferred for want of a customer
carries the condition that would revive it, in a form a gate can check) rather than as a signal that
can fire.

And the ceiling matters more than the floor. A trigger that fires on everything produces
documents nobody reads, at which point the COE joins the index of scars it replaced.

## What remains open

There is no forum, and nothing here substitutes for one. Amazon's COEs are reviewed out loud, by
people who ask whether last week's action items shipped, and that review is where the process gets
its teeth. This tree convenes nothing. The gate proposed above can prove that an action item resolved
to a file on the day it was written; it cannot ask whether the milestone was built, whether the
proposal was triaged, or whether the decision was ever answered. `design/roadmap/proposals/` holds
files awaiting triage today and nothing schedules that triage.

This is named as unsolved rather than answered. Inventing a weekly ritual that one person would be
responsible for running is rung four wearing a process's clothes, and this tree has enough of those.
The honest position is that the trigger and the gate are worth having without it, and that the
follow-through remains an architect's attention until somebody proposes something better.

## What happens to `notes/corrections.md`

Recommendation: it stays, as the index, and each COE becomes its own record that it points at.
That is what the page already claims to be, in its own words: it names the note that carries the full
account, and it says it is the index of scars rather than a replacement for them. What changes is
what it indexes. Its three existing entries keep pointing at `notes/boot-protocol.md` and
`notes/stack.md`; new entries point at COE records.

COE records go in a directory, one file each, for the reasons `design/decisions/` was split into
files by milestone 114 (split `DECISIONS.md`, and give a decision a status): a number cannot be
claimed twice by accident, text cannot land under the
wrong heading, and a status flip stops being a conflict.

The path is `notes/corrections/<date>-<slug>.md`, and that is the standard rather than a
suggestion (calef, 2026-09-23, reviewing the first COE written against this section). The date is
UTC, as every date in this tree is. A record does not argue for its own location, say that its
location is provisional, or explain the naming authority: it sits at the path and describes its
event. It sits awkwardly beside
the file of the same stem, and the tidier end state is the index moving to
`notes/corrections/README.md`, which is a later lane's work and not this one's.

**This lane does not restructure that page.** It is the record of a ruling, not the execution of it.
