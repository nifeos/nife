---
status: BUILT
raised: 2026-11-05
built: 2026-11-05
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1034. A runbook for moving a crate out of nife

*(Number provisional until the integrator mints it. The title and slug are drafts.)*

Every crate on [journey 5](../journeys/05-crates-released-on-their-own.md) leaves this tree the
same way, and the pilot, milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its
own), took the steps by hand. This milestone makes each later move mechanical: a runbook whose
steps each carry a command, an expected result and an owner, and a dry run that builds the crate
repository's first tree without creating anything. The next move, `file_allocation_table` of
milestone 813 (a proven FAT crate, released on its own), drives it.

Reuse: the template `nifeos/crate-workflows` (milestone 887 (crate repositories share one CI
template)) supplies the scaffold and the checks the dry run runs; `helpers/crate_release.py`
supplies the `Name:` block reader and the citation rules. Nothing here is written twice.

## What this builds

- [notes/crate-move.md](../../notes/crate-move.md): the runbook, from the review through the move,
  the nife-side pull request, the soak's five end conditions and their checks, to publication and
  consumption by version, with the architect's steps in one list.
- `script/crate-move --dry-run`: stages the crate repository in an empty directory, runs the
  template's cheap checks from its pinned release, and prints the nife-side follow-up. It never
  creates a repository, pushes or tags.

## Exit

`script/crate-move --dry-run --into <scratch> file_allocation_table` stages the crate and reports
its findings, and the runbook's table records them. `python3 helpers/crate_move.py --selftest`
runs in `script/lint`.

## BUGS

The gaps found in the move machinery are in the runbook's `BUGS` section, where a mover meets them.

## Index row

The runbook for moving a crate out of nife, step by step with owners, and a dry run that stages the
crate repository and checks it before anything is created.
