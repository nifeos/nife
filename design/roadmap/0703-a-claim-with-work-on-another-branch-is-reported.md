---
status: NOT-STARTED
raised: 2026-10-02
promoted_from: a-claim-with-work-on-another-branch-is-reported
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 703. A claim with its work on another branch is reported

*Title and slug are drafts.*
Raised 2026-10-02 by the lane that built the empty-diff check (milestone 627 (a pull request that
changes nothing does not merge), provisional number).
Pull request #1460 claimed under `lane/fatal-risk-colors` while the work was pushed to
`milestone/fatal-risk-colors`, a branch that descends from the claim commit. The draft never moved.

`helpers/lane-claim-check.sh` reports pushed branches with no pull request. It misses this shape,
because the work branch descends from a claim whose pull request exists. The addition: report a
pushed branch that contains a `claim:` commit belonging to a pull request whose head is a different
ref and whose head has not moved past that commit. It reports and never acts, like the rest of that
script. The empty-diff check catches the consequence at merge; this catches the cause while the
lane is still working.

No hardware, no other milestone, no decision owed.

## Index row

`helpers/lane-claim-check.sh` misses a claim whose work was pushed to a different branch descending from the claim commit, as in #1460. Proposed: report a pushed branch that contains a claim commit whose pull request head never moved past it.
