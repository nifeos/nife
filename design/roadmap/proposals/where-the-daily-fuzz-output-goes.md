---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Where the daily fuzz output goes

Raised by lane/continuous-fuzzing-host on 2026-11-01 (UTC). An architect rules; nothing here is decided.

The continuous fuzzing host writes a daily report: counter totals, the clean seed ranges, and the
count and age of held findings (`helpers/fuzz-daily-publish.sh`). The report has to land where others
can read it. The condition that the corpus and findings are public after the fix needs a stable
address, and the heartbeat needs a newest report with a date. Held findings never appear in it, and a
finding starts as a private security advisory.

Two places fit, and each needs the host's one credential widened from what it holds today.

## Options

- **A. A release per day**, tagged `fuzz-YYYY-MM-DD`, with the report and the clean-range table as
  assets. Old releases can be deleted without touching history. The heartbeat is the newest release's
  date. Nothing enters the git tree or any clone.
- **B. A branch of artifacts**, one commit per day on a branch of nifeos/nife that holds the same
  files. History is a diff view for free and the files are reachable by plain `git`. Every clone that
  fetches all branches pays for it, and a rewrite to drop data is a force-push, which this project
  forbids.

Prior art: OSS-Fuzz publishes coverage reports and corpora to a bucket, and syzbot serves a web page
with a public dashboard; neither writes to the source repository (from memory, not re-read). The
nearest analogue here is option A.

## Recommendation

Option A. Both options cost the same to build, so the recommendation is not about effort. A release
is deletable, which matters for a place that must never carry a held input by mistake, and it keeps
daily data out of every clone. Choosing the stricter, deletable place now stays compatible with
moving to a branch later; the reverse is not.

## What this blocks

The `--publish` path in `helpers/fuzz-daily-publish.sh` is a stub that refuses until this is ruled.
The credential's scope (releases or contents) follows from the answer.
