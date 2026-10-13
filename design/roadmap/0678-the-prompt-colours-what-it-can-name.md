---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: the-prompt-colours-what-it-can-name
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 678. The prompt colors a command by whether the shell can run it

*Title and slug are drafts.*
Raised by milestone 47 (navigation and naming), recorded rather than built. The stem is provisional.

Shell-side, over the line the shell now edits itself (DECISIONS §227 (how Tab
reaches the shell) option D).

## What it is

fish colors the first word red until it names something runnable. Here the question has a
sharper answer than on Unix: the word either names a builtin or a program this shell may spawn, or
it does not, and `grant_plan::parse` can say which without spawning anything. A red word would be
a refusal shown before Enter.

The engine repaints the input region already. Coloring needs it to carry attributes per byte, or
to take a repaint hook from the shell.

## Exit criterion

A host test shows `wc` drawn in one color, `wcx` in another, and the color changing as the word
is typed, with the line's bytes unchanged.

## Index row

The prompt could color the first word by whether this shell can run it. Proposed: a repaint that
carries per-byte attributes, fed by `grant_plan::parse`.
