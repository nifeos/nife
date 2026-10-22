#!/usr/bin/env python3
"""Is this diff prose that no build and no test reads? If so CI skips the heavy jobs.

    helpers/prose_only.py classify <base>   # the CI gate: <base>..HEAD, one line per changed file
    helpers/prose_only.py --selftest        # script/lint: fixtures, no git

Host-only tools (lane/ci-scope-host-scripts, 2026-10-07 UTC): the output is still called `prose`
and the skip still means "no build or test reads this", because renaming the output touches every
heavy job's `if:`. A second class rides it, see HOST_ONLY_TOOLS below.

Name: provisional, minted by lane/docs-only-ci on 2026-10-05 UTC. A helper under `helpers/`,
outside `script/names`' scope, so its provenance is this paragraph. Refused `docs_only`, because
"docs" also covers a crate's README and rustdoc, which are code here, and this decides prose only.

# WHY

Every pull request ran the whole suite, Markdown or not. On 2026-10-05 twelve pull requests that
only removed `**` markers from design/ and notes/ took the hosted runner pool, and required jobs on
#1708 and #1712 were cancelled with "The job was not acquired by Runner of type hosted even after
multiple attempts". Before this file each heavy job decided "documentation only" for itself, nine
copies of one regex inside steps, so every one of them still took a runner to find out it had
nothing to do. The gate now decides once, and a prose-only change skips those jobs outright.

# THE RULE

A changed file is **prose** when it ends in `.md` and lies under `notes/`, `design/` or `briefs/`,
or at the repository root, AND nothing in the tree at the head being tested reads it. A diff is
prose-only when it is non-empty and every file in it is prose, or comment-only Rust (COMMENT-ONLY
RUST below). Anything else runs everything.

"Nothing reads it" is computed from the tree on every run, never from a list. A list of consumers
somebody has to remember to extend is rung three at best and would rot the first time a lane added
an `include_str!` (calef, 2026-10-05). Every tracked file that could read something is scanned:
everything except Markdown, the prose directories themselves (their `.csv`, `.svg` and `.patch`
files are records), captured data (`.log`, `.csv`, `.tsv`) and `vendor/`. Comment lines are dropped
(`//`, `/*` or `*` anywhere, `#` outside Rust, where it opens an attribute; a `.patch` contributes
the lines it leaves in the tree). Two shapes count:

- **A file reference**: a `*.md` token used as a path. It is resolved repository-relative, relative
  to the referring file, with leading `../` stripped, and from any `notes/`, `design/` or `briefs/`
  inside it (`$root/notes/x.md`), and a changed file matching any of these is an input. This is
  what catches `include_str!`, `#[doc = include_str!(...)]`, a build script's read, xtask's
  `DOC_BUNDLES` (the notes the disk image installs and swish-check searches with `apropos`), a
  Cargo.toml `readme`, and a workflow that cats a note. A token inside a quoted string that holds
  whitespace is a sentence (`"see notes/stack.md"`) and is not counted: a message reads nothing.
  Anything the check cannot place, such as the middle line of a long Rust string, is counted, so
  it errs toward a full run.
- **A directory reference**: a quoted string that is exactly a prose directory (`"notes"`,
  `"design/roadmap"`) or a glob under one (`"design/roadmap/*.md"`). Code that names a directory
  walks it, so everything beneath it is read by the package that holds the code. In a `.rs` file
  the package is the nearest Cargo.toml's, and a prose-only run tests it (`prose_tests`): today that
  is `crates/documentation`'s corpus test, which went red on main on 2026-09-23 while the old
  predicate skipped it, and xtask. A package that cannot test on the host fails that step loudly.
  Rust in no package runs everything.

Why not the compiler's own record (cargo's dep-info, `target/**/*.d`)? It would be exact for
`include_str!`, but it exists only after a build, and the gate's purpose is to decide before any
build takes a runner. CI uploads no dep-info a gate could read. And dep-info cannot see a runtime
read, which is the larger half (DOC_BUNDLES, the corpus walk). The grep of the head tree is the
honest fallback: it errs toward running and it sees a new reference in the same diff that adds it.

The decision for every changed file is printed with the rule that made it, so a wrong skip can be
diagnosed from the gate's log.

# HOST-ONLY TOOLS

#1831 changed `notes/project-metrics.md` and `script/metrics` (a Python report generator) and ran
the whole kernel suite, because `script/metrics` is not Markdown. Nothing a gated job runs reads
it: the only caller is `.github/workflows/metrics.yml` (weekly, not a pull request job), and
`script/lint` runs the selftests of the helpers beside it. So a file named in HOST_ONLY_TOOLS counts as skippable
when no tree file outside `script/`, `helpers/` and `.github/workflows/metrics.yml` names it on a
non-comment line (by its path, or for a Python helper by its module name). Unlike the prose
rule this is a short list somebody had to verify, because `script/` and `helpers/` files name each
other in docstrings everywhere and a derived closure over them was measured on 2026-10-07 to
call all but a few of 159 files reachable; the check above is the fail-safe half, and the list is
the exception, written down as one. Unlisted paths (every other `script/` and `helpers/` file, the
workflow itself) run everything. The tools and metrics.yml also stop counting as readers of
prose, which is what lets #1831's note edit skip (the page names itself in both). `script/metrics --selftest` has no gate of its own, so the
`lint` job, which never skips, runs it.

# HOST-ONLY OUTPUTS

#1878 (the 2026W41 metrics update, 2026-10-09 UTC) changed the page and thirteen generated records
under `notes/project-metrics/` and ran the kernel suite, because `.csv` and `.svg` records are not
Markdown and `baseline-drift.md` is named by `script/citations`. The records are written by the
host-only tools above, so they ride the same skip: HOST_ONLY_OUTPUTS lists the one directory those
tools write, and a changed file under it skips when no scanned file outside the tools, their runner
and `.githooks/` names it on a non-comment line.

Unlike the tools rule this check does NOT forgive namers under `script/` and `helpers/`. A gated
script naming a data path on a code line is a read until proven otherwise:
`helpers/verify_times.py` names `falsification-times.tsv` and `verify-runs.csv` precisely because
`script/verify` shards by them (that is why it is not a host-only tool), and those files still run
everything. The one exception is a pathspec exclusion: `script/citations` names
`baseline-drift.md` only inside `:!`, which names a path to not read it, and lint runs citations on
every pull request besides, so a `:!`-prefixed naming does not count. The namers outside the
forgiven set on 2026-10-09 were `helpers/verify_times.py` (the two verify tables, gated) and
`falsifications.yml`, `verify-shard-refresh.yml` and `mutation.yml` (scheduled, none a pull-request
job), each naming its own files, so every record a gated job reads keeps running everything.

# COMMENT-ONLY RUST

Milestone 929 pays comments down across the tree in pull requests that change nothing else, and
each one ran the full suite. A changed `.rs` file is **comment-only** when its tokens at the base
and at the head are equal once every comment is removed, doc comments included. That is decided by
lexing (`rust_code_tokens`), not by a pattern on `//`: a string, raw string, byte string or char
literal is one token, so a `//` or `*/` inside one is code, and a block comment ends where its
nesting does. Whitespace outside literals is not a token, so a reindent rides the class too, and
the `rustfmt` job judges it. A comment-only file is skippable when all of these hold:

- It existed at the base and still does (an added, deleted or renamed file runs everything), it is
  outside `vendor/`, and its package is a member of the root workspace, the one the lint job tests.
- No tracked patch rewrites it. A falsification record is a patch, and a comment edit in its
  context lines stops it applying, which only the re-falsify shards would see.
- Nothing that could read its text names it on a code line or walks a directory above it (a quoted
  path with a slash, `"kernel/src"`). Rust in a root-workspace package that does is owed its
  package's host tests; Rust elsewhere runs everything. Manifests and assembly (WIRING), a
  `#[path]` line, a path inside a sentence, and a workflow that no pull request or merge group
  runs are not readers. A script under `script/` or `helpers/` is printed and not acted on, the
  way a directory read outside Rust is for prose, because lint runs them on every pull request.

A diff of prose and skippable comment-only Rust skips the heavy jobs like prose and sets
`comments=true`, so the `rustfmt` job still runs. `comment_tests` names the owed packages (each
file's own and its readers'). The `lint` job, which never skips, runs clippy and rustdoc as always,
and then `cargo test` for each owed package that builds on the host. That covers the doctests,
since doc comments hold them, and `missing_docs`, which clippy reads. `kernel` and `system_tests`
build only for bare metal, and their doc comments reach no host test, so for them clippy and
rustdoc are the whole check.

# BUGS

- **A comment-only Rust change shifts line numbers.** Panic locations and `file!()`/`line!()` move
  with it, and a test that pinned one would be skipped; no test in the tree reads `line!()`.
- **A script under `script/` or `helpers/` that reads a Rust file's comments and runs only in a
  skipped job is not seen.** `script/falsifications` reads `/// Falsification:` records, and the
  lint job runs its count. A new script that a QEMU job runs and that parses comments would slip
  through.
- **A Rust file a falsification patch rewrites runs everything** even when the edit is far from the
  patch's context (39 of 259 kernel files, 82 of 499 under `crates/` on 2026-10-22 UTC). A lint
  check that every patch still applies would let them skip.
- **HOST_ONLY_TOOLS is a hand-kept list, and a script that begins to call a listed tool is not
  seen.** The root check covers callers outside `script/` and `helpers/` only. A new `script/x`
  that runs `script/metrics` and is run by a gated job would let a metrics-only change skip a job
  that now depends on it. None does on 2026-10-07; nothing checks that none does later.
- **HOST_ONLY_OUTPUTS forgives exact-path namings only.** A file under the prefix that nothing
  names by its full path skips: `mutation-survivors/`'s census copies are named by `mutation.yml`
  only through the directory (`git add notes/project-metrics`), so they skip; nothing a pull
  request job runs reads them, and the pull requests that carry them are opened with GITHUB_TOKEN,
  which starts no CI at all. A reference through a runtime-built path or a bare basename is
  equally invisible. A second host-only data directory needs its own row in the list.
- **A directory read outside Rust is logged, not acted on.** A script that walks `design/` (lint's
  roadmap, citations and decisions checks, `helpers/prose_ratchet.py`) is printed as a reader and
  does not stop the skip, because lint runs on every pull request and skipping nothing would be the
  alternative. A script that walks a prose directory and is run *only* by a job the prose path
  skips would slip through. None did on 2026-10-05; nothing checks that none does later.
- **A path built at run time from parts that are not a literal** (an environment variable, a
  `format!` whose pieces are neither a `.md` token nor a quoted prose directory) is invisible. So
  is a reader of the repository root that names it as `""` or `"."`: xtask's `apropos` does, and
  it runs in no gate.
- **A path inside a sentence-shaped string is not counted**, by the rule above, so a shell loop over
  `"notes/a.md notes/b.md"` would read both unseen. Nothing in the tree does that today.
- A script under `notes/` (two `.py` files in `notes/capability-peak-trace/`) is a record and is not
  scanned; if a gate ever ran one, what it reads would be invisible.
- `vendor/` is not scanned. Upstream code names no file of ours.
"""

import os
import re
import subprocess
import sys

PROSE_DIRS = ("notes/", "design/", "briefs/")

# This file names prose paths in its fixtures and its rule; it is not a reader of them.
SELF = "helpers/prose_only.py"

MD_TOKEN = re.compile(r"[A-Za-z0-9_./-]*[A-Za-z0-9_-]\.md\b")
DIR_LITERAL = re.compile(
    r"""(["'])(?:\.\./)*(?:\./)?/?((?:notes|design|briefs)(?:/[A-Za-z0-9_-]+)*)(?:/[^"'\s]*\*[^"'\s]*)?/?\1""")
QUOTED = re.compile(r""""(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'""")


def path_tokens(line, token=MD_TOKEN):
    """The .md tokens on a line that are used as paths rather than mentioned in a sentence.

    A token inside a quoted string that contains whitespace is a message ("see notes/stack.md"),
    and a message reads nothing. Every other token counts: a string that is only a path is data
    (`"notes/pipes.md"`, `include_str!("../x.md")`, `format!("{}/notes/x.md", root)`), and an
    unquoted one is a shell word or a line inside a string this check cannot see the start of,
    both of which are kept, because a wrong keep costs minutes and a wrong skip a red main."""
    sentences = [m.span() for m in QUOTED.finditer(line) if re.search(r"\s", m.group(0))]
    for m in token.finditer(line):
        if not any(a < m.start() and m.end() < b for a, b in sentences):
            yield m.group(0)


# Host-side report generators that no pull-request job runs. Each was checked on 2026-10-07 (UTC)
# against every file outside script/ and helpers/: script/metrics and week_digest.py are named only
# by metrics.yml; baseline_drift.py and interface_stability.py are imported by script/metrics and
# selftested by script/lint (which the `lint` job runs on every pull request). helpers/verify_times.py
# looks like the same class and is not: script/verify shards by it.
HOST_ONLY_TOOLS = (
    "script/metrics",
    "helpers/baseline_drift.py",
    "helpers/interface_stability.py",
    "helpers/week_digest.py",
)
# The one workflow that runs them, and so not a reader that makes them inputs.
HOST_TOOL_RUNNER = ".github/workflows/metrics.yml"

# The directory those tools write, generated records rather than prose: the weekly page's tables
# and charts. A changed file under it skips on the HOST-ONLY OUTPUTS rule in the docstring, which
# unlike the tools rule counts namers under script/ and helpers/ (script/verify reads two of these
# files) and forgives only the tools, their runner, hooks and `:!` pathspec exclusions.
HOST_ONLY_OUTPUTS = ("notes/project-metrics/",)


def host_tool_readers(tool, tree):
    """Files outside script/, helpers/, hooks and the runner that name `tool` on a code line."""
    stem = os.path.basename(tool)
    pats = [re.compile(re.escape(tool) + r"(?![A-Za-z0-9_-])")]
    if tool.endswith(".py"):
        pats.append(re.compile(r"(?<![A-Za-z0-9_/.-])" + re.escape(stem[:-3]) + r"(?![A-Za-z0-9_-])"))
    out = []
    for path, text in tree.items():
        if text is None or path.startswith(("script/", "helpers/", ".githooks/")) \
                or path == HOST_TOOL_RUNNER or not scanned(path):
            continue
        for n, line in code_lines(path, text):
            if any(p.search(line) for p in pats):
                out.append(f"{path}:{n}")
                break
    return out


def host_output_readers(path, tree):
    """Scanned files that name a generated record on a code line and are not forgiven for it.

    The forgiven are the host-only tools (they write the records), their runner, hooks, and the
    `:!` pathspec exclusion, the one shape that names a path to not read it (script/citations
    excludes baseline-drift.md). Everything else counts, script/ and helpers/ included, because
    helpers/verify_times.py names the tsv script/verify shards by and is exactly a gated reader."""
    pat = re.compile(r"(?<!:!)" + re.escape(path) + r"(?![A-Za-z0-9_-])")
    out = []
    for p, text in tree.items():
        if text is None or not scanned(p) or p.startswith(".githooks/"):
            continue
        for n, line in code_lines(p, text):
            if pat.search(line):
                out.append(f"{p}:{n}")
                break
    return out


def is_prose_path(path):
    return path.endswith(".md") and (path.startswith(PROSE_DIRS) or "/" not in path)


# Captured output and tables: read by code, never a reader of anything.
DATA = (".log", ".csv", ".tsv")


def scanned(path):
    """Is this file one whose text could read a prose file? Not prose, not data, not upstream.

    Nor a host-only tool or the workflow that runs it: `script/metrics` reads and rewrites
    notes/project-metrics.md and metrics.yml commits it, and neither runs in a pull request job, so
    naming that page does not make it an input to any gated job."""
    return not path.startswith(PROSE_DIRS) and not path.endswith(".md") \
        and not path.endswith(DATA) and not path.startswith("vendor/") and path != SELF \
        and path not in HOST_ONLY_TOOLS and path != HOST_TOOL_RUNNER


def code_lines(path, text):
    """(line number, line) for every line that is not a whole-line comment.

    A comment is a line opening with `//`, `/*` or `*` anywhere, or `#` outside Rust, where `#`
    opens an attribute such as `#![doc = include_str!(...)]`. A patch (the falsification records,
    applied to Rust) contributes its context and added lines, judged as Rust; a removed line is
    gone from the tree the patch produces."""
    patch = path.endswith((".patch", ".diff"))
    rust = path.endswith(".rs") or patch
    for n, line in enumerate(text.splitlines(), 1):
        if patch:
            if line.startswith(("+++", "---", "@@", "diff ", "index ")) or not line[:1] in ("+", " "):
                continue
            line = line[1:]
        s = line.lstrip()
        if s.startswith(("//", "/*", "*")):
            continue
        if s.startswith("#") and not rust:
            continue
        yield n, line


def resolutions(referrer, token):
    out = set()
    t = token
    while t.startswith("./"):
        t = t[2:]
    out.add(os.path.normpath(t.lstrip("/")))
    out.add(os.path.normpath(os.path.join(os.path.dirname(referrer), token)))
    bare = t
    while bare.startswith("../"):
        bare = bare[3:]
    out.add(os.path.normpath(bare.lstrip("/")))
    # A path under a prefix the text cannot evaluate (`$root/notes/x.md`, `{}/design/y.md`).
    for m in re.finditer(r"(?:^|/)((?:notes|design|briefs)/.*)$", t):
        out.add(os.path.normpath(m.group(1)))
    return out


def package_of(path, tree):
    """The package name of the nearest Cargo.toml above `path`, or None."""
    d = os.path.dirname(path)
    while True:
        manifest = os.path.join(d, "Cargo.toml") if d else "Cargo.toml"
        text = tree.get(manifest)
        if text is not None:
            m = re.search(r'^\[package\][^\[]*?^name\s*=\s*"([^"]+)"', text, re.M | re.S)
            # A name is spliced into a `cargo test -p` line in CI, so anything but a plain
            # package name is refused here and its reader treated as in no package (run all).
            if m and re.fullmatch(r"[A-Za-z0-9_-]+", m.group(1)):
                return m.group(1)
            if m:
                return None
        if not d:
            return None
        d = os.path.dirname(d)


def scan(tree):
    """Every reference to a prose path in code: (files, dirs).

    files maps a resolved .md path to the places naming it; dirs maps a prose directory to
    (place, package-or-None) for each place that names it as a quoted literal."""
    files, dirs = {}, {}
    for path, text in tree.items():
        if not scanned(path) or text is None:
            continue
        for n, line in code_lines(path, text):
            for tok in path_tokens(line):
                for r in resolutions(path, tok):
                    files.setdefault(r, []).append(f"{path}:{n}")
            for m in DIR_LITERAL.finditer(line):
                pkg = package_of(path, tree) if path.endswith(".rs") else None
                dirs.setdefault(m.group(2), []).append((f"{path}:{n}", pkg))
    return files, dirs


# The comment-only Rust class (milestone 929): see COMMENT-ONLY RUST in the header.

RS_TOKEN = re.compile(r"[A-Za-z0-9_./-]*[A-Za-z0-9_-]\.rs\b")
PATH_LITERAL = re.compile(r"""(["'])(?:\.\./)*(?:\./)?([A-Za-z0-9_][A-Za-z0-9_.-]*(?:/[A-Za-z0-9_.-]+)+)/?\1""")
PATCH_TARGET = re.compile(r"^(?:\+\+\+ b/|--- a/)(\S+)", re.M)
# `#[path = "x.rs"] mod x;` compiles the file; it does not read it as text.
MODULE_PATH = re.compile(r"\s*#\[path\s*=")
# Namers that wire a file into a build or a mutation run without reading its text: manifests, and
# assembly, whose trailing `//` comments the whole-line comment filter does not drop.
WIRING = (".toml", ".s", ".S")
# Namers that script/lint or the lint job run, on every pull request: logged, as a directory read
# outside Rust is for prose. See COMMENT-ONLY RUST, and its BUGS.
LINT_SIDE = ("script/", "helpers/", ".githooks/")


def rust_code_tokens(src):
    """The tokens of a Rust source with every comment removed, doc comments included.

    Two texts with equal token lists differ only in comments and whitespace outside literals. It is
    a lexer, not a regex on `//`: a string, raw string, byte string or char literal is one token
    whatever it holds, so `"// x"` changing is a code change, and a block comment ends at the `*/`
    that closes its nesting depth."""
    out = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c in " \t\r\n":
            i += 1
            continue
        if src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
            continue
        if src.startswith("/*", i):
            depth, j = 0, i
            while j < n:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                    if depth == 0:
                        break
                else:
                    j += 1
            i = j
            continue
        ident_before = i > 0 and (src[i - 1].isalnum() or src[i - 1] == "_")
        m = None if ident_before else re.match(r'(?:b|c)?r(#*)"', src[i:i + 300])
        if m:
            end = '"' + m.group(1)
            j = src.find(end, i + m.end())
            j = n if j < 0 else j + len(end)
            out.append(src[i:j])
            i = j
            continue
        if c == '"' or (c in "bc" and src[i + 1:i + 2] == '"' and not ident_before):
            j = i + (1 if c == '"' else 2)
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append(src[i:j + 1])
            i = j + 1
            continue
        if c == "'" or (c == "b" and src[i + 1:i + 2] == "'" and not ident_before):
            k = i + (1 if c == "'" else 2)
            m = re.match(r"(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^\\'\n])'", src[k:k + 16])
            if m:
                out.append(src[i:k + m.end()])
                i = k + m.end()
                continue
        m = re.match(r"'?[A-Za-z_][A-Za-z0-9_]*|[0-9][0-9A-Za-z_.]*", src[i:i + 200])
        if m:
            out.append(m.group(0))
            i += m.end()
            continue
        out.append(c)
        i += 1
    return out


def rust_readers(tree):
    """What in the tree could notice a comment in a Rust file: (named, walked, patched).

    named maps a resolved `.rs` path to the places naming it on a code line; walked maps a quoted
    directory literal (one with a slash, `"kernel/src"`) to (place, package-or-None) for each place
    naming it; patched is every path a tracked patch rewrites, whose context lines a comment edit
    can break."""
    named, walked, patched = {}, {}, set()
    for path, text in tree.items():
        if not scanned(path) or text is None:
            continue
        if path.endswith((".patch", ".diff")):
            patched.update(PATCH_TARGET.findall(text))
        pkg = package_of(path, tree) if path.endswith(".rs") else None
        for n, line in code_lines(path, text):
            if MODULE_PATH.match(line):
                continue
            for tok in path_tokens(line, RS_TOKEN):
                for r in resolutions(path, tok):
                    named.setdefault(r, []).append((f"{path}:{n}", pkg, path))
            for m in PATH_LITERAL.finditer(line):
                d = m.group(2).rstrip("/")
                if d.endswith(".rs"):
                    continue
                walked.setdefault(d, []).append((f"{path}:{n}", pkg, path))
    return named, walked, patched


def workspace_members(tree):
    """The root workspace's member directories, read from its `members` list."""
    m = re.search(r"^members\s*=\s*\[(.*?)^\]", tree.get("Cargo.toml") or "", re.M | re.S)
    return set(re.findall(r'"([^"]+)"', re.sub(r"#.*", "", m.group(1)))) if m else set()


def manifest_dir(path, tree):
    """The directory of the nearest Cargo.toml above `path` ("" for the root), or None."""
    d = os.path.dirname(path)
    while True:
        if (os.path.join(d, "Cargo.toml") if d else "Cargo.toml") in tree:
            return d
        if not d:
            return None
        d = os.path.dirname(d)


def gated_workflow(text):
    """Does a workflow run on a pull request or a merge group? Only its `on:` block is read, so a
    job condition naming `pull_request` does not count."""
    m = re.search(r"^on:(.*(?:\n(?:[ \t#].*|))*)", text, re.M)
    return bool(m and re.search(r"\b(?:pull_request|pull_request_target|merge_group)\b",
                                re.sub(r"#.*", "", m.group(1))))


def comment_only_rust(f, tree, before, readers):
    """(skippable, reason, packages owed a test run) for one changed `.rs` file."""
    after = tree.get(f)
    if f.startswith("vendor/"):
        return False, "vendored upstream code", ()
    if before is None or after is None:
        return False, "added, deleted or renamed Rust", ()
    if rust_code_tokens(before) != rust_code_tokens(after):
        return False, "Rust whose code changed, not only its comments", ()
    pkg = package_of(f, tree)
    if pkg is None:
        return False, "comment-only Rust in no package, so nothing can test it", ()
    if manifest_dir(f, tree) not in workspace_members(tree):
        return False, "comment-only Rust outside the root workspace, where the lint job tests nothing", ()
    named, walked, patched = readers
    if f in patched:
        return False, "comment-only Rust that a falsification patch rewrites; its context may no longer apply", ()
    owed, logged = {pkg}, set()
    hits = [h for h in named.get(f, []) if h[2] != f]
    hits += [h for d, hs in walked.items() if f.startswith(d + "/") for h in hs]
    for place, rpkg, rpath in sorted(set(hits)):
        if rpath.endswith(".rs"):
            if rpkg is None or manifest_dir(rpath, tree) not in workspace_members(tree):
                return False, f"comment-only Rust read by Rust outside the root workspace ({place})", ()
            owed.add(rpkg)
        elif rpath.endswith(WIRING):
            continue
        elif rpath.startswith(LINT_SIDE):
            logged.add(place)
        elif rpath.startswith(".github/workflows/") and not gated_workflow(tree.get(rpath) or ""):
            continue
        else:
            return False, f"comment-only Rust named by {place}", ()
    note = f"; named by scripts lint runs: {' '.join(sorted(logged)[:3])}" if logged else ""
    return True, f"comment-only Rust; owes fmt, lint and the host tests of {' '.join(sorted(owed))}{note}", owed


def classify(changed, tree, before=None):
    """(prose_only, packages to test, report lines, comment tests). Pure: no git, no disk.

    `before` maps each changed `.rs` path to its text at the base, None where it did not exist.
    `comment tests` is None unless the diff skips and at least one changed file is comment-only
    Rust; then it is the packages whose host tests that Rust owes."""
    report = []
    if not changed:
        report.append("no changed files: a scope that failed to resolve runs everything")
        return False, [], report, None
    files, dirs = scan(tree)
    prose_only = True
    tests, unacted = set(), set()
    readers, comment_tests = None, None
    for f in changed:
        if f.endswith(".rs"):
            if readers is None:
                readers = rust_readers(tree)
            ok, why, owed = comment_only_rust(f, tree, (before or {}).get(f), readers)
            report.append(f"{'SKIP' if ok else 'RUN '}  {f}: {why}")
            if ok:
                comment_tests = (comment_tests or set()) | set(owed)
            else:
                prose_only = False
            continue
        if f in HOST_ONLY_TOOLS:
            readers = host_tool_readers(f, tree)
            if readers:
                report.append(f"RUN   {f}: a host-only tool, but named by {', '.join(readers[:3])}")
                prose_only = False
            else:
                report.append(f"SKIP  {f}: host-only tool; only metrics.yml runs it and the lint job selftests it")
            continue
        if f.startswith(HOST_ONLY_OUTPUTS):
            namers = host_output_readers(f, tree)
            if namers:
                report.append(f"RUN   {f}: a generated record, but named by {', '.join(namers[:3])}")
                prose_only = False
                continue
            reason = "generated record; the metrics tools write it and no gated file names it"
        elif not is_prose_path(f):
            report.append(f"RUN   {f}: not Markdown under notes/, design/, briefs/ or the root")
            prose_only = False
            continue
        elif f in files:
            where = ", ".join(sorted(set(files[f]))[:3])
            report.append(f"RUN   {f}: named by code ({where})")
            prose_only = False
            continue
        else:
            reason = "prose that no code names"
        readers = [(d, place, pkg) for d, hits in dirs.items() if f.startswith(d + "/")
                   for place, pkg in hits]
        rust = sorted({pkg for _, _, pkg in readers if pkg})
        unowned = sorted({place for _, place, pkg in readers if pkg is None and place.split(":")[0].endswith(".rs")})
        if unowned:
            report.append(f"RUN   {f}: its directory is read by Rust in no package ({', '.join(unowned)})")
            prose_only = False
            continue
        tests.update(rust)
        unacted.update(place.split(":")[0] for _, place, pkg in readers if pkg is None)
        note = f"; tests of the packages that walk its directory: {' '.join(rust)}" if rust else ""
        report.append(f"SKIP  {f}: {reason}{note}")
    if unacted:
        report.append("note: these read a changed file's directory outside Rust and are not acted on,")
        report.append("      because lint runs them on every pull request or no gate does (see BUGS):")
        report.append("      " + " ".join(sorted(unacted)))
    if not prose_only:
        return False, [], report, None
    return True, sorted(tests), report, (sorted(comment_tests) if comment_tests is not None else None)


def git_tree():
    """The checked-out tree, which in CI is the head under test."""
    paths = subprocess.run(["git", "ls-files"],
                           capture_output=True, text=True, check=True).stdout.splitlines()
    tree = {}
    for p in paths:
        if p.endswith("Cargo.toml") or scanned(p):
            try:
                with open(p, encoding="utf-8") as fh:
                    tree[p] = fh.read()
            except (UnicodeDecodeError, IsADirectoryError, FileNotFoundError):
                tree[p] = None
    return tree


def git_show(rev, path):
    """`path`'s text at `rev`, or None where it did not exist there."""
    r = subprocess.run(["git", "show", f"{rev}:{path}"], capture_output=True)
    if r.returncode != 0:
        return None
    return r.stdout.decode("utf-8", errors="replace")


def main_classify(base):
    changed = subprocess.run(["git", "diff", "--name-only", base, "HEAD"],
                             capture_output=True, text=True, check=True).stdout.splitlines()
    before = {f: git_show(base, f) for f in changed if f.endswith(".rs")}
    prose, tests, report, comment_tests = classify(changed, git_tree(), before)
    for line in report:
        print(line)
    print(f"prose-only: {'yes' if prose else 'no'} ({len(changed)} changed files)")
    print(f"prose-tests: {' '.join(tests)}")
    print(f"comment-only rust: {'yes' if comment_tests is not None else 'no'}")
    print(f"comment-tests: {' '.join(comment_tests or [])}")
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a") as fh:
            fh.write(f"prose={'true' if prose else 'false'}\n")
            fh.write(f"prose_tests={' '.join(tests)}\n")
            fh.write(f"comments={'true' if comment_tests is not None else 'false'}\n")
            fh.write(f"comment_tests={' '.join(comment_tests or [])}\n")
    return 0


def selftest():
    base_tree = {
        "Cargo.toml": '[workspace]\nmembers = [\n    "kernel",\n    "xtask",\n    "crates/abi",\n'
                      '    "crates/documentation",\n]\n',
        "vendor/x/Cargo.toml": '[package]\nname = "x"\n',
        "tools/host/Cargo.toml": '[package]\nname = "host"\n',
        "crates/documentation/Cargo.toml": '[package]\nname = "documentation"\nversion = "0.1.0"\n',
        "crates/documentation/tests/render.rs":
            '// walks the corpus\nfor dir in ["notes", "design/decisions"] {}\n',
        "crates/abi/Cargo.toml": '[package]\nname = "abi"\n',
        "crates/abi/src/lib.rs": '//! see notes/commented.md\npub fn f() {}\n',
        "xtask/Cargo.toml": '[package]\nname = "xtask"\n',
        "xtask/src/manual.rs": 'const B: &[&str] = &["notes/pipes.md"];\n',
        "script/lint": '# reads notes/lint-comment.md\nls "notes"\n',
        "notes/plain.md": None, "notes/pipes.md": None, "design/roadmap/0001-x.md": None,
    }

    def run(changed, extra=None):
        tree = dict(base_tree)
        tree.update(extra or {})
        return classify(changed, tree)[:3]

    cases = []
    # Code: a .rs file is never prose.
    p, t, r = run(["kernel/src/main.rs"])
    cases.append(("code", p is False and r[0].startswith("RUN")))
    # Docs only, under a directory nothing walks: skips, no tests owed.
    p, t, r = run(["design/roadmap/0001-x.md"])
    cases.append(("docs only", p is True and t == [] and r[0].startswith("SKIP")))
    # Docs under a walked directory: skips, and owes the walker's package tests.
    p, t, r = run(["notes/plain.md"])
    cases.append(("walked directory", p is True and t == ["documentation"]))
    # A script reading a directory is logged and does not stop the skip.
    cases.append(("script reader logged", any("script/lint" in x for x in r)))
    # A note named by code (DOC_BUNDLES): an input, runs everything.
    p, t, r = run(["notes/pipes.md"])
    cases.append(("named .md", p is False and "xtask/src/manual.rs:1" in r[0]))
    # include_str! relative to the source file, added in the same diff.
    p, t, r = run(["notes/plain.md", "crates/abi/src/lib.rs"],
                  {"crates/abi/src/lib.rs": 'pub const S: &str = include_str!("../../../notes/plain.md");\n'})
    cases.append(("included .md", p is False and any("named by code" in x for x in r)))
    p, t, r = run(["notes/plain.md"],
                  {"crates/abi/src/lib.rs": '#![doc = include_str!("../../../notes/plain.md")]\n'})
    cases.append(("doc attribute include", p is False))
    # A comment naming a note does not make it an input, in Rust or in a script.
    p, t, r = run(["notes/commented.md", "notes/lint-comment.md"])
    cases.append(("comments are not readers", p is True))
    # Mixed: one prose file and one code file runs everything.
    p, t, r = run(["design/roadmap/0001-x.md", "Cargo.lock"])
    cases.append(("mixed", p is False and t == []))
    # Markdown outside the prose directories (a crate README) is not prose.
    p, t, r = run(["crates/abi/README.md"])
    cases.append(("crate README", p is False))
    # A top-level .md is prose; a non-.md under notes/ is not (outside the metrics records).
    cases.append(("root md", run(["ARCHITECTS.md"])[0] is True))
    cases.append(("notes non-md", run(["notes/other/data.csv"])[0] is False))
    # Nothing changed is a scope that failed, and runs everything.
    cases.append(("empty", run([])[0] is False))
    # A directory read from Rust in no package cannot be tested, so it runs everything.
    p, t, r = run(["notes/plain.md"], {"loose/walk.rs": 'let d = "notes";\n'})
    cases.append(("unowned rust reader", p is False))

    # A path mentioned in a sentence is a message, not a read; the same path alone is data.
    p, t, r = run(["notes/plain.md"],
                  {"crates/abi/src/lib.rs": 'panic!("the guard was hit; see notes/plain.md");\n'})
    cases.append(("message is not a read", p is True))
    p, t, r = run(["notes/plain.md"], {"script/x": 'cat "$root/notes/plain.md"\n'})
    cases.append(("bare path in a script", p is False))
    # A glob over a prose directory is a directory read.
    p, t, r = run(["design/roadmap/0001-x.md"],
                  {"crates/abi/src/lib.rs": 'let g = glob("design/roadmap/*.md");\n'})
    cases.append(("glob reader", p is True and "abi" in t))

    # A package name that is not a plain name is never handed to the CI shell.
    p, t, r = run(["notes/plain.md"], {"evil/Cargo.toml": '[package]\nname = "x; curl y"\n',
                                        "evil/src/lib.rs": 'let d = "notes";\n'})
    cases.append(("hostile package name", p is False and t == []))

    # Host-only tools: script/metrics skips; a gated caller outside script/ and helpers/ stops it;
    # a neighbor that is not listed (script/test, helpers/verify_times.py) runs everything.
    p, t, r = run(["script/metrics", "notes/plain.md"])
    cases.append(("metrics is host-only", p is True and r[0].startswith("SKIP")))
    p, t, r = run(["script/metrics"], {".github/workflows/ci.yml": "      - run: script/metrics --check\n"})
    cases.append(("gated caller of a host tool", p is False and "named by" in r[0]))
    p, t, r = run(["script/metrics"], {".github/workflows/ci.yml": "      # script/metrics runs weekly\n"})
    cases.append(("comment is not a caller", p is True))
    p, t, r = run(["script/metrics"], {".github/workflows/metrics.yml": "run: script/metrics --update\n"})
    cases.append(("metrics.yml runs it", p is True))
    p, t, r = run(["helpers/baseline_drift.py"], {"xtask/src/x.rs": "let m = \"baseline_drift\";\n"})
    cases.append(("module name counts", p is False))
    cases.append(("script/test runs", run(["script/test"])[0] is False))
    cases.append(("verify_times runs", run(["helpers/verify_times.py"])[0] is False))
    cases.append(("metrics.yml runs", run([".github/workflows/metrics.yml"])[0] is False))
    p, t, r = run(["notes/project-metrics.md", "script/metrics"],
                  {"script/metrics": "x = 'notes/project-metrics.md'\n",
                   ".github/workflows/metrics.yml": "git add notes/project-metrics.md\n",
                   "notes/project-metrics.md": None})
    cases.append(("the #1831 shape skips", p is True))
    cases.append(("tool plus code runs", run(["script/metrics", "kernel/src/main.rs"])[0] is False))

    # Host-only outputs: the weekly metrics update (#1878 ran the kernel suite for this shape).
    # The page is prose, the records are generated, and none of the namers counts: the tools and
    # their runner are forgiven, and script/citations names baseline-drift.md only in a :!
    # pathspec exclusion, which is a refusal to read, not a read.
    weekly = ["notes/project-metrics.md", "notes/project-metrics/weeks.csv",
              "notes/project-metrics/baseline-drift.md", "notes/project-metrics/models-lines.svg"]
    p, t, r = run(weekly,
                  {"script/citations": 'EXCLUDE = (":!notes/project-metrics/baseline-drift.md",)\n',
                   "script/metrics": "# writes notes/project-metrics/weeks.csv\n",
                   ".github/workflows/metrics.yml": "          git add notes/project-metrics/weeks.csv\n"})
    cases.append(("weekly metrics update skips",
                  p is True and t == ["documentation"] and sum(x.startswith("SKIP") for x in r) == 4))
    # A gated workflow naming a record stops the skip.
    p, t, r = run(["notes/project-metrics/weeks.csv"],
                  {".github/workflows/ci.yml": "      - run: cat notes/project-metrics/weeks.csv\n"})
    cases.append(("gated caller of a record", p is False and "named by" in r[0]))
    # So does a gated helper under script/ or helpers/, unlike the tools rule: this is the
    # falsification-times.tsv shape, which script/verify shards by.
    p, t, r = run(["notes/project-metrics/falsification-times.tsv"],
                  {"helpers/verify_times.py": 'REPLAY = "notes/project-metrics/falsification-times.tsv"\n'})
    cases.append(("verify's tsv runs", p is False and "named by" in r[0]))
    # A host-only tool naming the record it writes is not a reader.
    p, t, r = run(["notes/project-metrics/baseline-drift.md"],
                  {"helpers/baseline_drift.py": 'OUT = "notes/project-metrics/baseline-drift.md"\n'})
    cases.append(("the writing tool is not a reader", p is True))

    # Comment-only Rust (milestone 929). Each case changes one kernel file from BASE_RS.
    BASE_RS = ('/// Maps a page.\n'
               'pub fn map(x: u64) -> u64 {\n'
               '    // SAFETY: the caller holds the lock.\n'
               '    let s = "// not a comment";\n'
               '    /* a block /* nested */ still */ x + 1\n'
               '}\n')
    kernel = {"kernel/Cargo.toml": '[package]\nname = "kernel"\n'}

    def rs(after, extra=None, path="kernel/src/map.rs", before=BASE_RS):
        tree = dict(base_tree, **kernel, **{path: after})
        tree.update(extra or {})
        return classify([path], tree, {path: before})

    p, t, r, c = rs(BASE_RS.replace("Maps a page.", "Maps one page into the table."))
    cases.append(("a doc comment edit skips, owing its package's tests", p is True and c == ["kernel"]))
    p, t, r, c = rs(BASE_RS.replace("the caller holds", "this CPU holds"))
    cases.append(("a plain comment edit skips", p is True and c == ["kernel"]))
    p, t, r, c = rs(BASE_RS.replace("still */", "still, longer\n   across lines */"))
    cases.append(("a nested block comment edit skips", p is True))
    # The test 929 asks for: a diff that touches code beside a comment runs everything.
    p, t, r, c = rs(BASE_RS.replace("the caller holds the lock.", "the caller holds it.")
                    .replace("x + 1", "x + 2"))
    cases.append(("code changed beside a comment runs everything",
                  p is False and c is None and "code changed" in r[0]))
    # A `//` inside a string literal is code, and so is a `*/` inside one.
    p, t, r, c = rs(BASE_RS.replace('"// not a comment"', '"// still not a comment"'))
    cases.append(("a // inside a string is code", p is False))
    p, t, r, c = rs(BASE_RS.replace("x + 1", 'x + 1; let r = r#"*/ x"#; 0'))
    cases.append(("a raw string holding */ is code", p is False))
    # Deleting a whole comment line is comment-only; deleting the code under it is not.
    p, t, r, c = rs(BASE_RS.replace("    // SAFETY: the caller holds the lock.\n", ""))
    cases.append(("a deleted comment line skips", p is True))
    p, t, r, c = rs(BASE_RS.replace('    let s = "// not a comment";\n', ""))
    cases.append(("a deleted code line runs", p is False))
    # A new or deleted file, vendored code, and Rust in no package run everything.
    cases.append(("a new file runs", rs(BASE_RS, before=None)[0] is False))
    cases.append(("vendored Rust runs", rs("// x\n" + BASE_RS, path="vendor/x/src/lib.rs")[0] is False))
    cases.append(("Rust in no package runs", rs("// x\n" + BASE_RS, path="loose/x.rs")[0] is False))
    cases.append(("Rust outside the root workspace runs",
                  rs("// x\n" + BASE_RS, path="tools/host/src/lib.rs")[0] is False))
    # Readers: a patch that rewrites the file, a file that names it, a walk of its directory.
    edit = BASE_RS.replace("Maps a page.", "Maps one page.")
    p, t, r, c = rs(edit, {"kernel/falsifications/f.patch": "--- a/kernel/src/map.rs\n+++ b/kernel/src/map.rs\n"})
    cases.append(("a file a falsification patch rewrites runs", p is False and "patch" in r[0]))
    p, t, r, c = rs(edit, {"xtask/src/lint.rs": 'const F: &str = "kernel/src/map.rs";\n'})
    cases.append(("Rust that names the file owes its package's tests", p is True and c == ["kernel", "xtask"]))
    p, t, r, c = rs(edit, {".github/workflows/x.yml": "on:\n  pull_request:\njobs:\n      - run: grep SAFETY kernel/src/map.rs\n"})
    cases.append(("a workflow that names the file runs", p is False and "named by" in r[0]))
    p, t, r, c = rs(edit, {".github/workflows/x.yml": "on:\n  workflow_dispatch:\n  # not on pull_request\n"
                           "jobs:\n  a:\n    if: github.event_name != 'pull_request'\n"
                           "    steps:\n      - run: git grep -l proof -- 'kernel/src'\n"})
    cases.append(("a workflow no pull request runs is not a reader", p is True))
    p, t, r, c = rs(edit, {".github/workflows/x.yml": "on:\n  push:\n  merge_group:\njobs:\n"
                           "      - run: git grep -l proof -- 'kernel/src'\n"})
    cases.append(("a merge-group workflow that walks the directory runs", p is False))
    p, t, r, c = rs(edit, {"helpers/x.py": 'SRC = "kernel/src/map.rs"\n'})
    cases.append(("a script lint runs is logged, not acted on", p is True and "helpers/x.py:1" in r[0]))
    p, t, r, c = rs(edit, {"kernel/Cargo.toml": '[package]\nname = "kernel"\n[[bin]]\npath = "src/map.rs"\n',
                           "kernel/src/lib.rs": '#[path = "map.rs"]\nmod map;\n',
                           "xtask/src/x.rs": 'panic!("see kernel/src/map.rs for why");\n'})
    cases.append(("a manifest, a #[path] and a message are not readers", p is True and c == ["kernel"]))
    p, t, r, c = rs(edit, {"xtask/src/lint.rs": '// kernel/src/map.rs is checked\n'})
    cases.append(("a comment naming the file is not a reader", p is True))
    p, t, r, c = rs(edit, {"xtask/src/archive.rs": 'walk(&root.join("kernel/src"));\n'})
    cases.append(("a Rust walk of its directory owes that package's tests", p is True and c == ["kernel", "xtask"]))
    p, t, r, c = rs(edit, {"loose/walk.rs": 'walk("kernel/src");\n'})
    cases.append(("a walk from Rust in no package runs", p is False))
    # Mixed with prose, comment-only Rust still skips; mixed with code it runs.
    tree = dict(base_tree, **kernel, **{"kernel/src/map.rs": edit})
    p, t, r, c = classify(["kernel/src/map.rs", "design/roadmap/0001-x.md"], tree, {"kernel/src/map.rs": BASE_RS})
    cases.append(("comment-only Rust beside prose skips", p is True and c == ["kernel"]))
    p, t, r, c = classify(["kernel/src/map.rs", "Cargo.lock"], tree, {"kernel/src/map.rs": BASE_RS})
    cases.append(("comment-only Rust beside code runs", p is False and c is None))
    # A prose-only diff reports no comment class, so rustfmt keeps skipping it.
    cases.append(("prose alone has no comment class", classify(["design/roadmap/0001-x.md"], dict(base_tree))[3] is None))
    # The lexer: lifetimes and chars, byte strings, and an escaped quote.
    if rust_code_tokens("fn f<'a>(c: char) { let q = '\"'; let b = b\"/*\"; } // x\n") != \
            ["fn", "f", "<", "'a", ">", "(", "c", ":", "char", ")", "{", "let", "q", "=", "'\"'",
             ";", "let", "b", "=", 'b"/*"', ";", "}"]:
        cases.append(("the lexer keeps lifetimes, chars and byte strings whole", False))

    bad = [name for name, ok in cases if not ok]
    for name, ok in cases:
        print(f"{'ok  ' if ok else 'FAIL'} {name}")
    if bad:
        print(f"prose_only selftest: {len(bad)} of {len(cases)} failed: {', '.join(bad)}", file=sys.stderr)
        return 1
    print(f"prose_only selftest: {len(cases)} cases")
    return 0


if __name__ == "__main__":
    args = sys.argv[1:]
    if args == ["--selftest"]:
        sys.exit(selftest())
    if len(args) == 2 and args[0] == "classify":
        sys.exit(main_classify(*args[1:]))
    print(__doc__.split("\n\n")[1], file=sys.stderr)
    sys.exit(2)
