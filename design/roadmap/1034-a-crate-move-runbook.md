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
supplies the `Name:` block reader and the citation rules. This milestone writes neither twice.

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

## The soak's period

Condition 5 of the runbook's soak runs four weeks: 28 days from the tag nife pins, with at least
four scheduled `weekly` runs green, and a major break restarts it at the tag that carries it.
The maintainer decides it, provisionally. Lengthening or shortening it is one pull request, and no
publication follows from it alone, since each still waits for the architect.

The grounds, measured on the pilot and the template:

- Every other condition is quick. The pilot, `jh7110_entropy`, took its tag `v0.1.0` on 2026-10-31
  (UTC) and recorded its stranger run that day. Its first scheduled `weekly` run went green on
  2026-11-02, after five red runs by hand while a lane wired its settings and secrets. Its one later
  tag, `v0.1.1` a day after the first, changed text and no API. So condition 5 sets the soak's length.
- The weekly run is the sampling rate. It runs once a week and its jobs finish in about 70 seconds,
  so the soak costs calendar time and no compute. Fewer than four runs is one or two samples.
- The template's stranger check fails a record older than 28 days. A four-week soak therefore ends
  when the stranger test falls due again, and the run that keeps condition 4 green reads the code
  the soak just held still.
- Fuzz findings set no lower bound. nife's cargo-fuzz shards failed in none of 300 merge-group runs
  from 2026-11-01 to 2026-11-05 (UTC); the bugs on record came from writing a target or a long run
  by hand; and the continuous host has filed no advisory. A longer soak adds five minutes per target per week
  of `weekly / fuzz`, and nife's queue spent about seven hours a week on each target in that window
  and found nothing.
- Longer than four weeks holds each waiting crate's publication for no measured gain, and shortening later
  is compatible while lengthening is not, so four weeks is the stricter of the lengths the data
  supports.

## BUGS

The gaps found in the move machinery are in the runbook's `BUGS` section, where a mover meets them.

## Follow-on

- **Recorded.** The gaps in the move machinery, each in `notes/crate-move.md`'s `BUGS`. Three are
  the template's: its unreleased naming gates, its orphan check and its scaffold's mentions of nife.
  The rest are nife's: the missing `[patch]` lint, the fuzz targets the dry run does not stage, the
  mutation baseline and the unmeasured release dry run.
- **Milestone 1043.** Milestone 1043 (fuzzing a moved crate in its own repository) records it:
  `design/roadmap/1043-a-moved-crate-is-fuzzed-in-its-own-repository.md`.

## Index row

The runbook for moving a crate out of nife, step by step with owners, and a dry run that stages the
crate repository and checks it before anyone creates anything.
