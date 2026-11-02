"""The table a ratifying pull request carries for a leaving crate's names, for `script/names`.

**Why this file exists.** A crate on the release track (milestones 813 to 820, the eight crates of
milestone 857 (crates released on their own)) leaves for its own repository. A ruling of 2026-10-26
(UTC) settles what happens to its names: once the crate leaves it is a third-party crate, and nife
keeps no record of a third-party crate's names. So a track crate has no names
record in the tree. Its `Name:` blocks stay in its source, censused like any other crate's, until
the crate breaks out, and the change that breaks it out strips them from the outgoing copy.

**Ratification is a pull request.** Its body carries this module's table, and it sets each block's
status to `ratified <date> (#PR)`, keeping any refusal the block already holds:

    python3 helpers/name_release.py --draft <crate>   # the table, from today's blocks
    python3 helpers/name_release.py --selftest

The table has one row per name, `| name | kind | path | status |`: the crate, each module, each
`pub` item and each Cargo feature. The status cell is a block's text without the `Name:`, or `no
block` where there is none. A feature has no block and no status to ratify, so its row always reads
`no block`. `script/crate-standalone` step 0 passes only when every `Name:` block in the crate is
ratified, and step 3 ignores the blocks, which the breaking-out change strips.

**Any crate under `crates/` is covered.** `--draft` takes any directory under `crates/` that holds a
`Cargo.toml`, which is every crate `script/crate-standalone` checks, and refuses a name that is no
such crate. It does not read `crate_release.MILESTONES`, so the contract crates that leave in the
order of the-contract-crates-leave-in-this-order can ratify without a roadmap block each.

Name: provisional, minted 2026-10-25 (UTC) by lane release-crates-names-ratified-first. A shared
helper under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the
record. `name_` for the family beside `name_rules.py`; `release` for the track it serves.
`crate_names` was considered and reads as every crate's.
"""

import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import crate_release  # noqa: E402

def _module_stem(rel):
    stem = os.path.splitext(os.path.basename(rel))[0]
    return os.path.basename(os.path.dirname(rel)) if stem in ("mod", "lib", "main") else stem


# --- the verbs ------------------------------------------------------------------------------------

def _tree():
    root = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True,
                          text=True, check=True).stdout.strip()
    listed = subprocess.run(["git", "-C", root, "ls-files", "-z", "--cached", "--others",
                             "--exclude-standard"], capture_output=True, text=True,
                            check=True).stdout
    paths = sorted(p for p in listed.split("\0") if p and os.path.isfile(os.path.join(root, p)))

    def read(rel):
        try:
            return open(os.path.join(root, rel), encoding="utf-8", errors="replace").read()
        except OSError:
            return None
    return paths, read


def rows(crate, paths, read):
    """`[(name, kind, path, status)]` for `crate` from its blocks, or None for a name that is no crate.

    `path` is relative to the crate. The status is the first line of a block's text, or `no block`.
    """
    import name_census
    prefix = f"crates/{crate}/"
    if f"{prefix}Cargo.toml" not in paths:
        return None
    records, _problems, _missing, unblocked = name_census.census(paths, read)
    status = {}
    for r in records:
        path = r["path"].split(":")[0]
        if path.startswith(prefix):
            status[(r["kind"], path, r["name"])] = r["text"].split("\n")[0]
    for row in unblocked:
        path, _, name = row.partition("::")
        if path.startswith(prefix):
            status[("item" if name else "module", path, name or _module_stem(path))] = "no block"
    order = {"crate": 0, "module": 1, "item": 2}
    out = [(name, kind, path[len(prefix):], text)
           for (kind, path, name), text in sorted(status.items(),
                                                  key=lambda k: (order.get(k[0][0], 3), k[0][1:]))
           if kind in order]
    manifest = read(prefix + "Cargo.toml") or ""
    if "[features]" in manifest:
        for f in re.findall(r"(?m)^([A-Za-z0-9_-]+)\s*=", manifest.split("[features]", 1)[1]
                            .split("\n[", 1)[0]):
            out.append((f, "feature", "Cargo.toml", "no block"))
    return out


# The page contract crates' family names, from design/naming.md ("Page contract crates"). A row
# for one of these names in one of these crates reads as pre-ruled, so a reviewer sees the rename
# the family rule already settles. `program_argument_contract` is absent from the content-length
# set: its `PAGE_BYTES` is a real page size and keeps the name.
PRE_RULED = {
    "current_cpu_contract": {"PAGE_BYTES": "PAGE_CONTENT_BYTES", "PAGE_VA": "PAGE_VIRTUAL_ADDRESS"},
    "environment_contract": {"PAGE_BYTES": "PAGE_CONTENT_BYTES"},
    "machine_statistics_contract": {"PAGE_BYTES": "PAGE_CONTENT_BYTES",
                                    "PAGE_VA": "PAGE_VIRTUAL_ADDRESS"},
}


def pre_ruled(crate, table):
    """`table` with the family rename noted on each row it settles."""
    ruled = PRE_RULED.get(crate, {})
    return [(name, kind, path,
             f"{text} Pre-ruled: rename to `{ruled[name]}`." if kind == "item" and name in ruled
             else text)
            for name, kind, path, text in table]


def draft(crate):
    """Print the table for `crate` from today's blocks, for the ratifying pull request's body."""
    paths, read = _tree()
    table = rows(crate, paths, read)
    if table is not None:
        table = pre_ruled(crate, table)
    if table is None:
        print(f"name_release: `{crate}` is not a crate under crates/", file=sys.stderr)
        return 1
    print("| name | kind | path | status |\n|---|---|---|---|")
    for name, kind, path, text in table:
        print(f"| `{name}` | {kind} | `{path}` | {text.replace('|', chr(92) + '|')} |")
    return 0


def selftest():
    block = "# 813. `fat`: a crate\n"
    crate = ("//! The crate.\n//!\n//! Name: ratified 2026-10-07 (#1806).\n\npub mod a;\n\n"
             "/// A volume.\n///\n/// Name: ratified 2026-11-02 (#1). Refused `Fs` (vague).\n"
             "pub struct Volume;\n/// Name: provisional.\npub fn new() {}\npub fn open() {}\n")
    module = "//! A module.\n\npub fn read() {}\n"
    manifest = '[package]\nname = "fat"\n\n[features]\nstd = []\n'
    base = {"design/roadmap/0813-fat.md": block, "crates/fat/src/lib.rs": crate,
            "crates/fat/src/a.rs": module, "crates/fat/Cargo.toml": manifest,
            "crates/other/src/lib.rs": "//! Name: provisional.\npub fn f() {}\n",
            "crates/other/Cargo.toml": '[package]\nname = "other"\n',
            "design/naming/convention-rulings.md": ""}

    def table(tree, name="fat"):
        return rows(name, sorted(tree), tree.get)

    got = {(n, k): (p, s) for n, k, p, s in table(base)}
    cases = [
        ("the crate and an item take their block's status",
         got[("fat", "crate")][1].startswith("ratified 2026-10-07")
         and got[("Volume", "item")][1].startswith("ratified 2026-11-02")),
        ("a block's refusal stays in the cell", "Refused `Fs`" in got[("Volume", "item")][1]),
        ("a provisional block reads provisional", got[("new", "item")][1] == "provisional."),
        ("an item or module with no block reads `no block`",
         got[("open", "item")][1] == "no block" and got[("a", "module")][1] == "no block"
         and got[("read", "item")][1] == "no block"),
        ("a feature has a row and no block", got[("std", "feature")] == ("Cargo.toml", "no block")),
        ("paths are relative to the crate", got[("a", "module")][0] == "src/a.rs"),
        ("a family name in a page crate reads as pre-ruled, and a real page size does not",
         [r[3] for r in pre_ruled("current_cpu_contract",
                                  [("PAGE_VA", "item", "src/lib.rs", "provisional.")])]
         == ["provisional. Pre-ruled: rename to `PAGE_VIRTUAL_ADDRESS`."]
         and pre_ruled("program_argument_contract",
                       [("PAGE_BYTES", "item", "src/lib.rs", "no block")])[0][3] == "no block"),
        ("a name that is no crate has no table", table(base, "nothing") is None),
        ("a crate off the release track has a table", table(base, "other") is not None),
        ("blanked blocks keep every line number",
         crate_release.strip_name_blocks(
             "//! A.\n//!\n//! Name: provisional. Long\n//! more.\n\nfn f() {}\n")
         == "//! A.\n\n\n\n\nfn f() {}\n"),
        ("no record file is read: a stale one changes nothing",
         table(dict(base, **{"design/roadmap/0857-crates-released-on-their-own/fat-names.md":
                             "Ratified: 2026-11-02 (#1).\n"})) == table(base)),
    ]
    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f"name_release selftest: {name}", file=sys.stderr)
    if not failed:
        print(f"name_release selftest: {len(cases)} cases pass")
    return 1 if failed else 0


if __name__ == "__main__":
    if sys.argv[1:] == ["--selftest"]:
        sys.exit(selftest())
    if len(sys.argv) == 3 and sys.argv[1] == "--draft":
        sys.exit(draft(sys.argv[2]))
    sys.exit("usage: name_release.py --selftest | --draft <crate>")
