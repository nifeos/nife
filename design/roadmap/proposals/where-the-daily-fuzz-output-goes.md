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

Raised by lane/continuous-fuzzing-host on 2026-11-01 (UTC). An architect ruled option A on 2026-11-01 (UTC); "What is in force" below states the ruling.

Reuse: `gh release create` and `git push` already do the publishing; the proposal writes no new
transport.

The continuous fuzzing host writes a daily report with counter totals, the clean seed ranges, and the
count and age of held findings (`helpers/fuzz-daily-publish.sh`). The report has to land where others
can read it. The corpus and the findings are public after the fix, which needs a stable address. The
heartbeat needs a newest report with a date. Held findings never appear in it, and a finding starts as
a private security advisory.

Two places fit, and each needs a wider credential than the host holds today.

## What is in force

Ruled 2026-11-01 (UTC): option A. The continuous fuzzing host publishes its daily output as one GitHub
release per day on nifeos/nife, tagged `fuzz-YYYY-MM-DD`, with `report.md` and `clean-ranges.tsv` as
assets. It does not publish a branch of artifacts. The reasons:

1. A release is deletable without touching history.
2. A release adds nothing to any clone.
3. The newest release's date is the heartbeat.
4. Moving from a release to a branch later is compatible; moving from a branch to a release is not.

Option B is rejected for the reason given under it: every clone that fetches all branches pays for
it, and dropping data needs a force-push, which this project forbids. The implementation is in
`helpers/fuzz-daily-publish.sh` and [the host note](../../../notes/fuzzing/continuous-host.md).

## Options

- A. A release per day, tagged `fuzz-YYYY-MM-DD`, with the report and the clean-range table as
  assets. Maintainers can delete an old release without touching history. The heartbeat is the newest
  release's date. Nothing enters the git tree or any clone.
- B. A branch of artifacts, one commit per day on a branch of nifeos/nife that holds the same files.
  Plain `git` reaches the files and shows each day's diff. Every clone that fetches all branches pays
  for it, and dropping data needs a force-push, which this project forbids.

Prior art: OSS-Fuzz publishes coverage reports and corpora to a bucket, and syzbot serves a public web
page. Neither writes to the source repository (from memory, not re-read). Option A is the nearest
analogue here.

## Recommendation

Option A. Both options cost the same to build, so effort plays no part. A release is deletable, which
matters for a place that must never carry a held input by mistake, and it keeps daily data out of every
clone. Moving from a release to a branch later stays compatible; moving back does not.

## What this blocked

The `--publish` path in `helpers/fuzz-daily-publish.sh` refused until the ruling. It now calls
`gh release create` and still refuses unless the host sets the enable variable. The credential needs
Contents: write on a fine-grained token (from memory of GitHub's permission model, not verified),
which also permits a push, so the guarantee that the host never pushes a branch rests on the script.
