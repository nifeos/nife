---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: the-prompt-suggests-from-history
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 679. The prompt suggests the rest of a line from history

*Title and slug are drafts.*
Raised by milestone 47 (navigation and naming), which built DECISIONS §227 (how Tab reaches the
shell) option D and records fish's extras here rather than building them. The stem is provisional.

The shell edits its own line, so this is shell-side work with no wire change.

## What it is

As a line is typed, the newest history entry that starts with it is shown after the cursor in a
dim color, and a right arrow at the end of the line accepts it. This is fish's autosuggestion.
`line_editor::LineDisc` already keeps eight lines of history; the engine would need a way to draw
text after the cursor that is not part of the line.

## Exit criterion

A host test in `crates/line_editor` shows the suggestion drawn after the cursor, erased when the
next key disagrees with it, and accepted by a right arrow at the end of the line.

## Index row

The shell could suggest the rest of a line from history as it is typed, fish's autosuggestion.
Proposed: a suggestion drawn after the cursor and accepted with the right arrow.
