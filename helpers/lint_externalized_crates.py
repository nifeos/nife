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
  nifeos-word     `nifeos`, except on a Cargo.toml `repository`, `homepage` or `documentation` line.
  consumer-name   the name of a nife program (a `[[bin]]` of components/Cargo.toml or the directory
                  of a top-level program crate with a hyphen or underscore, so one plain word such as
                  `console` is never taken for a program) or of a package (packages/*.package.toml).

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
# of crates that still describe their consumers. block_roster is the one with a fix already in
# flight in another lane, and it is the one whose entry must go first.
PENDING = "a text or value hit in the crate on the day the lint was added; reword or remove it, then delete the entry"
WAIVERS = {
    "abi": (6, PENDING),
    "block_roster": (2, "its README and Kani proofs are being rewritten by another lane; remove this entry when that pull request merges"),
    "byte_sink_contract": (2, PENDING),
    "capability_manifest_note": (37, PENDING),
    "clock_contract": (5, PENDING),
    "counter_frequency_contract": (1, PENDING),
    "current_cpu_contract": (3, PENDING),
    "domain_name_resolution_contract": (54, PENDING),
    "domain_name_system": (12, PENDING),
    "entropy_contract": (2, PENDING),
    "filesystem_contract": (21, PENDING),
    "filesystem_fixture": (18, PENDING),
    "globally_unique_identifier_partition_table": (23, PENDING),
    "grant_plan": (103, PENDING),
    "machine_statistics_contract": (1, PENDING),
    "program_argument_contract": (2, PENDING),
    "socket_contract": (6, PENDING),
    "user_mode_runtime": (5, PENDING),
    "interfaces_note": (9, PENDING),
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


def scan_text(relpath, text, rules):
    nife, nifeos, cons = rules
    is_manifest = relpath.endswith("Cargo.toml")
    hits = []
    for n, line in enumerate(text.splitlines(), 1):
        if nifeos.search(line):
            key = line.split("=", 1)[0].strip()
            if not (is_manifest and key in URL_KEYS):
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


def scan_tree(root):
    rules = compile_rules(consumer_names(root))
    result = {}
    for crate in listed_crates(root):
        cdir = os.path.join(root, "crates", crate)
        hits = []
        if os.path.isdir(cdir):
            for p in crate_files(cdir):
                try:
                    with open(p, encoding="utf-8") as f:
                        text = f.read()
                except (OSError, UnicodeDecodeError):
                    continue
                hits += scan_text(os.path.relpath(p, root), text, rules)
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
    import tempfile

    with tempfile.TemporaryDirectory() as root:
        for d in ("crates/good/src", "crates/bad/src", "components"):
            os.makedirs(os.path.join(root, d))
        files = {
            "crates/externalization-list.txt": "# c\ngood\nbad\n",
            "components/Cargo.toml": '[[bin]]\nname = "disk_surveyor"\n',
            "crates/good/Cargo.toml": '[package]\nrepository = "https://github.com/nifeos/good"\n',
            "crates/good/README.md": "A standalone crate.\n",
            "crates/good/src/lib.rs": "//! Fine.\n",
            "crates/bad/Cargo.toml": '[package]\ndescription = "x"\n',
            "crates/bad/src/lib.rs": "//! Read by disk_surveyor in nife.\n",
        }
        for name, text in files.items():
            with open(os.path.join(root, name), "w") as f:
                f.write(text)
        found = scan_tree(root)
        assert found["good"] == [], found
        assert sorted(h[2] for h in found["bad"]) == ["consumer-name", "nife-word"], found
    print("lint_externalized_crates selftest: ok")


def main(argv):
    if "--selftest" in argv:
        selftest()
        return 0
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    root = os.path.normpath(root)
    result = scan_tree(root)
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
    clean = sum(1 for c, h in result.items() if not h)
    print(f"externalized crates: {len(result)} listed, {clean} clean, {len(WAIVERS)} under a ceiling, none above it")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
