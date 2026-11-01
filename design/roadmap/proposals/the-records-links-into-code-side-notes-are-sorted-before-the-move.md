---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: 315
machine_requirements: none
specific_machine: none
needs_person: no
---
# The record's links into notes that stay with code are sorted before the move

Raised 2026-11-01 (UTC) by lane `lane/the-record-takes-the-whole-project-material`, while recording
the ruling that the record repository takes the whole-project material and each code repository
keeps its crate documentation, `BUGS` sections and notes about that crate alone. Every name below is
provisional.

Reuse: `script/citations` and the markdown link check already walk every link in the record. The
sort is a filter over their output, not a new parser.

## The failure

The ruling leaves some notes with code, and the record links to notes by relative path. After the
move, a link from a roadmap block, a decision or a journey to a note that stayed with a code
repository points at nothing in the record repository, and the site build fails on an internal link.
A rough count on 2026-11-01 (UTC), by name match and not by reading each note:

- 296 links from `design/` and `briefs/` reach 168 distinct files under `notes/`. Five of those
  files are named for a crate (`abi`, `address-space-map`, `e1000e`, `job-mix`, `nifefs`), so they
  are probably about that crate alone. The rest were not read.
- Two links reach a crate directory: `design/decisions/0148-reap-and-thread-identity.md` and
  `design/roadmap/0060-isa-discovery.md`. A decision is edited only through
  `script/record-ruling`, so the first needs a ruling or a bookkeeping exception.
- Roadmap blocks cite a crate's `BUGS` section by bare path, such as milestone 74 and milestone 230.
  The anchor in the main proposal keeps those true as written, so they need no change.
- Journey 3 cites `notes/bench-runbook.md` in its table. Whether that note is workflow data that
  stays or a cross-cutting note that moves is not decided.

## The work

1. List every link from the record to a path that stays, using the sort step 4 of the main proposal
   produces.
2. For each, choose one: retarget it to the qualified form `nifeos/<repo>:<path>`, move the note
   because it is cross-cutting after all, or drop the link and keep the reason in the sentence.
3. Make the record repository's link check name any link that reaches a path outside it.

## Done when

The site build in the record repository passes its internal link check with no link into a path
that stayed behind.

## Index row

Every link from the record resolves after the move, whichever repository holds the target.
