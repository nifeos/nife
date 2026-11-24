#!/usr/bin/env python3
"""Which merged pull requests carry the `debt-paydown` label, for the metrics page.

    helpers/debt_paydown_share.py update        # fetch every merged pull request, rewrite the CSV
    helpers/debt_paydown_share.py --selftest

Name: provisional, minted 2026-10-11 (UTC) by lane maintainer/debt-paydown-share-chart, for this
file, `notes/project-metrics/merged-pull-requests.csv` and its columns. A python helper under
`helpers/`, outside `script/names`' scope, so this paragraph is its provenance.

# WHY

calef asked on 2026-10-11 (UTC) for a chart of the share of pull requests that pay down debt. The
label and its definition are in nifeos/record:notes/debt-paydown.md. A label lives on GitHub and not in git, so
`script/metrics`, which reads git and committed records only, cannot see it. This helper is the
fetch, run daily by the fleet-metrics workflow (milestone 889 (the tree's gates stop depending on
the fleet tooling) moved it there from metrics.yml), and its CSV is the committed record
`script/metrics` sums by week. The same split `helpers/merge_queue_share.py` uses, for the
same reason.

# WHAT IT WRITES

One row per pull request merged into `main`: `number`, `merged_day` (the UTC date GitHub gives as
`mergedAt`) and `debt_paydown` (1 when the label is on it at the time of the fetch, else 0). One row
per pull request rather than a daily sum, so a reader can check any week's count against the pull
requests behind it, and so a relabel is visible in the diff as the one row it changes.

**The whole history is fetched every run and the file rewritten**, unlike the merge queue's
eight-day window. A label can be added to or taken off a pull request merged months ago (the
backfill of 2026-10-10 labeled 641 of them at once, and nifeos/record:notes/debt-paydown.md tells a lane to fix a
wrong one by hand), so a window would leave the record disagreeing with GitHub for good. The fetch
is one paginated listing, about ten seconds for 1,900 pull requests on 2026-10-11.

Only pull requests whose base is `main`. Seventeen merged into another branch by 2026-10-11, a lane
stacked on a lane, and their work reached `main` inside the parent's pull request; counting them
would count that work twice, and `merged_pull_requests` (merge commits on `main`) does not count
them either.

# BUGS

- The label is read as it stands on the day of the fetch, not as it stood when the pull request
  merged. That is the intent (a corrected label should correct history), but it means a past week's
  share can move on any run.
- The fetch refuses to write a file shorter than the one it would replace, since a merged pull
  request cannot unmerge; a truncated listing is the likelier explanation. A pull request whose base
  was retargeted away from `main` after it merged would trip the same refusal. None has.
"""

import csv
import io
import json
import os
import subprocess
import sys

REPO = "nifeos/nife"
CSV_PATH = "notes/project-metrics/merged-pull-requests.csv"
COLUMNS = ["number", "merged_day", "debt_paydown"]
LABEL = "debt-paydown"
# A ceiling on the listing, not an expected size. A result this long is treated as truncated.
LIMIT = 20000


def rows_of(prs):
    """CSV rows, sorted by number, from `gh pr list --json number,mergedAt,labels,baseRefName`.

    Pure, so the filtering and the label test are checked without the network."""
    out = []
    for pr in prs:
        if not pr.get("mergedAt") or pr.get("baseRefName") != "main":
            continue
        out.append({
            "number": int(pr["number"]),
            "merged_day": pr["mergedAt"][:10],
            "debt_paydown": 1 if any(l["name"] == LABEL for l in pr.get("labels") or []) else 0,
        })
    return sorted(out, key=lambda r: r["number"])


def to_csv(rows):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=COLUMNS, lineterminator="\n")
    w.writeheader()
    for r in rows:
        w.writerow({c: r[c] for c in COLUMNS})
    return buf.getvalue()


def row_count(text):
    return sum(1 for _ in csv.DictReader(io.StringIO(text)))


def fetch():
    """Every merged pull request, or None on an API failure or a listing that hit LIMIT."""
    done = subprocess.run(
        ["gh", "pr", "list", "--repo", REPO, "--state", "merged", "--base", "main",
         "--limit", str(LIMIT), "--json", "number,mergedAt,labels,baseRefName"],
        capture_output=True, text=True)
    if done.returncode:
        print("debt_paydown_share: gh pr list failed: %s" % done.stderr.strip(), file=sys.stderr)
        return None
    prs = json.loads(done.stdout)
    if len(prs) >= LIMIT:
        print("debt_paydown_share: the listing reached %d and may be truncated; raise LIMIT" % LIMIT,
              file=sys.stderr)
        return None
    return prs


def update():
    prs = fetch()
    if prs is None:
        return 2
    text = to_csv(rows_of(prs))
    if os.path.exists(CSV_PATH):
        with open(CSV_PATH) as f:
            before = row_count(f.read())
        if row_count(text) < before:
            print("debt_paydown_share: the fetch found %d merged pull requests and %s holds %d; "
                  "a merged pull request cannot unmerge, so the file is left as it was"
                  % (row_count(text), CSV_PATH, before), file=sys.stderr)
            return 2
    with open(CSV_PATH, "w") as f:
        f.write(text)
    rows = rows_of(prs)
    print("debt_paydown_share: %d merged pull requests, %d labeled %s, written to %s"
          % (len(rows), sum(r["debt_paydown"] for r in rows), LABEL, CSV_PATH))
    return 0


def selftest():
    ok = True

    def expect(label, got, want):
        nonlocal ok
        if got != want:
            ok = False
            print("FAIL %s: got %r want %r" % (label, got, want), file=sys.stderr)

    def pr(n, merged, labels=(), base="main"):
        return {"number": n, "mergedAt": merged, "baseRefName": base,
                "labels": [{"name": l} for l in labels]}

    rows = rows_of([
        pr(3, "2026-10-05T23:59:59Z", ["debt-paydown"]),
        pr(1, "2026-10-06T00:00:01Z", ["needs-architect"]),
        pr(2, "2026-10-06T10:00:00Z", ["debt-paydown"], base="lane/stacked"),
        pr(4, None, ["debt-paydown"]),
        pr(5, "2026-10-07T00:00:00Z", ["needs-architect", "debt-paydown"]),
    ])
    expect("sorted by number, main only, merged only", [r["number"] for r in rows], [1, 3, 5])
    expect("the day is mergedAt's UTC date", [r["merged_day"] for r in rows],
           ["2026-10-06", "2026-10-05", "2026-10-07"])
    expect("the label among others is found", [r["debt_paydown"] for r in rows], [0, 1, 1])
    text = to_csv(rows)
    expect("the header", text.split("\n")[0], "number,merged_day,debt_paydown")
    expect("one row per pull request", row_count(text), 3)
    expect("no labels is not labeled", rows_of([pr(9, "2026-10-01T00:00:00Z")])[0]["debt_paydown"], 0)

    print("debt_paydown_share selftest: %s" % ("ok" if ok else "FAILED"))
    return 0 if ok else 1


def main(argv):
    if argv[1:] == ["--selftest"]:
        return selftest()
    if argv[1:] == ["update"]:
        return update()
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
