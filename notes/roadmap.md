# The roadmap

*Name: provisional (a lane ships names provisionally; an architect names things). This file was
`notes/roadmap-index.md` until 2026-09-21, when the index it was named for was retired and its
vocabularies moved here.*

`design/roadmap/` is one file per milestone and nothing else. The directory listing is the
index. calef, 2026-09-21: *"I don't think the milestone index table is worth building any more.
It has too many rows to be digestible as a table."*

So there is no `design/roadmap/README.md`. What was in it that is not a row is here: the status
vocabulary, the frontmatter every block opens with, the follow-on dispositions, the rule that anybody may add a
proposal, and the history of the table itself. The rows are not here and are not anywhere, because
every one of them was generated from its own block's `## Index row` section and still lives there.

The queries are the other half of the index, and what a reader wants more often than 522 rows:

```console
$ script/roadmap --ready          # nothing open that it depends on, NOT-STARTED or PARTIAL: startable today
$ script/roadmap --outstanding    # what a PARTIAL milestone still says is left
$ script/roadmap --proposed       # unnumbered proposals, oldest first
$ script/roadmap --unclaimed      # follow-on work a finished milestone named that nobody took
$ script/roadmap --revisit        # a REFUSED milestone whose stated condition has come true
$ script/roadmap --check          # what the gate runs
$ script/roadmap --unmodelled     # the blocks that carry no dependency fields
$ script/roadmap --migrate FILE   # convert a block written in the old prose form (see below)
$ script/roadmap                  # every milestone that is not BUILT, grouped by what it waits on
```

## The task: add a milestone to the roadmap

You are a lane given milestone `N`, and you want it on the roadmap.

1. Write `design/roadmap/NNNN-<slug>.md`, the number padded to four digits. It opens with
   frontmatter, then `# N. <title>`:

   ```markdown
   ---
   status: NOT-STARTED
   raised: 2026-09-26
   milestone_dependencies: 527, 543
   decision_dependencies: none
   machine_requirements: riscv64 silicon; PMU cycle counter
   specific_machine: none
   needs_person: yes
   ---
   # N. <title>
   ```

   The keys, and what each status requires, are under *The frontmatter* below. The prose may not
   restate them: no `**Status:` line, no `**Gate:` line, no `**Built:**` line.
2. Give it a `## Index row` section, conventionally last, beside `## Follow-on` and `## BUGS`,
   holding one paragraph saying why this milestone matters. The Built date is the `built` key.

   The heading keeps its name although the index is gone (`script/audits` and
   `script/fatal-risks` still read it); renaming it is an architect's call.
3. Name what you considered taking, in a `Reuse:` line or a `## Reuse` section, or write
   `none exists` and where you searched. §46 (thin primitives or whole subsystems) makes taking the
   default outside the kernel and the crates Kani proves; `--check` requires this from 2026-10-05.
4. That is the whole procedure: no table to update, no regeneration to wait for.

### EXAMPLES

```console
$ script/roadmap --check
roadmap: 585 milestones, 586 files, status vocabulary clean, every '## Index row' parses, ...
roadmap: 280 of them classified by gate, 154 ready to start (script/roadmap --ready)
```

```console
$ script/roadmap --index | head -3     # the five columns, for the two scripts that read them
| #  | Status | Milestone | Why it matters (§14) | Built |
|----|--------|-----------|----------------------|------------|
| 1 | BUILT | [First boot][0001-first-boot.md] | ... | 2026-07-12 |
```

## The vocabularies

### The frontmatter

Every block and every proposal opens with it, since milestone 596 (the roadmap blocks get
frontmatter too). The schema was ratified by calef on 2026-09-26 (UTC), *"Yes to all"*, after the
same move for `design/decisions/` in milestone 582 (a decision's status becomes a field, and the
index becomes generated). The format is 582's: flat `key: value` lines between two `---`, no
nesting, a list written as one comma-separated value, dates UTC.

| key | values | required when |
|---|---|---|
| `status` | the tokens below; `PROPOSED` for a proposal | always |
| `raised` | `YYYY-MM-DD` | always |
| `built` | `YYYY-MM-DD` | `BUILT`; optional on `REMOVED`; otherwise refused |
| `branch` | a branch name | `IN-PROGRESS`; otherwise refused |
| `promoted_from` | a proposal slug | optional |
| `superseded_by` | milestone numbers or `§N`, comma-separated | `SUPERSEDED` |
| `refused_by` | milestone numbers or `§N`, or `none` | `REFUSED` |
| `milestone_dependencies` | milestone numbers, or `none` | every status but `BUILT`, `REMOVED`, `REFUSED` |
| `decision_dependencies` | section numbers, `unwritten`, or `none` | the same |
| `machine_requirements` | capabilities, or `none` | the same |
| `specific_machine` | `<host> (<reason>)`, or `none` | the same |
| `needs_person` | `yes` or `no` | the same |

The five dependency fields travel together. They are §207 (the roadmap is a graph, and the block
says so in fields a script can walk), and they replaced the `**Gate:**` line: see *Dependency
fields* below. The schema is spelled once, in `helpers/roadmap_block.py`, and a lane does not extend
it.

A block written in the old form. Until 2026-09-26 each of these facts was a bold line in the
prose (`**Status: BUILT.**`, `**Gate: NONE.**`, `**Built:** 2026-09-14`), read by regex.
`script/roadmap --check` now refuses that form and names the command that converts it,
`script/roadmap --migrate <file>`. `briefs/rebase-onto-main.md` has the case for a lane that wrote
its block before the switch.

Status vocabulary. The `status` key in a milestone's own file is the *one* canonical place its
state lives, and it uses exactly these tokens. This exists because status in prose scatters through the detail
blocks, phrased a dozen different ways ("Built 2026-07-28", "DONE", "Phase 1 built", "Largely done"),
which is unreadable at a glance and, worse, unparseable: a status sweep on 2026-07-30 mis-reported eight
milestones. `script/roadmap` validates this column and fails on anything outside the vocabulary.

| Token | Means |
|---|---|
| `BUILT` | Complete, and proven by the gate on every supported ISA. |
| `PARTIAL` | Some phases shipped and more remains, with nobody currently on it. The block says which phases. |
| `IN-PROGRESS` | Active work on a branch right now. **The block carries a `branch` key**, and `script/roadmap --check` fails if that branch has already merged. |
| `REMOVED` | Was built, in whole or in part, and the code was deliberately deleted. The block must record when it went and why. The row keeps its `Built` date if it ever had one. |
| `NOT-STARTED` | Specified, nothing built. |
| `OPTIONAL` | Deliberately off the thesis path; not a backlog item. |
| `REFUSED` | Work considered and deliberately not taken. **The block must carry a `## Revisit` section** saying what would change it, and `script/roadmap --check` fails without one. Not a backlog item: it is excluded from `--ready`, from the gate classification and from every count that reads as outstanding work. |

The `status` key is the only record of a milestone's state, and since 2026-09-21 there is no
second one to disagree with it. A detail block may narrate its state in prose, where the evidence
belongs, but the key answers "where do we stand". `REMOVED` and `IN-PROGRESS` each oblige the block
to say something no other status asks: when the code went and why, or which branch holds the work.
`REFUSED` obliges a `## Revisit` section.
Why each word exists, and the failures that minted `REMOVED`, `REFUSED` and the `IN-PROGRESS`
check, are in [roadmap/vocabulary-history.md](roadmap/vocabulary-history.md).

### Dependency fields

The status says where we stand; it does not say what stops a lane from starting. Five fields
answer that question, and they are §207's:

| field | says |
|---|---|
| `milestone_dependencies` | Work this roadmap tracks that this needs first, by number. |
| `decision_dependencies` | Sections in `design/decisions/` this waits on, or `unwritten` for a fork nobody has written up yet. |
| `machine_requirements` | A capability an emulator cannot provide ("riscv64 silicon with an ASID-tagged TLB"). Any host with it will do. |
| `specific_machine` | One machine, only where that machine is the point, with the reason: `argon (the Jetson TX1 under seL4's published numbers)`. |
| `needs_person` | `yes` when somebody has to be at a machine or an account: flashing, a serial console, a firmware setting. |

`unwritten` means the milestone waits on a fork nobody has written up yet. A dependency is never
cleared by hand: it stays true after its target lands, and whether it still blocks is computed
from the target's status. `needs_person` is separate from the machine fields because the old
`HARDWARE` gate hid it: 33 of its 34 uses needed a person. The reasons are in
[roadmap/vocabulary-history.md](roadmap/vocabulary-history.md).

`script/roadmap --ready` offers a milestone when its status is `NOT-STARTED` or `PARTIAL`, every
dependency is resolved, no machine is required and nobody needs to be present. The full report groups
what is not ready by what it waits on.

### Follow-on dispositions

Every finished block answers a `## Follow-on` section, which says what happened to the work the
milestone named on its way out (milestone 247 (follow-on work named by a finished milestone goes
nowhere, and this is the third time)). That work is the thing this project keeps losing.

A block whose status is `BUILT`, `REMOVED` or `PARTIAL` carries the section, and
`script/roadmap --check` fails on one that does not. It is a bullet list, and each bullet opens with
one of eight dispositions:

| Bullet opens | Means | Must resolve to |
|---|---|---|
| `**None.**` | Nothing was identified. | Nothing; it stands alone as the whole answer |
| `**Milestone N.**` | It became milestone N. | A block under `design/roadmap/`, not this one |
| `**Done.**` | It was done, and not as a milestone. | What carried it: a pull request, a branch, a file |
| `**Recorded.**` | It is a limitation and it stays one. | Prose, and any path it cites must exist |
| `**Refused.**` | Considered and deliberately not taken. | A reason, in prose |
| `**Decision.**` | It is an architect's call, written up as one. | A file under `design/decisions/` |
| `**Proposed.**` | Named, nobody took it, so it is now a proposal. | A file under `design/roadmap/proposals/` |
| `**Outstanding.**` | Still this milestone's own remaining scope, checked against the tree and still true. `PARTIAL` blocks only. | What is left, and how you checked |

A `PARTIAL` block's section is a standing claim about remaining work, which is why it alone may
say `Outstanding.` and may not say `None.`. The gate checks that every claim is enumerated and
resolves to something that exists; it cannot check that a claim is still true. An explicit
`None.` is a success, because the defect is silence. `Recorded.` points at a `BUGS` entry and never
replaces one. How `Outstanding.`, `Proposed.` and `Done.` arrived, and what each refuses, is in
[roadmap/vocabulary-history.md](roadmap/vocabulary-history.md).

The whole vocabulary is provisional until calef ratifies it. `REMOVED` was minted by him and
these six words are a lane's, offered with the sweep that produced them.

## Anybody may add to the roadmap: `design/roadmap/proposals/`

Ratified by calef on 2026-09-03, and it dissolves a rule this tree had been treating as one thing.
A lane writes a proposal itself, as `design/roadmap/proposals/<slug>.md`, with no number in the
name. No coordination, no maintainer in the path, no waiting.

*"On a human team, anybody should be able to add to the roadmap. That's different than prioritizing
that roadmap."* Lanes were barred from minting because concurrent lanes cannot see each other and
two reaching for the same number collide. The collision is in the number, not in the authority,
and conflating them meant a lane that found work had to route it through a report, through the
maintainer, into a decision that might be deferred. Every hop is a chance to lose it, and on the day
this landed the maintainer had buried three items by deferring them into chat messages. Prioritizing
the roadmap is still calef's, and so is every number and every name.

Why a slug and not a GUID, which was considered and rejected. Milestones are cited in prose
constantly and a GUID cannot be said out loud. This tree already found bare numbers too opaque, which
is why it cites a milestone by number *and* name; a GUID moves further along the axis that already
needed correcting. The slug is the readable half of a filename this directory already uses, so
nothing new has to be learned, and two lanes collide only by choosing the same words, which is
visible rather than silent. Numbers stay for promoted blocks, where they also carry recency for free.

A proposal carries the same frontmatter a numbered block does, with `status: PROPOSED` and the
date it was written as `raised`, so promotion changes almost nothing:

```markdown
---
status: PROPOSED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# <Title, the way a numbered block is titled>

Written by <who or what>, from milestone <N>'s block.

**In brief.** <what the work is>

Reuse: <what existing code was considered, and why it was or was not taken>
```

Promotion is the integrator's, at merge, like every other global name: give the file its number,
`git mv` it up a directory, set its `status`, and add `promoted_from` with the slug. `script/roadmap --check` validates the rest, and nothing else has to
happen, because there is no table for it to appear in.

The graveyard question, asked here rather than discovered later. A proposal nobody promotes is
the same burial in a new location. Nothing can force a promotion, and a gate that tried ("no proposal
older than N days") would be routed around by not writing proposals, which is worse than the pile.
What the gate does instead is make the pile impossible to miss: `raised` carries the date it was
written, `script/roadmap --check` prints the count and the oldest date on every lint run, and
`--proposed` lists them oldest first. An unread number is still rung two, where a paragraph in a
finished block was rung four.

The dependency fields say what stops a milestone's headline deliverable, and several milestones
have a startable piece behind a blocked headline: milestone 88 (nife on rented silicon)'s stage 1 boots UEFI locally with no cloud account. The prose says so where that is true,
and no gate checks it.

The converse is a rule. A dependency on part of a milestone means that milestone is too coarse:
split it, so the field names the part (calef, 2026-10-06).

## Effort

The milestone files give effort in lanes: one lane is one agent session end to end, which measured
31 to 57 minutes of wall clock across the first fourteen merged milestone branches (2026-07-30).
That calibration, the S/M/L scale it replaced and how to re-derive it are in
[roadmap/effort.md](roadmap/effort.md).

## Why this is markdown in the tree and not GitHub Issues

Decided 2026-07-30 by calef. A second place where status lives is a second source of truth, and
the roadmap in the tree is version-controlled with the code and checked by `script/roadmap --check`.
Revisit when more than one person files work; the shape then would be markdown canonical and issues
generated one way from it. The argument in full is in
[roadmap/why-not-issues.md](roadmap/why-not-issues.md).

## The index, and why it is gone

The roadmap has no index table. A generated one fixed a merge problem (eight conflicts in one
session, all in the table), and having none answers a readability one. Nothing was lost, because milestone 294 (the
index is generated, not hand-maintained) had already moved the two hand-written columns into the
blocks. `script/roadmap --index` still assembles the five columns to stdout for the two scripts
that read them. The record of the generation and its measurements is in
[roadmap/the-retired-index.md](roadmap/the-retired-index.md). What the gate stopped checking while
the table was generated is in
[milestone 294's block](../design/roadmap/0294-the-index-is-generated.md).

## BUGS

- `## Index row` is named after a thing that no longer exists. The section is still where a
  block states its Built date and its one-paragraph precis, and both are still read, so the name is
  wrong rather than the section. It appears in every numbered block in `design/roadmap/`, and a
  rename is an architect's (`design/naming.md`). Until then, read it as "the two facts about this
  milestone that are not in its prose".
- `## Index row` was provisional when it was minted, too. `## Summary` is the generic word this
  tree's naming tenet warns against, and `## Why it matters` was unavailable: 13 blocks already
  carry one, holding a different kind of text.
- The section is one paragraph, and a gate enforces that. It was one paragraph because a table
  cell is; the table is gone and the constraint stayed, because relaxing it is a change to every
  block for a block that has not wanted two paragraphs yet.
- `script/metrics` and `script/catch-up` read the committed table at historical revisions, and
  those revisions still have it. For revisions after 2026-09-21 both fall back to counting the
  blocks. The two answers are not identical for the weeks in between: a week whose revision carried
  a stale table counts what the table said, which is what that week's reader would have seen.
- Two lanes minting one milestone number can still collide, which `script/roadmap` calls fatal.
  `script/claim` refuses a number an open pull request holds (2026-10-10 UTC); same-minute claims
  and hand renumbers still slip past it.
- Nothing publishes any of this. calef's sentence retiring the table named a website as where
  the project plan eventually goes; there is no publishing story today, and the gap is written up
  in `design/roadmap/0565-a-website-for-the-project-plan.md`.
