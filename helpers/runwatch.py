#!/usr/bin/env python3
# Provisional name, calef has not ruled on it. Watch the CI runs that lanes are WAITING on
# (briefs/session-start.md, briefs/gate-in-ci.md): a lane ends its turn with one line,
# `WAITING [<owner>/<repo>:]<run_id> [...] on <what>`, and this script is what turns that line into a wake-up
# instead of a lane nobody is watching for.
#
# Usage: the maintainer appends one line per run to runs.txt, "<run> <lane_agent_id> <label>", where
# <run> is the WAITING token copied verbatim: a bare `<run_id>` means nifeos/nife, and
# `<owner>/<repo>:<run_id>` (nifeos/basalt:123, nifeos/crate-workflows:456) names another
# repository. The repository rides in the first field, not a trailing `repo=` key, because <label>
# is free text with spaces and a trailing key would be ambiguous with it. Then starts (or leaves running, with run_in_background) `python3 helpers/runwatch.py [timeout]`.
# It polls every two minutes and exits (so the maintainer wakes) the moment a lane's runs are ALL
# finished, or ANY of them did not succeed; fired lines move to runs-done.txt with their conclusions
# and everything else stays in runs.txt so a relaunch keeps watching the rest. `timeout` (seconds,
# default 7200) bounds a heartbeat exit when nothing is ready yet, so a background poll does not run
# forever unattended.
#
# State lives under a directory this script does not itself pick a fixed answer for, on purpose:
# `helpers/` is checked into git and shared by every worktree, so a state file "beside the script"
# would be either untracked clutter in a tracked directory or, worse, shared and clobbered across
# concurrent lanes and sessions. NIFE_WATCH_DIR (an environment variable) names it; unset, it falls
# back to a directory under the system temp dir, which is stable per machine and never inside the
# repository, so this script works the same run from the repo root, a worktree, or a cron job.
import subprocess, time, os, sys, json, tempfile

D = os.environ.get("NIFE_WATCH_DIR") or os.path.join(tempfile.gettempdir(), "nife-watch")
os.makedirs(D, exist_ok=True)
F = os.path.join(D, "runs.txt")
DONE = os.path.join(D, "runs-done.txt")

deadline = time.time() + float(sys.argv[1] if len(sys.argv) > 1 else 7200)


DEFAULT_REPO = "nifeos/nife"


def split_run(token):
    repo, sep, run_id = token.rpartition(":")
    return (repo, run_id) if sep else (DEFAULT_REPO, token)


def status(token):
    repo, run_id = split_run(token)
    r = subprocess.run(["gh", "run", "view", run_id, "-R", repo, "--json",
                         "status,conclusion"], capture_output=True, text=True).stdout
    try:
        j = json.loads(r)
    except Exception:
        return None
    return (j.get("conclusion") or "?") if j["status"] == "completed" else None


while True:
    lines = [l.split(None, 2) for l in open(F).read().splitlines() if l.strip()] \
        if os.path.exists(F) else []
    st = {l[0]: status(l[0]) for l in lines}
    lanes = {}
    for l in lines:
        lanes.setdefault(l[1], []).append(l)
    fire = [lane for lane, ls in lanes.items()
            if all(st[l[0]] for l in ls) or any(st[l[0]] not in (None, "success", "skipped") for l in ls)]
    if fire:
        out = [l for l in lines if l[1] in fire]
        open(F, "w").write("".join(" ".join(l) + "\n" for l in lines if l[1] not in fire))
        open(DONE, "a").write(
            "".join(f"{' '.join(l)} {st[l[0]] or 'running'}\n" for l in out))
        print("\n".join(f"lane {l[1]} run {l[0]} {st[l[0]] or 'running'} {l[2] if len(l) > 2 else ''}"
                        for l in out))
        sys.exit(0)
    if time.time() > deadline:
        print("heartbeat: no lane ready")
        sys.exit(0)
    time.sleep(120)
