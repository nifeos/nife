#!/usr/bin/env python3
"""helpers/fuzz-heartbeat.py: the dead-man's switch of the continuous fuzz loop.

    helpers/fuzz-heartbeat.py [--today YYYY-MM-DD]     read GitHub releases, open an issue on failure
    helpers/fuzz-heartbeat.py --selftest               run against fixtures, no network

Run daily by .github/workflows/fuzz-heartbeat.yml on a GitHub-hosted runner. The continuous fuzzing
host publishes one release per day tagged `fuzz-YYYY-MM-DD` with `report.md` and `clean-ranges.tsv`
(helpers/fuzz-daily-publish.sh). This script looks at the releases of the last two UTC days (today and
yesterday, so one late or skipped run is tolerated) and checks:

  1. a release exists for at least one of the two days;
  2. the newest of them has a `report.md` that shows at least one `ok` window;
  3. the share of `infrastructure` windows in that report is below INFRASTRUCTURE_SHARE_BOUND.

On a failure it opens one issue carrying ISSUE_LABEL that names the check and the day, unless an issue
with that label is already open. On success it does nothing. It repairs nothing: a person turns the
issue into an action (notes/fuzzing/continuous-host.md).

**Quiet before the first release.** The host publishes only after a person enables it. While no
`fuzz-*` release has ever existed, this passes with a log line and opens nothing. The first release
switches the checks on by itself.

**What it fetches.** The release list (tag names) and `report.md` of one release. Nothing else: no
`clean-ranges.tsv`, no other asset, nothing from the repository. The report holds counters only, and
this script prints check names and numbers, never report text.

Decided by the maintainer, provisional; reversing costs one pull request: the bound, the label and
the two-day window below.

Name: provisional, minted 2026-11-01 (UTC) by lane/fuzz-dead-mans-switch.
"""

import datetime
import json
import re
import subprocess
import sys

REPO = "nifeos/nife"
# Failing share: infrastructure windows divided by all windows. The check passes below the bound.
INFRASTRUCTURE_SHARE_BOUND = 0.5
ISSUE_LABEL = "fuzz-loop-broken"
WINDOW_DAYS = 2
TAG_RE = re.compile(r"^fuzz-(\d{4}-\d{2}-\d{2})$")


def parse_status_counts(report):
    """Counts by window status from the fenced block under "Windows by status:". None if absent."""
    lines = report.splitlines()
    try:
        start = next(i for i, l in enumerate(lines) if l.strip() == "Windows by status:")
    except StopIteration:
        return None
    counts, inside = {}, False
    for line in lines[start + 1:]:
        if line.startswith("```"):
            if inside:
                return counts
            inside = True
        elif inside:
            m = re.fullmatch(r"(\S+) (\d+)", line.strip())
            if m:
                counts[m.group(1)] = int(m.group(2))
    return None


def evaluate(today, tags, fetch_report):
    """Return (kind, day, message). kind is "pass", "quiet" or "fail"."""
    dated = {m.group(1) for t in tags if (m := TAG_RE.match(t))}
    if not dated:
        return ("quiet", today, "no fuzz-* release has ever existed; the publish is not on yet, "
                "so nothing is checked until the first release")
    t = datetime.date.fromisoformat(today)
    days = [(t - datetime.timedelta(days=i)).isoformat() for i in range(WINDOW_DAYS)]
    present = [d for d in days if d in dated]
    if not present:
        return ("fail", today, "no release: none of %s has a fuzz-YYYY-MM-DD release" % ", ".join(days))
    day = present[0]  # days run newest first
    counts = parse_status_counts(fetch_report("fuzz-" + day))
    if counts is None:
        return ("fail", day, "report unreadable: report.md has no window status table")
    total = sum(counts.values())
    if counts.get("ok", 0) < 1:
        return ("fail", day, "no ok window: report.md shows 0 ok of %d windows" % total)
    share = counts.get("infrastructure", 0) / total
    if share >= INFRASTRUCTURE_SHARE_BOUND:
        return ("fail", day, "infrastructure share too high: %.2f of windows, bound %.2f"
                % (share, INFRASTRUCTURE_SHARE_BOUND))
    return ("pass", day, "ok: release for %s, %d ok windows, infrastructure share %.2f"
            % (day, counts["ok"], share))


def run(today, backend):
    kind, day, message = evaluate(today, backend.release_tags(), backend.report)
    print("fuzz-heartbeat: %s: %s" % (kind, message))
    if kind != "fail":
        return 0
    if backend.open_issue_count() > 0:
        print("fuzz-heartbeat: an issue labelled %s is already open; not opening another" % ISSUE_LABEL)
        return 0
    title = "Fuzz loop check failed for %s" % day
    body = ("The daily fuzz heartbeat (.github/workflows/fuzz-heartbeat.yml) found a problem.\n\n"
            "Day: %s (UTC)\nCheck failed: %s\n\n"
            "This issue repairs nothing. A lane fixes anything in the tree; a repair of the fuzz host "
            "is a written spec the architect hands to the homelab agent. See "
            "notes/fuzzing/continuous-host.md. Close this issue when the loop is healthy again."
            % (day, message))
    backend.open_issue(title, body)
    print("fuzz-heartbeat: opened an issue for %s" % day)
    return 0


class GhBackend:
    def _gh(self, *args):
        return subprocess.run(["gh", *args], check=True, capture_output=True, text=True).stdout

    def release_tags(self):
        out = self._gh("release", "list", "--repo", REPO, "--limit", "1000", "--json", "tagName")
        return [r["tagName"] for r in json.loads(out)]

    def report(self, tag):
        return self._gh("release", "download", tag, "--repo", REPO, "--pattern", "report.md",
                        "--output", "-")

    def open_issue_count(self):
        out = self._gh("issue", "list", "--repo", REPO, "--label", ISSUE_LABEL, "--state", "open",
                       "--json", "number")
        return len(json.loads(out))

    def open_issue(self, title, body):
        # The label may not exist yet; creating it is idempotent here because an existing one is fine.
        subprocess.run(["gh", "label", "create", ISSUE_LABEL, "--repo", REPO,
                        "--description", "the continuous fuzz loop is broken"], capture_output=True)
        self._gh("issue", "create", "--repo", REPO, "--label", ISSUE_LABEL, "--title", title,
                 "--body", body)


class FixtureBackend:
    def __init__(self, reports, open_issues=0):
        self.reports, self.open_issues, self.created = reports, open_issues, []
        self.fetched = []

    def release_tags(self):
        return list(self.reports)

    def report(self, tag):
        self.fetched.append(tag)
        return self.reports[tag]

    def open_issue_count(self):
        return self.open_issues

    def open_issue(self, title, body):
        self.created.append((title, body))


def report_text(ok=10, infrastructure=0, held=0):
    rows = []
    for name, n in (("held-finding", held), ("infrastructure", infrastructure), ("ok", ok)):
        if n:
            rows.append("%s %d" % (name, n))
    return ("# Continuous fuzzing, daily report\n\n| driver | windows | seeds run |\n|---|---:|---:|\n\n"
            "Windows by status:\n\n```\n%s\n```\n\nNewest window: x\n" % "\n".join(rows))


def selftest():
    today = "2026-11-10"
    good = report_text(ok=10, infrastructure=2)

    def case(name, reports, open_issues, want_kind, want_issues, want_text=""):
        b = FixtureBackend(reports, open_issues)
        kind, _, msg = evaluate(today, b.release_tags(), b.report)
        assert kind == want_kind, "%s: kind %s (%s)" % (name, kind, msg)
        assert want_text in msg, "%s: message %r lacks %r" % (name, msg, want_text)
        b = FixtureBackend(reports, open_issues)
        assert run(today, b) == 0
        assert len(b.created) == want_issues, "%s: %d issues opened" % (name, len(b.created))
        assert set(b.fetched) <= {"fuzz-2026-11-10", "fuzz-2026-11-09"}, "%s: fetched %s" % (name, b.fetched)
        return b

    case("healthy today", {"fuzz-2026-11-10": good}, 0, "pass", 0)
    case("late release tolerated", {"fuzz-2026-11-09": good, "fuzz-2026-11-01": good}, 0, "pass", 0)
    case("missing release", {"fuzz-2026-11-08": good}, 0, "fail", 1, "no release")
    case("zero ok windows", {"fuzz-2026-11-10": report_text(ok=0, infrastructure=3, held=1)}, 0,
         "fail", 1, "no ok window")
    case("infrastructure share above bound", {"fuzz-2026-11-10": report_text(ok=4, infrastructure=6)}, 0,
         "fail", 1, "infrastructure share")
    case("share exactly at bound fails", {"fuzz-2026-11-10": report_text(ok=5, infrastructure=5)}, 0,
         "fail", 1, "infrastructure share")
    case("share just below bound", {"fuzz-2026-11-10": report_text(ok=6, infrastructure=5)}, 0, "pass", 0)
    case("unreadable report", {"fuzz-2026-11-10": "# empty\n"}, 0, "fail", 1, "unreadable")
    case("issue already open not duplicated", {"fuzz-2026-11-08": good}, 1, "fail", 0)
    case("quiet before the first release", {}, 0, "quiet", 0, "ever existed")
    case("unrelated tags are not fuzz releases", {"v1.0": good}, 0, "quiet", 0)
    # An old report that is bad does not count when a newer release in the window is fine.
    case("newest in window wins", {"fuzz-2026-11-10": good, "fuzz-2026-11-09": report_text(ok=0)}, 0,
         "pass", 0)
    b = case("issue names day and label", {"fuzz-2026-11-10": report_text(ok=0)}, 0, "fail", 1)
    assert "2026-11-10" in b.created[0][0] and "2026-11-10" in b.created[0][1]
    print("fuzz-heartbeat: selftest ok")


def main(argv):
    if argv[1:] == ["--selftest"]:
        selftest()
        return 0
    today = datetime.datetime.now(datetime.timezone.utc).date().isoformat()
    if len(argv) == 3 and argv[1] == "--today":
        today = argv[2]
    elif len(argv) != 1:
        print(__doc__.split("\n\n")[0], file=sys.stderr)
        return 2
    return run(today, GhBackend())


if __name__ == "__main__":
    sys.exit(main(sys.argv))
