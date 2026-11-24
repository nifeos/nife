"""Where the design record is, for the readers that stay in this repository when the record leaves.

Milestone 1018 (the design record moves to `nifeos/record` and is published as a website), step 6.
A reader that stays in nife and reads a record file, such as a roadmap block, a tenet or the naming
rules, asks here for the directory that holds the record and joins the record path onto it. The
move then changes this file and none of the readers.

The answer has two cases:

- While the record lives in this tree, it is the work tree itself, the directory above `helpers/`
  or the `top` a caller passes. The record lives here while `MARKER` is a file under that top.
- Once the record has left, it is a checkout of `nifeos/record` at the commit that the `[record]`
  table of basalt's `pins.toml`, on basalt's `main`, names: the rule `helpers/qualified_paths.py`
  follows for every `nifeos/<repo>:<path>` citation, so a reader and a citation see one record.
  `$RECORD_TREE` names a checkout at that commit instead (CI checks the record out there), and one
  at another commit is refused. Otherwise the checkout is kept under `$NIFE_PINNED_TREES` (default
  `~/.cache/nife/pinned-trees`), fetched once per pinned commit, with its write bits taken off so a
  reader cannot change what the next one reads.

    python3 helpers/record_root.py               # the directory, one line, for a shell reader
    python3 helpers/record_root.py --top DIR     # the same, for the tree at DIR
    python3 helpers/record_root.py --commit      # the record commit basalt pins
    python3 helpers/record_root.py --check       # what script/lint runs: the pin resolves and holds the record
    python3 helpers/record_root.py --selftest    # fixtures in a temporary directory, no network

A shell reader writes `RECORD="$(python3 "$REPO/helpers/record_root.py")"` and reads
`"$RECORD/notes/..."`. A python reader imports this module and calls `path("design", "roadmap")`.

Name: provisional, minted by the lane for milestone 1018's step 6 on 2026-11-23 (UTC). A python
module under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the
record.

BUGS

- The answer covers a reader of the record as it is now. A reader of the record's history, such
  as the block and decision counts `script/metrics` restates from each week's commit, needs the
  record's repository and its commits, which a directory does not give.
- `MARKER` is one file. A checkout that holds the record without that file reads as one the record
  has left, and the answer is the fetched tree.
- The fetch needs the network the first time a pin moves: one `git ls-remote` of basalt, and a
  shallow fetch of the record at its pin. A fetch that fails stops the reader, with the reason,
  rather than letting it read nothing and pass.
- `helpers/name_census.py` and `helpers/name_release.py` do not ask here. The census takes one
  path list and one `read` callback over a single tree, and reads `name_rules.RULES_DOC` and the
  documents' own blocks through it. `script/names` hands it a callback that reads `RULES_DOC`
  here. `script/metrics` hands it each week's commit, and a commit after the removal holds no
  copy, so that census needs a second reader for the record's paths.
"""

import os
import subprocess
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import qualified_paths                                                      # noqa: E402

# A record file that stays in the record as long as the record has a roadmap: the roadmap's index.
MARKER = "design/roadmap/README.md"

# The file a finished fetch writes last, so a checkout interrupted halfway is never read.
COMPLETE = ".record-tree-complete"


class RecordNotHere(SystemExit):
    """The record is not in the tree asked about, and its pinned tree cannot be read."""


def work_tree():
    """The top of the tree this file is in: the directory above `helpers/`."""
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def here(top):
    """Whether the record lives in the tree at `top`."""
    return os.path.isfile(os.path.join(top, MARKER))


def _git(*args):
    run = subprocess.run(["git", *args], capture_output=True, text=True)
    if run.returncode:
        raise RecordNotHere(f"record_root: git {' '.join(args)}: "
                            f"{run.stderr.strip() or 'failed'}")
    return run.stdout.strip()


def commit():
    """The record commit the `[record]` table of `pins.toml` on basalt's `main` names."""
    try:
        pins = qualified_paths._pins()
    except RuntimeError as e:
        raise RecordNotHere(f"record_root: cannot read basalt's pins.toml: {e}")
    if qualified_paths.RECORD not in pins:
        raise RecordNotHere("record_root: basalt's pins.toml on main has no [record] table, so "
                            "no commit of nifeos/record goes with this tree")
    return pins[qualified_paths.RECORD]


def _read_only(tree):
    """Take the write bits off a checkout, outside `.git`, so a reader that writes fails loudly."""
    for top, dirs, files in os.walk(tree):
        if top == tree and ".git" in dirs:
            dirs.remove(".git")
        for name in files:
            p = os.path.join(top, name)
            if not os.path.islink(p):
                os.chmod(p, os.stat(p).st_mode & ~0o222)
        if top != tree:
            os.chmod(top, os.stat(top).st_mode & ~0o222)


def fetched(top):
    """A checkout of `nifeos/record` at its pin, for a `top` the record has left."""
    want = commit()
    tree = os.environ.get("RECORD_TREE")
    if tree:
        have = _git("-C", tree, "rev-parse", "HEAD")
        if have != want:
            raise RecordNotHere(
                f"record_root: $RECORD_TREE ({tree}) is at {have[:12]}, and basalt's main pins "
                f"the record at {want[:12]}. Check out that commit there, or unset RECORD_TREE.")
        return os.path.abspath(tree)
    tree = os.path.join(qualified_paths._cache_dir(), f"record-tree-{want}")
    if os.path.isfile(os.path.join(tree, COMPLETE)):
        return tree
    partial = tree + f".partial-{os.getpid()}"
    _git("init", "-q", partial)
    _git("-C", partial, "fetch", "-q", "--depth=1",
         f"https://github.com/{qualified_paths.ORGANIZATION}/{qualified_paths.RECORD}", want)
    _git("-C", partial, "checkout", "-q", "--detach", "FETCH_HEAD")
    open(os.path.join(partial, COMPLETE), "w").close()
    _read_only(partial)
    try:
        os.rename(partial, tree)
    except OSError:
        # Another reader finished the same commit first; its tree is the same one.
        subprocess.run(["chmod", "-R", "u+w", partial])
        subprocess.run(["rm", "-rf", partial])
    if not here(tree):
        raise RecordNotHere(f"record_root: nifeos/record at {want[:12]} has no {MARKER}")
    return tree


def root(top=None):
    """The directory that holds the record's `design/` and `notes/`, for the tree at `top`.

    `top` defaults to `work_tree()`. While the record lives in that tree, the answer is `top`
    itself, unchanged; otherwise it is `fetched(top)`.
    """
    top = work_tree() if top is None else top
    return top if here(top) else fetched(top)


def path(*parts, top=None):
    """A record path, such as `path("design", "tenets")`, joined onto `root(top)`."""
    return os.path.join(root(top), *parts)


def helper(name, top=None):
    """The record's own `helpers/<name>.py`, imported from `root(top)` as a module.

    The record keeps the parsers of its formats, such as `roadmap_block`, and a reader here that
    parses a record file imports the record's copy at the pin. It loads by file path, so the
    record's other helpers never shadow this tree's on `sys.path`. A record helper that imports
    another record helper by its bare name, such as `roadmap_proposals` importing `roadmap_block`,
    gets the record's copy only when this tree has no helper of that name; a name this tree has
    resolves to this tree's helper, as any other import here does.
    """
    import importlib.util
    key = "record_" + name
    if key in sys.modules:
        return sys.modules[key]
    record_helpers = path("helpers", top=top)
    _record_finder(record_helpers)
    spec = importlib.util.spec_from_file_location(key, os.path.join(record_helpers, name + ".py"))
    module = importlib.util.module_from_spec(spec)
    sys.modules[key] = module
    spec.loader.exec_module(module)
    return module


def _record_finder(record_helpers):
    """Append, once, a finder for record helpers that this tree's `helpers/` does not hold.

    It sits last on `sys.meta_path`, so the standard library and every directory on `sys.path`
    are searched first.
    """
    import importlib.util
    mine = os.path.dirname(os.path.abspath(__file__))

    class RecordHelpers:
        directory = record_helpers

        @staticmethod
        def find_spec(fullname, _path=None, _target=None):
            if "." in fullname or os.path.exists(os.path.join(mine, fullname + ".py")):
                return None
            candidate = os.path.join(record_helpers, fullname + ".py")
            if not os.path.exists(candidate):
                return None
            return importlib.util.spec_from_file_location(fullname, candidate)

    if not any(getattr(f, "directory", None) == record_helpers for f in sys.meta_path):
        sys.meta_path.append(RecordHelpers)


def selftest():
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        present = os.path.join(tmp, "present")
        os.makedirs(os.path.join(present, os.path.dirname(MARKER)))
        open(os.path.join(present, MARKER), "w").close()
        # The record here: the answer is the top as given, so a relative or absolute top reads
        # exactly as it did before a reader asked.
        assert root(present) == present, root(present)
        assert path("design", "tenets", top=present) == os.path.join(present, "design", "tenets")
        assert path(MARKER, top=present) == os.path.join(present, MARKER)
        # The record gone: the answer is the pinned tree, read from $RECORD_TREE when it is set,
        # and a $RECORD_TREE at another commit is refused, never read.
        gone = os.path.join(tmp, "gone")
        os.makedirs(os.path.join(gone, "design"))
        pinned = os.path.join(tmp, "pinned")
        os.makedirs(os.path.join(pinned, os.path.dirname(MARKER)))
        open(os.path.join(pinned, MARKER), "w").close()
        env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.invalid",
                   GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@example.invalid")
        for args in (["init", "-q"], ["add", "-A"], ["commit", "-q", "-m", "record"]):
            subprocess.run(["git", *args], cwd=pinned, env=env, check=True, capture_output=True)
        head = _git("-C", pinned, "rev-parse", "HEAD")
        saved = (os.environ.get("RECORD_TREE"), qualified_paths._pins)
        try:
            os.environ["RECORD_TREE"] = pinned
            qualified_paths._pins = lambda: {"record": head}
            assert root(gone) == os.path.abspath(pinned), root(gone)
            assert path("notes", "x.md", top=gone) == os.path.join(pinned, "notes", "x.md")
            qualified_paths._pins = lambda: {"record": "0" * 40}
            for call in (lambda: root(gone), lambda: path("notes", "x.md", top=gone)):
                try:
                    call()
                except RecordNotHere as refusal:
                    assert "RECORD_TREE" in str(refusal) and "0" * 12 in str(refusal), refusal
                else:
                    raise AssertionError("a $RECORD_TREE at another commit answered")
            qualified_paths._pins = lambda: {"nife": head}
            try:
                root(gone)
            except RecordNotHere as refusal:
                assert "[record]" in str(refusal), refusal
            else:
                raise AssertionError("a pins.toml without [record] answered")
        finally:
            qualified_paths._pins = saved[1]
            if saved[0] is None:
                os.environ.pop("RECORD_TREE", None)
            else:
                os.environ["RECORD_TREE"] = saved[0]
        # A marker that is a directory is not the record's index.
        odd = os.path.join(tmp, "odd")
        os.makedirs(os.path.join(odd, MARKER))
        assert not here(odd)
    assert work_tree() == os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    print("record_root: selftest passed")


def main(argv):
    if argv == ["--selftest"]:
        selftest()
        return 0
    if argv == ["--commit"]:
        print(commit())
        return 0
    if argv == ["--check"]:
        top = work_tree()
        if here(top):
            print(f"record_root: the record lives in this work tree ({MARKER} is here)")
            return 0
        tree = root()
        print(f"record_root: the record is read from nifeos/record at {commit()[:12]}, the commit "
              f"basalt's main pins ({tree})")
        return 0
    if len(argv) == 2 and argv[0] == "--top":
        print(root(argv[1]))
        return 0
    if argv:
        print(__doc__.split("\n\n")[4], file=sys.stderr)
        return 2
    print(root())
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
