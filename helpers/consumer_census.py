"""Report what the tree delivered that nothing yet uses. Read-only, and it never fails.

**Why this exists.** The proposal `design/roadmap/proposals/delivered-work-names-its-consumer.md`
records the architect's ruling of 2026-10-31 (UTC): a lane brief and a review ask what drives the
work and what it saves, and this census runs report-only, by hand, monthly, to count how many
blocks are BUILT, how many already name something a consumer could be read from, how many crates
nothing depends on, and how many base-image programs nothing starts or uses. It is wired into no
gate and exits 0 whatever it finds. A count of unused things cannot tell work not yet consumed from
work built with no reason, so a finding is a question for a person.

    python3 helpers/consumer_census.py           # the counts
    python3 helpers/consumer_census.py --list    # the counts, then every item behind each count

**What each section reads, and what it cannot see.**

- Roadmap blocks: `design/roadmap/NNNN-*.md` frontmatter through `helpers/roadmap_block.py`. A
  block's *dependents* are the blocks whose `milestone_dependencies` name it, which is the only
  machine-readable consumer edge the schema has. BUILT blocks carry no dependency fields of their
  own, so the edge is visible only from a newer block that depends on an older one. The looser
  counts (another block's prose cites it; its own `## Follow-on` names a `**Milestone N.**`) are
  candidates a person confirms, not consumers.
- Crates: every `Cargo.toml` outside `vendor/`, `patches/` and `target/`, read with `tomllib`.
  An edge is a `path` dependency (normal, build or dev, any target table). A crate with only
  dev-dependents is used by tests alone. A crate with a `[[bin]]` or `src/main.rs` is a program
  and is reported apart, since an image, not a dependent, consumes it.
- Base-image programs: `packages/base-image.toml`. A program is *named* when code outside its
  own files spells it as a string literal (`"name"`, `b"name"`, `"name ` or `"name-`) or as
  `Program::Variant` from `crates/grant_plan/src/program/catalog.rs`. It is *reached* when code
  that is neither a program nor a test names it, or when a reached program does. A reference under
  a test directory, `xtask/src/swish_check/`, `script/`, `fuzz/` or `.github/` is a test's. Declarations do not count: Cargo files, `packages/`, the catalog and its manifests,
  markdown and bench logs. A program a person types at the prompt has no starter by design, so a
  zero here is a question, not a finding.

BUGS.

- A crate consumed by generated code reads as having no dependents. `allocation_cache` is the one
  today: `cargo xtask std-src` copies it into `std`, and no `Cargo.toml` names it.
- A program started by a name built at run time, rather than spelled as a literal, reads as named by
  nothing. None does today; the literal is how `crates/system_initializer` measures what it starts.

Name: provisional, minted 2026-10-31 (UTC) by the lane proposal/delivered-work-names-its-consumer.
`census` follows `helpers/name_census.py` and `helpers/mutation_kernel_census.py`, which also count
a whole tree. No architect has named it.
"""
import os
import re
import subprocess
import sys
import tomllib

sys.dont_write_bytecode = True
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, os.path.join(ROOT, "helpers"))
import roadmap_block  # noqa: E402

LIST = "--list" in sys.argv[1:]


def show(title, items):
    if LIST and items:
        print(f"  {title}:")
        for item in items:
            print(f"    {item}")


def git_files(*pathspec):
    out = subprocess.run(["git", "-C", ROOT, "ls-files", "--", *pathspec],
                         capture_output=True, text=True).stdout
    return [p for p in out.split("\n") if p]


# ---- roadmap blocks ------------------------------------------------------------------------------
def roadmap():
    d = os.path.join(ROOT, "design", "roadmap")
    blocks = {}
    for name in sorted(os.listdir(d)):
        m = re.fullmatch(r"(\d{4})-.*\.md", name)
        if not m:
            continue
        text = open(os.path.join(d, name)).read()
        fields, _, _ = roadmap_block.frontmatter(text.split("\n"))
        blocks[int(m.group(1))] = (name, fields or {}, text)
    status = {n: f.get("status", "?") for n, (_, f, _) in blocks.items()}
    dependents, cited_by, follow = {}, {}, {}
    cite = re.compile(r"\b[Mm]ilestones? (\d+)\b")
    for n, (_, f, text) in blocks.items():
        for tok in roadmap_block.tokens(f.get("milestone_dependencies")):
            if tok.isdigit() and int(tok) in blocks:
                dependents.setdefault(int(tok), set()).add(n)
        for c in {int(c) for c in cite.findall(text)}:
            if c != n and c in blocks:
                cited_by.setdefault(c, set()).add(n)
        for line in text.split("\n"):
            dm = roadmap_block.DISPOSITION.match(line)
            if dm and dm.group(2) and int(dm.group(2)) != n:
                follow.setdefault(n, set()).add(int(dm.group(2)))
    counts = {}
    for s in status.values():
        counts[s] = counts.get(s, 0) + 1
    built = sorted(n for n, s in status.items() if s == "BUILT")
    dep_any = [n for n in built if dependents.get(n)]
    dep_built = [n for n in built if any(status[m] == "BUILT" for m in dependents.get(n, ()))]
    loose = [n for n in built if dependents.get(n) or cited_by.get(n) or follow.get(n)]
    nothing = [n for n in built if n not in loose]
    unbuilt = [n for n, s in status.items() if s in ("NOT-STARTED", "PARTIAL", "IN-PROGRESS")]
    unbuilt_dep = [n for n in unbuilt if dependents.get(n)]
    print("roadmap blocks")
    print(f"  {len(blocks)} blocks: " + ", ".join(f"{k} {v}" for k, v in
                                                  sorted(counts.items(), key=lambda kv: -kv[1])))
    print(f"  BUILT: {len(built)}")
    print(f"    named in another block's milestone_dependencies: {len(dep_any)}"
          f" (by a BUILT block: {len(dep_built)})")
    print(f"    any candidate (a dependent, a prose citation from another block, or its own "
          f"Follow-on names a milestone): {len(loose)}")
    print(f"    no candidate at all: {len(nothing)}")
    print(f"  NOT-STARTED, PARTIAL or IN-PROGRESS: {len(unbuilt)};"
          f" named in another block's milestone_dependencies: {len(unbuilt_dep)}")
    show("BUILT with no candidate", [blocks[n][0] for n in nothing])


# ---- crates --------------------------------------------------------------------------------------
def dep_tables(doc):
    for key in ("dependencies", "build-dependencies", "dev-dependencies"):
        yield key, doc.get(key, {})
    for tdoc in doc.get("target", {}).values():
        for key in ("dependencies", "build-dependencies", "dev-dependencies"):
            yield key, tdoc.get(key, {})


def crates():
    manifests = [p for p in git_files("*Cargo.toml")
                 if not p.startswith(("vendor/", "patches/", "target/"))]
    pkg = {}  # crate directory -> (name, is_program, has_lib)
    deps = []  # (from dir, to dir, kind)
    for rel in manifests:
        path = os.path.join(ROOT, rel)
        doc = tomllib.load(open(path, "rb"))
        here = os.path.dirname(path)
        if "package" in doc:
            src = os.path.join(here, "src")
            program = bool(doc.get("bin")) or os.path.exists(os.path.join(src, "main.rs")) or \
                os.path.isdir(os.path.join(src, "bin"))
            lib = "lib" in doc or os.path.exists(os.path.join(src, "lib.rs"))
            pkg[here] = (doc["package"]["name"], program, lib)
        for kind, table in dep_tables(doc):
            for spec in table.values():
                if isinstance(spec, dict) and "path" in spec:
                    deps.append((here, os.path.normpath(os.path.join(here, spec["path"])), kind))
    users = {d: {"normal": set(), "dev": set()} for d in pkg}
    for src, dst, kind in deps:
        if dst in users and src != dst:
            users[dst]["dev" if kind == "dev-dependencies" else "normal"].add(src)
    libs = {d: v for d, v in pkg.items() if v[2]}
    none = sorted(os.path.relpath(d, ROOT) for d in libs
                  if not users[d]["normal"] and not users[d]["dev"] and not pkg[d][1])
    dev_only = sorted(os.path.relpath(d, ROOT) for d in libs
                      if not users[d]["normal"] and users[d]["dev"] and not pkg[d][1])
    programs = sorted(os.path.relpath(d, ROOT) for d in pkg if pkg[d][1])
    print("crates (outside vendor/ and patches/)")
    print(f"  {len(pkg)} packages; {len(libs)} carry a library, {len(programs)} carry a program")
    print(f"  library-only crates nothing depends on: {len(none)}")
    print(f"  library-only crates only tests depend on (dev-dependencies alone): {len(dev_only)}")
    show("nothing depends on", none)
    show("only dev-dependents", dev_only)


# ---- base-image programs -------------------------------------------------------------------------
TESTISH = ("system_tests/", "xtask/src/swish_check/", "script/", "fuzz/", ".github/", "fixtures/")
DECLARES = ("packages/", "crates/grant_plan/src/program/", "bench/", "design/", "notes/",
            "vendor/", "patches/")


def programs():
    image = tomllib.load(open(os.path.join(ROOT, "packages", "base-image.toml"), "rb"))
    names = image["programs"]
    catalog = open(os.path.join(ROOT, "crates/grant_plan/src/program/catalog.rs")).read()
    variant = {name: v for v, name in re.findall(r"(\w+) \{ id: \d+, name: \"(\w+)\" \}", catalog)}
    files = [p for p in git_files("*.rs", "*.py", "*.sh", "*.yml", "*.conf", "script/*")
             if not p.startswith(DECLARES) and not p.endswith((".md", "Cargo.toml", "Cargo.lock"))]
    text = {}
    for p in files:
        try:
            text[p] = open(os.path.join(ROOT, p), errors="replace").read()
        except (IsADirectoryError, FileNotFoundError):
            pass
    def own(name):
        return (f"components/src/{name}.rs", f"components/src/{name}/", f"crates/{name}/",
                f"{name}/")

    def is_test(p):
        return p.startswith(TESTISH) or "/tests" in p or p.endswith("tests.rs")

    # Who names each program: another program (by the program's own files), plain code that is
    # neither a program nor a test (the kernel, `crates/system_initializer`, xtask), or a test.
    by_program, by_code, by_test = {}, set(), set()
    for name in names:
        pat = r'b?"' + re.escape(name) + r'(?:"|[ -])'
        if name in variant:
            pat += r"|\bProgram::" + variant[name] + r"\b"
        rx = re.compile(pat)
        for p, t in text.items():
            if p.startswith(own(name)) or not (name in t or variant.get(name, name) in t):
                continue
            if not rx.search(t):
                continue
            starter = next((q for q in names if q != name and p.startswith(own(q))), None)
            if starter:
                by_program.setdefault(name, set()).add(starter)
            elif is_test(p):
                by_test.add(name)
            else:
                by_code.add(name)
    # A program counts as started when plain code names it, or when a started program does. One
    # started only by a program that only a test starts is reached in tests alone.
    started = set(by_code)
    while True:
        more = {n for n, qs in by_program.items() if n not in started and qs & started}
        if not more:
            break
        started |= more
    tested = [n for n in names if n not in started and (n in by_test or n in by_program)]
    unused = [n for n in names if n not in started and n not in tested]
    print("base-image programs (packages/base-image.toml)")
    print(f"  {len(names)} programs; {len(started)} reached from code outside the tests,"
          f" {len(tested)} reached only from tests, {len(unused)} named by nothing")
    show("reached only from tests", tested)
    show("named by nothing", unused)


roadmap()
crates()
programs()
