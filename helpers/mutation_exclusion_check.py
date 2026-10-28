#!/usr/bin/env python3
"""lint: every `exclude_re` entry in `.cargo/mutants.toml` names code that exists.

Part 2 of design/roadmap/0954-a-survivor-accepted-once-is-not-accepted-forever.md says
`script/lint` "checks each scoped exclusion" and fails "an entry whose function no longer exists
or whose named assertion is gone". Nothing in the tree read `exclude_re` before
lane/954-exclusion-check built this helper, so the record claimed a gate that did not exist: a
rename could strand an entry, the exclusion would stop matching, and the mutants it covered would
come back counted as new survivors with no signal anywhere.

    python3 helpers/mutation_exclusion_check.py check       # the real tree; script/lint
    python3 helpers/mutation_exclusion_check.py --selftest  # fixtures, no git, no cargo; script/lint

Exit 0: every entry passes. Exit 1: the failures, listed. Exit 2: bad input (the config does not
parse as TOML, or this module's own read of the array disagrees with tomllib's, which is this
module's bug and must not silently check the wrong entries).

THE GRAMMAR, read from the tool rather than guessed at. cargo-mutants 27.1.0 (the pin in
.cargo-mutants-version) matches each `exclude_re` against the mutant's whole name,
`<path>:<line>:<col>: replace <mutation>[ in <function path>]`. The tree's entries anchor both
ends, which makes the regex data with a fixed shape, and this check parses the shape instead of
interpreting regexes:

    ^<file>:\\d+:\\d+: replace <mutation>[ in <function path>]$    a scoped entry
    <module>::                                                    a module-path entry

A scoped entry's `<file>` is escaped-literal text (`registers\\.rs`); its line and column stay
`\\d+` because they move with every edit; `<function path>` is the function as the tool names it
(`receive_queue_mode`, `Command::word`, `Host<R, D>::reset`). A scoped entry with no ` in ` part
is a const item's mutant: cargo-mutants names it with no function, so the file is the whole scope
(what .cargo/mutants.toml's part 4 entries say in place). A module-path entry is unanchored and
names the head of a module path (`verification::`), which nothing else in the tree uses.

WHAT IS CHECKED.

- Every entry compiles as a regex and matches one of the two shapes above. Anything else fails: an
  entry this check cannot read must not pass by reading nothing
  (design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md).
- A scoped entry's file exists; and where it names a function, that function is declared in the
  file: the last `::` segment as a `fn`, and, on the two-segment `Type::fn` form, the type
  declared or implemented in the same file. That is the roadmap's claim: the entry's function
  pattern matches at least one function within the entry's path scope.
- A module-path entry's module is declared somewhere in the tree (`mod verification`).
- Where an entry's reason names a const assertion in the landed convention
  (`THE_*_SHARE_NO_BIT`, milestone 954 (a survivor accepted once is not accepted forever),
  part 4), the named const exists beside the code the entry excludes. An annotation
  `(registers.rs)` names the file for every name before it that no
  annotation has claimed yet, which is how the reasons write a list of names sharing one file; a
  name with no annotation lives in the entry's own file. A name that resolves nowhere fails lint.

COST. The config read, the entry files (about a dozen small files), and one walk of the tracked
`.rs` tree for `mod` declarations. No cargo, no tree build, a fraction of a second.

BUGS

- The function check is a declaration check, not a mutant check. It cannot see cargo-mutants'
  population: a function that exists but compiles into no test build passes here. The weekly
  census and the diff gate (helpers/mutation_diff_gate.py) see the real population.
- `fn`, type and `const` names are searched in the file's raw text, comments included, so a
  comment that keeps a name can keep a stale entry green.
- Only a function path's last two segments are checked, and the type only on the two-segment
  `Type::fn` form. Enclosing module segments are not resolved against the tree's module tree.
- A reason that names a const with no annotation while the const lives in another file fails, on
  purpose: write the annotation down.
- An exclude_re array with no entries fails rather than passing, so a merge that empties the
  array in a conflict resolution cannot silence this gate by giving it nothing to judge. If every
  exclusion genuinely leaves, this check leaves in the same pull request.

Name: provisional, minted 2026-10-28 (UTC) by lane/954-exclusion-check, for the lint section of
the same name. calef has not ratified it.
"""

import pathlib
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import PurePosixPath

CONFIG = ".cargo/mutants.toml"

# The scoped shape, anchored both ends. The path is escaped-literal: a bare character that is
# neither the `:` that starts the line and column nor an anchor, or a backslash escape such as
# `\\.`. The line and column stay `\\d+` because they move with every edit.
SCOPED = re.compile(
    r"\^"
    r"(?P<path>(?:[^\\:$^\n]|\\.)+?)"
    r":\\d\+:\\d\+:"
    r" replace .+? with .+?"
    r"(?: in (?P<function>.+?))?"
    r"\$"
)
# The module-path shape: unanchored, module segments and nothing else.
MODULE_PATH = re.compile(r"(?:[A-Za-z_][A-Za-z0-9_]*::)+")
IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")

# The const-assertion convention the part 4 exclusions name, and the `(file.rs)` annotation that
# says where a named const lives.
NAMED_ASSERTION = re.compile(
    r"\b(?P<name>THE_[A-Z0-9_]+)\b|\(\s*(?P<file>[A-Za-z0-9_./-]+?\.rs)\s*\)"
)

ARRAY = re.compile(r"(?ms)^exclude_re\s*=\s*\[(.*?)^\]")


def unescape(body):
    """The value of a TOML basic-string body, for the escapes a regex entry can carry."""
    out, i = [], 0
    while i < len(body):
        c = body[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        if i + 1 >= len(body) or body[i + 1] not in ('"', "\\"):
            raise ValueError(f"unsupported escape \\{body[i + 1:i + 2]}")
        out.append(body[i + 1])
        i += 2
    return "".join(out)


def array_strings(block):
    """(start, end, value) per string literal in an array's text, comment text ignored."""
    out = []
    i, quote, start = 0, None, None
    while i < len(block):
        c = block[i]
        if quote is not None:
            if quote == '"' and c == "\\":
                i += 2
                continue
            if c == quote:
                body = block[start + 1:i]
                value = unescape(body) if quote == '"' else body
                out.append((start, i + 1, value))
                quote = None
            i += 1
            continue
        if c == "#":
            nl = block.find("\n", i)
            if nl < 0:
                break
            i = nl + 1
            continue
        if c in "\"'":
            quote, start = c, i
        i += 1
    return out


def read_entries(raw):
    """(entry, reason, line) per exclude_re item, in file order.

    The reason is the raw text between this string and the one before it: the comment block above
    the entry. The items are cross-checked against tomllib's own read of the same array, so a
    string form the walk above misreads exits loudly instead of checking the wrong things.
    """
    try:
        listed = tomllib.loads(raw).get("exclude_re") or []
    except tomllib.TOMLDecodeError as err:
        sys.exit(f"mutation exclusions: {CONFIG} does not parse: {err}")
    block = ARRAY.search(raw)
    if block is None:
        if listed:
            sys.exit("mutation exclusions: tomllib reads an exclude_re array that this module's"
                     " textual read cannot find")
        return []
    entries, at = [], 0
    try:
        strings = array_strings(block.group(1))
    except ValueError as err:
        sys.exit(f"mutation exclusions: an exclude_re entry carries an escape this module does"
                 f" not read ({err})")
    for start, end, value in strings:
        opening = block.start(1) + start
        entries.append((value, raw[at:opening], raw.count("\n", 0, opening) + 1))
        at = block.start(1) + end
    if [e[0] for e in entries] != list(listed):
        sys.exit("mutation exclusions: this module's read of the exclude_re array disagrees with"
                 " tomllib's; failing rather than checking the wrong entries")
    return entries


def fn_declaration(name):
    return re.compile(r"\bfn\s+" + re.escape(name) + r"\b")


def type_declaration(base):
    """A type named `base`, declared or implemented: `struct Host`, or an `impl` header naming
    it, which is where an impl in a file beside its type's definition is caught."""
    return re.compile(
        r"\b(?:struct|enum|union|trait|type)\s+" + re.escape(base) + r"\b"
        r"|\bimpl\b[^{;]*\b" + re.escape(base) + r"\b"
    )


def mod_declaration(name):
    return re.compile(
        r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+" + re.escape(name) + r"\s*[;{]", re.M
    )


def const_declaration(name):
    return re.compile(r"\bconst\s+" + re.escape(name) + r"\b")


class Tree:
    """The `.rs` files a check may look at, each read once and cached."""

    def __init__(self, root, files):
        self.root = pathlib.Path(root)
        self.files = list(files)
        self._texts = {}

    def text(self, rel):
        if rel not in self._texts:
            path = self.root / rel
            self._texts[rel] = path.read_text(errors="replace") if path.is_file() else None
        return self._texts[rel]

    def declares(self, regex):
        """The first file whose text matches, or None."""
        for rel in self.files:
            text = self.text(rel)
            if text is not None and regex.search(text):
                return rel
        return None


def named_assertions(reason):
    """(name, file or None) per const assertion an entry's reason names. An annotation binds to
    every name before it that no annotation has claimed yet: the reasons write
    `THE_A_SHARE_NO_BIT and THE_B_SHARE_NO_BIT (registers.rs)` for two consts in one file."""
    out = []
    for m in NAMED_ASSERTION.finditer(reason):
        if m.group("name"):
            out.append([m.group("name"), None])
        elif out:
            for item in out:
                if item[1] is None:
                    item[1] = m.group("file")
    return out


def assertion_failures(reason, scope_rel, tree, where):
    failures = []
    for name, annotated in named_assertions(reason):
        if annotated is not None:
            found = False
            for rel in tree.files:
                if rel == annotated or PurePosixPath(rel).name == annotated:
                    source = tree.text(rel)
                    if source is not None and const_declaration(name).search(source):
                        found = True
                        break
            if not found:
                failures.append(
                    f"{where}: the reason names const {name} in {annotated}, and no file of that"
                    f" name declares it; the named assertion is gone")
        else:
            source = tree.text(scope_rel)
            if source is None or not const_declaration(name).search(source):
                failures.append(
                    f"{where}: the reason names const {name} with no file annotation, and"
                    f" {scope_rel} does not declare it")
    return failures


def check_scoped(scoped, text, reason, line, tree):
    where = f"{CONFIG}:{line}"
    failures = []
    rel = scoped.group("path").replace("\\.", ".")
    if "\\" in rel:
        return [f"{where}: the entry's path scope carries an escape this check reads as `\\.`"
                f" only: {text}"]
    source = tree.text(rel)
    if source is None:
        return [f"{where}: the entry's file {rel} is not in the tree; the exclusion is dead"]
    function = scoped.group("function")
    if function is not None:
        segments = function.split("::")
        name = segments[-1]
        if IDENTIFIER.fullmatch(name) is None:
            failures.append(f"{where}: the entry's function path ends in `{name}`, which is not"
                            f" an identifier: {text}")
        elif not fn_declaration(name).search(source):
            failures.append(f"{where}: no fn {name} in {rel}; the entry's function is gone")
        # The type half of the path, on the `Type::fn` form the landed entries use. On longer
        # paths the second-to-last segment can be a module, which this file-level check cannot
        # resolve; see BUGS.
        if len(segments) == 2:
            base = segments[0].split("<")[0].strip()
            if IDENTIFIER.fullmatch(base) and not type_declaration(base).search(source):
                failures.append(f"{where}: no type {base} declared or implemented in {rel}; the"
                                " entry's function path names a type that is gone")
    failures += assertion_failures(reason, rel, tree, where)
    return failures


def check_entry(entry, tree):
    """The failures for one (entry, reason, line)."""
    text, reason, line = entry
    where = f"{CONFIG}:{line}"
    try:
        re.compile(text)
    except re.error as err:
        return [f"{where}: the entry does not compile as a regex ({err}): {text}"]
    scoped = SCOPED.fullmatch(text)
    if scoped is not None:
        return check_scoped(scoped, text, reason, line, tree)
    if MODULE_PATH.fullmatch(text) is not None:
        mod = text.split("::")[0]
        if tree.declares(mod_declaration(mod)) is None:
            return [f"{where}: no mod {mod} anywhere in the tree; the module-path entry matches"
                    " nothing"]
        return []
    return [f"{where}: the entry matches no shape this check reads (an anchored"
            f" ^<file>:\\d+:\\d+: replace ...$ scope, or a `module::` path): {text}"]


def tracked_rs_files(root):
    out = subprocess.run(["git", "ls-files", "*.rs"], cwd=str(root), capture_output=True,
                         text=True, check=True).stdout.split()
    return [f for f in out if not f.startswith(("vendor/", "patches/"))]


def run(tree):
    """(entries, failures) for one tree. A `None` entries list means the array is gone or empty,
    which fails: a gate handed nothing to judge passes by checking nothing."""
    raw = (tree.root / CONFIG).read_text(errors="replace")
    entries = read_entries(raw)
    if not entries:
        return None, [f"{CONFIG}: the exclude_re array holds no entries, so this gate judges"
                      " nothing. If every exclusion genuinely leaves, this check leaves in the"
                      " same pull request."]
    failures = []
    for entry in entries:
        failures += check_entry(entry, tree)
    return entries, failures


REMEDY = """
An exclusion that names nothing excludes nothing: the mutants it covered run again and land in
the census as survivors. Re-point the entry at the name that replaced the one it named, or delete
the entry in the same pull request that deleted the code, moving its reason to whatever replaces
it. A named const assertion (`THE_*_SHARE_NO_BIT`) must be declared in the file the reason
annotates, or beside the excluded code when the reason annotates nothing.
"""


def selftest():
    """Each rule against fixtures: no git, no cargo, no tree."""

    def tree_with(config, files):
        tmp = tempfile.TemporaryDirectory()
        root = pathlib.Path(tmp.name)
        (root / ".cargo").mkdir(parents=True)
        (root / CONFIG).write_text(config)
        for rel, body in files.items():
            path = root / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(body)
        tracked = sorted(p.relative_to(root).as_posix() for p in root.rglob("*.rs"))
        return run(Tree(root, tracked))[1]

    def config(*entries):
        body = "exclude_re = [\n"
        for reason, entry in entries:
            body += f"    # {reason}\n    '{entry}',\n"
        return body + "]\n"

    def scoped(path, mutation, function=None):
        tail = f" in {function}" if function else ""
        return f"^{path}:\\d+:\\d+: replace {mutation}{tail}$"

    LIB = ("pub struct Thing;\n"
           "impl Thing {\n"
           "    pub fn compose(a: u32, b: u32) -> u32 { a | b }\n"
           "}\n")
    BITS = "pub const THE_FIELDS_SHARE_NO_BIT: () = assert!(1 + 2 == 3);\n"
    good_fn = scoped("crates/x/src/lib\\.rs", "\\| with \\^", "Thing::compose")
    good_reason = "disjoint per THE_FIELDS_SHARE_NO_BIT (registers.rs)"
    files = {"crates/x/src/lib.rs": LIB, "crates/x/src/registers.rs": BITS}

    # The whole landed shape set passes: a function-shaped entry with generics, its named
    # assertion in the annotated file, a file-scoped const-item entry, and the module paths.
    ok = tree_with(
        config(
            (good_reason, good_fn),
            ("the identity", scoped("crates/x/src/regs\\.rs", "<< with >>")),
            ("matched as a module path", "tests::"),
        ),
        dict(files, **{
            "crates/x/src/regs.rs": "pub const WIDE: u32 = 1 << 6;\n",
            "crates/x/src/other.rs": "#[cfg(test)]\nmod tests {\n}\n",
        }))
    assert ok == [], ok

    # The function is gone: renamed away, or the whole file with it.
    failures = tree_with(config((good_reason, good_fn)),
                         dict(files, **{"crates/x/src/lib.rs": LIB.replace("compose", "assembled")}))
    assert len(failures) == 1 and "no fn compose" in failures[0], failures
    failures = tree_with(config((good_reason, good_fn)),
                         {"crates/x/src/registers.rs": BITS})
    assert len(failures) == 1 and "crates/x/src/lib.rs is not in the tree" in failures[0], failures

    # The type half of the path is gone, with the fn still there.
    failures = tree_with(config((good_reason, good_fn)),
                         {"crates/x/src/lib.rs": "pub fn compose(a: u32) -> u32 { a }\n",
                          "crates/x/src/registers.rs": BITS})
    assert len(failures) == 1 and "no type Thing" in failures[0], failures

    # The landed generic form: `Host<R, D>::reset` against `struct Host` and an impl header.
    ok = tree_with(
        config(("disjoint", scoped("crates/x/src/host\\.rs", "\\| with \\^",
                                   "Host<R, D>::reset"))),
        {"crates/x/src/host.rs": "pub struct Host<R>(R);\n"
                                 "impl<R> Host<R> {\n"
                                 "    fn reset(&mut self) {}\n"
                                 "}\n"})
    assert ok == [], ok

    # A module-qualified path checks the function only; a module segment is not a type.
    ok = tree_with(
        config(("kani harness", scoped("crates/x/src/lib\\.rs", "\\| with \\^",
                                       "sub::Thing::compose"))),
        {"crates/x/src/lib.rs": "mod sub;\n" + LIB})
    assert ok == [], ok

    # A module-path entry whose module is nowhere.
    failures = tree_with(config(("matched as a module path", "interleavings::")), files)
    assert len(failures) == 1 and "no mod interleavings" in failures[0], failures

    # The named assertion: present in the annotated file, gone from it, and annotated for a file
    # that does not exist.
    failures = tree_with(config((good_reason, good_fn)),
                         {"crates/x/src/lib.rs": LIB,
                          "crates/x/src/registers.rs": "pub const SOMETHING_ELSE: u32 = 1;\n"})
    assert len(failures) == 1 and "THE_FIELDS_SHARE_NO_BIT in registers.rs" in failures[0], failures
    annotated_away = config(("disjoint per THE_FIELDS_SHARE_NO_BIT (nowhere.rs)", good_fn))
    failures = tree_with(annotated_away, files)
    assert len(failures) == 1 and "no file of that name declares it" in failures[0], failures

    # A list of names sharing one annotation: the annotation binds to both, so both must be in
    # the named file.
    two_names = config(("per THE_A_SHARE_NO_BIT and THE_B_SHARE_NO_BIT (bits.rs)", good_fn))
    both = ("pub const THE_A_SHARE_NO_BIT: () = ();\n"
            "pub const THE_B_SHARE_NO_BIT: () = ();\n")
    ok = tree_with(two_names, dict(files, **{"crates/x/src/bits.rs": both}))
    assert ok == [], ok
    failures = tree_with(two_names,
                         dict(files, **{"crates/x/src/bits.rs": both.replace(
                             "pub const THE_B_SHARE_NO_BIT", "pub const THE_OTHER")}))
    assert len(failures) == 1 and "THE_B_SHARE_NO_BIT in bits.rs" in failures[0], failures

    # No annotation: the const must live in the entry's own file.
    unannotated = config(("asserted beside the code, THE_C_SHARE_NO_BIT", good_fn))
    ok = tree_with(unannotated, dict(files, **{"crates/x/src/lib.rs": LIB +
                                               "pub const THE_C_SHARE_NO_BIT: () = ();\n"}))
    assert ok == [], ok
    failures = tree_with(unannotated, files)
    assert len(failures) == 1 and "with no file annotation" in failures[0], failures

    # A shape the check does not read fails instead of passing, and so does an entry that is not
    # a regex at all, and an array with nothing in it.
    failures = tree_with(config(("what?", "^.*$")), files)
    assert len(failures) == 1 and "matches no shape" in failures[0], failures
    failures = tree_with(config(("broken", "^([")), files)
    assert len(failures) == 1 and "does not compile" in failures[0], failures
    failures = tree_with("exclude_re = []\n", files)
    assert len(failures) == 1 and "holds no entries" in failures[0], failures

    # The reason belongs to the entry above it: a name in the second entry's comment block fails
    # at the second entry's line, not the first's.
    two = ("exclude_re = [\n"
           "    '" + scoped("crates/x/src/regs\\.rs", "<< with >>") + "',\n"
           "    # disjoint per THE_FIELDS_SHARE_NO_BIT\n"
           "    '" + good_fn + "',\n"
           "]\n")
    failures = tree_with(two, dict(files, **{"crates/x/src/regs.rs": "pub const WIDE: u32 = 1 << 6;\n"}))
    assert len(failures) == 1 and ":4: the reason names const" in failures[0], failures


def main(argv):
    if argv[1:] == ["--selftest"]:
        selftest()
        print("mutation exclusions: the selftest passes")
        return 0
    if argv[1:] != ["check"]:
        sys.exit("usage: mutation_exclusion_check.py check | --selftest")
    root = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True,
                          text=True, check=True).stdout.strip()
    entries, failures = run(Tree(root, tracked_rs_files(root)))
    if failures:
        print("lint: exclude_re entries in .cargo/mutants.toml that name no code:", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        print(REMEDY, file=sys.stderr, end="")
        return 1
    print(f"mutation exclusions: {len(entries)} entries, every one naming code that exists")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
