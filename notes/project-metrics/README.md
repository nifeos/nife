# Project metrics: the data behind notes/project-metrics.md

[`notes/project-metrics.md`](../project-metrics.md) is the page to read. Everything here except
[`ledger.md`](ledger.md) and [`limitations.md`](limitations.md) is written by `script/metrics` and reproduces byte for byte; `ledger.md` is appended by
hand and holds what no script can derive. `limitations.md` is the page's `BUGS` section, written by hand. Two files are the exception the other way: `effort.csv` and `commit-models.csv` are written by `script/effort` from session records on one machine, and `script/metrics` only reads them.
`verify-runs.csv` is the same kind: one row per sampled green verify run, written by
`helpers/verify_times.py update` from GitHub's job records (name provisional, 2026-10-07), and
undrawn since 2026-10-11. So is
`merged-pull-requests.csv`: one row per pull request merged into `main`, with whether it carries the
`debt-paydown` label, written by `helpers/debt_paydown_share.py update` (name provisional,
2026-10-11). So is `ci-builds.csv`: one row per green merge-group build and one per job in it,
written by `helpers/ci_job_times.py update` (name provisional, 2026-10-11).

*Name: provisional, minted 2026-09-02 with `script/metrics` (`175a2409b`), for the directory and
`ledger.md`; `notes/project-metrics.md` records the same. Naming is an architect's.*
