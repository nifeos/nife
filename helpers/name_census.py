"""Every name `script/names` tracks, out of any tree, for `script/names` and `script/metrics`.

**Why this file exists.** `script/names` reads the working tree and `script/metrics` reads a
`git ls-tree` at each week's commit, and until 2026-10-18 (UTC) each enumerated the named things
itself. `script/names` grew items, modules, directories, documents and the naming rules, and the
metrics copy still counted four kinds: 325 names for 2026W42 against 1,793, so the chart missed most
of the naming review. Now both pass a tree to `census()` here, and only the sourcing differs: a list
of paths and a function that reads one. `script/metrics` also fails when the current week's
`names_total` disagrees with `script/names`' total, so the sourcing cannot drift unseen either.

**What each caller supplies.** `paths` is every file in the tree, repository-relative. `read(path)`
returns the file's text, or None when the tree has no such file. Nothing here opens a file or runs
git, which is what lets a historical tree be counted without a checkout.

**`missing` is the names whose block is absent or malformed.** A surface that must carry a block
and does not is still a named thing, so `script/metrics` counts it toward `names_total` and toward
no status (the "No provenance block" band). On a tree that passes `script/names --check` it is zero.

**`unblocked` is every module, document and `pub` item with no block, each found in the tree, not
by its block.** Every `.rs` file under a `src/` directory is a module (`is_module_file`), every
document directly under `design/`, `notes/` or `briefs/` is a document (`is_top_document`; one in a
subdirectory is already a `--check` failure without its block), and every `pub` function, method,
constant, static, type and trait in a module is an item (`pub_items`; calef, 2026-10-18 UTC). So
the count is a fact of the tree, and writing a block moves a name from `unblocked` to a status
without changing the total. Counting only the names that carried a block would make the total a
count of annotation, rising whenever a block is written on code that already existed.
`helpers/module_block_ratchet.py` holds the unblocked list to a baseline that only shrinks, so new
code arrives with its block. A module or document is listed by its path, an item as
`path::name`. A private item, or one outside a `src/` directory, still counts where it carries a
block.

Name: provisional, minted 2026-10-18 (UTC) by lane maintainer/metrics-names-series. A shared helper
under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the record.
It takes `name_` for the family beside `name_provenance.py`; `census` because it counts a whole
tree, where `name_provenance` parses one block. calef has not named it.

BUGS. `_document_block` still reads a document's block from a README paragraph opening
`` `stem` Name: ``, until `helpers/document_name_migrate.py --apply`, run once milestone 929's lanes
have merged, moves the last one into its document; then that path goes.
"""

import collections
import os
import re

import name_crossings
import name_provenance
import name_rules

# Where a program's file lives. `user/src/` is only in history: milestone 175 (split `user/`:
# `components/` for services, `fixtures/` for test and benchmark programs) replaced it with the other
# two on 2026-09-13, and `script/metrics` counts each week from the tree it had.
PROGRAM_DIRS = ("components/src", "fixtures/src", "user/src")
_RECORD_STEM = re.compile(r"^\d")
# Trees whose `.rs` files are not this project's modules: vendored and patched upstream code, and
# code quoted inside documents.
_NOT_MODULES = ("vendor/", "patches/", "notes/", "design/", "briefs/")
_PACKAGE_NAME = re.compile(r'^\s*name\s*=\s*"([^"]+)"', re.M)

# Why a header-shaped line is not the block, phrased for whoever has to fix it. The tokens are
# `name_provenance`'s; the wording is the gate's.
STRAY_WHY = {
    name_provenance.STRAY_MARKUP:
        "markdown sits between the comment marker and `Name:`, which the parse does not skip",
    name_provenance.STRAY_INDENT:
        "it is indented past the single space the parse allows after the comment marker",
    name_provenance.STRAY_MARKER:
        "it uses a different comment marker from the one this surface's block is read at",
    name_provenance.STRAY_SECOND:
        "it is a second block below the one that was read, and nothing reads past the first",
}

# An exception to §75 (directories under `design/` and `notes/` carry provenance in their own
# README), from 2026-10-06 (UTC). Each entry under notes/skills/ is an Agent Skills directory whose
# SKILL.md name the specification fixes, so the entry has no README of its own. calef ratified all six entry names in one ruling, which notes/skills/README.md
# records beside the directory's own block, and judged a README per entry the gate's fault ("SKILL.md
# holds the information"). So a directory DIRECTLY under notes/skills/ that holds a SKILL.md is
# covered by the parent's block. helpers/agents-index-check.py fails an entry with no row there.
SKILLS_DIR = "notes/skills"


def _surfaces(paths, read):
    """`(kind, name, path, prefix)` for every crate, program, module, script and package."""
    pathset = set(paths)
    crates, out, bins = set(), [], {}
    for path in paths:
        parts = path.split("/")
        if parts[0] == "crates" and len(parts) > 2:
            crates.add(parts[1])
        elif len(parts) == 2 and parts[0] == "script":
            out.append(("script", parts[1], path, "#"))
    for crate in sorted(crates):
        out.append(("crate", crate, f"crates/{crate}/src/lib.rs", "//!"))
    for directory in PROGRAM_DIRS:
        depth = directory.count("/") + 2
        found = {}
        for path in paths:
            if not path.startswith(directory + "/"):
                continue
            parts = path.split("/")
            if len(parts) == depth and path.endswith(".rs"):
                found.setdefault(parts[-1][:-3], path)
            elif len(parts) == depth + 1 and parts[-1] == "main.rs":
                found[parts[-2]] = path
        if not found:
            continue
        # A program is a `[[bin]]` (calef's tiering, 2026-10-17 UTC). A file another program
        # includes through `mod` or `#[path]` is a module of that program.
        manifest = os.path.dirname(directory) + "/Cargo.toml"
        if manifest not in bins:
            bins[manifest] = name_crossings.bin_paths(read(manifest) or "")
        for name in sorted(found):
            # A directory program's block is in `main.rs` even when a `<name>.rs` sits beside it.
            path = found[name]
            rel = path[len(directory) + 1:]
            kind = "program" if name_crossings.is_program(rel, bins[manifest]) else "module"
            out.append((kind, name, path, "//!"))
    for path in paths:
        parts = path.split("/")
        if (parts[-1] != "Cargo.toml" or len(parts) == 1 or parts[0] in ("crates", "vendor")
                or any(p.startswith(".") or p == "target" for p in parts[:-1])):
            continue
        hit = _PACKAGE_NAME.search(read(path) or "")
        if hit:
            out.append(("package", hit.group(1), path, "#"))
    return out, pathset


def _includer(kind, path, paths, read):
    """For a program-directory file that is not a `[[bin]]`, the program that includes it."""
    if kind != "module":
        return None
    directory = os.path.dirname(path)
    name = os.path.splitext(os.path.basename(path))[0]
    sources = {os.path.basename(p)[:-3]: read(p) or "" for p in paths
               if os.path.dirname(p) == directory and p.endswith(".rs")}
    return name_crossings.includer_of(name, sources)


def _stray_problems(kind, name, path, prefix, text):
    """One problem per header-shaped line that is not the block (milestone 283 (one provenance
    block per file, in the spelling the parse reads)), and per refusal
    the parse cannot reach (2026-09-18), because either would leave a ruling invisible."""
    out = []
    for number, line, why in name_provenance.strays(text, prefix):
        if prefix == "//!" and line.lstrip().startswith("///"):
            continue    # an item's own block, judged with the `item` kind
        out.append(f"{kind} {name}: {path}:{number} reads as a provenance header and is not the "
                   f"block that was read, because {STRAY_WHY[why]}\n      {line.strip()}")
    for number, line in name_provenance.refusals_outside(text, prefix):
        out.append(f"{kind} {name}: {path}:{number} records a refusal the parse cannot read: it is "
                   f"outside the `Name:` block, so `script/names --refused` will never show it. Move "
                   f"it into the block, or move the block to the end of the comment.\n      "
                   f"{line.strip()}")
    return out


def is_module_file(path):
    """True for a `.rs` file under a `src/` directory outside `_NOT_MODULES`: a module, blocked or not.

    `tests/`, `benches/`, `examples/` and `build.rs` are out: each is its own Cargo target that a
    reader reaches by running it, not by `use`. One of those that carries a block is still counted.
    """
    return (path.endswith(".rs") and not path.startswith(_NOT_MODULES)
            and (path.startswith("src/") or "/src/" in path))


DOCUMENT_ROOTS = ("design", "notes", "briefs")


def is_top_document(path):
    """True for a document directly under `design/`, `notes/` or `briefs/`, blocked or not.

    Such a document's block is a paragraph of its own opening `Name:` (`_document_block`). It does
    not inherit the README's directory block the way a subdirectory's documents do (§75), because
    that block is about the word `notes` (or `design`, `briefs`), which `script/names` does not
    count. One with no block is `unblocked` rather than a `script/names --check` failure, held by
    the module-block ratchet instead.
    """
    parts = path.split("/")
    return (len(parts) == 2 and parts[0] in DOCUMENT_ROOTS and parts[1].endswith(".md")
            and parts[1] != "README.md" and not _RECORD_STEM.match(parts[1]))


def pub_items(text):
    """The name of every `pub` item in one Rust file, in order, a name once per item.

    `pub` exactly: `pub(crate)` and its kin are not public. `mod` is out because a module is
    counted as its file, and `macro_rules!` cannot be `pub`. The grammar is `name_provenance`'s, so
    an item a block documents and an item this finds are the same thing. Known misses, all rare
    here: an item a macro generates, and a `pub` line inside a block comment or a raw string, which
    this counts.
    """
    out = []
    for line in text.split("\n"):
        if not line.lstrip().startswith("pub "):
            continue
        m = name_provenance._ITEM.match(line)
        if m and m.group(1) not in ("mod", "macro_rules!"):
            out.append(m.group(2))
    return out


def _rust_records(paths, read, surface_files, problems):
    """`(records, unblocked)`: every item and module block, and every module and `pub` item with none."""
    out, unblocked = [], []
    for rel in paths:
        if not rel.endswith(".rs") or rel.startswith(("vendor/", "patches/")):
            continue
        text = read(rel)
        if text is None:
            continue
        module = is_module_file(rel) and rel not in surface_files
        # Matched by name within the file, as a multiset: a `pub fn new` in each of three impls is
        # three items, and a block on one of them leaves two.
        items = collections.Counter(pub_items(text) if is_module_file(rel) else [])
        if ("Name:" not in text and "Provisional name" not in text
                and "Name provisional" not in text):
            if module:
                unblocked.append(rel)
            unblocked.extend(f"{rel}::{name}" for name in items.elements())
            continue
        for it in name_provenance.rust_items(text):
            for number, line, why in it["strays"]:
                problems.append(f"item in {rel}:{number} reads as a provenance header and is not "
                                f"the block that was read, because {STRAY_WHY[why]}\n      {line}")
            for number in it["ad_hoc"]:
                problems.append(f"item in {rel}:{number} marks its name provisional in prose, which "
                                f"the worklist cannot see; write `/// Name: provisional, <who, "
                                f"when>.` instead")
            if it["block"] is None:
                continue
            if it["name"] is None:
                problems.append(f"item in {rel}:{it['line']}: a `Name:` block on a doc comment "
                                f"that documents nothing the parse can name")
                continue
            out.append(("item", it["name"], f"{rel}:{it['line']}", it["block"], it["what"]))
            items[it["name"]] -= 1
        unblocked.extend(f"{rel}::{name}" for name in items.elements())
        if rel in surface_files:
            continue
        # A module's own `//!` block. Strays are read over `//!` lines only: a `///` header belongs
        # to an item, and a `//` note beside code (xtask records subcommand rulings so) is prose.
        own = "\n".join(line for line in text.split("\n") if not line.lstrip().startswith("///"))
        for _number, line, why in name_provenance.strays(own, "//!"):
            if not line.lstrip().startswith("//!"):
                continue
            problems.append(f"module in {rel}: a line reads as a provenance header and is not the "
                            f"block that was read, because {STRAY_WHY[why]}\n      {line.strip()}")
        found = name_provenance.block(text, "//!")
        if found is not None:
            stem = os.path.splitext(os.path.basename(rel))[0]
            if stem in ("mod", "lib", "main"):
                stem = os.path.basename(os.path.dirname(rel))
            out.append(("module", stem, rel, found, "module"))
        elif module:
            unblocked.append(rel)
    return out, unblocked


def _document_block(path, stem, legacy, read, problems):
    """`(text, legacy)`: a document's block, from its own `Name:` paragraph or its README's.

    The document's own paragraph is the rule (calef, 2026-10-24 UTC). A README paragraph opening
    `` `stem` Name: `` is the older home, still read until the sweep after milestone 929 moves the
    last of them (helpers/document_name_migrate.py). Where both exist the document's is read first
    and the README's follows it, so a refusal written only in the README still reaches
    `script/names --refused`, and two statuses that disagree are a problem rather than a silent
    winner. `(None, False)` when neither exists.
    """
    mine = name_provenance.document_block(read(path) or "")
    old = legacy.get(stem)
    if mine is None:
        return old, old is not None
    if old is None:
        return mine, False
    ours, theirs = name_provenance.classify(mine)[0], name_provenance.classify(old)[0]
    if ours != theirs:
        problems.append(f"document {stem} ({path}): its own block says {ours or 'nothing parseable'} "
                        f"and its README paragraph says {theirs or 'nothing parseable'}; keep one, "
                        f"in the document (helpers/document_name_migrate.py lists both)")
    return f"{mine} {old}", True


def _document_records(markdown, pathset, read, problems):
    """Directories and documents under design/, notes/ and briefs/: `(records, missing, unblocked)`.

    A record is `(kind, name, path, block, what, legacy)`, `legacy` True when any of the block was
    read from a README paragraph rather than from the document.
    """
    out, missing, unblocked = [], 0, []
    for root in DOCUMENT_ROOTS:
        _own, stems, _loose = name_provenance.markdown_blocks(read(root + "/README.md") or "")
        for path in markdown:
            if os.path.dirname(path) != root or not is_top_document(path):
                continue
            stem = os.path.basename(path)[:-3]
            block, legacy = _document_block(path, stem, stems, read, problems)
            if block is None:
                unblocked.append(path)
            else:
                out.append(("document", stem, path, block, "document", legacy))
    for d in sorted({os.path.dirname(p) for p in markdown} - {"design", "notes", "briefs"}):
        if _RECORD_STEM.match(os.path.basename(d)):
            continue
        if os.path.dirname(d) == "design/roadmap/proposals":
            continue    # a proposal's appendix: its slug is a draft title, skipped like a block's
        if os.path.dirname(d) == SKILLS_DIR and d + "/SKILL.md" in pathset:
            continue    # covered by notes/skills/README.md's block; see SKILLS_DIR
        readme = d + "/README.md"
        roadmap = d == "design/roadmap" or d.startswith("design/roadmap/")
        names = [] if roadmap else sorted(
            os.path.basename(p)[:-3] for p in markdown
            if os.path.dirname(p) == d and os.path.basename(p) != "README.md"
            and not _RECORD_STEM.match(os.path.basename(p)))
        # A directory whose block is absent leaves its documents with none either, and they are
        # still named things: counted in `missing` beside the directory, never dropped.
        if readme not in pathset:
            problems.append(f"directory {d}/: no README.md, and §75 puts a directory's `Name:` "
                            f"block there")
            missing += 1 + len(names)
            continue
        own, stems, loose = name_provenance.markdown_blocks(read(readme) or "")
        for number, para in loose:
            problems.append(f"directory {d}/: {readme}:{number} reads as a provenance header the "
                            f"parse does not take (bold, a second directory block, or a prose "
                            f"`Provisional name`)\n      {para[:120]}")
        if own is None:
            problems.append(f"directory {d}/: {readme} carries no paragraph opening `Name:`")
            missing += 1 + len(names)
            continue
        out.append(("directory", d + "/", readme, own, "directory", False))
        if roadmap:
            continue
        for stem in names:
            path = f"{d}/{stem}.md"
            block, legacy = _document_block(path, stem, stems, read, problems)
            out.append(("document", stem, path, own if block is None else block, "document",
                        legacy))
        all_stems = {os.path.basename(p)[:-3] for p in markdown if os.path.dirname(p) == d}
        for stem in sorted(set(stems) - all_stems):
            problems.append(f"directory {d}/: {readme} records `{stem}`, which is not a file here")
    return out, missing, unblocked


def census(paths, read, british=None):
    """`(records, problems, missing, unblocked)` for one tree. See this module's docstring.

    A record is a dict with `kind`, `name`, `path`, `status`, `date`, `refused`, `text`, and `what`
    or `owner`. Names a rule on `name_rules.RULES_DOC` covers come back ratified, citing the rule.
    `british` is the word list the appendix rule's spelling check reads, `house_style`'s by default.
    """
    if british is None:
        try:
            import house_style
            british = house_style.EXTRA_BRITISH
        except ImportError:
            british = []
    paths = sorted(paths)
    problems, records, missing = [], [], 0
    surfaces, pathset = _surfaces(paths, read)
    surface_files = {path for kind, _n, path, _p in surfaces
                     if kind in ("crate", "program", "module")}
    markdown = [p for p in paths if p.endswith(".md") and p.startswith(("design/", "notes/",
                                                                         "briefs/"))]
    docs, missing, unblocked_docs = _document_records(markdown, pathset, read, problems)
    rust, unblocked = _rust_records(paths, read, surface_files, problems)
    unblocked += unblocked_docs
    for kind, name, path, text, what, legacy in [r + (False,) for r in rust] + docs:
        status, date, refused, problem = name_provenance.classify(text)
        if problem is not None:
            problems.append(f"{what} {name} ({path}): the block must start `ratified <date>`, "
                            f"`recorded (where)`, `provisional` or `unrecorded`, with a date on "
                            f"`ratified` and a citation on `recorded` (found {text[:48]!r})")
            missing += 1
            continue
        records.append({"kind": kind, "name": name, "path": path, "status": status, "date": date,
                        "refused": refused, "text": text, "what": what, "legacy": legacy})

    for kind, name, path, prefix in surfaces:
        source = read(path)
        if source is None:
            problems.append(f"{path} does not exist")
            missing += 1
            continue
        text = name_provenance.block(source, prefix)
        # Reported whether or not the block parsed: a file with no readable block is exactly the
        # file whose unreadable one is worth pointing at.
        problems.extend(_stray_problems(kind, name, path, prefix, source))
        if text is None:
            problems.append(f"{kind} {name}: no `Name:` block in {path}")
            missing += 1
            continue
        status, date, refused, problem = name_provenance.classify(text)
        if problem is not None:
            # The `recorded` example cites milestone 46 (rename the components for what they are,
            # and write down the naming rules), a real citation of the form.
            problems.append({
                name_provenance.NO_STATUS:
                    f"{kind} {name}: the block must start `ratified <date>`, `recorded (where)`, "
                    f"`provisional` or `unrecorded` (found {text[:48]!r})",
                name_provenance.NO_DATE:
                    f"{kind} {name}: `ratified` must carry a date (ratified 2026-08-04), "
                    f"because a ratification nobody can place is prose",
                name_provenance.NO_CITATION:
                    f"{kind} {name}: `recorded` must cite where (recorded (milestone 46)), "
                    f"because the claim IS that the reasoning is somewhere else",
            }[problem])
            missing += 1
            continue
        records.append({"kind": kind, "name": name, "path": path, "status": status, "date": date,
                        "refused": refused, "text": text,
                        "owner": _includer(kind, path, paths, read)})

    # **Names a rule covers are ratified, and cite the rule** (calef, 2026-10-17 UTC). Applied after
    # the blocks are read, so a covered name's own block still prints beside the rule.
    rules_text = read(name_rules.RULES_DOC)
    if rules_text is None:
        problems.append(f"{name_rules.RULES_DOC} does not exist, and the names it ratifies by rule "
                        f"are counted from it")
    else:
        _pulled, bad = name_rules.apply(records, name_rules.parse_rules(rules_text), markdown,
                                        lambda rel: read(rel) or "", british)
        problems.extend(bad)
    return records, problems, missing, unblocked
