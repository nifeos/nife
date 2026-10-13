---
status: AMENDED
raised: 2026-09-22
decided: 2026-09-23
ratified_by: calef
---

# 207. The roadmap is a graph, and the block says so in fields a script can walk

The field names are as written below. Raised by calef, 2026-09-22, after a week in which milestone promotion kept
going wrong: *"I don't quite get why promotions are so hard. This has been killing us for weeks."*
and then, once numbering turned out not to be the cause, *"Even more important is expressing
dependencies between milestones so that we execute the graph of milestones efficiently."*

## Amended 2026-10-02: the backfill starts with the finished work, not the ready set

calef, 2026-10-02 (UTC), ruled that this amendment lands. It sets the scope in "Scope, so this is
not a 547-file project": the finished work first, not the ready set, which is the population the
census in "The measurements taken before ratifying" calls the wrong place to look.

The reference counts behind it were measured 2026-09-22 by counting references to other milestones in
each block's body. The census below is the better evidence; these stay because the gap between the
rows is the point:

| Status | Blocks | Mean references | With none |
|---|---|---|---|
| BUILT | 241 | 4.23 | 3% |
| NOT-STARTED | 212 | 3.33 | 5% |
| PARTIAL | 32 | 7.41 | 0% |

PARTIAL milestones, the ones still in flight and so still facing their dependencies, carried nearly
twice the references of finished ones. That is calef's creation-order bias showing up in a count.

What the field backfill did instead was measured 2026-10-02 from the frontmatter on `main`: every
PARTIAL, NOT-STARTED and PROPOSED block carries the five fields, and 7 of 279 BUILT blocks do. Of the
38 blocks that name a milestone dependency, one is BUILT. The census's 128 prerequisite edges
(`notes/dependency-census/dependencies.tsv`) appear in a field 4 times. So the graph a script can walk
today is mostly forward guesses, and the edges hindsight already found are not in it. The work to put
them there is `design/roadmap/0704-backfill-the-census-edges-into-finished-blocks.md`.

The amendment's original refutation test is not carried over. It said that if prerequisites were a
small minority of references, this section was wrong; the census found 3.8% and the section was
ratified on the eightfold under-declaration instead. "What would refute it now" stands as written.

## What is being decided

Four things, together, because each one is wrong on its own:

1. That a milestone block states its dependencies in two fields, on milestones and on decisions.
2. That a dependency is never cleared when it resolves, and blocked-ness is computed instead.
3. That a machine requirement is stated as a capability rather than as a host name, with a
   separate field for the rare case where one specific machine is the point.
4. That `**Gate:**` is retired, because the three fields above absorb every use it has.

## The measurement that prompted it

Taken 2026-09-22, across 547 milestone files:

| Gate | Blocks |
|---|---|
| `NONE` | 156 |
| `DECISION` | 62 |
| `HARDWARE` | 26 |
| `MILESTONE N` | 15 |

Fifteen dependency edges across 547 nodes, and 288 blocks with no gate line at all. There is no
graph to execute. There is a list with a handful of arrows, and the ordering that results is the
number a milestone happened to be minted with, which encodes allocation order and nothing else.

## Why dependencies must not be cleared when they resolve

Today's gate clears, and the clearing is enforced. `script/roadmap --check` fails when a
`Gate: MILESTONE N` names a milestone that has since become BUILT. That is how the promotion lane
found a stale gate on 2026-09-22, on a block whose named dependency had landed weeks earlier.

That rule is the defect, and it is a live source of the merge pain this project has been
absorbing. It means a milestone landing on `main` obliges an edit in every other block that
depended on it, made by somebody in a different lane who is not looking at that file. So a
dependency being satisfied turns unrelated branches red, on a line those branches never touched.
The tree already knows this hurts: milestone 385 (*when a milestone's status flips, tell the lane
which notes cite it*) exists because of exactly this coupling.

So:

- A dependency is a fact about the work and stays true forever. That milestone 554 (a good upgrade sticks: what marks a trial boot successful) needed
  milestone 525 (a bad upgrade cannot brick the machine: two boot slots, tries and priority) before it could exist does not stop being true once 525 is built.
- Blocked is a question, not a stored field. A milestone is blocked when any dependency it names
  is unresolved, computed at read time from the target's own status, which is recorded in exactly
  one place already.
- Nothing needs editing when a dependency lands. The only thing that changed is the target's
  status, and the lane that built it already updated that.
- The history survives, so "what was on the critical path to the install story" stays answerable
  after the fact, which deleting edges makes impossible.

The cost is that a block lists dependencies long since met, which reads as clutter. That is a
display problem, not a data problem: `script/roadmap` renders met dependencies struck through or
behind a flag. Cheap, and reversible, which is the right side of the *move fast on what can be
undone* line.

## The fields

Every milestone block carries all five. A missing field is a defect rather than an assertion, so
`none` is written out and silence never means anything. Since milestone 596 (the roadmap blocks get
frontmatter too) they are frontmatter, and this is milestone 101 (the L4 calibration, read from the
IPC number that pays for the trap)'s, as it stands on 2026-10-02:

    ---
    status: PARTIAL
    raised: 2026-08-04
    milestone_dependencies: 74
    decision_dependencies: none
    machine_requirements: riscv64 silicon; PMU cycle counter
    specific_machine: none
    needs_person: yes
    ---

The first two are the graph. The last three are the machine model below.

## Why the machine model is three fields and not one

`Gate: HARDWARE` conflates four different things, found by reading all 26 of them, and the
dominant one is not hardware:

- A person physically at a machine. The most common by far, phrased as *"the board is on the
  desk and this needs hands on it."* The hardware is not missing. calef's attention is, which
  this project calls its scarcest resource and which a field named `HARDWARE` hides from every count.
- A capability an emulator cannot provide, such as *"only a core with a genuinely ASID-tagged
  TLB"* or a real cache. Any host with the property will do, which is the substitution case.
- One specific machine, because a comparison is only valid like-for-like. §203 (capacity is rented rather than bought) states it: renting
  gives you *a* machine, and the point is *the same* machine as the reference. argon against seL4,
  and xenon's fastpath footprint keyed to its Core i5-7500T.
- An external account or rented resource that is not a machine this project owns.

So a host name is the wrong thing to write down. radon is a satisfier of "riscv64 silicon", not
the requirement itself, and §203 already priced a Scaleway RV1 that satisfies the same
requirement on different silicon. Naming the host forecloses that substitution in prose nobody
re-reads.

`specific_machine` therefore takes a host and a reason, and having to write the reason is
the point: it is the expensive case, and an unreasoned entry is the failure mode.

`needs_person` is the field that earns this section. It makes one query possible that
cannot be asked today and that §203 explicitly left open: *which milestones would rented metal
unblock, and how many bench-hours would it buy back.* That is the number that should decide §203's
undecided spend split between inference, runners and bare metal.

## What this replaces

`**Gate:**` is retired. `DECISION` becomes the decision field, `MILESTONE N` becomes the milestone
field, `HARDWARE` splits across the three machine fields, and `NONE` becomes every field saying
`none`. Nothing it expressed is lost, and three things it could not express are gained: more than
one dependency, a decision and a milestone at once, and the difference between a missing machine and
a missing person.

## Scope, so this is not a 547-file project

Backfill the finished work first: BUILT and PARTIAL blocks, starting from the census's 128
prerequisite edges, because hindsight records what a milestone actually stood on. The ready set is a
population selected for having no live dependencies, so it gets the fields as a lane next touches the
block. A block with no dependency fields is legal during the migration and reported by
`script/roadmap --unmodelled`, so the debt is visible rather than assumed away.

## What it buys, in the order the payoff arrives

1. A real ready queue. `--unblocked` means every dependency resolved, rather than `NOT-STARTED`,
   which today returns 145 items and is useless as a worklist.
2. Ranking falls out of the graph instead of being built separately: depth toward a goal orders
   the queue, and `design/fatal-risks/README.md` supplies the tie-break `AGENTS.md` already names.
3. Independent subtrees name the lanes that cannot collide, which is the constraint `AGENTS.md`
   says actually bounds lane count: not queue depth, but *"what files will this lane touch, and who
   else is in them."*

## What is blocked until this is answered

Nothing is blocked, and nothing should be built before it is ratified. The field names are calef's
call under this tree's naming authority, and the block format is something every lane and several
scripts agree on, which puts it in *anything two programs agree on* and therefore in the expensive
category.

## The measurements taken before ratifying, and what each one moved

The first refutation test in this section was wrong, and calef caught it (2026-09-22): *"I think
we have not captured the dependencies well because we create the dependent tasks after we've cleared
the dependencies."* It proposed counting edges in the ready queue, which counts a population
selected for having no live dependencies. Worse, this roadmap is written as work is taken up rather
than planned ahead, so a task is usually filed after whatever it needed has landed, and forward edges
are structurally under-recorded.

So the finished work was measured instead, by four lanes reading all 547 blocks
(notes/dependency-census.md):

| | |
|---|---|
| References classified | 3,381 |
| Genuine prerequisite edges | 128 |
| Edges declared in `Gate:` fields | 15 |

3.8% of references are prerequisites, which reads like a refutation and is not: 128 against 15 is
an eightfold under-declaration.

Then git was asked the same question, because calef doubted prose could see it: *"Wouldn't the
PRs or commits do a better job of identifying dependencies?"* It does, and the overlap is the finding.
Of 13 edges confirmed from branch bases and pull request bodies, zero appear among the
census's 128 prerequisites. Six are invisible to the prose entirely; seven were seen and filed as
mere context.

That pass also found 18 census rows whose direction is impossible, where the source merged before
the target. About eleven hold up on inspection, including one whose own evidence field contains the
string `RECLASSIFIED-NEGATION ... not gated on` and was filed as a prerequisite anyway.

Those 18 are the argument for the don't-clear rule, stated here because it is easy to read them as
an argument against the whole section. Every one is a claim about ordering. A computed `is_blocked()`
catches all of them for free, from each target's own status; a hand-maintained field cannot, and
today's clearing rule actively invites them.

## How the fields get filled, which is not by remembering

A lane writing a field is rung four, and the measurement above says so. The prose route missed
every edge git found. So the fields are populated by a step rather than a duty:

- Branch base. A lane cut from another lane's branch rather than `main` is an unambiguous
  dependency recorded as a git fact. One API field, 100% precision on what it catches, and it fired
  on only 7 of 1,066 merged pull requests, so it is cheap and low-recall.
- A narrow idiom regex, over the small set of bolded forms this project already writes by
  convention: `**Stacked on #N**`, `**Based on ... (#N)**`, `**Depends on #N**`, `Blocked-by: #N`.
  Scoped to those, not to dependency vocabulary generally.
- Merge order as a periodic audit, not a per-merge gate, since it needs both ends mapped and
  merged to say anything and that holds for 61% of rows today.

A general dependency-word sweep is refused, and the reason is the failure mode rather than the
cost: *"this does not need #1088"* sliding through as a false edge is silent, and silence is what
this whole section exists to remove.

## The floor no mechanism gets past

A dependency that resolved before the dependent branch was cut leaves no trace in git or in prose.
Nobody wrote a sentence and no branch points anywhere. That is calef's creation-order observation
stated as a limit: the backfill recovers history and pattern, the idiom parser catches what an author
bothered to mark, and neither can recover a dependency that was simply satisfied before anyone needed
to mention it. This is why the fields are asked for going forward rather than reconstructed, and
why the backfill is evidence rather than the deliverable.

## What would refute it now

The section has survived the tests above, so what remains is an operational failure rather than an
empirical one: if the fields are added and then go stale, because nothing populates them and no
reader consults them, this will have bought a format instead of a graph. The tell would be
`--unblocked` returning roughly what `NOT-STARTED` returns today.
