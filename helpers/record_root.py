"""Where the design record is, for the readers that stay in this repository when the record leaves.

Milestone 1018 (the design record moves to `nifeos/record` and is published as a website), step 6.
A reader that stays in nife and reads a record file, such as a roadmap block, a tenet or the naming
rules, asks here for the directory that holds the record and joins the record path onto it. The
move then changes this file and none of the readers.

The answer has two cases:

- While the record lives in this tree, it is the work tree itself, the directory above `helpers/`
  or the `top` a caller passes. The record lives here while `MARKER` is a file under that top.
- Once the record has left, it is a tree of `nifeos/record` at the commit that basalt's `[record]`
  table names, in basalt's `pins.toml` at the basalt commit `xtask/basalt-pin` names. That table
  does not exist yet, and nothing fetches the tree, so `fetched` refuses with a message that says
  so. The pull request that removes the record from nife builds `fetched`, and it is the only
  function that changes.

    python3 helpers/record_root.py               # the directory, one line, for a shell reader
    python3 helpers/record_root.py --check       # what script/lint runs: today the work tree
    python3 helpers/record_root.py --selftest    # fixtures in a temporary directory, no git

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
  has left, and the refusal names the file it looked for.
- `helpers/name_census.py` and `helpers/name_release.py` do not ask here. The census takes one
  path list and one `read` callback over a single tree, and reads `name_rules.RULES_DOC` and the
  documents' own blocks through it; `script/metrics` hands it each week's commit the same way. A
  record path read through that callback stays in the code's tree, so the census needs a second
  reader for the record's paths before it can read the record anywhere else.
"""

import os
import sys

sys.dont_write_bytecode = True

# A record file that stays in the record as long as the record has a roadmap: the roadmap's index.
MARKER = "design/roadmap/README.md"

# Where the record's pin is read from once it has left, named so the refusal can say so.
BASALT_PIN = "xtask/basalt-pin"


class RecordNotHere(SystemExit):
    """The record is not in the tree asked about, and no fetched tree stands in for it yet."""


def work_tree():
    """The top of the tree this file is in: the directory above `helpers/`."""
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def here(top):
    """Whether the record lives in the tree at `top`."""
    return os.path.isfile(os.path.join(top, MARKER))


def fetched(top):
    """The fetched record tree at the pin, once the record has left `top`; refused until it is built."""
    raise RecordNotHere(
        f"record_root: {os.path.join(top, MARKER)} is missing, so the record is not in this tree, "
        f"and reading it at the commit basalt's [record] table names (through {BASALT_PIN}) is "
        f"not built yet (milestone 1018, step 6)")


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
        # The record gone: a refusal, never a silent fall back to the tree that lacks it.
        gone = os.path.join(tmp, "gone")
        os.makedirs(os.path.join(gone, "design"))
        for call in (lambda: root(gone), lambda: path("notes", "x.md", top=gone)):
            try:
                call()
            except RecordNotHere as refusal:
                message = str(refusal)
                assert os.path.join(gone, MARKER) in message, message
                assert BASALT_PIN in message and "[record]" in message, message
            else:
                raise AssertionError("a tree without the record answered")
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
    if argv == ["--check"]:
        top = work_tree()
        if root() != top:
            print(f"record_root: the record is not read from this work tree ({root()})",
                  file=sys.stderr)
            return 1
        print(f"record_root: the record lives in this work tree ({MARKER} is here)")
        return 0
    if argv:
        print(__doc__.split("\n\n")[4], file=sys.stderr)
        return 2
    print(root())
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
