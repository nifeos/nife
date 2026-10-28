---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: 303
machine_requirements: none
specific_machine: none
needs_person: no
---
# 356. Retention declares the thread capability and says nothing about the region

Filed as a proposal on 2026-09-03 by the
`maintainer/spawn-retention-field` lane, while building DECISIONS §142's answer; promoted by
milestone 433 on 2026-09-19. Premise checked against the tree that day and unchanged:
`crates/supervision_contract/src/lib.rs` still declares `Retention` with exactly two variants
(`Nothing` and `ThreadControlBlock { reason }`), the region is named only in prose there ("after
`START` the only authority over this child is the region it was built from"). §142 (what a spawner
retains over a child after `START`) has been DECIDED since 2026-09-03 with nothing added about the
region. The three spawn paths this file names as
disagreeing still disagree.

The question is decided:
[§303 (the region)](../decisions/0303-the-retention-declaration-says-nothing-about-the-region.md),
2026-10-15. It keeps the declaration the TCB's, and it forbids building a child from a retained
delegated region. The enforcement is named as work for this milestone or its own: present the
owning region beside `build_ut`, and the kernel refuses when `build_ut`'s recorded parent is not
it.

**In brief.** §142 gave `ChildEndowment` a `retention` field. One struct literal now states both
what a child is given and what its spawner keeps. What it states is the disposal of one capability,
the child's `ThreadControlBlock`, which after `START` authorizes only pausing the thread
(`SUSPEND` and `RESUME`); its other methods refuse anything but an `Embryo`. The capability that
can *end* the child, and that
actually decides whether a child can be ended, is the region it was built from, and the endowment
says nothing about that one.

## Why this is not tidiness

The declaration's whole claim is that reading a `ChildEndowment` tells you the authority in play in
both directions. It does not, yet, and the half it omits is the powerful half. `MemoryRegion::DESTROY`
on the region a child was built from is the only thing in this system that can end a live thread
(DECISIONS §16's amendment). Whether a spawner keeps that capability is decided today exactly the
way TCB retention was decided before §142: one call site at a time, by whether it happens to call
`cap_delete(region)` afterwards.

The sites disagree, and the disagreement is real rather than accidental. `components/src/spawner.rs`,
`fixtures/src/c_confiner.rs` and `components/src/timetable.rs` drop the region as soon as the child runs, so
they hold nothing that reaches a live instance's memory and say so in comments. `crates/system_initializer`'s
job path keeps its region across the child's life and reclaims it on death. Both are correct for
what they do. Neither is stated anywhere a reader of the endowment can see.

## What §40 already says about it

[§40 (a supervisor's death is its subtree's death)](../decisions/0040-no-reaper-of-last-resort.md)'s
first recorded caveat was this question in other words: the ownership chain that makes a
reap work "holds only if children are built from the supervisor's own region," and whether building
a child from a *delegated* region is forbidden or merely discouraged "is not yet decided, and it
should be before someone writes a supervisor that does it." A declared field is the natural place
for that answer to live. The declaration is per-child and the rule is per-child.

## The three shapes §303 weighed

They are not equivalent, which is why this was a `DECISION` gate rather than a lane:

- **Widen `Retention`** into an enum over both objects, so one field says what is kept of each. One
  noun to learn, and the two are genuinely one question about one child.
- **A second field beside it**, since the region is passed to `build_child` as an argument
  (`build_ut`) rather than named in the endowment. The two capabilities have different lifetimes.
  The TCB is disposed of at `START` and the region at reap, which may be much later.
- Leave it out and say so in `Retention`'s own docs, on the ground that the region's disposal is
  a property of the *builder's* bookkeeping rather than of the child. A field which cannot be acted
  on at one moment is a declaration the code cannot enforce.

§303 chose the third, and forbade building from a retained, delegated region. The rule is in
`build_child`'s and `Retention`'s docs; the kernel check that enforces it is this milestone's work.

## Index row

DECISIONS §142 gave `ChildEndowment` a `retention` field so one struct literal states both what a
child is given and what its spawner keeps. What it states is the disposal of the child's
`ThreadControlBlock`, which after `START` authorizes only pausing. The capability that
actually decides whether a child can be ended is the region it was built from, and the endowment
says nothing about it. `components/src/spawner.rs`, `fixtures/src/c_confiner.rs` and
`components/src/timetable.rs` drop the region when the child runs, while `crates/system_initializer`
keeps its across the child's life: both correct for what they do and neither stated where a reader
of the endowment can see it. §40's first recorded caveat is the same question in other words.
§303 keeps the declaration the TCB's, states the region's rule in `Retention`'s and
`build_child`'s docs, and forbids building from a retained, delegated region. What remains is the
kernel check that refuses such a build.
