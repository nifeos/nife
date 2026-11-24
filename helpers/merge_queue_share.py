#!/usr/bin/env python3
"""How much of what enters the merge queue leaves without merging, day by day.

    helpers/merge_queue_share.py update [--since YYYY-MM-DD]   # fetch, upsert the daily CSV
    helpers/merge_queue_share.py report [--days N]             # days over the threshold, for a watcher
    helpers/merge_queue_share.py --selftest

Milestone 724 (the merge queue reports its ejection share and its time to merge), provisional
number. Name provisional.

# WHY

calef asked whether merges were slow before the Claude allowance ran out. They were, and nothing
could have said so. From GitHub's merge-queue events, entries that left without merging went from 8%
(2026-09-15 to 09-19) to 43% (09-22 to 09-27), and a week of that read as an ordinary busy week.
Milestone 630 (a merge-queue ejection is caught before the queue, and recovered after it) handles
each ejection as it happens. Nothing counted them.

# WHAT IT COUNTS

Per UTC day, from each pull request's merge-queue timeline events (GraphQL `AddedToMergeQueueEvent`
and `RemovedFromMergeQueueEvent`, whose `reason` is `merged` on success and `failed_checks`,
`merge_conflict`, `manual` and so on otherwise):

- `entries`: Added events that day.
- `ejected`: Removed events that day whose reason is not `merged`. A pull request ejected three times
  counts three, because each is a group that was built and thrown away.
- `ejection_pct`: ejected over entries, to one place. It can pass 100 on a day whose ejections are
  mostly of the previous day's entries; that is the arithmetic and not an error.
- `merged`, `median_hours_to_merge`: pull requests merged that day, and the median hours from the
  pull request being opened to its merge. Blank when none merged.

An ejection is attributed to the day it happened and an entry to the day it entered, so a pull
request that enters at 23:50 and is ejected at 00:10 moves one number on each side of midnight.

# HOW THE NUMBER REACHES `script/metrics`

`script/metrics` reads git and committed records, and a GitHub API fetch is neither. So the fetch
is this helper's, run daily by the fleet-metrics workflow (milestone 889 (the tree's gates stop
depending on the fleet tooling) moved it there from metrics.yml), and it writes
`notes/project-metrics/merge-queue-daily.csv`. `script/metrics` reads that file the way it
reads `effort.csv`, a committed record of a fact it cannot re-derive, and sums it into weekly
columns. That is the cheapest honest path: no new carried column, no second fetch, and a week the
fetch never ran for is left empty rather than zeroed. The alternative was passing the figure in
with a flag the way `--coverage-from` does, which needs the number to be one value per run and this
is a series.

# THE REPORT

`report` fetches the last few days live (not from the CSV, which only moves when a metrics pull
request merges) and prints a line for each day with at least `MIN_ENTRIES` entries whose share is
over `THRESHOLD_PCT`. `trunk-health` runs it. 20% is calef's number: no day from 09-15 to 09-19
reached it and every day from 09-22 to 09-26 did.

# BUGS

- The fetch pages pull requests newest-updated first and stops at the first one last updated before
  the window, on the premise that a queue event updates the pull request. Checked on 2026-10-03 by
  reproducing the proposal's figures (see the milestone 724 block). A pull request touched by
  nothing since a queue event inside the window would be missed if that premise ever fails.
- A pull request with more than `TIMELINE_CAP` queue events has its newest `TIMELINE_CAP` read, which
  covers any recent window; the helper says so on stderr in the rare case the window reaches the oldest
  event read. Truncation is decided by `hasPreviousPage`, not `totalCount`, which overcounts the
  filtered types (2026-10-03: #1377, #1062, #970 and #546 read 101 to 119 and returned 2, 86, 8 and
  74 nodes, none truncated).
- `report` re-announces every time `trunk-health` runs, for the reason trunk-health.yml's BUGS gives
  for a red trunk: a scheduled run has no memory. A bad day is a warning on every run until the day
  rolls over.
"""

import csv
import datetime
import io
import json
import os
import statistics
import subprocess
import sys

REPO = "partsfoundry/nife"
CSV_PATH = "notes/project-metrics/merge-queue-daily.csv"
COLUMNS = ["day", "entries", "ejected", "ejection_pct", "merged", "median_hours_to_merge"]
THRESHOLD_PCT = 20.0
MIN_ENTRIES = 10
TIMELINE_CAP = 100
PAGE = 50
DEFAULT_WINDOW_DAYS = 8

QUERY = """
query($cursor: String) {
  repository(owner: "partsfoundry", name: "nife") {
    pullRequests(first: %d, after: $cursor, orderBy: {field: UPDATED_AT, direction: DESC}) {
      pageInfo { hasNextPage endCursor }
      nodes {
        number createdAt mergedAt updatedAt
        timelineItems(last: %d, itemTypes: [ADDED_TO_MERGE_QUEUE_EVENT, REMOVED_FROM_MERGE_QUEUE_EVENT]) {
          pageInfo { hasPreviousPage }
          nodes {
            __typename
            ... on AddedToMergeQueueEvent { createdAt }
            ... on RemovedFromMergeQueueEvent { createdAt reason }
          }
        }
      }
    }
  }
}
""" % (PAGE, TIMELINE_CAP)


def parse_time(s):
    return datetime.datetime.fromisoformat(s.replace("Z", "+00:00"))


def day_of(s):
    return parse_time(s).astimezone(datetime.timezone.utc).date().isoformat()


def aggregate(prs, since):
    """`{day: {entries, ejected, merged, hours: [..]}}` for days on or after `since` (an ISO date).

    Pure: `prs` is the GraphQL node list, so the arithmetic is tested without the network."""
    days = {}

    def slot(day):
        return days.setdefault(day, {"entries": 0, "ejected": 0, "merged": 0, "hours": []})

    for pr in prs:
        for ev in pr["timelineItems"]["nodes"]:
            day = day_of(ev["createdAt"])
            if day < since:
                continue
            if ev["__typename"] == "AddedToMergeQueueEvent":
                slot(day)["entries"] += 1
            elif ev.get("reason") != "merged":
                slot(day)["ejected"] += 1
        if pr.get("mergedAt"):
            day = day_of(pr["mergedAt"])
            if day >= since:
                s = slot(day)
                s["merged"] += 1
                s["hours"].append((parse_time(pr["mergedAt"]) - parse_time(pr["createdAt"])).total_seconds() / 3600.0)
    return days


def row_of(day, s):
    entries, ejected = s["entries"], s["ejected"]
    return {
        "day": day,
        "entries": entries,
        "ejected": ejected,
        "ejection_pct": "%.1f" % (100.0 * ejected / entries) if entries else "",
        "merged": s["merged"],
        "median_hours_to_merge": "%.1f" % statistics.median(s["hours"]) if s["hours"] else "",
    }


def merge_rows(existing, fresh):
    """`existing` and `fresh` are `{day: row}`. A fresh day replaces its own entry and a day the
    fetch did not cover is kept, which is what makes a rerun idempotent and a short window safe."""
    out = dict(existing)
    out.update(fresh)
    return [out[d] for d in sorted(out)]


def to_csv(rows):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=COLUMNS, lineterminator="\n")
    w.writeheader()
    for r in rows:
        w.writerow({c: r.get(c, "") for c in COLUMNS})
    return buf.getvalue()


def from_csv(text):
    return {r["day"]: r for r in csv.DictReader(io.StringIO(text))}


def over_threshold(days):
    """`[(day, pct, entries, ejected)]` for days worth speaking of, oldest first."""
    out = []
    for day in sorted(days):
        s = days[day]
        if s["entries"] >= MIN_ENTRIES:
            pct = 100.0 * s["ejected"] / s["entries"]
            if pct > THRESHOLD_PCT:
                out.append((day, pct, s["entries"], s["ejected"]))
    return out


def fetch(since):
    """Pull requests last updated on or after `since`, newest first, or None on an API failure."""
    prs, cursor = [], None
    while True:
        args = ["gh", "api", "graphql", "-f", "query=" + QUERY]
        if cursor:
            args += ["-f", "cursor=" + cursor]
        done = subprocess.run(args, capture_output=True, text=True)
        if done.returncode:
            print("merge_queue_share: GraphQL failed: %s" % done.stderr.strip(), file=sys.stderr)
            return None
        data = json.loads(done.stdout)["data"]["repository"]["pullRequests"]
        stop = False
        for pr in data["nodes"]:
            if day_of(pr["updatedAt"]) < since:
                stop = True
                break
            nodes = pr["timelineItems"]["nodes"]
            # `last:` reads the newest events, which are the ones inside a recent window. It
            # undercounts only when the oldest event it got is itself inside the window, so there
            # may be more in the window than were read.
            # Truncation is read from `hasPreviousPage`, never from `totalCount`: `totalCount`
            # overcounts the filtered item types (#1062 read 101 to 119 and returned 86 nodes).
            if (pr["timelineItems"]["pageInfo"]["hasPreviousPage"] and nodes
                    and day_of(nodes[0]["createdAt"]) >= since):
                print("merge_queue_share: #%d has more than %d queue events and the oldest one read "
                      "is inside the window; its days are undercounted" % (pr["number"], TIMELINE_CAP),
                      file=sys.stderr)
            prs.append(pr)
        if stop or not data["pageInfo"]["hasNextPage"]:
            return prs
        cursor = data["pageInfo"]["endCursor"]


def arg_value(argv, flag, default):
    return argv[argv.index(flag) + 1] if flag in argv else default


def today():
    return datetime.datetime.now(datetime.timezone.utc).date()


def update(argv):
    since = arg_value(argv, "--since", (today() - datetime.timedelta(days=DEFAULT_WINDOW_DAYS)).isoformat())
    prs = fetch(since)
    if prs is None:
        return 2
    days = aggregate(prs, since)
    existing = {}
    if os.path.exists(CSV_PATH):
        with open(CSV_PATH) as f:
            existing = from_csv(f.read())
    fresh = {d: row_of(d, s) for d, s in days.items()}
    with open(CSV_PATH, "w") as f:
        f.write(to_csv(merge_rows(existing, fresh)))
    print("merge_queue_share: %d day(s) from %s written to %s" % (len(fresh), since, CSV_PATH))
    return 0


def report(argv):
    n = int(arg_value(argv, "--days", "2"))
    since = (today() - datetime.timedelta(days=n - 1)).isoformat()
    prs = fetch(since)
    if prs is None:
        return 2
    prefix = "::warning::" if os.environ.get("GITHUB_ACTIONS") else ""
    for day, pct, entries, ejected in over_threshold(aggregate(prs, since)):
        print("%sthe merge queue ejected %.0f%% of its entries on %s (%d of %d), over the %.0f%% line"
              % (prefix, pct, day, ejected, entries, THRESHOLD_PCT))
    return 0


def selftest():
    ok = True

    def expect(label, got, want):
        nonlocal ok
        if got != want:
            ok = False
            print("FAIL %s: got %r want %r" % (label, got, want), file=sys.stderr)

    def pr(created, merged, events):
        nodes = []
        for ts, kind in events:
            nodes.append({"__typename": "AddedToMergeQueueEvent", "createdAt": ts} if kind == "add" else
                         {"__typename": "RemovedFromMergeQueueEvent", "createdAt": ts, "reason": kind})
        return {"number": 1, "createdAt": created, "mergedAt": merged, "updatedAt": merged or created,
                "timelineItems": {"pageInfo": {"hasPreviousPage": False}, "nodes": nodes}}

    # One pull request ejected twice then merged: three entries, two ejections, one merge.
    a = pr("2026-09-22T00:00:00Z", "2026-09-22T05:00:00Z",
           [("2026-09-22T01:00:00Z", "add"), ("2026-09-22T01:30:00Z", "failed_checks"),
            ("2026-09-22T02:00:00Z", "add"), ("2026-09-22T02:30:00Z", "manual"),
            ("2026-09-22T03:00:00Z", "add"), ("2026-09-22T05:00:00Z", "merged")])
    # One that merged cleanly the next day, opened the day before.
    b = pr("2026-09-22T20:00:00Z", "2026-09-23T01:00:00Z",
           [("2026-09-23T00:30:00Z", "add"), ("2026-09-23T01:00:00Z", "merged")])
    d = aggregate([a, b], "2026-09-22")
    expect("22nd entries", d["2026-09-22"]["entries"], 3)
    expect("22nd ejected counts each group", d["2026-09-22"]["ejected"], 2)
    expect("a merge is not an ejection", d["2026-09-23"]["ejected"], 0)
    expect("hours opened to merged", d["2026-09-22"]["hours"], [5.0])
    r = row_of("2026-09-22", d["2026-09-22"])
    expect("share to one place", (r["ejection_pct"], r["median_hours_to_merge"]), ("66.7", "5.0"))
    expect("no entries is a blank share, not a zero", row_of("x", {"entries": 0, "ejected": 0, "merged": 0, "hours": []})["ejection_pct"], "")
    expect("a day before the window is dropped", "2026-09-21" in aggregate([a], "2026-09-23"), False)

    # The median is a median, not a mean.
    s = {"entries": 1, "ejected": 0, "merged": 3, "hours": [1.0, 2.0, 30.0]}
    expect("median hours", row_of("x", s)["median_hours_to_merge"], "2.0")

    # The report speaks only of a day with enough entries and a share past the line.
    days = {"2026-09-21": {"entries": 2, "ejected": 2},
            "2026-09-22": {"entries": 93, "ejected": 81},
            "2026-09-19": {"entries": 40, "ejected": 4},
            "2026-09-23": {"entries": 50, "ejected": 10}}
    expect("report", [x[0] for x in over_threshold(days)], ["2026-09-22"])

    # A rerun replaces its own days and keeps the rest; the CSV round-trips.
    old = from_csv(to_csv([row_of("2026-09-20", {"entries": 5, "ejected": 1, "merged": 0, "hours": []})]))
    new = {"2026-09-22": row_of("2026-09-22", d["2026-09-22"])}
    rows = merge_rows(old, new)
    expect("kept and added", [x["day"] for x in rows], ["2026-09-20", "2026-09-22"])
    expect("idempotent", to_csv(merge_rows(from_csv(to_csv(rows)), new)), to_csv(rows))

    print("merge_queue_share selftest: %s" % ("ok" if ok else "FAILED"))
    return 0 if ok else 1


def main(argv):
    if argv[1:] == ["--selftest"]:
        return selftest()
    if len(argv) >= 2 and argv[1] == "update":
        return update(argv[2:])
    if len(argv) >= 2 and argv[1] == "report":
        return report(argv[2:])
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
