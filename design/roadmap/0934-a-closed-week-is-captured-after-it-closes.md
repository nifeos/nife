---
status: NOT-STARTED
promoted_from: a-closed-week-is-captured-after-it-closes
raised: 2026-10-19
milestone_dependencies: 519
decision_dependencies: none
machine_requirements: the host whose session records `script/effort` reads
specific_machine: none
needs_person: yes
---
# 934. A closed week's effort row is captured after the week closes

Raised by the lane `maintainer/effort-w42-capture`, while repairing the 2026W42 row of
`notes/project-metrics/effort.csv`. Name: provisional.

Reuse: the existing `script/effort --update`, `script/metrics --backfill-measure`, `script/claim`
and the snapshot job already on the host. Nothing new is written except a wrapper.

Held: calef ruled on 2026-10-19 (UTC) that no job is installed while development moves to another
host ("no for now"). Nobody builds this until development has settled on the new host.

## The failure

`effort.csv` gains a row only when a person runs `script/effort --update` on the host that ran the
lanes and commits the result. Nothing prompts that run at the right time.

The case that raised this: W42 was captured on 2026-10-13, two days into the week, and not again
until the week was over. The committed row held 18,663 turns against 35,118 for the full week. The
mean context per turn read 197,648 tokens against 215,892, and the peak 643,048 against 966,683.
Two days of a week read as a quiet week with no session near the context wall, and neither was
true.

`script/effort --check` reports this case. Each row carries a `captured_at` stamp, and the
check fails when the newest closed week was not read after it closed; `script/effort`'s header has
the rule. So a missed capture is reported loudly through `helpers/trunk-health.sh`. What is left
is that a person still has to act on the report.

## Options weighed

### A `script/metrics --check` failure when the newest closed week has no effort row

Refused. It reads the same committed file as `script/effort --check`, which already asks the
stronger question. `script/metrics --check` also has no caller, by its own header, because every
merge moves the current week. Adding a condition to a check nothing runs is rung four.

### A scheduled local job that opens a pull request each Monday

Recommended. Only the host with the session records can produce the number, so the only mechanism
that removes the person is one that runs there. It fires without being remembered, which is rung
two's property, though it is not a gate.

## The proposal

A launchd agent beside the existing six-hourly snapshot job, firing early each Monday UTC. launchd
runs a missed calendar job on wake, so a sleeping laptop delays the run rather than skipping it.
On another host the scheduler differs and the steps do not. The job:

1. Cuts a throwaway worktree from `origin/main` with
   `script/claim maintainer/effort-<week> --debt-paydown`, so it never writes into a checkout a
   lane or a person is using.
2. Runs `script/effort --update`, then
   `script/metrics --backfill-measure context-per-turn` and
   `script/metrics --backfill-measure cost`. Those are the two measures that read `effort.csv`.
   `--update` alone recomputes only the current week, so a closed week's row needs the backfill.
3. Runs `script/effort --check`, and stops with its message if the closed week is still not whole.
4. Commits only `effort.csv`, the two CSVs and `context-per-turn.svg`, pushes, and marks the draft
   ready. The commit body states the closed week's turns, mean and peak.
5. Exits nonzero, with the reason in its log, on any step that fails.

It does not merge. The pull request goes through the queue like any other, and whoever arms
ruled, green, independent pull requests arms this one.

The job needs `gh` credentials and a Python with `tomllib` on its `PATH`, as the snapshot job
already has. Installing it is a change to the host, so calef approves it. Its shape and the
reasoning live in `notes/project-metrics.md` beside the snapshot job's; unit-specific details are
kept off-tree by the maintainers.

## What this does not fix

The job runs on one host. A week the host is away for longer than the transcripts survive is still
lost, and the six-hourly snapshot remains what makes that a delay rather than a loss.

## Index row

A closed week's row in `notes/project-metrics/effort.csv` is right only if someone runs `script/effort --update` after the week ends. A Monday job on the host with the session records would capture it and open the pull request, so no person has to remember.
