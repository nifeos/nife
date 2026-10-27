"""The per-pull-request mutation gate: a surviving mutant on a line the pull request changed
cannot merge untriaged.

Milestone 924 (a survivor on a pull request's own lines cannot merge untriaged), provisional
number: calef's ruling "Reopen, build it" (2026-10-15 UTC) on milestone 479 (a blocking
`--in-diff` mutation gate), whose evidence condition milestone 517 (what fraction of survivor
growth arrives on lines a pull request touched) met and whose two objections milestone 438
(would a diff-scoped mutation check have caught the 55, and what would it cost) answers from
its own findings. The rule the block states, and this module enforces:

    python3 helpers/mutation_diff_gate.py gate --run-dir mutants.out --triage CSV \\
        [--renames CSV] [--summary FILE] [--base-ref REV]
    python3 helpers/mutation_diff_gate.py cap --summary FILE
    python3 helpers/mutation_diff_gate.py --selftest      # fixtures, no git, no cargo; script/lint

`gate` reads a finished `cargo mutants --in-diff` run directory and the triage ledger, and fails
when a survivor of the diff has no row that clears it. `cap` is what the workflow runs when the
diff's own mutant run hits the 20-minute cap, and always fails: a cap is a failure, never a pass.
The workflow that wires both to a pull request is .github/workflows/mutation-gate.yml.

A ROW EXPIRES WHEN ITS CODE CHANGES (part 2 of the proposal a survivor accepted once is not accepted
forever). With `--base-ref REV` the gate also reads the triage file as it stood at REV, and a survivor
on a changed line is cleared only by a row this pull request added or changed. The row said something
true of the function as it stood before the pull request rewrote it; the author reads it again, and
the proof that someone did is a diff in the row, normally its `verified` date. An unchanged row fails
as `stale`, and the summary says what clears it. Without `--base-ref` the rule is off, which is how
the workflow runs until the replay in notes/mutation-testing/row-expiry-replay.md has been read.

EXIT CODES. 0: every survivor has an `equivalent` or `gap` row, or the diff produced none. 1:
some survivor has no row, only a `killed` one, or the cap fired. 2: bad input, which fails the
check too, because a required check that fails open is a check that examined nothing and reported
clean (empty-diff's rule). A run directory without `outcomes.json` is exit 2: the tool did not
complete, so this is a broken run and never a pass.

THE KEY is the same `(crate, function, mutation)` the weekly inflow check triages under, imported
from helpers/mutation_inflow.py rather than restated, so the gate and the census cannot disagree
about what a row covers. The rename chain applies to both sides, so a row written under a crate's
old name still matches after the crate is renamed.

A `killed` ROW NEVER CLEARS A SURVIVOR. The gate has just run the mutant and watched it live, so a
`killed` row for it is false here whatever it meant for the census that wrote it: one function's
`cfg!` branch is measured caught on macOS and missed in every Linux census, and the ledger keeps
those rows for that reconciliation. The gate names the row and says so, rather than passing on a
verdict it has direct evidence against.

A TIMEOUT IS UNTRIAGED, THE SAME AS A SURVIVOR. A mutant that hangs is one the gate cannot tell
from a slow test, and a triage row is the only way it clears: the author either kills it with a
test, records the gap, or rules it equivalent.

Name: provisional, minted 2026-10-15 for milestone 924; calef has not ruled on it.
"""

import argparse
import collections
import pathlib
import subprocess
import sys
import tempfile

from mutation_inflow import load_renames, parse_mutant, read_triage, resolve

CAP_MINUTES = 20
CAP_TEXT = (
    "### Mutation gate: the diff's mutants did not finish inside the cap\n\n"
    f"A diff whose mutants do not finish in {CAP_MINUTES} minutes fails, never passes. "
    "Split this pull request or triage by hand.\n"
)


def read_survivors(run_dir):
    """A completed run's survivors as (key, kind) pairs, key resolved through no chain yet.

    `kind` is `missed` or `timeout`, the two outcomes a survivor can have. The evidence that the
    run completed is `outcomes.json`, which cargo-mutants writes last; the per-kind text files are
    absent when their set is empty, so a missing file is an empty set and not an error.
    """
    d = pathlib.Path(run_dir)
    if not (d / "outcomes.json").exists():
        raise FileNotFoundError(f"{run_dir}: no outcomes.json; this is a broken run, not a pass")
    out = []
    for name, kind in (("missed.txt", "missed"), ("timeout.txt", "timeout")):
        p = d / name
        if not p.exists():
            continue
        for raw in p.read_text().splitlines():
            parsed = parse_mutant(raw)
            if parsed:
                out.append((parsed, kind))
    return out


def decide(survivors, triage, chain, base=None):
    """Split survivors into (cleared, untriaged, keys). A row clears a key only at `equivalent` or
    `gap`; `killed` names a verdict the gate's own run just disproved, and no row names one nobody
    made. With a `base` ledger (the triage file at the pull request's base), a live row identical to
    the base's is `stale`: it was written for the function before this pull request changed it."""
    by_key = collections.OrderedDict()
    for (crate, function, mutation, _path, _line), kind in sorted(
        survivors, key=lambda s: (resolve(s[0][0], chain), s[0][1], s[0][2], s[1])
    ):
        key = (resolve(crate, chain), function, mutation)
        by_key.setdefault(key, set()).add(kind)
    cleared, untriaged = 0, []
    for key, kinds in by_key.items():
        row = triage.get(key)
        if row is not None and row[0] in ("equivalent", "gap"):
            if base is not None and base.get(key) == row:
                untriaged.append((key, kinds, "stale"))
            else:
                cleared += 1
        elif row is not None and row[0] == "killed":
            untriaged.append((key, kinds, "killed"))
        else:
            untriaged.append((key, kinds, "none"))
    return cleared, untriaged, len(by_key)


def render(untriaged, keys, cleared):
    """The step summary: what failed, and what clears each kind of failure."""
    kinds = " and ".join(sorted({k for _k, kinds, _w in untriaged for k in kinds}))
    out = [
        "### Mutation gate: {} untriaged survivor key{} on the changed lines ({})".format(
            len(untriaged), "" if len(untriaged) == 1 else "s", kinds or "no run"
        ),
        "",
        f"{keys} survivor key(s) in the diff, {cleared} cleared by the ledger.",
        "",
        "| crate | function | mutation | kinds | what is missing |",
        "|---|---|---|---|---|",
    ]
    for (crate, function, mutation), kinds, why in untriaged:
        missing = {
            "none": "a row in notes/project-metrics/mutation-triage.csv (`equivalent` or `gap`), or a test that kills it",
            "killed": "the gate ran this mutant and saw it live, so its `killed` row is false here: kill it with a test, or write the `equivalent` or `gap` row",
            "stale": "its row is unchanged, and this pull request changed the function it describes: read the row against the new code and update it (`verified` to today's UTC date, with a new `kind` or reason if it changed), or kill the mutant with a test",
        }[why]
        out.append(f"| {crate} | `{function}` | `{mutation}` | {', '.join(sorted(kinds))} | {missing} |")
    return "\n".join(out) + "\n"


def base_triage(rev, triage_path, chain):
    """The triage ledger as it stood at `rev`, read leniently (the base may predate a column), or an
    empty ledger when the base has no such file."""
    r = subprocess.run(["git", "show", f"{rev}:{triage_path}"], capture_output=True, text=True)
    if r.returncode != 0:
        return {}
    with tempfile.NamedTemporaryFile("w", suffix=".csv") as fh:
        fh.write(r.stdout)
        fh.flush()
        return read_triage(fh.name, chain, strict=False)


def run_gate(run_dir, triage_path, renames_path, base_ref=None):
    chain = load_renames(renames_path)
    triage = read_triage(triage_path, chain)
    survivors = read_survivors(run_dir)
    base = base_triage(base_ref, triage_path, chain) if base_ref else None
    cleared, untriaged, keys = decide(survivors, triage, chain, base)
    if not untriaged:
        text = (
            "### Mutation gate: clean\n\n"
            f"Every survivor key the diff produced is cleared by the ledger ({cleared} key(s)), "
            "or the diff produced none.\n"
        )
        return 0, text
    return 1, render(untriaged, keys, cleared)


def main(argv):
    if argv[:1] == ["--selftest"]:
        return selftest()
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    gate_p = sub.add_parser("gate", help="decide a finished run against the triage ledger")
    gate_p.add_argument("--run-dir", default="mutants.out")
    gate_p.add_argument("--triage", default="notes/project-metrics/mutation-triage.csv")
    gate_p.add_argument("--renames", default="notes/project-metrics/mutation-census-renames.csv")
    gate_p.add_argument("--summary")
    gate_p.add_argument("--base-ref", help="a revision whose triage file a cleared row must differ from")
    cap_p = sub.add_parser("cap", help="report the cap failure; always exits 1")
    cap_p.add_argument("--summary")
    args = ap.parse_args(argv)

    try:
        if args.cmd == "cap":
            code, text = 1, CAP_TEXT
        else:
            code, text = run_gate(args.run_dir, args.triage, args.renames, args.base_ref)
    except (ValueError, FileNotFoundError) as e:
        print(f"mutation_diff_gate: {e}", file=sys.stderr)
        return 2
    print(text, end="" if text.endswith("\n") else "\n")
    if args.summary:
        with open(args.summary, "a") as fh:
            fh.write(text)
    return code


def selftest():
    def mk_run(root, name, missed=(), timed_out=(), complete=True):
        d = root / name
        d.mkdir()
        if complete:
            (d / "outcomes.json").write_text("[]")
        if missed:
            (d / "missed.txt").write_text("\n".join(missed) + "\n")
        if timed_out:
            (d / "timeout.txt").write_text("\n".join(timed_out) + "\n")
        return str(d)

    with tempfile.TemporaryDirectory() as tmp:
        root = pathlib.Path(tmp)
        tri = root / "triage.csv"
        tri.write_text(
            "crate,function,mutation,disposition,reason,kind,verified,home\n"
            "fresh,checked,replace == with !=,equivalent,both arms refuse,algebraic,2026-10-01,\n"
            "fresh,loader,replace ! with =,gap,needs a clock,,,\n"
            "fresh,dup,replace * with /,killed,a census on another host saw it die,,,\n"
        )
        renames = root / "renames.csv"
        renames.write_text("old,new,commit,date\nstale,fresh,abc,2026-10-01\n")
        no_renames = str(root / "absent.csv")

        # An equivalent row and a gap row clear their survivors; a killed row and a missing row
        # do not, and the summary says which failure each is.
        run = mk_run(root, "run", missed=[
            "crates/fresh/src/lib.rs:3:5: replace == with != in checked",
            "crates/fresh/src/lib.rs:9:5: replace ! with = in loader",
            "crates/fresh/src/lib.rs:12:5: replace < with <= in strict",
            "crates/fresh/src/lib.rs:30:5: replace + with - in plus",
            "crates/fresh/src/lib.rs:33:5: replace * with / in dup",
        ])
        code, text = run_gate(run, str(tri), no_renames)
        assert code == 1, text
        assert "| fresh | `strict` | `replace < with <=` | missed |" in text, text
        assert "| fresh | `plus` | `replace + with -` | missed |" in text, text
        assert "3 untriaged" in text, text
        assert "`killed` row is false here" in text, text

        full = root / "full.csv"
        full.write_text(
            "crate,function,mutation,disposition,reason,kind,verified,home\n"
            "fresh,checked,replace == with !=,equivalent,both arms refuse,algebraic,2026-10-01,\n"
            "fresh,loader,replace ! with =,gap,needs a clock,,,\n"
            "fresh,dup,replace * with /,equivalent,both orders refuse,algebraic,2026-10-01,\n"
            "fresh,strict,replace < with <=,equivalent,disjoint arms,algebraic,2026-10-01,\n"
            "fresh,plus,replace + with -,equivalent,wraps anyway,algebraic,2026-10-01,\n"
        )
        code, text = run_gate(run, str(full), no_renames)
        assert code == 0 and "clean" in text, text

        # A timeout is untriaged the same as a survivor: the gate cannot tell a hang from a slow
        # test, so only a row clears it.
        hung = mk_run(root, "hung", timed_out=[
            "crates/fresh/src/lib.rs:40:5: replace < with <= in walks",
        ])
        code, text = run_gate(hung, str(tri), no_renames)
        assert code == 1 and "timeout" in text, text
        code, text = run_gate(hung, str(full), no_renames)
        assert code == 1, text
        full.write_text(full.read_text() + "fresh,walks,replace < with <=,gap,guards a real hang,,,\n")
        code, text = run_gate(hung, str(full), no_renames)
        assert code == 0, text

        # The key is crate-renamed on both sides, so a row under the old name still matches.
        stale = mk_run(root, "stale", missed=[
            "crates/stale/src/lib.rs:3:5: replace == with != in checked",
        ])
        code, _text = run_gate(stale, str(tri), str(renames))
        assert code == 0

        # A run that never completed is exit 2, never a pass; so is a malformed ledger row. Both
        # go through main, because the exit code is the contract the workflow consumes.
        assert main(["gate", "--run-dir", mk_run(root, "empty", complete=False),
                     "--triage", str(tri), "--renames", no_renames]) == 2
        assert main(["gate", "--run-dir", mk_run(root, "clean"),
                     "--triage", str(tri), "--renames", no_renames]) == 0
        bad = root / "bad.csv"
        bad.write_text("crate,function,mutation,disposition,reason,kind,verified,home\nx,f,m,fixed,because\n")
        assert main(["gate", "--run-dir", run, "--triage", str(bad), "--renames", no_renames]) == 2

        # An expiring row: with a base ledger, a live row identical to the base's is stale, and a
        # row the pull request added or changed clears. `decide` takes the ledgers directly, so no git.
        chain = {}
        now = read_triage(str(full), chain)
        survivors = read_survivors(run)
        assert decide(survivors, now, chain, None)[1] == []
        _c, stale, _k = decide(survivors, now, chain, dict(now))
        assert {w for _k2, _kinds, w in stale} == {"stale"} and len(stale) == 5, stale
        older = dict(now)
        key = ("fresh", "strict", "replace < with <=")
        older[key] = now[key]._replace(verified="2026-09-01")
        _c, stale, _k = decide(survivors, now, chain, older)
        assert key not in [k for k, _kinds, _w in stale] and len(stale) == 4, stale
        del older[("fresh", "plus", "replace + with -")]
        _c, stale, _k = decide(survivors, now, chain, older)
        assert len(stale) == 3, stale
        assert "its row is unchanged" in render(stale, 5, 2)

        # The cap always fails, with the message that names the two ways out.
        assert main(["cap"]) == 1
        assert "Split this pull request or triage by hand" in CAP_TEXT

    print("mutation_diff_gate selftest: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
