"""Replay the row-expiry rule over merged pull requests: how many would it have stopped, and why.

Part 2 of the proposal design/roadmap/proposals/a-survivor-accepted-once-is-not-accepted-forever.md.
The diff gate's `--base-ref` rule (helpers/mutation_diff_gate.py) fails a survivor on a changed line
whose triage row the pull request did not change. Before the workflow passes that flag, this script
measures what it would have cost.

    python3 helpers/mutation_gate_replay.py [--last 100] [--census DATE] [--rev REV] [--table]
    python3 helpers/mutation_gate_replay.py --selftest

WHAT IT DOES NOT DO: run cargo-mutants. A real replay would mutate 100 diffs, hours of runner time.
This is an estimate from three things the tree already has, and it says so in its output:

1. the pull request's diff against its first parent (what the merge queue's gate sees), reduced to
   the Rust functions that contain a changed or deleted line (cargo-mutants `--in-diff` mutates the
   functions a diff touches), minus code after a `#[cfg(test)] mod` and files under `tests/`;
2. the newest committed census's survivor keys, matched to those functions by crate and the last path
   segment of the function name. A survivor already dead by now is missed, and a key with no function
   (a constant's operator swap) cannot be placed, so both bias the count DOWN;
3. the triage file at the pull request's first parent and at its merge, read leniently: a survivor
   with a live row at the merge whose row is identical at the parent is one the new rule stops. A
   survivor with no live row is already stopped by the existing rule and is counted apart.

The function finder is a heuristic over rustfmt-shaped source (a function ends at the first `}` at its
own indent). It places a changed line in the wrong function when the code is not rustfmt-shaped, and
it matches by name, so two functions named `new` in one crate both count. It biases UP.
"""

import argparse
import collections
import pathlib
import re
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import mutation_inflow as mi  # noqa: E402
import mutation_real_holes as rh  # noqa: E402

TRIAGE = "notes/project-metrics/mutation-triage.csv"
FN = re.compile(r"^(\s*)(?:pub(?:\([^)]*\))?\s+)?(?:(?:const|unsafe|async|default)\s+)*(?:extern\s+\"[^\"]*\"\s+)?fn\s+(\w+)")
HUNK = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")


def git(*args, check=True):
    r = subprocess.run(["git", *args], capture_output=True, text=True)
    if check and r.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {r.stderr.strip()}")
    return r.stdout


def functions(text):
    """[(name, first_line, last_line)] for the non-test functions of rustfmt-shaped source, 1-based."""
    lines = text.splitlines()
    test_from = len(lines) + 1
    for i, ln in enumerate(lines):
        if ln.strip() == "#[cfg(test)]" and any(re.match(r"\s*(?:pub\s+)?mod\s", x) for x in lines[i + 1:i + 3]):
            test_from = i + 1
            break
    out = []
    for i, ln in enumerate(lines):
        m = FN.match(ln)
        if not m or i + 1 >= test_from:
            continue
        indent, name = m.groups()
        if i > 0 and lines[i - 1].strip() == "#[test]":
            continue
        end = None
        for j in range(i, min(len(lines), i + 4000)):
            if j > i and lines[j].startswith(indent + "}") and not lines[j][len(indent):].startswith("} "):
                end = j + 1
                break
            if j == i and ln.rstrip().endswith(";"):
                break
        if end:
            out.append((name, i + 1, end))
    return out


def changed_lines(diff_text):
    """{path: [line, ...]} of post-image lines a -U0 diff adds, or sits at for a pure deletion."""
    out = collections.defaultdict(list)
    path = None
    for ln in diff_text.splitlines():
        if ln.startswith("+++ b/"):
            path = ln[6:]
        elif ln.startswith("+++ "):
            path = None
        elif path and (m := HUNK.match(ln)):
            start, count = int(m.group(1)), int(m.group(2) or 1)
            out[path].extend(range(start, start + max(count, 1)))
    return out


def crate_of(path):
    return path.split("/src/")[0].split("/")[-1] if "/src/" in path else None


def touched(diff_text, rev):
    """{(crate, fn name)} the diff touches in the tree at `rev`."""
    out = set()
    for path, lines in changed_lines(diff_text).items():
        crate = crate_of(path)
        if not path.endswith(".rs") or crate is None or "/tests/" in path:
            continue
        src = git("show", f"{rev}:{path}", check=False)
        if not src:
            continue
        spans = functions(src)
        for ln in lines:
            inner = [s for s in spans if s[1] <= ln <= s[2]]
            if inner:
                out.add((crate, min(inner, key=lambda s: s[2] - s[1])[0]))
    return out


def fn_name(function):
    """The last path segment of a survivor key's function, or None for a constant's swap."""
    return re.split(r"::", function)[-1] if function else None


def merged_prs(last, rev):
    """[(number, merge sha)] newest first, from the first-parent merge commits of `rev`."""
    out = []
    for ln in git("log", "--first-parent", "--merges", "-n", str(last), "--format=%H %s", rev).splitlines():
        sha, _, subject = ln.partition(" ")
        m = re.match(r"Merge pull request #(\d+)", subject)
        if m:
            out.append((int(m.group(1)), sha))
    return out


def replay(last, census, rev):
    chain = mi.load_renames(pathlib.Path(mi.__file__).resolve().parents[1] / "notes/project-metrics/mutation-census-renames.csv")
    _meta, merged, _t = rh.load(census)
    by_fn = collections.defaultdict(list)
    for (crate, function, mutation), n in merged.items():
        name = fn_name(function)
        if name:
            by_fn[(crate, name)].append((crate, function, mutation))
    results = []
    for number, sha in merged_prs(last, rev):
        diff = git("diff", "-U0", f"{sha}^1", sha, "--", "*.rs")
        fns = touched(diff, sha)
        keys = [k for f in fns for k in by_fn.get((mi.resolve(f[0], chain), f[1]), [])]
        row = {"pr": number, "functions": len(fns), "keys": len(set(keys)), "stopped": [], "already": 0}
        if keys:
            after = read_ledger(sha, chain)
            before = read_ledger(f"{sha}^1", chain)
            for key in sorted(set(keys)):
                r = after.get(key)
                if r is None or r.disposition == "killed":
                    row["already"] += 1
                elif before.get(key) == r:
                    row["stopped"].append((key, r))
        results.append(row)
    return results


_ledgers = {}


def read_ledger(rev, chain):
    sha = git("rev-parse", rev).strip()
    if sha not in _ledgers:
        text = git("show", f"{sha}:{TRIAGE}", check=False)
        import tempfile
        with tempfile.NamedTemporaryFile("w", suffix=".csv") as fh:
            fh.write(text)
            fh.flush()
            _ledgers[sha] = mi.read_triage(fh.name, chain, strict=False) if text else {}
    return _ledgers[sha]


def summarize(results, table=False):
    total = len(results)
    with_fns = sum(1 for r in results if r["functions"])
    with_keys = sum(1 for r in results if r["keys"])
    stopped = [r for r in results if r["stopped"]]
    rows = [k for r in stopped for k in r["stopped"]]
    shape = collections.Counter(rh.shape(k[0][2]) for k in rows)
    disp = collections.Counter(k[1].disposition for k in rows)
    kind = collections.Counter(k[1].kind or "none" for k in rows)
    per = sorted(len(r["stopped"]) for r in stopped)
    lines = [
        f"merged pull requests replayed: {total}",
        f"  touching a Rust function in a crate: {with_fns}",
        f"  touching a function that holds a census survivor: {with_keys}",
        f"  already stopped by the existing rule (a survivor key with no live row): "
        f"{sum(1 for r in results if r['already'])}",
        f"  stopped by the row-expiry rule: {len(stopped)}",
        f"rows those pull requests would have had to re-read: {len(rows)} "
        f"(median {per[len(per) // 2] if per else 0} per stopped pull request, max {per[-1] if per else 0})",
        f"  by disposition: {dict(disp)}",
        f"  by kind: {dict(kind)}",
        f"  by mutation shape: {dict(shape)}",
        f"  stopped pull requests whose every row is an or/xor or shift shape: "
        f"{sum(1 for r in stopped if all(rh.shape(k[0][2]) in ('or_xor', 'shift') for k in r['stopped']))}",
    ]
    if table:
        lines.append("")
        for r in stopped:
            crates = sorted({k[0][0] for k in r["stopped"]})
            lines.append(f"#{r['pr']}: {len(r['stopped'])} row(s) in {', '.join(crates)}")
    return "\n".join(lines)


def main(argv):
    if argv[:1] == ["--selftest"]:
        return selftest()
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--last", type=int, default=100)
    ap.add_argument("--census")
    ap.add_argument("--rev", default="origin/main")
    ap.add_argument("--table", action="store_true")
    args = ap.parse_args(argv)
    print(summarize(replay(args.last, args.census, args.rev), args.table))
    return 0


def selftest():
    src = ("pub struct S;\nimpl S {\n    pub fn a(&self) -> u8 {\n        1\n    }\n\n"
           "    fn b() {\n        let x = 2;\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    fn t() {\n    }\n}\n")
    assert functions(src) == [("a", 3, 5), ("b", 7, 9)], functions(src)
    diff = ("diff --git a/crates/c/src/lib.rs b/crates/c/src/lib.rs\n--- a/crates/c/src/lib.rs\n"
            "+++ b/crates/c/src/lib.rs\n@@ -4 +4,2 @@\n+x\n+y\n@@ -9,2 +9,0 @@\n")
    assert changed_lines(diff) == {"crates/c/src/lib.rs": [4, 5, 9]}, dict(changed_lines(diff))
    assert crate_of("crates/c/src/lib.rs") == "c" and crate_of("script/x") is None
    assert fn_name("Host<R, D>::read") == "read" and fn_name("") is None
    print("mutation_gate_replay: selftest ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
