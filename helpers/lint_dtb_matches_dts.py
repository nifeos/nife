#!/usr/bin/env python3
"""lint: every committed `.dtb` is what the `.dts` beside it compiles to.

The `script/lint` section "==> every committed .dtb matches its .dts" runs this helper. For each
committed `.dts` (`git ls-files`, vendor/ and patches/ excluded) with a same-stem `.dtb` in the
same directory, it compiles the source with `dtc`, decompiles BOTH blobs with the same local
`dtc` (`dtc -I dtb -O dts`), and diffs the two decompilations. Comparing decompiled structure
rather than bytes is what keeps a `dtc` version difference from failing a contributor: one local
tool produced both texts, so anything that is merely this `dtc`'s house style cancels out, and a
difference that survives is a real drift between source and blob.

Where `dtc` is absent the helper skips, and its output says so: a skip is a pass on a machine
without `dtc`, and CI's lint job installs `dtc`, so the gate is real where merges happen.

What it covers, said where a reader meets it: a comparison, so only the blobs that have a source
beside them. `bench/` images, the font atlases, the RedoxFS test image and the QEMU-dumped `.dtb`
fixtures have no source to compare against; the helper prints their count rather than passing
over them in silence. A pair set of zero fails rather than passing, because a gate that selects
the set it judges can pass by checking nothing (milestone 401 (a gate that selects the set it
judges)).

BUGS:
- The selftest drives a stub `dtc`, so it proves the plumbing (match, mismatch, skip, coverage
  counts, the zero-pair guard) and not real device-tree parsing. The parsing is proved on the
  tree's own fixtures on every lint run.
- A `.dts` compiled with flags this helper does not pass (none is today) would mismatch; the
  remedy line names the one command that regenerates the blob.

Name: provisional, minted 2026-10-15 (UTC) by milestone 491 (a committed .dtb is checked against
what its .dts compiles to), for the lint section of the same name. calef has not ratified it.
"""

import difflib
import itertools
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

# Both facts the block's design asks the skip to carry, in the check's own output: a skip is a
# pass here, and the gate is real in CI.
SKIP_NOTE = (
    "dtb blobs: dtc not found, so the .dts/.dtb comparison is skipped. A skip is a pass on a\n"
    "machine without dtc; CI's lint job installs dtc (device-tree-compiler), so the gate is\n"
    "real where merges happen."
)

MAX_DIFF_LINES = 14


def git_files(root, pattern):
    """Sorted tracked paths matching pattern, vendor/ and patches/ excluded."""
    out = subprocess.run(
        ["git", "ls-files", pattern, ":!vendor", ":!patches"],
        cwd=str(root), capture_output=True, text=True, check=True,
    )
    return sorted(out.stdout.split())


def decompile(dtc, blob, cwd):
    """`dtc -I dtb -O dts` stdout, or None if dtc itself failed."""
    proc = subprocess.run([dtc, "-I", "dtb", "-O", "dts", str(blob)],
                          cwd=str(cwd), capture_output=True, text=True)
    if proc.returncode != 0:
        return None
    return proc.stdout


def compare_pair(dtc, root, scratch, dts):
    """(diff_head, error). Exactly one of the two is a real result.

    diff_head is None or empty when the pair matches; error is a compile failure.
    """
    dtb = dts[:-4] + ".dtb"
    fresh = scratch / "fresh.dtb"
    proc = subprocess.run([dtc, "-I", "dts", "-O", "dtb", "-o", str(fresh), dts],
                          cwd=str(root), capture_output=True, text=True)
    if proc.returncode != 0:
        return None, (f"{dts}: dtc refuses to compile it:\n{proc.stderr.strip()}")
    committed = decompile(dtc, root / dtb, root)
    rebuilt = decompile(dtc, fresh, root)
    if committed is None or rebuilt is None:
        return None, f"{dts}: dtc failed to decompile one of the two blobs"
    if committed == rebuilt:
        return [], None
    diff = difflib.unified_diff(
        committed.splitlines(), rebuilt.splitlines(),
        fromfile=f"decompiled {dtb} (committed)",
        tofile=f"decompiled {dtb} (fresh from {dts})", lineterm="")
    return list(itertools.islice(diff, MAX_DIFF_LINES)), None


def check(root, dts_files, dtb_files, dtc, out, err):
    """Run the comparison over explicit file lists. Returns the exit code.

    The file lists are parameters rather than queries so the selftest can drive fixtures
    without a git tree; `main()` is the one that asks git.
    """
    dtb_set = set(dtb_files)
    pairs, bare_dts = [], []
    for dts in dts_files:
        dtb = dts[:-4] + ".dtb"
        if dtb in dtb_set:
            pairs.append(dts)
        else:
            bare_dts.append(dts)
    paired_dtbs = {d[:-4] + ".dtb" for d in pairs}
    bare_dtb = sorted(dtb_set - paired_dtbs)

    if not pairs:
        # A selector that matches nothing passes by checking nothing; both ways of arriving at
        # an empty pair set are loud (milestone 401 (a gate that selects the set it judges)).
        if not dts_files:
            err.write("lint: git ls-files '*.dts' matched nothing, so the committed-blob "
                      "comparison judged nothing.\nThe fixtures moved or are gone; fix the "
                      "selector above (milestone 401 (a gate that selects the set it judges)).\n")
        else:
            err.write("lint: no committed .dts sits beside a committed .dtb, so the "
                      "committed-blob comparison judged nothing.\n"
                      f"{len(dts_files)} .dts file(s) matched, none paired. If the blobs moved "
                      "beside or away from their sources, move this check with them\n"
                      "(milestone 401 (a gate that selects the set it judges)).\n")
        return 1

    scratch = pathlib.Path(tempfile.mkdtemp(prefix="lint-dtb-"))
    try:
        results = [(dts,) + compare_pair(dtc, root, scratch, dts) for dts in pairs]
    finally:
        shutil.rmtree(scratch, ignore_errors=True)

    bad = [(dts, diff_head, error) for dts, diff_head, error in results
           if error or diff_head]
    if bad:
        err.write("lint: a committed .dtb is not what the .dts beside it compiles to:\n")
        for dts, diff_head, error in bad:
            entry = error if error else "\n".join(diff_head)
            err.write(f"\n--- {dts} ---\n{entry}\n")
        err.write(
            "\nRegenerate the blob from its source and commit both:\n"
            "  dtc -I dts -O dtb -o <the .dtb> <the .dts>\n"
            "The comparison is decompiled structure, not bytes, so a dtc version difference\n"
            "cannot cause this: the drift is real, and a sweep that edits the text cannot\n"
            "reach the binary beside it.\n"
            "(milestone 491 (a committed .dtb is checked against what its .dts compiles to))\n")
        return 1

    out.write(f"dtb blobs: {len(pairs)} pair(s) compared, each committed .dtb is what its "
              f".dts compiles to\n")
    if bare_dtb:
        out.write(f"  {len(bare_dtb)} committed .dtb with no .dts beside them are NOT covered "
                  "(nothing to compare against):\n")
        for path in bare_dtb:
            out.write(f"    {path}\n")
    if bare_dts:
        out.write(f"  {len(bare_dts)} committed .dts with no .dtb beside them are NOT covered "
                  "(no blob to check):\n")
        for path in bare_dts:
            out.write(f"    {path}\n")
    return 0


# The stub `dtc` the selftest drives: `-I dts -O dtb` writes "blob:" plus the source text,
# `-I dtb -O dts` prints the text back. A committed blob that does not start with "blob:" is a
# stale one by construction, which is how the mismatch fixture is built.
STUB_DTC = """#!{interpreter}
import sys
args = sys.argv[1:]
src = args[-1]
if "-I" in args and args[args.index("-I") + 1] == "dtb":
    data = open(src, "rb").read()
    sys.stdout.write(data.decode() if not data.startswith(b"blob:") else data[len(b"blob:"):].decode())
else:
    out = args[args.index("-o") + 1]
    open(out, "wb").write(("blob:" + open(src).read()).encode())
"""


class Case:
    def __init__(self, name, files, dts, dtbs):
        self.name = name
        self.files = files
        self.dts = dts
        self.dtbs = dtbs


def selftest():
    import io

    failures = []

    def expect(name, got, want, what):
        if got != want:
            failures.append(f"{name}: {what}: got {got!r}, want {want!r}")

    def run_case(case, dtc):
        with tempfile.TemporaryDirectory(prefix="lint-dtb-selftest.") as td:
            root = pathlib.Path(td)
            for rel, content in case.files.items():
                path = root / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            out, err = io.StringIO(), io.StringIO()
            rc = check(root, case.dts, case.dtbs, dtc, out, err)
            return rc, out.getvalue(), err.getvalue()

    with tempfile.TemporaryDirectory(prefix="lint-dtb-selftest-bin.") as td:
        stub_dir = pathlib.Path(td)
        stub = stub_dir / "dtc"
        stub.write_text(STUB_DTC.format(interpreter=sys.executable))
        stub.chmod(0o755)

        cases = [
            Case("a matching pair passes",
                 {"a.dts": "hello\n", "a.dtb": "blob:hello\n"},
                 ["a.dts"], ["a.dtb"]),
            Case("a stale blob fails",
                 {"a.dts": "hello\n", "a.dtb": "blob:an older tree\n"},
                 ["a.dts"], ["a.dtb"]),
            Case("an empty pair set fails",
                 {"x.dtb": "blob:x\n"},
                 [], ["x.dtb"]),
            Case("uncovered blobs are counted, not passed in silence",
                 {"a.dts": "hello\n", "a.dtb": "blob:hello\n",
                  "dump.dtb": "blob:dump\n", "lonely.dts": "hi\n"},
                 ["a.dts", "lonely.dts"], ["a.dtb", "dump.dtb"]),
        ]
        for case in cases:
            rc, out, err = run_case(case, str(stub))
            if case.name == "a matching pair passes":
                expect(case.name, rc, 0, "exit code")
                expect(case.name, "1 pair(s) compared" in out, True, "prints the pair count")
            elif case.name == "a stale blob fails":
                expect(case.name, rc, 1, "exit code")
                expect(case.name, "a.dtb" in err, True, "names the blob")
                expect(case.name, "dtc -I dts -O dtb" in err, True, "names the remedy")
            elif case.name == "an empty pair set fails":
                expect(case.name, rc, 1, "exit code")
                expect(case.name, "judged nothing" in err, True, "says it judged nothing")
            else:
                expect(case.name, rc, 0, "exit code")
                expect(case.name, "NOT covered" in out, True, "counts uncovered blobs")
                expect(case.name, "dump.dtb" in out, True, "names the sourceless blob")
                expect(case.name, "lonely.dts" in out, True, "names the blobless source")

    # The skip path, end to end: git reachable, dtc not. The throwaway repo holds one good
    # pair; PATH keeps git (symlinked into a scratch bin) and loses every other entry.
    with tempfile.TemporaryDirectory(prefix="lint-dtb-selftest-repo.") as td:
        repo = pathlib.Path(td) / "repo"
        (repo / "d").mkdir(parents=True)
        (repo / "d" / "a.dts").write_text("hello\n")
        (repo / "d" / "a.dtb").write_text("blob:hello\n")
        bin_dir = pathlib.Path(td) / "bin"
        bin_dir.mkdir()
        git = shutil.which("git")
        if git is None:
            print("selftest: no git on PATH; skipping the end-to-end skip case", file=sys.stderr)
        else:
            os.symlink(git, str(bin_dir / "git"))
            subprocess.run(["git", "init", "-q"], cwd=str(repo), check=True,
                           env={**os.environ, "PATH": str(bin_dir)})
            subprocess.run(["git", "add", "-A"], cwd=str(repo), check=True,
                           env={**os.environ, "PATH": str(bin_dir)})
            subprocess.run(["git", "-c", "user.email=t@t", "-c", "user.name=t", "commit",
                            "-qm", "fixtures"], cwd=str(repo), check=True,
                           env={**os.environ, "PATH": str(bin_dir)})
            # CI itself sets CI=true, which turns this skip into a failure; the skip case must
            # not inherit it.
            skip_env = {k: v for k, v in os.environ.items() if k != "CI"}
            proc = subprocess.run([sys.executable, os.path.abspath(__file__)],
                                  cwd=str(repo), capture_output=True, text=True,
                                  env={**skip_env, "PATH": str(bin_dir)})
            expect("the skip path", proc.returncode, 0, "exit code")
            ci = subprocess.run([sys.executable, os.path.abspath(__file__)],
                                cwd=str(repo), capture_output=True, text=True,
                                env={**os.environ, "PATH": str(bin_dir), "CI": "true"})
            expect("missing dtc under CI", ci.returncode, 1, "exit code")
            expect("missing dtc under CI", "nothing was judged" in ci.stderr, True,
                   "says nothing was judged")
            expect("the skip path", "A skip is a pass" in proc.stdout, True,
                   "says a skip is a pass")
            expect("the skip path", "CI's lint job installs dtc" in proc.stdout, True,
                   "says CI has the tool")

    if failures:
        for f in failures:
            print(f"selftest: {f}", file=sys.stderr)
        sys.exit(1)
    print("selftest: match, mismatch, skip, zero-pair guard and coverage counts, all green")


def main():
    if "--selftest" in sys.argv:
        selftest()
        return
    root = pathlib.Path.cwd()
    dtc = shutil.which("dtc")
    if dtc is None:
        if os.environ.get("CI") == "true":
            # CI installs dtc; a missing one there is a broken runner image, and a skip would
            # let every blob through unjudged.
            print("dtb blobs: CI=true but dtc is not installed, so nothing was judged. "
                  "Install device-tree-compiler on the runner.", file=sys.stderr)
            sys.exit(1)
        print(SKIP_NOTE)
        return
    sys.exit(check(root, git_files(root, "*.dts"), git_files(root, "*.dtb"), dtc,
                   sys.stdout, sys.stderr))


if __name__ == "__main__":
    main()
