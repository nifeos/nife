"""The inflow check: a survivor a merged pull request adds on its own lines must have a triage row.

Milestone 740 (the survivors a merged pull request adds are checked against a triage record), provisional number minted by the mutation-inflow-check lane on 2026-10-04 UTC: calef's
ruling "Yes, build the inflow check" (2026-10-04 UTC) on fatal risk 3, spec in
notes/mutation-testing/inflow-2026-10-03.md. Before this, nothing could say whether the survivors each
merged pull request adds were triaged; `script/mutation --report` prints per-crate totals and the
triage ledgers are prose.

    python3 helpers/mutation_inflow.py check --census DATE --run ID --sha SHA \\
        --survivors-dir notes/project-metrics/mutation-survivors \\
        --triage notes/project-metrics/mutation-triage.csv \\
        --snapshot-out OUT.csv.gz [--summary FILE] SHARD_DIR...
    python3 helpers/mutation_inflow.py snapshot --census DATE --run ID --sha SHA --out OUT SHARD_DIR...
    python3 helpers/mutation_inflow.py --selftest      # fixtures, no git, no network; script/lint
    python3 helpers/mutation_inflow.py orphans         # every triage row names a package; script/lint

`check` reads one census's `missed.txt` shard files, diffs them against the latest committed survivor
list that is older than this census, blames each new survivor's line to the commit and merged pull
request that wrote it, and looks the survivor up in the triage CSV. Exit 0: every inflow survivor has a
row. Exit 1: some do not, and they are listed with crate, function, mutation and pull request. Exit 2:
bad input. It never blocks a pull request: it runs in the weekly workflow only. The check that
blocks a pull request is the diff gate, helpers/mutation_diff_gate.py (milestone 924 (a survivor
on a pull request's own lines cannot merge untriaged)).

THE KEY. A survivor is `(crate, function, mutation)`, never a line number, because lines move and a
triage row written on Tuesday must still match on Monday. cargo-mutants names a mutant
`path:line:col: replace < with <= in Type::fn`. The crate is the path component before `/src/`. The
function is what follows the last ` in ` when that tail holds no ` with `; a function-body replacement
(`replace Foo::bar -> u64 with 0`) has no ` in `, so its function is the name after `replace` and its
mutation is the whole text; a constant's operator (`replace | with ^`, no enclosing function) has
function empty. A crate in `BY_MODULE` keys by its top-level module instead
(`driver_process_contract::e1000e`), because it holds one module per driver and two drivers'
`Handoff::pack` are two functions, not one. Two mutants with one key (the same operator swapped on two lines of one function) share
a row, and a row covers all of them. The crate is resolved through
notes/project-metrics/mutation-census-renames.csv on both sides of every comparison, so a renamed crate
is not a vanished one with a new one beside it.

WHAT COUNTS AS INFLOW. A key is new when this census holds more mutants under it than the previous
census did. A new key is **inflow** when at least one of its current lines was last written by a commit
that is not an ancestor of the previous census's commit; that commit's merged pull request is blamed.
A new key whose every line predates the previous census is **unattributed**: no pull request wrote it
in the window, so it is the work of something else (a mutant the tool did not generate before, a test
that was deleted, a shard lost from the previous census). Unattributed keys are reported and counted,
never failed, because failing a job on a cause nobody can act on teaches people to ignore it.

BUGS

- A key with several lines is judged by its newest line, so a pull request that adds a third copy of an
  operator pattern to a function already holding two triaged ones is covered by the existing row. The
  row's reason is the place to say so; the check cannot.
- The blame is at the census's own commit (`--sha`), by `git blame`, so a line a later refactor moved
  between files is attributed to the move, as it is everywhere in this tree.
- Pull request lookup is `gh api commits/SHA/pulls`. A commit with no pull request (a direct push) is
  named by its abbreviated hash.
- A census with fewer than eight shards is not diffed: a lost shard reads as a crate that got better
  and then worse. The workflow decides that, not this module.
"""

import argparse
import collections
import csv
import gzip
import io
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile
import typing

DISPOSITIONS = ("killed", "equivalent", "gap")
# Part 2 of the survivor proposal (a survivor accepted once is not accepted forever) added the last
# three columns. `kind` says what an `equivalent` row claims: `algebraic` holds in every caller,
# `caller` holds only for today's callers and the reason names them, `unclassified` is a legacy row
# nobody has read yet. `verified` is the UTC date someone last read the row against its code. `home`
# is a `gap` row's tracked home: `milestone N`, `proposal SLUG` or `BUGS PATH`. Names provisional.
TRIAGE_FIELDS = ["crate", "function", "mutation", "disposition", "reason", "kind", "verified", "home"]
OLD_TRIAGE_FIELDS = TRIAGE_FIELDS[:5]
KINDS = ("algebraic", "caller", "unclassified")
HOME = re.compile(r"^(?:milestone \d+|proposal [a-z0-9][a-z0-9-]*|BUGS \S+)$")
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


class Row(typing.NamedTuple):
    """A triage row. Indexing [0] and [1] still give disposition and reason, as before the columns."""

    disposition: str
    reason: str
    kind: str = ""
    verified: str = ""
    home: str = ""


def row_problem(row):
    """Why a row is malformed, or None. The shape rules of the three part-2 columns."""
    if row.kind not in ("",) + KINDS:
        return f"kind {row.kind!r} is not one of {KINDS}"
    if row.disposition == "equivalent" and not row.kind:
        return "an `equivalent` row needs a kind"
    if row.disposition != "equivalent" and row.kind:
        return f"a `{row.disposition}` row has no kind; only `equivalent` rows do"
    if row.verified and not DATE.match(row.verified):
        return f"verified {row.verified!r} is not a YYYY-MM-DD date"
    if row.kind in ("algebraic", "caller") and not row.verified:
        return f"a `{row.kind}` row needs the date it was verified"
    if row.kind == "unclassified" and row.verified:
        return "an `unclassified` row has not been read, so it has no verified date; give it a kind"
    if row.home and row.disposition != "gap":
        return "only a `gap` row has a home"
    if row.home and not HOME.match(row.home):
        return f"home {row.home!r} is not `milestone N`, `proposal SLUG` or `BUGS PATH`"
    return None
SNAPSHOT_FIELDS = ["crate", "function", "mutation", "count"]
# Crates keyed `<crate>::<module>`, by the first path component under `src/`. One module per driver,
# each with its own `Handoff`, so the crate alone would merge two drivers' survivors under one key.
BY_MODULE = frozenset({"driver_process_contract"})
LINE = re.compile(r"^(?P<path>[^:]+):(?P<line>\d+):(?P<col>\d+): (?P<text>.+)$")


def parse_mutant(raw):
    """One missed.txt line to (crate, function, mutation, path, line), or None for a blank line."""
    raw = raw.strip()
    if not raw:
        return None
    m = LINE.match(raw)
    if not m:
        raise ValueError(f"not a cargo-mutants line: {raw!r}")
    path, text = m["path"], m["text"]
    crate = path.split("/src/")[0].split("/")[-1]
    if crate in BY_MODULE and "/src/" in path:
        module = path.split("/src/", 1)[1].split("/")[0].removesuffix(".rs")
        if module not in ("lib", "main"):
            crate = f"{crate}::{module}"
    head, sep, tail = text.rpartition(" in ")
    if sep and " with " not in tail and head.startswith(("replace ", "delete ")):
        function, mutation = tail, head
    else:
        mutation = text
        fm = re.match(r"replace (\S+?)(?: -> .*)? with ", text)
        function = fm.group(1) if fm and _is_name(fm.group(1)) else ""
    return crate, function, mutation, path, int(m["line"])


def _is_name(s):
    """A function-body replacement names a path (`Foo::bar`, `bar`); an operator swap names a symbol."""
    return re.fullmatch(r"[A-Za-z_][\w:<>']*", s) is not None


def read_shards(dirs):
    """Every missed mutant of every shard directory, as parsed tuples."""
    out = []
    found = False
    for d in dirs:
        p = pathlib.Path(d) / "missed.txt"
        if not p.exists():
            continue
        found = True
        for raw in p.read_text().splitlines():
            parsed = parse_mutant(raw)
            if parsed:
                out.append(parsed)
    if not found:
        raise FileNotFoundError(f"no missed.txt under {' '.join(map(str, dirs))}")
    return out


def load_renames(path):
    chain = {}
    p = pathlib.Path(path)
    if p.exists():
        for row in csv.DictReader(p.open()):
            chain[row["old"]] = row["new"]
    return chain


def resolve(name, chain):
    seen = set()
    while name in chain and name not in seen:
        seen.add(name)
        name = chain[name]
    return name


def orphans(triage_path, chain, root):
    """Triage-row crate names that, through the rename table, name no package in the tree.

    A row is keyed by crate name, so a crate renamed without a row in the rename table strands every
    row it had: the next census reports its survivors under the new name, none of them matches, and
    because their lines predate the previous census `check` files them as unattributed rather than
    failing. The 2026-10-18 `_contract` rename stranded 75 survivors that way. A package is a
    directory holding `Cargo.toml` and `src/`, at the root or one level down (cargo-mutants takes
    the crate name from the path component before `/src/`). A deleted crate's rows go with it.
    """
    root = pathlib.Path(root)
    seen = set()
    for row in csv.DictReader(pathlib.Path(triage_path).open()):
        seen.add(resolve(row["crate"], chain).split("::")[0])
    def present(name):
        return any((d / "Cargo.toml").is_file() and (d / "src").is_dir()
                   for d in [root / name, *root.glob(f"*/{name}")])
    return sorted(n for n in seen if not present(n))


def counts(mutants, chain):
    c = collections.Counter()
    for crate, function, mutation, _p, _l in mutants:
        c[(resolve(crate, chain), function, mutation)] += 1
    return c


def write_snapshot(path, mutants, meta):
    """A committed survivor list: key and count only, sorted, gzip with mtime 0 so a rerun is
    byte-identical and a PR diff means the survivors moved. `meta` is the census's date, run and sha."""
    c = counts(mutants, {})
    buf = io.StringIO()
    buf.write(f"# census={meta['census']} run={meta['run']} sha={meta['sha']}\n")
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(SNAPSHOT_FIELDS)
    for (crate, function, mutation), n in sorted(c.items()):
        w.writerow([crate, function, mutation, n])
    pathlib.Path(path).parent.mkdir(parents=True, exist_ok=True)
    with open(path, "wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=9) as gz:
            gz.write(buf.getvalue().encode())
    return len(buf.getvalue())


def read_snapshot(path):
    """(meta, Counter) from a committed survivor list."""
    with gzip.open(path, "rt") as fh:
        first = fh.readline()
        meta = dict(kv.split("=", 1) for kv in first.lstrip("# ").split())
        c = collections.Counter()
        for row in csv.DictReader(fh):
            c[(row["crate"], row["function"], row["mutation"])] = int(row["count"])
    return meta, c


def latest_before(survivors_dir, census):
    """The newest committed survivor list strictly older than `census`, or None."""
    d = pathlib.Path(survivors_dir)
    if not d.is_dir():
        return None
    older = sorted(p for p in d.glob("*.csv.gz") if p.name.split(".")[0] < census)
    return older[-1] if older else None


def read_triage(path, chain, strict=True):
    """key -> Row. A malformed file is an error, because a row that silently fails to match turns
    a triaged survivor into a red job nobody can explain. `strict=False` also reads the older
    five-column file and skips the part-2 shape rules: the gate reads the base commit's copy that
    way, since the base predates whatever the pull request changed."""
    out = {}
    p = pathlib.Path(path)
    if not p.exists():
        return out
    rows = list(csv.DictReader(p.open()))
    if rows:
        header = list(rows[0].keys())
        if header != TRIAGE_FIELDS and not (not strict and header == OLD_TRIAGE_FIELDS):
            raise ValueError(f"{path}: header must be {','.join(TRIAGE_FIELDS)}")
    for i, row in enumerate(rows, start=2):
        if row["disposition"] not in DISPOSITIONS:
            raise ValueError(f"{path}:{i}: disposition {row['disposition']!r} is not one of {DISPOSITIONS}")
        if not row["reason"].strip():
            raise ValueError(f"{path}:{i}: a row with no reason is a verdict nobody can check")
        key = (resolve(row["crate"], chain), row["function"], row["mutation"])
        if key in out:
            raise ValueError(f"{path}:{i}: duplicate key {key}")
        out[key] = Row(row["disposition"], row["reason"], row.get("kind") or "",
                       row.get("verified") or "", row.get("home") or "")
        if strict:
            bad = row_problem(out[key])
            if bad:
                raise ValueError(f"{path}:{i}: {bad}")
    return out


class GitBlame:
    """Blame at one revision, cached per file, and pull request lookup cached per commit."""

    def __init__(self, rev, old_sha, repo="."):
        self.rev, self.old_sha, self.repo = rev, old_sha, repo
        self._files, self._ancestor, self._pr = {}, {}, {}

    def _git(self, *args):
        return subprocess.run(["git", "-C", self.repo, *args], capture_output=True, text=True)

    def commit_of(self, path, line):
        if path not in self._files:
            r = self._git("blame", "--porcelain", self.rev, "--", path)
            lines = {}
            if r.returncode == 0:
                for ln in r.stdout.splitlines():
                    m = re.match(r"^([0-9a-f]{40}) \d+ (\d+)", ln)
                    if m:
                        lines[int(m.group(2))] = m.group(1)
            self._files[path] = lines
        return self._files[path].get(line)

    def predates(self, sha):
        """True when `sha` is an ancestor of the previous census's commit."""
        if sha not in self._ancestor:
            self._ancestor[sha] = self._git("merge-base", "--is-ancestor", sha, self.old_sha).returncode == 0
        return self._ancestor[sha]

    def pr_of(self, sha):
        if sha not in self._pr:
            name = sha[:8]
            if os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN"):
                r = subprocess.run(
                    ["gh", "api", f"repos/{{owner}}/{{repo}}/commits/{sha}/pulls", "--jq", ".[0].number"],
                    capture_output=True, text=True, cwd=self.repo,
                )
                if r.returncode == 0 and r.stdout.strip().isdigit():
                    name = "#" + r.stdout.strip()
            self._pr[sha] = name
        return self._pr[sha]


def classify(new_mutants, old_counts, triage, chain, blamer):
    """Split the new survivors into inflow (triaged or not) and unattributed."""
    new_counts = counts(new_mutants, chain)
    lines = collections.defaultdict(list)
    for crate, function, mutation, path, line in new_mutants:
        lines[(resolve(crate, chain), function, mutation)].append((path, line))
    inflow, unattributed = [], []
    for key in sorted(new_counts):
        if new_counts[key] <= old_counts.get(key, 0):
            continue
        prs = set()
        for path, line in lines[key]:
            sha = blamer.commit_of(path, line)
            if sha and not blamer.predates(sha):
                prs.add(blamer.pr_of(sha))
        row = {"key": key, "prs": sorted(prs), "triage": triage.get(key)}
        (inflow if prs else unattributed).append(row)
    return new_counts, inflow, unattributed


def render(meta, old_meta, new_counts, inflow, unattributed):
    untriaged = [r for r in inflow if r["triage"] is None]
    out = [f"### Mutation inflow: census {meta['census']}", ""]
    if old_meta is None:
        out.append("No earlier survivor list is committed, so there is nothing to diff against.")
    else:
        out.append(
            f"Against the census of {old_meta['census']} (run {old_meta['run']}): "
            f"{sum(new_counts.values())} survivors now."
        )
        out.append("")
        out.append(
            f"- new on lines a merged pull request wrote: {len(inflow)} "
            f"({len(inflow) - len(untriaged)} triaged, **{len(untriaged)} untriaged**)"
        )
        out.append(f"- new on lines older than the previous census (unattributed, not failed): {len(unattributed)}")
    if untriaged:
        out += ["", "Untriaged inflow, each needs a row in notes/project-metrics/mutation-triage.csv:", "",
                "| crate | function | mutation | pull request |", "|---|---|---|---|"]
        for r in untriaged:
            crate, function, mutation = r["key"]
            out.append(f"| {crate} | `{function}` | `{mutation}` | {', '.join(r['prs'])} |")
    return "\n".join(out) + "\n", len(untriaged)


def run_check(args, blamer=None):
    meta = {"census": args.census, "run": args.run, "sha": args.sha}
    chain = load_renames(args.renames)
    mutants = read_shards(args.shards)
    triage = read_triage(args.triage, chain)
    if args.snapshot_out:
        write_snapshot(args.snapshot_out, mutants, meta)
    prev = latest_before(args.survivors_dir, args.census)
    if prev is None:
        text, _ = render(meta, None, counts(mutants, chain), [], [])
        return 0, text
    old_meta, old_raw = read_snapshot(prev)
    old_counts = collections.Counter()
    for (crate, function, mutation), n in old_raw.items():
        old_counts[(resolve(crate, chain), function, mutation)] += n
    blamer = blamer or GitBlame(args.sha, old_meta["sha"], args.repo)
    new_counts, inflow, unattributed = classify(mutants, old_counts, triage, chain, blamer)
    text, bad = render(meta, old_meta, new_counts, inflow, unattributed)
    return (1 if bad else 0), text


def selftest():
    class Fake:
        """Commit `old` predates the previous census; `new` does not. PR 12 wrote `new`."""

        def __init__(self, table):
            self.table = table

        def commit_of(self, path, line):
            return self.table.get((path, line))

        def predates(self, sha):
            return sha == "old"

        def pr_of(self, sha):
            return "#12"

    def mk(root, name, lines):
        d = pathlib.Path(root) / name
        d.mkdir()
        (d / "missed.txt").write_text("\n".join(lines) + "\n")
        return str(d)

    # A `BY_MODULE` crate keys each driver module apart, so two `Handoff::pack`s are two keys.
    a = parse_mutant("crates/driver_process_contract/src/e1000e.rs:9:5: replace | with ^ in Handoff::pack")
    b = parse_mutant("crates/driver_process_contract/src/extensible_host_controller_interface/"
                     "proofs.rs:3:5: replace | with ^ in Handoff::pack")
    assert a[0] == "driver_process_contract::e1000e", a
    assert b[0] == "driver_process_contract::extensible_host_controller_interface", b
    assert parse_mutant("crates/driver_process_contract/src/lib.rs:1:1: replace f -> u8 with 0")[0] \
        == "driver_process_contract"
    assert parse_mutant("crates/e1000e/src/descriptor.rs:9:5: replace | with ^ in f")[0] == "e1000e"

    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)
        ren = tmp / "renames.csv"
        ren.write_text("old,new,commit,date\nlinedisc,lineedit,abc,2026-07-30\n")
        # The previous census: one survivor that stays, one in a crate that will be renamed, and a
        # function holding one copy of an operator swap.
        old = mk(tmp, "old", [
            "crates/stays/src/lib.rs:10:5: replace < with <= in keep",
            "crates/linedisc/src/lib.rs:7:5: replace + with - in shift",
            "crates/dup/src/lib.rs:5:5: replace > with >= in twice",
        ])
        sd = tmp / "survivors"
        write_snapshot(sd / "2026-09-21.csv.gz", read_shards([old]),
                       {"census": "2026-09-21", "run": "1", "sha": "oldsha"})
        # This census: the three above (the renamed crate under its new name, the duplicate now
        # twice), a triaged new survivor, an untriaged one, a new one on a line that predates the
        # previous census, and a constant's operator swap with no enclosing function.
        new = mk(tmp, "new", [
            "crates/stays/src/lib.rs:11:5: replace < with <= in keep",
            "crates/lineedit/src/lib.rs:7:5: replace + with - in shift",
            "crates/dup/src/lib.rs:5:5: replace > with >= in twice",
            "crates/dup/src/lib.rs:50:5: replace > with >= in twice",
            "crates/fresh/src/lib.rs:3:5: replace == with != in checked",
            "crates/fresh/src/lib.rs:9:5: replace Fresh::get -> u8 with 0",
            "crates/ancient/src/lib.rs:4:5: replace - with / in ancient_fn",
            "crates/fresh/src/lib.rs:20:5: replace | with ^",
        ])
        tri = tmp / "triage.csv"
        tri.write_text(
            "crate,function,mutation,disposition,reason,kind,verified,home\n"
            "fresh,checked,replace == with !=,killed,a_test_names_it,,,\n"
            "fresh,Fresh::get,replace Fresh::get -> u8 with 0,gap,needs a clock,,,\n"
        )
        table = {
            ("crates/stays/src/lib.rs", 11): "old",
            ("crates/lineedit/src/lib.rs", 7): "old",
            ("crates/dup/src/lib.rs", 5): "old",
            ("crates/dup/src/lib.rs", 50): "new",
            ("crates/fresh/src/lib.rs", 3): "new",
            ("crates/fresh/src/lib.rs", 9): "new",
            ("crates/fresh/src/lib.rs", 20): "new",
            ("crates/ancient/src/lib.rs", 4): "old",
        }

        def go(triage_path):
            ns = argparse.Namespace(census="2026-10-03", run="2", sha="newsha", survivors_dir=str(sd),
                                    triage=str(triage_path), renames=str(ren), snapshot_out=str(tmp / "o.csv.gz"),
                                    shards=[new], repo=".")
            return run_check(ns, Fake(table))

        code, text = go(tri)
        assert code == 1, text
        # untriaged: the duplicate's second copy and the constant's swap; triaged: two in `fresh`
        assert "| dup | `twice` | `replace > with >=` | #12 |" in text, text
        assert "| fresh | `` | `replace | with ^` | #12 |" in text, text
        assert "2 untriaged" in text, text
        # a renamed crate is not new, an old line is not inflow, a triaged row is not listed
        assert "shift" not in text and "keep" not in text and "checked" not in text, text
        assert "unattributed, not failed): 1" in text, text
        assert "ancient_fn" not in text, text
        # with every inflow key triaged the check passes, and the snapshot round-trips
        full = tmp / "full.csv"
        full.write_text(tri.read_text()
                        + "dup,twice,replace > with >=,equivalent,both arms refuse,algebraic,2026-10-01,\n"
                        + "fresh,,replace | with ^,equivalent,disjoint bits,algebraic,2026-10-01,\n")
        code, text = go(full)
        assert code == 0 and "0 untriaged" in text.replace("**", ""), text
        meta, c = read_snapshot(tmp / "o.csv.gz")
        assert meta["sha"] == "newsha" and sum(c.values()) == 8, (meta, c)
        # no earlier list: nothing to diff, never red
        ns = argparse.Namespace(census="2026-09-01", run="0", sha="s", survivors_dir=str(sd),
                                triage=str(tri), renames=str(ren), snapshot_out=None, shards=[new], repo=".")
        code, text = run_check(ns, Fake(table))
        assert code == 0 and "nothing to diff" in text, text
        # a bad disposition is an input error, not a silent non-match
        bad = tmp / "bad.csv"
        bad.write_text("crate,function,mutation,disposition,reason,kind,verified,home\nx,f,m,fixed,because\n")
        try:
            read_triage(bad, {})
        except ValueError:
            pass
        else:
            raise AssertionError("a bad disposition was accepted")
        # a crate renamed without a rename row strands its triage rows; the row un-strands them
        for name in ("lib_now", "kept"):
            (tmp / "crates" / name / "src").mkdir(parents=True)
            (tmp / "crates" / name / "Cargo.toml").write_text("")
        rows = tmp / "rows.csv"
        rows.write_text("crate,function,mutation,disposition,reason,kind,verified,home\n"
                        "lib_was,f,m,equivalent,r\nkept,f,m,equivalent,r,algebraic,2026-10-01,\n")
        assert orphans(rows, {}, tmp) == ["lib_was"]
        assert orphans(rows, {"lib_was": "lib_now"}, tmp) == []
    # the key's three shapes
    assert parse_mutant("crates/a/src/lib.rs:1:2: replace < with <= in T<'a>::f")[:3] == ("a", "T<'a>::f", "replace < with <=")
    assert parse_mutant("crates/a/src/lib.rs:1:2: replace X::g -> u64 with 0")[:3] == ("a", "X::g", "replace X::g -> u64 with 0")
    assert parse_mutant("crates/a/src/lib.rs:1:2: replace | with ^")[:3] == ("a", "", "replace | with ^")
    assert parse_mutant("crates/a/src/lib.rs:1:2: replace verify with ()")[:3] == ("a", "verify", "replace verify with ()")
    print("mutation_inflow selftest: ok")
    return 0


def main(argv):
    if argv[:1] == ["--selftest"]:
        return selftest()
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    o = sub.add_parser("orphans", help="fail when a triage row's crate names no package")
    o.add_argument("--triage", default="notes/project-metrics/mutation-triage.csv")
    o.add_argument("--renames", default="notes/project-metrics/mutation-census-renames.csv")
    o.add_argument("--root", default=".")
    for name in ("check", "snapshot"):
        p = sub.add_parser(name)
        p.add_argument("--census", required=True)
        p.add_argument("--run", required=True)
        p.add_argument("--sha", required=True)
        p.add_argument("shards", nargs="+")
        if name == "check":
            p.add_argument("--survivors-dir", default="notes/project-metrics/mutation-survivors")
            p.add_argument("--triage", default="notes/project-metrics/mutation-triage.csv")
            p.add_argument("--renames", default="notes/project-metrics/mutation-census-renames.csv")
            p.add_argument("--snapshot-out")
            p.add_argument("--summary")
            p.add_argument("--repo", default=".")
        else:
            p.add_argument("--out", required=True)
    args = ap.parse_args(argv)
    try:
        if args.cmd == "orphans":
            lost = orphans(args.triage, load_renames(args.renames), args.root)
            for name in lost:
                print(f"mutation_inflow: triage rows name `{name}`, which is no package in the tree; "
                      "if it was renamed, run script/mutation-census --renames and commit the table, "
                      "and if it was deleted, remove its rows", file=sys.stderr)
            return 1 if lost else 0
        if args.cmd == "snapshot":
            n = write_snapshot(args.out, read_shards(args.shards),
                               {"census": args.census, "run": args.run, "sha": args.sha})
            print(f"wrote {args.out} ({os.path.getsize(args.out)} bytes gzipped, {n} raw)")
            return 0
        code, text = run_check(args)
    except (ValueError, FileNotFoundError) as e:
        print(f"mutation_inflow: {e}", file=sys.stderr)
        return 2
    print(text)
    if args.summary:
        with open(args.summary, "a") as fh:
            fh.write(text)
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
