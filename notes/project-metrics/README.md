# Project metrics: the data behind nifeos/record:notes/project-metrics.md

[`notes/project-metrics.md`](https://github.com/nifeos/record/blob/main/notes/project-metrics.md) is the page to read. `script/metrics` writes
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

`prose-duplication.csv` and its two charts, `prose-duplication-copies.svg` and
`prose-duplication-share.svg`, count how much of the record's prose is stated in more than one
file. For each week, `script/metrics --record <checkout>` takes the last first-parent commit of
nifeos/record's main on or before the week's end, which reaches back to the weeks when `design/`
and `notes/` lived in nife, and reads every Markdown file under `design/` and `notes/` there. The
rule is the record's own one-home lint, `helpers/repeated_sentences.py` in that checkout, loaded
from it so that this script holds no second copy: a sentence of 100 or more characters that two or
more files hold, read a block at a time, with a link read as its text and `*` dropped, and with
block quotes, front matter, table header rows, fenced code, HTML comments and `Name:` paragraphs
left out. The columns are the Markdown files, the duplicated sentences, the copies (the sum of
files over the duplicated sentences) and the duplicated share (the characters in copies beyond the
first, as a percent of all characters of the files, with each run of whitespace one space). The
latest week's sentence and copy counts equal the lint's own `--report` count at the same record
commit. The record's rule applies to every week, so an earlier week reads as the lint would have
read it. The spike in 2026W38 (3197 copies, 2.86 percent) is `design/roadmap/README.md`, a
generated roadmap index of 420,245 bytes that held 1397 sentences also present in other files,
88 percent of that week's duplicated characters. It was retired on 2026-09-22 UTC.
Names provisional (lane/prose-duplication-metric).

*Name: provisional, minted 2026-09-02 with `script/metrics` (`175a2409b`), for the directory and
`ledger.md`; `nifeos/record:notes/project-metrics.md` records the same. Naming is an architect's.*
