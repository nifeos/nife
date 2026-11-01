---
status: AMENDED
raised: 2026-09-20
decided: 2026-09-20
ratified_by: calef
---

# 201. One roadmap until a citation has to cross, and the blocked side declares the dependency

calef, 2026-09-20, on the three asks of the proposal
`the-roadmap-after-the-split`. *(Section number provisional until the merge queue lands it; §200 is
in flight on another branch.)*

*Amended 2026-10-28 (UTC) on #2851: section 4 states the citation identity across repositories,
answering the deferral in section 1. The moved-things registry lives at `packages/moved.toml`, not
at `design/roadmap/MOVED`.*

*Amended 2026-11-01 (UTC) on #3034: the numbering namespace in section 4 moves with the record to the
`nifeos.org` repository, and `nifeos/nife` remains for history only.*

The ruling, in one line. Keep option E, one roadmap in `basalt`, until the split makes a
citation cross a repository boundary. Record the deferral rather than letting it happen by default.
When the split comes, express a cross-repository dependency the way this tree already expresses
an in-tree one, on the blocked side only.

## 1. The citation identity is deferred, deliberately

calef: *"It seems like this is a deferrable decision once we split everything. So we could stick with
E for now and then decide once we're working across multiple repos."*

Why deferring is the right call here and not merely the cheap one. The expensive part of option C
(the slug becomes the identity) was never the decision, it was the 3,148 citation sites that would
have to resolve by something other than a bare number. Milestone 444 (a citation says what it cites)
shipped the gloss ratchet on the morning of this ruling, and it converts those sites at the rate the
tree changes. Waiting therefore makes C cheaper rather than dearer, which inverts the usual
deferral trade. The decision's real input is also a thing this project does not have yet: what a
citation across two live repositories is like to write and to read.

Two conditions make the deferral honest rather than passive.

- **`design/roadmap/MOVED` is written at the split, not after.** (The 2026-10-28 amendment puts
  this registry at `packages/moved.toml` instead, keeping the form; section 4 states it.) FreeBSD's ports form,
  `old|new|date|reason`. It is the one thing that turns every option in the proposal from lossy into
  merely indirect, and it is only writable while somebody still knows where each record went. Under E
  it stays nearly empty, which is exactly why establishing it costs nothing now.
- **The revisit condition is an event, not a date**, in the shape milestone 448 (a refusal gets a
  number, a status, and a condition that would change it) made mandatory. The condition is the
  first citation that has to cross a repository boundary, and equally the first time two
  repositories mint the same number independently.

The cost being accepted, stated because the options table underplays it. Under E the number stays
hand-minted and global. Milestone 443 (lanes wait on each other for three reasons) made a collision
cheap to *resolve* rather than impossible to *have*. Two sessions that cannot see each other still
both reach for the next free number, and the integrator arbitrates at merge. That happened on the day
of this ruling, when one lane reserved 449 to 490 and reported its ceiling so the maintainer could
mint 491 to 516 above it. That coordination was a message between two agents, which is rung four,
and after a split those sessions may not share a machine.

## 2. A cross-repository dependency is declared by the blocked side

calef's expected direction, recorded as direction rather than as a decision to build now: *"I suspect
we evolve to issues per repo and as we have cross repo issues we have deliverables in both repos that
reference each other and express dependencies."*

The half that is adopted. The *edge* between two repositories' work is the thing that genuinely
has no home in either tree, and naming it is the right instinct: putting it in one repository makes
that repository authoritative over another's schedule.

The half that is amended, and the amendment is the whole of this clause. Deliverables in both
repositories referencing each other means two records asserting one relationship. That is
DECISIONS §90 (the claim is a draft pull request)'s "the same fact in two places", and it drifts in
the worst direction: the blocker is built and updated, and the blocked side still says it is
waiting. This tree has that failure on file, in
milestone 16 (real hardware + IOMMU-backed driver isolation), which listed three things as remaining
when two were finished, and calef caught it by asking rather than any mechanism catching it.

So: one writer per fact, which is what the tree already does. Every non-BUILT block carries a
`Gate:` line, and `MILESTONE N` in it means this waits on that. The dependency is declared by the
thing that is blocked and never by the thing that blocks. Extending it across repositories is a
spelling change rather than a new mechanism: the blocked repository's block says what it waits for,
and the blocking repository says nothing and needs to know nothing.

**The caveat against this ruling's own recommendation.** A one-sided cross-repository gate is only
machine-checkable if the checking repository can see the other's records, which is the proposal's
option D and its stated cost: a gate whose answer depends on what the operator has cloned. Under E
that is moot, since there is one roadmap. It becomes live at exactly the moment the split happens,
which is a third reason the `MOVED` file earns its place at the split rather than after it.

## 3. Issues, for the two things a tree cannot do

Adopted as asked. GitHub Issues are for a stranger's bug report on a released artifact (they have
no commit bit and no checkout) and for cross-repository coordination that is **not** a dependency.
Not for milestones, not for status, not for claims, and not for the argument behind a piece of work,
which stays in the tree. `AGENTS.md`'s third principle is that a newcomer succeeds with only this
repository, and an argument in a tracker fails that for anyone with a clone and no network. Twenty-one
scripts under `script/` read `design/roadmap/` directly, and every one of them is a reason the record
stays readable offline.

The proposal's own prior art is the evidence: all eight projects it checked keep the durable record in
a versioned tree.

## 4. The citation identity across repositories

Ruled 2026-10-28 (UTC), on the question section 1 deferred, ahead of the first code cut (milestone
756 (process-tools, core-tools and disk-tools are the first code to leave this repository)), the
revisit event section 1 defines. The rule, in force now:

- One-way links. A record points at code as `nifeos/<repo>@<sha>` for a commit and as
  `nifeos/<repo>` for a repository. Code does not cite roadmap records or decisions. When code
  moves out, the moves/stays/gone sort (ruled 2026-10-26,
  [§315 (a crate's documentation leaves with it)](0315-a-crates-documentation-leaves-with-it.md),
  its number provisional until the merge queue mints it) converts citations to rulings into
  reasoning carried in the crate's own documentation.
- Ratify a name before anything moves, and leave ratification history behind: a component
  repository receives names, not their history.
- The moved-things registry is `packages/moved.toml` (provisional until the first move writes
  it), a sibling of `packages/homes.toml`: one `[[moved]]` table per departure, holding the old
  path, the new home, the date and the reason. The move writes it; nothing creates it earlier. The
  location `design/roadmap/MOVED` (milestone 533 (whether the roadmap should become issues, and
  what a citation means after the repository split)'s mechanism) gives way to it; the substance
  holds: the FreeBSD four-field form, append-only, written at the split.
- The numbering namespace stays with the record repository, `nifeos.org` from 2026-11-01 (UTC), and
  moves with the record when it leaves `nifeos/nife`, which remains for history only. A record's
  number stays its identity. A bare code path in an existing record means that path at the last
  `nifeos/nife` commit before the move, the anchor, which `packages/moved.toml` records in one line.
  New text uses `nifeos/<repo>:<path>`, resolved at basalt's pin. The site gives each number a stable
  address derived from the number alone. The revisit condition stays armed: when a component
  repository first needs numbered records of its own, milestone 533's options A and C re-open on
  evidence.

## What is not decided here

Whether a component repository ever needs numbered records of its own. Section 4 arms that
trigger; until it fires, one numbering namespace, the record repository's, covers every record.
