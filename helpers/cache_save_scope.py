#!/usr/bin/env python3
"""A workflow that runs on a merge group saves no rust-cache entry from it, and what `main-caches`
saves is what the other jobs restore.

    helpers/cache_save_scope.py              # script/lint: both rules below
    helpers/cache_save_scope.py --selftest

Name provisional (lane/actions-cache-budget, 2026-10-07 UTC).

# WHY

A run restores caches from its own ref and from the default branch's. Every merge group runs on a
fresh `gh-readonly-queue/main/pr-N-<sha>` ref, so an entry saved there can never be restored by a
later run. On 2026-10-07 the repository held 10.7 GB of caches against GitHub's 10 GB limit; 163
entries (3.8 GB) were rust-cache saves from merge groups, and 111 merge-group jobs sampled that day
restored none. Those entries pushed out the pull request entries that do hit. The measurement is in
partsfoundry/record:notes/actions-cache-budget-2026-10-07.md.

# THE RULE

In every workflow under `.github/workflows/` whose `on:` lists `merge_group`, each step that uses
`Swatinem/rust-cache` sets `save-if:` to an expression that names `merge_group`. The check reads
text, not YAML, because a runner's Python and this Mac's both lack a YAML module; it finds the step
by its `uses:` line and looks for `save-if:` before the next step begins.

# THE SECOND RULE: `main-caches` saves what the restoring jobs read

ci.yml's `main-caches` job (milestone 870 (`main` keeps its CI caches warm), provisional) saves the
QEMU and patched-Kani entries on every push to `main`, so that pull requests and merge groups find
them there. It copies each `actions/cache` step's `path:`, `key:` and pinned `uses:` SHA from the
jobs that restore them. A copy that drifts saves an entry nobody reads, and the slow CI it exists
to prevent comes back with every job still green. So for each `actions/cache` step in
`main-caches`, its restorers are every `actions/cache` step in any other job of any workflow under
`.github/workflows/` (ci.yml, verify.yml and the scheduled workflows that run on `main`) whose
`path:` or `key:` equals the step's. There must be at least one, and each must carry the same
`path:`, `key:` and `uses:` ref, character for character. Matching on either field means a drift in
one of them is still compared rather than silently unmatched; a step that drifts in both has no
restorer, and that fails too. Comparing text rather than evaluated keys is deliberately strict: a
reordered `hashFiles` argument may hash the same, but it is a copy that has stopped being a copy.

# BUGS

- Only rust-cache is covered by the first rule. `actions/cache` steps (QEMU, the patched Kani, the vendor pins) use
  stable keys that hit `main`'s entries, so they save only on a miss. After a key change a merge
  group's save is just as unreachable, so ci.yml's `main-caches` job saves the QEMU and
  patched-Kani keys on every push to `main` (milestone 870 (`main` keeps its CI caches warm),
  provisional), and the second rule holds those keys to the restoring jobs'. The vendor pins are
  left to miss: one HTTP request.
- The check accepts any `save-if:` that mentions `merge_group`, so `== 'merge_group'` passes too. It
  catches a step that forgot the scope, not one that inverted it.
- The second rule compares keys, paths and SHAs, not runners. A restoring job moved to a runner
  pool `main-caches` has no leg for (or a Kani restorer moved off ARM64, where the Kani step runs
  only when `matrix.kani` is set) restores a key `main` never saves, and nothing here notices.
"""

import os
import re
import sys

WORKFLOW_DIR = ".github/workflows"
USES = re.compile(r"^(\s*)-?\s*uses:\s*Swatinem/rust-cache@")
STEP = re.compile(r"^(\s*)-\s")


def runs_on_merge_group(text):
    return re.search(r"^  merge_group:", text, re.M) is not None


def unscoped_steps(text):
    """Line numbers (1-based) of rust-cache steps with no merge_group save-if."""
    lines = text.split("\n")
    bad = []
    for i, line in enumerate(lines):
        m = USES.match(line)
        if not m:
            continue
        indent = len(m.group(1))
        ok = False
        for j in range(i + 1, len(lines)):
            nxt = lines[j]
            s = STEP.match(nxt)
            if s and len(s.group(1)) <= indent:
                break
            if nxt.strip() and not nxt.lstrip().startswith("#") and \
                    len(nxt) - len(nxt.lstrip()) < indent:
                break
            if re.match(r"\s*save-if:.*merge_group", nxt):
                ok = True
                break
        if not ok:
            bad.append(i + 1)
    return bad


CACHE_USES = re.compile(r"^(\s*)-?\s*uses:\s*(actions/cache@\S+)")
JOB = re.compile(r"^  ([A-Za-z0-9_-]+):\s*$")
WARMER = "main-caches"


def cache_steps(text):
    """Every actions/cache step as (job, line, uses, path, key), read from text.

    A job is a two-space key under `jobs:`; `uses` is the action ref without its trailing comment.
    A block-scalar `path: |` is joined line by line."""
    lines = text.split("\n")
    steps = []
    job = None
    in_jobs = False
    for i, line in enumerate(lines):
        if line.startswith("jobs:"):
            in_jobs = True
            continue
        if in_jobs:
            j = JOB.match(line)
            if j:
                job = j.group(1)
        m = CACHE_USES.match(line)
        if not m or job is None:
            continue
        indent = len(m.group(1))
        fields = {}
        k = i + 1
        while k < len(lines):
            nxt = lines[k]
            s = STEP.match(nxt)
            if s and len(s.group(1)) <= indent:
                break
            if nxt.strip() and not nxt.lstrip().startswith("#") and \
                    len(nxt) - len(nxt.lstrip()) < indent:
                break
            f = re.match(r"^(\s*)(path|key):\s*(.*?)\s*$", nxt)
            if f:
                value = f.group(3)
                if value in ("|", ">", "|-", ">-"):
                    own = len(f.group(1))
                    block = []
                    while k + 1 < len(lines) and lines[k + 1].strip() and \
                            len(lines[k + 1]) - len(lines[k + 1].lstrip()) > own:
                        k += 1
                        block.append(lines[k].strip())
                    value = "\n".join(block)
                fields[f.group(2)] = value
            k += 1
        steps.append((job, i + 1, m.group(2), fields.get("path"), fields.get("key")))
    return steps


def warmer_drift(workflows):
    """Mismatches between main-caches' cache steps and their restorers.

    `workflows` maps a file name to its text. Returns one message per mismatch."""
    warm, others = [], []
    for name in sorted(workflows):
        for job, line, uses, path, key in cache_steps(workflows[name]):
            where = "%s/%s:%d" % (WORKFLOW_DIR, name, line)
            entry = (where, uses, path, key)
            (warm if name == "ci.yml" and job == WARMER else others).append(entry)
    bad = []
    for where, uses, path, key in warm:
        readers = [o for o in others if o[2] == path or o[3] == key]
        if not readers:
            bad.append("%s: no other job restores path %r or key %r" % (where, path, key))
        for field, mine, at in (("path", path, 2), ("key", key, 3), ("uses", uses, 1)):
            # One message per distinct disagreeing value, naming every restorer that holds it.
            seen = {}
            for r in readers:
                if r[at] != mine:
                    seen.setdefault(r[at], []).append(r[0].replace(WORKFLOW_DIR + "/", ""))
            for theirs, sites in sorted(seen.items(), key=lambda kv: str(kv[0])):
                bad.append("%s: %s %r, but %s restore%s with %r" %
                           (where, field, mine, ", ".join(sites),
                            "" if len(sites) > 1 else "s", theirs))
    return bad


def read_workflows(root):
    d = os.path.join(root, WORKFLOW_DIR)
    out = {}
    for name in sorted(os.listdir(d)):
        if name.endswith((".yml", ".yaml")):
            out[name] = open(os.path.join(d, name)).read()
    return out


def lint(root="."):
    workflows = read_workflows(root)
    rc = 0
    failures = []
    for name, text in sorted(workflows.items()):
        if not runs_on_merge_group(text):
            continue
        for n in unscoped_steps(text):
            failures.append("%s/%s:%d" % (WORKFLOW_DIR, name, n))
    if failures:
        sys.stderr.write(
            "lint: a rust-cache step in a merge-group workflow saves an entry no run can restore.\n"
            "  Add `with: save-if: ${{ github.event_name != 'merge_group' }}` (see\n"
            "  helpers/cache_save_scope.py):\n")
        for f in failures:
            sys.stderr.write("    %s\n" % f)
        rc = 1
    # A renamed or removed job would otherwise make the comparison below pass over nothing.
    if not any(job == WARMER for job, *_ in cache_steps(workflows.get("ci.yml", ""))):
        sys.stderr.write("lint: ci.yml has no `%s` job with an actions/cache step, so the\n"
                         "  key comparison read nothing (helpers/cache_save_scope.py).\n" % WARMER)
        return 1
    drift = warmer_drift(workflows)
    if drift:
        sys.stderr.write(
            "lint: ci.yml's `main-caches` saves a cache entry the restoring jobs do not read\n"
            "  (milestone 870; helpers/cache_save_scope.py). Make each path, key and pinned\n"
            "  actions/cache SHA identical in both places:\n")
        for f in drift:
            sys.stderr.write("    %s\n" % f)
        rc = 1
    return rc


def selftest():
    scoped = (
        "on:\n  merge_group:\njobs:\n  a:\n    steps:\n"
        "      - uses: Swatinem/rust-cache@v2\n"
        "        if: x\n"
        "        with:\n"
        "          save-if: ${{ github.event_name != 'merge_group' }}\n"
        "      - run: true\n")
    bare = (
        "on:\n  merge_group:\njobs:\n  a:\n    steps:\n"
        "      - uses: Swatinem/rust-cache@v2\n"
        "      - run: true\n"
        "        with:\n"
        "          save-if: merge_group\n")
    other_key = (
        "jobs:\n  a:\n    steps:\n"
        "      - uses: Swatinem/rust-cache@v2\n"
        "        with:\n"
        "          shared-key: x\n"
        "      - run: true\n")
    cases = [
        (unscoped_steps(scoped), []),
        # The next step's `save-if` must not count for the bare one above it.
        (unscoped_steps(bare), [6]),
        (unscoped_steps(other_key), [4]),
        (runs_on_merge_group(scoped), True),
        (runs_on_merge_group(other_key), False),
    ]
    # The second rule. One warmer step and two restorers, one in another workflow; then each of
    # path, key and SHA drifted on its own, and all three at once.
    sha = "actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
    key = "qemu-${{ runner.os }}-${{ hashFiles('.qemu-version') }}"

    def cache_step(uses=sha, path="~/.cache/nife-qemu", k=key, extra=""):
        return ("      - name: Cache QEMU\n"
                "        uses: %s # v6\n"
                "        with:\n"
                "          path: %s\n"
                "          key: %s\n%s"
                "      - run: script/ci-qemu\n" % (uses, path, k, extra))

    def ci(warm_step):
        return ("on:\n  push:\njobs:\n"
                "  main-caches:\n    steps:\n" + warm_step +
                "  test:\n    steps:\n" + cache_step() +
                "  supply-chain:\n    steps:\n" +
                cache_step(path="~/.cache/nife-vendor", k="nife-vendor-x"))

    verify = "on:\n  push:\njobs:\n  prove:\n    steps:\n" + cache_step()

    def drift(warm_step):
        return warmer_drift({"ci.yml": ci(warm_step), "verify.yml": verify})

    clean = drift(cache_step(extra="          lookup-only: true\n"))
    bad_key = drift(cache_step(k=key.replace("qemu-", "qemu-${{ runner.arch }}-")))
    bad_path = drift(cache_step(path="~/.cache/qemu"))
    bad_sha = drift(cache_step(uses="actions/cache@0123456789abcdef0123456789abcdef01234567"))
    bad_all = drift(cache_step(uses="actions/cache@v5", path="~/q", k="q-1"))
    block = cache_steps("jobs:\n  a:\n    steps:\n      - uses: %s\n        with:\n"
                        "          path: |\n            ~/a\n            ~/b\n"
                        "          key: k\n      - run: true\n" % sha)
    cases += [
        (clean, []),
        # Each planted drift is caught once, names its field, and names both restorers.
        ([m.split(": ")[1].split(" ")[0] for m in bad_key], ["key"]),
        ([m.split(": ")[1].split(" ")[0] for m in bad_path], ["path"]),
        ([m.split(": ")[1].split(" ")[0] for m in bad_sha], ["uses"]),
        (all("ci.yml:" in m and "verify.yml:" in m for m in bad_key + bad_path + bad_sha), True),
        (len(bad_all) == 1 and "no other job restores" in bad_all[0], True),
        (block, [("a", 4, sha, "~/a\n~/b", "k")]),
    ]
    ok = all(got == want for got, want in cases)
    for got, want in cases:
        if got != want:
            print("cache_save_scope selftest: got %r, want %r" % (got, want))
    print("cache_save_scope selftest: %s" % ("ok" if ok else "FAILED"))
    return 0 if ok else 1


def main(argv):
    if argv[1:] == ["--selftest"]:
        return selftest()
    if argv[1:]:
        sys.stderr.write(__doc__)
        return 2
    return lint()


if __name__ == "__main__":
    sys.exit(main(sys.argv))
