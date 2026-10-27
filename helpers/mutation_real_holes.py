"""Classify a census's survivors by mutation shape and by what their triage row claims, and count the real holes.

Part 1 of the proposal design/roadmap/proposals/a-survivor-accepted-once-is-not-accepted-forever.md
(milestone number provisional, minted by the integrator when the proposal is promoted). A census
counts every missed mutant alike, but most are mutants no test can kill: an OR swapped for an
exclusive OR over operands that share no bit, a value shifted by zero or zero shifted. The number
that matters is the real holes, the survivors that no algebraic argument answers. This script
computes it from the census's survivor list and the triage file, so the number can be regenerated.

    python3 helpers/mutation_real_holes.py write [--census DATE]   # regenerate the two committed outputs
    python3 helpers/mutation_real_holes.py check [--census DATE]   # fail when they differ from a regeneration
    python3 helpers/mutation_real_holes.py --selftest              # fixtures only; script/lint

Inputs are the ones helpers/mutation_inflow.py reads, under its key and rename table: the committed
survivor list notes/project-metrics/mutation-survivors/DATE.csv.gz (default: the newest) and
notes/project-metrics/mutation-triage.csv. Outputs: notes/project-metrics/mutation-survivor-classes.csv
(one row per survivor key) and notes/mutation-testing/survivor-classification.md (the per-crate table).

THE SHAPE comes from the mutation text alone: `replace | with ^` is an `or_xor` swap, `replace << with
>>` or the reverse is a `shift`, and so on. Text cannot say whether the operands share a bit or the
shift is by zero, so a shape is evidence and a row's reason is the claim. The class of a survivor:

- `noise`: the shape is `or_xor` or a zero `shift`, and its row is `equivalent` with a reason that
  states the argument (disjoint or distinct bits, no shared bit, a shift of `1 << 0` or by zero).
- `noise_unargued`: the same shapes with an `equivalent` row whose reason does not state it. Counted
  as noise, flagged for a better reason.
- `algebraic`: any other `equivalent` row whose reason names no caller and no harness.
- `caller`: an `equivalent` row whose reason rests on today's callers (a bound or case no caller reaches).
- `harness`: an `equivalent` row whose reason rests on this test harness (a release-only guard, a
  register or a path no host test observes). The proposal calls this a gap with a better name.
- `gap`: a `gap` row.
- `stale_killed`: a `killed` row for a mutant the census just showed alive.
- `unrowed`: no row, and a shape that is not `or_xor` or a shift.
- `unrowed_noise_candidate`: no row, and shape `or_xor` or shift. Not counted as a real hole, because
  the shape is likely noise, and not counted as noise, because nobody has argued it.

REAL HOLES = gap + stale_killed + unrowed + caller + harness. The upper bound adds
unrowed_noise_candidate. `copied` is a flag, not a class: a row's reason text appears under more than
one crate, it cites a `crates/<other>` path, or it names an ALL_CAPS constant that its own crate's
source never mentions, so the reason was written for another crate and nobody has read it against
this one.

BUGS

- The caller and harness classes come from keywords in the reason (CALLER_WORDS, HARNESS_WORDS). A
  reason that rests on callers without one of those words reads as `algebraic`. Part 5's crate lanes
  read each row, and the `kind` column of part 2 replaces the keywords.
- The shape of a row with no line number is the operator alone, so a `<<` with a nonzero shift reads
  as a noise shape until its row's reason says otherwise.
- Counts are mutants (the snapshot's `count`), not keys, so a key with four mutants weighs four.
"""

import collections
import csv
import gzip
import io
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import mutation_inflow as mi  # noqa: E402

SURVIVORS = pathlib.Path("notes/project-metrics/mutation-survivors")
TRIAGE = pathlib.Path("notes/project-metrics/mutation-triage.csv")
RENAMES = pathlib.Path("notes/project-metrics/mutation-census-renames.csv")
CLASSES_CSV = pathlib.Path("notes/project-metrics/mutation-survivor-classes.csv")
REPORT_MD = pathlib.Path("notes/mutation-testing/survivor-classification.md")

FIELDS = ["crate", "function", "mutation", "count", "shape", "disposition", "class", "copied"]
REAL = ("gap", "stale_killed", "unrowed", "caller", "harness")
CLASSES = ("noise", "noise_unargued", "algebraic", "caller", "harness", "gap", "stale_killed",
           "unrowed", "unrowed_noise_candidate")

# A reason that states the argument for a noise shape.
NOISE_ARGUMENT = re.compile(
    r"disjoint|distinct|share no bit|shares no bit|do not overlap|no bit in common|single bit|"
    r"<< ?0\b|by zero|>> ?0\b|zero shift|0b00 << 6", re.I)
CALLER_WORDS = re.compile(
    r"\bno caller|\bevery caller|\ball callers|\bcallers? (?:never|only|always|pass)|\bnever called|"
    r"\bno call site|\bevery call site|\bonly called|\bthe only caller|\bon every entry", re.I)
HARNESS_WORDS = re.compile(
    r"debug_assert|release-only|release build|\bin release\b|host test|no host test|\bharness\b|"
    r"real tty|real hardware|only on hardware|hardware only|\bwatchdog\b", re.I)
OTHER_CRATE = re.compile(r"crates/([a-z0-9_]+)")
# An ALL_CAPS name in a reason: a constant the reason's author read in some crate's source.
CONSTANT = re.compile(r"\b[A-Z][A-Z0-9]*_[A-Z0-9_]+\b|\b[A-Z]{4,}\b")


def shape(mutation):
    """The mutation's operator family, from its text alone."""
    m = re.match(r"replace (\S+) with (\S+)$", mutation)
    if m:
        a, b = m.groups()
        if {a, b} == {"|", "^"}:
            return "or_xor"
        if {a, b} == {"<<", ">>"}:
            return "shift"
        if a in ("&", "|", "^") or b in ("&", "|", "^"):
            return "bitwise"
        if a in ("<", ">", "<=", ">=", "==", "!=") and b in ("<", ">", "<=", ">=", "==", "!="):
            return "comparison"
        if a in ("&&", "||") and b in ("&&", "||"):
            return "logic"
        if re.fullmatch(r"[-+*/%]=?", a) and re.fullmatch(r"[-+*/%]=?", b):
            return "arithmetic"
    return "other"


def classify_one(shp, row, rows_for_noise=True):
    """(class, reason_text) for one survivor given its triage row or None."""
    noise_shape = shp in ("or_xor", "shift")
    if row is None:
        return ("unrowed_noise_candidate" if noise_shape else "unrowed"), ""
    disposition, reason = row
    if disposition == "gap":
        return "gap", reason
    if disposition == "killed":
        return "stale_killed", reason
    if HARNESS_WORDS.search(reason):
        return "harness", reason
    # A stated disjoint-bits or zero-shift argument is algebra even when it mentions callers.
    if noise_shape and NOISE_ARGUMENT.search(reason):
        return "noise", reason
    if CALLER_WORDS.search(reason):
        return "caller", reason
    if noise_shape:
        return "noise_unargued", reason
    return "algebraic", reason


def normal(reason):
    """A reason with its dates, run numbers and hashes removed, so two copies compare equal."""
    return re.sub(r"\b\d[\d-]*\b|\b[0-9a-f]{8,}\b", "#", reason).strip().lower()


def load(census=None, root="."):
    root = pathlib.Path(root)
    chain = mi.load_renames(root / RENAMES)
    triage = mi.read_triage(root / TRIAGE, chain)
    if census is None:
        path = sorted((root / SURVIVORS).glob("*.csv.gz"))[-1]
    else:
        path = root / SURVIVORS / f"{census}.csv.gz"
    meta, raw = mi.read_snapshot(path)
    merged = collections.Counter()
    for (crate, function, mutation), n in raw.items():
        merged[(mi.resolve(crate, chain), function, mutation)] += n
    return meta, merged, triage


def crate_source(crate, root):
    """All Rust source text of a crate, or None when the tree has no such package."""
    root = pathlib.Path(root)
    name = crate.split("::")[0]
    for d in [root / name, *root.glob(f"*/{name}")]:
        if (d / "Cargo.toml").is_file() and (d / "src").is_dir():
            return "\n".join(p.read_text(errors="replace") for p in sorted(d.rglob("*.rs")))
    return None


def classify(merged, triage, root=None):
    """Rows of FIELDS values for every survivor key, sorted. With `root`, a reason that names a
    constant its own crate's source never mentions is flagged copied, since that is a reason
    written for another crate."""
    sources = {}
    crates_of = collections.defaultdict(set)
    for (crate, _f, _m), (_d, reason) in triage.items():
        crates_of[normal(reason)].add(crate)
    out = []
    for (crate, function, mutation), n in sorted(merged.items()):
        shp = shape(mutation)
        row = triage.get((crate, function, mutation))
        cls, reason = classify_one(shp, row)
        copied = ""
        if row and row[0] == "equivalent":
            cited = {c for c in OTHER_CRATE.findall(reason) if c != crate}
            if len(crates_of[normal(reason)]) > 1 or cited:
                copied = "yes"
            elif root is not None:
                if crate not in sources:
                    sources[crate] = crate_source(crate, root)
                src = sources[crate]
                if src is not None and any(c not in src for c in set(CONSTANT.findall(reason))):
                    copied = "yes"
        out.append([crate, function, mutation, n, shp, row[0] if row else "none", cls, copied])
    return out


def per_crate(rows):
    t = collections.defaultdict(collections.Counter)
    for crate, _f, _m, n, _s, _d, cls, copied in rows:
        t[crate]["missed"] += n
        t[crate][cls] += n
        if copied:
            t[crate]["copied"] += n
    return t


def real(counter):
    return sum(counter[c] for c in REAL)


def render_csv(rows):
    buf = io.StringIO()
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(FIELDS)
    w.writerows(rows)
    return buf.getvalue()


def render_report(meta, rows):
    t = per_crate(rows)
    total = collections.Counter()
    for c in t.values():
        total.update(c)
    head = ["crate", "missed", "noise", "algebraic", "caller", "harness", "gap", "stale killed",
            "unrowed", "unrowed noise?", "real holes", "copied"]

    def line(name, c):
        return "| " + " | ".join(str(x) for x in [
            name, c["missed"], c["noise"] + c["noise_unargued"], c["algebraic"], c["caller"],
            c["harness"], c["gap"], c["stale_killed"], c["unrowed"], c["unrowed_noise_candidate"],
            real(c), c["copied"]]) + " |"

    lines = [
        "# Survivor classification: real holes per crate",
        "",
        f"The census of {meta['census']} (run {meta['run']}) has {total['missed']} missed mutants. "
        f"{real(total)} are real holes and at most {real(total) + total['unrowed_noise_candidate']} "
        "if every unrowed or-versus-xor and shift candidate turns out to be a hole.",
        "",
        "This page is generated by `python3 helpers/mutation_real_holes.py write`, and `script/lint` "
        "fails when it differs from a regeneration. The classes, and what counts as a real hole, are "
        "in that script's docstring. The per-survivor rows are in "
        "`notes/project-metrics/mutation-survivor-classes.csv`.",
        "",
        "| " + " | ".join(head) + " |",
        "|" + "---|" * len(head),
    ]
    for crate in sorted(t, key=lambda k: (-real(t[k]), -t[k]["missed"], k)):
        lines.append(line(crate, t[crate]))
    lines.append(line("**all**", total))
    lines += [
        "",
        "`noise` is the count of survivors whose operator is an or-versus-xor swap or a shift and whose "
        "row argues disjoint bits or a zero shift (including the rows that do not state it). `copied` "
        "counts survivors whose row reason appears under more than one crate or cites another crate's "
        "path, so nobody has read it against this crate. It is a flag on top of the other columns and "
        "does not add to them.",
        "",
    ]
    return "\n".join(lines)


def outputs(census, root="."):
    meta, merged, triage = load(census, root)
    rows = classify(merged, triage, root)
    return render_csv(rows), render_report(meta, rows), meta, rows


def main(argv):
    if argv[:1] == ["--selftest"]:
        return selftest()
    if not argv or argv[0] not in ("write", "check"):
        print(__doc__.split("\n")[0], file=sys.stderr)
        return 2
    census = argv[argv.index("--census") + 1] if "--census" in argv else None
    csv_text, md_text, meta, rows = outputs(census)
    total = collections.Counter()
    for c in per_crate(rows).values():
        total.update(c)
    if argv[0] == "write":
        CLASSES_CSV.write_text(csv_text)
        REPORT_MD.write_text(md_text)
        print(f"mutation_real_holes: census {meta['census']}: {total['missed']} missed, "
              f"{real(total)} real holes")
        return 0
    stale = [str(p) for p, text in ((CLASSES_CSV, csv_text), (REPORT_MD, md_text))
             if not p.exists() or p.read_text() != text]
    for p in stale:
        print(f"mutation_real_holes: {p} differs from a regeneration; run "
              "`python3 helpers/mutation_real_holes.py write`", file=sys.stderr)
    return 1 if stale else 0


def selftest():
    assert shape("replace | with ^") == "or_xor"
    assert shape("replace >> with <<") == "shift"
    assert shape("replace < with <=") == "comparison"
    assert shape("replace Foo::bar -> u64 with 0") == "other"
    eq = lambda r: ("equivalent", r)  # noqa: E731
    assert classify_one("or_xor", eq("the bits are disjoint"))[0] == "noise"
    assert classify_one("or_xor", eq("argued in a note"))[0] == "noise_unargued"
    assert classify_one("comparison", eq("a tie gives the same value"))[0] == "algebraic"
    assert classify_one("comparison", eq("no caller passes zero"))[0] == "caller"
    assert classify_one("comparison", eq("guarded by a debug_assert"))[0] == "harness"
    assert classify_one("shift", None)[0] == "unrowed_noise_candidate"
    assert classify_one("arithmetic", None)[0] == "unrowed"
    assert classify_one("arithmetic", ("gap", "x"))[0] == "gap"
    assert classify_one("arithmetic", ("killed", "x"))[0] == "stale_killed"
    merged = collections.Counter({("a", "f", "replace | with ^"): 2, ("b", "g", "replace < with <="): 1,
                                  ("b", "h", "replace + with -"): 1})
    triage = {("a", "f", "replace | with ^"): ("equivalent", "same reason, disjoint bits"),
              ("b", "g", "replace < with <="): ("equivalent", "same reason, disjoint bits"),
              ("b", "z", "replace < with <="): ("gap", "x")}
    rows = classify(merged, triage)
    assert [r[6] for r in rows] == ["noise", "algebraic", "unrowed"], rows
    assert [r[7] for r in rows] == ["yes", "yes", ""], rows
    t = per_crate(rows)
    assert real(t["b"]) == 1 and real(t["a"]) == 0
    assert normal("run 39238962856 on 2026-10-12") == normal("run 1 on 2026-10-13")
    print("mutation_real_holes: selftest ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
