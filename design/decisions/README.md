# nife: Architecture Decisions

*Name: ratified 2026-08-04 (calef, §75 (directories under `design/` and `notes/` carry provenance in their own README) covers this directory). `decisions` names what the files are, one each, and the tree cites them as `§N` regardless of the directory's name. The plural is deliberate, since a singular would read as one decision about design rather than a place where decisions live.*

Each file records one decision: what we chose, what we rejected, and why. The first were made on
2026-07-12, before any code was written. Revisit these deliberately, not accidentally.

## How this directory works

`script/decisions` is the index. It reads every file's frontmatter and prints the decisions grouped
by status, and `script/decisions --check` validates them in `script/lint`. Nothing here is generated,
so a lane minting a section writes one file and nothing else.

One decision, one file, named `NNNN-slug.md`. The number is the identity: `§14` (the project's
direction) is `0014-project-direction.md` and nothing else, and the 2,000-odd `§N` citations spread
across the kernel, the crates, the notes and the roadmap resolve here.

The number in the file name is zero-padded to exactly four digits (calef, 2026-10-07 UTC), so a
directory listing sorts in numeric order; before that ruling `100-` sorted between `10-` and `11-`.
Four, not three, so nothing is renamed again at 1000, the same width he ruled the same day for
`design/roadmap/`. Only the path is padded: a citation stays `§14`, and the H1 stays `# 14. <title>`.
`script/lint` fails a numbered file here whose number is not four digits, so a decision opened on a
branch cut before the rename has to be renamed before it lands.

This was one 5,320-line file until milestone 114 (split `DECISIONS.md`, and give a decision a status). Splitting it does the same three things the
roadmap split (milestone 76) did one directory over:

- **A number cannot be claimed twice by accident.** Two lanes both wrote `## 30.` into the single
  file on 2026-07-30 and it merged clean, because both sides' prose survived. As two files claiming
  one number it is a gate failure, which is what `script/decisions --check` now reports.
- **Text cannot land under the wrong heading.** §67 was appended into the middle of §61's BUGS
  section on 2026-08-03, because the insertion anchored on the string `## Reading` and the first
  occurrence of that string in the file was a wrapped prose line inside §61. Every gate reported
  clean for a day. `cat >>` into `0067-second-stream.md` can only add text to §67.
- **A status flip stops being a conflict.** Marking one decision superseded used to edit the file
  every other lane was also editing.

The index went the same way in two steps. Milestone 582 generated its table from the files, which
ended the hand-edited row every lane minting a section had to add. The generated table still
conflicted, because every new or changed decision rewrote one committed file. On 2026-10-12 (UTC)
seven pull requests hit it in one day, and calef ruled to retire the table and keep this prose. He
had made the same call for the roadmap's table on 2026-09-21 (#1079).

Do not renumber. A number that moves breaks citations that no gate can see are wrong, because a
well-formed citation to the wrong section still resolves. Milestone 97 is where that check gets
built, and this directory is what makes it cheap: once a decision is a file with a title, a
citation's parenthetical name can be compared against that title.

Two blocks that lived among the decisions are not decisions, and they moved out with the split the
same way milestone 76 (split the roadmap, one file per milestone) moved four essays out of the
roadmap. [The open design
ideas](../open-design-ideas.md) are proposals parked against a trigger rather than choices made.
[The original eleven-milestone plan](../original-milestone-plan.md) is the record of what was
planned before anything was built; the roadmap backfilled it into `design/roadmap/`.

## Frontmatter, which is where the status lives

Every decision opens with frontmatter, and it is the only record of its status. The prose may not
restate it, because two copies of one fact in one file is what the schema was ratified to remove.
`script/decisions` reads these keys and nothing else.

```yaml
---
status: DECIDED
raised: 2026-09-23
decided: 2026-09-23
ratified_by: calef
---
```

| key | values | required when |
|---|---|---|
| `status` | `PROPOSED`, `DECIDED`, `AMENDED`, `SUPERSEDED` | always |
| `raised` | `YYYY-MM-DD`, UTC | always |
| `decided` | `YYYY-MM-DD`, UTC | `status` is `DECIDED` or `AMENDED` |
| `ratified_by` | a GitHub username listed in `nifeos/nife:ARCHITECTS.md` | `status` is `DECIDED` or `AMENDED` |
| `superseded_by` | a section number | `status` is `SUPERSEDED` |

The schema was ratified 2026-09-23 (calef); a lane does not extend it. Keys are snake_case,
values uppercase, dates UTC like every date in this tree.

This was `**Status: DECIDED.**` in prose until milestone 582 (a decision's status becomes a field,
and the index becomes generated). Two failures came from reading a field out of a sentence. A file
said `**Status: NOT YET`, the regex captured `NOT`, dropped `YET`, and the report printed a word
nobody had written, which is §211 (what a fatal-risk verdict says, and what the chart can plot as a result)'s finding. `design/fatal-risks.md`'s risk 7 carried two status
lines eighteen lines apart, and the first one won, so `AUDITED` was invisible to every consumer for
weeks.

Where the dates came from: most of this corpus predates the schema and states neither date. So
milestone 582 filled them from the file's own prose where it says, and from git where it does not.
`raised` is the first commit that wrote the decision down, followed back through milestone 114's
split into the single `DECISIONS.md` it came from. `decided` is the first commit whose text reads
DECIDED or AMENDED. Both are author dates in UTC. Where the two sources disagree the earlier wins,
since prose written before 2026-09-13 dates by calef's local day.

A git date is the date, not a bound on it: calef, 2026-09-24, *"We write things down when we raise
them. There is no gap between the commit and when it was raised."* The split commit's own date,
2026-08-04, dates nothing here. The block for milestone 582 (a decision's status becomes a field,
and the index becomes generated) has the counts and the three decisions whose prose says otherwise.

`ratified_by` names an architect listed in `nifeos/nife:ARCHITECTS.md`.

| Status | Means |
|---|---|
| `PROPOSED` | Raised, not yet decided. Options and a recommendation are in the file; nothing is built on it, and nothing should cite it as settled. Waiting on an architect. |
| `DECIDED` | It holds as written. |
| `AMENDED` | It holds, but part of it was revised or overtaken by later work. The file states the decision as it now stands, or names the decision that amends it; what changed is in git, per §298 (a document states what is in force now). |
| `SUPERSEDED` | A later decision replaces it, and `superseded_by` names which. Kept, never deleted, because the reasoning is the record: §8 (DEFERRED to a hard decision point)'s deferral was correct and §10 (process model: capability-based, microkernel) is what it deferred to. `script/decisions` lists it as `§M title -> §N`, which is the two keys read together. |

`AMENDED` is the token that pays for the vocabulary. Eleven decisions carry a revision that a reader
of the opening paragraph would otherwise miss, and §26 is the sharpest: its first line still says
"not yet built" while three blocks below it record milestone 22 building it. Nothing flagged that,
because a decision had no status to contradict.

**A decision waiting on an architect is `PROPOSED`, and lives here, not in a queue of its own.**
`design/open-decisions.md` was that queue for one day, and it existed for a good reason: a decision
that lives only in a conversation's scrollback is in the medium milestone 94 was written to abolish.
But a proposal is the same object one lifecycle step before `DECIDED`, with the same shape (what is
being decided, the options, the recommendation and its reason, what is blocked until it is
answered). So it gets a file and a number when it is raised, and answering it flips a status in
place instead of moving text between two systems. Numbers 68 to 73 are the six that queue held.

## Reading

- The seL4 manual, and Klein et al., *seL4: Formal Verification of an OS Kernel* (SOSP'09)
- Liedtke, *On µ-Kernel Construction* (SOSP'95): why Mach was slow and why that was not a law
- The xv6 book (MIT, ~100pp) for how a real Unix-shaped kernel is structured. Read it as the
  road not taken (§10), not as a template.
- `rust-raspberrypi-OS-tutorials` for the aarch64-specific mechanics
- OSDev wiki as a reference, not a tutorial
- *Operating Systems: Three Easy Pieces* for the theory
