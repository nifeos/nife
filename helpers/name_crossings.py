"""Which unratified names cross a Cargo package boundary, and what a program is, for `script/names`.

**Why this file exists.** On 2026-10-17 (UTC) calef ruled that a name crossing a crate boundary is
reviewed before a name internal to one crate: a crossing name is read by someone who does not own
the crate, so a wrong one costs more. Internal names still need ratifying later; the order is a
priority, not an exemption. `script/names --unratified` ranks the items and modules tier by it.

**The measure is a stated heuristic, not the compiler.** A compiler-backed answer (`cargo doc`
JSON, rust-analyzer) means building eight workspaces, minutes per run, and `script/names` is run
for a single name. `Reach` below reads text: a name crosses when another package that depends on
its own reaches it through the defining crate's path, and `helpers/name_checks.py` reports reach
by the same measure. A bare word never counts: matched that way, every crate's `Error` reached 38
packages. What it still misses is in `Reach`'s docstring and design/naming/rules-a-script-can-check.md.

**A program is a `[[bin]]`** (calef's tiering, 2026-10-17). A file in `components/src/` that another
program includes through `mod` or `#[path]` and that has no `[[bin]]` entry is a module of the
program that includes it, and ranks as one.

Name: provisional, minted 2026-10-17 (UTC) by the lane that ranked crossings. A shared helper under
`helpers/`, which `script/names` puts out of its own scope, so this paragraph is the record. It sits
beside `name_provenance.py` and `name_rules.py`; `boundaries` was considered and collides with
`packages.py`'s package boundaries, which are nife packages and not Cargo's. calef has not named it.
"""

import functools
import os
import re
import sys

_DEP_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


def bin_paths(manifest_text):
    """The `src/...` path of every `[[bin]]` a manifest declares.

    Read by line rather than with `tomllib`, which is 3.11 and later: `script/names <name>` runs
    this on every call and must work under the older `python3` a machine may put first on PATH.
    """
    out, in_bin = set(), False
    for line in manifest_text.split("\n"):
        s = line.strip()
        if s.startswith("["):
            in_bin = s == "[[bin]]"
        elif in_bin:
            m = re.match(r'path\s*=\s*"([^"]+)"', s)
            if m:
                out.add(m.group(1))
    return out


def is_program(rel_in_src, bins):
    """True when `src/<rel>` (a file, or `<dir>/main.rs`) is a declared binary."""
    return "src/" + rel_in_src in bins


def includer_of(name, sources):
    """The file stem that declares `name` through `mod` or `#[path]`, from `{stem: text}`, or None."""
    pattern = re.compile(rf'#\[path\s*=\s*"{re.escape(name)}\.rs"\]|^\s*(?:pub\s+)?mod\s+{re.escape(name)}\s*;',
                         re.M)
    for stem in sorted(sources):
        if stem != name and pattern.search(sources[stem]):
            return stem
    return None


def parse_manifest(text):
    """`(package name, lib identifier, set of dependency package names)`, name None for a workspace."""
    import tomllib    # here and not at the top: only the ranking needs it (3.11 and later)
    doc = tomllib.loads(text)
    name = doc.get("package", {}).get("name")
    ident = doc.get("lib", {}).get("name") or (name or "").replace("-", "_")
    tables = [doc]
    tables += list(doc.get("target", {}).values())
    deps = set()
    for table in tables:
        for key in _DEP_TABLES:
            for dep, spec in table.get(key, {}).items():
                deps.add(spec.get("package", dep) if isinstance(spec, dict) else dep)
    return name, ident, deps


def package_of(path, dirs):
    """The package name owning `path`: the longest directory in `dirs` that is a prefix of it."""
    best = None
    for d in dirs:
        if (d == "" or path.startswith(d + "/")) and (best is None or len(d) > len(best)):
            best = d
    return dirs[best] if best is not None else None


def declaration(lines, header):
    """The declaration line under the doc run that starts at 1-based `header`, or ''."""
    for line in lines[header - 1:]:
        s = line.strip()
        if s and not s.startswith(("//", "#[", "#![")):
            return line
    return ""


def is_pub(lines, header, what):
    """Whether the item whose doc run starts at 1-based 'header' is declared `pub` with no scope."""
    if what == "variant":
        return True
    return re.match(r"pub\b(?!\s*\()", declaration(lines, header).strip()) is not None


_RAW = re.compile(r'(?:b|c)?r(#*)"')
_CHAR = re.compile(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^\\'\n])'")
# Text that cannot open a comment or a literal: anything but `/`, a quote or `b`/`r`/`c`, and those
# three letters too unless they could begin a raw or byte string (a word start followed by `"`, `#`
# or `r`). `strip` jumps over it in one match instead of one step per character.
_PLAIN = re.compile(r"(?:[^/\"'bcr]|(?<=\w)[bcr]|[bcr](?![\"#r]))*")
_ATTRIBUTE = re.compile(r"#!?\[")
_VISIBILITY = re.compile(r"^(?:pub(?:\s*\([^)]*\))?\s+)?(?:unsafe\s+)?")
_MOD = re.compile(r"mod\s+(\w+)")
_BLOCK_EDGE = re.compile(r"[{};\n]")
_WORD = re.compile(r"[A-Za-z_]\w*")
_IMPL = re.compile(r"impl\b(?:\s*<.*?>(?=\s*[\w&(]))?\s*(?:!?\s*[\w:]+(?:<.*>)?\s+for\s+)?"
                   r"(?:&\s*(?:'\w+\s+)?(?:mut\s+)?)?(?:dyn\s+)?([\w:]+)", re.S)


@functools.lru_cache(maxsize=None)
def strip(text):
    """`text` with comments and literals blanked to spaces, newlines kept, so lines still line up.

    A word in a comment or a string is not a read, and a brace in one is not a block. The result is
    kept per text, so a file read by every census in one run is lexed once.
    """
    out, i, n = [], 0, len(text)

    def blank(a, b):
        out.append("\n".join(" " * len(part) for part in text[a:b].split("\n")))

    def word_start(k):
        return k == 0 or not (text[k - 1].isalnum() or text[k - 1] == "_")

    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif c in "brc" and word_start(i) and _RAW.match(text, i):
            m = _RAW.match(text, i)
            j = text.find('"' + m.group(1), m.end())
            j = n if j < 0 else j + 1 + len(m.group(1))
            out.append('"')
            blank(i + 1, j)
            i = j
        elif c == '"' or (c in "bc" and text.startswith('"', i + 1) and word_start(i)):
            j = i + (1 if c == '"' else 2)
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            j = min(j + 1, n)
            out.append('"')
            blank(i + 1, j)
            i = j
        elif c == "'" and _CHAR.match(text, i):
            m = _CHAR.match(text, i)
            out.append("'")
            blank(i + 1, m.end())
            i = m.end()
        else:
            j = _PLAIN.match(text, i + 1).end()
            out.append(text[i:j])
            i = j
    return "".join(out)


@functools.lru_cache(maxsize=None)
def _without_attributes(header):
    """A block header with its `#[...]` and `#![...]` attributes removed, brackets nested."""
    while True:
        m = _ATTRIBUTE.search(header)
        if not m:
            return header.strip()
        depth, j = 0, m.end() - 1
        while j < len(header):
            depth += {"[": 1, "]": -1}.get(header[j], 0)
            j += 1
            if depth == 0:
                break
        header = header[:m.start()] + " " + header[j:]


@functools.lru_cache(maxsize=None)
def encloser(header):
    """`("impl", type)`, `("mod", name)`, `("extern", None)` or `("body", None)` for a block's header.

    The same header text recurs across a tree (`impl Default for X`, a bare `{`), so the answer is
    kept per header.
    """
    h = _VISIBILITY.sub("", _without_attributes(header))
    if h.startswith("impl"):
        m = _IMPL.match(h)
        return "impl", (m.group(1).split("::")[-1] if m else None)
    m = _MOD.fullmatch(h)
    if m:
        return "mod", m.group(1)
    if h.startswith("extern"):
        return "extern", None    # foreign declarations, reached as free items would be
    if h.startswith("trait"):
        return "mod", None
    return "body", None


def enclosers(stripped):
    """`{1-based line: (encloser, ...)}`: every block open at the start of each line, outermost first."""
    stack, stmt, line, out = [], 0, 1, {1: ()}
    current = ()    # the tuple for the blocks now open, rebuilt only when a block opens or closes
    for m in _BLOCK_EDGE.finditer(stripped):
        ch = m.group(0)
        if ch == "\n":
            line += 1
            out[line] = current
            continue
        if ch == "{":
            stack.append(encloser(" ".join(stripped[stmt:m.start()].split())))
            current = tuple(stack)
        elif ch == "}" and stack:
            stack.pop()
            current = tuple(stack)
        stmt = m.end()
    return out


class Reach:
    """Which packages read each `pub` name of each package, measured by the defining crate's path.

    The one implementation of a read: `script/names --unratified` ranks by it and
    `helpers/name_checks.py` reports reach by it. A package Q reads a name of package P
    only when Q depends on P, and then only through P's path, in comments and strings blanked:
    `p::a::name`, a `use p::...;` listing it, `m::name` for an `m` imported from `p`, or any word
    of a file holding a glob import from `p`. A member (a method or associated constant) is read
    when its `impl`'s type is, or when a file of Q naming `p` calls it as `.name(`, `::name(` or
    `::NAME`. `$crate::name` in a file that exports a macro is read by `$crate`, and every name in
    a file another package includes with `#[path]` is read by `#path`. A bare word is never a read:
    matched that way, every crate's `Error` reached 38 packages.
    """

    def __init__(self, manifests, sources):
        self.dirs, self.info = {}, {}
        for d, text in manifests.items():
            name, ident, deps = parse_manifest(text)
            if name:
                self.dirs[d], self.info[name] = name, (ident, deps)
        self.sources = sources
        self.stripped = {p: strip(t) for p, t in sources.items()}
        self._enclosers = {}
        words = {p: set(_WORD.findall(t)) for p, t in self.stripped.items()}
        by_package = {}
        for path in sources:
            by_package.setdefault(package_of(path, self.dirs), []).append(path)
        self.paths_reached, self.calls = {}, {}
        for q, (_ident, deps) in self.info.items():
            for p in deps:
                if p == q or p not in self.info:
                    continue
                ident = self.info[p][0]
                for path in by_package.get(q, []):
                    if ident not in words[path]:
                        continue
                    got, glob = self._reached(self.stripped[path], ident)
                    for w in (words[path] if glob else got):
                        self.paths_reached.setdefault(p, {}).setdefault(w, set()).add(q)
                    text = self.stripped[path]
                    for w in re.findall(r"(?:\.|::)\s*([A-Za-z_]\w*)\s*(?:\(|::\s*<)", text):
                        self.calls.setdefault(p, {}).setdefault(w, set()).add(q)
                    for w in re.findall(r"::\s*([A-Z_][A-Z0-9_]*)\b", text):
                        self.calls.setdefault(p, {}).setdefault(w, set()).add(q)
        for p, files in by_package.items():
            for path in files:
                if "macro_export" not in sources[path]:
                    continue
                for m in re.finditer(r"\$crate((?:\s*::\s*\w+)+)", sources[path]):
                    for w in _WORD.findall(m.group(1)):
                        self.paths_reached.setdefault(p, {}).setdefault(w, set()).add("$crate")
        self.pathed = set()
        for path, text in sources.items():
            for m in re.finditer(r'#\[path\s*=\s*"([^"]+)"\]', text):
                target = os.path.normpath(os.path.join(os.path.dirname(path), m.group(1)))
                if package_of(target, self.dirs) != package_of(path, self.dirs):
                    self.pathed.add(target)

    @staticmethod
    def _reached(text, ident):
        """`(words, glob)`: what one file reaches of crate `ident` by a path, and whether it globs."""
        got, glob, aliases = set(), False, {ident}
        for m in re.finditer(rf"\buse\s+(?:::)?{re.escape(ident)}\b([^;]*);", text):
            glob = glob or "*" in m.group(1)
            found = set(_WORD.findall(m.group(1))) - {"self", "super", "crate", "as"}
            got |= found
            aliases |= found
        alias = "|".join(sorted(map(re.escape, aliases), key=len, reverse=True))
        for m in re.finditer(rf"\b(?:{alias})((?:\s*::\s*[A-Za-z_]\w*)+)", text):
            got |= set(_WORD.findall(m.group(1)))
        return got, glob

    def package(self, path):
        return package_of(path, self.dirs)

    def stack(self, path, line):
        if path not in self._enclosers:
            self._enclosers[path] = enclosers(self.stripped[path])
        return self._enclosers[path].get(line, ())

    def readers(self, path, line, name):
        """The sorted packages (or `$crate`, `#path`) that read the item declared at `path:line`."""
        p = self.package(path)
        if p is None:
            return []
        if path in self.pathed:
            return ["#path"]
        stack = self.stack(path, line)
        if any(kind == "body" for kind, _ in stack):
            return []      # inside a function, a macro or a struct body: nothing outside sees it
        impl = next((t for kind, t in reversed(stack) if kind == "impl"), "")
        reached = self.paths_reached.get(p, {})
        if impl == "":
            return sorted(reached.get(name, ()))
        if impl is None:
            return ["?impl"]   # an impl header this parse could not read: assume it is read
        return sorted(set(reached.get(impl, ())) | self.calls.get(p, {}).get(name, set()))


def crossings(cands, manifests, sources, reach=None):
    """`{candidate key: (defining package, sorted reading packages)}` for each candidate that crosses.

    `cands` is `[(key, name, file, what, header_line)]` with `what` an item kind or `module`.
    `manifests` is `{directory: manifest text}`; `sources` is `{path: text}` of every Rust file.
    The measure is `Reach`'s. A candidate must also be visible: an item declared `pub` with no
    scope, or a module some file of its package declares `pub mod`.
    """
    reach = reach or Reach(manifests, sources)
    by_package = {}
    for path in sources:
        by_package.setdefault(reach.package(path), []).append(path)
    out = {}
    for key, name, file, what, header in cands:
        owner = reach.package(file)
        if owner is None or owner not in reach.info:
            continue
        lines = sources[file].split("\n")
        if what == "module":
            if not any(re.search(rf"\bpub\s+mod\s+{re.escape(name)}\b", reach.stripped[p])
                       for p in by_package.get(owner, [])):
                continue
            readers = sorted(reach.paths_reached.get(owner, {}).get(name, ()))
        else:
            if not is_pub(lines, header, what):
                continue
            number = next((header + i for i, l in enumerate(lines[header - 1:])
                           if l.strip() and not l.strip().startswith(("//", "#[", "#!["))), header)
            readers = reach.readers(file, number, name)
        if readers:
            out[key] = (owner, readers)
    return out


def _selftest():
    lib = '[package]\nname = "lib-a"\n'
    app = '[package]\nname = "app"\n[dependencies]\nlib-a = { path = "../lib_a" }\n'
    other = '[package]\nname = "other"\n'
    manifests = {"lib_a": lib, "app": app, "other": other}
    sources = {
        "lib_a/src/lib.rs": "/// Name: provisional.\npub fn exported() {}\n\n/// Name: provisional.\n"
                            "pub(crate) fn hidden() {}\n\n/// Name: provisional.\nfn private() {}\n\n"
                            "/// Name: provisional.\npub fn unused() {}\n\npub mod open;\nmod closed;\n",
        "lib_a/src/open.rs": "", "lib_a/src/closed.rs": "",
        "app/src/main.rs": "use lib_a::exported;\nfn main() { exported(); lib_a::hidden(); }\n",
        "other/src/lib.rs": "fn x() { exported(); unused(); }\n",
    }
    cands = [("exported", "exported", "lib_a/src/lib.rs", "function", 1),
             ("hidden", "hidden", "lib_a/src/lib.rs", "function", 4),
             ("private", "private", "lib_a/src/lib.rs", "function", 7),
             ("unused", "unused", "lib_a/src/lib.rs", "function", 10),
             ("open", "open", "lib_a/src/open.rs", "module", 1)]
    got = crossings(cands, manifests, sources)
    assert got == {"exported": ("lib-a", ["app"])}, got       # `other` names it without depending on lib-a
    sources["app/src/main.rs"] += "use lib_a::open;\n"
    assert crossings(cands, manifests, sources)["open"] == ("lib-a", ["app"])
    assert "private" not in got and "hidden" not in got, "a non-pub item cannot cross"

    manifests["app2"] = app.replace('"app"', '"app2"')
    sources["app2/src/main.rs"] = "// lib_a\nfn exported() {}\nfn main() { x.hidden(); }\n"
    assert crossings(cands, manifests, sources)["exported"] == ("lib-a", ["app"]), \
        "the crate's name and the item's name in one file are not a reference"

    # The path measure: a module imported from the crate reaches through it, a glob reaches every
    # word of its file, and an item inside a function body is reached by nothing outside.
    lib_b = "pub mod wire {\n    pub const FRAME: u8 = 1;\n}\npub fn outer() {\n    pub fn inner() {}\n}\n"
    reach = Reach({"lib_b": '[package]\nname = "lib_b"\n',
                   "user": '[package]\nname = "user"\n[dependencies]\nlib_b = "1"\n',
                   "globber": '[package]\nname = "globber"\n[dependencies]\nlib_b = "1"\n'},
                  {"lib_b/src/lib.rs": lib_b,
                   "user/src/main.rs": "use lib_b::wire;\nfn main() { wire::FRAME; inner(); }\n",
                   "globber/src/main.rs": "use lib_b::*;\nfn main() { outer(); }\n"})
    assert reach.readers("lib_b/src/lib.rs", 2, "FRAME") == ["user"]
    assert reach.readers("lib_b/src/lib.rs", 4, "outer") == ["globber"]
    assert reach.readers("lib_b/src/lib.rs", 5, "inner") == [], "a body's item is unreachable"

    manifest = '[[bin]]\nname = "net_stack"\npath = "src/net_stack.rs"\n'
    bins = bin_paths(manifest)
    assert is_program("net_stack.rs", bins)
    assert not is_program("socket_table.rs", bins), "a #[path]-included file is not a program"
    host = {"net_stack": '#[path = "socket_table.rs"]\nmod socket_table;\n', "socket_table": ""}
    assert includer_of("socket_table", host) == "net_stack"
    assert includer_of("net_stack", host) is None

    # The lexer and the block stack, held to what the character-at-a-time scan returned. Each case
    # is a way the jump over plain text could swallow a delimiter: a `b`, `r` or `c` that opens a
    # literal and one inside a word, a lifetime beside a char literal, a nested comment, a comment
    # that spans lines. Line counts must survive, since callers index by line.
    lexed = [
        ('let a = "x { y"; // c { \nfn b() {}\n', 'let a = "      ;        \nfn b() {}\n'),
        ('let r = r#"a "q" {"#; let s = br"b{"; let t = c"z"; /* a /* nested { */ { */ let u = 1;\n',
         'let r = "           ; let s = "     ; let t = "   ;                          let u = 1;\n'),
        ("fn f<'a>(x: &'a str) { let c = '{'; let d = '\\''; let e = b'x'; }\n",
         "fn f<'a>(x: &'a str) { let c = '  ; let d = '   ; let e = b'  ; }\n"),
        ('abc rbc crab bar "s" cr"raw" b"bytes" xr"notraw"\n',
         'abc rbc crab bar "   "       "        xr"       \n'),
        ('/* multi\nline\ncomment */ pub fn k() {}\n', '        \n    \n           pub fn k() {}\n'),
    ]
    for source, want in lexed:
        assert strip(source) == want, (source, strip(source))
        assert strip(source) is strip(source), "the lexed text is kept per file"
        assert strip(source).count("\n") == source.count("\n")
    nest = strip("pub mod m {\n    impl Foo {\n        fn f() {\n        }\n    }\n}\nimpl Bar for Baz {\n}\n")
    assert enclosers(nest) == {
        1: (), 2: (("mod", "m"),), 3: (("mod", "m"), ("impl", "Foo")),
        4: (("mod", "m"), ("impl", "Foo"), ("body", None)), 5: (("mod", "m"), ("impl", "Foo")),
        6: (("mod", "m"),), 7: (), 8: (("impl", "Baz"),), 9: ()}, enclosers(nest)
    assert encloser("#[cfg(test)] #[derive(Debug)] mod tests") == ("mod", "tests")
    assert encloser("pub(crate) unsafe impl Send for X") == ("impl", "X")
    print("name_crossings: selftest ok")


if __name__ == "__main__":
    if sys.argv[1:] == ["--selftest"]:
        _selftest()
    else:
        sys.exit("usage: name_crossings.py --selftest")
