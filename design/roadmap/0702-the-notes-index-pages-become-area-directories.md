---
status: NOT-STARTED
raised: 2026-09-30
promoted_from: the-notes-index-pages-become-area-directories
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 702. The notes index pages become area directories

*Title and slug are drafts.*
## What is being decided

The shape of `notes/` going forward. Today it is flat, 233 top-level files, indexed by
`notes/README.md` plus 14 area pages under `notes/README/`. That directory exists because the index
outgrew the 3,000-word prose cap, so its lines sit one level down rather than the notes moving. calef's diagnosis on 2026-09-30: a directory named `README/` is an index split
masquerading as structure, and the collection, not the pointer, should have the home.

## The options

- A. Full migration. Every note moves into an area directory (`notes/kernel/`,
  `notes/memory/`, ...) with a `README.md` beside it, the shape `design/fatal-risks/` and
  `notes/net/` already ratified. `notes/README/` is deleted and `script/lint`'s index check reads
  area READMEs instead.
- B. New collections get directories; legacy stays flat. Adopt "a collection gets a directory
  with its README beside it" as the rule for anything new (`notes/coes/` is the first). The flat
  notes and the `notes/README/` pages remain as the legacy shape, migrated area by area only when
  an area is next swept anyway.
- C. Keep the `README/` convention. Extend it with new area pages as notes accumulate.

## The recommendation

Option B, on these grounds:

- The measured cost of A is 233 file moves and 69 files outside `notes/` (in `kernel/`, `crates/`,
  `design/`, `briefs/`) whose links rewrite, plus the lint check's rewrite, plus the citation
  ratchet's path pairs, in one sweep. All mechanical, none of it buys anything B does not get from
  the same endpoint reached gradually.
- A also forces single homes. The flat shape lets a note sit in one place while two area pages
  mention it; subdirectories make that a lie one level down. `notes/net/prior-art-and-the-contract.md`
  is linked from build guidance and from the net page, and it is the better for it.
- C is refused because it keeps growing the shape calef named as wrong, and every new area page
  deepens the eventual migration.

## What it costs either way

B leaves two shapes living side by side, which reads as inconsistency until the migration
finishes. That is marked here rather than hidden: the flat shape is the legacy, the directory
with its README is the direction, and the marker is this block until the last area moves.

## What is blocked

Nothing hard. `notes/coes/` is created under B's rule regardless of the answer, on calef's
2026-09-30 commission. If A is ruled instead, `notes/coes/` is already in its final shape.

## Prior art

`design/fatal-risks/` (consolidated 2026-09-30, calef's ruling) and `notes/net/` (split from
`notes/net.md` by a lane, kept since). Both put the README beside the collection it summarizes.

## Index row

`notes/` is flat, 233 top-level files, behind an index split into a `README/` directory. Proposed: choose between a full migration into area directories and directories for new collections only.
