"""Names meet the current naming rules: the gate, the ratchet on what does not yet, and the worklist.

calef, 2026-10-18 (UTC): "Honestly I think we could automate making our existing unratified names
meet our existing rules. But we need to continue to ratify and revise our rules." So the cycle is:
calef ratifies names and revises rules, a revised rule lands with its check here, and lanes rename
unratified names that break it until they comply. A renamed name stays `provisional`. Nothing is
ratified by rule.

    python3 helpers/name_checks.py [PREFIX..]   # the worklist, local names first, and a report
    python3 helpers/name_checks.py --check      # the gate script/lint runs
    python3 helpers/name_checks.py --bank       # drop the rows of names that now comply
    python3 helpers/name_checks.py --selftest   # the checks, the reach and the ratchet, as fixtures

**The gate.** A name a change adds or renames, and that is not ratified, passes every check in
`GATED`, with no baseline allowance. The unratified names that break one today are the baseline
(`BASELINE`), which only shrinks. A row is added only in the change that bumps `RULES`, because it
adds or revises a check, so new debt arrives with its cause. Ratified names are exempt, since a
later rule never silently overrules calef's ruling, and the worklist reports each one that fails a
current check so he can reopen it. Spelling and `cap` have ratchets of their own
(`helpers/house_style.py`, `helpers/cap_abbreviation.py`) and are reported here, not held twice.

**Reach**, in the order a fix lane works the list:

- `local`: no other Cargo package reads it, so a rename touches one package.
- `crosses into N`: N other packages read it, measured by the defining crate's path
  (`name_crossings.Reach`, never the bare word). Renamed by family.
- `wire or format`: defined in a package two programs agree through (`WIRE`). A rename there
  changes what another program parses, so it is listed for calef and never renamed by a lane.

A name that passes every check is not thereby right: design/naming/rules-a-script-can-check.md
lists the rules no script can apply.

Name: provisional, minted 2026-10-18 (UTC) by lane maintainer/names-ratify-local-by-rule. A shared
helper under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the
record. It takes `name_` for the family beside `name_census.py`, and `checks` for what it runs.
calef has not named it.
"""

import collections
import json
import os
import re
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cap_abbreviation  # noqa: E402
import moved_rows  # noqa: E402
import name_census  # noqa: E402
import name_crossings  # noqa: E402
import name_provenance  # noqa: E402

# Components a ruling spells out: §154 (the acronym test is whether the phrase is spoken)'s table
# and design/naming/spelled-out-rulings.md. A plural is the same abbreviation. A foreign identifier
# keeps its owner's spelling, and telling one from ours is judgment, so it is listed too and the
# lane that fixes it decides.
SPELLED_OUT = {
    "cap", "caps", "op", "ops", "recv", "config", "configs", "spec", "specs", "arg", "args",
    "mem", "dir", "dirs", "va", "pa", "ipi", "ipis", "nmi", "nmis", "ist", "msr", "msrs", "ipc",
    "dtb", "asid", "asids", "gpt", "nvme", "pcie", "dma", "pr", "prs", "initrd", "prog", "progs",
    "pos", "opt", "opts", "mtime",
}
# Packages two programs agree through: `abi`, the wire-contract crates (`script/lint` check 3 holds
# their spelling, `network_time_protocol` and `supervision_protocol` its two exceptions), and the
# on-disk formats §307 (how long a name may be) lists. Something else another program parses is
# missed; the BUGS below say so.
WIRE = {"abi", "network_time_protocol", "supervision_protocol", "nifefs", "package_archive",
        "documentation"}
# Spelled in two halves because script/lint's naming check refuses the word outside design/.
_SERVICE_WORD = "dae" + "mon"
_CAP = {"cap", "caps"}
# Names a check refuses and a rule of design/naming.md exempts, keyed by `path::name`, each with the
# checks it is excused from and why. Two kinds, the rule each one applies named in its reason: a
# foreign identifier keeps its owner's spelling, and a function that acts and reports whether it
# worked is not a question. An entry is added in the change that adds the name or reads it, and
# removed by renaming the name. The check cannot tell either kind apart from a break; a reader can.
_SBI = "RISC-V SBI's own name, kept as the precedents keep `sbi_send_ipi`"
_ACTS = ("acts and reports whether it worked, which design/naming.md exempts from the question "
         "shape")
KEPT = {
    "crates/designware_ethernet/src/mdio.rs::PA_SHIFT":
        ({"spelled-out"}, "DesignWare's `PA` field, the PHY address, not a physical address"),
    "kernel/src/arch/riscv64/harts.rs::sbi_send_ipi": ({"spelled-out"}, _SBI),
    "kernel/src/arch/riscv64/remote_fence.rs::sbi_remote_sfence_vma_asid": ({"spelled-out"}, _SBI),
    "kernel/src/arch/aarch64/interrupts.rs::disable": ({"question-shape"}, _ACTS),
    "kernel/src/arch/riscv64/interrupts.rs::disable": ({"question-shape"}, _ACTS),
    "kernel/src/arch/x86_64/interrupts.rs::disable": ({"question-shape"}, _ACTS),
    "kernel/src/arch/aarch64/mmu.rs::permit_kernel_access_to_user_pages": ({"question-shape"}, _ACTS),
    "kernel/src/arch/riscv64/mmu.rs::permit_kernel_access_to_user_pages": ({"question-shape"}, _ACTS),
    "kernel/src/arch/x86_64/mmu/user_space.rs::permit_kernel_access_to_user_pages":
        ({"question-shape"}, _ACTS),
    "kernel/src/arch/aarch64/timer.rs::within_raise_bound": ({"question-shape"}, _ACTS),
    "kernel/src/arch/riscv64/timer.rs::within_raise_bound": ({"question-shape"}, _ACTS),
    "kernel/src/arch/x86_64/mmu/shootdown.rs::serve_shootdown_nmi": ({"question-shape"}, _ACTS),
    "kernel/src/sched/inspection.rs::kill_thread": ({"question-shape"}, _ACTS),
    "kernel/src/sched/inspection.rs::hold_on_its_core": ({"question-shape"}, _ACTS),
    "kernel/src/sched/switch.rs::mark_current_fp_live": ({"question-shape"}, _ACTS),
    "kernel/src/user/fs_service/std_start.rs::release": ({"question-shape"}, _ACTS),
    "kernel/src/user/holding.rs::release": ({"question-shape"}, _ACTS),
    "kernel/src/user/install_service.rs::ask_to_confirm": ({"question-shape"}, _ACTS),
    "kernel/src/arch/aarch64/percpu.rs::percpu_matches_hart":
        ({"question-shape"}, "a finite verb phrase (`matches`) the check cannot see, which the "
                             "question rule allows"),
    "kernel/src/user/test_support.rs::wait_for": ({"question-shape"}, _ACTS),
    # A gate: it builds, boots and reports whether the boot passed, like `stick_boot` beside it.
    "xtask/src/stick.rs::acpi_boot": ({"question-shape"}, _ACTS),
}
# design/naming.md's question shapes. A finite verb first (`contains`, `starts_with`) is read as a
# first word ending in `s`, which also passes a plural noun: the check's known false pass.
_QUESTION_FIRST = {"is", "has", "can", "was", "will", "all", "any"}
_QUESTION_INNER = {"is", "has", "can", "was"}
# A `take_` clears a flag and reports it, which design/naming.md exempts by name.
_ACTS_FIRST = {"take"}
# The casing each Rust kind takes. A file name's casing is `script/lint`'s naming check 6, and a
# name declared in an `extern` block is its owner's, so neither is checked here.
_CASING = {
    "fn": r"[a-z_][a-z0-9_]*", "const": r"[A-Z_][A-Z0-9_]*", "static": r"[A-Z_][A-Z0-9_]*",
    "type": r"[A-Z][A-Za-z0-9]*", "field": r"[a-z_][a-z0-9_]*", "variant": r"[A-Z][A-Za-z0-9]*",
    "module": r"[a-z_][a-z0-9_]*", "crate": r"[a-z][a-z0-9_]*", "program": r"[a-z][a-z0-9_]*",
}
_KEYWORD = {"fn": "fn", "const": "const", "static": "static", "struct": "type", "enum": "type",
            "union": "type", "trait": "type", "type": "type", "function": "fn",
            "constant": "const", "macro": "fn", "field": "field", "variant": "variant"}


def components(name):
    return [c.lower() for c in cap_abbreviation.components(name) if c != "-"]


def failures(name, kind, returns_bool, british):
    """Each check `name` fails, as `check: why` strings; empty when it passes all of them."""
    out = []
    shape = _CASING.get(kind)
    if shape and not re.fullmatch(shape, name):
        out.append(f"casing: not this tree's casing for a {kind}")
    words = components(name)
    # "opt in" and "opt out" are English verbs, not `option` abbreviated.
    ruled = [w for i, w in enumerate(words) if w in SPELLED_OUT and w not in _CAP
             and not (w == "opt" and words[i + 1:i + 2] in (["in"], ["out"]))]
    if ruled:
        out.append(f"spelled-out: `{ruled[0]}` is ruled to expand")
    elif any(w in _CAP for w in words):
        out.append("cap: `cap` is ruled to expand (helpers/cap_abbreviation.py holds it)")
    if name in british:
        out.append(f"spelling: `{british[name]}` is British (helpers/house_style.py holds it)")
    if _SERVICE_WORD in words or (len(words) > 1 and words[-1] == "d"):
        out.append("service-model: claims a Unix service")
    if kind == "fn" and returns_bool and words:
        first = words[0]
        # A finite verb anywhere passes (design/naming.md: `machine_has_no_rtc`), read as an inner
        # `is`/`has`/`can`/`was` or a last word ending in `s` (`grant_allows`, `day_matches`).
        last = words[-1]
        if not (first in _QUESTION_FIRST or first in _ACTS_FIRST
                or (first.endswith("s") and not first.endswith("ss"))
                or (len(words) > 1 and last.endswith("s") and not last.endswith("ss"))
                or any(w in _QUESTION_INNER for w in words[1:])):
            out.append("question-shape: returns `bool` and is not `is_`, `has_`, `can_` or a "
                       "finite verb")
    return out


def british_of(names):
    """`{name: word}` for names holding a British form: `typos` en-us over en, and house_style's list."""
    import house_style
    if not shutil.which("typos"):
        sys.exit("name_checks: typos is not installed; run script/bootstrap (it installs it)")
    names = sorted(set(names))

    def run(locale):
        res = subprocess.run(["typos", "--locale", locale, "--format", "json", "-"],
                             input="\n".join(names), capture_output=True, text=True)
        return {(r["line_num"], r["typo"]) for r in map(json.loads, res.stdout.splitlines())
                if r.get("type") == "typo"}
    out = {names[line - 1]: word for line, word in run("en-us") - run("en")}
    for name in names:
        hit = [w for w in components(name) if w in house_style.EXTRA_BRITISH]
        if hit:
            out.setdefault(name, hit[0])
    return out


def _returns_bool(stripped_lines, line):
    """Whether the function on `line` answers a question: returns `bool` and takes no `&mut`.

    One taking `&mut` acts and reports whether it worked (`insert`), which design/naming.md exempts.
    """
    sig = " ".join(stripped_lines[line - 1:line + 11])
    sig = re.split(r"[{;]", sig, maxsplit=1)[0]
    return re.search(r"->\s*bool\b", sig) is not None and "&mut" not in sig


def _declaration_line(lines, header):
    """The 1-based line of the declaration under the doc run that starts at `header`."""
    for i in range(header - 1, len(lines)):
        s = lines[i].strip()
        if s and not s.startswith(("//", "#[", "#![")):
            return i + 1
    return header


def is_wire(package):
    return package is not None and (package in WIRE or package.endswith("_contract"))


def candidates(records, unblocked, sources, ratified=False):
    """`(kind, name, path, line)` for every unratified name: blocked but not ratified, or unblocked.

    With `ratified`, the ratified names instead, which the gate exempts and the report lists.
    """
    out = []
    for r in records:
        if (r["status"] == "ratified") != ratified:
            continue
        if r["kind"] == "item":
            path, _, header = r["path"].partition(":")
            if path not in sources:
                continue
            line = _declaration_line(sources[path].split("\n"), int(header))
            m = name_provenance._ITEM.match(sources[path].split("\n")[line - 1])
            kind = _KEYWORD.get(m.group(1) if m else r.get("what"), "fn")
            out.append((kind, r["name"], path, line))
        elif r["kind"] in ("module", "crate", "program", "script", "document"):
            out.append((r["kind"], r["name"], r["path"], 1))
    for row in ([] if ratified else unblocked):
        path, sep, name = row.partition("::")
        if sep:
            lines = sources.get(path, "").split("\n")
            for number, text in enumerate(lines, 1):
                m = name_provenance._ITEM.match(text)
                if (m and m.group(2) == name and m.group(1) in _KEYWORD
                        and text.lstrip().startswith("pub ")):
                    out.append((_KEYWORD[m.group(1)], name, path, number))
        elif path.endswith(".rs"):
            stem = os.path.splitext(os.path.basename(path))[0]
            if stem in ("mod", "lib", "main"):
                stem = os.path.basename(os.path.dirname(path))
            out.append(("module", stem, path, 1))
        else:
            out.append(("document", os.path.basename(path)[:-3], path, 1))
    return sorted(set(out), key=lambda c: (c[2], c[3], c[1]))


def survey(cands, sources, manifests, british=None, reach=None):
    """Every candidate that fails a check: dicts with `kind`, `name`, `path`, `line`, `reach`,
    `readers` and `failures`."""
    reach = reach or name_crossings.Reach(manifests, sources)
    if british is None:
        british = british_of(c[1] for c in cands)
    out = []
    for kind, name, path, line in cands:
        returns_bool = (kind == "fn" and path in reach.stripped
                        and _returns_bool(reach.stripped[path].split("\n"), line))
        if path in reach.stripped and any(k == "extern" for k, _ in reach.stack(path, line)):
            continue
        excused = KEPT.get(f"{path}::{name}", (set(), ""))[0]
        why = [f for f in failures(name, kind, returns_bool, british)
               if f.split(":")[0] not in excused]
        if not why:
            continue
        package = reach.package(path)
        if kind == "crate":
            package = reach.package(path) or name
        if is_wire(package):
            where, readers = "wire or format", []
        elif kind in ("crate", "program", "script", "document"):
            where, readers = kind, []
        else:
            if kind == "module":
                readers = sorted(reach.paths_reached.get(package, {}).get(name, ()))
            else:
                readers = reach.readers(path, line, name)
            where = f"crosses into {len(readers)}" if readers else "local"
        out.append({"kind": kind, "name": name, "path": path, "line": line, "reach": where,
                    "readers": readers, "failures": why})
    return out


def _order(row):
    rank = {"local": 0, "wire or format": 3}.get(row["reach"], 1 if row["readers"] else 2)
    return rank, row["path"], row["line"]


def load(root):
    out = subprocess.run(["git", "-C", root, "ls-files", "-z", "--cached", "--others",
                          "--exclude-standard"], capture_output=True, text=True, check=True).stdout
    files = [p for p in out.split("\0") if p and os.path.isfile(os.path.join(root, p))]

    def read(rel):
        try:
            return open(os.path.join(root, rel), errors="replace").read()
        except (FileNotFoundError, IsADirectoryError):
            return None
    records, _problems, _missing, unblocked = name_census.census(files, read)
    manifests = {os.path.dirname(p): read(p) for p in files
                 if p.endswith("Cargo.toml") and not p.startswith("vendor/")}
    sources = {p: read(p) for p in files
               if p.endswith(".rs") and not p.startswith(("vendor/", "patches/"))}
    return records, unblocked, manifests, sources


# --- the ratchet: names meet the current rules ------------------------------------------------------

BASELINE = "design/naming-rule-baseline.txt"
# The rules' revision. Bump it in the change that adds or revises a check, and only then: that is
# the one change whose new rows the merge-base comparison admits, so new debt arrives with its cause.
# This is an exception and a foot gun on purpose, the module-block ratchet's FORMAT line one door
# over: a bump with no revised check behind it would admit any violation, and nothing else stops it.
RULES = 1
# The checks the gate holds. `spelling` and `cap` have ratchets of their own
# (helpers/house_style.py, helpers/cap_abbreviation.py), so they are listed but not held twice.
GATED = ("casing", "spelled-out", "service-model", "question-shape")
_RULES_LINE = "# rules {}"
HEADER = """# rules {rules}
# The naming-rule ratchet's baseline: helpers/name_checks.py, whose header is the manual. One row per
# unratified name that broke a checked naming rule when this list was written, as
# `<path>::<name> <check>`, once per break. The list only shrinks: a fixed name's row goes in the
# change that fixes it (`python3 helpers/name_checks.py --bank`). A row may be added only by the
# change that bumps the rules line above, because it adds or revises the check.
"""


def rows_of(survey_rows):
    """The baseline rows for a survey: one `path::name check` per gated failure."""
    return collections.Counter(f"{r['path']}::{r['name']} {f.split(':')[0]}" for r in survey_rows
                               for f in r["failures"] if f.split(":")[0] in GATED)


def read_baseline(text):
    rules = next((int(line.split()[-1]) for line in text.splitlines()
                  if re.fullmatch(r"# rules \d+", line.strip())), None)
    rows = collections.Counter(line.strip() for line in text.splitlines()
                               if line.strip() and not line.startswith("#"))
    return rules, rows


def write_baseline(root, rows):
    with open(os.path.join(root, BASELINE), "w") as f:
        f.write(HEADER.format(rules=RULES) + "".join(f"{row}\n" for row in sorted(rows.elements())))


def _git(root, *args):
    r = subprocess.run(("git", "-C", root) + args, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


def ratchet_problems(judged, tree, rows, rules, base=None, renames=(), kept=(), before=None,
                     after=None):
    """Every failure of the gate. `judged` is the `path::name` of every name the gate judged, and
    `base` is `(rules, rows)` at the merge base, or None. `before` and `after` read a file at the
    merge base and now: a row that moved with its item in a split (`helpers/moved_rows.py`) is not
    an addition, and without them no row moves."""
    if not judged:
        return ["the check judged no name at all. A selector that matches nothing passes by "
                "checking nothing (design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md)"]
    bad = []
    if rules != RULES:
        bad.append(f"{BASELINE}: its rules line says {rules} and name_checks.py's RULES is {RULES}. "
                   f"Run --bank in the change that revised the check")
    for key in sorted(set(kept) - judged):
        bad.append(f"name_checks.py KEPT names {key}, which the gate does not judge. Remove the "
                   f"entry; one left behind would excuse whatever name takes its place")
    for row, n in sorted((tree - rows).items()):
        name, check = row.rsplit(" ", 1)
        bad.append(f"{name} breaks the {check} rule, and it is not on the baseline. A name a change "
                   f"adds or renames meets the current naming rules (design/naming.md); rename it "
                   f"to comply and mark it `Name: provisional, minted <date> (UTC) by <lane>.`")
    # A row whose name left the judged set (narrowed to `pub(crate)`, deleted, moved or ratified)
    # still fails rather than passing quietly: left behind, it would admit the same break the day
    # the name comes back into scope. `--bank` drops it, so the message says that plainly.
    for row, n in sorted((rows - tree).items()):
        if row.rsplit(" ", 1)[0] in judged:
            why = "no longer breaks its rule"
        else:
            why = ("names something the gate no longer judges (narrowed to `pub(crate)`, deleted, "
                   "moved to another file, or ratified)")
        bad.append(f"{BASELINE}: {row} {why}. Run `python3 helpers/name_checks.py --bank` in this "
                   f"change to drop it; the list cannot go stale")
    if base is not None and base[0] == RULES:
        was = {new: old for old, new in renames}

        def as_it_was(row):
            path = row.split("::", 1)[0]
            return row if path not in was else was[path] + row[len(path):]
        moved = collections.Counter()
        for row, n in rows.items():
            moved[as_it_was(row)] += n
        added = moved - base[1]
        if added and before is not None:
            added -= moved_rows.arrivals(moved_rows.moves(added, base[1] - moved, before, after))
        for row in sorted(added):
            bad.append(f"{BASELINE}: {row} was added. The list only shrinks, except in the change "
                       f"that bumps RULES because it adds or revises a check")
    return bad


def _reader(root, ref=None):
    """A function from a path to its text: at `ref` when given, else in the working tree."""
    if ref is not None:
        return lambda path: _git(root, "show", f"{ref}:{path}")

    def read(path):
        try:
            return open(os.path.join(root, path), errors="replace").read()
        except OSError:
            return None
    return read


def _renames(root, ref):
    """[(old, new)] for every file git itself calls renamed between `ref` and the tree."""
    pairs = ()
    for line in (_git(root, "diff", "--name-status", "-M", ref) or "").splitlines():
        cells = line.split("\t")
        if cells[0].startswith("R") and len(cells) == 3:
            pairs += ((cells[1], cells[2]),)
    return pairs


def run_check(root, rows_now, judged):
    path = os.path.join(root, BASELINE)
    if not os.path.exists(path):
        print(f"names: {BASELINE} is missing. The gate and its list are one change",
              file=sys.stderr)
        return 1
    rules, rows = read_baseline(open(path).read())
    base, renames = None, ()
    merge = (_git(root, "merge-base", "HEAD", "origin/main") or "").strip()
    head = (_git(root, "rev-parse", "HEAD") or "").strip()
    if merge and merge != head:
        old = _git(root, "show", f"{merge}:{BASELINE}")
        if old is not None:
            base = read_baseline(old)
        renames = _renames(root, merge)
    else:
        merge = None
    bad = ratchet_problems(judged, rows_now, rows, rules, base, renames, KEPT,
                           _reader(root, merge) if merge else None, _reader(root))
    if bad:
        print("names: unratified names must meet the current naming rules:", file=sys.stderr)
        for b in bad:
            print(f"  {b}", file=sys.stderr)
        return 1
    print(f"names: {len(judged)} unratified names judged against rules revision {RULES}; "
          f"{sum(rows.values())} known breaks held to the baseline, none new")
    return 0


def _banked(root, old, now):
    """The rows `--bank` keeps: those the tree still breaks, re-keyed where git calls a file renamed
    or where an item moved to another file of its crate."""
    merge = (_git(root, "merge-base", "HEAD", "origin/main") or "").strip()
    head = (_git(root, "rev-parse", "HEAD") or "").strip()
    ref = merge if merge and merge != head else "HEAD"
    called = dict(_renames(root, ref))
    renamed = collections.Counter()
    for row, n in old.items():
        path = row.split("::", 1)[0]
        renamed[called.get(path, path) + row[len(path):]] += n
    pairs = moved_rows.moves(now - renamed, renamed - now, _reader(root, ref), _reader(root))
    return moved_rows.banked(renamed, now, pairs)


def _selftest():
    manifests = {"lib_a": '[package]\nname = "lib-a"\n',
                 "app": '[package]\nname = "app"\n[dependencies]\nlib-a = { path = "../lib_a" }\n',
                 "x_contract": '[package]\nname = "x_contract"\n'}
    lib = "\n".join([
        "pub fn mem_size() {}",                                   # 1: local, spelled out
        "pub fn arg_count() {}",                                  # 2: app reads it by path
        "pub struct Seen;",                                       # 3: app names the type
        "impl Seen {",
        "    pub fn pending(&self) -> bool { true }",             # 5: crosses with its type
        "}",
        "pub fn contains() -> bool { true }",                     # 7: a finite verb, passes
        "/// Name: ratified 2026-10-01 (calef, here).",
        "pub fn op_ok() {}",                                      # 9: ratified, not listed
        "/// Name: provisional.",
        "pub fn tint() {}",                                       # 11: blocked, flagged by the list
        "",
    ])
    app = ("use lib_a::{arg_count, Seen};\n// lib_a::mem_size in a comment is not a read\n"
           "fn main() { arg_count(); let s: Seen; }\n")
    sources = {"lib_a/src/lib.rs": lib, "app/src/main.rs": app,
               "x_contract/src/lib.rs": "pub const CONFIG_WORD: u32 = 1;\n"}
    # A census record's `path` is `file:doc line`; these name the fixture above, not tree code.
    # line-citation: exception. Reason: fixture records in the census's own path form
    records = [{"kind": "item", "name": "op_ok", "path": "lib_a/src/lib.rs:8", "status": "ratified",
                "what": "function"},
               # line-citation: exception. Reason: fixture records in the census's own path form
               {"kind": "item", "name": "tint", "path": "lib_a/src/lib.rs:10",
                "status": "provisional", "what": "function"}]
    unblocked = ["lib_a/src/lib.rs::mem_size", "lib_a/src/lib.rs::arg_count",
                 "lib_a/src/lib.rs::Seen", "lib_a/src/lib.rs::pending",
                 "lib_a/src/lib.rs::contains", "x_contract/src/lib.rs::CONFIG_WORD"]
    cands = candidates(records, unblocked, sources)
    assert ("fn", "op_ok", "lib_a/src/lib.rs", 9) not in cands, "a ratified name is not a candidate"
    rows = survey(cands, sources, manifests, {"tint": "tint"})
    got = {r["name"]: (r["reach"], r["failures"][0].split(":")[0]) for r in rows}
    assert got == {"mem_size": ("local", "spelled-out"),
                   "arg_count": ("crosses into 1", "spelled-out"),
                   "pending": ("crosses into 1", "question-shape"),
                   "tint": ("local", "spelling"),
                   "CONFIG_WORD": ("wire or format", "spelled-out")}, got
    order = [r["name"] for r in sorted(rows, key=_order)]
    assert order[-1] == "CONFIG_WORD" and order.index("mem_size") < order.index("arg_count"), order
    assert failures("kernel-print-d", "script", False, {})[0].startswith("service"), \
        "a trailing `d` component claims a service"
    assert failures("BadCase", "fn", False, {})[0].startswith("casing")
    assert failures("take_aborted", "fn", True, {}) == [], "a take_ clears a flag, exempt by name"
    assert failures("grant_allows", "fn", True, {}) == [], "a finite verb last is a question"
    assert failures("frame_waiting", "fn", True, {})[0].startswith("question-shape")
    assert failures("why-opt-in", "document", False, {}) == [], "opt in is a verb"
    extern = 'extern "system" {\n    pub fn GetLogicalDrives() -> u32;\n}\n'
    assert survey([("fn", "GetLogicalDrives", "lib_a/src/ffi.rs", 2)],
                  dict(sources, **{"lib_a/src/ffi.rs": extern}), manifests, {}) == [], \
        "an extern block's name is its owner's"
    KEPT["lib_a/src/lib.rs::mem_size"] = ({"spelled-out"}, "fixture")
    assert "mem_size" not in {r["name"] for r in survey(cands, sources, manifests, {"tint": "tint"})}
    del KEPT["lib_a/src/lib.rs::mem_size"]
    acting = "pub fn insert(&mut self) -> bool { true }\n"
    assert not _returns_bool(acting.split("\n"), 1), "an action that reports success is exempt"
    cases = rows_of(rows)
    assert cases["lib_a/src/lib.rs::mem_size spelled-out"] == 1, cases
    assert not any(k.endswith(" spelling") for k in cases), "house_style holds spelling"
    base_rows = collections.Counter(cases)
    judged = {f"{c[2]}::{c[1]}" for c in cands} | {"lib_a/src/lib.rs::dir_len",
                                                    "lib_a/src/b.rs::mem_size"}
    assert ratchet_problems(judged, cases, cases, RULES, (RULES, base_rows)) == []
    fixed = cases - collections.Counter(["lib_a/src/lib.rs::mem_size spelled-out"])
    stale = ratchet_problems(judged, fixed, cases, RULES)
    assert len(stale) == 1 and "--bank" in stale[0], "a fixed name's row is removed"
    gone = ratchet_problems(judged - {"lib_a/src/lib.rs::mem_size"}, fixed, cases, RULES)
    assert len(gone) == 1 and "pub(crate)" in gone[0] and "--bank" in gone[0], \
        "a row whose name left scope says so, and how to drop it"
    new = cases + collections.Counter(["lib_a/src/lib.rs::dir_len spelled-out"])
    bad = ratchet_problems(judged, new, cases, RULES)
    assert len(bad) == 1 and "dir_len breaks the spelled-out rule" in bad[0], "a new break fails"
    added = ratchet_problems(judged, new, new, RULES, (RULES, base_rows))
    assert len(added) == 1 and "was added" in added[0], "a row added against the base fails"
    assert ratchet_problems(judged, new, new, RULES, (RULES - 1, base_rows)) == [], \
        "a rules bump admits the rows its revised check finds"
    moved = collections.Counter(r.replace("lib_a/src/lib.rs", "lib_a/src/b.rs") for r in
                                cases.elements())
    assert ratchet_problems(judged, moved, moved, RULES, (RULES, base_rows),
                            (("lib_a/src/lib.rs", "lib_a/src/b.rs"),)) == [], "a rename moves rows"
    stale_kept = ratchet_problems(judged, cases, cases, RULES, kept={"x/src/gone.rs::f": None})
    assert len(stale_kept) == 1 and "KEPT names x/src/gone.rs::f" in stale_kept[0]
    assert "judged no name" in ratchet_problems(set(), cases, cases, RULES)[0]
    _selftest_split()
    print("name_checks: selftest ok")


def _selftest_split():
    """A split git calls no rename: the parent stays, and its items move to a sibling file."""
    lib, part = "s/src/lib.rs", "s/src/part.rs"
    was = {lib: "pub fn mem_size() {}\npub fn arg_count() {}\n"
                "pub fn dma_new() {}\npub fn dma_new() {}\n"}
    now = {lib: "mod part;\npub fn arg_count() {}\npub fn dma_new() {}\n",
           part: "pub fn mem_size() {}\npub fn dma_new() {}\n"}
    base = collections.Counter([f"{lib}::mem_size spelled-out", f"{lib}::arg_count spelled-out",
                                f"{lib}::dma_new spelled-out", f"{lib}::dma_new spelled-out"])
    tree = collections.Counter([f"{part}::mem_size spelled-out", f"{lib}::arg_count spelled-out",
                                f"{lib}::dma_new spelled-out", f"{part}::dma_new spelled-out"])
    judged = {f"{lib}::arg_count", f"{lib}::dma_new", f"{part}::mem_size", f"{part}::dma_new"}
    rows = moved_rows.banked(base, tree, moved_rows.moves(tree - base, base - tree, was.get,
                                                          now.get))
    assert rows == tree and sum(rows.values()) == sum(base.values()), \
        f"--bank re-keys the rows a split moved, and keeps their count: {rows}"
    assert ratchet_problems(judged, tree, rows, RULES, (RULES, base), (), (), was.get,
                            now.get) == [], "the gate takes a split's moved rows"
    assert ratchet_problems(judged, tree, rows, RULES, (RULES, base)) != [], \
        "without the sources no row moves"
    grown = dict(now, **{part: now[part] + "pub fn dir_len() {}\n"})
    new = tree + collections.Counter([f"{part}::dir_len spelled-out"])
    kept = moved_rows.banked(base, new, moved_rows.moves(new - base, base - new, was.get,
                                                         grown.get))
    assert kept == tree, "--bank never admits a new name that arrived with a split"
    bad = ratchet_problems(judged | {f"{part}::dir_len"}, new, new, RULES, (RULES, base), (), (),
                           was.get, grown.get)
    assert len(bad) == 1 and "dir_len spelled-out was added" in bad[0], bad
    copied = dict(now, **{lib: was[lib]})
    copies = ratchet_problems(judged | {f"{lib}::mem_size"}, tree, tree, RULES, (RULES, base), (),
                              (), was.get, copied.get)
    assert [b.split(" was added")[0] for b in copies] == [
        f"{BASELINE}: {part}::dma_new spelled-out", f"{BASELINE}: {part}::mem_size spelled-out"], \
        f"a copy that leaves the original behind moves nothing: {copies}"
    other = {"t/src/part.rs": now[part], lib: now[lib]}
    moved = collections.Counter(r.replace(part, "t/src/part.rs") for r in tree.elements())
    across = ratchet_problems({r.rsplit(" ", 1)[0] for r in moved}, moved, moved, RULES,
                              (RULES, base), (), (), was.get, other.get)
    assert [b.split(" was added")[0] for b in across] == [
        f"{BASELINE}: t/src/part.rs::dma_new spelled-out",
        f"{BASELINE}: t/src/part.rs::mem_size spelled-out"], \
        f"a name that crossed into another crate is not a move: {across}"


def _report(rows, ratified_rows):
    by_reach = collections.Counter("crosses" if r["readers"] else r["reach"] for r in rows)
    by_check = collections.Counter(f.split(":")[0] for r in rows for f in r["failures"])
    print(f"unratified names failing a naming rule: {len(rows)} "
          f"({', '.join(f'{k} {v}' for k, v in by_reach.most_common())})")
    print("  by check: " + ", ".join(f"{k} {v}" for k, v in by_check.most_common()))
    heads = {0: "LOCAL: rename in its package",
             1: "CROSSING: rename by family",
             2: "CRATES, PROGRAMS, SCRIPTS AND DOCUMENTS: newcomer-facing, rename by family",
             3: "WIRE OR FORMAT: list for calef, never renamed by a lane"}
    section = None
    for r in sorted(rows, key=_order):
        if _order(r)[0] != section:
            section = _order(r)[0]
            print("\n" + heads[section])
        seen = f"  <- {', '.join(r['readers'][:3])}" if r["readers"] else ""
        print(f"  {r['path']}:{r['line']} {r['kind']} {r['name']} [{r['reach']}] "
              f"{'; '.join(r['failures'])}{seen}")
    print(f"\nRATIFIED, FAILING A CURRENT CHECK ({len(ratified_rows)}): exempt from the gate, since a "
          f"later rule never overrules calef's ruling; listed so he can reopen one")
    for r in sorted(ratified_rows, key=lambda r: (r["path"], r["line"])):
        print(f"  {r['path']}:{r['line']} {r['kind']} {r['name']} {'; '.join(r['failures'])}")


def main(argv):
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    if argv == ["--selftest"]:
        _selftest()
        return 0
    mode = argv[0] if argv and argv[0].startswith("-") else "--list"
    prefixes = tuple(a for a in argv if not a.startswith("-"))
    if mode not in ("--list", "--check", "--bank", "--init"):
        sys.exit(__doc__.split("\n\n")[2])
    records, unblocked, manifests, sources = load(root)
    reach = name_crossings.Reach(manifests, sources)
    cands = candidates(records, unblocked, sources)
    kept = candidates(records, unblocked, sources, ratified=True)
    british = british_of(c[1] for c in cands + kept)
    rows = survey(cands, sources, manifests, british, reach)
    if mode == "--check":
        return run_check(root, rows_of(rows), {f"{c[2]}::{c[1]}" for c in cands})
    if mode in ("--bank", "--init"):
        path = os.path.join(root, BASELINE)
        if mode == "--init" and os.path.exists(path):
            sys.exit(f"name_checks: {BASELINE} exists; --bank lowers it")
        now = rows_of(rows)
        if mode == "--bank":
            rules, old = read_baseline(open(path).read())
            if rules == RULES:
                now = _banked(root, old, now)
        write_baseline(root, now)
        print(f"name_checks: {sum(now.values())} rows written to {BASELINE} (rules {RULES})")
        return 0
    if prefixes:
        rows = [r for r in rows if r["path"].startswith(prefixes)]
    _report(rows, survey(kept, sources, manifests, british, reach))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
