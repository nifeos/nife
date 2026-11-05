#!/usr/bin/env python3
"""The measured peak memory of each script/verify row, and the Kani pool weight it earns.

    python3 helpers/kani_peaks.py show                 # every row: peak, weight, fresh or stale
    python3 helpers/kani_peaks.py measure <row>...     # prove each row once, record its peak (Linux)

Name: provisional, minted 2026-11-05 (UTC) by lane/kani-memory-inventory; calef has not ruled on it.
So are helpers/kani-peaks.tsv and NIFE_KANI_PEAKS.

What it is for. helpers/heavy_slot.py's Kani pool weighs every harness at HARNESS_GIB (10 GiB) unless
told otherwise, and most proofs use a small fraction of that, so a run of small proofs holds memory it
never touches and others wait for it. helpers/kani-peaks.tsv records, per row of script/verify's crate
table, the peak memory of one `script/verify --only <row>` at VERIFY_JOBS=1: compile, solver and page
cache together, read from the run's own cgroup (`memory.peak`). A row's weight is that peak rounded up
to whole pool units. The peak already counts the compile and the page cache, and CBMC is deterministic
on the same program and version, so the rounding is the only margin. notes/verification/kani-memory.md
has the numbers and the reasoning.

A row counts only for the code it measured. Each row records the git tree of the crate's directory
and the Kani version (.kani-version) at measurement. When the checkout's tree differs, the directory
has uncommitted or untracked changes, or Kani moved, the row is stale and weighs the default again.
That matters: a lane that edits a crate's harnesses can make them ten times larger than the measured
ones, and the measured weight would then under-reserve the run.

Measuring. `measure` runs each row through script/verify (so it takes the heavy slot and the Kani
pool like any run) inside a transient systemd scope capped at --cap GiB (16 by default) with no swap,
then writes the row. It refuses a row whose directory has local changes, since the record would name
a tree it did not prove. A run that fails or reaches the cap records nothing. Linux only; on another
system measure the row on a Linux host.

BUGS: the fingerprint is the crate's own directory. A change in a dependency of the crate can grow
its proofs without staling the row.

BUGS: a row goes stale whenever its crate changes, comments included, and nothing re-measures it on a
schedule. A stale row weighs the default, which is safe and slower.

BUGS: the peaks come from an x86_64 Linux host. The `kernel` row proves different harnesses on each
host architecture, so it is not measured and weighs the default everywhere.

BUGS: a weight is a reservation, not a limit. A harness that grows past its weight (an edited one
counts as stale, but a mutant under script/falsifications does not consult this table at all) is
not stopped by the pool, and the host's own out-of-memory killer is what ends it.
"""

import math
import os
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
MEBIBYTE = 1 << 20
FIELDS = ("row", "path", "tree", "kani", "peak_mib")
HEADER = """\
# helpers/kani-peaks.tsv: the peak memory of one `script/verify --only <row>` at VERIFY_JOBS=1,
# compile included, read from the run's cgroup. Written by `python3 helpers/kani_peaks.py measure`;
# do not hand-edit a number. helpers/kani_peaks.py says when a row counts (its tree and Kani version
# must match the checkout) and notes/verification/kani-memory.md has the reasoning.
# Measured on an x86_64 Linux host.
"""


def table_path():
    return os.environ.get("NIFE_KANI_PEAKS") or os.path.join(HERE, "kani-peaks.tsv")


def load(path=None):
    """{row: {field: value}} from the table; an absent table is an empty one."""
    rows = {}
    try:
        text = open(path or table_path()).read()
    except FileNotFoundError:
        return rows
    for line in text.splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        values = line.split("\t")
        if len(values) != len(FIELDS) or values[0] == "row":
            continue
        rec = dict(zip(FIELDS, values))
        rec["peak_mib"] = int(rec["peak_mib"])
        rows[rec["row"]] = rec
    return rows


def save(rows, path=None):
    lines = [HEADER.rstrip("\n"), "\t".join(FIELDS)]
    for name in sorted(rows):
        rec = rows[name]
        lines.append("\t".join(str(rec[f]) for f in FIELDS))
    with open(path or table_path(), "w") as f:
        f.write("\n".join(lines) + "\n")


def git(root, *args):
    return subprocess.run(["git", "-C", root, *args], capture_output=True, text=True)


def kani_pin(root):
    try:
        return open(os.path.join(root, ".kani-version")).read().strip()
    except FileNotFoundError:
        return ""


def trees(root, paths):
    """{path: object id at HEAD} for each of `paths` that names a directory (or a file) there."""
    wanted, found = set(paths), {}
    # `-d` lists a directory as itself rather than its children; a file needs the plain listing.
    for flags in (["-d"], []):
        for line in git(root, "ls-tree", *flags, "HEAD", "--", *paths).stdout.splitlines():
            meta, _, path = line.partition("\t")
            if path in wanted:
                found[path] = meta.split()[2]
    return found


def dirty(root, paths):
    """The subset of `paths` with uncommitted or untracked changes."""
    out = git(root, "status", "--porcelain", "--untracked-files=all", "--", *paths).stdout
    changed = set()
    for line in out.splitlines():
        name = line[3:].split(" -> ")[-1].strip('"')
        for p in paths:
            if name == p or name.startswith(p.rstrip("/") + "/"):
                changed.add(p)
    return changed


def fresh(rows, root=ROOT):
    """{row: reason} for each of `rows` (a dict from load), where reason is '' when the row counts."""
    if not rows:
        return {}
    paths = sorted({rec["path"] for rec in rows.values()})
    head, changed, pin = trees(root, paths), dirty(root, paths), kani_pin(root)
    verdict = {}
    for name, rec in rows.items():
        if rec["kani"] != pin:
            verdict[name] = "measured with Kani %s, the pin is %s" % (rec["kani"], pin or "absent")
        elif head.get(rec["path"]) != rec["tree"]:
            verdict[name] = "%s changed since it was measured" % rec["path"]
        elif rec["path"] in changed:
            verdict[name] = "%s has local changes" % rec["path"]
        else:
            verdict[name] = ""
    return verdict


def units_of(peak_mib, unit_gib):
    """Pool units a peak earns: the peak rounded up to whole units, at least one."""
    return max(1, math.ceil(peak_mib / (unit_gib * 1024)))


def weights(names, unit_gib, default_units, root=ROOT):
    """{row: (units, why)} for each of `names`: the measured weight, or the default and the reason."""
    table = load()
    known = {n: table[n] for n in names if n in table}
    verdict = fresh(known, root)
    out = {}
    for n in names:
        if n not in known:
            out[n] = (default_units, "not measured")
        elif verdict[n]:
            out[n] = (default_units, verdict[n])
        else:
            out[n] = (units_of(known[n]["peak_mib"], unit_gib), "measured")
    return out


def verify_rows(root=ROOT):
    """The rows of script/verify's crate table, in order."""
    sys.path.insert(0, os.path.join(root, "helpers"))
    import verify_times
    return [r[0] for r in verify_times.read_table(open(os.path.join(root, "script/verify")).read())]


def crate_dirs(root):
    """{package name: its directory relative to the workspace root}, from cargo metadata."""
    import json
    out = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=root,
                         capture_output=True, text=True, check=True).stdout
    meta = json.loads(out)
    return {p["name"]: os.path.relpath(os.path.dirname(p["manifest_path"]), meta["workspace_root"])
            for p in meta["packages"]}


def show():
    import heavy_slot
    names = verify_rows()
    table = load()
    ws = weights(names, heavy_slot.UNIT_GIB, heavy_slot.default_harness_units())
    print("%-62s %9s %6s  %s" % ("row", "peak GiB", "units", "status"))
    for n in names:
        units, why = ws[n]
        peak = "%.2f" % (table[n]["peak_mib"] / 1024) if n in table else "-"
        print("%-62s %9s %6d  %s" % (n, peak, units, why))


def measure(names, cap_gib):
    if sys.platform != "linux":
        sys.exit("kani_peaks: measure reads a cgroup's memory.peak, so it runs on Linux only")
    rows = verify_rows()
    unknown = [n for n in names if n not in rows]
    if unknown:
        sys.exit("kani_peaks: not rows of script/verify's table: " + " ".join(unknown))
    if "kernel" in names:
        sys.exit("kani_peaks: the kernel row proves different harnesses on each host; it stays unmeasured")
    dirs = crate_dirs(ROOT)
    paths = {n: dirs[n.split("::")[0]] for n in names}
    local = dirty(ROOT, sorted(set(paths.values())))
    if local:
        sys.exit("kani_peaks: local changes in %s; commit first, so the record names the tree it proved"
                 % " ".join(sorted(local)))
    head, pin = trees(ROOT, sorted(set(paths.values()))), kani_pin(ROOT)
    failed = []
    for n in names:
        with tempfile.NamedTemporaryFile("r", suffix=".peak") as peak_file:
            inner = ('VERIFY_JOBS=1 script/verify --only "$1"; rc=$?; '
                     'cat "/sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/memory.peak" > "$2"; exit $rc')
            began = time.monotonic()
            r = subprocess.run(["systemd-run", "--user", "--scope", "--quiet",
                                "-p", "MemoryMax=%dG" % cap_gib, "-p", "MemorySwapMax=0",
                                "sh", "-c", inner, "measure", n, peak_file.name], cwd=ROOT)
            wall = time.monotonic() - began
            text = open(peak_file.name).read().strip()
        if r.returncode != 0 or not text:
            print("kani_peaks: %s exited %d after %.0f s; nothing recorded (a run at the %d GiB cap "
                  "ends this way too)" % (n, r.returncode, wall, cap_gib), file=sys.stderr, flush=True)
            failed.append(n)
            continue
        peak_mib = math.ceil(int(text) / MEBIBYTE)
        table = load()
        table[n] = {"row": n, "path": paths[n], "tree": head[paths[n]], "kani": pin,
                    "peak_mib": peak_mib}
        save(table)
        print("kani_peaks: %s peaked at %.2f GiB in %.0f s" % (n, peak_mib / 1024, wall), flush=True)
    if failed:
        sys.exit("kani_peaks: no record for " + " ".join(failed))


def main(argv):
    sys.path.insert(0, HERE)
    if argv == ["show"]:
        show()
        return
    if argv[:1] == ["measure"]:
        args, cap = argv[1:], 16
        if args[:1] == ["--cap"]:
            cap, args = int(args[1]), args[2:]
        if not args:
            sys.exit("usage: kani_peaks.py measure [--cap <GiB>] <row>...")
        measure(args, cap)
        return
    sys.exit("usage: kani_peaks.py show | measure [--cap <GiB>] <row>...")


if __name__ == "__main__":
    main(sys.argv[1:])
