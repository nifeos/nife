"""The record's side of the move: which notes stay with a crate, and which links leave the record.

Milestone 1018 (the design record moves to `nifeos/record` and is published as a website), step 4.
`packages/homes.toml` sorts every tracked path, and a path whose home is the repository `record`
moves with the record. This reads that sort through helpers/packages.py and answers two questions
from it:

- How many notes are about one crate alone. A Markdown file under `notes/` whose home is neither
  `record` nor an undecided tool-data home names one crate in its home's reason, or a crate's
  repository as its home. §315 (a crate's documentation leaves with it) sends each one with its
  crate.
- Which links leave the record. Every relative Markdown link in a file that moves, whose target is
  a tracked path that stays. The site's link check fails the build on a link into nothing, so each
  of these becomes a qualified `nifeos/<repo>:<path>` or plain text when the record moves.

    python3 helpers/record_links.py            # the counts, and the links grouped by home
    python3 helpers/record_links.py --list     # one line per link: source, target, its home

A target that is a directory stays when any tracked file under it stays. Links inside fenced code
and inline code spans are examples, not links, and are skipped.

Name: provisional, minted by the lane for milestone 1018's step 4 on 2026-11-11 (UTC). A python
module under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the
record.

BUGS

- Only inline links, `[text](target)`, are read. A reference-style link (`[text][label]` with a
  `[label]: target` line) is not, and the record holds a few.
- A crate note is told from a tool-data note by its home's reason naming `crates/`. A reason
  written another way would count the note as tool data.
"""

import os
import posixpath
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import packages                                                             # noqa: E402

RECORD = "record"
FENCE = re.compile(r"^\s*(```|~~~)")
INLINE_CODE = re.compile(r"`[^`]*`")
LINK = re.compile(r"\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")


def moves(home):
    return bool(home) and home.get("repo") == RECORD


def links(text):
    """Every inline link target in Markdown `text`, as (line number, target)."""
    out, fenced = [], False
    for n, line in enumerate(text.split("\n"), start=1):
        if FENCE.match(line):
            fenced = not fenced
            continue
        if fenced:
            continue
        for m in LINK.finditer(INLINE_CODE.sub("", line)):
            out.append((n, m.group(1)))
    return out


def resolve(source, target):
    """The repository path a relative link from `source` names, or None for any other link."""
    if re.match(r"^[a-z][a-z0-9+.-]*:", target, re.I) or target.startswith(("#", "/")):
        return None
    path = target.split("#")[0].split("?")[0]
    if not path:
        return None
    return posixpath.normpath(posixpath.join(posixpath.dirname(source), path))


def sort(file_home, read):
    """(crate notes, leaving links) from {path: home} and `read(path)` -> text."""
    crate_notes = sorted(
        p for p, h in file_home.items()
        if p.startswith("notes/") and p.endswith(".md") and h and not moves(h)
        and (h["status"] != "undecided" or "crates/" in h.get("reason", "")))
    staying = {p for p, h in file_home.items() if not moves(h)}
    staying_dirs = {posixpath.dirname(p) for p in staying}
    for d in list(staying_dirs):
        while d:
            d = posixpath.dirname(d)
            staying_dirs.add(d)
    leaving = []
    for source in sorted(p for p, h in file_home.items() if moves(h) and p.endswith(".md")):
        for n, target in links(read(source)):
            path = resolve(source, target)
            if path is None:
                continue
            if path in staying:
                leaving.append((source, n, target, path, file_home[path]))
            elif path in staying_dirs and path not in file_home:
                leaving.append((source, n, target, path + "/", None))
    return crate_notes, leaving


def home_name(home, grouped=False):
    """How a summary or a list line names a home; `grouped` folds the crate-note homes into one."""
    if home is None:
        return "(a directory with a file that stays)"
    if home.get("repo"):
        return home["repo"]
    if grouped and "crates/" in home["reason"]:
        return "undecided: a note about one crate alone (one home per crate)"
    return "undecided: " + home["reason"]


def selftest():
    text = "[a](../crates/x/src/lib.rs) `[b](no.md)`\n```\n[c](no.md)\n```\n[d](https://e) [f](#g)"
    got = [resolve("notes/n.md", t) for _n, t in links(text)]
    assert got == ["crates/x/src/lib.rs", None, None], got
    homes = {"notes/n.md": {"status": "provisional", "repo": RECORD},
             "crates/x/src/lib.rs": {"status": "provisional", "repo": "contracts"},
             "notes/c.md": {"status": "undecided", "reason": "about crates/x alone"},
             "notes/t.md": {"status": "undecided", "reason": "tool data"}}
    texts = {"notes/n.md": "[a](../crates/x/src/lib.rs) [b](c.md) [d](../crates/x)"}
    notes, leaving = sort(homes, lambda p: texts.get(p, ""))
    assert notes == ["notes/c.md"], notes
    assert [x[3] for x in leaving] == ["crates/x/src/lib.rs", "notes/c.md", "crates/x/"], leaving
    print("record_links: selftest passed")


def main(argv):
    if argv == ["--selftest"]:
        selftest()
        return 0
    if argv not in ([], ["--list"]):
        print(__doc__.split("\n\n")[2], file=sys.stderr)
        return 2
    tracked, texts = packages.working_tree()
    result = packages.evaluate(tracked, texts)
    if result["errors"]:
        print("record_links: helpers/packages.py --check fails; fix the sort first", file=sys.stderr)
        return 1

    def read(path):
        with open(path, encoding="utf-8", errors="replace") as f:
            return f.read()

    crate_notes, leaving = sort(result["file_home"], read)
    if argv == ["--list"]:
        for source, n, target, _path, home in leaving:
            print(f"{source}:{n}\t{target}\t{home_name(home)}")
        return 0
    by_home = {}
    for *_rest, home in leaving:
        name = home_name(home, grouped=True)
        by_home[name] = by_home.get(name, 0) + 1
    print(f"record_links: {len(crate_notes)} notes are about one crate alone and stay with it")
    print(f"record_links: {len(leaving)} links from {len({x[0] for x in leaving})} record files "
          f"name a path that stays")
    for name, count in sorted(by_home.items(), key=lambda kv: -kv[1]):
        print(f"  {count:6d}  {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
