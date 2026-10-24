"""Move each document's `Name:` block out of its directory's README and into the document.

    python3 helpers/document_name_migrate.py                 list what would move, and what cannot
    python3 helpers/document_name_migrate.py --apply         move every one that can
    python3 helpers/document_name_migrate.py --apply README  only the paragraphs in these READMEs
    python3 helpers/document_name_migrate.py --check         exit 1 if anything is left to move
    python3 helpers/document_name_migrate.py --selftest      the rules, as fixtures

A document under `design/`, `notes/` or `briefs/` carries its own `Name:` block, a paragraph under
its title (design/naming.md, "Where a name's provenance lives"; calef, 2026-10-24 UTC). The older
home was a paragraph opening `` `stem` Name: `` in the directory's `README.md`, and
`helpers/name_census.py` still reads it so that pull requests opened before the rule changed keep
passing. This is the sweep that empties the older home. It is re-runnable: a tree with nothing left
to move lists nothing and changes nothing. `--check`, which `script/lint` runs, holds the older home
empty: any paragraph it would move, drop or keep fails the build.

Name: provisional, minted 2026-10-24 (UTC) by lane maintainer/note-names-in-notes. A shared helper
under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the record.
`document_name` for what it moves, `migrate` for the family beside `roadmap_migrate.py`. Refused
`note_name_migrate` (it moves the blocks of documents under `design/` and `briefs/` too). calef has
not named it.

**What moves.** Every README paragraph the census reads as a stem's block: README files under
`design/`, `notes/` and `briefs/`, outside `design/roadmap/` and `design/decisions/`, whose stems the
census never reads, and outside directories whose name opens with a digit, for the same reason. The
paragraph's text is moved as it stands, emphasis stripped, and written under the document's title as
`*Name: <text>*`, wrapped at 100 columns. The README loses the paragraph and the blank line that set
it apart, and once its last one has gone, an introduction to "the stems below" says "here".

**What does not.** A stem whose document already carries a block of its own. When the two say the
same thing (the same status, and the README's text inside the document's) the README paragraph is
dropped. Otherwise both stay and are listed as `KEEP BOTH`, for a person to merge: the census reads
the two together until then, and `script/names --check` fails them if their statuses disagree. A
stem naming no file is left too, because `script/names --check` already reports it.

BUGS. The census's reading of the README form (`helpers/name_census.py`, `_document_block`) is
transitional and goes once a run of this lists nothing, after the lanes of milestone 929 (comments
in the Linux style across first-party Rust) have merged.
"""

import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import name_provenance  # noqa: E402

ROOTS = ("design/", "notes/", "briefs/")
SKIPPED = ("design/roadmap/", "design/decisions/")
WIDTH = 100
# A wrapped line may not open with one of these, or markdown reads a list, heading, table or quote
# and the block's paragraph ends there.
_UNSAFE_OPENER = re.compile(r"^(?:[-+*>|#]|\d+[.)])$|^#")


def in_scope(readme):
    """True for a README whose stem paragraphs the census reads."""
    if not readme.startswith(ROOTS) or readme.startswith(SKIPPED):
        return False
    return not any(part[:1].isdigit() for part in readme.split("/")[1:-1])


def wrap(text, width=WIDTH):
    """`text` as lines of at most `width` columns where possible, none opening unsafely."""
    lines, cur = [], ""
    for word in text.split():
        if cur and len(cur) + 1 + len(word) > width and not _UNSAFE_OPENER.match(word):
            lines.append(cur)
            cur = word
        else:
            cur = f"{cur} {word}" if cur else word
    if cur:
        lines.append(cur)
    return lines


def block_lines(content):
    """The block a document carries under its title. Italic, like a README's own, unless the text
    holds an asterisk that would end the emphasis early."""
    return wrap(f"Name: {content}" if "*" in content else f"*Name: {content}*")


def insert_block(text, content):
    """`text` with the block written under its title (after any frontmatter), or at its top."""
    lines = text.split("\n")
    start = 0
    if lines and lines[0] == "---":
        close = next((i for i in range(1, len(lines)) if lines[i] == "---"), None)
        if close is not None:
            start = close + 1
    title = next((i for i in range(start, len(lines)) if lines[i].startswith("# ")), None)
    at = start if title is None else title + 1
    after = lines[at:]
    while after and not after[0].strip():
        after.pop(0)
    head = lines[:at] + ([""] if title is not None else [])
    return "\n".join(head + block_lines(content) + [""] + after)


def remove_paragraph(text, first, last):
    """`text` without lines `first`..`last` (1-based) and one blank line that set them apart."""
    lines = text.split("\n")
    lo, hi = first - 1, last
    before_blank = lo > 0 and not lines[lo - 1].strip()
    after_blank = hi < len(lines) and not lines[hi].strip()
    if before_blank and (after_blank or hi == len(lines)):
        lo -= 1
    elif after_blank and lo == 0:
        hi += 1
    return "\n".join(lines[:lo] + lines[hi:])


# The sentence many collection READMEs open their stem list with ("Each stem below is named for its
# content, and calef names files."). It is a reason for every stem's name, so it stays once the
# list has gone, pointing at the directory instead of at nothing.
_BELOW = re.compile(r"\b(stems?) below\b")


def retarget(text):
    """`text` with "stem(s) below" read as "stem(s) here", for a README whose stem list is empty."""
    return _BELOW.sub(r"\1 here", text)


def _norm(text):
    return re.sub(r"[\s*_`]+", " ", text).strip().rstrip(".").lower()


def plan(readme, readme_text, read):
    """`[(stem, first, last, content, action)]` for one README. `action` is `move`, `drop` (the
    document already says it) or `keep` (both stay, for a person to merge)."""
    out = []
    directory = os.path.dirname(readme)
    for first, last, para in name_provenance.markdown_paragraphs(readme_text):
        hit = name_provenance.stem_paragraph(para)
        if hit is None:
            continue
        stem, content = hit
        doc = read(f"{directory}/{stem}.md")
        if doc is None:
            continue
        mine = name_provenance.document_block(doc)
        if mine is None:
            action = "move"
        elif (name_provenance.classify(mine)[0] == name_provenance.classify(content)[0]
              and _norm(content) in _norm(mine)):
            action = "drop"
        else:
            action = "keep"
        out.append((stem, first, last, content, action))
    return out


def migrate(readme, read):
    """`({path: new text}, [kept stems])` for one README: every write its migration makes."""
    text = read(readme)
    steps = plan(readme, text, read)
    writes, kept = {}, []
    directory = os.path.dirname(readme)
    # Bottom up, so each removal leaves the line numbers above it valid.
    for stem, first, last, content, action in sorted(steps, key=lambda s: -s[1]):
        if action == "keep":
            kept.append(stem)
            continue
        if action == "move":
            doc = f"{directory}/{stem}.md"
            writes[doc] = insert_block(read(doc), content)
        text = remove_paragraph(text, first, last)
    if any(action != "keep" for *_rest, action in steps):
        if not name_provenance.markdown_blocks(text)[1]:
            text = retarget(text)
        writes[readme] = text
    return writes, sorted(kept)


def unlinked(before, after):
    """The stems a README linked before a migration and no longer links after it."""
    if after is None:
        return []
    links = re.compile(r"\]\((?:\./)?([A-Za-z0-9_.-]+)\.md(?:#[^)]*)?\)")
    return sorted(set(links.findall(before)) - set(links.findall(after)))


def readmes(root):
    out = subprocess.run(["git", "-C", root, "ls-files", "--", "design", "notes", "briefs"],
                         capture_output=True, text=True, check=True).stdout.split()
    return sorted(p for p in out if os.path.basename(p) == "README.md" and in_scope(p))


def main(argv):
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    def read(rel):
        try:
            with open(os.path.join(root, rel), encoding="utf-8") as fh:
                return fh.read()
        except (FileNotFoundError, IsADirectoryError):
            return None

    apply = "--apply" in argv
    check = "--check" in argv
    chosen = [a for a in argv if not a.startswith("--")]
    targets = chosen or readmes(root)
    moved = dropped = 0
    kept = []
    for readme in targets:
        if not in_scope(readme) or read(readme) is None:
            sys.exit(f"document_name_migrate: {readme} is not a README whose stems the census reads")
        for stem, first, _last, _content, action in plan(readme, read(readme), read):
            word = {"move": "MOVE", "drop": "DROP", "keep": "KEEP BOTH"}[action]
            print(f"{word:<9} {readme}:{first} -> {os.path.dirname(readme)}/{stem}.md")
            moved += action == "move"
            dropped += action == "drop"
            if action == "keep":
                kept.append(f"{os.path.dirname(readme)}/{stem}.md")
        if apply:
            writes, _kept = migrate(readme, read)
            for stem in unlinked(read(readme), writes.get(readme)):
                print(f"UNLINKED  {readme}: the paragraph that moved held its only link to {stem}.md; "
                      f"give that document an index line there (§212 (a prose budget): an "
                      f"unlinked appendix is lost)")
            for path, text in writes.items():
                with open(os.path.join(root, path), "w", encoding="utf-8") as fh:
                    fh.write(text)
    verb = "moved" if apply else "to move"
    print(f"document_name_migrate: {moved} {verb}, {dropped} already in the document, "
          f"{len(kept)} kept in both places for a person to merge")
    if check and (moved or dropped or kept):
        print("document_name_migrate: a document's Name: block goes under its own title, never in "
              "its directory's README (design/naming.md, \"Where a name's provenance lives\"). "
              "Run python3 helpers/document_name_migrate.py --apply, merge any KEEP BOTH by hand, "
              "and commit.")
        return 1
    return 1 if apply and kept else 0


def selftest():
    import name_census

    def census(tree):
        records, problems, _missing, unblocked = name_census.census(
            sorted(tree), tree.get, british=[])
        return ({r["path"]: (r["status"], r["date"], r["refused"]) for r in records
                 if r["kind"] == "document"}, problems, unblocked)

    readme = ("# Notes\n\n*Name: ratified 2026-08-16 (calef, in session).*\n\n- [A](a.md): the a.\n\n"
              "`a.md`: Name: ratified 2026-10-18 (calef, \"Yes\"). Refused `aa` (too short).\n\n"
              "`b` Name: provisional, minted 2026-10-24 (UTC) by lane/b.\n")
    sub = ("# X\n\n*Name: provisional.*\n\nEach stem below is named for its content.\n\n"
           "- `c` Name: provisional. After the thing it\n"
           "  describes.\n- `d` Name: ratified 2026-10-18 (calef, \"Approve all\").\n")
    tree = {
        "notes/README.md": readme,
        "notes/a.md": "# A\n\nThe a.\n",
        "notes/b.md": ("---\nk: v\n---\n# B\n*Name: provisional, minted 2026-10-24 (UTC) by lane/b.\n"
                       "After the b.*\n\nB.\n"),
        "notes/x/README.md": sub,
        "notes/x/c.md": "# C\n\nC.\n",
        "notes/x/d.md": "# D\n\n*Name: provisional. Stale.*\n\nD.\n",
        "design/naming/convention-rulings.md": "# Rulings\n",
    }
    before, problems_before, _ = census(tree)

    after = dict(tree)
    kept = []
    for path in ("notes/README.md", "notes/x/README.md"):
        writes, k = migrate(path, after.get)
        after.update(writes)
        kept += k
    now, problems_now, unblocked = census(after)
    rerun = dict(after)
    for path in ("notes/README.md", "notes/x/README.md"):
        rerun.update(migrate(path, rerun.get)[0])

    cases = [
        ("the README form is read", before["notes/a.md"][0] == "ratified"),
        ("a refusal in the README form is read", before["notes/a.md"][2] == ["aa"]),
        ("a moved block reads the same", now["notes/a.md"] == before["notes/a.md"]),
        ("a wrapped list item moves whole", now["notes/x/c.md"] == before["notes/x/c.md"]),
        ("the block lands under the title",
         after["notes/a.md"].startswith("# A\n\n*Name: ratified 2026-10-18 (calef, \"Yes\"). "
                                        "Refused `aa` (too short).*\n\nThe a.\n")),
        ("the README keeps its own block and index line",
         after["notes/README.md"] == "# Notes\n\n*Name: ratified 2026-08-16 (calef, in session).*\n\n"
                                     "- [A](a.md): the a.\n"),
        ("a README paragraph the document already says is dropped",
         "`b` Name" not in after["notes/README.md"] and after["notes/b.md"] == tree["notes/b.md"]),
        ("an introduction stays while a stem is left below it",
         "Each stem below" in after["notes/x/README.md"]),
        ("and points at the directory once none is", retarget("Each stem below is x.\nThe stems "
                                                              "below were minted.")
         == "Each stem here is x.\nThe stems here were minted."),
        ("a moved paragraph's only link is reported",
         unlinked("- [A](a.md)\n\n`b` Name: x. See [b.md](b.md).\n", "- [A](a.md)\n") == ["b"]),
        ("two that disagree both stay", kept == ["d"] and "`d` Name" in after["notes/x/README.md"]),
        ("and the census fails them",
         any("notes/x/d.md" in p and "disagree" not in p and "README paragraph says ratified" in p
             for p in problems_before)),
        ("a document with no block of either kind is unblocked, not inherited at the top",
         "notes/a.md" in census({**after, "notes/a.md": "# A\n"})[2]),
        ("a document under a subdirectory inherits the directory's block",
         census({**after, "notes/x/c.md": "# C\n"})[0]["notes/x/c.md"][0] == "provisional"),
        ("two ratifications on different dates fail",
         any("different dates" in p for p in census({
             **tree, "notes/a.md": "# A\n\n*Name: ratified 2026-10-19 (calef, in session).*\n"})[1])),
        ("a second run changes nothing", rerun == after),
        ("nothing else changed status",
         {p: v for p, v in now.items() if p != "notes/x/d.md"}
         == {p: v for p, v in before.items() if p != "notes/x/d.md"}),
        ("no new problem", set(problems_now) <= set(problems_before)),
        ("no new unblocked document", unblocked == []),
        ("a wrapped line never opens a list",
         all(not re.match(r"^(?:[-+*>|#]|\d+[.)])(?:\s|$)", line)
             for line in wrap("x " * 49 + "- y " + "z " * 60)[1:])),
        ("the roadmap and the decisions are out of scope",
         not in_scope("design/roadmap/0020-portable-hal/README.md")
         and not in_scope("design/decisions/0027-filesystem-service/README.md")
         and in_scope("notes/x/README.md") and in_scope("design/naming/README.md")),
    ]
    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f"document_name_migrate selftest FAILED: {name}", file=sys.stderr)
    if failed:
        return 1
    print(f"document_name_migrate selftest: {len(cases)} cases pass")
    return 0


if __name__ == "__main__":
    if "--selftest" in sys.argv[1:]:
        sys.exit(selftest())
    sys.exit(main(sys.argv[1:]))
