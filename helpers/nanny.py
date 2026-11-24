#!/usr/bin/env python3
# Provisional name, calef has not ruled on it. Poll the merge queue and exit (printing why) on the
# first event that needs the maintainer's action, or on timeout. Companion to helpers/runwatch.py:
# that one watches a single lane's dispatched runs, this one watches the queue as a whole (a
# rebase needed, a failing check, a pull request that dropped out of the queue unexpectedly).
#
# Usage: `python3 helpers/nanny.py [timeout_seconds]`, typically started with run_in_background per
# briefs/session-start.md. It polls once a minute and exits on the first batch of events, or after
# `timeout_seconds` (default 1500) with a heartbeat if nothing happened; relaunch it to keep
# watching.
#
# State lives under NIFE_WATCH_DIR (an environment variable), or a directory under the system temp
# dir when that is unset. Not beside this script: `helpers/` is checked into git and shared by every
# worktree, so a state file here would be untracked clutter at best and, across concurrent lanes and
# sessions, a shared file two processes clobber at worst.
#
# BUGS: it still calls `enqueuePullRequest` on a pull request that stays armed, CLEAN and unqueued
# across two polls, with the session's token. Since 2026-10-03 the merge drain arms and enqueues
# nothing (milestone 727 (a queue eviction goes to a maintainer session)), so this is the last
# automatic enqueuer; whether it should report instead is open.
import json, os, re, subprocess, sys, tempfile, time

REPO = "partsfoundry/nife"
D = os.environ.get("NIFE_WATCH_DIR") or os.path.join(tempfile.gettempdir(), "nife-watch")
os.makedirs(D, exist_ok=True)
p_log = os.path.join(D, "nanny-merged.log")
STATE = os.path.join(D, "nanny-state.json")
DRAFTS = os.path.join(D, "nanny-drafts.log")
SEEN = os.path.join(D, "nanny-seen.txt")

Q = ("""query{repository(owner:"partsfoundry",name:"nife"){mergeQueue(branch:"main")"""
     """{entries(first:50){nodes{position state pullRequest{number}}}}}}""")


def sh(*a):
    return subprocess.run(a, capture_output=True, text=True).stdout


def snap():
    prs = json.loads(sh("gh", "pr", "list", "-R", REPO, "--state", "open", "--limit", "100", "--json",
                         "number,title,isDraft,mergeStateStatus,autoMergeRequest,statusCheckRollup,labels"))
    q = json.loads(sh("gh", "api", "graphql", "-f", "query=" + Q) or "{}")
    queued = {n["pullRequest"]["number"]: n["state"] for n in
              (((q.get("data") or {}).get("repository") or {}).get("mergeQueue") or {}).get(
                  "entries", {}).get("nodes", [])}
    out = {}
    for p in prs:
        fails = sorted({c.get("name") or c.get("context") for c in p["statusCheckRollup"] or []
                         if (c.get("conclusion") or c.get("state")) in ("FAILURE", "ERROR", "TIMED_OUT")}
                        - {"architect hold (needs-architect label)"})
        out[str(p["number"])] = dict(
            title=p["title"][:60], draft=p["isDraft"], ms=p["mergeStateStatus"],
            armed=p["autoMergeRequest"] is not None, queued=queued.get(p["number"]), fails=fails,
            hold=bool({"needs-architect", "held-by-lane"} & {l["name"] for l in p["labels"]}),
            # The merge drain's hand-off (milestone 727 (a queue eviction goes to a maintainer session), provisional): a pull request a maintainer
            # session must pick up. Waking on it is the in-session half of briefs/session-start.md.
            nm="needs-maintainer" in [l["name"] for l in p["labels"]])
    return out


prev = json.load(open(STATE)) if os.path.exists(STATE) else None
deadline = time.time() + float(sys.argv[1] if len(sys.argv) > 1 else 1500)

while True:
    try:
        cur = snap()
    except (json.JSONDecodeError, KeyError, TypeError):
        # gh returned nothing (rate limit or network blip); try again rather than die
        time.sleep(60)
        continue
    events = []
    if prev is not None:
        for n in prev:
            if n not in cur:
                st = sh("gh", "pr", "view", n, "-R", REPO, "--json", "state", "-q", ".state").strip()
                if st == "MERGED":
                    open(p_log, "a").write(f"{time.strftime('%H:%M')} #{n} merged\n")
                else:
                    events.append(f"#{n} left the open list: {st}")
    for n, c in cur.items():
        p = (prev or {}).get(n, {})
        for f in c["fails"]:
            if f not in (p.get("fails") or []):
                events.append(f"#{n} failing: {f} ({c['title']})")
        if c.get("nm") and not p.get("nm"):
            events.append(f"#{n} labelled needs-maintainer; read the drain's comment ({c['title']})")
        if c["ms"] in ("DIRTY", "CONFLICTING") and p.get("ms") != c["ms"]:
            events.append(f"#{n} needs a rebase: {c['ms']} ({c['title']})")
        if (prev is not None and c["ms"] == "CLEAN" and c["armed"] and not c["queued"]
                and p.get("ms") == "CLEAN" and p.get("armed") and not p.get("queued")):
            pid = sh("gh", "pr", "view", n, "-R", REPO, "--json", "id", "-q", ".id").strip()
            r = sh("gh", "api", "graphql", "-f",
                   f'query=mutation{{enqueuePullRequest(input:{{pullRequestId:"{pid}"}}){{mergeQueueEntry{{position}}}}}}')
            open(p_log, "a").write(
                f"{time.strftime('%H:%M')} #{n} was armed+CLEAN but unqueued; enqueued: {r.strip()[:80]}\n")
        was = p.get("armed") or p.get("queued")
        if prev is not None and was and not c["armed"] and not c["queued"] and not c["draft"]:
            st = sh("gh", "pr", "view", n, "-R", REPO, "--json", "state", "-q", ".state").strip()
            if st == "MERGED":
                open(p_log, "a").write(f"{time.strftime('%H:%M')} #{n} merged\n")
            else:
                events.append(f"#{n} dropped out: not armed, not queued ({c['title']})")
    # Drafts belong to live lanes, which rebase and fix their own; log their
    # events for the next wake instead of waking the maintainer for each one.
    dnums = {n for n, c in cur.items() if c["draft"]}
    dev = [e for e in events if re.match(r"#(\d+) ", e) and re.match(r"#(\d+) ", e).group(1) in dnums]
    if dev:
        open(DRAFTS, "a").write("".join(f"{time.strftime('%H:%M')} {e}\n" for e in dev))
    events = [e for e in events if e not in dev]
    seen = set(open(SEEN).read().splitlines()) if os.path.exists(SEEN) else set()
    events = [e for e in events if e.rsplit(" (", 1)[0] not in seen]
    open(SEEN, "a").write("".join(e.rsplit(" (", 1)[0] + "\n" for e in events))
    json.dump(cur, open(STATE, "w"))
    if events or prev is None:
        print("\n".join(events) if events else "baseline taken")
        for n, c in sorted(cur.items()):
            print(f"  #{n:5} {'draft' if c['draft'] else 'ready'} {c['ms']:9} "
                  f"armed={int(c['armed'])} queue={c['queued'] or '-':12} "
                  f"fails={','.join(c['fails']) or '-'} {c['title']}")
        if events or (prev is None and len(sys.argv) > 2):
            sys.exit(0)
        if prev is None:
            prev = cur
            sys.exit(0)
    prev = cur
    if time.time() > deadline:
        print("heartbeat: no events")
        sys.exit(0)
    time.sleep(60)
