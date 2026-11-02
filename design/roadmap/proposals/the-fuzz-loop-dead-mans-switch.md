---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The fuzz loop's dead-man's switch

Lane/fuzz-dead-mans-switch raised this on 2026-11-01 (UTC). No milestone in `design/roadmap/` covers
monitoring the continuous fuzz loop; this proposal names one.

Reuse: the daily release from `helpers/fuzz-daily-publish.sh` is the signal, `gh` reads it, and the
scheduled-workflow arrangement of `toolchain-drift.yml` carries it.

## What is in force

Ruled 2026-11-01 (UTC): a scheduled GitHub workflow on a GitHub-hosted runner, never a self-hosted
one, detects a broken fuzz loop and notifies by opening an issue with a fixed label. It repairs
nothing; a person turns the issue into an action for now.

The maintainer decides these, provisionally; reversing costs one pull request:

- The workflow looks at the releases of the last two UTC days, so the workflow
  tolerates one late or skipped run.
- It fails when no release exists for either day, when the newest one's `report.md` shows no `ok`
  window, or when the share of `infrastructure` windows is 0.5 or more.
- The label is `fuzz-loop-broken`. One issue at a time: none opens while one with the label is open.
- It passes quietly while no `fuzz-*` release has ever existed, and switches on at the first one.
- It holds `contents: read` and `issues: write`, takes no secret, runs on `schedule` and
  `workflow_dispatch` only, and fetches only the release list and `report.md`.

The implementation is `helpers/fuzz-heartbeat.py` and `.github/workflows/fuzz-heartbeat.yml`, and the selftest
that `script/lint` runs proves it.

## Not done here

Nothing repairs the loop, and nothing watches the watcher: if the schedule itself stops, GitHub
disables scheduled workflows after sixty days without repository activity, which this repository does
not reach.
