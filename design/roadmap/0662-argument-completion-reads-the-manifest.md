---
status: NOT-STARTED
raised: 2026-09-26
promoted_from: argument-completion-reads-the-manifest
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 662. Tab completes an argument from the program's manifest

*Title and slug are drafts.*
Raised by milestone 47 (navigation and naming), which built name completion (DECISIONS §227 (how
Tab reaches the shell) option D) and recorded this rather than building it. The stem is provisional.

Shell-side, over the line the shell now edits itself, with no wire change.

## What it is

Tab after a program name offers every entry of the directory today (`swish::complete`'s BUGS). A
program's manifest (`grant_plan::Manifest`) already says what it takes: a file to read, a file to
write, a directory, a memory range, options. Completion could offer only what fits. `rm <Tab>` would
offer names, `rm -<Tab>` the options its manifest declares, and a builtin such as `cd` directories only. It is
the same property as the rest of this shell: what is offered is what the line could grant.

## Exit criterion

Host tests in `crates/swish` showing a directory-only position offers no files, and an option
position offers the manifest's options and nothing else.

## Index row

Argument completion offers every name in the directory regardless of the program. Proposed:
complete an argument from what the program's manifest says it takes.
