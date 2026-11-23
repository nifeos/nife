"""The record's extraction path lists, written from `packages/homes.toml` and checked against it.

Milestone 1018 (the design record moves to `nifeos/record` and is published as a website), step 6.
The extraction runs `git filter-repo` twice over a clone of this repository: once keeping the paths
in `keep`, then once more dropping the paths in `drop`. This writes both files from the home of
every tracked path, as helpers/packages.py sorts it, and checks that the two passes together
select exactly the files that move:

- `keep` holds every claim whose home is the repository `record`, the record's own tooling (the
  `TOOLS` below) and `DECISIONS.md`, which held the decisions before they split into one file each
  and so exists only in history.
- `drop` holds every claim whose home is elsewhere and that lies under a `keep` directory, such
  as a baseline under `design/` or a note about one crate alone under `notes/`.

The check reads each list the way `git filter-repo` reads a path: a line names that file, or every
file under it when it is a directory. A tracked file the two passes select must be one whose home
is the record or one of the tools, and every such file must be selected. A claim nested three deep
(a record path under a staying path under a record path) is the one shape two passes cannot
express, and the check refuses it.

    python3 helpers/record_paths.py                 # the check, and the counts
    python3 helpers/record_paths.py --check         # what script/lint runs: the check, one line
    python3 helpers/record_paths.py --write <dir>   # the check, then <dir>/keep and <dir>/drop
    python3 helpers/record_paths.py --selftest      # fixtures, no tree and no git

Name: provisional, minted by the lane for milestone 1018's step 6 on 2026-11-23 (UTC). A python
module under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the
record.

BUGS

- `TOOLS` is a list here, not a home in `packages/homes.toml`, because those scripts read the tree
  as part of the build and gates until the record leaves. A tool added to the record's side must
  be added here by hand.
"""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import packages                                                             # noqa: E402

RECORD = "record"

# The record's own tooling, which leaves with it and keeps its history (milestone 1018's step 6
# plan, "What moves").
TOOLS = (
    "script/roadmap",
    "script/decisions",
    "script/journeys",
    "script/citations",
    "helpers/roadmap_block.py",
    "helpers/roadmap_proposals.py",
    "helpers/coe_actions.py",
    "helpers/roadmap_migrate.py",
)

# Paths only history holds. Each is kept so the record's history reaches back past it, and each
# must stay untracked, since a tracked one would move without a home that says so.
HISTORICAL = ("DECISIONS.md",)

HEADER = "# written by helpers/record_paths.py from packages/homes.toml; one git filter-repo path a line\n"


def matches(entry, path):
    """Whether a path-file line names `path`, as `git filter-repo` reads one."""
    if entry.endswith("/"):
        return path.startswith(entry)
    return path == entry or path.startswith(entry + "/")


def moves(home):
    return bool(home) and home.get("repo") == RECORD


def lists(claim_homes):
    """(keep, drop) from {claim prefix: its home}."""
    record = sorted(p for p, h in claim_homes.items() if moves(h))
    record_dirs = [p for p in record if p.endswith("/")]
    keep = record + [t for t in TOOLS if t not in record] + list(HISTORICAL)
    drop = sorted(p for p, h in claim_homes.items()
                  if not moves(h) and any(matches(d, p) and d != p for d in record_dirs))
    return keep, drop


def check(tracked, file_home, keep, drop):
    """Every way the two passes disagree with the homes, as messages; none when they agree."""
    errors = []
    tracked = set(tracked)
    for t in TOOLS:
        if t not in tracked:
            errors.append(f"{t} is a record tool and is not tracked; remove it from TOOLS")
    for h in HISTORICAL:
        if h in tracked:
            errors.append(f"{h} is tracked, so it needs a home, not a place in HISTORICAL")
    for entry in keep + drop:
        if entry not in HISTORICAL and not any(matches(entry, p) for p in tracked):
            errors.append(f"{entry} names no tracked path")
    for path in sorted(tracked):
        selected = any(matches(k, path) for k in keep) and not any(matches(d, path) for d in drop)
        expected = moves(file_home.get(path)) or path in TOOLS
        if selected and not expected:
            errors.append(f"{path} would move, and its home is not the record")
        elif expected and not selected:
            errors.append(f"{path} has the record as its home, and the two passes leave it behind")
    return errors


def selftest():
    rec = {"status": "provisional", "repo": RECORD}
    stay = {"status": "undecided", "reason": "tool data"}
    other = {"status": "provisional", "repo": "contracts"}
    claims = {"design/": rec, "notes/": rec, "notes/skills/": stay, "design/base.tsv": stay,
              "notes/c.md": other, "script/": stay, "crates/x/": other}
    keep, drop = lists(claims)
    assert keep == ["design/", "notes/"] + list(TOOLS) + ["DECISIONS.md"], keep
    assert drop == ["design/base.tsv", "notes/c.md", "notes/skills/"], drop
    tracked = ["design/a.md", "design/base.tsv", "notes/b.md", "notes/c.md", "notes/skills/s.md",
               "crates/x/lib.rs", "notes/cmore.md"] + list(TOOLS)
    homes = {p: (stay if p.startswith(("script/", "helpers/")) else None) for p in TOOLS}
    homes.update({"design/a.md": rec, "design/base.tsv": stay, "notes/b.md": rec,
                  "notes/c.md": other, "notes/skills/s.md": stay, "crates/x/lib.rs": other,
                  "notes/cmore.md": rec})
    # notes/c.md must not drop notes/cmore.md: a file line names that file alone.
    assert check(tracked, homes, keep, drop) == [], check(tracked, homes, keep, drop)
    # Planted: a record path nested under a staying one under a record one.
    claims3 = dict(claims, **{"notes/skills/deep/": rec})
    homes3 = dict(homes, **{"notes/skills/deep/d.md": rec})
    k3, d3 = lists(claims3)
    got = check(tracked + ["notes/skills/deep/d.md"], homes3, k3, d3)
    assert got == ["notes/skills/deep/d.md has the record as its home, and the two passes leave it "
                   "behind"], got
    # Planted: a tool that is gone, a historical path that is tracked, a line that names nothing.
    got = check([p for p in tracked if p != TOOLS[0]] + ["DECISIONS.md"], homes, keep,
                drop + ["notes/gone/"])
    assert any(TOOLS[0] in e and "not tracked" in e for e in got), got
    assert any(e.startswith("DECISIONS.md is tracked") for e in got), got
    assert any(e.startswith("DECISIONS.md would move") for e in got), got
    assert "notes/gone/ names no tracked path" in got, got
    # Planted: a staying file under a record directory with no drop line.
    got = check(tracked, homes, keep, [d for d in drop if d != "design/base.tsv"])
    assert got == ["design/base.tsv would move, and its home is not the record"], got
    print("record_paths: selftest passed")


def main(argv):
    if argv == ["--selftest"]:
        selftest()
        return 0
    if not (argv in ([], ["--check"]) or (len(argv) == 2 and argv[0] == "--write")):
        print(__doc__.split("\n\n")[4], file=sys.stderr)
        return 2
    tracked, texts = packages.working_tree()
    result = packages.evaluate(tracked, texts)
    if result["errors"]:
        print("record_paths: helpers/packages.py --check fails; fix the sort first", file=sys.stderr)
        return 1
    keep, drop = lists(result["claim_homes"])
    errors = check(tracked, result["file_home"], keep, drop)
    for e in errors:
        print(f"record_paths: {e}", file=sys.stderr)
    if errors:
        return 1
    moving = sum(1 for p in tracked if moves(result["file_home"].get(p))) + len(TOOLS)
    print(f"record_paths: keep {len(keep)} paths, drop {len(drop)}; together they select the "
          f"{moving} tracked files that move with the record, and no other")
    if argv == []:
        for name, entries in (("keep", keep), ("drop", drop)):
            print(f"{name}:")
            for e in entries:
                print(f"  {e}")
    if argv[:1] == ["--write"]:
        out = argv[1]
        os.makedirs(out, exist_ok=True)
        for name, entries in (("keep", keep), ("drop", drop)):
            with open(os.path.join(out, name), "w", encoding="utf-8") as f:
                f.write(HEADER + "".join(e + "\n" for e in entries))
        print(f"record_paths: wrote {os.path.join(out, 'keep')} and {os.path.join(out, 'drop')}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
