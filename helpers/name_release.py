"""Where a release-track crate's names are recorded once they are ratified, for `script/names`.

**Why this file exists.** A crate on the release track (milestones 813 to 820, the eight crates of
milestone 857 (crates released on their own)) leaves for its own repository, and two rules met in
it. The module-block ratchet and `script/names --check` want a `Name:` block on the crate, each
module and each `pub` item; `script/crate-standalone` step 3 refuses every such block, because a
stranger cannot follow one. The ruling of 2026-10-25 (UTC) settles it: a release-track crate's
names are ratified before it leaves, and once they are, its blocks stop mattering. So the record
moves out of the crate's shipped files into one file per crate, and the census reads it there.

**The record.** `RECORD_DIR/<crate, hyphenated>-names.md`, for example
`design/roadmap/0857-crates-released-on-their-own/file-allocation-table-names.md`. It holds a line
`Ratified: <YYYY-MM-DD> (<where>)` once the crate's names are ratified, and a table, one row
per name:

    | name | kind | path | status | ... |
    |---|---|---|---|---|
    | `file_allocation_table` | crate | `src/lib.rs` | ratified 2026-10-07 (#1806). ... |
    | `long_name` | module | `src/long_name.rs` | ratified 2026-11-02 (#NNNN). |
    | `Volume` | item | `src/volume.rs` | ratified 2026-11-02 (#NNNN). Refused `Fs` (why). |
    | `std` | feature | `Cargo.toml` | ratified 2026-11-02 (#NNNN). |

The status cell is a `Name:` block's text without the `Name:`, judged by
`name_provenance.classify`, so refusals keep the grammar `script/names --refused` reads. Columns
after the fourth are for the reader and are not parsed. A row covers every item of that name in
that file, so three `pub fn new` in one file take one row (and count as three names, as the
census counts them).

**Before the `Ratified:` line, the record changes nothing.** The crate's blocks and the ratchet's
baseline stand as for any crate; the table is the list under review, and its status column is a
snapshot (`--draft` prints a fresh one) that nothing checks.

**Once the line is there, the record replaces the blocks for that crate, and only that crate.**
`census()` reads no block under `crates/<crate>/`, takes each row as the name's record, and reports
as a problem any `Name:` block still inside the crate (`script/crate-standalone` refuses it), any
module or `pub` item with no row (add one, `provisional` until ratified, so a new name stays on the
worklist without entering the shipped files), and any row naming something the crate no longer
has. `script/crate-standalone` step 0 passes only when every row is ratified. A crate that has
left (`crates/<crate>/` is gone) keeps its record, and the census reads nothing from it.

**Only the track is covered.** The track is the crates the blocks of `crate_release.MILESTONES`
name in their titles; a record file for any other crate is a problem, so this never relaxes the
block rule for a crate that is not leaving.

    python3 helpers/name_release.py --draft <crate>   # the table rows, from today's tree
    python3 helpers/name_release.py --selftest

Name: provisional, minted 2026-10-25 (UTC) by lane release-crates-names-ratified-first. A shared
helper under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the
record. `name_` for the family beside `name_rules.py`; `release` for the track it serves.
`crate_names` was considered and reads as every crate's.
"""

import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import crate_release  # noqa: E402
import name_provenance  # noqa: E402

RECORD_DIR = "design/roadmap/0857-crates-released-on-their-own"
RECORD_SUFFIX = "-names.md"
KINDS = ("crate", "module", "item", "feature")
PROVENANCE = dict(crate_release.CITATIONS)["provenance block"]

_RATIFIED = re.compile(r"^Ratified:\s*(.*)$", re.M)
_DATED = re.compile(r"^\d{4}-\d{2}-\d{2}\b")
_ROW = re.compile(r"^\|\s*`([^`]+)`\s*\|\s*([a-z]+)\s*\|\s*`([^`]+)`\s*\|([^|]*)\|")
_MILESTONE = re.compile(r"^design/roadmap/0(\d{3})-[^/]+\.md$")


def record_path(crate):
    return f"{RECORD_DIR}/{crate.replace('_', '-')}{RECORD_SUFFIX}"


def track(paths, read):
    """The crates on the release track, from the titles of `crate_release.MILESTONES`' blocks."""
    out = set()
    for path in paths:
        m = _MILESTONE.match(path)
        if m and int(m.group(1)) in crate_release.MILESTONES:
            crate = crate_release.crate_of(read(path) or "")
            if crate:
                out.add(crate)
    return out


def parse(text):
    """`(ratified, rows, problems)`: the `Ratified:` text or None, and `[(line, name, kind, path, cell)]`."""
    ratified, problems, rows = None, [], []
    m = _RATIFIED.search(text)
    if m and _DATED.match(m.group(1).strip()):
        ratified = m.group(1).strip()
    for number, line in enumerate(text.split("\n"), 1):
        row = _ROW.match(line)
        if not row:
            continue
        name, kind, path, cell = row.group(1), row.group(2), row.group(3), row.group(4).strip()
        if kind not in KINDS:
            problems.append((number, f"kind `{kind}` is none of {', '.join(KINDS)}"))
            continue
        rows.append((number, name, kind, path, cell))
    return ratified, rows, problems


def _module_stem(rel):
    stem = os.path.splitext(os.path.basename(rel))[0]
    return os.path.basename(os.path.dirname(rel)) if stem in ("mod", "lib", "main") else stem


def _item_lines(text):
    """`{name: (1-based line, what)}` for every `pub` item, first occurrence, kinds as the census has them."""
    out = {}
    for number, line in enumerate(text.split("\n"), 1):
        if not line.lstrip().startswith("pub "):
            continue
        m = name_provenance._ITEM.match(line)
        if m and m.group(1) not in ("mod", "macro_rules!"):
            out.setdefault(m.group(2), (number, name_provenance.ITEM_KINDS[m.group(1)]))
    return out


def _member_line(text, name):
    """`(1-based line, what)` for a field or variant called `name`, which a block may also name, or None."""
    for number, line in enumerate(text.split("\n"), 1):
        for what, rx in (("variant", name_provenance._VARIANT), ("field", name_provenance._FIELD)):
            m = rx.match(line)
            if m and m.group(1) == name and not name_provenance._ITEM.match(line):
                return number, what
    return None


def load(paths, read):
    """`({crate: (record path, ratified, rows)}, problems)` for every record file, in force or not."""
    on_track = track(paths, read)
    found, problems = {}, []
    for path in paths:
        if os.path.dirname(path) != RECORD_DIR or not path.endswith(RECORD_SUFFIX):
            continue
        crate = os.path.basename(path)[:-len(RECORD_SUFFIX)].replace("-", "_")
        if crate not in on_track:
            problems.append(f"{path}: `{crate}` is not on the release track (the crates milestones "
                            f"{crate_release.MILESTONES[0]} to {crate_release.MILESTONES[-1]} name), "
                            f"so its names keep their `Name:` blocks")
            continue
        ratified, rows, bad = parse(read(path) or "")
        problems += [f"{path}:{n}: {why}" for n, why in bad]
        found[crate] = (path, ratified, rows)
    return found, problems


def in_force(paths, read):
    """`({crate: (record path, ratified, rows)}, problems)` for the crates whose record replaces their blocks.

    A crate is in force when its record carries a dated `Ratified:` line and `crates/<crate>/` is
    still in the tree.
    """
    found, problems = load(paths, read)
    crates = {p.split("/")[1] for p in paths if p.startswith("crates/") and p.count("/") > 1}
    return {c: v for c, v in found.items() if v[1] and c in crates}, problems


def apply(release, paths, read, is_module_file, pub_items):
    """`(records, problems, missing)` for the crates in `release`, replacing their blocks.

    `is_module_file` and `pub_items` are the census's, passed in so the two cannot disagree about
    what a module or an item is.
    """
    records, problems, missing = [], [], 0
    for crate, (record, ratified, rows) in sorted(release.items()):
        prefix = f"crates/{crate}/"
        mine = [p for p in paths if p.startswith(prefix)]
        for rel in mine:
            if not rel.endswith(crate_release.TEXT):
                continue
            for number, line in enumerate((read(rel) or "").split("\n"), 1):
                if PROVENANCE.search(line):
                    problems.append(f"{rel}:{number}: a `Name:` block in `{crate}`, whose names are "
                                    f"ratified in {record} ({ratified}). Its row there is the "
                                    f"record and `script/crate-standalone` refuses the block; move "
                                    f"anything the block holds that the row lacks, then delete it")
        modules = {p: _module_stem(p) for p in mine if is_module_file(p) and p != prefix + "src/lib.rs"}
        items = {p: _item_lines(read(p) or "") for p in mine if is_module_file(p)}
        manifest = read(prefix + "Cargo.toml") or ""
        features = set(re.findall(r"(?m)^([A-Za-z0-9_-]+)\s*=", manifest.split("[features]", 1)[1]
                                  .split("\n[", 1)[0])) if "[features]" in manifest else set()
        covered = set()
        for number, name, kind, path, cell in rows:
            where = f"{record}:{number}"
            source = prefix + path
            if kind == "crate":
                ok = name == crate and path == "src/lib.rs"
            elif kind == "module":
                ok = modules.get(source) == name
            elif kind == "item":
                member = None if name in items.get(source, {}) else _member_line(read(source) or "", name)
                ok = name in items.get(source, {}) or member is not None
            else:
                ok = path == "Cargo.toml" and name in features
            if not ok:
                problems.append(f"{where}: `{crate}` has no {kind} `{name}` at `{path}`; fix the "
                                f"row or remove it")
                continue
            status, date, refused, problem = name_provenance.classify(cell)
            if problem is not None:
                problems.append(f"{where}: the status must read as a `Name:` block does, "
                                f"`ratified <date> (...)`, `recorded (where)`, `provisional` or "
                                f"`unrecorded` (found {cell[:48]!r})")
                missing += kind != "feature"
                covered.add((kind, source, name))
                continue
            covered.add((kind, source, name))
            if kind == "feature":
                continue    # a stranger reads it, and the census counts no feature names
            rec = {"kind": kind, "name": name, "path": source, "status": status, "date": date,
                   "refused": refused, "text": cell, "record": where}
            if kind == "item" and member is not None:
                rec.update(path=f"{source}:{member[0]}", what=member[1])
            elif kind == "item":
                line, what = items[source][name]
                rec.update(path=f"{source}:{line}", what=what)
                # One record per item, as the census counts them, so the total does not move.
                count = pub_items(read(source) or "").count(name)
                records.extend(dict(rec) for _ in range(count))
                continue
            if kind == "module":
                rec["what"] = "module"
            records.append(rec)
        if ("crate", prefix + "src/lib.rs", crate) not in covered:
            problems.append(f"{record}: no row for the crate `{crate}` itself (kind `crate`, path "
                            f"`src/lib.rs`)")
            missing += 1
        for source, stem in sorted(modules.items()):
            if ("module", source, stem) not in covered:
                problems.append(f"{source}: module `{stem}` has no row in {record}; add one, "
                                f"`provisional` until ratified (`python3 helpers/name_release.py "
                                f"--draft {crate}` prints it)")
                missing += 1
        for source in sorted(items):
            for name in sorted(set(pub_items(read(source) or ""))):
                if ("item", source, name) not in covered:
                    problems.append(f"{source}: `pub` item `{name}` has no row in {record}; add "
                                    f"one, `provisional` until ratified")
                    missing += 1
    return records, problems, missing


# --- the verbs ------------------------------------------------------------------------------------

def _tree():
    root = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True,
                          text=True, check=True).stdout.strip()
    listed = subprocess.run(["git", "-C", root, "ls-files", "-z", "--cached", "--others",
                             "--exclude-standard"], capture_output=True, text=True,
                            check=True).stdout
    paths = sorted(p for p in listed.split("\0") if p and os.path.isfile(os.path.join(root, p)))

    def read(rel):
        try:
            return open(os.path.join(root, rel), encoding="utf-8", errors="replace").read()
        except OSError:
            return None
    return paths, read


def draft(crate):
    """Print the record's rows for `crate` from today's tree: each name and its block's status."""
    import name_census
    paths, read = _tree()
    if crate not in track(paths, read):
        print(f"name_release: `{crate}` is not on the release track", file=sys.stderr)
        return 1
    prefix = f"crates/{crate}/"
    records, _problems, _missing, unblocked = name_census.census(paths, read)
    status = {}
    for r in records:
        path = r["path"].split(":")[0]
        if path.startswith(prefix):
            status[(r["kind"], path, r["name"])] = r["text"].split("\n")[0]
    for row in unblocked:
        path, _, name = row.partition("::")
        if path.startswith(prefix):
            status[("item" if name else "module", path, name or _module_stem(path))] = "no block"
    print("| name | kind | path | status |\n|---|---|---|---|")
    order = {"crate": 0, "module": 1, "item": 2}
    for (kind, path, name), text in sorted(status.items(), key=lambda k: (order[k[0][0]], k[0][1:])):
        if kind in order:
            cell = text.replace("|", "\\|")
            print(f"| `{name}` | {kind} | `{path[len(prefix):]}` | {cell} |")
    manifest = read(prefix + "Cargo.toml") or ""
    if "[features]" in manifest:
        for f in re.findall(r"(?m)^([A-Za-z0-9_-]+)\s*=", manifest.split("[features]", 1)[1]
                            .split("\n[", 1)[0]):
            print(f"| `{f}` | feature | `Cargo.toml` | no block |")
    return 0


def selftest():
    import name_census
    block = "# 813. `fat`: a crate\n"
    crate = ("//! The crate.\n\npub mod a;\n\n/// A volume.\npub struct Volume;\n"
             "pub fn new() {}\npub fn new() {}\npub enum Error {\n    Full,\n}\n")
    module = "//! A module.\n\npub fn read() {}\n"
    manifest = '[package]\nname = "fat"\n\n[features]\nstd = []\n'
    rows = ("Ratified: 2026-11-02 (#1).\n\n| name | kind | path | status |\n|---|---|---|---|\n"
            "| `fat` | crate | `src/lib.rs` | ratified 2026-10-07 (#1806). |\n"
            "| `a` | module | `src/a.rs` | ratified 2026-11-02 (#1). |\n"
            "| `Volume` | item | `src/lib.rs` | ratified 2026-11-02 (#1). Refused `Fs` (vague). |\n"
            "| `new` | item | `src/lib.rs` | ratified 2026-11-02 (#1). |\n"
            "| `read` | item | `src/a.rs` | provisional. |\n"
            "| `Error` | item | `src/lib.rs` | provisional. |\n"
            "| `Full` | item | `src/lib.rs` | provisional. |\n"
            "| `std` | feature | `Cargo.toml` | ratified 2026-11-02 (#1). |\n")
    base = {"design/roadmap/0813-fat.md": block, "crates/fat/src/lib.rs": crate,
            "crates/fat/src/a.rs": module, "crates/fat/Cargo.toml": manifest,
            "crates/other/src/lib.rs": "pub fn f() {}\n",
            f"{RECORD_DIR}/fat-names.md": rows, "design/naming/convention-rulings.md": ""}

    def run(tree):
        return name_census.census(sorted(tree), tree.get)

    def problems_of(tree):
        return [p for p in run(tree)[1] if "fat" in p or RECORD_DIR in p]

    recs, _p, missing, unblocked = run(base)
    fat = {(r["kind"], r["name"]): r for r in recs if r["path"].startswith("crates/fat/")}
    unblocked_rows = dict(base, **{f"{RECORD_DIR}/fat-names.md": rows.split("\n", 1)[1]})
    blocked = dict(unblocked_rows, **{"crates/fat/src/lib.rs": "//! Name: provisional.\n" + crate})
    cases = [
        ("an in-force record replaces every block in its crate",
         not problems_of(base) and not [u for u in unblocked if u.startswith("crates/fat/")]),
        ("rows become records with their status and refusals",
         fat[("crate", "fat")]["status"] == "ratified" and fat[("item", "Volume")]["refused"] == ["Fs"]
         and fat[("item", "read")]["status"] == "provisional"
         and fat[("item", "Volume")]["path"].rpartition(":")[2] == "6"),
        ("a variant takes a row, as a block may name one",
         fat[("item", "Full")]["what"] == "variant"),
        ("a crate off the track keeps the block rule",
         any("crate other: no `Name:` block" in p for p in run(base)[1])
         and "crates/other/src/lib.rs::f" in unblocked),
        ("a record with no `Ratified:` line changes nothing",
         any("crate fat: no `Name:` block" in p for p in run(unblocked_rows)[1])
         and "crates/fat/src/a.rs" in run(unblocked_rows)[3]),
        ("before ratification a block in the crate passes",
         not any("fat" in p for p in run(blocked)[1] if "Name:` block in" in p)),
        ("a block left in a ratified crate fails",
         any("a `Name:` block in `fat`" in p
             for p in problems_of(dict(base, **{"crates/fat/src/a.rs": "//! Name: provisional.\n" + module})))),
        ("a new pub item with no row fails, naming the record",
         any("`pub` item `write` has no row" in p
             for p in problems_of(dict(base, **{"crates/fat/src/a.rs": module + "pub fn write() {}\n"})))),
        ("a new module with no row fails",
         any("module `b` has no row" in p
             for p in problems_of(dict(base, **{"crates/fat/src/b.rs": "fn x() {}\n"})))),
        ("a row for a name the crate lacks fails",
         any("has no item `gone`" in p for p in problems_of(dict(base, **{
             f"{RECORD_DIR}/fat-names.md": rows + "| `gone` | item | `src/a.rs` | provisional. |\n"})))),
        ("a malformed status fails",
         any("must read as a `Name:` block" in p for p in problems_of(dict(base, **{
             f"{RECORD_DIR}/fat-names.md": rows.replace("provisional.", "maybe.")})))),
        ("a record for a crate off the track fails",
         any("`other` is not on the release track" in p
             for p in run(dict(base, **{f"{RECORD_DIR}/other-names.md": rows}))[1])),
        ("a crate that has left reads nothing",
         not problems_of({k: v for k, v in base.items() if not k.startswith("crates/fat/")})),
        # A variant counts only where it carries a record, as it counts only where it carries a block.
        ("the total does not move when a record replaces the blocks",
         len(recs) + missing + len(unblocked) - 1
         == len(run(unblocked_rows)[0]) + run(unblocked_rows)[2] + len(run(unblocked_rows)[3])),
    ]
    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f"name_release selftest: {name}", file=sys.stderr)
    if not failed:
        print(f"name_release selftest: {len(cases)} cases pass")
    return 1 if failed else 0


if __name__ == "__main__":
    if sys.argv[1:] == ["--selftest"]:
        sys.exit(selftest())
    if len(sys.argv) == 3 and sys.argv[1] == "--draft":
        sys.exit(draft(sys.argv[2]))
    sys.exit("usage: name_release.py --selftest | --draft <crate>")
