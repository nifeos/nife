#!/usr/bin/env python3
"""helpers/fuzz-status.py: is the continuous fuzz working? One PASS, FAIL or UNKNOWN line per check.

    script/fuzz-status [OPTIONS]            checks that need only `gh` and the repository
    script/fuzz-status --host [OPTIONS]     checks on the fuzz host itself

The front door is script/fuzz-status, which runs this file. Read-only: it writes nothing, restarts
nothing, and never uses sudo. It prints counts, ages and check names, and never a finding, a seed, a
crash input or the contents of any held file. Exit status: 1 when any check FAILS, else 0 (an UNKNOWN
means the check could not be made, and is not a failure); 2 is a usage error.

Default mode (`gh` logged in to read nifeos/nife):
  heartbeat runs        the newest completed run of the fuzz heartbeat workflow succeeded and is recent
  fuzz release exists   at least one fuzz-YYYY-MM-DD release exists
  daily publish is on   the newest fuzz-* release is from today or yesterday (UTC)
  loop-broken issues    no open issue carries the label fuzz-loop-broken (count only)
  advisories            the count of security advisories this token can see (count only; drafts show
                        only to a maintainer, so the count may undercount; never FAILS)
  host commit           the commit the follower would have the host at: main's tip when every required
                        check is green on it (the rule of helpers/fuzz-follow-main.py, applied here)

--host mode (run on the fuzz host, as a user that can read the journal, such as a member of the
`systemd-journal` or `adm` group; no sudo is used, and a check that needs more says UNKNOWN):
  service active        nife-fuzz.service is active
  publish timer         nife-fuzz-publish.timer is enabled and active; prints its next run
  slot iteration        a "continuous-fuzz: slot N <driver>: <status>" line is among the last 20
                        journal lines of nife-fuzz.service, no older than --max-age-minutes
  follow-main           the clone's commit equals or trails the main tip the follower last fetched
                        (refs/fuzz-follow/main) by at most --max-behind commits
  output directory      out/<today> or out/<yesterday> exists under the fuzz home
  slot worktrees        work/0 .. work/N-1 exist and answer `git rev-parse HEAD`; the recent journal
                        holds no "failed to read ... commondir" (the race #3264 fixed)
  corpus growth         clean windows in windows.tsv. Without --wait, at least one clean window ended
                        in the last hour. With --wait SECONDS, the clean-window count and clean seed
                        total are read twice and must grow.
  disk free             the filesystem under the fuzz home has at least --disk-floor-gib free

Options:
  --repo OWNER/NAME        default nifeos/nife
  --max-age-minutes N      slot iteration freshness, default 30 (a cold build can take that long)
  --max-behind N           follow-main tolerance in commits, default 25
  --guests N               configured slots, default $NIFE_FUZZ_GUESTS or 4
  --home DIR               the fuzz home, default $NIFE_FUZZ_HOME, else $HOME/fuzz
  --wait SECONDS           second corpus reading after this long, default 0 (off)
  --disk-floor-gib N       default 20
  --heartbeat-max-age-hours N  default 48

Name: provisional, minted 2026-11-05 (UTC) by maintainer/fuzz-status.
"""

import datetime
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import time

PASS, FAIL, UNKNOWN = "PASS", "FAIL", "UNKNOWN"
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
HEARTBEAT_WORKFLOW = "fuzz-heartbeat.yml"
LOOP_LABEL = "fuzz-loop-broken"
TAG_RE = re.compile(r"^fuzz-(\d{4}-\d{2}-\d{2})$")
MARKER_RE = re.compile(r"continuous-fuzz: slot \d+ (kernel-thread|confined): \S+")
COMMONDIR_RE = re.compile(r"failed to read .*commondir")
FOLLOW_REF = "refs/fuzz-follow/main"
SERVICE = "nife-fuzz.service"
TIMER = "nife-fuzz-publish.timer"


class Unknown(Exception):
    """The check could not be made; the message says why."""


def now():
    return float(os.environ.get("NIFE_FUZZ_STATUS_NOW") or time.time())


def today():
    return datetime.datetime.fromtimestamp(now(), datetime.timezone.utc).date()


def run(argv, timeout=60):
    try:
        proc = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
    except FileNotFoundError:
        raise Unknown("%s is not installed" % argv[0])
    except subprocess.TimeoutExpired:
        raise Unknown("%s did not answer within %d s" % (argv[0], timeout))
    return proc


def gh_json(*args):
    proc = run(["gh", *args])
    if proc.returncode != 0:
        last = (proc.stderr.strip().splitlines() or ["exit %d" % proc.returncode])[-1]
        raise Unknown("gh %s failed: %s" % (args[0], last[:120]))
    try:
        return json.loads(proc.stdout)
    except ValueError:
        raise Unknown("gh %s answered something that is not JSON" % args[0])


def parse_time(text):
    return datetime.datetime.fromisoformat(text.replace("Z", "+00:00")).timestamp()


# ---- default mode -------------------------------------------------------------------------------

def check_heartbeat(o):
    runs = gh_json("run", "list", "--repo", o.repo, "--workflow", HEARTBEAT_WORKFLOW, "--limit", "7",
                   "--json", "status,conclusion,createdAt")
    done = [r for r in runs if r.get("status") == "completed"]
    if not done:
        raise Unknown("no completed run of %s yet" % HEARTBEAT_WORKFLOW)
    ok = sum(1 for r in done if r.get("conclusion") == "success")
    latest = done[0]
    age_h = (now() - parse_time(latest["createdAt"])) / 3600
    note = "%d of the last %d completed runs succeeded; latest %s %.0f h ago" % (
        ok, len(done), latest.get("conclusion"), age_h)
    if latest.get("conclusion") != "success":
        return FAIL, note
    if age_h > o.heartbeat_max_age_hours:
        return FAIL, note + ", older than %d h, so the schedule is not running" % o.heartbeat_max_age_hours
    return PASS, note


def release_days(o):
    rels = gh_json("release", "list", "--repo", o.repo, "--limit", "200", "--json", "tagName")
    return sorted((m.group(1) for r in rels if (m := TAG_RE.match(r.get("tagName", "")))), reverse=True)


def check_release_exists(o):
    days = release_days(o)
    if not days:
        return FAIL, "no fuzz-YYYY-MM-DD release exists; the publish has never run"
    return PASS, "%d fuzz release(s); newest fuzz-%s" % (len(days), days[0])


def check_publish_on(o):
    days = release_days(o)
    if not days:
        return FAIL, "no fuzz-* release, so the daily publish is off or has never run"
    t = today()
    newest = datetime.date.fromisoformat(days[0])
    behind = (t - newest).days
    if behind <= 1:
        return PASS, "newest release fuzz-%s is %d day(s) old" % (days[0], max(behind, 0))
    return FAIL, "newest release fuzz-%s is %d days old; the daily publish is off or failing" % (days[0], behind)


def check_loop_issues(o):
    issues = gh_json("issue", "list", "--repo", o.repo, "--label", LOOP_LABEL, "--state", "open",
                     "--json", "number", "--limit", "100")
    if issues:
        return FAIL, "%d open issue(s) labeled %s" % (len(issues), LOOP_LABEL)
    return PASS, "no open issue labeled %s" % LOOP_LABEL


def check_advisories(o):
    ads = gh_json("api", "repos/%s/security-advisories" % o.repo)
    if not isinstance(ads, list):
        raise Unknown("the advisory list is not readable with this token")
    return PASS, "%d advisory(ies) visible to this token (count only; drafts show only to a maintainer)" % len(ads)


def load_follow_main():
    spec = importlib.util.spec_from_file_location("fuzz_follow_main", os.path.join(HERE, "fuzz-follow-main.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def check_host_commit(o):
    fm = load_follow_main()
    tip = gh_json("api", "repos/%s/commits/main" % o.repo).get("sha")
    if not isinstance(tip, str) or not fm.SHA_RE.match(tip):
        raise Unknown("main's tip is unreadable")
    rules = gh_json("api", "repos/%s/rules/branches/main" % o.repo)
    try:
        required = fm.required_checks(rules)
    except fm.Stay as stay:
        raise Unknown(str(stay))
    first = gh_json("api", "repos/%s/commits/%s/check-runs?per_page=100&page=1" % (o.repo, tip))
    total = first.get("total_count") if isinstance(first, dict) else None
    if not isinstance(total, int):
        raise Unknown("the check runs are unreadable")
    runs = list(first.get("check_runs") or [])
    for page in range(2, (total + 99) // 100 + 1):
        more = gh_json("api", "repos/%s/commits/%s/check-runs?per_page=100&page=%d" % (o.repo, tip, page))
        runs.extend(more.get("check_runs") or [])
    reason = fm.verdict(tip, required, runs)
    if reason is None:
        return PASS, "main's tip %s is green; the host should be at it" % tip[:12]
    if "concluded" in reason:
        return FAIL, "main's tip %s is red (%s); the host stays at an older commit" % (tip[:12], reason)
    return UNKNOWN, "main's tip %s is not green yet (%s); the host stays where it is" % (tip[:12], reason)


# ---- host mode ----------------------------------------------------------------------------------

def systemctl_show(unit, *props):
    args = ["systemctl", "show", unit, "--no-pager"] + ["-p%s" % p for p in props]
    proc = run(args)
    if proc.returncode != 0:
        raise Unknown("systemctl show %s failed" % unit)
    out = {}
    for line in proc.stdout.splitlines():
        k, _, v = line.partition("=")
        out[k] = v
    return out


def check_service(o):
    proc = run(["systemctl", "is-active", SERVICE])
    state = proc.stdout.strip() or "unknown"
    if state == "active":
        return PASS, "%s is active" % SERVICE
    return FAIL, "%s is %s" % (SERVICE, state)


def check_timer(o):
    p = systemctl_show(TIMER, "UnitFileState", "ActiveState", "NextElapseUSecRealtime")
    enabled, active = p.get("UnitFileState", ""), p.get("ActiveState", "")
    nxt = p.get("NextElapseUSecRealtime") or "none scheduled"
    if enabled == "enabled" and active == "active":
        return PASS, "%s enabled and active; next run %s" % (TIMER, nxt)
    return FAIL, "%s is %s and %s" % (TIMER, enabled or "not installed", active or "inactive")


def journal(lines):
    proc = run(["journalctl", "-u", SERVICE, "-n", str(lines), "--no-pager", "-o", "short-unix"])
    if proc.returncode != 0:
        raise Unknown("journalctl failed; this user may not read the journal (no sudo is used)")
    out = [l for l in proc.stdout.splitlines() if l.strip() and not l.startswith("-- ")]
    if not out:
        raise Unknown("the journal of %s is empty or unreadable by this user" % SERVICE)
    return out


def check_iteration(o):
    best = None
    for line in journal(20):
        if MARKER_RE.search(line):
            m = re.match(r"(\d+(?:\.\d+)?)\s", line)
            if m:
                best = float(m.group(1))
    if best is None:
        return FAIL, "no slot iteration line among the last 20 journal lines of %s" % SERVICE
    age = (now() - best) / 60
    if age <= o.max_age_minutes:
        return PASS, "last slot iteration %.0f min ago (limit %d)" % (age, o.max_age_minutes)
    return FAIL, "last slot iteration %.0f min ago, over the %d-minute limit" % (age, o.max_age_minutes)


CLONE = os.environ.get("NIFE_FUZZ_STATUS_CLONE") or ROOT


def git(*args):
    return run(["git", "-C", CLONE, *args], timeout=30)


def check_follow(o):
    head = git("rev-parse", "HEAD")
    if head.returncode != 0:
        raise Unknown("the clone at %s does not answer git" % CLONE)
    tip = git("rev-parse", "--verify", "--quiet", FOLLOW_REF + "^{commit}")
    if tip.returncode != 0:
        raise Unknown("%s is absent; the follower has not looked yet" % FOLLOW_REF)
    h, t = head.stdout.strip(), tip.stdout.strip()
    if h == t:
        return PASS, "the clone is at the followed main tip %s" % h[:12]
    ancestry = git("merge-base", "--is-ancestor", h, t)
    if ancestry.returncode != 0:
        return FAIL, "the clone's %s is not an ancestor of the followed main %s" % (h[:12], t[:12])
    n = int(git("rev-list", "--count", "%s..%s" % (h, t)).stdout.strip() or 0)
    if n <= o.max_behind:
        return PASS, "the clone trails the followed main by %d commit(s) (tolerance %d)" % (n, o.max_behind)
    return FAIL, "the clone trails the followed main by %d commits, over the tolerance of %d" % (n, o.max_behind)


def check_output(o):
    out = os.path.join(o.home, "out")
    if not os.path.isdir(o.home):
        raise Unknown("the fuzz home %s is not readable here" % o.home)
    t = today()
    for d in (t, t - datetime.timedelta(days=1)):
        if os.path.isdir(os.path.join(out, d.isoformat())):
            return PASS, "out/%s exists" % d.isoformat()
    return FAIL, "neither out/%s nor out/%s exists" % (t.isoformat(), (t - datetime.timedelta(days=1)).isoformat())


def check_worktrees(o):
    if not os.path.isdir(o.home):
        raise Unknown("the fuzz home %s is not readable here" % o.home)
    bad = []
    for n in range(o.guests):
        d = os.path.join(o.home, "work", str(n))
        if not os.path.isdir(d):
            bad.append("slot %d has no worktree" % n)
            continue
        try:
            proc = run(["git", "-C", d, "rev-parse", "HEAD"], timeout=20)
        except Unknown:
            bad.append("slot %d is stuck (git did not answer)" % n)
            continue
        if proc.returncode != 0:
            bad.append("slot %d does not resolve HEAD" % n)
    if any(COMMONDIR_RE.search(l) for l in journal(200)):
        bad.append("the recent journal holds a commondir read failure (the race #3264 fixed)")
    if bad:
        return FAIL, "; ".join(bad)
    return PASS, "%d slot worktree(s) present and answering; no commondir failure in the last 200 journal lines" % o.guests


def clean_windows(o):
    path = os.path.join(o.home, "state", "windows.tsv")
    try:
        with open(path) as f:
            rows = [l.rstrip("\n").split("\t") for l in f]
    except OSError:
        raise Unknown("state/windows.tsv is not readable")
    count, seeds, last = 0, 0, 0.0
    for r in rows:
        if len(r) >= 7 and r[6] == "ok":  # only clean rows are read; a held row is never looked at
            count += 1
            seeds += int(r[4]) if r[4].isdigit() else 0
            try:
                last = max(last, parse_time(r[0]))
            except ValueError:
                pass
    return count, seeds, last


def check_corpus(o):
    c1, s1, last = clean_windows(o)
    if o.wait > 0:
        time.sleep(o.wait)
        c2, s2, _ = clean_windows(o)
        if c2 > c1 and s2 > s1:
            return PASS, "clean windows %d -> %d and clean seeds %d -> %d over %d s" % (c1, c2, s1, s2, o.wait)
        return FAIL, "no growth over %d s: clean windows %d -> %d, clean seeds %d -> %d" % (o.wait, c1, c2, s1, s2)
    if c1 == 0:
        return FAIL, "windows.tsv holds no clean window"
    age = (now() - last) / 60
    note = "%d clean windows, %d clean seeds; the newest ended %.0f min ago" % (c1, s1, age)
    if age <= 60:
        return PASS, note + " (pass --wait SECONDS for two readings)"
    return FAIL, note + ", over an hour"


def check_disk(o):
    if not os.path.isdir(o.home):
        raise Unknown("the fuzz home %s is not readable here" % o.home)
    free = shutil.disk_usage(o.home).free / 2**30
    if free >= o.disk_floor_gib:
        return PASS, "%.0f GiB free (floor %d)" % (free, o.disk_floor_gib)
    return FAIL, "%.0f GiB free, under the floor of %d" % (free, o.disk_floor_gib)


DEFAULT_CHECKS = [
    ("heartbeat runs", check_heartbeat),
    ("fuzz release exists", check_release_exists),
    ("daily publish is on", check_publish_on),
    ("loop-broken issues", check_loop_issues),
    ("advisories", check_advisories),
    ("host commit", check_host_commit),
]
HOST_CHECKS = [
    ("service active", check_service),
    ("publish timer", check_timer),
    ("slot iteration", check_iteration),
    ("follow-main", check_follow),
    ("output directory", check_output),
    ("slot worktrees", check_worktrees),
    ("corpus growth", check_corpus),
    ("disk free", check_disk),
]


def parse(argv):
    class Opt:
        host = False
        repo = "nifeos/nife"
        max_age_minutes = 30
        max_behind = 25
        guests = int(os.environ.get("NIFE_FUZZ_GUESTS") or 4)
        home = os.environ.get("NIFE_FUZZ_HOME") or os.path.join(os.path.expanduser("~"), "fuzz")
        wait = 0
        disk_floor_gib = 20
        heartbeat_max_age_hours = 48

    o = Opt()
    ints = {"--max-age-minutes": "max_age_minutes", "--max-behind": "max_behind", "--guests": "guests",
            "--wait": "wait", "--disk-floor-gib": "disk_floor_gib",
            "--heartbeat-max-age-hours": "heartbeat_max_age_hours"}
    it = iter(argv)
    for a in it:
        if a == "--host":
            o.host = True
        elif a in ("-h", "--help"):
            print(__doc__.split("\nName:")[0].strip())
            sys.exit(0)
        elif a in ints or a in ("--repo", "--home"):
            v = next(it, None)
            if v is None:
                print("fuzz-status: %s needs a value" % a, file=sys.stderr)
                sys.exit(2)
            if a == "--repo":
                o.repo = v
            elif a == "--home":
                o.home = v
            else:
                if not v.isdigit():
                    print("fuzz-status: %s needs a whole number" % a, file=sys.stderr)
                    sys.exit(2)
                setattr(o, ints[a], int(v))
        else:
            print("fuzz-status: unknown argument %s (try --help)" % a, file=sys.stderr)
            sys.exit(2)
    return o


def main(argv):
    o = parse(argv)
    failed = 0
    unknown = 0
    for name, fn in (HOST_CHECKS if o.host else DEFAULT_CHECKS):
        try:
            status, note = fn(o)
        except Unknown as why:
            status, note = UNKNOWN, str(why)
        except Exception as error:  # a check that breaks is unknown, never a silent pass
            status, note = UNKNOWN, "%s: %s" % (type(error).__name__, error)
        failed += status == FAIL
        unknown += status == UNKNOWN
        print("%-7s %-20s %s" % (status, name, note))
    print("fuzz-status: %d failed, %d unknown (%s mode)" % (failed, unknown, "host" if o.host else "default"))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
