#!/usr/bin/env python3
"""helpers/fuzz-follow-main.py: the continuous fuzz host follows main, within four limits.

    python3 -I helpers/fuzz-follow-main.py look         print the commit to move to, or say why not
    python3 -I helpers/fuzz-follow-main.py apply SHA    move the clone to SHA, which `look` named
    python3 -I helpers/fuzz-follow-main.py --selftest   the green rule against fixtures, no network

The supervisor (helpers/continuous-fuzz.sh) runs `look` at most every 5 minutes. When `look` names a
commit, the supervisor lets every slot finish its current window, runs `apply`, and re-executes
itself from the moved clone; the new supervisor moves each slot worktree to the clone's commit before
any slot starts. An architect ruled the four limits on 2026-11-01 (UTC) in
design/roadmap/1015-fuzz-findings-without-a-person.md ("What is in force: the host follows
main"), and each is enforced here:

  1. Fast-forward only. `look` and `apply` both refuse a commit whose history does not contain the
     commit the clone is at (`git merge-base --is-ancestor`).
  2. Only the main branch of the public nifeos/nife repository, over https. The address and the ref
     are constants below, not settings. The fetch allows the https protocol alone and asks no
     credential helper, and `apply` moves only to the commit that fetch wrote.
  3. Only a commit whose checks on main are green, read from the public API with no credential.
     `required_checks` reads the names of main's required checks from the branch's rules, and
     `verdict` calls the commit green only when every one of them has a successful run by GitHub
     Actions on that exact commit and none has a run that is unfinished or failed. Anything else,
     including an error, a rate limit or an unexpected response, keeps the current commit.
  4. Unit files are never applied. When `apply` moves past a change under helpers/systemd/, it
     prints one line naming the changed files and saying that a person with root must install them.

A commit made from a batch of the merge queue carries the gates' results on its last commit only;
the commits before it carry skipped runs alone, so they are never green. Main's tip is always the
last commit of a batch, so this costs nothing in practice.

Rate: a look whose fetch finds nothing new makes no API request. A look at a new commit makes one
request for the rules and one per 100 check runs (two for a commit of 125 runs), against GitHub's
documented 60 unauthenticated requests an hour per address.

Exit status: `look` exits 0 and prints the full commit on standard output only when the clone should
move; otherwise it exits 1 and prints one line saying why on standard error. `apply` exits 0 when
the clone moved and 1 when it refused. 2 is a usage error.

Decided by the maintainer, provisional; reversing costs one pull request: the required checks are
read live from main's rules rather than from a list kept in the tree, a check run counts only when
GitHub Actions made it, and a check whose runs were all skipped is not green.

Name: provisional, minted 2026-11-02 (UTC) by lane/the-fuzz-host-follows-main. No architect has
ruled on it.
"""

import json
import os
import re
import subprocess
import sys

UPSTREAM = "https://github.com/nifeos/nife.git"
API = "https://api.github.com/repos/nifeos/nife"
FOLLOW_REF = "refs/fuzz-follow/main"
UNIT_DIR = "helpers/systemd/"
PER_PAGE = 100
MAX_PAGES = 5
ACTIONS_APP = "github-actions"
# A completed run with one of these conclusions passes; GitHub treats a skipped or neutral required
# check as satisfied, so a skipped duplicate (the push-triggered run of a merge-queue gate) does not
# block. A pass still needs at least one `success`.
PASSING = {"success", "skipped", "neutral"}
SHA_RE = re.compile(r"^[0-9a-f]{40}$")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


class Stay(Exception):
    """The clone keeps its commit; the message says why."""


def git(*args):
    proc = subprocess.run(["git", "-C", ROOT, *args], capture_output=True, text=True)
    if proc.returncode != 0:
        verb = next((a for a in args if not a.startswith("-") and "=" not in a), "")
        last = (proc.stderr.strip().splitlines() or [f"exit {proc.returncode}"])[-1]
        raise Stay(f"git {verb} failed: {last}")
    return proc.stdout.strip()


def is_ancestor(old, new):
    proc = subprocess.run(["git", "-C", ROOT, "merge-base", "--is-ancestor", old, new],
                          capture_output=True, text=True)
    return proc.returncode == 0


def fetch_main():
    """Limit 2: main of the public repository, over https, with no credential."""
    git("-c", "protocol.allow=never", "-c", "protocol.https.allow=always", "-c", "credential.helper=",
        "fetch", "--quiet", "--no-tags", "--no-recurse-submodules", UPSTREAM,
        f"+refs/heads/main:{FOLLOW_REF}")
    return git("rev-parse", "--verify", f"{FOLLOW_REF}^{{commit}}")


def get_json(path):
    """One public API request with no credential. `-q` first, so no curlrc can add a header."""
    proc = subprocess.run(
        ["curl", "-q", "-fsS", "--proto", "=https", "--max-time", "30",
         "-H", "Accept: application/vnd.github+json", "-H", "X-GitHub-Api-Version: 2022-11-28",
         f"{API}/{path}"],
        capture_output=True, text=True)
    if proc.returncode != 0:
        raise Stay(f"the API did not answer {path.split('?')[0]} (curl exit {proc.returncode}: "
                   f"{proc.stderr.strip()[:200]})")
    try:
        return json.loads(proc.stdout)
    except ValueError:
        raise Stay(f"the API answer to {path.split('?')[0]} is not JSON")


def required_checks(rules):
    """Main's required checks from the branch's rules: (name, App id or None)."""
    if not isinstance(rules, list):
        raise Stay("main's rules are unreadable")
    out = []
    for rule in rules:
        if isinstance(rule, dict) and rule.get("type") == "required_status_checks":
            for check in (rule.get("parameters") or {}).get("required_status_checks") or []:
                if isinstance(check, dict) and isinstance(check.get("context"), str):
                    out.append((check["context"], check.get("integration_id")))
    if not out:
        raise Stay("main's rules name no required check, so nothing can show the commit green")
    return out


def check_runs(sha):
    first = get_json(f"commits/{sha}/check-runs?per_page={PER_PAGE}&page=1")
    total = first.get("total_count") if isinstance(first, dict) else None
    if not isinstance(total, int) or total < 0:
        raise Stay("the check runs are unreadable")
    pages = (total + PER_PAGE - 1) // PER_PAGE
    if pages > MAX_PAGES:
        raise Stay(f"the commit has {total} check runs, more than {MAX_PAGES} pages")
    runs = list(first.get("check_runs") or [])
    for page in range(2, pages + 1):
        more = get_json(f"commits/{sha}/check-runs?per_page={PER_PAGE}&page={page}")
        runs.extend((more.get("check_runs") or []) if isinstance(more, dict) else [])
    if len(runs) != total:
        raise Stay(f"read {len(runs)} check runs of {total}; the list moved while it was read")
    return runs


def verdict(sha, required, runs):
    """Limit 3. Returns None when green, else the reason the clone stays."""
    by_name = {}
    for run in runs:
        if not isinstance(run, dict):
            return "a check run is unreadable"
        if run.get("head_sha") != sha:
            return f"a check run names commit {str(run.get('head_sha'))[:12]}, not {sha[:12]}"
        by_name.setdefault(run.get("name"), []).append(run)
    for name, app_id in required:
        mine = []
        for run in by_name.get(name, []):
            app = run.get("app") or {}
            if app_id is None and app.get("slug") != ACTIONS_APP:
                continue
            if app_id is not None and app.get("id") != app_id:
                continue
            mine.append(run)
        if not mine:
            return f"required check '{name}' has no run on {sha[:12]} yet"
        for run in mine:
            if run.get("status") != "completed":
                return f"required check '{name}' is {run.get('status')} on {sha[:12]}"
            if run.get("conclusion") not in PASSING:
                return f"required check '{name}' concluded {run.get('conclusion')} on {sha[:12]}"
        if not any(run.get("conclusion") == "success" for run in mine):
            return f"required check '{name}' has no successful run on {sha[:12]}"
    return None


def clean_or_stay():
    if git("status", "--porcelain", "--untracked-files=no"):
        raise Stay("the clone has local changes to tracked files")


def look():
    current = git("rev-parse", "HEAD")
    clean_or_stay()
    tip = fetch_main()
    if tip == current:
        raise Stay(f"main is at {current[:12]}, where the clone already is")
    if not is_ancestor(current, tip):
        raise Stay(f"main's {tip[:12]} does not contain the clone's {current[:12]}; not a fast-forward")
    reason = verdict(tip, required_checks(get_json("rules/branches/main")), check_runs(tip))
    if reason:
        raise Stay(reason)
    return tip


def apply(sha):
    if not SHA_RE.match(sha):
        raise Stay(f"'{sha}' is not a full commit id")
    tip = git("rev-parse", "--verify", f"{FOLLOW_REF}^{{commit}}")
    if sha != tip:
        raise Stay(f"{sha[:12]} is not the main commit the last look fetched ({tip[:12]})")
    current = git("rev-parse", "HEAD")
    if not is_ancestor(current, sha):
        raise Stay(f"{sha[:12]} does not contain the clone's {current[:12]}; not a fast-forward")
    clean_or_stay()
    units = git("diff", "--name-only", current, sha, "--", UNIT_DIR).split()
    git("checkout", "--quiet", "--detach", sha)
    print(f"fuzz-follow-main: moved the clone from {current[:12]} to {sha[:12]}")
    if units:
        print(f"fuzz-follow-main: the unit files changed ({', '.join(units)}); nothing was installed, "
              f"and a person with root must install them (notes/fuzzing/continuous-host.md)")


def selftest():
    sha = "a" * 40
    required = [("lint", None), ("test", None)]

    def run(name, status="completed", conclusion="success", app="github-actions", head=sha):
        return {"name": name, "status": status, "conclusion": conclusion, "head_sha": head,
                "app": {"slug": app, "id": 15368}}

    cases = [
        ("green", [run("lint"), run("test")], True),
        ("green beside a skipped duplicate", [run("lint"), run("lint", conclusion="skipped"),
                                              run("test")], True),
        ("a red check", [run("lint"), run("test", conclusion="failure")], False),
        ("a cancelled duplicate", [run("lint"), run("lint", conclusion="cancelled"), run("test")],
         False),
        ("a pending check", [run("lint"), run("test", status="in_progress", conclusion=None)], False),
        ("a missing check", [run("lint")], False),
        ("skipped alone", [run("lint"), run("test", conclusion="skipped")], False),
        ("success from another App", [run("lint"), run("test", app="someone-else")], False),
        ("a run on another commit", [run("lint"), run("test", head="b" * 40)], False),
    ]
    for label, runs, green in cases:
        if (verdict(sha, required, runs) is None) != green:
            print(f"fuzz-follow-main selftest: {label}: wanted {'green' if green else 'stay'}",
                  file=sys.stderr)
            return 1
    for label, rules in [("no rules", []), ("not a list", {"message": "API rate limit exceeded"}),
                         ("no required check", [{"type": "pull_request"}])]:
        try:
            required_checks(rules)
        except Stay:
            continue
        print(f"fuzz-follow-main selftest: {label}: rules were accepted", file=sys.stderr)
        return 1
    pinned = required_checks([{"type": "required_status_checks", "parameters": {
        "required_status_checks": [{"context": "test", "integration_id": 99}]}}])
    if verdict(sha, pinned, [run("test")]) is None:
        print("fuzz-follow-main selftest: a check pinned to App 99 passed on another App's run",
              file=sys.stderr)
        return 1
    print("fuzz-follow-main selftest: ok")
    return 0


def main(argv):
    if argv == ["--selftest"]:
        return selftest()
    try:
        if argv == ["look"]:
            print(look())
            return 0
        if len(argv) == 2 and argv[0] == "apply":
            apply(argv[1])
            return 0
    except Stay as stay:
        print(f"fuzz-follow-main: {stay}", file=sys.stderr)
        return 1
    except Exception as error:  # fail closed: any surprise keeps the current commit
        print(f"fuzz-follow-main: staying, unexpected {type(error).__name__}: {error}",
              file=sys.stderr)
        return 1
    print(__doc__.split("\n\n")[1], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
