#!/usr/bin/env python3
"""Externalized crates do not mention nife (a lint section of script/lint).

A crate that is to leave for a repository of its own stands alone: its README, its Cargo.toml and
its sources name no nife program, no nife package and not nife itself. The crates are listed in
crates/externalization-list.txt (from the order of extraction in the proposal
the-contract-crates-leave-in-this-order). Decided by the maintainer 2026-11-02 (UTC), provisional;
reversing costs one pull request (delete the section in script/lint and this file).

Rules, each named in a failure:
  nife-word       the word `nife` (case-insensitive, not inside a longer word) in a README,
                  a Cargo.toml or a source file under src/.
  nifeos-word     `nifeos`, except on a Cargo.toml `repository`, `homepage` or `documentation` line
                  and on a README badge line (`[![`), which names the crate's own repository.
  consumer-name   the name of a nife program (a `[[bin]]` of components/Cargo.toml or the directory
                  of a top-level program crate with a hyphen or underscore, so one plain word such as
                  `console` is never taken for a program) or of a package (packages/*.package.toml).
                  A crate's own name is not a consumer of it: a crate whose program shares its name
                  (`line_editor`) still names itself in its manifest and its examples.

A wire literal is exempt (the architect's ruling of 2026-11-14 UTC, provisional spelling): bytes that
already sit on a wire or in built programs keep their spelling after the crate leaves, so an ELF
note owner string, a section name, a magic value, a console line head and a test asserting those
bytes may say `nife`. The line carries the marker `wire literal` as its last comment
(`// wire literal`, `# wire literal` or `/* wire literal */`), and the lint accepts it and prints
every marked line so the exemptions stay visible. An unmarked mention is refused as before, and
prose never names nife: it says "the owner string" or similar.

The program and package names are derived from the tree on every run, so the list cannot go stale.

Usage: helpers/lint_externalized_crates.py [--count] [--selftest]
  --count     print the hit count per listed crate and exit 0 (the measurement).
  --selftest  prove the rules against fixtures, with no tree.
"""
import os
import re
import sys

# Crates with hits that this lint found when it was added. Each entry is a ceiling, not a pardon:
# the crate may not gain a hit, and a crate that falls below its ceiling is told to lower it. An
# entry goes when its crate is clean, which the lint reports; until then it is the maintainer's list
# of crates that still describe their consumers.
PENDING = "a text or value hit in the crate on the day the lint was added; reword or remove it, then delete the entry"
WAIVERS = {
    "abi": (6, PENDING),
    "byte_sink_contract": (1, PENDING),
    "capability_manifest_note": (28, PENDING),
    "counter_frequency_contract": (1, PENDING),
    "current_cpu_contract": (3, PENDING),
    "domain_name_resolution_contract": (54, PENDING),
    "domain_name_system": (12, PENDING),
    "filesystem_contract": (20, PENDING),
    "filesystem_fixture": (18, PENDING),
    "globally_unique_identifier_partition_table": (21, PENDING),
    "grant_plan": (54, PENDING),
    "machine_statistics_contract": (1, PENDING),
    "program_argument_contract": (2, PENDING),
    "socket_contract": (2, PENDING),
    "user_mode_runtime": (5, PENDING),
    "interfaces_note": (6, PENDING),
    "subtree_scope": (1, PENDING),
}

URL_KEYS = ("repository", "homepage", "documentation")


def listed_crates(root):
    path = os.path.join(root, "crates", "externalization-list.txt")
    with open(path) as f:
        return [l.strip() for l in f if l.strip() and not l.startswith("#")]


def consumer_names(root):
    names = set()
    try:
        with open(os.path.join(root, "components", "Cargo.toml")) as f:
            text = f.read()
        names.update(re.findall(r'\[\[bin\]\]\s*\nname\s*=\s*"([^"]+)"', text))
    except OSError:
        pass
    for entry in sorted(os.listdir(root)):
        if os.path.isfile(os.path.join(root, entry, "Cargo.toml")) and entry not in (
            "crates", "components", "kernel", "xtask", "fuzz", "vendor", "target"
        ):
            names.add(entry)
    pkgdir = os.path.join(root, "packages")
    if os.path.isdir(pkgdir):
        for f in os.listdir(pkgdir):
            if f.endswith(".package.toml"):
                names.add(f[: -len(".package.toml")])
    names.add("basalt")
    # Single plain words are never consumer evidence: too many are ordinary English.
    return sorted(n for n in names if "_" in n or "-" in n or n == "basalt")


def compile_rules(consumers):
    nife = re.compile(r"(?<![A-Za-z0-9])nife(?![A-Za-z0-9])", re.I)
    nifeos = re.compile(r"(?<![A-Za-z0-9])nifeos(?![A-Za-z0-9])", re.I)
    cons = None
    if consumers:
        cons = re.compile(
            r"(?<![A-Za-z0-9_-])(?:" + "|".join(re.escape(n) for n in consumers) + r")(?![A-Za-z0-9_-])"
        )
    return nife, nifeos, cons


# The marker ends the line, as the last comment on it.
WIRE_LITERAL = re.compile(r"(?://|#|/\*)\s*wire literal(?:\s*\*/)?\s*$")


def scan_text(relpath, text, rules, marked=None):
    """The hits of one file. A line that ends in the marker is exempt and, if `marked` is a list,
    appended to it as (relpath, line number, text)."""
    nife, nifeos, cons = rules
    is_manifest = relpath.endswith("Cargo.toml")
    hits = []
    for n, line in enumerate(text.splitlines(), 1):
        if WIRE_LITERAL.search(line):
            if marked is not None:
                marked.append((relpath, n, line.strip()))
            continue
        if nifeos.search(line):
            key = line.split("=", 1)[0].strip()
            # A README badge row names the crate's own repository, which the template requires.
            is_badge = relpath.endswith("README.md") and line.startswith("[![")
            if not (is_manifest and key in URL_KEYS) and not is_badge:
                hits.append((relpath, n, "nifeos-word", line.strip()))
        if nife.search(line):
            hits.append((relpath, n, "nife-word", line.strip()))
        if cons and cons.search(line):
            hits.append((relpath, n, "consumer-name", line.strip()))
    return hits


def crate_files(cdir):
    for name in sorted(os.listdir(cdir)):
        p = os.path.join(cdir, name)
        if os.path.isfile(p) and (name == "Cargo.toml" or name.lower().startswith("readme")):
            yield p
    for dp, dn, fn in os.walk(os.path.join(cdir, "src")):
        dn.sort()
        for f in sorted(fn):
            yield os.path.join(dp, f)


def scan_tree(root, marked=None):
    consumers = consumer_names(root)
    result = {}
    for crate in listed_crates(root):
        rules = compile_rules([n for n in consumers if n != crate])
        cdir = os.path.join(root, "crates", crate)
        hits = []
        if os.path.isdir(cdir):
            for p in crate_files(cdir):
                try:
                    with open(p, encoding="utf-8") as f:
                        text = f.read()
                except (OSError, UnicodeDecodeError):
                    continue
                hits += scan_text(os.path.relpath(p, root), text, rules, marked)
        result[crate] = hits
    return result


def selftest():
    rules = compile_rules(["disk_surveyor", "core-tools"])
    t = lambda p, s: [h[2] for h in scan_text(p, s, rules)]
    assert t("a/README.md", "Used by nife.") == ["nife-word"]
    assert t("a/README.md", "NIFE rocks") == ["nife-word"]
    assert t("a/README.md", "nifefs and unife are other words") == []
    assert t("a/src/lib.rs", "// read by disk_surveyor") == ["consumer-name"]
    assert t("a/src/lib.rs", "// the disk_surveyor_x") == []
    assert t("a/src/lib.rs", "// part of core-tools") == ["consumer-name"]
    assert t("a/src/lib.rs", "// console and input") == []
    assert t("a/Cargo.toml", 'repository = "https://github.com/nifeos/x"') == []
    assert t("a/Cargo.toml", 'homepage = "https://github.com/nifeos/x"') == []
    assert t("a/Cargo.toml", 'description = "for nifeos"') == ["nifeos-word"]
    assert t("a/README.md", "see github.com/nifeos/x") == ["nifeos-word"]
    assert t("a/README.md", "[![check](https://github.com/nifeos/x/actions/workflows/ci.yml/badge.svg)](https://github.com/nifeos/x)") == []
    # A marked line is exempt; an unmarked one, or a marker that is not last, is not.
    assert t("a/src/lib.rs", 'const OWNER: &[u8] = b"nife"; // wire literal') == []
    assert t("a/src/lib.rs", "KEEP(*(.note.nife.*)) /* wire literal */") == []
    assert t("a/Cargo.toml", "x = 1 # wire literal") == []
    assert t("a/src/lib.rs", 'const OWNER: &[u8] = b"nife";') == ["nife-word"]
    assert t("a/src/lib.rs", "// nife, a wire literal") == ["nife-word"]
    assert t("a/src/lib.rs", "// wire literal nife") == ["nife-word"]
    assert t("a/src/lib.rs", "// read by disk_surveyor // wire literal") == []
    seen = []
    scan_text("a/src/lib.rs", 'b"nife"; // wire literal\nb"x";\n', rules, seen)
    assert seen == [("a/src/lib.rs", 1, 'b"nife"; // wire literal')], seen
    import tempfile

    with tempfile.TemporaryDirectory() as root:
        for d in ("crates/good/src", "crates/bad/src", "crates/disk_surveyor/src", "components"):
            os.makedirs(os.path.join(root, d))
        files = {
            "crates/externalization-list.txt": "# c\ngood\nbad\ndisk_surveyor\n",
            "components/Cargo.toml": '[[bin]]\nname = "disk_surveyor"\n',
            "crates/good/Cargo.toml": '[package]\nrepository = "https://github.com/nifeos/good"\n',
            "crates/good/README.md": "A standalone crate.\n",
            "crates/good/src/lib.rs": "//! Fine.\n",
            "crates/bad/Cargo.toml": '[package]\ndescription = "x"\n',
            "crates/bad/src/lib.rs": "//! Read by disk_surveyor in nife.\n",
            "crates/disk_surveyor/Cargo.toml": '[package]\nname = "disk_surveyor"\n',
            "crates/disk_surveyor/src/lib.rs": "//! use disk_surveyor::Survey;\n",
            "crates/good/src/note.rs": 'const OWNER: &[u8] = b"nife"; // wire literal\n',
        }
        for name, text in files.items():
            with open(os.path.join(root, name), "w") as f:
                f.write(text)
        marked = []
        found = scan_tree(root, marked)
        assert found["good"] == [], found
        assert [m[:2] for m in marked] == [("crates/good/src/note.rs", 1)], marked
        assert sorted(h[2] for h in found["bad"]) == ["consumer-name", "nife-word"], found
        assert found["disk_surveyor"] == [], found
    print("lint_externalized_crates selftest: ok")


def main(argv):
    if "--selftest" in argv:
        selftest()
        return 0
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    root = os.path.normpath(root)
    marked = []
    result = scan_tree(root, marked)
    if "--count" in argv:
        for crate, hits in result.items():
            print(f"{crate}|{len(hits)}")
        return 0
    failed = 0
    notes = []
    for crate, hits in result.items():
        ceiling, why = WAIVERS.get(crate, (0, ""))
        if len(hits) > ceiling:
            for path, n, rule, text in hits:
                print(f"lint: {path}:{n}: [{rule}] {text}", file=sys.stderr)
            failed += 1
        elif len(hits) < ceiling:
            notes.append(f"{crate} has {len(hits)} hits, below its ceiling of {ceiling}; lower or delete its WAIVERS entry")
    for crate in WAIVERS:
        if crate not in result:
            notes.append(f"{crate} is waived but not listed in crates/externalization-list.txt; delete its entry")
    if failed:
        print(
            "      An externalized crate stands alone and names no nife program, package or nife\n"
            "      itself. How to fix: reword the line for any reader (\"the consumer\"). A crate over\n"
            "      its WAIVERS ceiling gained a hit; all its hits are listed above.",
            file=sys.stderr,
        )
        return 1
    for n in notes:
        print(f"note: {n}")
    if marked:
        print(f"wire literals exempt by their marker ({len(marked)}):")
        for path, n, text in marked:
            print(f"  {path}:{n}: {text}")
    clean = sum(1 for c, h in result.items() if not h)
    print(f"externalized crates: {len(result)} listed, {clean} clean, {len(WAIVERS)} under a ceiling, none above it")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
