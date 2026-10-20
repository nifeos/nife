---
status: BUILT
raised: 2026-10-03
built: 2026-10-04
promoted_from: the-pre-push-hook-runs-what-fits-in-seconds
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 729. The pre-push hook runs what fits in seconds

Raised 2026-10-03 (UTC). The number is provisional until the merge queue lands it; the title and
slug are drafts, and every new name below is provisional.

## Why

Milestone 630 (a merge-queue ejection is caught before the queue, and recovered after it) made the
pre-push hook all of `script/lint`. On 2026-10-03 that cost 70 to 120 s warm and 202 s cold on a
loaded machine, a claim push sat in it for a whole lane, and it refused five claim pushes.
The roadmap block they lacked cannot exist yet. Lanes took to `--no-verify`. calef approved the narrower
decision on #1564 (2026-10-03 UTC): "Approve the revised decision 3."

## Built

1. `script/lint --no-cargo` runs every lint section that does not invoke cargo. The partition is
   read out of `script/lint` by `helpers/lint-no-cargo.awk`, so a new check lands in one bucket
   when it is written. 46 of 65 sections run. A section that opts in with a marker, as counted
   claims does for its harness-count cross-check, must also guard on `$LINT_NO_CARGO`.
   `helpers/lint-no-cargo-selftest.sh` proves the rule and runs inside lint.
2. `.githooks/pre-push` runs `script/fmt --check`, then `script/lint --no-cargo`, then
   `--ready-branch`, and skips all three for a push that changes no files, so a claim push is no
   longer refused for a missing roadmap block. `--no-verify` stays documented as legitimate.
3. Measured on patagonia at a load average of 15 to 17: `script/lint --no-cargo` took 41.6, 47.2
   and 44.7 s on a warm tree, and fmt plus lint 43.0 s with no `target/` directory. The proposal's
   estimate of about 33 s was low. The numbers are in `notes/queue-ejection.md`.

## BUGS

- Clippy and the checks that call `cargo metadata` wait for CI. The `cargo metadata` ones are fast
  and could run here; the rule is "does not invoke cargo" because that is what calef approved.
- The hook no longer fits in seconds. On 2026-10-20 (UTC) on patagonia, `script/lint --no-cargo`
  took 84 to 100 s at a load average of 3 to 10 and 176 s at 29, and a fleet clone's
  `script/lint-fleet` adds 45 to 50 s. No section dominates: the slowest took 11 s. GitHub drops an
  idle push connection after 300 to 480 s, so `script/push` keeps it alive and confirms the push.

## Follow-on

- **Recorded.** Clippy and the `cargo metadata` checks waiting for CI is the BUGS entry above, beside
  the feature in `notes/scripts.md`.

## Index row

The pre-push hook runs `script/fmt --check`, then every lint check that does not invoke cargo
(`script/lint --no-cargo`), then the ready-status check, and skips all of it for a push that changes
no files.
