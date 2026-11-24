#!/usr/bin/env python3
"""helpers/idle_pull_requests.py, run through script/idle-pull-requests: flag every open pull request that nobody owns the next step of.

    script/idle-pull-requests [<repo> ...]       repo is owner/name, or a bare name under nifeos
    script/idle-pull-requests --all-nifeos       every nifeos repository, not the default list
    script/idle-pull-requests --selftest         the classifier against fixtures, no network

Prints one line per flagged pull request (class, repository and number, title, the next step it
needs) and exits 1 if any line printed, else 0 (2 if a repository could not be read). The queue
sweep runs it; partsfoundry/record:notes/idle-pull-requests.md says how a held pull request declares itself.

Name: provisional, minted 2026-11-18 (UTC) by lane/idle-prs-check.

Classes, first match wins:

  QUEUED   in the merge queue (nife's GraphQL merge queue entries). Never flagged.
  ARMED    auto-merge is set. Never flagged.
  HELD     the body has a line `Held on <owner>/<repo>#N` or `Held on #N` (same repository), or a
           label names a wait: needs-architect, held-by-lane, held-for-red-trunk
           (helpers/hold-labels.sh). Flagged HELD-UNBLOCKED once a named blocker is closed or merged.
  RUNNING  updated within the last 30 minutes, an approximation of "a lane is working on it".
           Never flagged, so a fresh claim draft (all its checks skipped) is not IDLE-GREEN.
  IDLE     everything else.

Flags, in priority order, one per pull request:

  HELD-UNBLOCKED  HELD, and a blocker named by a `Held on` line is closed or merged
  RED             not a draft, not queued, and a check failed (the `architect hold` check is ignored:
                  it goes red on purpose; the required-check set is not read, so any other failing
                  check counts)
  IDLE-GREEN      not queued, not armed, not HELD, not RUNNING, every check success or skipped
  IDLE-STALE      not queued, not armed, not HELD, no activity for 2 hours
"""
import json
import re
import subprocess
import sys
from datetime import datetime, timedelta, timezone

DEFAULT_REPOS = [
    "nife", "crate-workflows", "entropy_contract", "designware_mobile_storage",
    "block_roster_contract", "jh7110_entropy", "basalt",
]
HOLD_LABELS = ("needs-architect", "held-by-lane", "held-for-red-trunk")
RUNNING_WINDOW = timedelta(minutes=30)
STALE_AFTER = timedelta(hours=2)
FIELDS = "number,title,isDraft,mergeStateStatus,autoMergeRequest,labels,updatedAt,body,statusCheckRollup"
HELD_ON = re.compile(r"^[ \t>*-]*Held on[ \t]+(?:([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+))?#(\d+)\b", re.I | re.M)
FAILED = {"FAILURE", "TIMED_OUT", "CANCELLED", "ACTION_REQUIRED", "STARTUP_FAILURE", "ERROR"}
PASSED = {"SUCCESS", "SKIPPED", "NEUTRAL"}


def parse_time(text):
    return datetime.fromisoformat(text.replace("Z", "+00:00"))


def blockers(body, repo):
    """The (owner/repo, number) pairs the body's `Held on` lines name, in order written."""
    out = []
    for slug, number in HELD_ON.findall(body or ""):
        pair = (slug or repo, int(number))
        if pair not in out:
            out.append(pair)
    return out


def check_state(rollup):
    """('green'|'red'|'pending', names of failing checks). An empty rollup is pending."""
    failing, pending, seen = [], False, 0
    for item in rollup or []:
        name = item.get("name") or item.get("context") or "?"
        if "architect hold" in name:
            continue
        seen += 1
        verdict = (item.get("conclusion") or item.get("state") or "").upper()
        if verdict in FAILED:
            failing.append(name)
        elif verdict not in PASSED:
            pending = True
    if failing:
        return "red", failing
    return ("pending" if pending or seen == 0 else "green"), []


def classify(pr, repo, queued, closed, now):
    """Returns (class, flag-or-None, next step). `closed(slug, number)` says if a blocker is done."""
    number = pr["number"]
    labels = [label["name"] for label in pr.get("labels") or []]
    held_labels = [name for name in labels if name in HOLD_LABELS]
    named = blockers(pr.get("body"), repo)
    state, failing = check_state(pr.get("statusCheckRollup"))
    draft = bool(pr.get("isDraft"))
    age = now - parse_time(pr["updatedAt"])
    if number in queued:
        return "QUEUED", None, ""
    if pr.get("autoMergeRequest"):
        return "ARMED", None, ""
    if named or held_labels:
        done = [f"{slug}#{n}" for slug, n in named if closed(slug, n)]
        if done:
            step = f"{', '.join(done)} is closed or merged: merge main in, drop the `Held on` line, "
            step += "mark ready and run helpers/arm.sh " + str(number) if draft else "run helpers/arm.sh " + str(number)
            return "HELD", "HELD-UNBLOCKED", step
        return "HELD", None, ""
    if not draft and state == "red":
        return "IDLE", "RED", f"fix {', '.join(failing)}, or say in a comment why it waits"
    if age < RUNNING_WINDOW:
        return "RUNNING", None, ""
    if state == "green":
        if draft:
            return "IDLE", "IDLE-GREEN", "run script/lint, mark ready, wait for the ready run, then helpers/arm.sh " + str(number) + "; or add a `Held on #N` line"
        return "IDLE", "IDLE-GREEN", f"run helpers/arm.sh {number}"
    if age >= STALE_AFTER:
        hours = int(age.total_seconds() // 3600)
        return "IDLE", "IDLE-STALE", f"no activity for {hours}h: resume the lane, or add a `Held on #N` line naming what it waits for"
    return "IDLE", None, ""


def sweep(repos, now, prs_of, queued_of, closed):
    """Yields (repo, pr, class, flag, step) for every open pull request."""
    for repo in repos:
        queued = queued_of(repo)
        for pr in prs_of(repo):
            cls, flag, step = classify(pr, repo, queued, closed, now)
            yield repo, pr, cls, flag, step


def gh_json(args):
    done = subprocess.run(["gh", *args], capture_output=True, text=True)
    if done.returncode != 0:
        raise RuntimeError(done.stderr.strip() or f"gh {' '.join(args)} failed")
    return json.loads(done.stdout)


def gh_prs(repo):
    return gh_json(["pr", "list", "-R", repo, "--state", "open", "--limit", "200", "--json", FIELDS])


def gh_queued(repo):
    owner, name = repo.split("/")
    query = "query($o:String!,$n:String!){repository(owner:$o,name:$n){mergeQueue{entries(first:100){nodes{pullRequest{number}}}}}}"
    try:
        data = gh_json(["api", "graphql", "-f", f"query={query}", "-F", f"o={owner}", "-F", f"n={name}"])
        queue = data["data"]["repository"]["mergeQueue"]
    except (RuntimeError, KeyError, TypeError):
        return set()  # a repository with no merge queue has no entries
    return {node["pullRequest"]["number"] for node in (queue or {}).get("entries", {}).get("nodes", [])}


def gh_closed(slug, number):
    try:
        return gh_json(["api", f"repos/{slug}/issues/{number}"]).get("state") == "closed"
    except RuntimeError:
        return False  # unreadable is not unblocked


def format_line(repo, pr, flag, step):
    return f"{flag:<15} {repo}#{pr['number']} {pr['title']!r}: {step}"


def selftest():
    now = parse_time("2026-11-18T12:00:00Z")

    def ago(**kw):
        return (now - timedelta(**kw)).strftime("%Y-%m-%dT%H:%M:%SZ")

    ok = [{"conclusion": "SUCCESS", "name": "lint"}, {"conclusion": "SKIPPED", "name": "kani"}]
    bad = [{"conclusion": "FAILURE", "name": "lint"}]
    hold_check = [{"conclusion": "FAILURE", "name": "architect hold (needs-architect label)"}] + ok

    def pr(number, **kw):
        base = {"number": number, "title": f"t{number}", "isDraft": False, "mergeStateStatus": "CLEAN",
                "autoMergeRequest": None, "labels": [], "updatedAt": ago(hours=5), "body": "",
                "statusCheckRollup": ok}
        base.update(kw)
        return base

    closed_set = {("partsfoundry/nife", 7), ("partsfoundry/basalt", 9)}
    closed = lambda slug, n: (slug, n) in closed_set
    cases = [
        ("queued, red and old", pr(1, statusCheckRollup=bad), {1}, "QUEUED", None),
        ("armed and old", pr(2, autoMergeRequest={"mergeMethod": "MERGE"}), set(), "ARMED", None),
        ("green ready, old", pr(3), set(), "IDLE", "IDLE-GREEN"),
        ("green draft, old", pr(4, isDraft=True), set(), "IDLE", "IDLE-GREEN"),
        ("fresh claim draft (checks skipped)", pr(5, isDraft=True, updatedAt=ago(minutes=5)), set(), "RUNNING", None),
        ("pending draft, 1h", pr(6, isDraft=True, updatedAt=ago(hours=1), statusCheckRollup=[{"status": "IN_PROGRESS", "name": "lint"}]), set(), "IDLE", None),
        ("pending draft, 3h", pr(8, isDraft=True, statusCheckRollup=[{"status": "IN_PROGRESS", "name": "lint"}]), set(), "IDLE", "IDLE-STALE"),
        ("red ready", pr(10, statusCheckRollup=bad), set(), "IDLE", "RED"),
        ("red draft, 3h is stale not red", pr(11, isDraft=True, statusCheckRollup=bad), set(), "IDLE", "IDLE-STALE"),
        ("architect hold alone is not red", pr(12, labels=[{"name": "needs-architect"}], statusCheckRollup=hold_check), set(), "HELD", None),
        ("held on a same-repo open pr", pr(13, isDraft=True, body="x\nHeld on #8\n"), set(), "HELD", None),
        ("held on a merged same-repo pr", pr(14, isDraft=True, body="Held on #7"), set(), "HELD", "HELD-UNBLOCKED"),
        ("held on a closed other-repo pr", pr(15, body="intro\n- Held on partsfoundry/basalt#9 (pin)\n"), set(), "HELD", "HELD-UNBLOCKED"),
        ("held on two, one done", pr(16, body="Held on #7\nHeld on partsfoundry/basalt#10"), set(), "HELD", "HELD-UNBLOCKED"),
        ("held label, no line", pr(17, labels=[{"name": "held-by-lane"}], isDraft=True), set(), "HELD", None),
        ("held line is a whole line", pr(18, body="We were Held on #7 once"), set(), "IDLE", "IDLE-GREEN"),
        ("no checks yet", pr(19, statusCheckRollup=[], updatedAt=ago(minutes=50)), set(), "IDLE", None),
        ("status context failure", pr(20, statusCheckRollup=[{"state": "FAILURE", "context": "ci"}]), set(), "IDLE", "RED"),
    ]
    failed = 0
    for label, item, queued, want_class, want_flag in cases:
        cls, flag, step = classify(item, "partsfoundry/nife", queued, closed, now)
        if (cls, flag) != (want_class, want_flag) or (flag and not step):
            print(f"idle-pull-requests selftest: {label}: got {(cls, flag)}, wanted {(want_class, want_flag)}", file=sys.stderr)
            failed += 1
    if blockers("Held on #3\nheld on a/b#4\nHeld on #3", "x/y") != [("x/y", 3), ("a/b", 4)]:
        print("idle-pull-requests selftest: blockers parse", file=sys.stderr)
        failed += 1
    # The whole sweep: exit status follows flags, and a quiet set exits 0.
    fixtures = {"partsfoundry/nife": [pr(3), pr(2, autoMergeRequest={"x": 1})], "partsfoundry/basalt": [pr(2, autoMergeRequest={"x": 1})]}
    flagged = [x for x in sweep(list(fixtures), now, fixtures.__getitem__, lambda r: set(), closed) if x[3]]
    quiet = [x for x in sweep(["partsfoundry/basalt"], now, fixtures.__getitem__, lambda r: set(), closed) if x[3]]
    if len(flagged) != 1 or quiet:
        print("idle-pull-requests selftest: sweep", file=sys.stderr)
        failed += 1
    if failed:
        return 1
    print(f"idle-pull-requests selftest: {len(cases) + 2} cases passed")
    return 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    args = [a for a in argv if not a.startswith("--")]
    if "--all-nifeos" in argv:
        args = [r["name"] for r in gh_json(["repo", "list", "partsfoundry", "--limit", "100", "--json", "name"])]
    repos = [a if "/" in a else f"partsfoundry/{a}" for a in (args or DEFAULT_REPOS)]
    now = datetime.now(timezone.utc)
    flagged, errors = 0, 0
    for repo in repos:
        try:
            for r, item, _cls, flag, step in sweep([repo], now, gh_prs, gh_queued, gh_closed):
                if flag:
                    print(format_line(r, item, flag, step))
                    flagged += 1
        except RuntimeError as err:
            print(f"ERROR           {repo}: {err}", file=sys.stderr)
            errors += 1
    if flagged:
        return 1
    return 2 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
