#!/usr/bin/env python3
"""helpers/fuzz-red-shards.py: the continuous fuzz host learns which CI fuzz shards went red.

    python3 -I helpers/fuzz-red-shards.py look         queue each new red shard for a replay
    python3 -I helpers/fuzz-red-shards.py --selftest   the red-shard rule against fixtures, no network

The `fuzz-guests` job of `.github/workflows/ci.yml` runs four shards of the in-guest drivers on every
merge-queue entry, and a red shard prints only a pointer: the commit, the shard and the line "the fuzz
host replays this shard". The supervisor (helpers/continuous-fuzz.sh) runs `look` at most every 15
minutes (`NIFE_FUZZ_REPLAY_EVERY`). `look` appends one line per red shard to
`$NIFE_FUZZ_HOME/state/replay-queue`, and the supervisor replays each queued shard with
`helpers/continuous-fuzz.sh --replay` while no slot runs. The design is in
design/roadmap/1038-the-fuzz-host-replays-a-red-ci-shard.md.

How it reads GitHub. A poll, with no credential, of public data only:

  1. One request lists the newest failed runs of ci.yml started by the merge queue
     (`actions/workflows/ci.yml/runs?event=merge_group&status=failure`). A run that is not a
     completed `merge_group` run on a `gh-readonly-queue/main/` branch, or whose commit is not a full
     commit id, is ignored.
  2. A run counts only once the clone's HEAD contains its commit. The clone moves only as
     helpers/fuzz-follow-main.py allows (fast-forward, main of the public repository, green), so a
     replay runs only a commit that main holds. A merge-queue entry whose only red check is
     `fuzz-guests` lands, because that check is not required, so its commit reaches main unchanged.
     An entry that never lands is never replayed: after 24 hours its run is recorded as
     `never-on-main` and dropped. The host fetches nothing for this; GitHub deleting the queue's
     branch does not matter.
  3. For a run on main, one request lists the run's jobs (`actions/runs/<id>/jobs?filter=latest`).
     A red shard is a job of that run named `fuzz-guests (shard N, ...)` with N from 0 to 3, on the
     run's commit, completed with the conclusion `failure` or `timed_out`. A green, skipped or
     cancelled shard, a red job of any other name and a job on another commit start nothing.
  4. Before it queues, the commit must be green by the follower's own rule (`verdict`), on that
     exact commit: main's required checks read from the branch's rules, and the commit's check runs.
     Two or three more requests, made only when a red shard exists. Not green yet keeps the run for
     the next look; not green after 24 hours records it as `not-green`.

The pointer line in the job's log is never read: the run's commit and the job's name carry the
commit and the shard, and a job's log is the one place the hosted runner could have written guest
text by mistake.

Rate: a look makes one request, plus one per new failed run that main already holds, plus three per
red shard. Main's failed merge-queue runs are mostly entries that never land, which cost nothing
past the listing. At one look every 15 minutes that is about 4 requests an hour, beside the
follower's, against GitHub's documented 60 unauthenticated requests an hour per address.

State, under `$NIFE_FUZZ_HOME/state/` (default `$HOME/fuzz/state/`):
  replay-queue      one line per shard to replay: commit, shard, the time it went red (seconds since
                    the epoch), attempts so far. The supervisor removes a line once it replays it.
  replay-seen.tsv   one line per run decided: run id, verdict, the UTC time. Runs no longer in the
                    listing are dropped from it.

Every line either file holds is public already: the commit and the shard are in the hosted log.

Exit status: `look` exits 0 when it queued at least one shard and prints one line saying how many;
otherwise it exits 1 and prints one line saying why on standard error. 2 is a usage error.

Name: provisional, minted 2026-11-10 (UTC) by lane milestone/1038-the-fuzz-host-replays-a-red-ci-shard,
as are the subcommand, the state files, their verdict words and `NIFE_FUZZ_REPLAY_EVERY`. No
architect has ruled on them.
"""

import importlib.util
import os
import re
import sys
import time
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("fuzz_follow_main",
                                               os.path.join(HERE, "fuzz-follow-main.py"))
follow = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(follow)
Stay = follow.Stay

PER_PAGE = 50
RUNS = f"actions/workflows/ci.yml/runs?event=merge_group&status=failure&per_page={PER_PAGE}"
QUEUE_BRANCH = "gh-readonly-queue/main/"
SHARD_JOB = re.compile(r"^fuzz-guests \(shard ([0-3]), ")
RED = {"failure", "timed_out"}
# A run that main has not taken, or whose commit is not green, within this long never will be.
PATIENCE_SECONDS = 24 * 3600


def home():
    return os.environ.get("NIFE_FUZZ_HOME") or os.path.join(os.path.expanduser("~"), "fuzz")


def epoch(text):
    try:
        return int(datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ")
                   .replace(tzinfo=timezone.utc).timestamp())
    except (TypeError, ValueError):
        raise Stay(f"the time {str(text)[:30]!r} is not a UTC time")


def queue_runs(listing):
    """[(run id, commit, created epoch)] for each failed merge-queue run of the listing."""
    if not isinstance(listing, dict) or not isinstance(listing.get("workflow_runs"), list):
        raise Stay("the run listing is unreadable")
    out = []
    for run in listing["workflow_runs"]:
        if not isinstance(run, dict):
            raise Stay("a run of the listing is unreadable")
        if (run.get("event") != "merge_group" or run.get("status") != "completed"
                or not str(run.get("head_branch") or "").startswith(QUEUE_BRANCH)
                or not isinstance(run.get("id"), int)
                or not follow.SHA_RE.match(str(run.get("head_sha")))):
            continue
        out.append((run["id"], run["head_sha"], epoch(run.get("created_at"))))
    return out


def red_shards(jobs, sha):
    """[(shard, completed epoch)] for each red fuzz-guests shard of a run's job listing."""
    if not isinstance(jobs, dict) or not isinstance(jobs.get("jobs"), list):
        raise Stay("the job listing is unreadable")
    total = jobs.get("total_count")
    if not isinstance(total, int) or total != len(jobs["jobs"]):
        raise Stay("the job listing is incomplete")
    out = {}
    for job in jobs["jobs"]:
        if not isinstance(job, dict):
            raise Stay("a job of the listing is unreadable")
        m = SHARD_JOB.match(str(job.get("name")))
        if (not m or job.get("head_sha") != sha or job.get("status") != "completed"
                or job.get("conclusion") not in RED):
            continue
        out[int(m.group(1))] = epoch(job.get("completed_at"))
    return sorted(out.items())


def on_main(sha):
    """Whether the clone's HEAD, which only the follower moves, contains the commit."""
    return follow.is_ancestor(sha, follow.git("rev-parse", "HEAD"))


def read_lines(path):
    try:
        with open(path) as f:
            return [line.rstrip("\n") for line in f if line.strip()]
    except FileNotFoundError:
        return []


def write_lines(path, lines):
    tmp = f"{path}.tmp"
    with open(tmp, "w") as f:
        f.writelines(line + "\n" for line in lines)
    os.replace(tmp, path)


def decide(runs, seen, now, get_json, on_main):
    """Look at each failed run not yet decided. Returns (queue lines, {run id: verdict})."""
    queued, decided = [], {}
    for run_id, sha, created in runs:
        if str(run_id) in seen:
            continue
        old = now - created > PATIENCE_SECONDS
        if not on_main(sha):
            if old:
                decided[run_id] = "never-on-main"
            continue
        red = red_shards(get_json(f"actions/runs/{run_id}/jobs?filter=latest&per_page=100"), sha)
        if not red:
            decided[run_id] = "no-red-shard"
            continue
        reason = follow.verdict(sha, follow.required_checks(get_json("rules/branches/main")),
                                follow.check_runs(sha))
        if reason:
            if old:
                decided[run_id] = "not-green"
            continue
        queued.extend(f"{sha} {shard} {red_at} 0" for shard, red_at in red)
        decided[run_id] = "queued"
    return queued, decided


def look():
    state = os.path.join(home(), "state")
    os.makedirs(state, exist_ok=True)
    queue_path = os.path.join(state, "replay-queue")
    seen_path = os.path.join(state, "replay-seen.tsv")
    runs = queue_runs(follow.get_json(RUNS))
    seen = {}
    for line in read_lines(seen_path):
        parts = line.split("\t")
        if len(parts) == 3:
            seen[parts[0]] = line
    now = int(time.time())
    queued, decided = decide(runs, seen, now, follow.get_json, on_main)
    utc = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now))
    for run_id, word in decided.items():
        seen[str(run_id)] = f"{run_id}\t{word}\t{utc}"
    listed = {str(run_id) for run_id, _, _ in runs}
    if queued:
        have = read_lines(queue_path)
        keys = {" ".join(line.split()[:2]) for line in have}
        write_lines(queue_path, have + [q for q in queued if " ".join(q.split()[:2]) not in keys])
    write_lines(seen_path, [line for key, line in seen.items() if key in listed])
    if not queued:
        raise Stay("no new red fuzz-guests shard on a commit main holds")
    return len(queued)


def selftest():
    sha = "a" * 40
    other = "b" * 40

    def job(name, conclusion="failure", status="completed", head=sha):
        return {"name": name, "status": status, "conclusion": conclusion, "head_sha": head,
                "completed_at": "2026-11-10T12:00:00Z"}

    def listing(*jobs):
        return {"total_count": len(jobs), "jobs": list(jobs)}

    shard = "fuzz-guests (shard %d, the in-guest drivers under KVM)"
    cases = [
        ("a green shard", listing(job(shard % 0, "success")), []),
        ("a skipped shard", listing(job(shard % 1, "skipped")), []),
        ("a cancelled shard", listing(job(shard % 2, "cancelled")), []),
        ("a shard still running", listing(job(shard % 3, None, "in_progress")), []),
        ("a red job of another name", listing(job("coverage (host crates)"),
                                              job(shard % 0, "success")), []),
        ("a red job named like a shard", listing(job("fuzz-guests (shard 4, x)"),
                                                 job("fuzz (parsers, 60s per target)")), []),
        ("a red shard on another commit", listing(job(shard % 1, head=other)), []),
        ("a red shard", listing(job(shard % 2), job(shard % 0, "success")), [2]),
        ("a timed-out shard", listing(job(shard % 3, "timed_out")), [3]),
    ]
    for label, jobs, want in cases:
        got = [s for s, _ in red_shards(jobs, sha)]
        if got != want:
            print(f"fuzz-red-shards selftest: {label}: wanted shards {want}, got {got}",
                  file=sys.stderr)
            return 1
    for label, bad in [("not a listing", {"message": "API rate limit exceeded"}),
                       ("a short listing", {"total_count": 2, "jobs": [job(shard % 0)]})]:
        try:
            red_shards(bad, sha)
        except Stay:
            continue
        print(f"fuzz-red-shards selftest: {label}: the listing was accepted", file=sys.stderr)
        return 1

    def run(run_id, head=sha, event="merge_group", branch="gh-readonly-queue/main/pr-1-x"):
        return {"id": run_id, "head_sha": head, "event": event, "status": "completed",
                "head_branch": branch, "created_at": "2026-11-10T11:00:00Z"}

    runs = queue_runs({"workflow_runs": [run(1), run(2, event="push"), run(3, branch="main"),
                                         run(4, head="abc")]})
    if [r[0] for r in runs] != [1]:
        print(f"fuzz-red-shards selftest: the listing kept runs {[r[0] for r in runs]}, wanted [1]",
              file=sys.stderr)
        return 1

    rules = [{"type": "required_status_checks", "parameters": {
        "required_status_checks": [{"context": "lint", "integration_id": None}]}}]
    green = {"total_count": 1, "check_runs": [{"name": "lint", "status": "completed",
                                               "conclusion": "success", "head_sha": sha,
                                               "app": {"slug": "github-actions", "id": 15368}}]}
    red_jobs = listing(job(shard % 1))
    created = epoch("2026-11-10T11:00:00Z")

    def answers(jobs):
        def get_json(path):
            if path.startswith("actions/runs/"):
                return jobs
            if path == "rules/branches/main":
                return rules
            if "/check-runs" in path:
                return green
            raise Stay(f"unexpected request {path}")
        return get_json

    flows = [
        ("a red shard on main", [(1, sha, created)], red_jobs, True, created + 60,
         ["%s 1 %d 0" % (sha, epoch("2026-11-10T12:00:00Z"))], {1: "queued"}),
        ("a red shard not on main yet", [(1, sha, created)], red_jobs, False, created + 60,
         [], {}),
        ("a red shard never on main", [(1, sha, created)], red_jobs, False,
         created + PATIENCE_SECONDS + 1, [], {1: "never-on-main"}),
        ("a failed run with every shard green", [(1, sha, created)],
         listing(job(shard % 0, "success"), job("lint")), True, created + 60, [],
         {1: "no-red-shard"}),
    ]
    for label, runs, jobs, landed, now, want_q, want_d in flows:
        # follow.check_runs reads through follow.get_json, so the fixture stands in for both.
        real_get_json = follow.get_json
        follow.get_json = answers(jobs)
        try:
            got_q, got_d = decide(runs, {}, now, answers(jobs), lambda s, v=landed: v)
        finally:
            follow.get_json = real_get_json
        if got_q != want_q or got_d != want_d:
            print(f"fuzz-red-shards selftest: {label}: queued {got_q} decided {got_d}, wanted "
                  f"{want_q} {want_d}", file=sys.stderr)
            return 1
    print("fuzz-red-shards selftest: ok")
    return 0


def main(argv):
    if argv == ["--selftest"]:
        return selftest()
    if argv == ["look"]:
        try:
            n = look()
        except Stay as stay:
            print(f"fuzz-red-shards: {stay}", file=sys.stderr)
            return 1
        except Exception as error:  # fail closed: any surprise queues nothing
            print(f"fuzz-red-shards: queued nothing, unexpected {type(error).__name__}: {error}",
                  file=sys.stderr)
            return 1
        print(f"fuzz-red-shards: {n} red shard{'s' if n != 1 else ''} queued for a replay")
        return 0
    print(__doc__.split("\n\n")[1], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
