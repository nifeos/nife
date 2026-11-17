---
status: PROPOSED
raised: 2026-11-17
milestone_dependencies: 795
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# The Kani bump says the queue proves it

Raised 2026-11-17 (UTC) by lane `audit/2026-11-17-the-verifier`, as finding 2 of the security audit
[the verifier](../../audit-reports/2026-11-17-the-verifier.md).

Reuse: nothing new; three texts change.

## The problem

The `propose-kani` job in `.github/workflows/toolchain-bump.yml` writes, in its comment, its commit
message and its pull request body, that the pull request's own `verify` run is the proof on the new
Kani. Milestone 795's block says the same. A diff that changes only `.kani-version` is not a harness
change to `helpers/harness_change.py`, so the pull request's prove shards skip, as the 2026-11-16
ruling on heavy gates intends. The proof runs in the merge queue, where `script/falsifications
--plan-since` also re-falsifies every replayable record, because the root file reaches everything.

The protection holds. The text tells a reader that a green check on the pull request is the proof,
and it is not.

## The work

- The job's comment, commit message and body say that the merge queue proves the bump and
  re-falsifies every replayable record, and that a red result there is the reason to hold it.
- Milestone 795's block says the same, in its present tense.

## Done when

No text in the tree says a Kani bump's pull request run is its proof, and the next bump pull request
says where it is proved.

## Index row

The Kani bump pull request says the merge queue proves it.
