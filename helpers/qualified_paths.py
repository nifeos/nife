"""Resolve a record's `nifeos/<repo>:<path>` citations at the commit basalt's manifest pins.

A record cites a path in another repository as `nifeos/<repo>:<path>`, the form milestone 1018 (the
design record moves to `nifeos/record` and is published as a website) adds beside §201 (one
roadmap until a citation has to cross)'s `nifeos/<repo>@<sha>`. The path means that file at the
commit basalt builds, so the record keeps no second list of commits. `script/roadmap --check`
calls `problems()` on every roadmap block and fails a citation that names no file.

Which commit a repository resolves at:

- `nifeos/record` is the record itself: the path is read in the tree that runs the check. That is
  this repository until the record moves, and `nifeos/record` after it.
- A repository that `pins.toml` on basalt's `main` names is read at the commit it pins.
- Any other `nifeos` repository is read at every revision a pinned component's `Cargo.lock` files
  name for it, at that component's pinned commit. A crate such as `nifeos/jh7110_entropy` is no
  basalt component, but the nife commit basalt pins locks it at one revision, and that revision is
  what the image builds. The path has to exist at each revision named, so a crate locked at two
  revisions cannot hide a path that one of them lacks.
- A repository that nothing pins is a problem: there is no commit to read the path at.

A span is a citation only when its path holds no `<`, `>` or whitespace and is not all digits, so
the placeholder `nifeos/record:<path>` and a run id such as `nifeos/basalt:123` are prose.

This needs the network: one `git ls-remote` of basalt, then a blobless shallow fetch of each
pinned commit it has not seen before. A commit's file list never changes, so each one is cached
under `$NIFE_PINNED_TREES`, or `~/.cache/nife/pinned-trees`, by repository and commit. A fetch that
fails is a problem rather than a pass, because a check that cannot read the pin cannot say the
path exists.

Name: provisional, minted by the lane for milestone 1018's step 2 on 2026-11-11 (UTC). A shared
python module under `helpers/`, which `script/names` puts out of its own scope, so this paragraph
is the record. "Qualified" is milestone 1018's own word for the `nifeos/<repo>:<path>` form.

BUGS

- Only roadmap blocks are read. A note or a decision that writes the form is not checked yet.
- A `Cargo.lock` names a git dependency by URL. A dependency taken from crates.io, or by a path
  into a vendored copy, gives no revision, and such a repository reads as unpinned.
"""

import json
import os
import re
import subprocess
import tempfile
import tomllib

ORGANIZATION = "nifeos"
BASALT = f"https://github.com/{ORGANIZATION}/basalt"
QUALIFIED = re.compile(r"`" + ORGANIZATION + r"/([A-Za-z0-9_.-]+):([^`\s<>]+)`")
LOCKED_SOURCE = re.compile(
    r'^source = "git\+https://github\.com/' + ORGANIZATION
    + r'/([A-Za-z0-9_.-]+?)(?:\.git)?\?[^#"]*#([0-9a-f]{40})"', re.M)
RECORD = "record"


def citations(text):
    """Every `nifeos/<repo>:<path>` citation in `text`, as (line number, repo, path)."""
    out = []
    for n, line in enumerate(text.split("\n"), start=1):
        for m in QUALIFIED.finditer(line):
            path = m.group(2).split("#")[0].rstrip("/")
            if path and not path.isdigit():
                out.append((n, m.group(1), path))
    return out


def _cache_dir():
    d = os.environ.get("NIFE_PINNED_TREES") or os.path.join(
        os.path.expanduser("~"), ".cache", "nife", "pinned-trees")
    os.makedirs(d, exist_ok=True)
    return d


def _cached(key, compute):
    """`compute()`'s JSON value, kept under `key`; written by rename, so a reader sees all or none."""
    path = os.path.join(_cache_dir(), key + ".json")
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        pass
    value = compute()
    fd, tmp = tempfile.mkstemp(dir=_cache_dir())
    with os.fdopen(fd, "w") as f:
        json.dump(value, f)
    os.replace(tmp, path)
    return value


def _git(*args, cwd=None):
    r = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if r.returncode:
        raise RuntimeError(f"git {' '.join(args)}: {r.stderr.strip() or 'failed'}")
    return r.stdout


def _snapshot(repo, commit, local_root):
    """{"files": [...], "locks": {lockfile: [[repo, sha], ...]}} for `repo` at `commit`."""
    def read(git_dir):
        files = _git("--git-dir", git_dir, "ls-tree", "-r", "--name-only", commit).split("\n")
        files = [f for f in files if f]
        locks = {}
        for f in files:
            if f == "Cargo.lock" or f.endswith("/Cargo.lock"):
                text = _git("--git-dir", git_dir, "show", f"{commit}:{f}")
                locks[f] = sorted({(r, s) for r, s in LOCKED_SOURCE.findall(text)})
        return {"files": files, "locks": locks}

    def compute():
        if local_root:
            common = _git("-C", local_root, "rev-parse", "--path-format=absolute",
                          "--git-common-dir").strip()
            if subprocess.run(["git", "--git-dir", common, "cat-file", "-e", commit + "^{commit}"],
                              capture_output=True).returncode == 0:
                return read(common)
        with tempfile.TemporaryDirectory() as tmp:
            _git("init", "-q", "--bare", tmp)
            _git("--git-dir", tmp, "remote", "add", "origin",
                 f"https://github.com/{ORGANIZATION}/{repo}")
            _git("--git-dir", tmp, "config", "remote.origin.promisor", "true")
            _git("--git-dir", tmp, "config", "remote.origin.partialclonefilter", "blob:none")
            _git("--git-dir", tmp, "fetch", "-q", "--depth=1", "--filter=blob:none", "origin",
                 commit)
            return read(tmp)

    return _cached(f"{repo}-{commit}", compute)


def _pins():
    """{repo: commit} from `pins.toml` on basalt's `main`, keyed by basalt's commit."""
    head = _git("ls-remote", BASALT, "refs/heads/main").split()
    if not head:
        raise RuntimeError(f"{BASALT} has no main")

    def compute():
        with tempfile.TemporaryDirectory() as tmp:
            _git("init", "-q", "--bare", tmp)
            _git("--git-dir", tmp, "fetch", "-q", "--depth=1", BASALT, head[0])
            manifest = tomllib.loads(_git("--git-dir", tmp, "show", f"{head[0]}:pins.toml"))
        prefix = f"https://github.com/{ORGANIZATION}/"
        return {t["repository"][len(prefix):].removesuffix(".git"): t["commit"]
                for t in manifest.values()
                if isinstance(t, dict) and t.get("repository", "").startswith(prefix)}

    return _cached(f"basalt-pins-{head[0]}", compute)


def _commits(repo, root):
    """The commits `repo` resolves at, as [(commit, how it is pinned)]; empty when nothing pins it."""
    pins = _pins()
    if repo in pins:
        return [(pins[repo], "basalt's pins.toml")]
    found = {}
    for component, commit in sorted(pins.items()):
        snap = _snapshot(component, commit, root if component == "nife" else None)
        for lockfile, entries in sorted(snap["locks"].items()):
            for locked, sha in entries:
                if locked == repo:
                    found.setdefault(sha, []).append(f"{component}@{commit[:12]}:{lockfile}")
    return [(sha, ", ".join(hows)) for sha, hows in sorted(found.items())]


def _exists(files, path):
    return path in files or any(f.startswith(path + "/") for f in files)


def problems(cited, root):
    """`cited` is [(where, repo, path)]; the problems for each citation that names no file."""
    out = []
    resolved = {}
    for where, repo, path in cited:
        if repo == RECORD:
            if not os.path.exists(os.path.join(root, path)):
                out.append(f"{where}: cites `{ORGANIZATION}/{repo}:{path}`, which is not in the "
                           f"record's own tree")
            continue
        try:
            if repo not in resolved:
                resolved[repo] = [(c, how, set(_snapshot(repo, c, None)["files"]))
                                  for c, how in _commits(repo, root)]
        except RuntimeError as e:
            out.append(f"{where}: cannot read `{ORGANIZATION}/{repo}` at basalt's pin, so cannot "
                       f"say `{path}` exists there: {e}")
            continue
        if not resolved[repo]:
            out.append(f"{where}: cites `{ORGANIZATION}/{repo}:{path}`, and nothing basalt pins "
                       f"names `{ORGANIZATION}/{repo}`: neither basalt's pins.toml nor a "
                       f"Cargo.lock at a pinned commit. A path resolves at a pin, and this "
                       f"repository has none")
            continue
        for commit, how, files in resolved[repo]:
            if not _exists(files, path):
                out.append(f"{where}: cites `{ORGANIZATION}/{repo}:{path}`, which does not exist "
                           f"at {commit[:12]}, the commit that {how} pins")
    return out


def selftest():
    """The parse and the existence rule, with no network; returns the failures as strings."""
    failed = []
    text = ("`nifeos/jh7110_entropy:src/lib.rs` and `nifeos/record:<path>`\n"
            "`nifeos/basalt:123`, `nifeos/record:design/naming.md#a-heading`, `crates/x`")
    want = [(1, "jh7110_entropy", "src/lib.rs"), (2, "record", "design/naming.md")]
    if citations(text) != want:
        failed.append(f"citations read {citations(text)}, expected {want}")
    lock = ('source = "git+https://github.com/nifeos/jh7110_entropy?rev=ab#' + "a" * 40 + '"\n'
            'source = "git+https://github.com/other/x?rev=ab#' + "b" * 40 + '"\n')
    if LOCKED_SOURCE.findall(lock) != [("jh7110_entropy", "a" * 40)]:
        failed.append(f"LOCKED_SOURCE read {LOCKED_SOURCE.findall(lock)}")
    files = {"src/lib.rs", "docs/a/b.md"}
    for path, want_exists in [("src/lib.rs", True), ("docs/a", True), ("docs/a/c.md", False),
                              ("src/lib", False)]:
        if _exists(files, path) != want_exists:
            failed.append(f"_exists({path!r}) is {not want_exists}")
    with tempfile.TemporaryDirectory() as tmp:
        os.makedirs(os.path.join(tmp, "design"))
        open(os.path.join(tmp, "design", "naming.md"), "w").close()
        got = problems([("first", "record", "design/naming.md"),
                        ("second", "record", "design/missing.md")], tmp)
        if len(got) != 1 or "second" not in got[0]:
            failed.append(f"record paths gave {got}")
    return failed
