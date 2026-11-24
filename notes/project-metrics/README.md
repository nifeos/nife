# Project metrics: the data behind nifeos/record:notes/project-metrics.md

[`notes/project-metrics.md`](../project-metrics.md) is the page to read. `script/metrics` writes
everything here except [`ledger.md`](ledger.md) and [`limitations.md`](limitations.md), and its
output reproduces byte for byte. A person appends `ledger.md` by hand, because it holds what no
script can derive. `limitations.md` is the page's `BUGS` section, also by hand.

Two files are the exception the other way. `script/effort` writes `effort.csv` and
`commit-models.csv` from session records on one machine, and `script/metrics` only reads them.
Four more records come from GitHub's job and pull request data, each written by a helper:

- `verify-runs.csv`: one row per sampled green verify run, from `helpers/verify_times.py update`
  (name provisional, 2026-10-07). Nothing draws it since 2026-10-11.
- `merged-pull-requests.csv`: one row per pull request merged into `main`, with whether it carries
  the `debt-paydown` label, from `helpers/debt_paydown_share.py update` (name provisional,
  2026-10-11).
- `ci-builds.csv`: one row per green merge-group build and one per job in it, from
  `helpers/ci_job_times.py update` (name provisional, 2026-10-11).
- `ci-job-wait-days.csv`: one row per day with the hosted merge-group jobs that ran, their median
  wait for a runner and how many waited over five minutes, from the same command (name
  provisional).

*Name: provisional, minted 2026-09-02 with `script/metrics` (`175a2409b`), for the directory and
`ledger.md`; `nifeos/record:notes/project-metrics.md` records the same. Naming is an architect's.*
