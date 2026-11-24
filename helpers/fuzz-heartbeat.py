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
  3. the share of `infrastructure` windows in that report is below INFRASTRUCTURE_SHARE_BOUND;
  4. the report's `Filer:` line (helpers/fuzz-daily-publish.sh) shows a live filer: the tally is
     readable, no cause that is not a budget cause is left unfiled, `fuzz-filer.timer` is enabled, and
     the timer last ran no more than FILER_QUIET_DAYS UTC days before the report's day. A report from
     FILER_LINE_FROM on that has no such line fails; an older release without one passes.

On a failure it opens one issue carrying ISSUE_LABEL that names the check and the day, unless an issue
with that label is already open. On success it does nothing. It repairs nothing: a person turns the
issue into an action (partsfoundry/record:notes/fuzzing/continuous-host.md).

**Parked causes notify a person.** The filer parks budget and hang causes and never files them; each
waits for a person to write a `file` or `suppress` rule in fuzz-host `etc/fuzz-filer/rules.tsv`. So,
separately from the checks above, when the same report's `Filer:` line shows `parked` above zero and
above the count already noticed, this opens one issue carrying PARKED_LABEL. With one already open it
adds a comment instead, and never opens a second. The count already noticed is read back from the
open issue's body, which carries it in a PARKED_MARKER comment and is rewritten to the latest count
each day the count changes, so a fall followed by a rise is noticed and the workflow keeps no state of
its own. With no open issue the count already noticed is zero. The issue and its comments carry the
count and the release day only, never a signature, an ID, an input or a log line: issues are public
and the findings stay private on the fuzz host.

**Quiet before the first release.** The host publishes only after a person enables it. While no
`fuzz-*` release has ever existed, this passes with a log line and opens nothing. The first release
switches the checks on by itself.

**Why the filer is checked.** A release and an ok window prove the supervisor runs. They say nothing
about the filer, which turns a held finding into an advisory: with its timer disabled every other
check stayed green. The report carries counts and timestamps only, so this reads no held log.

**What it fetches.** The release list (tag names) and `report.md` of one release. Nothing else: no
`clean-ranges.tsv`, no other asset, nothing from the repository. The report holds counters only, and
this script prints check names and numbers, never report text.

Decided by the maintainer, provisional; reversing costs one pull request: the bound, the label and
the two-day window below. The parked notice was ruled 2026-11-22 (UTC); its label, PARKED_LABEL, is
provisional.

Name: provisional, minted 2026-11-01 (UTC) by lane/fuzz-dead-mans-switch.
"""

import datetime
import json
import re
import subprocess
import sys

REPO = "partsfoundry/nife"
# Failing share: infrastructure windows divided by all windows. The check passes below the bound.
INFRASTRUCTURE_SHARE_BOUND = 0.5
ISSUE_LABEL = "fuzz-loop-broken"
# Name: provisional, minted 2026-11-22 (UTC) by maintainer/fuzz-heartbeat-parked.
PARKED_LABEL = "fuzz-findings-parked"
PARKED_MARKER = "fuzz-heartbeat parked="
PARKED_MARKER_RE = re.compile(r"<!-- fuzz-heartbeat parked=(\d+) -->")
RULES_LINK = "https://github.com/partsfoundry/fuzz-host/blob/main/docs/fuzz-filer.md#rules"
WINDOW_DAYS = 2
# The filer must have run within this many UTC days of the report's day.
FILER_QUIET_DAYS = 2
# Reports from this day on must carry the `Filer:` line; older releases never had it and pass.
FILER_LINE_FROM = "2026-11-19"
FILER_LINE_RE = re.compile(r"^Filer: (.*)$", re.M)
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


def parse_filer_line(report):
    """{key: value} from the report's `Filer: key=value ...` line, or None when the line is absent."""
    m = FILER_LINE_RE.search(report)
    if m is None:
        return None
    return dict(w.split("=", 1) for w in m.group(1).split() if "=" in w)


def check_filer(day, report):
    """None when the filer looks alive, else the message of the failed check."""
    fields = parse_filer_line(report)
    if fields is None:
        if day >= FILER_LINE_FROM:
            return "filer line absent: report.md has no Filer line, and reports from %s on carry one" % FILER_LINE_FROM
        return None
    unfiled = fields.get("unfiled", "unknown")
    if not unfiled.isdigit():
        return "filer tally unavailable: the host could not count the causes left to file"
    if int(unfiled) > 0:
        return ("filer left %s cause(s) unfiled that are not budget causes: the filer is dead, "
                "unscheduled or refusing" % unfiled)
    if fields.get("timer-enabled") != "yes":
        return "filer timer not enabled: fuzz-filer.timer is %s" % fields.get("timer-enabled", "unknown")
    last = fields.get("timer-last-run", "unknown")
    try:
        last_day = datetime.date.fromisoformat(last[:10])
    except ValueError:
        return "filer has never run: fuzz-filer.timer has no recorded run (%s)" % last
    quiet = (datetime.date.fromisoformat(day) - last_day).days
    if quiet > FILER_QUIET_DAYS:
        return "filer has not run for %d days: fuzz-filer.timer last ran %s, bound %d days" % (
            quiet, last_day.isoformat(), FILER_QUIET_DAYS)
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
    report = fetch_report("fuzz-" + day)
    counts = parse_status_counts(report)
    if counts is None:
        return ("fail", day, "report unreadable: report.md has no window status table")
    total = sum(counts.values())
    if counts.get("ok", 0) < 1:
        return ("fail", day, "no ok window: report.md shows 0 ok of %d windows" % total)
    share = counts.get("infrastructure", 0) / total
    if share >= INFRASTRUCTURE_SHARE_BOUND:
        return ("fail", day, "infrastructure share too high: %.2f of windows, bound %.2f"
                % (share, INFRASTRUCTURE_SHARE_BOUND))
    filer = check_filer(day, report)
    if filer is not None:
        return ("fail", day, filer)
    return ("pass", day, "ok: release for %s, %d ok windows, infrastructure share %.2f"
            % (day, counts["ok"], share))


def newest_report(today, tags, fetch_report):
    """(day, report) of the newest release in the window, or None when the window has none."""
    dated = {m.group(1) for t in tags if (m := TAG_RE.match(t))}
    t = datetime.date.fromisoformat(today)
    for i in range(WINDOW_DAYS):
        day = (t - datetime.timedelta(days=i)).isoformat()
        if day in dated:
            return day, fetch_report("fuzz-" + day)
    return None


def parked_count(report):
    """The report's parked cause count, or None when its Filer line is absent or the count unreadable."""
    fields = parse_filer_line(report)
    if fields is None or not fields.get("parked", "").isdigit():
        return None
    return int(fields["parked"])


def noticed_count(body):
    """The parked count an open issue's body records, or 0 when it records none."""
    m = PARKED_MARKER_RE.search(body or "")
    return int(m.group(1)) if m else 0


def parked_body(day, parked):
    # Counts and the release day only. Issues are public; signatures, IDs, inputs and log lines stay
    # on the fuzz host.
    return ("The daily fuzz heartbeat (.github/workflows/fuzz-heartbeat.yml) found fuzz causes parked "
            "on the fuzz host.\n\n"
            "Release day: %s (UTC)\nParked causes: %d\n\n"
            "The fuzz filer parks budget and hang causes and never files them on its own. Each needs a "
            "person to decide it:\n\n"
            "1. Read the day's report on the fuzz host.\n"
            "2. For each parked cause, write a `file` or `suppress` rule in partsfoundry/fuzz-host "
            "`etc/fuzz-filer/rules.tsv` through a pull request. See %s.\n"
            "3. Close this issue once every parked cause has a rule.\n\n"
            "This issue names no cause on purpose: it is public, and the findings stay private on the "
            "fuzz host. A comment is added when the count grows.\n\n"
            "<!-- %s%d -->\n" % (day, parked, RULES_LINK, PARKED_MARKER, parked))


def notify_parked(day, report, backend):
    """Open or update the parked issue when the parked count grows past the count already noticed."""
    parked = parked_count(report)
    if parked is None:
        return
    issues = backend.open_issues(PARKED_LABEL)
    if not issues:
        if parked == 0:
            return
        backend.open_issue(PARKED_LABEL, "Fuzz causes parked for a person (%s)" % day,
                           parked_body(day, parked))
        print("fuzz-heartbeat: %d parked cause(s) on %s; opened an issue labeled %s"
              % (parked, day, PARKED_LABEL))
        return
    number, body = issues[0]
    noticed = noticed_count(body)
    if parked == noticed:
        print("fuzz-heartbeat: %d parked cause(s), already noticed in #%d" % (parked, number))
        return
    backend.edit_issue_body(number, parked_body(day, parked))
    if parked > noticed:
        backend.comment(number, "Parked causes grew from %d to %d in the release of %s (UTC). "
                        "Each needs a `file` or `suppress` rule; see %s." % (noticed, parked, day, RULES_LINK))
        print("fuzz-heartbeat: parked causes grew from %d to %d; commented on #%d" % (noticed, parked, number))
    else:
        print("fuzz-heartbeat: parked causes fell from %d to %d; updated #%d" % (noticed, parked, number))


def run(today, backend):
    tags = backend.release_tags()
    cache = {}

    def fetch(tag):
        if tag not in cache:
            cache[tag] = backend.report(tag)
        return cache[tag]

    kind, day, message = evaluate(today, tags, fetch)
    print("fuzz-heartbeat: %s: %s" % (kind, message))
    newest = newest_report(today, tags, fetch)
    if newest is not None:
        notify_parked(newest[0], newest[1], backend)
    if kind != "fail":
        return 0
    if backend.open_issues(ISSUE_LABEL):
        print("fuzz-heartbeat: an issue labeled %s is already open; not opening another" % ISSUE_LABEL)
        return 0
    title = "Fuzz loop check failed for %s" % day
    body = ("The daily fuzz heartbeat (.github/workflows/fuzz-heartbeat.yml) found a problem.\n\n"
            "Day: %s (UTC)\nCheck failed: %s\n\n"
            "This issue repairs nothing. A lane fixes anything in the tree; a repair of the fuzz host "
            "is a written spec the architect hands to the homelab agent. See "
            "partsfoundry/record:notes/fuzzing/continuous-host.md. Close this issue when the loop is healthy again."
            % (day, message))
    backend.open_issue(ISSUE_LABEL, title, body)
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

    def open_issues(self, label):
        """[(number, body)] of the open issues carrying label, oldest first."""
        out = self._gh("issue", "list", "--repo", REPO, "--label", label, "--state", "open",
                       "--limit", "100", "--json", "number,body")
        return sorted((i["number"], i["body"]) for i in json.loads(out))

    def open_issue(self, label, title, body):
        # The label may not exist yet; creating it is idempotent here because an existing one is fine.
        description = {ISSUE_LABEL: "the continuous fuzz loop is broken",
                       PARKED_LABEL: "fuzz causes parked on the fuzz host wait for a rule"}[label]
        subprocess.run(["gh", "label", "create", label, "--repo", REPO, "--description", description],
                       capture_output=True)
        self._gh("issue", "create", "--repo", REPO, "--label", label, "--title", title, "--body", body)

    def edit_issue_body(self, number, body):
        self._gh("issue", "edit", str(number), "--repo", REPO, "--body", body)

    def comment(self, number, body):
        self._gh("issue", "comment", str(number), "--repo", REPO, "--body", body)


class FixtureBackend:
    """open_issues is the count of open ISSUE_LABEL issues; parked_issue is the body of an open
    PARKED_LABEL issue (number 7), or None."""

    def __init__(self, reports, open_issues=0, parked_issue=None):
        self.reports, self.loop_open, self.created = reports, open_issues, []
        self.parked_issue, self.comments, self.edits = parked_issue, [], []
        self.fetched = []

    def release_tags(self):
        return list(self.reports)

    def report(self, tag):
        self.fetched.append(tag)
        return self.reports[tag]

    def open_issues(self, label):
        if label == ISSUE_LABEL:
            return [(n, "") for n in range(1, self.loop_open + 1)]
        return [] if self.parked_issue is None else [(7, self.parked_issue)]

    def open_issue(self, label, title, body):
        self.created.append((label, title, body))

    def edit_issue_body(self, number, body):
        self.edits.append((number, body))
        self.parked_issue = body

    def comment(self, number, body):
        self.comments.append((number, body))

    def labeled(self, label):
        return [(t, b) for (l, t, b) in self.created if l == label]


def filer_line(unfiled=0, enabled="yes", last="2026-11-19T09:30:00Z", held=10616, causes=17,
               filed=17, parked=0, ledger="2026-11-07T12:00:00Z"):
    return ("Filer: held=%s causes=%s filed=%s parked=%s unfiled=%s ledger-newest=%s timer-enabled=%s "
            "timer-last-run=%s\n" % (held, causes, filed, parked, unfiled, ledger, enabled, last))


def report_text(ok=10, infrastructure=0, held=0, filer=""):
    rows = []
    for name, n in (("held-finding", held), ("infrastructure", infrastructure), ("ok", ok)):
        if n:
            rows.append("%s %d" % (name, n))
    return ("# Continuous fuzzing, daily report\n\n| driver | windows | seeds run |\n|---|---:|---:|\n\n"
            "Windows by status:\n\n```\n%s\n```\n\nNewest window: x\n\n%s" % ("\n".join(rows), filer))


def selftest():
    today = "2026-11-10"
    good = report_text(ok=10, infrastructure=2)

    def case(name, reports, open_issues, want_kind, want_issues, want_text="", today=today):
        b = FixtureBackend(reports, open_issues)
        kind, _, msg = evaluate(today, b.release_tags(), b.report)
        assert kind == want_kind, "%s: kind %s (%s)" % (name, kind, msg)
        assert want_text in msg, "%s: message %r lacks %r" % (name, msg, want_text)
        b = FixtureBackend(reports, open_issues)
        assert run(today, b) == 0
        assert len(b.labeled(ISSUE_LABEL)) == want_issues, "%s: %d issues opened" % (name, len(b.created))
        assert len(b.fetched) <= 1, "%s: fetched %s" % (name, b.fetched)
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
    assert "2026-11-10" in b.created[0][1] and "2026-11-10" in b.created[0][2]

    # The filer checks. Reports from FILER_LINE_FROM on must carry a live Filer line.
    later = "2026-11-20"
    tag = "fuzz-" + later

    def filer_case(name, line, want_kind, want_text=""):
        case(name, {tag: report_text(filer=line)}, 0, want_kind, int(want_kind == "fail"), want_text,
             today=later)

    filer_case("filer alive", filer_line(), "pass")
    filer_case("filer ran two days ago", filer_line(last="2026-11-18T00:00:00Z"), "pass")
    filer_case("filer ran three days ago", filer_line(last="2026-11-17T23:59:59Z"), "fail",
               "has not run for 3 days")
    filer_case("filer never ran", filer_line(last="n/a"), "fail", "never run")
    filer_case("filer last run unknown", filer_line(last="unknown"), "fail", "never run")
    filer_case("timer disabled", filer_line(enabled="no"), "fail", "timer not enabled")
    filer_case("timer state missing", "Filer: unfiled=0 timer-last-run=2026-11-20T01:00:00Z\n", "fail",
               "timer not enabled")
    filer_case("one unfiled cause", filer_line(unfiled=1), "fail", "1 cause(s) unfiled")
    b = case("parked causes alone open the parked issue",
             {tag: report_text(filer=filer_line(filed=1, parked=16))}, 0, "pass", 0, today=later)
    assert [t for t, _ in b.labeled(PARKED_LABEL)] == ["Fuzz causes parked for a person (%s)" % later]
    filer_case("tally unavailable", filer_line(unfiled="unknown"), "fail", "tally unavailable")
    filer_case("line absent from a new report", "", "fail", "filer line absent")
    case("line absent from the first day that requires it", {"fuzz-2026-11-19": report_text()}, 0,
         "fail", 1, "filer line absent", today="2026-11-19")
    case("old release without the line passes", {"fuzz-2026-11-18": report_text()}, 0, "pass", 0,
         today="2026-11-18")
    b = FixtureBackend({tag: report_text(filer=filer_line(unfiled=3))})
    run(later, b)
    assert "unfiled" in b.created[0][2] and "10616" not in b.created[0][2]

    # The parked notice. It opens one issue, comments on growth, and never opens a second.
    def parked_case(name, parked, issue_body, want_new, want_comment, want_edit):
        b = FixtureBackend({tag: report_text(filer=filer_line(filed=17 - parked, parked=parked))},
                           parked_issue=issue_body)
        assert run(later, b) == 0
        assert len(b.labeled(PARKED_LABEL)) == want_new, "%s: %d parked issues" % (name, len(b.created))
        assert len(b.comments) == want_comment, "%s: %d comments" % (name, len(b.comments))
        assert len(b.edits) == want_edit, "%s: %d edits" % (name, len(b.edits))
        assert not b.labeled(ISSUE_LABEL), "%s: opened %s" % (name, ISSUE_LABEL)
        return b

    noticed_16 = parked_body("2026-11-19", 16)
    b = parked_case("parked issue states counts and day only", 16, None, 1, 0, 0)
    _, body = b.labeled(PARKED_LABEL)[0]
    assert "16" in body and later in body and RULES_LINK in body and "10616" not in body
    assert noticed_count(body) == 16
    parked_case("no parked causes open nothing", 0, None, 0, 0, 0)
    parked_case("no growth with an issue open stays quiet", 16, noticed_16, 0, 0, 0)
    b = parked_case("growth with an issue open comments, no second issue", 18, noticed_16, 0, 1, 1)
    assert "16 to 18" in b.comments[0][1] and b.comments[0][0] == 7 and noticed_count(b.parked_issue) == 18
    b = parked_case("a fall updates the noticed count without a comment", 10, noticed_16, 0, 0, 1)
    assert noticed_count(b.parked_issue) == 10
    parked_case("an open issue with no marker counts as zero noticed", 3, "opened by hand", 0, 1, 1)
    b = FixtureBackend({tag: report_text(filer="Filer: unfiled=0 parked=unknown timer-enabled=yes "
                                                "timer-last-run=2026-11-20T01:00:00Z\n")})
    run(later, b)
    assert not b.created and not b.comments, "unknown parked count opened something"
    # A broken loop and parked causes are two issues, each under its own label.
    b = FixtureBackend({tag: report_text(filer=filer_line(filed=1, parked=16, enabled="no"))})
    run(later, b)
    assert len(b.labeled(ISSUE_LABEL)) == 1 and len(b.labeled(PARKED_LABEL)) == 1
    assert b.fetched == [tag], "fetched %s" % b.fetched
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
