---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: a-live-history-search-at-the-prompt
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 657. A live history search at the prompt

*Title and slug are drafts.*
Raised by milestone 47 (navigation and naming), recorded rather than built. The stem is provisional.

Shell-side, in `crates/line_editor` (DECISIONS §227 (how Tab reaches the shell)
option D put the engine in the shell).

## What it is

`^R` starts an incremental search backwards through history, as bash and fish do: each key
narrows it, `^R` again finds the next older match, Enter runs it and `^G` or `^C` gives up. The
engine's history is eight entries today, so this is worth building alongside a longer history.

## Exit criterion

Host tests in `crates/line_editor` for the narrowing, the repeat, accepting and abandoning, with
the screen model the crate's tests already use.

## Index row

The prompt has no incremental history search. Proposed: `^R` in the line editor, with a longer
history behind it.
