# nife: user journeys

*Name: ratified 2026-08-26 (calef's own word: "I'd like to start tracking user journeys"), not a
lane's choice to make provisional.*

A journey is an end-to-end, user-facing story, told as an ordered bundle of the milestones (and,
where a step is blocked by one, the decisions) that have to be true at once for someone to actually
live it. `design/roadmap/` tracks work item by item; a journey is the read that answers a different
question than any single milestone can: *could a person actually do the whole thing today, start to
finish?* Journey 1 (login to `kilo`) is the worked example this directory was minted from: no single
milestone was "blocked," but tracing the story end to end found a real, unowned gap (milestone 177)
that nothing surfaced by scanning the roadmap milestone by milestone.

## How this directory works

One journey, one file, named `NN-slug.md` (single digits zero-padded so the directory sorts, the
same convention `design/roadmap/` uses). calef, 2026-08-26, on adding the second journey: numbers
should be the shorthand identifier, cited as **`journey N`** in prose, the same shape `milestone N`
already has. A journey's number is its identity, the way a milestone's or a decision's is:
`journey 1` is `01-login-to-kilo.md` and nothing else. Roadmap blocks, decisions and notes cite a
journey by number, so the number is load-bearing: the filename and the H1 have to agree, and
`script/journeys --check` fails a file where they do not.

## The frontmatter

Every journey opens with frontmatter, and it is the only place a journey's status lives. The format
is the one `design/decisions/` and `design/roadmap/` use: `---`, flat `key: value` lines, `---`; no
lists, no nesting, dates UTC.

```yaml
---
status: PARTIAL
raised: 2026-08-30
---
```

| key | values | required when |
|---|---|---|
| `status` | `PROPOSED`, `WORKING`, `PARTIAL`, `DELIVERED`, `RETIRED` | always |
| `raised` | `YYYY-MM-DD`, UTC | always |
| `retired` | `YYYY-MM-DD`, UTC | `RETIRED`; refused on every other status |

| status | means |
|---|---|
| `PROPOSED` | A story the project intends to tell, and no cited milestone is `BUILT` yet. |
| `WORKING` | Somebody is on it now. |
| `PARTIAL` | Some steps are delivered, more remain, and nobody is on it. |
| `DELIVERED` | Every cited milestone is `BUILT`: a person can do the whole thing today. |
| `RETIRED` | The project stopped telling the story. The file stays, and `retired` says when. |

The number stays in the filename and the title in the H1, as a milestone's and a decision's do.
There is no `title:`, `number:` or `delivered:` key: `script/journeys` derives the delivered date
from the latest `built` date among the cited steps.

**Progress stays computed.** `script/journeys` counts the steps sitting on a `BUILT` milestone on
every run, because a hand-written count drifts the moment one step moves. There is no progress
token, and a journey's prose never restates the status or a date as a field-shaped line.

`script/journeys --check` fails a file with no frontmatter or no `status`, a status outside the five
words, a `raised` or `retired` that is not `YYYY-MM-DD`, a `retired` on any status but `RETIRED`, a
`RETIRED` journey with no date, a `DELIVERED` journey citing a milestone that is not `BUILT`, and a
`PROPOSED` journey citing a milestone that is `BUILT`.

## Index

GitHub renders this README as the directory index. `script/journeys` is the live view, with each
journey's status; this table is the identity mapping only: "what number is this", never "what state
is it in". Neither `design/roadmap/` nor `design/decisions/` keeps an index any more, and the reason
they can afford not to is that nothing there needs a second copy of a number.

| # | journey |
|---|---|
| 1 | [Boot to a login prompt, into `swish`, to editing a file with `kilo`](01-login-to-kilo.md) |
| 2 | [Run a Time Machine backup host](02-time-machine-backup-host.md) |
| 3 | [The same story, on real silicon, on all three architectures](03-login-to-kilo-on-silicon.md) |
| 4 | [Install a system composed from packages, and update it from packages](04-install-and-update-from-packages.md) |
| 5 | [Externalize the publish-ours crates, one release at a time](05-crates-released-on-their-own.md) |

## The format

A journey file is the frontmatter above, a short narrative (what the story is, why it matters, and
what "done" looks like), and one table:

| step | milestone | decision | what this step needs |
|---|---|---|---|
| 1 | 33 | | one line naming what this step of the story needs from that milestone |

- **step.** Ordinal, for reading order only; `script/journeys` does not enforce that a lower step
  finishes before a higher one starts; some journeys have steps that are genuinely independent
  (journey 1's own steps 3 and 4 are unordered with respect to each other, and its own prose says
  so).
- **milestone.** The milestone number this step's progress is read from. Required, and the block it
  names must exist.
- **decision.** The number of a decision this specific step is additionally blocked by (`§120` is
  written `120`), when the milestone's own status does not tell the whole story (a `PARTIAL`
  milestone can still have the one piece a journey needs sitting behind a separately-decided hold;
  journey 1's own step 4, before DECISIONS §120 was amended 2026-08-26, is the worked example).
  Blank when the milestone's own status is the whole answer.
- **what this step needs.** One line, human-written, naming the specific slice of that milestone
  this journey's story needs, not the milestone's full scope. This is the column a milestone's own
  coarse status cannot carry: `PARTIAL` says nothing about *which* part is missing.

Every journey carries at least one step row, and `--check` fails a file with none.

## The task: add a journey

1. Write `design/journeys/NN-<slug>.md`, the number zero-padded so the directory sorts. It opens
   with frontmatter, then `# N. <title>`:

   ```markdown
   ---
   status: PROPOSED
   raised: 2026-10-29
   ---
   # N. <title>
   ```

   The keys and the rules for each status are under *The frontmatter* above. The prose may not
   restate them.
2. Tell the story: what it is, why it matters, and what a person can do at the end of it. Journey
   1's `## What "done" looks like` section is the shape.
3. Add the step table, one row per step. Each row names the milestone, the decision when one also
   blocks the step, and the slice of the milestone the story needs. The columns and their rules are
   under *The format* above.
4. Run `script/journeys --check`, then `script/journeys N` to read the story against the blocks.
5. That is the whole procedure: no table to update, no regeneration to wait for. A lane picks the
   next free number provisionally, and the integrator mints it at merge like every other global
   name.

## Reading a journey's state

`script/journeys` prints the status, the raise date and, where they apply, the retired and delivered
dates from the frontmatter and the cited steps. Then it prints, per step, the milestone's status
from its block and (when a decision is cited) that decision's status from its frontmatter, and a
one-line rollup of how many steps sit on a `BUILT` milestone. A step whose milestone is `BUILT` but
whose cited decision is not `DECIDED` in the direction the step needs still reads as blocked; the
tool does not try to parse *which way* a decision went; it says what a step needs and lets a reader
hold the decision's own text against it.

**The state report is not a gate.** Nothing fails a build over a journey's progress: a journey can
regress if a milestone it depends on is re-scoped down to `PARTIAL`, and that is information rather
than a defect for CI to catch. What `--check` gates is the record, the schema above and the status's
agreement with the steps it cites.

## BUGS

- `script/journeys --check` runs in no automatic gate. `script/lint`, the pre-push hook and CI do not
  call it, so a journey file that breaks the schema surfaces only when somebody runs the command.
  Wiring it into `script/lint` changes that script's section list and wants its own lane.
