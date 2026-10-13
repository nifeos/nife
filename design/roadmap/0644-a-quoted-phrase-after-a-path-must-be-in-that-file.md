---
status: NOT-STARTED
raised: 2026-09-24
promoted_from: a-quoted-phrase-after-a-path-must-be-in-that-file
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 644. A phrase quoted after a path must still be in that file

*Title and slug are drafts.*
Raised by the 2026-09-24 documentation audit
(`design/audit-reports/2026-09-24-split-documents-read-from-the-inbound-side.md`), whose lens was
the day's document splits read from the side of the files that cite them. **Name provisional**, this
file's and nothing else's.

No hardware, no decision, no dependency. It extends `script/citations`, which
already checks the same thing for two other shapes.

## The claim

The tree cites a section of a document in one common shape: a path, then the section's heading or a
phrase from it, in quotation marks. `notes/interrupts.md, "Testing it with no device on RISC-V"` is
one. Nothing checks that the quoted phrase is still in the file the path names.

`script/citations` already checks two neighbors of this shape. An attributed block quote
(`> -- path`) must still exist in the file it names, and a gloss after `milestone N` or `§N` must
match the target. The path-then-quote form is the third, and it is the one a document split breaks,
because a split moves the section and leaves the path pointing at the main page.

## The measurement

On 2026-09-24, at the audit's base `edfb4cd6b`, the tree held 121 citations in this shape whose path
resolves to a file. 14 quoted a phrase the file does not contain:

- 9 were rot, and the audit fixed them. Five were headings that moved into an appendix in that
  day's splits, and four were headings in code comments that the note had since renamed or never
  had.
- 5 were not rot: code syntax the pattern caught (`", title: "`), a relative path the scan resolved
  to the wrong README, and three quotes that record an old wording on purpose.

A further 37 citations named a section by date or by a bare noun ("the 2026-09-19 section", "the
multi-hart section"). No script can check that shape without false positives, so it stays with the
documentation sweep.

## What it would be

A `--ratchet` rule in `script/citations`: a line a branch adds that carries `path`, then a quoted
phrase of six characters or more, fails if the phrase is not in that file after whitespace and
markup are normalized. It covers added lines only, like the gloss ratchet, so the five deliberate
exceptions above never fire. An intentional old quotation is marked the way the attributed-quote
form already allows.

## What would make it not worth doing

If a lane's added lines produce more false positives than the 5 in 14 measured over the whole
tree, the rule costs more attention than it saves. The first week's failures are the measurement.

## Index row

`script/citations` checks two citation shapes but not a path followed by a quoted section phrase. Proposed: extend it so a phrase quoted after a path must still be in that file.
