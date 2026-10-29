#!/usr/bin/env python3
"""How long the Kani proofs take in CI, per crate and per shard, and whether script/verify knows.

    python3 helpers/verify_times.py shard k/n < table          # script/verify's packer (crates on stdout)
    python3 helpers/verify_times.py shards                     # CI's prove shard count, from verify.yml
    python3 helpers/verify_times.py shard-check k/n SECONDS < table   # warn if a shard ran far off its estimate
    python3 helpers/verify_times.py crates [--runs N]          # median CI seconds per crate, as TSV
    python3 helpers/verify_times.py refresh [--runs N] [--dry-run]   # rewrite the table if it drifted
    python3 helpers/verify_times.py update [--since YYYY-MM-DD]       # per-run shard times for the metrics page
    python3 helpers/verify_times.py --selftest

Name: provisional, minted 2026-10-07 (UTC) by lane/verify-shard-refresh. A python helper under
`helpers/`, outside `script/names`' scope, so this paragraph is its provenance. Refused
`shard_balance`, which names one of its four jobs.

# WHY

Milestone 119 (the merge queue is the bottleneck, and the long pole is one prover) packs the `prove`
shards by a seconds-per-crate table in script/verify. Those seconds were read once, on 2026-08-14,
from one arm64 CI log, and five rows after that were dev-Mac numbers. By 2026-10-06 the shards had
moved to x86_64 (milestone 587 (most CI jobs do not need an arm64 host)) and `machine_discovery` had
grown from 180 seconds to 689, so the table planned 19.8 minutes for a shard that took 27, and
nothing said so: a stale row costs balance, never correctness, and so it failed in silence. calef approved a
refresh on 2026-10-07 and asked for a chart.

This file is the mechanism that keeps the table honest without anyone remembering to:

- `shard` is the packer itself, moved out of script/verify so that the run, the refresh and the
  check below all plan shards with one function rather than three that can disagree.
- `shard-check` runs at the end of every CI shard and compares what the shard took against what the
  table planned. **It warns and never fails** (see SHARD_WARN_PCT).
- `refresh` is what `.github/workflows/verify-shard-refresh.yml` runs weekly: medians from the
  last REFRESH_RUNS green runs, and a rewrite of the table's seconds when they drifted.
- `update` writes `notes/project-metrics/verify-runs.csv`, one row per sampled run. Nothing reads
  it since `verify-shards.svg` was retired on 2026-10-11 (UTC) for `ci-jobs.svg`, which draws every
  verify job; it stays as the raw per-shard record.

# WHERE THE SECONDS COME FROM

From job logs. script/verify prints `==> kani-seconds: <crate> <n>` after each crate (since
2026-10-07). A log older than that line is read from the GitHub timestamps on its `==> kani:`
lines instead, which is how the 2026-08-14 table was made and is accurate to the second: a crate's
time runs from its own `==> kani:` line to the next one, and the last crate's to its final
`Complete - ` line. Both measure wall clock at VERIFY_JOBS=2, compile included, which is the
quantity the packer needs (crates run one after another; harnesses inside one run two at a time).

# BUGS

- `update` samples at most PER_DAY runs a day rather than every run, to keep a daily fetch near a
  hundred API calls. The weekly figures are medians over the sample, not over every run.
- A run counts only if every `prove` shard in it succeeded. A shard killed by the memory limit or
  by a failing proof says nothing about how long a passing one takes, and would drag a median.
- Runner speed is not controlled. The same crate on the same commit varied by 30% across the 40
  runs of 2026-10-06 (glob: 455 to 801 seconds), which is why every decision here uses medians and
  thresholds well above that spread.
"""

import csv
import datetime
import io
import json
import os
import re
import statistics
import subprocess
import sys
import time

REPO = "nifeos/nife"
WORKFLOW = "verify.yml"
VERIFY = "script/verify"
VERIFY_YML = ".github/workflows/verify.yml"
REPLAY_TIMES = "notes/project-metrics/falsification-times.tsv"
RUNS_CSV = "notes/project-metrics/verify-runs.csv"
RUN_COLUMNS = ["run", "created", "event", "prove_minutes", "refalsify_minutes", "slowest_minutes"]

# **Warn above 30%, never fail.** Measured on 2026-10-06: one shard's proving time moves about 20%
# either side of its median from run to run on the same table (shard 2/2: 21.6 to 29.7 minutes), so
# a gate at 30% would still fire on noise often enough to make main red for nothing. And a wrong
# estimate is never wrong proofs: the packer asserts the partition, so the only cost is a slower
# shard. A warning on the run plus the weekly refresh below is the proportionate pair. 30% would
# have fired on the 2026-10-06 runs (planned 19.8, took about 27: +36%) and on none of the shards
# after this refresh.
SHARD_WARN_PCT = 30.0
# The weekly refresh rewrites the table when a planned shard is off its median by more than this,
# or when repacking by the fresh medians would shorten the slowest shard by REPACK_SAVING_S. Medians
# over REFRESH_RUNS runs are steadier than one run, so the line sits lower than the warning's.
REFRESH_PCT = 15.0
REPACK_SAVING_S = 60
REFRESH_RUNS = 20
# A replay time is rewritten only from a record seen in this many shard logs: one diff reaches a
# record once, and one sample is what the weekly sweep already gives.
REPLAY_MIN_SAMPLES = 3
PER_DAY = 12
DEFAULT_WINDOW_DAYS = 8

PROVE_JOB = re.compile(r"^prove \(shard (\d+)/(\d+)\)$")
REFALSIFY_JOB = re.compile(r"^re-falsify \(shard (\d+)/(\d+)\)$")
# Before 2026-10-05 the replay was this one job; since then it is the aggregate over the shards
# above, so it is read only for a run that has no shard jobs.
REFALSIFY_SINGLE = "re-falsify the harnesses this change can reach"

TABLE_START = "CRATES=\"$(cat <<'TABLE'\n"
TABLE_END = "\nTABLE\n"
REFRESHED = re.compile(r"^# Seconds refreshed: .*$", re.M)


# ---------------------------------------------------------------------------------------------
# The table and the packer.

def read_table(text):
    """[(name, seconds, description)] from script/verify's source, in table order."""
    body = text.split(TABLE_START, 1)[1].split(TABLE_END, 1)[0]
    rows = []
    for line in body.splitlines():
        if line.strip():
            name, secs, desc = line.split("\t", 2)
            rows.append((name, int(secs), desc))
    return rows


def write_table(text, seconds, stamp):
    """`text` with each row's seconds replaced from `seconds` ({name: int}; rows it lacks keep
    theirs) and the `# Seconds refreshed:` line set to `stamp`. Order and descriptions are kept."""
    head, rest = text.split(TABLE_START, 1)
    body, tail = rest.split(TABLE_END, 1)
    lines = []
    for line in body.splitlines():
        name, secs, desc = line.split("\t", 2)
        lines.append("%s\t%d\t%s" % (name, seconds.get(name, int(secs)), desc))
    head = REFRESHED.sub("# Seconds refreshed: " + stamp, head, count=1)
    return head + TABLE_START + "\n".join(lines) + TABLE_END + tail


def pack(rows, n):
    """Greedy longest-processing-time over [(seconds, name)]: ([[names] per bin], [load per bin]).

    Sorting first is the whole trick: a round-robin over the table order puts glob and
    machine_discovery in the same bin and doubles the shard. Ties break on the name, so the
    packing is a function of the table and not of its order."""
    bins = [[] for _ in range(n)]
    load = [0] * n
    for secs, name in sorted(rows, key=lambda r: (-r[0], r[1])):
        i = load.index(min(load))
        bins[i].append(name)
        load[i] += secs
    return bins, load


def parse_spec(spec):
    try:
        k, n = (int(x) for x in spec.split("/", 1))
    except ValueError:
        sys.exit("script/verify: --shard wants k/n (for example 1/2), got %r" % spec)
    return k, n


def stdin_rows():
    rows = []
    for line in sys.stdin:
        if line.strip():
            name, secs, _desc = line.rstrip("\n").split("\t", 2)
            rows.append((int(secs), name))
    return rows


def shard(spec):
    k, n = parse_spec(spec)
    rows = stdin_rows()
    if not 1 <= k <= n:
        sys.exit("script/verify: shard %d/%d is out of range" % (k, n))
    if n > len(rows):
        sys.exit("script/verify: %d shards for %d crates leaves empty ones" % (n, len(rows)))
    bins, load = pack(rows, n)
    # **The shards must partition the table exactly**, and this asserts it on every invocation
    # rather than in a gate somebody has to run. A crate dropped from every shard is not proved at
    # all, and that failure is INVISIBLE: the suite goes green faster and nothing says a harness
    # stopped running. A crate in two shards only wastes a runner. So the dangerous direction is
    # checked here, where it cannot drift away from the code that does the packing.
    placed = [name for b in bins for name in b]
    if sorted(placed) != sorted(name for _, name in rows) or len(placed) != len(set(placed)):
        sys.exit("script/verify: sharding did not partition the crate table; refusing to prove a "
                 "subset and report it as the suite")
    sys.stderr.write("shard %d/%d: %d crates, ~%.1f min of a %.1f-minute serial total\n"
                     % (k, n, len(bins[k - 1]), load[k - 1] / 60, sum(load) / 60))
    print(" ".join(bins[k - 1]))
    return 0


def shard_verdict(estimate, actual):
    """None when `actual` seconds is within SHARD_WARN_PCT of `estimate`, else the signed percent."""
    if estimate <= 0:
        return None
    pct = 100.0 * (actual - estimate) / estimate
    return pct if abs(pct) > SHARD_WARN_PCT else None


def shard_check(spec, actual):
    k, n = parse_spec(spec)
    _bins, load = pack(stdin_rows(), n)
    estimate = load[k - 1]
    pct = shard_verdict(estimate, actual)
    print("shard %d/%d: planned %.1f min, proved in %.1f min" % (k, n, estimate / 60, actual / 60))
    if pct is not None:
        prefix = "::warning::" if os.environ.get("GITHUB_ACTIONS") else "warning: "
        print("%sprove shard %d/%d took %.1f min against the %.1f the table in script/verify "
              "planned (%+.0f%%, warns past %.0f%%). The table has drifted; the weekly "
              "verify-shard-refresh workflow rewrites it, or run "
              "`python3 helpers/verify_times.py refresh` by hand."
              % (prefix, k, n, actual / 60, estimate / 60, pct, SHARD_WARN_PCT))
    return 0


# ---------------------------------------------------------------------------------------------
# Reading a job log.

STAMP = re.compile(r"^\ufeff?(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?)Z (.*)$")


def _seconds(stamp):
    whole = datetime.datetime.fromisoformat(stamp[:19] + "+00:00").timestamp()
    return whole + float("0" + stamp[19:]) if len(stamp) > 19 else whole


def parse_log(text):
    """{crate: seconds} from one prove shard's log. The explicit `==> kani-seconds:` lines win;
    without them, the GitHub timestamps on the `==> kani:` lines (see the header)."""
    explicit, starts, completes = {}, [], []
    for line in text.splitlines():
        m = STAMP.match(line)
        body = m.group(2) if m else line
        e = re.match(r"==> kani-seconds: (\S+) (\d+)\s*$", body)
        if e:
            explicit[e.group(1)] = int(e.group(2))
            continue
        if not m:
            continue
        t = _seconds(m.group(1))
        k = re.match(r"==> kani: (\S+)", body)
        if k:
            starts.append((t, k.group(1)))
        elif body.startswith("Complete - "):
            completes.append(t)
    if explicit:
        return explicit
    out = {}
    for i, (t, crate) in enumerate(starts):
        if i + 1 < len(starts):
            end = starts[i + 1][0]
        else:
            after = [c for c in completes if c >= t]
            if not after:
                continue
            end = after[-1]
        out[crate] = round(end - t)
    return out


REPLAY_START = re.compile(r"==> (\S+::\S+)\s*$")
REPLAY_VERDICT = re.compile(r"\s{4}(?:red|SURVIVOR|ERROR|STALE)\b.*\(([0-9.]+) s\)\s*$")
REPLAY_PLAN = re.compile(r"shard (\d+)/(\d+): (\d+) record\(s\), estimated ([0-9.]+) s")


def parse_replay_log(text):
    """One re-falsify shard's log: ({record: seconds}, planned seconds or None).

    script/falsifications already prints both: the plan as `shard k/n: N record(s), estimated S s`
    and each verdict with its wall clock, build included, as `(12.3 s)`. The same two patterns its
    own `--record-times` reads from the weekly sweep."""
    records, plan, cur = {}, None, None
    for line in text.splitlines():
        m = STAMP.match(line)
        body = m.group(2) if m else line
        p = REPLAY_PLAN.search(body)
        if p:
            plan = float(p.group(4))
            continue
        s = REPLAY_START.search(body)
        if s:
            cur = s.group(1)
            continue
        v = REPLAY_VERDICT.search(body)
        if v and cur:
            records[cur] = float(v.group(1))
            cur = None
    return records, plan


# ---------------------------------------------------------------------------------------------
# GitHub.

def gh_json(path, tries=3):
    # Retried, because one TLS timeout in hour-long backfill of thousands of calls lost the lot on
    # 2026-10-07; a call that fails three times still raises.
    for attempt in range(tries):
        done = subprocess.run(["gh", "api", path], capture_output=True, text=True)
        if not done.returncode:
            return json.loads(done.stdout)
        if attempt + 1 < tries:
            time.sleep(5 * (attempt + 1))
    raise RuntimeError("gh api %s: %s" % (path, done.stderr.strip()))


def gh_text(path):
    done = subprocess.run(["gh", "api", "--allow-escape-sequences", path], capture_output=True)
    if done.returncode:
        raise RuntimeError("gh api %s: %s" % (path, done.stderr.decode(errors="replace").strip()))
    return done.stdout.decode(errors="replace")


def _minutes(job):
    t0 = datetime.datetime.fromisoformat(job["started_at"].replace("Z", "+00:00"))
    t1 = datetime.datetime.fromisoformat(job["completed_at"].replace("Z", "+00:00"))
    return (t1 - t0).total_seconds() / 60.0


def run_row(run, jobs):
    """One CSV row for a run, or None when it ran no prove shard or one of them did not pass.

    Pure over the API's job objects, so the selection is tested without the network."""
    prove, refalsify, single = {}, {}, None
    for job in jobs:
        name = job["name"]
        p, r = PROVE_JOB.match(name), REFALSIFY_JOB.match(name)
        if p:
            if job["conclusion"] != "success":
                return None
            prove[int(p.group(1))] = _minutes(job)
        elif r and job["conclusion"] == "success":
            refalsify[int(r.group(1))] = _minutes(job)
        elif name == REFALSIFY_SINGLE and job["conclusion"] == "success":
            single = _minutes(job)
    if not prove:
        return None
    if not refalsify and single is not None:
        refalsify = {1: single}
    fmt = lambda d: ";".join("%.1f" % d[k] for k in sorted(d))
    every = list(prove.values()) + list(refalsify.values())
    return {"run": run["id"], "created": run["created_at"], "event": run["event"],
            "prove_minutes": fmt(prove), "refalsify_minutes": fmt(refalsify),
            "slowest_minutes": "%.1f" % max(every)}


# Workflow-run listings are fetched 50 to a page rather than the API's maximum of 100. The
# metrics workflow on 2026-10-12 got HTTP 502 from a 100-run page on three retries running,
# for a query that succeeded at once from a laptop; a smaller page asks less of the server.
RUNS_PAGE = 50


def runs_on(day):
    """Successful verify runs created on `day`, newest first."""
    out, page = [], 1
    while True:
        data = gh_json("repos/%s/actions/workflows/%s/runs?status=success&created=%s&per_page=%d&page=%d"
                       % (REPO, WORKFLOW, day, RUNS_PAGE, page))
        out += data["workflow_runs"]
        if len(data["workflow_runs"]) < RUNS_PAGE:
            return out
        page += 1


# A run shorter than this cannot have run a prove shard (the fastest one on record took 13
# minutes), so its jobs are not fetched. It is a cost filter only: `run_row` still decides.
MIN_RUN_MINUTES = 10


def long_enough(run):
    if not run.get("run_started_at") or not run.get("updated_at"):
        return True
    t0 = datetime.datetime.fromisoformat(run["run_started_at"].replace("Z", "+00:00"))
    t1 = datetime.datetime.fromisoformat(run["updated_at"].replace("Z", "+00:00"))
    return (t1 - t0).total_seconds() >= MIN_RUN_MINUTES * 60


def jobs_of(run_id):
    return gh_json("repos/%s/actions/runs/%d/jobs?per_page=100" % (REPO, run_id))["jobs"]


def recent_prove_runs(count):
    """The newest `count` runs whose prove shards all passed: [(run, jobs)]."""
    found, page = [], 1
    while len(found) < count and page <= 40:
        data = gh_json("repos/%s/actions/workflows/%s/runs?status=success&per_page=%d&page=%d"
                       % (REPO, WORKFLOW, RUNS_PAGE, page))
        for run in data["workflow_runs"]:
            if not long_enough(run):
                continue
            jobs = jobs_of(run["id"])
            if run_row(run, jobs):
                found.append((run, jobs))
                if len(found) == count:
                    break
        if not data["workflow_runs"]:
            break
        page += 1
    return found


# ---------------------------------------------------------------------------------------------
# The refresh.

def medians(per_run):
    """{crate: median seconds} over a list of {crate: seconds}."""
    seen = {}
    for d in per_run:
        for crate, secs in d.items():
            seen.setdefault(crate, []).append(secs)
    return {c: int(round(statistics.median(v))) for c, v in seen.items()}


def drift(table, measured, n):
    """Why the table should be rewritten, as a list of sentences; empty when it should not.

    `table` is {crate: seconds}, `measured` {crate: median seconds}. A crate missing from
    `measured` (new since the runs read) keeps its table seconds on both sides."""
    fresh = dict(table)
    fresh.update({c: s for c, s in measured.items() if c in table})
    bins, planned = pack([(s, c) for c, s in table.items()], n)
    reasons = []
    for i, names in enumerate(bins):
        actual = sum(fresh[c] for c in names)
        if planned[i] and abs(actual - planned[i]) * 100.0 / planned[i] > REFRESH_PCT:
            reasons.append("shard %d/%d is planned at %.1f min and its crates' medians sum to %.1f"
                           % (i + 1, n, planned[i] / 60, actual / 60))
    now = max(sum(fresh[c] for c in names) for names in bins)
    _b, better = pack([(s, c) for c, s in fresh.items()], n)
    if now - max(better) >= REPACK_SAVING_S:
        reasons.append("repacking by the medians shortens the slowest shard from %.1f to %.1f min"
                       % (now / 60, max(better) / 60))
    return reasons


def shard_count():
    """How many prove shards CI runs, read from the matrix in verify.yml.

    This is the one place the number is parsed, so `shards` below and the refresh's own drift
    check cannot disagree about it."""
    m = re.search(r"script/verify --shard \$\{\{ matrix\.shard \}\}/(\d+)", open(VERIFY_YML).read())
    if not m:
        sys.exit("verify_times: cannot find the prove shard count in %s" % VERIFY_YML)
    return int(m.group(1))


def measure(count):
    """The newest `count` runs with passing prove shards: (prove {crate: s} per run, replay logs
    as [(records, plan)] per re-falsify shard, run ids)."""
    runs = recent_prove_runs(count)
    per_run, replays, ids = [], [], []
    for run, jobs in runs:
        merged = {}
        for job in jobs:
            log = lambda: gh_text("repos/%s/actions/jobs/%d/logs" % (REPO, job["id"]))
            if PROVE_JOB.match(job["name"]):
                merged.update(parse_log(log()))
            elif REFALSIFY_JOB.match(job["name"]) and job["conclusion"] == "success":
                replays.append(parse_replay_log(log()))
        per_run.append(merged)
        ids.append(run["id"])
    return per_run, replays, ids


def arg_value(argv, flag, default):
    return argv[argv.index(flag) + 1] if flag in argv else default


def crates(argv):
    per_run, _replays, ids = measure(int(arg_value(argv, "--runs", str(REFRESH_RUNS))))
    for crate, secs in sorted(medians(per_run).items(), key=lambda x: -x[1]):
        print("%s\t%d" % (crate, secs))
    print("# %d runs: %s" % (len(ids), " ".join(map(str, ids))), file=sys.stderr)
    return 0


# ---------------------------------------------------------------------------------------------
# The re-falsify replay times (calef, 2026-10-07 UTC, on #1802: refresh them from CI, then lower
# the target). script/falsifications packs its shards by REPLAY_TIMES, which the weekly sweep
# also writes from its own single transcript; this side writes medians over the verify runs.

def read_times(text):
    """(comment lines, {record: seconds}) from the times file."""
    head, times = [], {}
    for line in text.splitlines():
        if line.startswith("#") or not line.strip():
            head.append(line)
            continue
        key, secs = line.split("\t")[:2]
        times[key] = float(secs)
    return head, times


def write_times(head, times, stamp):
    """The times file with every record sorted and the `# Seconds refreshed:` line set (added
    after the other comment lines when absent)."""
    head = [l for l in head if not REFRESHED.match(l)] + ["# Seconds refreshed: " + stamp]
    return "\n".join(head + ["%s\t%.1f" % (k, times[k]) for k in sorted(times)]) + "\n"


def replay_medians(replays, minimum=REPLAY_MIN_SAMPLES):
    """{record: median seconds} over every shard log a record appeared in at least `minimum` times."""
    seen = {}
    for records, _plan in replays:
        for k, v in records.items():
            seen.setdefault(k, []).append(v)
    return {k: round(statistics.median(v), 1) for k, v in seen.items() if len(v) >= minimum}


def replay_drift(times, measured, replays):
    """Why the replay times should be rewritten; empty when they should not.

    Three reasons, each about the plan rather than one record: a measured record the file lacks
    (it is packed at its package's mean), the records both know summing more than REFRESH_PCT
    apart, or the shards themselves running a median more than REFRESH_PCT off their plans."""
    reasons = []
    missing = sorted(k for k in measured if k not in times)
    if missing:
        reasons.append("%d measured record(s) have no time and are packed by a guess, e.g. %s"
                       % (len(missing), missing[0]))
    common = [k for k in measured if k in times]
    planned, actual = sum(times[k] for k in common), sum(measured[k] for k in common)
    if planned and abs(actual - planned) * 100.0 / planned > REFRESH_PCT:
        reasons.append("the %d records both know sum to %.1f min measured against %.1f planned"
                       % (len(common), actual / 60, planned / 60))
    ratios = [sum(r.values()) / plan for r, plan in replays if plan]
    if ratios and abs(statistics.median(ratios) - 1) * 100 > REFRESH_PCT:
        reasons.append("re-falsify shards ran a median %.0f%% of their plan" % (100 * statistics.median(ratios)))
    return reasons


def refresh(argv):
    count = int(arg_value(argv, "--runs", str(REFRESH_RUNS)))
    text = open(VERIFY).read()
    table = {name: secs for name, secs, _d in read_table(text)}
    per_run, replays, ids = measure(count)
    if not ids:
        print("verify_times: no run with passing prove shards found; nothing to compare", file=sys.stderr)
        return 2
    today = datetime.datetime.now(datetime.timezone.utc).date().isoformat()
    stamp = ("%s (UTC), the median of %d CI runs, %d to %d, by helpers/verify_times.py refresh"
             % (today, len(ids), min(ids), max(ids)))
    measured = medians(per_run)
    reasons = drift(table, measured, shard_count())
    head, times = read_times(open(REPLAY_TIMES).read())
    replayed = replay_medians(replays)
    replay_reasons = replay_drift(times, replayed, replays)
    for r in reasons + replay_reasons:
        print("- " + r)
    if not reasons and not replay_reasons:
        print("verify_times: the prove table and the replay times match %d runs within %.0f%%; "
              "no change" % (len(ids), REFRESH_PCT))
    if "--dry-run" in argv:
        return 0
    if reasons:
        open(VERIFY, "w").write(write_table(text, {c: s for c, s in measured.items() if c in table}, stamp))
        print("verify_times: rewrote %s from %d runs" % (VERIFY, len(ids)))
    if replay_reasons:
        # Records not measured keep their time: a record no recent diff reached is not stale for that.
        times.update(replayed)
        open(REPLAY_TIMES, "w").write(write_times(head, times, stamp))
        print("verify_times: rewrote %s from %d shard logs" % (REPLAY_TIMES, len(replays)))
    return 0


# ---------------------------------------------------------------------------------------------
# The per-run record for the metrics page.

def to_csv(rows):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=RUN_COLUMNS, lineterminator="\n")
    w.writeheader()
    for r in rows:
        w.writerow({c: r.get(c, "") for c in RUN_COLUMNS})
    return buf.getvalue()


def from_csv(text):
    return {str(r["run"]): r for r in csv.DictReader(io.StringIO(text))}


def merge_rows(existing, fresh):
    """A fresh run replaces its own row; every other row is kept. Sorted by creation time."""
    out = dict(existing)
    out.update({str(r["run"]): r for r in fresh})
    return sorted(out.values(), key=lambda r: (r["created"], str(r["run"])))


def update(argv):
    today = datetime.datetime.now(datetime.timezone.utc).date()
    since = datetime.date.fromisoformat(
        arg_value(argv, "--since", (today - datetime.timedelta(days=DEFAULT_WINDOW_DAYS)).isoformat()))
    total = 0
    day = since
    while day <= today:
        fresh, taken = [], 0
        for run in runs_on(day.isoformat()):
            if not long_enough(run):
                continue
            row = run_row(run, jobs_of(run["id"]))
            if row:
                fresh.append(row)
                taken += 1
                if taken == PER_DAY:
                    break
        # Written a day at a time, so a failure part way keeps every day before it.
        existing = from_csv(open(RUNS_CSV).read()) if os.path.exists(RUNS_CSV) else {}
        with open(RUNS_CSV, "w") as f:
            f.write(to_csv(merge_rows(existing, fresh)))
        print("verify_times: %s: %d run(s)" % (day, len(fresh)), file=sys.stderr)
        total += len(fresh)
        day += datetime.timedelta(days=1)
    print("verify_times: %d run(s) from %s written to %s" % (total, since, RUNS_CSV))
    return 0


# ---------------------------------------------------------------------------------------------

def selftest():
    ok = True

    def expect(label, got, want):
        nonlocal ok
        if got != want:
            ok = False
            print("FAIL %s: got %r want %r" % (label, got, want), file=sys.stderr)

    # The packer: LPT, a partition, and order-independent.
    rows = [(700, "glob"), (690, "machine_discovery"), (310, "calendar"), (270, "subtree_scope"),
            (5, "paging")]
    bins, load = pack(rows, 2)
    expect("the two big crates are split", sorted([bins[0][0], bins[1][0]]), ["glob", "machine_discovery"])
    expect("a partition", sorted(sum(bins, [])), sorted(n for _, n in rows))
    expect("independent of table order", pack(list(reversed(rows)), 2), (bins, load))

    # The run's own warning: inside the band is silent, either side past it speaks.
    expect("20% over is noise", shard_verdict(1200, 1440), None)
    expect("36% over warns", round(shard_verdict(1188, 1620)), 36)
    expect("far under warns too", round(shard_verdict(1200, 600)), -50)
    expect("no estimate, no verdict", shard_verdict(0, 600), None)

    # The log reader: the explicit line wins, and the timestamp fallback measures start to start
    # and the last crate to its final Complete line.
    log = ("\ufeff2026-10-07T02:28:48.8997507Z ==> kani: calendar (the civil-date arithmetic)\n"
           "2026-10-07T02:34:37.6677234Z Complete - 11 successfully verified harnesses, 0 failures, 11 total.\n"
           "2026-10-07T02:34:37.7072705Z ==> kani: machine_discovery (the firmware tables)\n"
           "2026-10-07T02:47:24.6726048Z Complete - 16 successfully verified harnesses, 0 failures, 16 total.\n"
           "2026-10-07T02:47:30.0000000Z ##[group]Post job cleanup.\n")
    expect("timestamps", parse_log(log), {"calendar": 349, "machine_discovery": 767})
    expect("explicit lines win", parse_log(log + "2026-10-07T02:47:24Z ==> kani-seconds: glob 708\n"),
           {"glob": 708})
    expect("a crate that never completed is not timed",
           parse_log("2026-10-07T02:28:48Z ==> kani: glob (x)\n"), {})

    # Run selection: a failed prove shard disqualifies the run; the old single replay job counts
    # only where there are no replay shards.
    def job(name, minutes, conclusion="success"):
        return {"name": name, "conclusion": conclusion, "started_at": "2026-10-06T00:00:00Z",
                "completed_at": "2026-10-06T00:%02d:%02dZ" % (int(minutes), round(minutes % 1 * 60))}
    run = {"id": 7, "created_at": "2026-10-06T00:00:00Z", "event": "merge_group"}
    good = [job("prove (shard 1/2)", 20.5), job("prove (shard 2/2)", 30.0),
            job("re-falsify (shard 1/2)", 25.0), job("re-falsify (shard 2/2)", 22.0),
            job(REFALSIFY_SINGLE, 0.5), job("gate", 1)]
    r = run_row(run, good)
    expect("row", (r["prove_minutes"], r["refalsify_minutes"], r["slowest_minutes"]),
           ("20.5;30.0", "25.0;22.0", "30.0"))
    expect("a failed prove shard drops the run",
           run_row(run, good[:1] + [job("prove (shard 2/2)", 9, "failure")]), None)
    expect("no prove shard, no row", run_row(run, [job("gate", 1)]), None)
    three = run_row(run, [job("prove (shard 1/3)", 15.0), job("prove (shard 2/3)", 16.5),
                          job("prove (shard 3/3)", 16.0)])
    expect("three prove shards, in shard order", (three["prove_minutes"], three["slowest_minutes"]),
           ("15.0;16.5;16.0", "16.5"))
    expect("a five-minute run is skipped unfetched",
           long_enough({"run_started_at": "2026-10-06T00:00:00Z", "updated_at": "2026-10-06T00:05:00Z"}), False)
    expect("a forty-minute run is fetched",
           long_enough({"run_started_at": "2026-10-06T00:00:00Z", "updated_at": "2026-10-06T00:40:00Z"}), True)
    expect("the single replay job stands in before shards existed",
           run_row(run, [job("prove (shard 1/2)", 10), job(REFALSIFY_SINGLE, 12)])["refalsify_minutes"], "12.0")

    # Drift: the 2026-10-06 shape is caught, a table that matches is left alone, and a crate the
    # runs never saw keeps its seconds.
    old = {"glob": 902, "calendar": 600, "machine_discovery": 180, "subtree_scope": 120, "usb": 32,
           "direct_memory_access_validator": 175}
    now = {"glob": 708, "calendar": 313, "machine_discovery": 689, "subtree_scope": 267, "usb": 170,
           "direct_memory_access_validator": 148}
    expect("stale table drifts", bool(drift(old, now, 2)), True)
    expect("three shards on a matching table are quiet", drift(now, now, 3), [])
    expect("the two-shard table packed three ways shortens the slowest shard",
           any("repacking" in r for r in drift(old, now, 3)), True)
    expect("matching table is quiet", drift(now, now, 2), [])
    expect("unmeasured crate kept", drift(dict(now, new_crate=5), now, 2), [])
    expect("10% everywhere is under the line", drift(now, {c: int(s * 1.1) for c, s in now.items()}, 2), [])

    # The replay side: the log reader, the drift rule and the times file.
    rlog = ("2026-10-07T02:28:47.0337237Z shard 2/3: 70 record(s), estimated 1185 s\n"
            "2026-10-07T02:28:47.2629172Z ==> glob::a\n"
            "2026-10-07T02:28:50.1808178Z     red      the harness caught it (2.9 s)\n"
            "2026-10-07T02:28:50.1824960Z ==> glob::b\n"
            "2026-10-07T02:29:08.5367948Z     SURVIVOR it stayed green with its own falsification applied (13.5 s)\n"
            "2026-10-07T02:29:08.6Z ==> glob::c\n"
            "2026-10-07T02:29:09Z     STALE    the patch no longer applies; the covered code moved\n")
    expect("replay log", parse_replay_log(rlog), ({"glob::a": 2.9, "glob::b": 13.5}, 1185.0))
    logs = [({"glob::a": 10.0, "glob::b": 20.0}, 30.0)] * 2 + [({"glob::a": 12.0}, 12.0)]
    expect("a record needs three samples", replay_medians(logs), {"glob::a": 10.0})
    expect("matching times are quiet", replay_drift({"glob::a": 10.0}, {"glob::a": 10.5}, logs), [])
    expect("a measured record without a time drifts",
           len(replay_drift({"glob::b": 1.0}, {"glob::a": 10.0, "glob::b": 1.0}, logs)), 1)
    expect("a sum 20% off drifts", len(replay_drift({"glob::a": 10.0}, {"glob::a": 12.0}, logs)), 1)
    expect("shards running 1.3 times their plan drift",
           len(replay_drift({"glob::a": 10.0}, {"glob::a": 10.0}, [({"x::y": 13.0}, 10.0)])), 1)
    head, times = read_times("# a comment\n# Seconds refreshed: old\nb::x\t2.0\na::x\t1.25\n")
    expect("times file", write_times(head, times, "new"),
           "# a comment\n# Seconds refreshed: new\na::x\t1.2\nb::x\t2.0\n")
    live_head, live_times = read_times(open(REPLAY_TIMES).read())
    expect("live times file parses and is stamped",
           (len(live_times) > 100, any(REFRESHED.match(l) for l in live_head)), (True, True))

    # The table rewrite: seconds replaced, descriptions, order and unmeasured rows kept, stamp set.
    src = ("# Seconds refreshed: never\n" + TABLE_START + "glob\t902\tmatching is total\n"
           "kernel\t5\tthe run arithmetic" + TABLE_END + "rest\n")
    out = write_table(src, {"glob": 708}, "2026-10-07 (UTC), test")
    expect("rewrite", read_table(out), [("glob", 708, "matching is total"), ("kernel", 5, "the run arithmetic")])
    expect("stamp", out.splitlines()[0], "# Seconds refreshed: 2026-10-07 (UTC), test")
    expect("tail kept", out.endswith(TABLE_END + "rest\n"), True)

    # The live script/verify parses, every row has seconds, and the packer partitions it.
    live = read_table(open(VERIFY).read())
    expect("live table has rows", len(live) > 10, True)
    lb, _l = pack([(s, n) for n, s, _d in live], shard_count())
    expect("live table partitions", sorted(sum(lb, [])), sorted(n for n, _s, _d in live))
    expect("live table carries a refresh stamp", bool(REFRESHED.search(open(VERIFY).read())), True)

    # The CSV: a rerun replaces its own rows and keeps the rest.
    a = {"run": 1, "created": "2026-10-05T00:00:00Z", "event": "push", "prove_minutes": "20.0;27.0",
         "refalsify_minutes": "", "slowest_minutes": "27.0"}
    b = dict(a, run=2, created="2026-10-06T00:00:00Z")
    rows = merge_rows(from_csv(to_csv([a])), [b])
    expect("kept and added", [str(x["run"]) for x in rows], ["1", "2"])
    expect("idempotent", to_csv(merge_rows(from_csv(to_csv(rows)), [b])), to_csv(rows))

    print("verify_times selftest: %s" % ("ok" if ok else "FAILED"))
    return 0 if ok else 1


def main(argv):
    if argv[1:] == ["--selftest"]:
        return selftest()
    cmd = argv[1] if len(argv) > 1 else ""
    if cmd == "shard" and len(argv) == 3:
        return shard(argv[2])
    if cmd == "shards" and len(argv) == 2:
        # Read by `script/verify --parallel`, whose default child count is the smaller of this and
        # the machine's heavy-job slots (helpers/verify_parallel.py).
        print(shard_count())
        return 0
    if cmd == "shard-check" and len(argv) == 4:
        return shard_check(argv[2], float(argv[3]))
    if cmd == "crates":
        return crates(argv[2:])
    if cmd == "refresh":
        return refresh(argv[2:])
    if cmd == "update":
        return update(argv[2:])
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
