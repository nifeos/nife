"""The census-wide check, and the shape rules for the triage file's `kind`, `verified` and `home` columns.

Part 2 of the proposal design/roadmap/proposals/a-survivor-accepted-once-is-not-accepted-forever.md
(milestone number provisional, minted by the integrator when the proposal is promoted). Part 1 is
helpers/mutation_real_holes.py. The inflow check (helpers/mutation_inflow.py) fails only on a
survivor it can blame to a pull request merged since the last census, and the diff gate
(helpers/mutation_diff_gate.py) reads only a pull request's own lines, so a survivor on an older line
with no row failed nothing: 91 of the 2026-10-26 census's 712 sat that way. This module closes it.

    python3 helpers/mutation_survivor_rows.py census SHARD_DIR... [--summary FILE]  # the weekly job
    python3 helpers/mutation_survivor_rows.py census --census DATE                  # a committed list
    python3 helpers/mutation_survivor_rows.py lint        # script/lint: the file's rules and baselines
    python3 helpers/mutation_survivor_rows.py bank        # lower the baselines to the tree's counts
    python3 helpers/mutation_survivor_rows.py rowless --census DATE   # (re)write the carried list
    python3 helpers/mutation_survivor_rows.py --selftest

THE CENSUS CHECK walks every survivor key the census holds through CENSUS_RULES, a list of
`(name, rule)` pairs, each `rule(key, outcomes, row) -> problem or None`. Today there is one rule,
`row`: the key needs an `equivalent` or `gap` row, and a `killed` row for a live survivor is the
census contradicting it, the gate's own rule. A later rule is one more pair in that list and, when it
reads an outcome other than `missed`, one more pair in OUTCOME_FILES (the timeouts proposal adds
`timeout.txt`). Exit 0: no problem. Exit 1: problems, listed. Exit 2: bad input.

THE CARRIED LIST, notes/project-metrics/mutation-rowless-survivors.csv, holds the keys that fail the
`row` rule today (the 91 unrowed mutants and the 3 stale `killed` rows, as 50 keys). They are carried so the
weekly job is red only for a new problem. Part 3's driver-crate lanes empty it: a key leaves the list
in the pull request that gives it a live row, and `lint` fails a listed key that has one. A rule other
than `row` has no carried list; add one only with its own file.

THE TRIAGE COLUMNS. The shape of each row (mutation_inflow.row_problem) is enforced on every read.
`lint` adds what needs the tree:

- a `gap` row's `home` names something that exists: `milestone N` (a block in design/roadmap/),
  `proposal SLUG` (a file in design/roadmap/proposals/) or `BUGS PATH` (a file with a BUGS section);
- two counts are held to a baseline, notes/project-metrics/mutation-triage-baseline.txt, that only
  falls: `unclassified` (`equivalent` rows nobody has read, kind `unclassified`) and `homeless_gap`
  (`gap` rows with no home). Both are above zero only for rows that predate the columns. A new row
  must arrive classified and, if a gap, housed. `bank` lowers the baseline after a lane improves it.

Names (`kind`, `verified`, `home`, `unclassified`, the two files): provisional, minted by the part 2
lane on 2026-10-27 (UTC). An architect names things.

BUGS

- The check reads the committed survivor list or a census's `missed.txt`, so a mutant that timed out
  is not a survivor here. The timeouts proposal adds that outcome through OUTCOME_FILES.
- `verified` is a date the author writes. Nothing checks that the author read the code; the diff gate
  checks only that a row for a survivor on a changed line changed in the same pull request.
- A `BUGS PATH` home is checked for a BUGS heading in the file, not for an entry about the row.
"""

import argparse
import collections
import csv
import pathlib
import re
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import mutation_inflow as mi  # noqa: E402

TRIAGE = pathlib.Path("notes/project-metrics/mutation-triage.csv")
RENAMES = pathlib.Path("notes/project-metrics/mutation-census-renames.csv")
SURVIVORS = pathlib.Path("notes/project-metrics/mutation-survivors")
CARRIED = pathlib.Path("notes/project-metrics/mutation-rowless-survivors.csv")
BASELINE = pathlib.Path("notes/project-metrics/mutation-triage-baseline.txt")
CARRIED_FIELDS = ["crate", "function", "mutation"]
# The outcome files of a cargo-mutants shard, and the outcome each one names. The timeouts proposal
# appends ("timeout.txt", "timeout"); a rule below then sees it in `outcomes`.
OUTCOME_FILES = (("missed.txt", "missed"),)
BUGS_HEADING = re.compile(r"(?m)^[#*\s]*BUGS\b")


def live(row):
    return row is not None and row.disposition in ("equivalent", "gap")


def rule_row(key, outcomes, row):
    """Every survivor needs a live row. A `killed` row is contradicted by the census itself."""
    if row is None:
        return "no row"
    if row.disposition == "killed":
        return "only a `killed` row, which this census contradicts"
    return None


# Each rule is `(name, function)`; the name is what the carried list and the report call it.
CENSUS_RULES = [("row", rule_row)]


def read_outcomes(dirs):
    """key -> set of outcomes, from shard directories. Keys are not yet through the rename chain."""
    out = collections.defaultdict(set)
    found = False
    for d in dirs:
        for name, outcome in OUTCOME_FILES:
            p = pathlib.Path(d) / name
            if not p.exists():
                continue
            found = True
            for raw in p.read_text().splitlines():
                parsed = mi.parse_mutant(raw)
                if parsed:
                    out[parsed[:3]].add(outcome)
    if not found:
        raise FileNotFoundError(f"no {' or '.join(n for n, _ in OUTCOME_FILES)} under {' '.join(map(str, dirs))}")
    return out


def read_committed(date, root="."):
    root = pathlib.Path(root)
    path = root / SURVIVORS / f"{date}.csv.gz" if date else sorted((root / SURVIVORS).glob("*.csv.gz"))[-1]
    meta, counts = mi.read_snapshot(path)
    return meta, {key: {"missed"} for key in counts}


def read_carried(path, chain):
    out = set()
    p = pathlib.Path(path)
    if not p.exists():
        return out
    rows = list(csv.DictReader(p.open()))
    if rows and list(rows[0].keys()) != CARRIED_FIELDS:
        raise ValueError(f"{path}: header must be {','.join(CARRIED_FIELDS)}")
    for i, r in enumerate(rows, start=2):
        key = (mi.resolve(r["crate"], chain), r["function"], r["mutation"])
        if key in out:
            raise ValueError(f"{path}:{i}: duplicate key {key}")
        out.add(key)
    return out


def census_check(survivors, triage, carried, chain):
    """Problems as (key, outcomes, rule, why), sorted. `survivors` is key -> set of outcomes."""
    merged = collections.defaultdict(set)
    for (crate, function, mutation), outcomes in survivors.items():
        merged[(mi.resolve(crate, chain), function, mutation)] |= outcomes
    problems = []
    for key in sorted(merged):
        row = triage.get(key)
        for name, rule in CENSUS_RULES:
            why = rule(key, merged[key], row)
            if why and not (name == "row" and key in carried):
                problems.append((key, merged[key], name, why))
    return problems


def render(label, total, problems, carried_n):
    out = [f"### Mutation census rows: {label}", ""]
    out.append(f"{total} survivor key(s); {carried_n} carried in {CARRIED} (to be emptied by giving them rows).")
    if not problems:
        out += ["", "Every other survivor has a live row."]
        return "\n".join(out) + "\n"
    out += ["", f"**{len(problems)} survivor key(s) fail**, each needs a row in {TRIAGE} "
            "(`equivalent` with a kind, or `gap` with a home), or a test that kills it:", "",
            "| crate | function | mutation | outcomes | rule | why |", "|---|---|---|---|---|---|"]
    for (crate, function, mutation), outcomes, name, why in problems:
        out.append(f"| {crate} | `{function}` | `{mutation}` | {', '.join(sorted(outcomes))} | {name} | {why} |")
    return "\n".join(out) + "\n"


def home_problem(home, root):
    """Why a `gap` row's home names nothing, or None."""
    root = pathlib.Path(root)
    kind, _, rest = home.partition(" ")
    if kind == "milestone":
        if not list((root / "design/roadmap").glob(f"{int(rest):04d}-*")):
            return f"home {home!r}: no design/roadmap/{int(rest):04d}-* block"
    elif kind == "proposal":
        if not (root / "design/roadmap/proposals" / f"{rest}.md").is_file():
            return f"home {home!r}: no design/roadmap/proposals/{rest}.md"
    elif kind == "BUGS":
        p = root / rest
        if not p.is_file() or not BUGS_HEADING.search(p.read_text(errors="replace")):
            return f"home {home!r}: {rest} is not a file with a BUGS section"
    return None


def counts_of(triage):
    c = collections.Counter()
    for row in triage.values():
        if row.kind == "unclassified":
            c["unclassified"] += 1
        if row.disposition == "gap" and not row.home:
            c["homeless_gap"] += 1
    return c


def read_baseline(path):
    out = {}
    p = pathlib.Path(path)
    if p.exists():
        for line in p.read_text().splitlines():
            if line.strip() and not line.startswith("#"):
                name, _, n = line.partition("=")
                out[name.strip()] = int(n)
    return out


def lint_problems(root="."):
    """Problems for the tree at `root`: the homes, the baselines, the carried list."""
    root = pathlib.Path(root)
    chain = mi.load_renames(root / RENAMES)
    triage = mi.read_triage(root / TRIAGE, chain)  # raises on a malformed row
    problems = []
    for key, row in sorted(triage.items()):
        if row.home:
            why = home_problem(row.home, root)
            if why:
                problems.append(f"{'/'.join(key[:2])}: {why}")
    have, ceiling = counts_of(triage), read_baseline(root / BASELINE)
    for name in ("unclassified", "homeless_gap"):
        if name not in ceiling:
            problems.append(f"{BASELINE}: no `{name}=N` line")
        elif have[name] > ceiling[name]:
            what = ("an `equivalent` row with kind `unclassified`; give each new row `algebraic` or "
                    "`caller` and the date you verified it" if name == "unclassified" else
                    "a `gap` row with no home; name a `milestone N`, `proposal SLUG` or `BUGS PATH`")
            problems.append(f"{name} is {have[name]}, above its baseline {ceiling[name]}: the tree gained {what}")
    for key in sorted(read_carried(root / CARRIED, chain)):
        if live(triage.get(key)):
            problems.append(f"{CARRIED}: {'/'.join(key[:2])} {key[2]!r} now has a row; remove it from the list")
    return problems


def bank(root="."):
    root = pathlib.Path(root)
    chain = mi.load_renames(root / RENAMES)
    have = counts_of(mi.read_triage(root / TRIAGE, chain))
    ceiling = read_baseline(root / BASELINE)
    for name in ("unclassified", "homeless_gap"):
        if name in ceiling and have[name] > ceiling[name]:
            raise ValueError(f"{name} is {have[name]}, above its baseline {ceiling[name]}; fix the rows, never bank up")
    text = ("# The triage file's two legacy counts. They only fall: `python3 helpers/mutation_survivor_rows.py bank`\n"
            "# lowers them after a lane improves the tree, and `lint` fails a count above its line.\n"
            + "".join(f"{n}={have[n]}\n" for n in ("unclassified", "homeless_gap")))
    (root / BASELINE).write_text(text)
    return have


def write_carried(date, root="."):
    root = pathlib.Path(root)
    chain = mi.load_renames(root / RENAMES)
    triage = mi.read_triage(root / TRIAGE, chain)
    _meta, surv = read_committed(date, root)
    keys = sorted({key for key, _o, name, _w in census_check(surv, triage, set(), chain) if name == "row"})
    with (root / CARRIED).open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(CARRIED_FIELDS)
        w.writerows(keys)
    return len(keys)


def main(argv):
    if argv[:1] == ["--selftest"]:
        return selftest()
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("census")
    c.add_argument("shards", nargs="*")
    c.add_argument("--census")
    c.add_argument("--summary")
    sub.add_parser("lint")
    sub.add_parser("bank")
    r = sub.add_parser("rowless")
    r.add_argument("--census")
    args = ap.parse_args(argv)
    try:
        if args.cmd == "lint":
            problems = lint_problems()
            for p in problems:
                print(f"mutation_survivor_rows: {p}", file=sys.stderr)
            return 1 if problems else 0
        if args.cmd == "bank":
            print("mutation_survivor_rows: banked", dict(bank()))
            return 0
        if args.cmd == "rowless":
            print(f"mutation_survivor_rows: {write_carried(args.census)} key(s) carried")
            return 0
        chain = mi.load_renames(RENAMES)
        triage = mi.read_triage(TRIAGE, chain)
        carried = read_carried(CARRIED, chain)
        if args.shards:
            surv, label = read_outcomes(args.shards), "this census"
        else:
            meta, surv = read_committed(args.census)
            label = f"census {meta['census']}"
        problems = census_check(surv, triage, carried, chain)
        text = render(label, len(surv), problems, len(carried))
    except (ValueError, FileNotFoundError, IndexError) as e:
        print(f"mutation_survivor_rows: {e}", file=sys.stderr)
        return 2
    print(text)
    if args.summary:
        with open(args.summary, "a") as fh:
            fh.write(text)
    return 1 if problems else 0


def selftest():
    R = mi.Row
    key = lambda f: ("c", f, "replace < with <=")  # noqa: E731
    triage = {
        key("ok"): R("equivalent", "r", "algebraic", "2026-10-27"),
        key("gap"): R("gap", "r", "", "", "milestone 1"),
        key("stale"): R("killed", "r"),
    }
    surv = {key(f): {"missed"} for f in ("ok", "gap", "stale", "none", "carried")}
    got = census_check(surv, triage, {key("carried")}, {})
    assert [(p[0][1], p[2]) for p in got] == [("none", "row"), ("stale", "row")], got
    # a renamed crate's survivor meets the row written under the new name
    assert census_check({("old", "ok", "replace < with <="): {"missed"}},
                        {("c", "ok", "replace < with <="): triage[key("ok")]}, set(), {"old": "c"}) == []
    assert "2 survivor key(s) fail" in render("x", 5, got, 1)

    # the shape rules of the three columns
    ok = lambda **kw: mi.row_problem(R(**kw))  # noqa: E731
    assert ok(disposition="equivalent", reason="r", kind="unclassified") is None
    assert ok(disposition="equivalent", reason="r", kind="caller", verified="2026-10-27") is None
    assert "needs a kind" in ok(disposition="equivalent", reason="r")
    assert "needs the date" in ok(disposition="equivalent", reason="r", kind="algebraic")
    assert "has not been read" in ok(disposition="equivalent", reason="r", kind="unclassified", verified="2026-10-27")
    assert "no kind" in ok(disposition="gap", reason="r", kind="caller")
    assert "only a `gap` row" in ok(disposition="killed", reason="r", home="milestone 3")
    assert "not `milestone N`" in ok(disposition="gap", reason="r", home="soon")
    assert ok(disposition="gap", reason="r", home="BUGS notes/x.md") is None
    assert "YYYY-MM-DD" in ok(disposition="gap", reason="r", verified="yesterday")

    with tempfile.TemporaryDirectory() as tmp:
        root = pathlib.Path(tmp)
        for d in ("design/roadmap/proposals", "notes", "notes/project-metrics"):
            (root / d).mkdir(parents=True, exist_ok=True)
        (root / "design/roadmap/0007-a-thing.md").write_text("# a thing\n")
        (root / "design/roadmap/proposals/later.md").write_text("# later\n")
        (root / "notes/has.md").write_text("text\n\n## BUGS\n\n- a limit\n")
        (root / "notes/lacks.md").write_text("text\n")
        assert home_problem("milestone 7", root) is None
        assert "no design/roadmap/0008-*" in home_problem("milestone 8", root)
        assert home_problem("proposal later", root) is None
        assert home_problem("proposal never", root)
        assert home_problem("BUGS notes/has.md", root) is None
        assert home_problem("BUGS notes/lacks.md", root) and home_problem("BUGS notes/none.md", root)

        head = ",".join(mi.TRIAGE_FIELDS) + "\n"
        tri = root / TRIAGE
        tri.write_text(head + "c,f,replace < with <=,equivalent,r,unclassified,,\n"
                       "c,g,replace < with <=,gap,r,,,\n"
                       "c,h,replace < with <=,gap,r,,,milestone 7\n")
        assert dict(counts_of(mi.read_triage(tri, {}))) == {"unclassified": 1, "homeless_gap": 1}
        (root / BASELINE).write_text("unclassified=1\nhomeless_gap=1\n")
        assert lint_problems(root) == []
        # a new unclassified row, a new homeless gap and a dangling home each fail
        tri.write_text(tri.read_text() + "c,i,replace < with <=,equivalent,r,unclassified,,\n"
                       "c,j,replace < with <=,gap,r,,,milestone 9\n"
                       "c,k,replace < with <=,gap,r,,,\n")
        got = lint_problems(root)
        assert len(got) == 3 and any("unclassified is 2" in p for p in got) \
            and any("homeless_gap is 2" in p for p in got) and any("milestone 9" in p for p in got), got
        # banking up is refused, banking down is not
        try:
            bank(root)
        except ValueError:
            pass
        else:
            raise AssertionError("banked up")
        tri.write_text(head + "c,f,replace < with <=,equivalent,r,unclassified,,\n")
        assert bank(root)["unclassified"] == 1 and read_baseline(root / BASELINE) == {"unclassified": 1, "homeless_gap": 0}
        # a carried key that now has a row must leave the list
        (root / CARRIED).write_text("crate,function,mutation\nc,f,replace < with <=\nc,zz,replace < with <=\n")
        got = lint_problems(root)
        assert len(got) == 1 and "now has a row" in got[0], got

        # shards: missed.txt lines key like the inflow check, and no outcome file is an input error
        shard = root / "shard"
        shard.mkdir()
        (shard / "missed.txt").write_text("crates/c/src/lib.rs:3:5: replace < with <= in f\n")
        assert read_outcomes([shard]) == {("c", "f", "replace < with <="): {"missed"}}
        try:
            read_outcomes([root / "nothing"])
        except FileNotFoundError:
            pass
        else:
            raise AssertionError("an empty input passed")
    print("mutation_survivor_rows: selftest ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
