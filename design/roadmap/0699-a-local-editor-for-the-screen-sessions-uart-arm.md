---
status: NOT-STARTED
raised: 2026-09-30
promoted_from: a-local-editor-for-the-screen-sessions-uart-arm
milestone_dependencies: 632
decision_dependencies: 227
machine_requirements: none
specific_machine: none
needs_person: no
---
# 699. A local editor for the graphical terminal session's UART arm

*Title and slug are drafts.*
Raised 2026-09-30 by the milestone 632 (graphics on demand: `graphical_terminal`, launched from the
swish prompt) lane (graphics on demand). The `graphical_terminal` program has a UART arm. It reads
the boot's line discipline raw with `OPERATION_READRAW`, per DECISIONS §227 (how Tab reaches the
shell: the shell edits its own line)'s shape, and paints its own echo. That arm has no line
editing: backspace and the arrows are stored and echoed as the control bytes they are, so a
mistyped `quit` cannot be corrected. The keyboard arm gets full editing for free from its line
discipline; only the arm every real board uses goes without.

The work: the sans-IO `line_editor` engine the shell itself runs at its prompt (§227 option D),
fed from the same `OPERATION_READRAW` reads inside `graphical_terminal`'s UART arm. It echoes
through the session's own `Echo`. The engine already exists and is host-tested; this is wiring it
into a second program.

## Index row

`graphical_terminal`'s UART arm has no line editing, so a mistyped command cannot be corrected on the arm every real board uses. Proposed: feed the shell's `line_editor` engine from the same raw reads inside it.
