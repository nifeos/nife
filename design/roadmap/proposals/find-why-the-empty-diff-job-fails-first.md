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
runner), whose decision depends on where a pull request spends its time. This milestone
investigates one number and builds nothing until the investigation says what to change.

Reuse: `.github/workflows/empty-diff.yml` and `helpers/empty-diff-check.sh` from milestone 627 (a
pull request that changes nothing does not merge), `script/ci-log-baseline` for per-job
conclusions, and `gh run list` for the run data. No new dependency.

## What the measurement found

Over the 83 pull requests:

- The `empty diff` job failed first on 53 of them. Nobody verified the cause.
- The median time from ready to merged was 44 minutes: 25 minutes before the queue and 17 inside it.
- Every runner is GitHub-hosted, and a job waited a median of 2 to 5 seconds for a runner. The
  numbers do not support owning a runner, because pickup is not where the time goes.

## The open question

Why does `empty diff` fail first on 53 of 83? The check fails a pull request whose tree equals its
base's. A draft that `script/claim` opens carries only an empty `claim:` commit, so the check can
fail on it by design until the first real commit lands. If so, the 53 are mostly the claim and not
a defect, and "first failing job" overstates how often a lane breaks something. If not, a real cause
remains to find.

The investigation reads the 53 runs and sorts each into one of three kinds: a head with only claim
commits, a head with changes whose tree still equals its base (a change and its revert), or
something else.

## What a yes unlocks

If most are claim-only heads, the finding is a note, and the measurement restates itself with those
pull requests set aside. The pre-queue median probably moves. If many are not, the check has a
second cause that needs its own fix. Either way, milestone 965 reads a pre-queue median that means
what it says.

## Alternatives

- Do nothing. The check passes once a real commit lands, and the cost is a red mark on drafts.
- Skip the check on drafts. That hides the failure milestone 627 exists to show, so read 627's
  reasons first.
