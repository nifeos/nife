"""The removed-files index check: every row of design/removed.tsv is true, and no number returns.

The index lists files taken out of the tree on purpose (path, removal date, the last commit that
held the file, reason). This check keeps it honest in three ways. A row's path is absent from the
tree. A row's sha names a commit that exists and held the path, so `git show <sha>:<path>` works.
And a removed roadmap block keeps its number retired: no file under `design/roadmap/` may start
with the four-digit number of a removed block, so a number is never minted twice.

Name: provisional, minted 2026-10-31 (UTC) by maintainer/remove-uncited-files. A module under
`helpers/`, which `script/names` leaves out of its scope, so this paragraph is its provenance. calef
names things.

    python3 helpers/removed_index.py --check      judge the tree
    python3 helpers/removed_index.py --selftest   judge planted fixtures
"""
import os
import re
import subprocess
import sys
import tempfile

INDEX = "design/removed.tsv"
SHA = re.compile(r"[0-9a-f]{40}")
DATE = re.compile(r"\d{4}-\d{2}-\d{2}")
BLOCK = re.compile(r"design/roadmap/(\d{4})-[^/]+\.md$")


def rows(text):
    """The (path, removed, sha, reason) tuples of an index, skipping comments and the header."""
    out = []
    for line in text.splitlines():
        if not line or line.startswith("#") or line.startswith("path\t"):
            continue
        out.append(tuple(line.split("\t")))
    return out


def problems(root, text, holds):
    """Problems for index `text` against the tree at `root`; `holds(sha, path)` asks git."""
    bad = []
    numbers = {}
    for row in rows(text):
        if len(row) != 4 or not all(row):
            bad.append(f"{INDEX}: a row needs four tab-separated fields, got {row!r}")
            continue
        path, removed, sha, _reason = row
        if not DATE.fullmatch(removed):
            bad.append(f"{INDEX}: {path}: removal date {removed!r} is not YYYY-MM-DD")
        if os.path.exists(os.path.join(root, path)):
            bad.append(f"{INDEX}: {path} is present in the tree; drop the row or remove the file")
        if not SHA.fullmatch(sha):
            bad.append(f"{INDEX}: {path}: {sha!r} is not a full 40-digit sha")
        elif not holds(sha, path):
            bad.append(f"{INDEX}: {path}: commit {sha} does not exist or did not hold the path")
        m = BLOCK.match(path)
        if m:
            numbers[m.group(1)] = path
    roadmap = os.path.join(root, "design/roadmap")
    if os.path.isdir(roadmap):
        for name in sorted(os.listdir(roadmap)):
            if name[:4] in numbers and name[4:5] == "-":
                bad.append(f"design/roadmap/{name}: number {name[:4]} was retired with "
                           f"{numbers[name[:4]]} and is never minted again ({INDEX})")
    return bad


def git_holds(sha, path):
    return subprocess.run(["git", "cat-file", "-e", f"{sha}:{path}"],
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0


def selftest():
    sha = "a" * 40
    head = "path\tremoved\tlast_commit\treason\n"
    with tempfile.TemporaryDirectory() as root:
        os.makedirs(os.path.join(root, "design/roadmap"))
        ok = head + f"design/roadmap/0007-gone.md\t2026-10-31\t{sha}\tBUILT and uncited\n"
        assert problems(root, ok, lambda s, p: True) == []
        assert len(problems(root, ok, lambda s, p: False)) == 1
        open(os.path.join(root, "design/roadmap/0007-gone.md"), "w").close()
        assert len(problems(root, ok, lambda s, p: True)) == 2  # present, and the number returned
        assert len(problems(root, head + "x\t2026-10-31\tshort\twhy\n", lambda s, p: True)) == 1
        assert len(problems(root, head + "x\tyesterday\t" + sha + "\twhy\n", lambda s, p: True)) == 1
        assert len(problems(root, head + "x\t2026-10-31\n", lambda s, p: True)) == 1
    print("removed_index: selftest ok")


def main(argv):
    if argv == ["--selftest"]:
        selftest()
        return 0
    if argv == ["--check"]:
        with open(INDEX) as f:
            bad = problems(".", f.read(), git_holds)
        for line in bad:
            print(line)
        print(f"removed_index: {len(rows(open(INDEX).read()))} rows, {len(bad)} problems")
        return 1 if bad else 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
