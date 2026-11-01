#!/usr/bin/env python3
"""Where CI's merge-group time goes, day by day, and a tracking issue for a job near its budget.

    python3 helpers/ci_job_times.py update [--since YYYY-MM-DD]   # the daily record
    python3 helpers/ci_job_times.py update --repo <owner/name>     # the same record for one crate repository
    python3 helpers/ci_job_times.py fold [--today YYYY-MM-DD]      # fold the files' old weeks, no API
    python3 helpers/ci_job_times.py sync [--dry-run]               # open, update or close issues
    python3 helpers/ci_job_times.py --selftest

Milestone 808 (every gate accounts for its time), under §254 (a gate prints what each item cost, and
a job near its budget warns rather than fails). Both run in `.github/workflows/metrics.yml` once a
day, as `nife-smelter[bot]`. Milestone 887 (crate repositories share one CI template, adapted
from nife's gates) added `--repo`: a crate repository has no merge group, so
its record reads its pull-request runs of the two caller workflows and lands in a per-repository CSV
beside this one, so a slow template job shows up in this tree's metrics rather than in nine places
nobody watches.

Name: provisional, minted 2026-10-07 (UTC) by milestone 808's lane, as are the CSV's name, the
`near-budget` label and the chart `ci-jobs.svg`. A python helper under `helpers/`, outside
`script/names`' scope, so this paragraph is its provenance. Refused: `job_budget_watch`, which
names one of its two jobs and is one letter from `helpers/job-budget.py`.

# WHY

Milestone 721 (each merge-group CI job has a 20-minute budget) gave every `ci.yml` job one number
and a gate on it. On 2026-10-06 the gate fired on `cpu-matrix` at 20.1 to 21.0 minutes, and the
answer took a lane reading 31 runs out of the Actions API by hand: no step had regressed, the suite
had grown about 0.1 minutes per model per day for two weeks, and every run in that window showed
it. Nothing read CI time over weeks. This does, in two halves.

# `update`: the daily record (§254, Fork 4)

For each UTC day, every completed `merge_group` run of `ci.yml` and `verify.yml` (`RECORDED`), read
through the jobs API, which already holds each job's and each step's start and end, so no job
had to change to provide them. One row per day, workflow, job and step, appended to `CSV`:

    date,workflow,job,variant,kind,name,runs,median_seconds,max_seconds

`job` is the workflow's job key, the name `.github/ci-job-budgets` uses. `variant` is what a matrix
filled into the display name (`rv64 sifive-u54 rva22s64`), empty otherwise. `kind` is `job` (the
whole job; `name` is empty) or `step` (`name` is the step's).

The per-item rows come from each green run's `time-record-<job>` artifacts (milestone 807 (the
kernel suite reports what each test cost) wrote the format; `helpers/time_record.py` maps an
artifact to its job). Their `kind` is the record's own (`phase`, `test`, `line`, `target`,
`falsification`), `variant` is `<leg> / <image>`, `name` is the phase or item, and the seconds
carry one decimal. Every `phase` row is kept. An item is kept only when its day's median is at
least `ITEM_FLOOR_MS`: one run records about 5,300 kernel tests, and a row for each a day would
add about 650 kB a day to this file, where the 1 s floor keeps about 380. A drift spread over
many fast tests, the shape PR #1775 found, still shows in the `suite` phase of its image.
`script/metrics` read the `job` rows into `ci-jobs.svg` until 2026-10-11 (UTC); the record stays as
the per-step history a budget issue's reader digs into.

The same pass writes `BUILDS_CSV`, which `ci-jobs.svg` and `ci-critical-path.svg` now read (calef,
2026-10-11 UTC: "how long a PR build takes against a lid of 20 minutes broken down by each job"). A
build is a merge commit's runs of both workflows, matched on `head_sha` (`build_rows`), and its
wall time is first job start to last job end across them (`build_wall`):

    date,build,kind,workflow,job,start_seconds,end_seconds,seconds

`kind` is `build` (one row, `seconds` the wall time) or `job` (start and end in seconds from the
build's first job start; a matrix job is one row, `seconds` its slowest variant).

# The fold: per-job rows for the latest 13 weeks, one row per week before that

Ruled 2026-11-01 (UTC): `CSV` and `BUILDS_CSV` keep per-job rows for the latest `FOLD_AFTER_WEEKS`
ISO weeks (this week and the twelve before it) and nothing finer, so the files stop growing without
a person trimming them. `update` folds every older week into one row per week and job as it appends
(`fold_times`, `fold_builds`), and a week once folded is never read from the API again (`update`
starts no earlier than `fold_cutoff`). The fold is lossy by design. The raw rows stay in git
history, one `git log -p` away.

A folded row is told by its `date`, which is the ISO week (`2026W33`) and not a day. Folded rows
keep their file's header, with these meanings:

- `CSV`: one row per week, workflow, job, variant, kind and name. `runs` is the sum of the days'
  runs, `median_seconds` the days' medians weighted by their runs, `max_seconds` the largest.
- `BUILDS_CSV`: one `build` row per week (`builds` is how many green builds it had, `seconds` and
  `end_seconds` their median wall time, `p90_seconds` the 90th percentile) and one `job` row per
  week and job (`start_seconds`, `end_seconds` and `seconds` are medians over the builds that ran
  it, `builds` how many did). The columns `builds` and `p90_seconds` are empty on a per-build row.

These are the figures `script/metrics` charts: a week's build count, median and p90 wall time, and
each job's weekly median. A week folded keeps all of them to the digit; nothing it draws needs a
per-build row. The fold is idempotent, since a row that is already folded passes through unchanged.

# `sync`: the tracking issue (§254, Fork 2)

For each job, the newest `HISTORY` merge-group runs that ran it. When the newest is at or past
`WARN_FRACTION` of the job's budget and no issue is open for the job, it opens one, labeled `LABEL`.
While one is open it rewrites the issue's body on every pass with the recent runs, so the same issue
is updated rather than a second opened. It closes the issue when the newest `CLOSE_AFTER` runs are
all under the line. While an issue is open, it appends one row a week to week-notes.csv, which
`script/metrics` draws as a bullet under `ci-jobs.svg` (milestone 623 (bullet under the chart
explains a cliff)).

**Damping: one run opens, three runs close.** §254's Open section left to this lane whether the line
reads one run or a median. Opening on one run keeps the warning as early as the ruling wants; it is
a warning, it fails nothing, and the wall time of a job sitting at 84% one day and 86% the next is a
job that deserves a look. Closing on one run would flap: the same job on the same commit varies by
about 30% across hosted runners (helpers/verify_times.py measured 455 to 801 seconds for one crate on
2026-10-06). Three in a row under the line is the hysteresis.

The issue reaches a session through `needs-maintainer` (the `budget` cause in
helpers/needs-maintainer.jq), as calef ruled on 2026-10-06 (UTC).

# BUGS

- `--repo` trusts the scaffold's caller filenames (ci.yml, proofs.yml). A caller that renames them
  records a repository with no runs, which is a visible zero in the CSV, not a silent skip.
- `--repo` skips the builds record entirely. A pull-request run's wall time says more about
  GitHub's queueing than about the template's jobs, and nife's build charts read merge groups.
- `sync` runs once a day, so an issue opens up to a day after the run that crossed. The drift it
  exists for is measured in weeks.
- A matrix job's runs are judged by their slowest variant, since that is the pole that meets the
  budget; the issue names it.
- Runner speed is not controlled. Every number is wall time on a shared hosted machine.
- `update` matches a run's display names against every version of its workflow in `git log`
  (`patterns_across`), not the one it ran, so a name two versions gave to different keys reads as
  the newer key. `sync` reads only the newest runs and so only the current file.
- A build's wall time starts at its first job's start, so the wait for the first runner is not in
  it; a later job's wait for a runner is.
- A build counts only when every run in it succeeded. A failed build stops early or is ejected, and
  folding it in would pull the median down for the wrong reason.
- Runs are read from the API on the day, and GitHub keeps run history for 90 days, so a missed day
  can be backfilled with `--since` only within that window.
"""

import concurrent.futures
import csv
import datetime
import glob
import importlib.util
import io
import json
import os
import re
import statistics
import subprocess
import sys
import time
import zipfile

REPO = "nifeos/nife"
CSV = "notes/project-metrics/ci-job-times.csv"
COLUMNS = ["date", "workflow", "job", "variant", "kind", "name", "runs", "median_seconds", "max_seconds"]
WEEK_NOTES = "notes/project-metrics/week-notes.csv"
CHART = "ci-jobs.svg"
# The smallest daily median an item row is kept at (see the docstring's per-item rows).
ITEM_FLOOR_MS = 1000
KIND_ORDER = {"job": 0, "step": 1}
# The issue's label. helpers/needs-maintainer.jq's `nm_budget_label` and helpers/merge-drain.sh's
# `NM_BUDGET_LABEL` hold the same string; helpers/needs-maintainer-selftest.sh checks all three.
# The check sits there and not in this file's selftest (milestone 889 (the tree's gates stop
# depending on the fleet tooling)): this file's selftest runs in script/lint, which reads no fleet
# file, and the fleet's selftest may read a tree file.
LABEL = "near-budget"
HISTORY = 10
CLOSE_AFTER = 3
DEFAULT_WINDOW_DAYS = 2
MARKER = "<!-- ci-job-times:job=%s -->"
# The workflows the daily record reads. Wider than `helpers/job-budget.py`'s WORKFLOWS on purpose:
# `verify.yml` is outside milestone 721's budget until its medians exist (that milestone's first
# BUGS entry), and this record is where they come from. `sync` reads only the budgeted ones, since a
# job with no budget has no line to cross.
RECORDED = [".github/workflows/ci.yml", ".github/workflows/verify.yml"]
# One row per green merge-group build and one per job in it, each job's start and end in seconds
# from the build's first job start (`build_rows`). `script/metrics` reads it into `ci-jobs.svg`
# and `ci-critical-path.svg`. File name provisional, minted 2026-10-11 (UTC) by
# maintainer/build-time-chart.
BUILDS_CSV = "notes/project-metrics/ci-builds.csv"
BUILD_COLUMNS = ["date", "build", "kind", "workflow", "job", "start_seconds", "end_seconds", "seconds",
                 "builds", "p90_seconds"]
# Weeks of per-job rows kept (this one and the twelve before it); older weeks fold to one row per
# week and job (the docstring's fold section). Ruled 2026-11-01 (UTC).
FOLD_AFTER_WEEKS = 13
# The workflow every build has. A merge group without it was never built.
ANCHOR = "ci.yml"


def _load_budget_module():
    """helpers/job-budget.py, whose file name has a hyphen and so cannot be imported by name. The
    budget, the warning line and the job parser come from there, so the two cannot disagree."""
    here = os.path.dirname(os.path.abspath(__file__))
    spec = importlib.util.spec_from_file_location("job_budget", os.path.join(here, "job-budget.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


jb = _load_budget_module()
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import time_record  # noqa: E402  the record's parser and the artifact rule, from one place
WARN_FRACTION = jb.WARN_FRACTION


# ---------------------------------------------------------------------------------------------
# Display names to job keys.

def display_patterns(workflow_text):
    """[(job key, compiled regex over the display name)] for a workflow file.

    A job's display name is its `name:`, or its key when it has none. A matrix job's name carries
    `${{ matrix.x }}`, which GitHub fills per variant; that part becomes a capture group, and what it
    matched is the row's `variant`."""
    out = []
    for key, body in jb.jobs_of(workflow_text).items():
        name = key
        for line in body:
            m = re.match(r"^    name:\s*(.+?)\s*$", line)
            if m:
                name = m.group(1).strip("'\"")
                break
        parts = re.split(r"\$\{\{.*?\}\}", name)
        pattern = "(.*)".join(re.escape(p) for p in parts)
        out.append((key, re.compile("^%s$" % pattern)))
    return out


def patterns_across(versions):
    """display_patterns over every version of a workflow, newest first, so a backfilled run is read
    by the names its jobs had when it ran. The 2026-09-30 split (a6fdf1093) renamed `test`'s display
    name from `build + test (host + QEMU)`, and a backfill matched only against the current file
    dropped every `test` run before it. The newest version's patterns come first, so it wins where
    two disagree; an older version adds only a pattern the newer ones lack."""
    out, seen = [], set()
    for text in versions:
        for key, rx in display_patterns(text):
            if (key, rx.pattern) not in seen:
                seen.add((key, rx.pattern))
                out.append((key, rx))
    return out


def workflow_versions(path):
    """The workflow file's text now and at each commit that changed it, newest first. A shallow
    checkout has only the commits it fetched, which degrades to the current file alone."""
    try:
        shas = subprocess.run(["git", "log", "--format=%H", "--", path], capture_output=True,
                              text=True, check=True).stdout.split()
    except (OSError, subprocess.CalledProcessError):
        shas = []
    texts = [open(path).read()]
    for sha in shas:
        done = subprocess.run(["git", "show", "%s:%s" % (sha, path)], capture_output=True, text=True)
        if done.returncode == 0:
            texts.append(done.stdout)
    return texts


def job_key(display, patterns):
    """(key, variant) for a display name, or (None, "") for a job this file does not have."""
    for key, rx in patterns:
        m = rx.match(display)
        if m:
            return key, " ".join(g for g in m.groups() if g)
    return None, ""


# ---------------------------------------------------------------------------------------------
# Durations, pure over the API's objects.

def _ts(s):
    return datetime.datetime.fromisoformat(s.replace("Z", "+00:00"))


def seconds(obj):
    """Wall seconds of a job or a step, or None when it did not run (skipped, cancelled, or never
    started)."""
    if obj.get("conclusion") not in ("success", "failure"):
        return None
    if not obj.get("started_at") or not obj.get("completed_at"):
        return None
    return max(0.0, (_ts(obj["completed_at"]) - _ts(obj["started_at"])).total_seconds())


def run_durations(jobs, patterns):
    """{(key, variant): (job seconds, {step name: seconds})} for one run's jobs."""
    out = {}
    for job in jobs:
        s = seconds(job)
        if s is None:
            continue
        key, variant = job_key(job["name"], patterns)
        if key is None:
            continue
        steps = {}
        for st in job.get("steps") or []:
            t = seconds(st)
            if t is not None:
                steps[st["name"]] = steps.get(st["name"], 0.0) + t
        out[(key, variant)] = (s, steps)
    return out


def day_rows(date, workflow, per_run):
    """The CSV rows for one day: medians and maxima over the day's runs, per job and per step."""
    jobs, steps = {}, {}
    for durations in per_run:
        for (key, variant), (s, st) in durations.items():
            jobs.setdefault((key, variant), []).append(s)
            for name, t in st.items():
                steps.setdefault((key, variant, name), []).append(t)
    rows = []
    for (key, variant), v in sorted(jobs.items()):
        rows.append(_row(date, workflow, key, variant, "job", "", v))
    for (key, variant, name), v in sorted(steps.items()):
        rows.append(_row(date, workflow, key, variant, "step", name, v))
    return rows


def _row(date, workflow, key, variant, kind, name, values, fmt="%.0f"):
    return {"date": date, "workflow": workflow, "job": key, "variant": variant, "kind": kind,
            "name": name, "runs": len(values), "median_seconds": fmt % statistics.median(values),
            "max_seconds": fmt % max(values)}


def record_job(artifact, uploaders):
    """The job key an artifact named `time-record-<job>[-<matrix value>]` came from, or None. The
    longest uploading job wins, so `swish-check-x86_64` is not read as `swish-check`. Only jobs
    that upload today count, so an artifact from before a rename maps to nothing rather than to
    the wrong job."""
    if not artifact.startswith(time_record.ARTIFACT):
        return None
    rest = artifact[len(time_record.ARTIFACT):]
    fits = [j for j in uploaders if rest == j or rest.startswith(j + "-")]
    return max(fits, key=len) if fits else None


def uploaders_of(workflow_text):
    """The jobs of a workflow that upload their time record (helpers/time_record.py's rule)."""
    return {job for job, body in jb.jobs_of(workflow_text).items()
            if time_record.uploads_record(job, jb.steps_of(body) or [])}


def record_day_rows(date, workflow, per_run, uploaders):
    """The per-item rows for one day. `per_run` holds, for each green run, a list of
    `(artifact name, record rows)`."""
    values = {}
    for artifacts in per_run:
        for artifact, rows in artifacts:
            key = record_job(artifact, uploaders)
            if key is None:
                continue
            for r in rows:
                k = (key, "%s / %s" % (r["leg"], r["image"]), r["kind"], r["name"])
                values.setdefault(k, []).append(r["milliseconds"] / 1000.0)
    out = []
    for (key, variant, kind, name), v in sorted(values.items()):
        if kind != "phase" and statistics.median(v) * 1000 < ITEM_FLOOR_MS:
            continue
        out.append(_row(date, workflow, key, variant, kind, name, v, fmt="%.1f"))
    return out


def records_of(run_id):
    """`[(artifact name, record rows)]` for one run's unexpired `time-record-*` artifacts."""
    data = gh_json("repos/%s/actions/runs/%d/artifacts?per_page=100" % (REPO, run_id))
    out = []
    for a in data.get("artifacts", []):
        if a.get("expired") or not a["name"].startswith(time_record.ARTIFACT):
            continue
        done = subprocess.run(["gh", "api", "repos/%s/actions/artifacts/%d/zip" % (REPO, a["id"])],
                              capture_output=True)
        if done.returncode:
            print("ci_job_times: artifact %s of run %d: %s" % (a["name"], run_id, done.stderr.decode().strip()),
                  file=sys.stderr)
            continue
        with zipfile.ZipFile(io.BytesIO(done.stdout)) as z:
            for member in z.namelist():
                if member.endswith(".tsv"):
                    out.append((a["name"], time_record.parse(z.read(member).decode())))
    return out


def to_csv(rows):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=COLUMNS, lineterminator="\n")
    w.writeheader()
    for r in rows:
        w.writerow({c: r.get(c, "") for c in COLUMNS})
    return buf.getvalue()


def from_csv(text):
    return list(csv.DictReader(io.StringIO(text)))


def merge_days(existing, fresh):
    """Every row of a day `fresh` covers is replaced by `fresh`'s; other days are kept."""
    days = {(r["date"], r["workflow"]) for r in fresh}
    kept = [r for r in existing if (r["date"], r["workflow"]) not in days]
    return sorted(kept + list(fresh), key=_times_order)


# ---------------------------------------------------------------------------------------------
# The fold: weeks older than FOLD_AFTER_WEEKS become one row per week and job.

FOLDED = re.compile(r"^\d{4}W\d{2}$")


def fold_cutoff(today):
    """The Monday of the oldest week that keeps per-job rows: a day before it is folded."""
    monday = today - datetime.timedelta(days=today.weekday())
    return monday - datetime.timedelta(weeks=FOLD_AFTER_WEEKS - 1)


def _when(date):
    """A row's `date` as a sortable ISO day: a folded week sorts at its Monday."""
    if FOLDED.match(date):
        return datetime.date.fromisocalendar(int(date[:4]), int(date[5:]), 1).isoformat()
    return date


def _needs_fold(date, cutoff):
    return not FOLDED.match(date) and _when(date) < cutoff.isoformat()


def weighted_median(pairs):
    """The lowest value at which the weights of the values at or under it reach half the total."""
    pairs = sorted(pairs)
    half = sum(w for _v, w in pairs) / 2.0
    seen = 0
    for v, w in pairs:
        seen += w
        if seen >= half:
            return v


def fold_times(rows, today):
    """`rows` with every day before `fold_cutoff(today)` replaced by one row per week, workflow,
    job, variant, kind and name: runs summed, the days' medians weighted by their runs, the largest
    maximum. A row already folded, and every row of a kept week, passes through."""
    cutoff = fold_cutoff(today)
    keep, held = [], {}
    for r in rows:
        if _needs_fold(r["date"], cutoff):
            key = (week_of(datetime.date.fromisoformat(r["date"])), r["workflow"], r["job"],
                   r["variant"], r["kind"], r["name"])
            held.setdefault(key, []).append(r)
        else:
            keep.append(r)
    for (week, workflow, job, variant, kind, name), days in held.items():
        fmt = "%.1f" if any("." in d["median_seconds"] + d["max_seconds"] for d in days) else "%.0f"
        keep.append({"date": week, "workflow": workflow, "job": job, "variant": variant, "kind": kind,
                     "name": name, "runs": sum(int(d["runs"]) for d in days),
                     "median_seconds": fmt % weighted_median([(float(d["median_seconds"]), int(d["runs"]))
                                                              for d in days]),
                     "max_seconds": fmt % max(float(d["max_seconds"]) for d in days)})
    return sorted(keep, key=_times_order)


def _times_order(r):
    return (_when(r["date"]), r["workflow"], r["job"], r["variant"], KIND_ORDER.get(r["kind"], 2),
            r["kind"], r["name"])


def p90(values):
    """The 90th percentile by linear interpolation between closest ranks, so one build is its own
    p90 and the figure never leaves the range measured. `script/metrics` draws the same figure."""
    if len(values) < 2:
        return values[0]
    return statistics.quantiles(values, n=10, method="inclusive")[8]


def fold_builds(rows, today):
    """`rows` with every build before `fold_cutoff(today)` replaced by, per week, one `build` row
    (count, median and p90 wall time) and one `job` row per workflow and job (median start, end and
    length over the builds that ran it). Rows already folded, and every build of a kept week, pass
    through."""
    cutoff = fold_cutoff(today)
    keep, weeks = [], {}
    for r in rows:
        if not _needs_fold(r["date"], cutoff):
            keep.append(r)
            continue
        week = weeks.setdefault(week_of(datetime.date.fromisoformat(r["date"])), {"walls": {}, "jobs": {}})
        build = (r["date"], r["build"])
        if r["kind"] == "build":
            week["walls"][build] = float(r["seconds"])
        else:
            week["jobs"].setdefault((r["workflow"], r["job"]), []).append(
                (float(r["start_seconds"]), float(r["end_seconds"]), float(r["seconds"])))
    for week, held in weeks.items():
        walls = list(held["walls"].values())
        if walls:
            keep.append({"date": week, "build": "", "kind": "build", "workflow": "", "job": "",
                         "start_seconds": 0, "end_seconds": "%.2f" % statistics.median(walls),
                         "seconds": "%.2f" % statistics.median(walls), "builds": len(walls),
                         "p90_seconds": "%.2f" % p90(walls)})
        for (workflow, job), v in held["jobs"].items():
            keep.append({"date": week, "build": "", "kind": "job", "workflow": workflow, "job": job,
                         "start_seconds": "%.2f" % statistics.median(x[0] for x in v),
                         "end_seconds": "%.2f" % statistics.median(x[1] for x in v),
                         "seconds": "%.2f" % statistics.median(x[2] for x in v), "builds": len(v),
                         "p90_seconds": ""})
    return sorted(keep, key=_build_order)


# ---------------------------------------------------------------------------------------------
# Builds: one merge group's runs of every RECORDED workflow, as one timeline.

def build_jobs(runs, patterns):
    """{(workflow, key): (start, end, seconds)} over one merge group's runs, `runs` being
    {workflow: jobs} and `patterns` {workflow: display patterns}. A matrix job is one entry: its
    earliest variant's start, its latest variant's end, and its slowest variant's seconds, which is
    the figure `history` and the budget read."""
    out = {}
    for workflow, jobs in runs.items():
        for job in jobs:
            s = seconds(job)
            if s is None:
                continue
            key, _variant = job_key(job["name"], patterns[workflow])
            if key is None:
                continue
            start, end = _ts(job["started_at"]), _ts(job["completed_at"])
            if (workflow, key) in out:
                a, b, t = out[(workflow, key)]
                start, end, s = min(a, start), max(b, end), max(t, s)
            out[(workflow, key)] = (start, end, s)
    return out


def build_wall(jobs):
    """A build's wall seconds: its first job's start to its last job's end, across every workflow.
    Not the slowest job, which a `needs:` chain and a queued runner both exceed, and not one
    workflow's, since the merge waits for the later of the two."""
    if not jobs:
        return None
    return (max(e for _s, e, _t in jobs.values()) - min(s for s, _e, _t in jobs.values())).total_seconds()


def build_rows(date, runs, patterns):
    """The BUILDS_CSV rows for one day. `runs` is {workflow: [(run, jobs)]}, newest first.

    A build is the runs that share a `head_sha`: the merge queue builds each entry's merge commit,
    and every workflow it triggers reports that commit, so the match is exact. Only a build whose
    every run succeeded counts, since a failed one is ejected and its time answers another question.
    A merge group with no `ci.yml` run is not a build (`ANCHOR`); one with no `verify.yml` run is,
    since verify can be absent where ci never is."""
    groups = {}
    for workflow, items in runs.items():
        for run, jobs in items:
            # Newest first, so a re-queued commit keeps its latest run.
            groups.setdefault(run["head_sha"], {}).setdefault(workflow, (run, jobs))
    rows = []
    for sha, held in groups.items():
        if ANCHOR not in held or any(run.get("conclusion") != "success" for run, _j in held.values()):
            continue
        jobs = build_jobs({w: j for w, (_r, j) in held.items()}, patterns)
        wall = build_wall(jobs)
        if wall is None:
            continue
        t0 = min(s for s, _e, _t in jobs.values())
        build = sha[:12]
        rows.append({"date": date, "build": build, "kind": "build", "workflow": "", "job": "",
                     "start_seconds": 0, "end_seconds": "%.0f" % wall, "seconds": "%.0f" % wall})
        for (workflow, key), (s, e, t) in jobs.items():
            rows.append({"date": date, "build": build, "kind": "job", "workflow": workflow, "job": key,
                         "start_seconds": "%.0f" % (s - t0).total_seconds(),
                         "end_seconds": "%.0f" % (e - t0).total_seconds(), "seconds": "%.0f" % t})
    return sorted(rows, key=_build_order)


def _build_order(r):
    return (_when(r["date"]), r["build"], r["kind"] != "build", r["workflow"], r["job"])


def builds_to_csv(rows):
    # The two columns a folded week needs are written only once a week is folded, so a file with
    # nothing folded keeps the eight columns it always had.
    columns = BUILD_COLUMNS if any(FOLDED.match(r["date"]) for r in rows) else BUILD_COLUMNS[:8]
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=columns, lineterminator="\n")
    w.writeheader()
    for r in rows:
        w.writerow({c: r.get(c, "") for c in columns})
    return buf.getvalue()


def merge_build_days(existing, fresh_days, fresh):
    """Every row of a day in `fresh_days` is replaced by `fresh`'s, so a day with no green build
    still clears; other days are kept."""
    kept = [r for r in existing if r["date"] not in fresh_days]
    return sorted(kept + list(fresh), key=_build_order)


# ---------------------------------------------------------------------------------------------
# The tracking issue's decision, pure over runs and issues.

def history(per_run):
    """{key: [(run id, created, seconds, variant), ...]} newest first, one entry per run: a matrix
    job's slowest variant, since that is the pole the budget binds."""
    out = {}
    for run_id, created, durations in per_run:
        best = {}
        for (key, variant), (s, _steps) in durations.items():
            if key not in best or s > best[key][0]:
                best[key] = (s, variant)
        for key, (s, variant) in best.items():
            out.setdefault(key, []).append((run_id, created, s, variant))
    for key in out:
        out[key].sort(key=lambda e: e[1], reverse=True)
        out[key] = out[key][:HISTORY]
    return out


def decide(hist, open_issues, budgets):
    """The actions for one pass: [(action, job, detail)].

    `open_issues` is {job: issue number}. Actions are `open` (no issue, newest run at or past the
    line), `update` (an issue is open and stays open), and `close` (an issue is open and the newest
    CLOSE_AFTER runs are all under the line)."""
    actions = []
    for key in sorted(set(hist) | set(open_issues)):
        runs = hist.get(key, [])
        budget = jb.budget_for(key, budgets) * 60.0
        line = budget * WARN_FRACTION
        over = [s >= line for (_r, _c, s, _v) in runs]
        issue = open_issues.get(key)
        if issue is None:
            if over and over[0]:
                actions.append(("open", key, runs[0][2] / budget))
        elif len(over) >= CLOSE_AFTER and not any(over[:CLOSE_AFTER]):
            actions.append(("close", key, issue))
        else:
            actions.append(("update", key, issue))
    return actions


def issue_title(key):
    return "CI job `%s` is near its budget" % key


def issue_body(key, runs, budgets, slowest_steps):
    budget = jb.budget_for(key, budgets)
    lines = [
        MARKER % key,
        "Opened by `helpers/ci_job_times.py sync` (milestone 808 (every gate accounts for its time), "
        "§254 (a gate prints what each item cost)). The job `%s` ran past %d%% of its %d-minute budget "
        "in `.github/ci-job-budgets` on a merge-group run. Nothing has failed: the hard limit is the "
        "budget itself. This issue is rewritten on each daily pass, and closes itself when %d runs in "
        "a row are back under the line."
        % (key, round(WARN_FRACTION * 100), budget, CLOSE_AFTER),
        "",
        "The ways out: make something faster, split the job, or raise the budget with a reason line in "
        "`.github/ci-job-budgets`, which moves this line with it.",
        "",
        "| run | started (UTC) | minutes | share of budget | variant |",
        "|---|---|---:|---:|---|",
    ]
    for run_id, created, s, variant in runs:
        lines.append("| [%s](https://github.com/%s/actions/runs/%s) | %s | %.1f | %.0f%% | %s |"
                     % (run_id, REPO, run_id, created[:16].replace("T", " "), s / 60.0,
                        100.0 * s / (budget * 60.0), variant))
    if slowest_steps:
        lines += ["", "The newest run's slowest steps:", "", "| step | minutes |", "|---|---:|"]
        lines += ["| %s | %.1f |" % (n, t / 60.0) for n, t in slowest_steps]
    lines += ["", "A job that runs the kernel suite also uploads its per-test record as the run's "
              "`time-record-*` artifact (milestone 807 (the kernel suite reports what each test cost))."]
    return "\n".join(lines) + "\n"


def week_of(date):
    y, w, _ = date.isocalendar()
    return "%dW%02d" % (y, w)


def week_note_row(week, key, share, issue):
    """The week-notes.csv line for an open budget issue (milestone 623's bullet)."""
    note = "CI job %s ran at %.0f%% of its budget on its newest merge-group run; tracking issue #%s" % (
        key, share * 100, issue)
    buf = io.StringIO()
    csv.writer(buf, lineterminator="\n").writerow([week, CHART, note])
    return buf.getvalue()


def has_week_note(text, week, issue):
    for row in csv.reader(io.StringIO(text)):
        if len(row) == 3 and row[0] == week and row[1] == CHART and row[2].endswith("#%s" % issue):
            return True
    return False


# ---------------------------------------------------------------------------------------------
# GitHub.

def gh_json(path, tries=3):
    for attempt in range(tries):
        done = subprocess.run(["gh", "api", path], capture_output=True, text=True)
        if not done.returncode:
            return json.loads(done.stdout)
        if attempt + 1 < tries:
            time.sleep(5 * (attempt + 1))
    raise RuntimeError("gh api %s: %s" % (path, done.stderr.strip()))


def gh(*args):
    done = subprocess.run(["gh"] + list(args), capture_output=True, text=True)
    if done.returncode:
        raise RuntimeError("gh %s: %s" % (" ".join(args[:3]), done.stderr.strip()))
    return done.stdout


# Workflow-run listings are fetched 50 to a page rather than the API's maximum of 100. The
# metrics workflow on 2026-10-12 got HTTP 502 from a 100-run page on three retries running,
# for a query that succeeded at once from a laptop; a smaller page asks less of the server.
RUNS_PAGE = 50


def merge_group_runs(workflow, created=None, limit=None, event="merge_group"):
    """Completed runs of `workflow` for `event`, newest first, not cancelled.

    A crate repository has no merge queue, so its `--repo` record reads `pull_request` runs of
    its two caller workflows instead; the caller files' names (ci.yml, proofs.yml) are the
    scaffold's, and a caller that renames them reads as a repository with no runs rather than as
    a silent zero.
    """
    out, page = [], 1
    q = "event=%s&status=completed&per_page=%d" % (event, RUNS_PAGE)
    if created:
        q += "&created=%s" % created
    while True:
        data = gh_json("repos/%s/actions/workflows/%s/runs?%s&page=%d"
                       % (REPO, os.path.basename(workflow), q, page))
        out += [r for r in data["workflow_runs"] if r.get("conclusion") != "cancelled"]
        if len(data["workflow_runs"]) < RUNS_PAGE or (limit and len(out) >= limit):
            return out[:limit] if limit else out
        page += 1


def jobs_of(run_id):
    jobs, page = [], 1
    while True:
        data = gh_json("repos/%s/actions/runs/%d/jobs?per_page=100&page=%d" % (REPO, run_id, page))
        jobs += data["jobs"]
        if len(data["jobs"]) < 100:
            return jobs
        page += 1


def ratchet():
    try:
        with open(jb.RATCHET) as f:
            return jb.parse_ratchet(f.read())[0]
    except FileNotFoundError:
        return {}


def arg_value(argv, flag, default):
    return argv[argv.index(flag) + 1] if flag in argv and argv.index(flag) + 1 < len(argv) else default


def update(argv):
    # Milestone 887: `--repo` points the same record at one crate repository, so a slow template
    # job shows up in this tree's metrics rather than in nine places nobody watches. The runs are
    # the caller's pull-request runs of its two caller workflows (there is no merge queue), the
    # CSV is a per-repository file beside this one, and the builds record is skipped: a build's
    # wall time is a merge-group idea, and a crate repository's pull-request wall time says more
    # about queueing than about the template. `sync` stays nife-only: the near-budget issues live
    # here, where the budgets and the reader are.
    global REPO, CSV, BUILDS_CSV, RECORDED
    event = "merge_group"
    if "--repo" in argv:
        REPO = arg_value(argv, "--repo", "")
        if "/" not in REPO:
            print("ci_job_times: --repo wants owner/name, got %r" % REPO, file=sys.stderr)
            return 2
        CSV = "notes/project-metrics/ci-job-times-%s.csv" % REPO.split("/")[1]
        BUILDS_CSV = None
        RECORDED = [".github/workflows/ci.yml", ".github/workflows/proofs.yml"]
        event = "pull_request"
    today = datetime.datetime.now(datetime.timezone.utc).date()
    since = datetime.date.fromisoformat(
        arg_value(argv, "--since", (today - datetime.timedelta(days=DEFAULT_WINDOW_DAYS)).isoformat()))
    patterns = {os.path.basename(w): patterns_across(workflow_versions(w)) for w in RECORDED}
    # A week that has folded is not read again: its rows are one per job already, and a day re-read
    # into it would put a per-day row beside the week's.
    day = max(since, fold_cutoff(today))
    if day > since:
        print("ci_job_times: starting at %s, the oldest day not yet folded" % day, file=sys.stderr)
    while day <= today:
        # Each run's jobs are read once, for both records: the per-job day rows and the builds.
        runs = {}
        for workflow in RECORDED:
            name = os.path.basename(workflow)
            listed = merge_group_runs(workflow, created=day.isoformat(), event=event)
            # Eight at a time: a busy day is sixty runs per workflow, and one request at a time
            # made a 57-day backfill take hours.
            with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
                runs[name] = list(zip(listed, pool.map(lambda r: jobs_of(r["id"]), listed)))
            fresh = day_rows(day.isoformat(), name, [run_durations(j, patterns[name]) for _r, j in runs[name]])
            if BUILDS_CSV:
                green = [r["id"] for r, _j in runs[name] if r.get("conclusion") == "success"]
                with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
                    records = list(pool.map(records_of, green))
                fresh += record_day_rows(day.isoformat(), name, records, uploaders_of(open(workflow).read()))
            existing = from_csv(open(CSV).read()) if os.path.exists(CSV) else []
            # Written a day at a time, so a failure part way keeps every day before it.
            with open(CSV, "w") as f:
                f.write(to_csv(fold_times(merge_days(existing, fresh), today)))
            print("ci_job_times: %s %s: %d run(s)" % (name, day, len(runs[name])), file=sys.stderr)
        if BUILDS_CSV:
            builds = build_rows(day.isoformat(), runs, patterns)
            existing = from_csv(open(BUILDS_CSV).read()) if os.path.exists(BUILDS_CSV) else []
            with open(BUILDS_CSV, "w") as f:
                f.write(builds_to_csv(fold_builds(merge_build_days(existing, {day.isoformat()}, builds), today)))
            print("ci_job_times: %s: %d green build(s)" % (day, sum(r["kind"] == "build" for r in builds)),
                  file=sys.stderr)
        day += datetime.timedelta(days=1)
    return 0


def fold(argv):
    """Fold the record files in place: `CSV`, `BUILDS_CSV` and each crate repository's
    `ci-job-times-<crate>.csv`. `update` does this as it appends; this is for the files as they stood
    when the fold was ruled, and does nothing to a file with no week to fold."""
    today = datetime.date.fromisoformat(
        arg_value(argv, "--today", datetime.datetime.now(datetime.timezone.utc).date().isoformat()))
    pattern = os.path.join(os.path.dirname(CSV), "ci-job-times-*.csv")
    for path, folder, writer in ([(CSV, fold_times, to_csv), (BUILDS_CSV, fold_builds, builds_to_csv)]
                                 + [(p, fold_times, to_csv) for p in sorted(glob.glob(pattern))]):
        if os.path.exists(path):
            with open(path) as f:
                rows = from_csv(f.read())
            with open(path, "w") as f:
                f.write(writer(folder(rows, today)))
            print("ci_job_times: %s: %d row(s) before, %d after" % (path, len(rows), len(folder(rows, today))),
                  file=sys.stderr)
    return 0


def open_budget_issues():
    """{job: issue number} for the open issues carrying LABEL and this script's marker."""
    data = gh_json("repos/%s/issues?labels=%s&state=open&per_page=100" % (REPO, LABEL))
    out = {}
    for issue in data:
        m = re.search(r"<!-- ci-job-times:job=([A-Za-z0-9_-]+) -->", issue.get("body") or "")
        if m and "pull_request" not in issue:
            out[m.group(1)] = issue["number"]
    return out


def sync(argv):
    dry = "--dry-run" in argv
    budgets = ratchet()
    per_run, steps_of_run = [], {}
    for workflow in jb.WORKFLOWS:
        patterns = display_patterns(open(workflow).read())
        for r in merge_group_runs(workflow, limit=HISTORY):
            d = run_durations(jobs_of(r["id"]), patterns)
            per_run.append((r["id"], r["created_at"], d))
            steps_of_run[r["id"]] = d
    hist = history(per_run)
    shares = sorted(((runs[0][2] / (jb.budget_for(k, budgets) * 60.0), k) for k, runs in hist.items() if runs),
                    reverse=True)
    for share, key in shares[:5]:
        print("ci_job_times: %s's newest merge-group run took %.0f%% of its budget" % (key, share * 100))
    issues = open_budget_issues()
    week = week_of(datetime.datetime.now(datetime.timezone.utc).date())
    notes = open(WEEK_NOTES).read() if os.path.exists(WEEK_NOTES) else ""
    for action, key, detail in decide(hist, issues, budgets):
        runs = hist.get(key, [])
        slowest = []
        if runs:
            newest = runs[0]
            st = steps_of_run.get(newest[0], {}).get((key, newest[3]), (0, {}))[1]
            slowest = sorted(st.items(), key=lambda kv: -kv[1])[:5]
        body = issue_body(key, runs, budgets, slowest)
        share = runs[0][2] / (jb.budget_for(key, budgets) * 60.0) if runs else 0.0
        if dry:
            print("ci_job_times: (dry run) would %s for %s (%s)" % (action, key, detail))
            continue
        if action == "open":
            url = gh("issue", "create", "--repo", REPO, "--title", issue_title(key), "--label", LABEL,
                     "--body", body).strip()
            number = url.rsplit("/", 1)[-1]
            print("ci_job_times: OPENED #%s for %s at %.0f%%" % (number, key, share * 100))
        elif action == "update":
            number = str(detail)
            gh("issue", "edit", number, "--repo", REPO, "--body", body)
            print("ci_job_times: UPDATED #%s for %s at %.0f%%" % (number, key, share * 100))
        else:
            gh("issue", "close", str(detail), "--repo", REPO, "--comment",
               "Closed by `helpers/ci_job_times.py sync`: the newest %d merge-group runs of `%s` were all "
               "under %d%% of its budget." % (CLOSE_AFTER, key, round(WARN_FRACTION * 100)))
            print("ci_job_times: CLOSED #%s for %s" % (detail, key))
            continue
        if not has_week_note(notes, week, number):
            row = week_note_row(week, key, share, number)
            with open(WEEK_NOTES, "a") as f:
                f.write(row)
            notes += row
    return 0


# ---------------------------------------------------------------------------------------------

FIXTURE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ci-job-times-fixtures",
                       "run-37648588582.json")

WORKFLOW_FIXTURE = """\
name: CI
jobs:
  test:
    name: test (host crates and the kernel suite under QEMU)
    timeout-minutes: 25
  cpu-matrix-shard:
    name: cpu matrix shard (${{ matrix.models }})
    timeout-minutes: 25
  rustfmt:
    timeout-minutes: 10
"""


def selftest():
    ok = True

    def expect(label, got, want):
        nonlocal ok
        if got != want:
            ok = False
            print("FAIL %s:\n  got  %r\n  want %r" % (label, got, want), file=sys.stderr)

    pats = display_patterns(WORKFLOW_FIXTURE)
    expect("plain name", job_key("test (host crates and the kernel suite under QEMU)", pats), ("test", ""))
    expect("matrix name", job_key("cpu matrix shard (rv64 sifive-u54 rva22s64)", pats),
           ("cpu-matrix-shard", "rv64 sifive-u54 rva22s64"))
    expect("key as name", job_key("rustfmt", pats), ("rustfmt", ""))
    expect("unknown", job_key("draft gate", pats), (None, ""))
    older = WORKFLOW_FIXTURE.replace("test (host crates and the kernel suite under QEMU)",
                                     "build + test (host + QEMU)")
    across = patterns_across([WORKFLOW_FIXTURE, older])
    expect("an old name still reads as its key", job_key("build + test (host + QEMU)", across), ("test", ""))
    expect("the current name still does", job_key("test (host crates and the kernel suite under QEMU)",
                                                   across), ("test", ""))
    expect("an unchanged name adds no second pattern", len(across), len(pats) + 1)

    # The recorded run: a real merge-group run's jobs, trimmed to the fields read.
    with open(FIXTURE) as f:
        recorded = json.load(f)
    d = run_durations(recorded["jobs"], pats)
    expect("recorded keys", sorted(d), [("cpu-matrix-shard", "rv64 sifive-u54 rva22s64"),
                                        ("cpu-matrix-shard", "rva23s64 thead-c906"), ("rustfmt", ""),
                                        ("test", "")])
    expect("recorded test seconds", d[("test", "")][0], 777.0)
    expect("the suite's own step is timed", round(d[("test", "")][1]["Run script/ci-build test"]) > 0, True)
    expect("the draft gate is no budgeted job", any(k[0] == "draft gate" for k in d), False)
    rows = day_rows("2026-10-07", "ci.yml", [d, d])
    expect("one job row per variant, then steps",
           [(r["job"], r["variant"], r["kind"]) for r in rows][:4],
           [("cpu-matrix-shard", "rv64 sifive-u54 rva22s64", "job"),
            ("cpu-matrix-shard", "rva23s64 thead-c906", "job"), ("rustfmt", "", "job"), ("test", "", "job")])
    expect("csv round trip", from_csv(to_csv(rows)), [{k: str(v) for k, v in r.items()} for r in rows])
    merged = merge_days(from_csv(to_csv(rows)), [dict(rows[0], date="2026-10-08")])
    expect("a new day is added, an old one kept", sorted({r["date"] for r in merged}),
           ["2026-10-07", "2026-10-08"])

    # Per-item rows (exit criterion 1's per-test times), from two runs' records.
    up = {"test-shard", "swish-check", "swish-check-x86_64"}
    expect("an artifact maps to its job", record_job("time-record-test-shard-test-x86_64", up), "test-shard")
    expect("the longest job wins", record_job("time-record-swish-check-x86_64", up), "swish-check-x86_64")
    expect("a name from before the rename maps to nothing", record_job("time-record-test-x86_64", up), None)
    rec = lambda suite, slow, fast: [
        {"leg": "riscv64", "image": "system_tests", "kind": "phase", "name": "suite", "milliseconds": suite},
        {"leg": "riscv64", "image": "system_tests", "kind": "test", "name": "a::slow", "milliseconds": slow},
        {"leg": "riscv64", "image": "system_tests", "kind": "test", "name": "a::fast", "milliseconds": fast}]
    per_run = [[("time-record-test-shard-test-aarch64-riscv64", rec(120000, 9000, 40))],
               [("time-record-test-shard-test-aarch64-riscv64", rec(124000, 10000, 60)),
                ("time-record-test-x86_64", rec(1, 99999, 1))]]
    items = record_day_rows("2026-10-19", "ci.yml", per_run, up)
    expect("a phase and a slow test are kept, a fast test and an old artifact are not",
           [(r["job"], r["variant"], r["kind"], r["name"], r["runs"], r["median_seconds"]) for r in items],
           [("test-shard", "riscv64 / system_tests", "phase", "suite", 2, "122.0"),
            ("test-shard", "riscv64 / system_tests", "test", "a::slow", 2, "9.5")])
    expect("item rows sort after the job's steps",
           [r["kind"] for r in merge_days([], items + [dict(items[0], kind="step", name="Set up job"),
                                                       dict(items[0], kind="job", name="")])],
           ["job", "step", "phase", "test"])
    wf = ("jobs:\n  quick:\n    steps:\n      - uses: actions/checkout@v5\n"
          "      - uses: actions/upload-artifact@v7\n        with:\n          name: time-record-quick\n")
    expect("uploaders are read from the workflow", uploaders_of(wf), {"quick"})

    # The three transitions (exit criterion 5), driven by the recorded run with its test job's
    # duration rewritten. The budget is 20 minutes, the line 17.
    def runs_at(minutes_newest_first):
        out = []
        for i, m in enumerate(minutes_newest_first):
            job = dict(next(j for j in recorded["jobs"] if j["name"].startswith("test ")))
            t0 = _ts(job["started_at"])
            job["completed_at"] = (t0 + datetime.timedelta(minutes=m)).strftime("%Y-%m-%dT%H:%M:%SZ")
            out.append((1000 - i, "2026-10-%02dT12:00:00Z" % (20 - i), run_durations([job], pats)))
        return history(out)

    expect("quiet below the line opens nothing", decide(runs_at([16.9, 16, 15]), {}, {}), [])
    expect("crossing opens", [a[:2] for a in decide(runs_at([17.2, 16, 15]), {}, {})], [("open", "test")])
    expect("a later crossing updates the same issue",
           decide(runs_at([18, 17.5, 16]), {"test": 77}, {}), [("update", "test", 77)])
    expect("one run under does not close", decide(runs_at([16, 18, 17.5]), {"test": 77}, {}),
           [("update", "test", 77)])
    expect("three runs under close", decide(runs_at([16, 15, 16.5, 18]), {"test": 77}, {}),
           [("close", "test", 77)])
    expect("a raised budget moves the line", decide(runs_at([18, 16, 15]), {}, {"test": 24}), [])

    body = issue_body("test", runs_at([18, 16])["test"], {}, [("Run script/ci-build test", 600.0)])
    expect("the body carries the marker the next pass finds", body.startswith(MARKER % "test"), True)
    expect("the body names the share", "| 90% |" in body, True)

    row = week_note_row("2026W41", "test", 0.9, 77)
    expect("week note", row, '2026W41,ci-jobs.svg,CI job test ran at 90% of its budget on its newest '
                             'merge-group run; tracking issue #77\n')
    expect("week note found", has_week_note("week,chart,note\n" + row, "2026W41", "77"), True)
    expect("week note per week", has_week_note("week,chart,note\n" + row, "2026W42", "77"), False)

    # Builds. Two workflows, one merge commit; times are minutes past 12:00.
    def job(name, start, end, conclusion="success"):
        at = lambda m: "2026-10-07T12:%02d:%02dZ" % (int(m), round((m % 1) * 60))
        return {"name": name, "conclusion": conclusion, "started_at": at(start), "completed_at": at(end)}

    verify_pats = display_patterns("name: Verify\njobs:\n  gate:\n    name: draft gate\n  prove:\n"
                                   "    name: prove (shard ${{ matrix.shard }}/3)\n")
    chain_pats = display_patterns("name: CI\njobs:\n  gate:\n    name: draft gate\n  lint:\n"
                                  "    name: lint\n  clippy:\n    name: clippy\n  test:\n    name: test\n")
    bp = {"ci.yml": chain_pats, "verify.yml": verify_pats}
    ci_jobs = [job("draft gate", 0, 0.5), job("test", 1, 15), job("lint", 1, 6), job("clippy", 6.5, 12)]
    verify_long = [job("draft gate", 0.2, 0.6), job("prove (shard 1/3)", 1, 18),
                   job("prove (shard 2/3)", 1.5, 19), job("prove (shard 3/3)", 1, 9)]
    verify_skipped = [job("draft gate", 0.2, 0.6), job("prove (shard 1/3)", 0, 0, "skipped")]
    run = lambda wf, conclusion="success": {"head_sha": "a" * 40, "conclusion": conclusion, "id": 1}

    jobs = build_jobs({"ci.yml": ci_jobs, "verify.yml": verify_long}, bp)
    expect("the later workflow ends the build", build_wall(jobs), 19 * 60.0)
    expect("a matrix job spans its variants and counts its slowest",
           jobs[("verify.yml", "prove")][2], 17.5 * 60)
    expect("a skipped verify leaves ci's end", build_wall(
        build_jobs({"ci.yml": ci_jobs, "verify.yml": verify_skipped}, bp)), 15 * 60.0)
    chain = [job("draft gate", 0, 0.5), job("lint", 1, 6), job("clippy", 6.5, 17)]
    expect("a needs: chain is longer than its slowest job", build_wall(build_jobs({"ci.yml": chain}, bp)),
           17 * 60.0)
    rows = build_rows("2026-10-07", {"ci.yml": [(run("ci.yml"), ci_jobs)],
                                     "verify.yml": [(run("verify.yml"), verify_long)]}, bp)
    expect("one build row, then its jobs", [(r["kind"], r["workflow"], r["job"]) for r in rows][:3],
           [("build", "", ""), ("job", "ci.yml", "clippy"), ("job", "ci.yml", "gate")])
    expect("the build row holds the wall time", rows[0]["end_seconds"], "1140")
    expect("a start is an offset from the first job's",
           next(r["start_seconds"] for r in rows if r["job"] == "clippy"), "390")
    expect("a failed verify is no green build", build_rows("2026-10-07", {
        "ci.yml": [(run("ci.yml"), ci_jobs)], "verify.yml": [(run("verify.yml", "failure"), verify_long)]},
        bp), [])
    expect("verify alone is no build", build_rows("2026-10-07", {
        "verify.yml": [(run("verify.yml"), verify_long)]}, bp), [])
    merged = merge_build_days(rows, {"2026-10-07"}, [])
    expect("a re-read day with no green build clears", merged, [])

    # The fold (ruled 2026-11-01 UTC): per-job rows for the latest 13 weeks, one row per week and
    # job before that. Built from the recorded run, copied across 15 weeks of Mondays.
    today = datetime.date(2026, 11, 1)   # a Sunday, in 2026W44; the oldest kept week is W32
    expect("the oldest kept week starts on its Monday", fold_cutoff(today), datetime.date(2026, 8, 3))
    expect("a Monday is its own week", fold_cutoff(datetime.date(2026, 11, 2)), datetime.date(2026, 8, 10))
    days = [datetime.date(2026, 7, 6) + datetime.timedelta(days=i) for i in range(0, 119)]
    fold_rows = []
    for i, d in enumerate(days):
        slower = json.loads(json.dumps(recorded["jobs"]))
        for j in slower:
            if j["name"].startswith("test "):
                j["completed_at"] = (_ts(j["completed_at"]) + datetime.timedelta(seconds=10 * (i % 7))
                                     ).strftime("%Y-%m-%dT%H:%M:%SZ")
        fold_rows += day_rows(d.isoformat(), "ci.yml", [run_durations(slower, pats)])
    week_of_row = lambda r: r["date"] if "W" in r["date"] else week_of(datetime.date.fromisoformat(r["date"]))
    folded = fold_times(fold_rows, today)
    old = [r for r in folded if "W" in r["date"]]
    expect("every day before the cutoff is gone", any("-" in r["date"] and r["date"] < "2026-08-03"
                                                      for r in folded), False)
    expect("a kept week keeps its day rows",
           [r for r in folded if "-" in r["date"]] == sorted(
               (r for r in fold_rows if r["date"] >= "2026-08-03"), key=_times_order), True)
    expect("the folded weeks are the four before W32", sorted({r["date"] for r in old}),
           ["2026W28", "2026W29", "2026W30", "2026W31"])
    first = lambda rows, w: next(r for r in rows if r["date"] == w and r["kind"] == "job"
                                 and r["job"] == "test")
    daily_test = [r for r in fold_rows if r["kind"] == "job" and r["job"] == "test"
                  and week_of_row(r) == "2026W28"]
    expect("a week is one row per job", sum(1 for r in old if r["date"] == "2026W28" and r["kind"] == "job"
                                           and r["job"] == "test"), 1)
    expect("a week sums its runs", first(old, "2026W28")["runs"], sum(int(r["runs"]) for r in daily_test))
    expect("a week keeps its largest maximum", first(old, "2026W28")["max_seconds"],
           "%.0f" % max(float(r["max_seconds"]) for r in daily_test))
    expect("the fold is smaller", len(folded) < len(fold_rows), True)
    expect("folding twice changes nothing", fold_times(folded, today) == folded, True)
    expect("a later fold keeps what an earlier one made",
           [r for r in fold_times(folded, today + datetime.timedelta(weeks=1)) if r["date"] == "2026W28"]
           == [r for r in folded if r["date"] == "2026W28"], True)
    expect("a folded file round trips", from_csv(to_csv(folded)) == [
        {k: str(v) for k, v in r.items()} for r in folded], True)
    expect("a re-read day inside a folded week is not merged into it",
           len([r for r in merge_days(folded, []) if r["date"] == "2026W28"]),
           len([r for r in folded if r["date"] == "2026W28"]))
    expect("a weighted median follows the runs", weighted_median([(10.0, 1), (20.0, 5), (90.0, 1)]), 20.0)

    build_pats = {"ci.yml": pats}
    build_days = {}
    brows = []
    for i, d in enumerate(days):
        for n in range(1 + i % 3):
            run_n = {"head_sha": "%02d%02d" % (i, n) + "a" * 36, "conclusion": "success", "id": 1}
            slower = json.loads(json.dumps(recorded["jobs"]))
            for j in slower:
                if j["name"].startswith("test "):
                    j["completed_at"] = (_ts(j["completed_at"]) + datetime.timedelta(seconds=30 * n + i)
                                         ).strftime("%Y-%m-%dT%H:%M:%SZ")
            brows += build_rows(d.isoformat(), {"ci.yml": [(run_n, slower)]}, build_pats)
    expect("the recorded run makes builds", len([r for r in brows if r["kind"] == "build"]) > 100, True)
    fb = fold_builds(brows, today)
    fb_old = [r for r in fb if "W" in r["date"]]
    walls28 = [float(r["seconds"]) for r in brows if r["kind"] == "build"
               and week_of(datetime.date.fromisoformat(r["date"])) == "2026W28"]
    week_build = next(r for r in fb_old if r["date"] == "2026W28" and r["kind"] == "build")
    expect("a week counts its builds", week_build["builds"], len(walls28))
    expect("a week keeps the median wall", float(week_build["seconds"]), statistics.median(walls28))
    expect("a week keeps the p90 wall", float(week_build["p90_seconds"]), p90(walls28))
    expect("a week has one row per job", sorted(r["job"] for r in fb_old if r["date"] == "2026W28"
                                                and r["kind"] == "job"),
           sorted({r["job"] for r in brows if r["kind"] == "job"}))
    expect("no per-build row is left before the cutoff",
           [r for r in fb if "-" in r["date"] and r["date"] < "2026-08-03"], [])
    expect("a kept week keeps its builds", [r for r in fb if "-" in r["date"]] == sorted(
        (r for r in brows if r["date"] >= "2026-08-03"), key=_build_order), True)
    expect("folding builds twice changes nothing", fold_builds(fb, today) == fb, True)
    expect("a file with nothing folded keeps its eight columns",
           builds_to_csv(brows).splitlines()[0], ",".join(BUILD_COLUMNS[:8]))
    expect("a folded file has all ten", builds_to_csv(fb).splitlines()[0], ",".join(BUILD_COLUMNS))
    expect("a folded builds file round trips", from_csv(builds_to_csv(fb)) == [
        {c: str(r.get(c, "")) for c in BUILD_COLUMNS} for r in fb], True)
    expect("a re-read day outside the fold still replaces its own rows",
           merge_build_days(fb, {"2026-10-30"}, []) == [r for r in fb if r["date"] != "2026-10-30"], True)

    print("ci_job_times selftest: %s" % ("ok" if ok else "FAILED"))
    return 0 if ok else 1


def main(argv):
    if argv[1:] == ["--selftest"]:
        return selftest()
    cmd = argv[1] if len(argv) > 1 else ""
    if cmd == "update":
        return update(argv[2:])
    if cmd == "fold":
        return fold(argv[2:])
    if cmd == "sync":
        return sync(argv[2:])
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
