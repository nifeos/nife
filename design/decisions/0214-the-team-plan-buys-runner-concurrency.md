---
status: DECIDED
raised: 2026-09-24
decided: 2026-09-24
ratified_by: calef
---

# 214. The Team plan buys runner concurrency, and merge throughput is no longer third

calef upgraded the `crickertech` organization from GitHub Free to GitHub Team on 2026-09-24 (UTC). This section amends the ranking in
[§203 (capacity is rented rather than bought)](0203-capacity-is-rented-not-bought.md). It does not
touch that section's ruling.

## What changed

§203 ranked what rented runners buy, and put merge throughput third. Its reason was that GitHub's
concurrency "binds on heavy branches rather than on a typical day". That held while lanes gated on
this laptop. It stopped holding once lanes gated in CI, per
`nifeos/nife:briefs/gate-in-ci.md`, written 2026-09-22. Every lane now spends
runner jobs, so the runner pool is the shared ceiling.

## The evidence, measured 2026-09-24

All of these were re-read from GitHub at 18:50 UTC on 2026-09-24.

- `gh api orgs/nifeos -q .plan` reads `team`, with 1 seat and 1 filled.
- The last pull request to merge before that time was #1202, at 16:51:21 UTC. Nothing merged for
  the following two hours.
- At 18:50 UTC, 14 workflow runs were queued and 7 in progress. A higher peak earlier in the day
  was reported by the maintainer but not re-measured here.
- #1213's issue timeline: added to the merge queue at 16:53:43, removed at 17:54:14, re-added at
  17:55:01.

The #1213 removal is worth reading closely, because the obvious account of it is wrong. Its first
merge-group build did start: `CI` took its first job at 16:58:42 and passed at 17:45:21. The
`verify` workflow took its first job at 17:00:43 and passed at 17:58:55. That is four minutes after
the queue gave up at the 60-minute check timeout. So the first eviction was a slow prover against a
short timeout.

The re-entry shows the concurrency cost. Its `CI` run was created at 17:55:20 and took its first job
at 18:11:08. That is sixteen minutes of waiting for a runner before any work began.

## What the plan buys, and what it does not

GitHub's [limits reference](https://docs.github.com/en/actions/reference/limits) gives standard
hosted runners 20 concurrent jobs on Free and 60 on Team. That tripling is the whole purchase.

Minutes were already free. The September usage summary
(`gh api /organizations/nifeos/settings/billing/usage/summary`) shows 147,712 Linux arm64
minutes, a gross $738.56, all discounted to $0 because the repository is public. The other SKUs
(Linux x64, Windows, storage) are also discounted to $0.

Every metered product is capped at zero. `gh api /organizations/nifeos/settings/billing/budgets`
shows $0 budgets for Actions, Packages, Codespaces and Git LFS, each set to stop usage at the cap.
So the seat is the only cash this adds.

The seat is $4 per user per month for the first 12 months, per
[github.com/pricing](https://github.com/pricing). That page does not state the price after the first
12 months. This section does not guess it. With one seat, the plan is $4.00 a month, recorded in
`nifeos/nife:notes/project-metrics/ledger.md`.

## The amended ranking

Merge throughput moves from third to first among what rented capacity buys. It now binds on an
ordinary day, and it is the one this project could buy the same afternoon. Lane concurrency is
second, because gating in CI already moved that load off the laptop and onto the same runner pool.
Hardware-gated milestones stay where §203 put them.

## What stands

§203's ruling stands: rent, do not buy. A plan tier is renting. Its condition on runners stands too.
A self-hosted runner is acceptable only when restricted to this repository's own branches, as
milestone 488 (a self-hosted CI runner) requires.

## Demand was cut the same day

Buying concurrency was not the only response. Three cuts went in on 2026-09-24:

- The merge queue's `check_response_timeout_minutes` went from 60 to 240. That is a ruleset setting
  with no pull request. The ruleset reads 240 at the time of writing.
- #1213 changes `always()` to `!cancelled()`, so a cancelled run stops instead of running its whole
  suite.
- #1220, called A′, makes a push to `main` skip suites that a merge group already tested.

Both pull requests were open at the time of writing.

## When to look again

Measure queue latency for a week on Team, once #1213 and #1220 have landed. Reconsider a
self-hosted runner only if 60 concurrent jobs still binds. That runner would still be under §203's
restriction.

## BUGS

- Nothing here measures job wait time continuously. The sixteen-minute wait is one run, read by
  hand. The week of measurement above is what turns it into a number.
- The seat price after the first year is unknown, so the ledger row's rate may change without notice.
