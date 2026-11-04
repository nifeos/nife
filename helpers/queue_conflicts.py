#!/usr/bin/env python3
"""Catch a pull request that conflicts with the merge queue before it is armed.

**Why this exists.** On 2026-11-03 the queue removed 24 entries for merge conflicts, 14 of them in
one minute, and each removal cost the pull request its place and a full run of checks. GitHub had
kept 13 entries it already knew were UNMERGEABLE until a merge moved `main`. The conflicts were
knowable before any arm: `git merge-tree` answers in milliseconds, and the queue lists what is
ahead. See notes/coes/2026-11-03-merge-queue-backlog.md, action 6, and notes/merge-queue.md.

Two commands, both reached through `script/queue-conflicts`:

- `check`: merge a pull request's head against `main` plus every entry ahead of it in the queue,
  and say which files conflict and which entries ahead touch them. Exit 0 clean, 1 conflict,
  2 could not tell (no queue, no network, bad arguments). `--advisory` prints but always exits 0,
  and keeps quiet when nothing is wrong; `script/push` uses it.
- `shared`: given the scope files of a planned set of lanes, list every path two lanes would both
  write. Exit 1 when there is one. This is the launch check of briefs/run-a-milestone-wide.md.

**It touches nothing of the caller's.** The merges happen in a scratch bare repository that borrows
the caller's objects through `objects/info/alternates`. Nothing is written to a worktree, a branch
or a ref of the caller's repository, and the scratch directory is removed afterwards.

**The tip.** Entries are folded in by queue position, each by `git merge-tree --write-tree` against
the running tip, whose result is committed with two parents so the next merge sees it. An entry
that does not merge cleanly into the tip is reported and left out of the tip, which is what the
queue itself would do by removing it. The pull request is then merged against the final tip. A
conflict there is attributed: each conflicting file is listed with the entries ahead that changed it
since they left `main`, and `main` itself when the pull request conflicts with `main` alone.

Name: provisional. Run `python3 helpers/queue_conflicts.py --selftest` for the tests.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile

REPO_DEFAULT = "nifeos/nife"

QUERY = """query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    mergeQueue(branch: "main") { entries(first: 100) { nodes {
      position state pullRequest { number headRefOid } } } }
  }
}"""


def git(gitdir, *args, check=True, input_text=None):
    """Run git against a scratch repository, with no ambient configuration."""
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    env.update(GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1", GIT_TERMINAL_PROMPT="0",
               GIT_AUTHOR_NAME="queue-conflicts", GIT_AUTHOR_EMAIL="queue-conflicts@localhost",
               GIT_COMMITTER_NAME="queue-conflicts", GIT_COMMITTER_EMAIL="queue-conflicts@localhost")
    for key in ("GIT_SSH", "GIT_SSH_COMMAND"):
        if key in os.environ:
            env[key] = os.environ[key]
    cmd = ["git"]
    if gitdir:
        cmd += ["--git-dir", gitdir]
    proc = subprocess.run(cmd + list(args), capture_output=True, text=True, env=env,
                          input=input_text)
    if check and proc.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {proc.stderr.strip()}")
    return proc


def entries_ahead(entries, pr_number):
    """The entries ahead of `pr_number` (all of them when it is not queued), by position."""
    nodes = sorted((e for e in entries if e.get("pullRequest")), key=lambda e: e["position"])
    mine = [e for e in nodes if e["pullRequest"]["number"] == pr_number]
    limit = mine[0]["position"] if mine else None
    return [e["pullRequest"] for e in nodes
            if e["pullRequest"]["number"] != pr_number and (limit is None or e["position"] < limit)]


def merge_tree(gitdir, left, right):
    """`(tree, conflicted_files)`; files is empty on a clean merge."""
    proc = git(gitdir, "merge-tree", "--write-tree", "--name-only", "-z", left, right, check=False)
    if proc.returncode not in (0, 1):
        raise RuntimeError(f"git merge-tree: {proc.stderr.strip()}")
    # -z: the tree id, then NUL-separated conflicted names, then an empty field, then messages.
    fields = proc.stdout.split("\0")
    tree = fields[0].strip()
    files = []
    if proc.returncode == 1:
        for name in fields[1:]:
            if name == "":
                break
            files.append(name)
    return tree, files


def touched(gitdir, base, head):
    """Files `head` changed since it left `base`."""
    mb = git(gitdir, "merge-base", base, head, check=False).stdout.strip()
    if not mb:
        return set()
    return set(git(gitdir, "diff", "--name-only", mb, head).stdout.split("\n")) - {""}


def evaluate(gitdir, main, ahead, head):
    """Fold `ahead` (a list of `{number, headRefOid}`) onto `main`, then merge `head` into that.

    Returns a dict: `conflicts` (file -> entries ahead that touch it, `main` for main alone),
    `skipped` (entries ahead that did not merge into the tip, with files).
    """
    tip = main
    skipped = []
    folded = []
    for entry in ahead:
        tree, files = merge_tree(gitdir, tip, entry["headRefOid"])
        if files:
            skipped.append({"number": entry["number"], "files": files})
            continue
        tip = git(gitdir, "commit-tree", tree, "-p", tip, "-p", entry["headRefOid"],
                  "-m", f"queue #{entry['number']}").stdout.strip()
        folded.append(entry)
    _, files = merge_tree(gitdir, tip, head)
    conflicts = {}
    if files:
        _, against_main = merge_tree(gitdir, main, head)
        for name in files:
            who = [f"#{e['number']}" for e in folded
                   if name in touched(gitdir, main, e["headRefOid"])]
            if name in against_main:
                who.insert(0, "main")
            conflicts[name] = who
    return {"conflicts": conflicts, "skipped": skipped}


def scratch_repo(source_gitdir, remote, main_ref, shas):
    """A bare scratch repository holding main and `shas`, borrowing `source_gitdir`'s objects."""
    work = tempfile.mkdtemp(prefix="queue-conflicts.")
    gitdir = os.path.join(work, "scratch.git")
    git(None, "init", "-q", "--bare", gitdir)
    if source_gitdir:
        info = os.path.join(gitdir, "objects", "info")
        os.makedirs(info, exist_ok=True)
        with open(os.path.join(info, "alternates"), "w") as fh:
            fh.write(os.path.join(os.path.abspath(source_gitdir), "objects") + "\n")
    git(gitdir, "fetch", "-q", "--no-tags", remote, f"+{main_ref}:refs/scratch/main")
    for sha in sorted(set(shas)):
        have = git(gitdir, "cat-file", "-e", f"{sha}^{{commit}}", check=False)
        if have.returncode != 0:
            git(gitdir, "fetch", "-q", "--no-tags", remote, sha)
    return work, gitdir


def gh_json(args):
    proc = subprocess.run(["gh"] + args, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(f"gh {' '.join(args)}: {proc.stderr.strip()}")
    return json.loads(proc.stdout)


def load_entries(repo, entries_file):
    if entries_file:
        with open(entries_file) as fh:
            data = json.load(fh)
    else:
        owner, name = repo.split("/")
        data = gh_json(["api", "graphql", "-f", f"query={QUERY}", "-F", f"owner={owner}",
                        "-F", f"name={name}"])
    nodes = data["data"]["repository"]["mergeQueue"]
    return [] if nodes is None else nodes["entries"]["nodes"]


def report(result, number, out):
    for entry in result["skipped"]:
        print(f"queue-conflicts: note: #{entry['number']}, ahead of #{number}, already conflicts with "
              f"the entries before it ({', '.join(entry['files'])}); the queue will drop it, "
              f"so it is left out of the merge below.", file=out)
    if not result["conflicts"]:
        return
    print(f"queue-conflicts: #{number} CONFLICTS with the queue. Arming it would cost its place and a "
          f"full run of checks.", file=out)
    for name, who in sorted(result["conflicts"].items()):
        print(f"  {name}  (changed by {', '.join(who) if who else 'the merge of several entries'})",
              file=out)
    print("queue-conflicts: merge main in, resolve, push, and run this again. "
          "Arm only when it is clean.", file=out)


def cmd_check(args):
    repo_root = args.repo_dir or git(None, "rev-parse", "--git-common-dir", check=False).stdout.strip() or None
    remote = args.remote or "origin"
    url = subprocess.run(["git", "remote", "get-url", remote], capture_output=True, text=True)
    if url.returncode == 0:
        remote = url.stdout.strip()
    try:
        number = args.pr
        head = args.head
        if number is None or head is None:
            which = ["-R", args.repo, str(number)] if number else []
            view = gh_json(["pr", "view"] + which + ["--json", "number,headRefOid,state,isDraft"])
            if view.get("state") != "OPEN":
                if not args.advisory:
                    print(f"queue-conflicts: #{view['number']} is {view.get('state')}.", file=sys.stderr)
                return 0 if args.advisory else 2
            if args.advisory and view.get("isDraft"):
                return 0
            number = number or view["number"]
            head = head or view["headRefOid"]
        ahead = entries_ahead(load_entries(args.repo, args.entries_json), number)
    except (RuntimeError, OSError, KeyError, ValueError) as err:
        if not args.advisory:
            print(f"queue-conflicts: could not read the queue ({err}); "
                  f"nothing was checked.", file=sys.stderr)
        return 0 if args.advisory else 2
    work = None
    try:
        work, gitdir = scratch_repo(repo_root, remote, args.main_ref,
                                    [e["headRefOid"] for e in ahead] + [head])
        result = evaluate(gitdir, "refs/scratch/main", ahead, head)
    except (RuntimeError, OSError) as err:
        if not args.advisory:
            print(f"queue-conflicts: could not run the merges ({err}).", file=sys.stderr)
        return 0 if args.advisory else 2
    finally:
        if work:
            shutil.rmtree(work, ignore_errors=True)
    bad = bool(result["conflicts"])
    if bad or not args.advisory:
        report(result, number, sys.stderr if args.advisory else sys.stdout)
    if not bad and not args.advisory:
        print(f"queue-conflicts: #{number} merges cleanly with main and the {len(ahead)} "
              f"entries ahead of it.")
    return 1 if bad and not args.advisory else 0


def overlap(a, b):
    """Two scope entries overlap when equal, or when one is a directory (trailing `/`) holding the other."""
    if a == b:
        return True
    return (a.endswith("/") and b.startswith(a)) or (b.endswith("/") and a.startswith(b))


def read_scope(path):
    with open(path) as fh:
        return [ln.strip() for ln in fh if ln.strip() and not ln.lstrip().startswith("#")]


def shared_paths(scopes):
    """`scopes`: lane -> list of paths. Returns {path: [lanes]} for every path two lanes both write."""
    found = {}
    lanes = sorted(scopes)
    for i, one in enumerate(lanes):
        for two in lanes[i + 1:]:
            for a in scopes[one]:
                for b in scopes[two]:
                    if overlap(a, b):
                        path = a if len(a) >= len(b) else b
                        found.setdefault(path, set()).update((one, two))
    return {p: sorted(v) for p, v in found.items()}


def cmd_shared(args):
    scopes = {os.path.basename(p): read_scope(p) for p in args.scope_files}
    if len(scopes) != len(args.scope_files):
        print("queue-conflicts: two scope files share a name; lanes are named by file name.", file=sys.stderr)
        return 2
    found = shared_paths(scopes)
    if not found:
        print(f"queue-conflicts: {len(scopes)} lanes, no path in two scopes.")
        return 0
    print("queue-conflicts: these paths are in more than one lane's scope. Give each one owner, or a "
          "layout in which each lane has its own lines, before the first claim:")
    for path, lanes in sorted(found.items()):
        print(f"  {path}  ({', '.join(lanes)})")
    return 1


def main(argv):
    if "--selftest" in argv:
        return selftest()
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check", help="merge a pull request against main and the entries ahead of it")
    c.add_argument("--pr", type=int, help="pull request number (default: the current branch's)")
    c.add_argument("--head", help="head commit to test (default: the pull request's head on GitHub)")
    c.add_argument("--repo", default=REPO_DEFAULT)
    c.add_argument("--remote", help="git remote or path holding main and the heads (default: origin)")
    c.add_argument("--main-ref", default="refs/heads/main")
    c.add_argument("--repo-dir", help="git directory whose objects the scratch repository borrows")
    c.add_argument("--entries-json", help="read the queue from this GraphQL response instead of GitHub")
    c.add_argument("--advisory", action="store_true", help="print only a conflict; always exit 0")
    s = sub.add_parser("shared", help="list paths two planned lanes would both write")
    s.add_argument("scope_files", nargs="+", help="one file per lane, one path per line")
    args = parser.parse_args(argv)
    return cmd_check(args) if args.cmd == "check" else cmd_shared(args)


def selftest():
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import queue_conflicts_selftest
    return queue_conflicts_selftest.run(sys.modules[__name__])


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
