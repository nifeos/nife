#!/usr/bin/env python3
"""Which mutants of the code a standing Kani harness covers does the proof kill?

The measurement behind milestone 741 (does a standing proof notice a regression), whose number is
provisional, for fatal risk 2 (the proofs prove trivia). A harness that kills no mutant of the code it claims to cover
proves nothing a regression could break. Name provisional (2026-10-04, UTC); naming is an
architect's.

Three steps, so the expensive one can be sharded in CI and the cheap ones run anywhere:

    kani_reach.py plan   --package P [--kani-args "..."] [--file GLOB]... --out plan.json
    kani_reach.py run    --plan plan.json [--shard k/n] [--timeout S] --out results.jsonl
    kani_reach.py report --plan plan.json --results results.jsonl... [--census DIR] --out table.md

**plan** lists cargo-mutants' mutants for one package (`--no-config`, with the same harness and
test-module exclusions `.cargo/mutants.toml` makes, so `kernel/**` can be listed at all), parses
every `#[kani::proof]` in that package's source, and computes each harness's reach over the text.
Reach is a name-based call graph over comment-and-literal-stripped source, and it is stated as an
approximation rather than hidden:

  - **direct**: functions named in the harness body, plus the bodies of non-harness helpers in the
    harness's own file that it names (so a harness that builds its input in a helper still reaches
    what the helper calls).
  - **transitive**: the closure of direct over every `fn` definition in the package, by name.

Both over-approximate where two functions share a name (`new`, `len`) and under-approximate where a
call is not spelled as a name (an operator impl, `?` through `From`, a trait object). The `run` step
therefore proves every harness in the package against every mutant by default, so a kill outside
the computed reach is seen and counted rather than lost; the report says how many there were.

**run** proves each mutant with `cargo kani`, one mutant at a time, applying cargo-mutants' own diff
to the source and reverting it from the bytes it read. A harness that goes FAILED on a mutant kills
it. A harness that times out is a timeout, never a kill. A mutant that does not compile under Kani
is unviable and leaves every denominator.

**report** joins the results with a mutation census's `outcomes.json` files (the weekly
`mutation testing` workflow's artifacts), so each mutant is classed by whether a proof kills it,
whether a test kills it, both, or neither.

BUGS

- **An `unsafe fn` is never mutated, so a proof over one is invisible here.** cargo-mutants 27.1.0
  generates no mutant inside an `unsafe fn`. Measured 2026-10-04: `inter_process_communication`'s
  `Rendezvous::send`, `recv`, `remove_sender` and `remove_receiver` yield none, so four of its
  harnesses (`send_rendezvous_iff_a_receiver_waited`, `recv_drains_a_pending_signal_first`,
  `a_collected_sender_is_forgotten`, `a_wait_returns_a_nonzero_word_or_blocks`) reach no mutant
  and get no score. The weekly mutation census has the same blind spot, for the same reason. A
  hand-written mutant set for those functions is the way round it; none exists yet.
- **Reach is a name-based approximation.** It misses a call that is not spelled as a name: the
  pilot found 8 of elf's kills outside the computed reach, through `Iterator::next` under a `for`
  loop and through `u32le`. With `--reached-only` such kills are never looked for, so a
  `--reached-only` score is a lower bound.
- **A mutant outside every `fn` (a `const` initializer) is reached by name only**, and a `const`
  that feeds another `const` is not followed.
- **Kani is installed unpinned in CI**, as `verify.yml` installs it: the 2026-10-04 runs used Kani
  0.68.0 and CBMC 6.11.0, where patagonia has 0.67.0.
"""

import argparse
import fnmatch
import glob
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rust_source import strip_non_code  # noqa: E402

EXCLUDE_RE = ["verification::", "proofs::", "interleavings::", "tests::"]
EXCLUDE_GLOB = ["**/src/**/proofs.rs", "**/src/**/verification.rs"]
PROOF = re.compile(r"#\[kani::proof\]")
FN_DEF = re.compile(r"\bfn\s+([A-Za-z_]\w*)")
MOD_OPEN = re.compile(r"\bmod\s+([A-Za-z_]\w*)\s*\{")
CALL = re.compile(r"\b([A-Za-z_]\w*)\s*(?:::\s*<[^()]*?>\s*)?\(")
IDENT = re.compile(r"\b([A-Za-z_]\w*)\b")
CONST_DEF = re.compile(r"\b(?:const|static)\s+([A-Za-z_]\w*)\s*:")
KEYWORDS = {"if", "while", "for", "match", "loop", "return", "Some", "Ok", "Err", "None", "fn",
            "assert", "assert_eq", "assert_ne", "matches", "let", "in", "as", "unsafe", "move"}


def sh(argv, **kw):
    return subprocess.run(argv, capture_output=True, text=True, **kw)


def package_dir(package):
    meta = json.loads(sh(["cargo", "metadata", "--no-deps", "--format-version", "1"],
                         check=True).stdout)
    for p in meta["packages"]:
        if p["name"] == package:
            root = os.path.dirname(p["manifest_path"])
            return os.path.relpath(root)
    sys.exit(f"kani_reach: no package {package!r} in this workspace")


def match_braces(text, open_at):
    """Index one past the `}` closing the `{` at `open_at`, in already-stripped text."""
    depth = 0
    for i in range(open_at, len(text)):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i + 1
    return len(text)


def signature_end(text, at):
    """The `{` opening a fn body, or None for a bodiless declaration. Depth-aware, because a
    signature like `-> [u32; 16]` holds a `;` that ends nothing."""
    depth = 0
    for i in range(at, len(text)):
        c = text[i]
        if c in "([":
            depth += 1
        elif c in ")]":
            depth -= 1
        elif depth == 0 and c == "{":
            return i
        elif depth == 0 and c == ";":
            return None
    return None


def file_module_path(path, src_root):
    rel = os.path.relpath(path, src_root)
    parts = rel[:-3].split(os.sep)
    if parts[-1] in ("lib", "main", "mod"):
        parts = parts[:-1]
    return parts


def parse_file(path, src_root):
    """Every `fn` definition in one file: name, module path, line range, stripped body, harness?"""
    raw = open(path, encoding="utf-8").read()
    text = strip_non_code(raw)
    line_of = [0]
    for i, c in enumerate(text):
        if c == "\n":
            line_of.append(i + 1)

    def line(pos):
        lo, hi = 0, len(line_of) - 1
        while lo < hi:
            mid = (lo + hi + 1) // 2
            if line_of[mid] <= pos:
                lo = mid
            else:
                hi = mid - 1
        return lo + 1

    mods = []  # (name, start, end) of inline modules
    for m in MOD_OPEN.finditer(text):
        brace = text.index("{", m.end() - 1)
        mods.append((m.group(1), m.start(), match_braces(text, brace)))
    base = file_module_path(path, src_root)
    out = []
    for m in FN_DEF.finditer(text):
        brace = signature_end(text, m.end())
        if brace is None:
            continue  # a declaration in a trait or an extern block, no body
        end = match_braces(text, brace)
        inline = [n for (n, s, e) in mods if s < m.start() < e]
        # A harness is a fn whose own attribute run holds `#[kani::proof]`: nothing but
        # attributes and whitespace between the attribute and the `fn`. Found by the 2026-10-04
        # scaled run: a helper fn nested INSIDE a harness body (capability's `occupied`, pci's
        # `queued`) has the harness's attribute 400 characters back with no `}` between, and the
        # first version of this check called it a harness, which Kani then refused to match.
        head = text[max(0, m.start() - 400):m.start()]
        hits = list(PROOF.finditer(head))
        harness = bool(hits) and not re.search(r"[{};]", head[hits[-1].end():])
        out.append({
            "name": m.group(1),
            "path": "::".join(base + inline + [m.group(1)]),
            "file": path,
            "start": line(m.start()),
            "end": line(end - 1),
            "body": text[brace:end],
            "harness": harness,
            "in_proof_module": any(n in ("verification", "proofs") for n in inline)
                               or os.path.basename(path) in ("verification.rs", "proofs.rs"),
        })
    return out


def calls(body):
    return {n for n in CALL.findall(body) if n not in KEYWORDS}


def plan(args):
    pdir = package_dir(args.package)
    src_root = os.path.join(pdir, "src")
    files = sorted(glob.glob(os.path.join(pdir, "src", "**", "*.rs"), recursive=True))
    defs = [d for f in files for d in parse_file(f, src_root)]
    by_name = {}
    for d in defs:
        by_name.setdefault(d["name"], []).append(d)

    def resolve(name, from_file):
        # An unqualified call resolves in scope first, so a name defined in the caller's own file
        # is that one and not every same-named fn in the package (`init`, `enable`, `new`).
        cands = by_name.get(name, [])
        local = [d for d in cands if d["file"] == from_file]
        return local or cands

    def closure(seed_file, seed_names, through_helpers_only):
        seen = set()
        frontier = {(seed_file, n) for n in seed_names}
        done = set()
        while frontier:
            item = frontier.pop()
            if item in done:
                continue
            done.add(item)
            from_file, n = item
            for d in resolve(n, from_file):
                key = (d["file"], d["start"])
                if key in seen or d["harness"]:
                    continue
                if through_helpers_only and not d["in_proof_module"]:
                    seen.add(key)  # reached, but do not follow its own calls
                    continue
                seen.add(key)
                frontier |= {(d["file"], c) for c in calls(d["body"])}
        return seen

    harnesses = []
    for d in defs:
        if not d["harness"]:
            continue
        if args.harness_re and not re.search(args.harness_re, d["path"]):
            continue
        seeds = calls(d["body"])
        direct = closure(d["file"], seeds, through_helpers_only=True)
        trans = closure(d["file"], seeds, through_helpers_only=False)
        idents = set(IDENT.findall(d["body"]))
        for key in trans:
            for dd in defs:
                if (dd["file"], dd["start"]) == key:
                    idents |= set(IDENT.findall(dd["body"]))
        harnesses.append({
            "path": d["path"], "file": d["file"], "line": d["start"],
            "direct": sorted(f"{f}:{s}" for f, s in direct),
            "transitive": sorted(f"{f}:{s}" for f, s in trans),
            "idents": sorted(idents),
        })

    argv = ["cargo", "mutants", "--no-config", "-p", args.package, "--list", "--json", "--diff"]
    for r in EXCLUDE_RE:
        argv += ["--exclude-re", r]
    for g in EXCLUDE_GLOB:
        argv += ["--exclude", g]
    for f in args.file or []:
        argv += ["--file", f]
    mutants = json.loads(sh(argv, check=True).stdout)

    def enclosing(file, line):
        best = None
        for d in defs:
            if d["file"] == file and d["start"] <= line <= d["end"] and not d["harness"]:
                if best is None or d["start"] >= best["start"]:
                    best = d
        return best

    src_cache = {}
    for m in mutants:
        line = m["span"]["start"]["line"]
        d = enclosing(m["file"], line)
        m["def"] = f"{d['file']}:{d['start']}" if d else None
        m["const"] = None
        if d is None:
            if m["file"] not in src_cache:
                src_cache[m["file"]] = strip_non_code(open(m["file"]).read()).split("\n")
            # A mutant outside any fn is in a const or static initializer; walk up to its name.
            lines = src_cache[m["file"]]
            for back in range(line - 1, max(-1, line - 20), -1):
                c = CONST_DEF.search(lines[back])
                if c:
                    m["const"] = c.group(1)
                    break
        reach_d, reach_t = [], []
        for h in harnesses:
            if m["def"]:
                if m["def"] in h["direct"]:
                    reach_d.append(h["path"])
                if m["def"] in h["transitive"]:
                    reach_t.append(h["path"])
            elif m["const"] and m["const"] in h["idents"]:
                reach_t.append(h["path"])
        m["reach_direct"], m["reach_transitive"] = reach_d, reach_t

    out = {"package": args.package, "kani_args": args.kani_args or "", "files": args.file or [],
           "reach_method": "name-based call graph over stripped source (see kani_reach.py)",
           "harnesses": [{k: v for k, v in h.items() if k != "idents"} for h in harnesses],
           "mutants": mutants}
    json.dump(out, open(args.out, "w"), indent=1)
    print(f"plan: {args.package}: {len(harnesses)} harnesses, {len(mutants)} mutants, "
          f"{sum(1 for m in mutants if m['reach_transitive'])} in some harness's transitive reach")


def apply_diff(diff, original):
    """Apply one cargo-mutants unified diff to `original`; refuse on any context mismatch."""
    lines = original.split("\n")
    out, cursor = [], 0
    hunk = re.compile(r"^@@ -(\d+)(?:,(\d+))? \+\d+(?:,\d+)? @@")
    dl = diff.split("\n")
    i = 0
    while i < len(dl) and not dl[i].startswith("@@"):
        i += 1
    while i < len(dl):
        h = hunk.match(dl[i])
        if not h:
            i += 1
            continue
        start = int(h.group(1)) - 1
        out += lines[cursor:start]
        cursor = start
        i += 1
        while i < len(dl) and not dl[i].startswith("@@"):
            t = dl[i]
            if t.startswith(" ") or t == "":
                if t == "" and i == len(dl) - 1:
                    break
                if lines[cursor] != t[1:]:
                    raise ValueError(f"context mismatch at line {cursor + 1}")
                out.append(lines[cursor])
                cursor += 1
            elif t.startswith("-"):
                if lines[cursor] != t[1:]:
                    raise ValueError(f"removal mismatch at line {cursor + 1}")
                cursor += 1
            elif t.startswith("+"):
                out.append(t[1:])
            i += 1
    out += lines[cursor:]
    return "\n".join(out)


HARNESS_START = re.compile(r"Checking harness (\S+)\.\.\.")
VERDICT = re.compile(r"VERIFICATION:- (SUCCESSFUL|FAILED)")
VTIME = re.compile(r"Verification Time: ([0-9.]+)s")


MEMORY_CEILING = 12 << 30  # of a 16 GiB runner, leaving the runner agent room to report
# Linux only: macOS refuses to lower RLIMIT_AS, and the sweep runs in CI, not on a laptop.


def _memory_ceiling():
    import resource
    resource.setrlimit(resource.RLIMIT_AS, (MEMORY_CEILING, MEMORY_CEILING))


def killed_by(per):
    """Harnesses that turned a mutant red on a property. FAILED alone is not enough: a CBMC that
    ran out of memory, or died, also ends FAILED, and that is a resource outcome, not a kill."""
    return sorted(h for h, x in per.items()
                  if x["verdict"] == "FAILED" and not x["timeout"] and not x.get("resource")
                  and x.get("failed_checks", True))


def run_kani(package, kani_args, harnesses, timeout):
    argv = ["cargo", "kani", "-p", package, "-j", "1", "--output-format=terse",
            "-Z", "unstable-options", "--harness-timeout", f"{timeout}s"]
    argv += kani_args.split()
    for h in harnesses:
        argv += ["--harness", h, "--exact"]
    t0 = time.monotonic()
    # An address-space ceiling on cargo, rustc and CBMC alike, so a mutant that makes CBMC's
    # formula explode fails its own allocation instead of taking the runner down with it. That
    # is what lost `subtree_scope` shard 1/2 on 2026-10-04 ("the hosted runner lost communication
    # with the server" after 93 minutes), the same failure `helpers/memory-bounded-runner.sh`
    # bounds for the mutation census.
    p = subprocess.run(argv, capture_output=True, text=True, preexec_fn=_memory_ceiling if sys.platform == "linux" else None)
    wall = time.monotonic() - t0
    log = p.stdout + p.stderr
    per, cur = {}, None
    for line in log.split("\n"):
        m = HARNESS_START.search(line)
        if m:
            cur = m.group(1)
            per[cur] = {"verdict": None, "secs": None, "timeout": False, "failed_checks": False,
                        "resource": False, "tail": []}
            continue
        if cur is None:
            continue
        per[cur]["tail"] = (per[cur]["tail"] + [line])[-12:]
        if line.lstrip().startswith("Failed Checks:"):
            per[cur]["failed_checks"] = True
        if re.search(r"bad_alloc|out of memory|Cannot allocate|memory exhausted|signal 9|Killed",
                     line):
            per[cur]["resource"] = True
        if "timed out" in line.lower() or "timeout" in line.lower():
            per[cur]["timeout"] = True
        m = VERDICT.search(line)
        if m:
            per[cur]["verdict"] = m.group(1)
        m = VTIME.search(line)
        if m:
            per[cur]["secs"] = float(m.group(1))
    compiled = "Checking harness" in log
    return {"exit": p.returncode, "wall": wall, "compiled": compiled, "harnesses": per,
            "log_tail": log[-3000:] if not compiled or p.returncode not in (0, 1) else ""}


def run(args):
    import heavy_slot
    heavy_slot.take_kani(1, "kani_reach")  # one proof at a time; off in CI (GITHUB_ACTIONS)
    pl = json.load(open(args.plan))
    pkg, kargs = pl["package"], pl["kani_args"]
    all_h = [h["path"] for h in pl["harnesses"]]
    out = open(args.out, "a")
    base = run_kani(pkg, kargs, all_h, max(args.timeout, 1800))
    out.write(json.dumps({"baseline": True, **base}) + "\n")
    out.flush()
    bad = [h for h, r in base["harnesses"].items() if r["verdict"] != "SUCCESSFUL"]
    missing = sorted(set(all_h) - set(base["harnesses"]))
    if not base["compiled"] or bad or missing:
        print(f"baseline is not green: failed {bad}, not run {missing}\n{base['log_tail']}")
        sys.exit(1)
    mutants = pl["mutants"]
    if args.shard:
        k, n = (int(x) for x in args.shard.split("/"))
        mutants = [m for i, m in enumerate(mutants) if i % n == k]
    for m in mutants:
        if args.reached_only and not m["reach_transitive"]:
            continue
        targets = all_h if not args.reached_only else m["reach_transitive"]
        # A mutant may legitimately make a harness slower; three times its green time (and never
        # less than --timeout) is the clock, so a slow-but-honest verdict is not filed as a hang.
        slowest = max((base["harnesses"][h]["secs"] or 0) for h in targets)
        limit = max(args.timeout, int(3 * slowest) + 10)
        path = m["file"]
        original = open(path, encoding="utf-8").read()
        try:
            mutated = apply_diff(m["diff"], original)
        except (ValueError, IndexError) as e:
            out.write(json.dumps({"name": m["name"], "error": f"diff did not apply: {e}"}) + "\n")
            continue
        try:
            open(path, "w", encoding="utf-8").write(mutated)
            r = run_kani(pkg, kargs, targets, limit)
        finally:
            open(path, "w", encoding="utf-8").write(original)
        r["name"] = m["name"]
        r["limit"] = limit
        out.write(json.dumps(r) + "\n")
        out.flush()
        killed = killed_by(r["harnesses"])
        print(f"{'KILLED' if killed else ('UNVIABLE' if not r['compiled'] else 'lived ')} "
              f"{r['wall']:6.1f}s {m['name']}", flush=True)


def census_outcomes(dirs):
    """(file, description) -> list of (line, outcome), from cargo-mutants outcomes.json files."""
    table = {}
    for d in dirs:
        for f in glob.glob(os.path.join(d, "**", "outcomes.json"), recursive=True):
            for o in json.load(open(f))["outcomes"]:
                sc = o.get("scenario")
                if not isinstance(sc, dict) or "Mutant" not in sc:
                    continue
                mu = sc["Mutant"]
                desc = mu["name"].split(": ", 1)[1]
                table.setdefault((mu["file"], desc), []).append(
                    (mu["span"]["start"]["line"], o["summary"]))
    for v in table.values():
        v.sort()
    return table


CENSUS_CLASS = {"CaughtMutant": "caught", "MissedMutant": "missed", "Timeout": "timeout",
                "Unviable": "unviable"}


def report(args):
    pl = json.load(open(args.plan))
    results = {}
    baselines = []
    for f in args.results:
        for line in open(f):
            r = json.loads(line)
            if r.get("baseline"):
                baselines.append(r)
            else:
                results[r["name"]] = r
    census = census_outcomes(args.census or [])
    # Pair this tree's mutants with the census's by (file, description) in line order, because
    # line numbers move between the census's tree and this one and the description does not.
    groups = {}
    for m in pl["mutants"]:
        desc = m["name"].split(": ", 1)[1]
        groups.setdefault((m["file"], desc), []).append(m)
    test_of = {}
    for key, ms in groups.items():
        ms.sort(key=lambda m: m["span"]["start"]["line"])
        cs = census.get(key, [])
        by_line = {line: o for line, o in cs}
        for i, m in enumerate(ms):
            line = m["span"]["start"]["line"]
            if not args.census:
                o = None
            elif line in by_line:
                o = by_line[line]          # same line in both trees
            elif len(cs) == len(ms):
                o = cs[i][1]               # lines moved, pair in order
            else:
                o = None
            test_of[m["name"]] = CENSUS_CLASS.get(o, o) if o else "unknown"
    PROOF_CLASSES = ("killed", "no verdict", "lived")  # no verdict: a timeout or a CBMC out of resources
    TEST_CLASSES = ("caught", "missed", "timeout", "unviable", "unknown")
    cross = {(p, t): 0 for p in PROOF_CLASSES for t in TEST_CLASSES}
    summary = {"unviable under kani": 0, "not run": 0, "kills outside computed reach": 0}
    per_h = {h["path"]: {"direct": [0, 0], "transitive": [0, 0], "outside": 0, "timeouts": 0}
             for h in pl["harnesses"]}
    rows = []
    for m in pl["mutants"]:
        r = results.get(m["name"])
        t = test_of.get(m["name"], "unknown")
        if r is None or "error" in r:
            summary["not run"] += 1
            continue
        if not r["compiled"]:
            summary["unviable under kani"] += 1
            continue
        hs = r["harnesses"]
        killers = killed_by(hs)
        timeouts = sorted(h for h, x in hs.items() if x["timeout"] or x.get("resource"))
        for h in timeouts:
            if h in per_h:
                per_h[h]["timeouts"] += 1
        for h, ph in per_h.items():
            for kind in ("direct", "transitive"):
                if h in m["reach_" + kind]:
                    ph[kind][1] += 1
                    if h in killers:
                        ph[kind][0] += 1
            if h in killers and h not in m["reach_transitive"]:
                ph["outside"] += 1
        if any(h not in m["reach_transitive"] for h in killers):
            summary["kills outside computed reach"] += 1
        pc = "killed" if killers else "no verdict" if timeouts else "lived"
        cross[(pc, t if t in TEST_CLASSES else "unknown")] += 1
        rows.append((m["name"], pc, t, killers, timeouts, bool(m["reach_transitive"])))
    cpu = sum(r["wall"] for r in results.values() if "wall" in r) + sum(b["wall"] for b in baselines)
    with open(args.out, "w") as o:
        o.write(f"# {pl['package']}\n\n")
        o.write(f"mutants planned: {len(pl['mutants'])}; kani wall seconds (sum over runs): {cpu:.0f}\n\n")
        o.write("| proof \\ census | " + " | ".join(TEST_CLASSES) + " |\n")
        o.write("|---" * (len(TEST_CLASSES) + 1) + "|\n")
        for p in PROOF_CLASSES:
            o.write(f"| {p} | " + " | ".join(str(cross[(p, t)]) for t in TEST_CLASSES) + " |\n")
        o.write("\n")
        for k, v in summary.items():
            o.write(f"- {k}: {v}\n")
        o.write("\n| harness | direct killed/total | transitive killed/total | kills outside reach | timeouts |\n")
        o.write("|---|---|---|---|---|\n")
        for h, ph in sorted(per_h.items()):
            o.write(f"| `{h}` | {ph['direct'][0]}/{ph['direct'][1]} | "
                    f"{ph['transitive'][0]}/{ph['transitive'][1]} | {ph['outside']} | {ph['timeouts']} |\n")
        o.write("\n| mutant | in reach | proof | census | killed by | timed out |\n"
                "|---|---|---|---|---|---|\n")
        for name, pc, t, k, to, reached in rows:
            o.write(f"| `{name}` | {'yes' if reached else 'no'} | {pc} | {t} | "
                    f"{', '.join(x.split('::')[-1] for x in k)} | "
                    f"{', '.join(x.split('::')[-1] for x in to)} |\n")
    json.dump({"package": pl["package"], "summary": summary, "per_harness": per_h, "cpu_seconds": cpu,
               "cross": {f"{p}/{t}": v for (p, t), v in cross.items()}},
              open(args.out + ".json", "w"), indent=1)
    print(open(args.out).read().split("\n| mutant |")[0])


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("plan")
    p.add_argument("--package", required=True)
    p.add_argument("--kani-args", default="")
    p.add_argument("--file", action="append")
    p.add_argument("--harness-re", help="keep only harnesses whose module path matches, for a "
                   "package like `kernel` whose harnesses are split by the host's architecture")
    p.add_argument("--out", required=True)
    r = sub.add_parser("run")
    r.add_argument("--plan", required=True)
    r.add_argument("--shard")
    r.add_argument("--timeout", type=int, default=120)
    r.add_argument("--reached-only", action="store_true",
                   help="prove only the harnesses whose computed reach includes the mutant")
    r.add_argument("--out", required=True)
    q = sub.add_parser("report")
    q.add_argument("--plan", required=True)
    q.add_argument("--results", nargs="+", required=True)
    q.add_argument("--census", nargs="*")
    q.add_argument("--out", required=True)
    a = ap.parse_args()
    {"plan": plan, "run": run, "report": report}[a.cmd](a)


if __name__ == "__main__":
    main()
