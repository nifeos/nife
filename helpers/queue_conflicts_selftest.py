"""Selftest for helpers/queue_conflicts.py. No network: the origin is a local bare repository.

Run `python3 helpers/queue_conflicts.py --selftest`; script/lint-fleet runs it.

The fixture is a repository with `main` and several branches off its first commit. Queue entries
are given as the GraphQL response `mergeQueue.entries.nodes` would hold, in a JSON file.

**Falsification.** `test_falsified` replaces the module's `merge_tree` with a no-op that reports
no conflicts and requires the conflict case to FAIL under it. A selftest that still passes with
the merge removed checks nothing, which is the failure the first draft of this file would have
had if it only asserted exit codes of fixtures chosen not to conflict.
"""

import contextlib
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile

ENV = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
ENV.update(GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1", GIT_AUTHOR_NAME="t",
           GIT_AUTHOR_EMAIL="t@example.invalid", GIT_COMMITTER_NAME="t",
           GIT_COMMITTER_EMAIL="t@example.invalid")


def sh(cwd, *args):
    return subprocess.run(args, cwd=cwd, env=ENV, check=True, capture_output=True,
                          text=True).stdout.strip()


class Fixture:
    def __init__(self):
        self.root = tempfile.mkdtemp(prefix="queue-conflicts-selftest.")
        self.origin = os.path.join(self.root, "origin.git")
        self.work = os.path.join(self.root, "work")
        sh(self.root, "git", "init", "-q", "--bare", "-b", "main", self.origin)
        sh(self.root, "git", "-C", self.origin, "config", "uploadpack.allowAnySHA1InWant", "true")
        sh(self.root, "git", "init", "-q", "-b", "main", self.work)
        for name in "abc":
            self.write(name, f"{name}1\n{name}2\n{name}3\n")
        self.commit("base")
        self.base = sh(self.work, "git", "rev-parse", "HEAD")
        sh(self.work, "git", "remote", "add", "origin", self.origin)
        sh(self.work, "git", "push", "-q", "origin", "main")

    def write(self, name, text):
        with open(os.path.join(self.work, name), "w") as fh:
            fh.write(text)

    def commit(self, msg):
        sh(self.work, "git", "add", "-A")
        sh(self.work, "git", "commit", "-q", "-m", msg)

    def branch(self, name, files):
        """A commit off the base that rewrites `files` ({name: text}); returns its sha."""
        sh(self.work, "git", "checkout", "-q", "-B", name, self.base)
        for fname, text in files.items():
            self.write(fname, text)
        self.commit(name)
        sha = sh(self.work, "git", "rev-parse", "HEAD")
        sh(self.work, "git", "push", "-q", "origin", f"{name}:refs/heads/{name}")
        return sha

    def advance_main(self, files):
        sh(self.work, "git", "checkout", "-q", "main")
        for fname, text in files.items():
            self.write(fname, text)
        self.commit("main moves")
        sh(self.work, "git", "push", "-q", "origin", "main")

    def entries(self, *numbered):
        """Write the queue response: `numbered` is (number, sha) in queue order."""
        nodes = [{"position": i + 1, "state": "QUEUED",
                  "pullRequest": {"number": n, "headRefOid": sha}}
                 for i, (n, sha) in enumerate(numbered)]
        path = os.path.join(self.root, "queue.json")
        with open(path, "w") as fh:
            json.dump({"data": {"repository": {"mergeQueue": {"entries": {"nodes": nodes}}}}}, fh)
        return path

    def run(self, mod, pr, head, queue, *extra):
        out, err = io.StringIO(), io.StringIO()
        argv = ["check", "--pr", str(pr), "--head", head, "--entries-json", queue,
                "--remote", self.origin, "--repo-dir", os.path.join(self.work, ".git"), *extra]
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = mod.main(argv)
        return code, out.getvalue() + err.getvalue()

    def close(self):
        shutil.rmtree(self.root, ignore_errors=True)


LINES = "{n}1\nCHANGED {x}\n{n}3\n"


def edit(name, tag):
    return LINES.format(n=name, x=tag)


def test_conflict_with_entry_ahead(mod):
    fx = Fixture()
    try:
        ahead = fx.branch("ahead", {"a": edit("a", "ahead")})
        mine = fx.branch("mine", {"a": edit("a", "mine")})
        code, text = fx.run(mod, 2, mine, fx.entries((1, ahead)))
        assert code == 1, (code, text)
        assert "a" in text and "#1" in text, text
    finally:
        fx.close()


def test_clean_and_ordering(mod):
    fx = Fixture()
    try:
        ahead = fx.branch("ahead", {"a": edit("a", "ahead")})
        behind = fx.branch("behind", {"a": edit("a", "behind")})
        mine = fx.branch("mine", {"c": edit("c", "mine")})
        queue = fx.entries((1, ahead), (2, mine), (3, behind))
        code, text = fx.run(mod, 2, mine, queue)
        assert code == 0, (code, text)
        # An entry behind ours in the queue is not ahead of it: this one touches `a` as `ahead` does.
        mine_a = fx.branch("mine_a", {"a": edit("a", "mine")})
        queue = fx.entries((1, ahead), (2, mine_a), (3, behind))
        code, text = fx.run(mod, 2, mine_a, queue)
        assert code == 1 and "#1" in text and "#3" not in text, (code, text)
        # Not queued at all: everything in the queue is ahead.
        code, text = fx.run(mod, 9, mine_a, fx.entries((1, ahead)))
        assert code == 1, (code, text)
    finally:
        fx.close()


def test_conflict_with_main_alone(mod):
    fx = Fixture()
    try:
        mine = fx.branch("mine", {"b": edit("b", "mine")})
        fx.advance_main({"b": edit("b", "main")})
        code, text = fx.run(mod, 5, mine, fx.entries())
        assert code == 1 and "main" in text, (code, text)
    finally:
        fx.close()


def test_entry_that_conflicts_is_left_out(mod):
    fx = Fixture()
    try:
        first = fx.branch("first", {"a": edit("a", "first")})
        second = fx.branch("second", {"a": edit("a", "second"), "b": edit("b", "second")})
        mine = fx.branch("mine", {"b": edit("b", "mine")})
        code, text = fx.run(mod, 3, mine, fx.entries((1, first), (2, second)))
        # `second` conflicts with `first`, so the queue would drop it and `mine` would merge.
        assert code == 0 and "#2" in text and "already conflicts" in text, (code, text)
    finally:
        fx.close()


def test_advisory(mod):
    fx = Fixture()
    try:
        ahead = fx.branch("ahead", {"a": edit("a", "ahead")})
        mine = fx.branch("mine", {"a": edit("a", "mine")})
        code, text = fx.run(mod, 2, mine, fx.entries((1, ahead)), "--advisory")
        assert code == 0 and "CONFLICTS" in text, (code, text)
        clean = fx.branch("clean", {"c": edit("c", "clean")})
        code, text = fx.run(mod, 3, clean, fx.entries((1, ahead)), "--advisory")
        assert code == 0 and text == "", (code, text)
        code, text = fx.run(mod, 3, clean, os.path.join(fx.root, "missing.json"), "--advisory")
        assert code == 0 and text == "", (code, text)
        code, text = fx.run(mod, 3, clean, os.path.join(fx.root, "missing.json"))
        assert code == 2, (code, text)
    finally:
        fx.close()


def test_shared(mod):
    scopes = {"one": ["a.rs", "notes/names.md", "dir/"], "two": ["b.rs", "notes/names.md"],
              "three": ["dir/x.rs", "c.rs"]}
    found = mod.shared_paths(scopes)
    assert found == {"notes/names.md": ["one", "two"], "dir/x.rs": ["one", "three"]}, found
    assert mod.shared_paths({"one": ["a"], "two": ["b"]}) == {}
    work = tempfile.mkdtemp(prefix="queue-conflicts-scopes.")
    try:
        paths = []
        for lane, files in {"x": ["f", "g"], "y": ["g"]}.items():
            paths.append(os.path.join(work, lane))
            with open(paths[-1], "w") as fh:
                fh.write("# scope\n" + "\n".join(files) + "\n")
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            assert mod.main(["shared", *paths]) == 1
        assert "g  (x, y)" in out.getvalue(), out.getvalue()
        with contextlib.redirect_stdout(io.StringIO()):
            assert mod.main(["shared", paths[0]]) == 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


def test_falsified(mod):
    """With merge_tree replaced by a no-op that finds no conflict, the conflict test must fail."""
    real = mod.merge_tree
    mod.merge_tree = lambda gitdir, left, right: ("0" * 40, [])
    try:
        try:
            test_conflict_with_entry_ahead(mod)
        except AssertionError:
            return
        raise SystemExit("queue_conflicts selftest: passes with merge-tree replaced by a no-op")
    finally:
        mod.merge_tree = real


def run(mod):
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    for fn in tests:
        fn(mod)
    print(f"queue_conflicts selftest: {len(tests)} tests passed")
    return 0
