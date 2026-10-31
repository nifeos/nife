---
status: PROPOSED
raised: 2026-10-31
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# Find why the `empty diff` job is the first job to fail on most pull requests

The maintainer session raised this on 2026-10-31 (UTC) after a CI bottleneck measurement over 83
pull requests. The consumer is milestone 965 (gating CI rents more concurrency before it owns a
runner), whose decision depends on where a pull request's time goes; this finding says the time is
not in runner pickup. This milestone investigates one number and builds nothing until the
investigation says what to change.

Reuse: `.github/workflows/empty-diff.yml` and `helpers/empty-diff-check.sh` (milestone 627),
`script/ci-log-baseline` for per-job conclusions, and `gh run list` for the run data. No new
dependency.

## What was measured

Over the 83 pull requests of the measurement:

- The `empty diff` job was the first failing job on 53 of them. Its cause was not verified.
- Median time from ready to merged: 44 minutes. Median of the pre-queue stage: 25 minutes. Median
  of the queue stage: 17 minutes.
- Every runner is GitHub-hosted, and the median time from a job being queued to a runner picking it
  up was 2 to 5 seconds. These figures do not support owning a runner: pickup is not where the time
  goes.

## The open question

Why is `empty diff` the first red on 53 of 83? The check fails a pull request whose tree equals its
base's. A draft opened by `script/claim` carries only an empty `claim:` commit, so the check can
fail on it by design until the first real commit lands. If so, the 53 are mostly the claim and not a
defect, and "first failing job" overstates how often a lane breaks something. If not, a real cause
is waiting to be found. The investigation reads the 53 runs and sorts each into: a claim-only head,
a head with changes whose tree still equals its base (a change and its revert), or something else.

## What a yes unlocks

If most are claim-only heads, the finding is a note and the measurement is restated with those
pull requests set aside; the pre-queue median probably moves. If many are not, the check has a
second cause that needs a fix of its own. Either way, milestone 965 reads a pre-queue median that
means what it says.

## Alternatives

- Do nothing. The check passes once a real commit lands, and the cost is a red mark on drafts.
- Skip the check on drafts. This hides the failure that milestone 627 exists to show, so it needs
  the reason in 627 read first.
