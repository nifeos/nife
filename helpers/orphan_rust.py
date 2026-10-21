#!/usr/bin/env python3
"""Every tracked .rs file under a crate is declared by something that compiles.

    helpers/orphan_rust.py              # script/lint: fail on an undeclared file
    helpers/orphan_rust.py --list       # print every orphan, allowlisted or not, and the runtime
    helpers/orphan_rust.py --selftest   # fixtures; script/lint

Milestone 743 (no orphaned rust source), provisional number; the integrator mints it at merge.
The file name is provisional too, and calef has not ratified it. A shared python module under
`helpers/`, outside `script/names`' scope, so its provenance is this paragraph.

# WHY

rustc compiles only the files reachable through `mod` declarations from a crate root. A file nothing
declares is silently dead: no error, no warning, no test run. Commit d0b1ff821 dropped `mod proofs;`
and the timetable Kani proofs and their falsification records went stale with every gate green
(repaired by #1554). A test or proof file nothing compiles is a test that cannot fail, which is
fatal risk 3's whole subject (design/fatal-risks/README.md).

# WHAT IT CHECKS

A tracked `.rs` file whose nearest enclosing Cargo.toml directory exists must be reached from a
build target's module tree. Reached means a declaration exists, not that this build compiles it:
`#[cfg(kani)] mod`, `#[cfg(test)] mod`, `#[cfg(target_arch = ..)] mod` all count.

Roots: `[lib]`/`[[bin]]`/`[[test]]`/`[[bench]]`/`[[example]]` `path`, plus cargo's auto-discovered
`src/lib.rs`, `src/main.rs`, `src/bin/*.rs`, `src/bin/*/main.rs`, `build.rs`, and `*.rs` or `*/main.rs`
under `tests/`, `benches/`, `examples/`. Edges: `mod x;` (2018 `x.rs` or `x/mod.rs`, nested directories,
inline `mod a { mod b; }`), `#[path = ".."]` (also inside `cfg_attr`), and `include!`/`include_str!`
of a `.rs` file.

# WHAT IT CANNOT SEE (recorded where a reader meets it)

- A `mod` declared by a macro expansion (`mod $name;`) is not parsed. A file reached only that way
  reads as an orphan; allowlist it with the reason.
- A file under no Cargo.toml (compiled by a bare `rustc`, like helpers/kani-lint-shim/) is out of
  scope. Their count is printed by `--list`.
- A declaration that exists but sits in a file that is itself unreachable does not count (the walk
  starts at roots), which is the intended behavior.
- Cargo's `autobins = false` and friends are not honored: an auto-discovered path is a root anyway.

Plain parse, no cargo, so it stays cheap enough for the pre-push hook.
"""
import os
import re
import subprocess
import sys
import tempfile
import time
import tomllib

# Every entry is a finding: a file nothing declares, kept so the check can land green. One reason per
# line. An entry that stops being an orphan fails the check, so this list can only shrink honestly.
ALLOWLIST = {
    'vendor/talc/src/ext.rs': "upstream talc 5.1.1's own, vendored unchanged (vendor/README.md); no `mod ext` anywhere upstream",
    'vendor/talc/src/source/vheaps.rs': "upstream talc 5.1.1's own, vendored unchanged (vendor/README.md); upstream comments out its `mod` (`// pub mod vheaps;`)",
    'vendor/talc/src/source/vheaps/unix.rs': "upstream talc 5.1.1's own, vendored unchanged (vendor/README.md); upstream comments out its `mod`, as its parent's is",
    'vendor/talc/src/source/vheaps/win.rs': "upstream talc 5.1.1's own, vendored unchanged (vendor/README.md); upstream comments out its `mod`, as its parent's is",
    'vendor/talc/src/sync/unix.rs': "upstream talc 5.1.1's own, vendored unchanged (vendor/README.md); upstream comments out its `mod` (`// pub mod unix;`)",
    'vendor/talc/src/sync/win.rs': "upstream talc 5.1.1's own, vendored unchanged (vendor/README.md); upstream comments out its `mod` (`// pub mod win;`)",
}


RAW = re.compile(r'r#*"')
BRAW = re.compile(r'br#*"')
RAWOPEN = re.compile(r'b?r(#*)"')
CHAR = re.compile(r"'(\\.[^']*|[^\\'])'")
IDENT = re.compile(r'[A-Za-z0-9_]+')
NUM = re.compile(r'[0-9A-Za-z_.]+')


def tokens(text):
    """Yield (kind, value): 'id', 'str', or 'p' (one punctuation char). Skips comments, chars."""
    i, n = 0, len(text)
    out = []
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
        elif text.startswith('//', i):
            j = text.find('\n', i)
            i = n if j < 0 else j
        elif text.startswith('/*', i):
            depth, i = 1, i + 2
            while i < n and depth:
                if text.startswith('/*', i):
                    depth, i = depth + 1, i + 2
                elif text.startswith('*/', i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
        elif c == 'r' and RAW.match(text, i) or (
                c == 'b' and BRAW.match(text, i)):
            m = RAWOPEN.match(text, i)
            end = '"' + m.group(1)
            j = text.find(end, m.end())
            j = n if j < 0 else j
            out.append(('str', text[m.end():j]))
            i = j + len(end)
        elif c == '"' or (c == 'b' and text.startswith('b"', i)):
            i += 1 if c == '"' else 2
            buf = []
            while i < n and text[i] != '"':
                if text[i] == '\\':
                    buf.append(text[i + 1:i + 2])
                    i += 2
                else:
                    buf.append(text[i])
                    i += 1
            i += 1
            out.append(('str', ''.join(buf)))
        elif c == "'":
            # A char literal ('x', '\n', '{') or a lifetime ('a). Only the first has a closing quote
            # within a few characters.
            m = CHAR.match(text, i)
            i = m.end() if m else i + 1
        elif c.isalpha() or c == '_':
            m = IDENT.match(text, i)
            out.append(('id', m.group()))
            i = m.end()
        elif c.isdigit():
            i = NUM.match(text, i).end()
        else:
            out.append(('p', c))
            i += 1
    return out


def declarations(text):
    """Return a list of (kind, name_or_path, inline_dirs, explicit_paths) for one file.

    kind is 'mod' (a `mod x;`, with explicit_paths from #[path]) or 'include' (a path string).
    inline_dirs is the tuple of enclosing inline module names.
    """
    toks = tokens(text)
    found = []
    stack = []        # one entry per open brace: the inline module name or None
    pending = []      # #[path] strings seen since the last item boundary
    i, n = 0, len(toks)
    while i < n:
        k, v = toks[i]
        if (k, v) == ('p', '#') and i + 1 < n and toks[i + 1][1] in ('[', '!'):
            j = i + 1
            if toks[j][1] == '!':
                j += 1
            depth = 0
            start = j
            while j < n:
                if toks[j] == ('p', '['):
                    depth += 1
                elif toks[j] == ('p', ']'):
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            for m in range(start, j):
                if toks[m] == ('id', 'path') and m + 2 <= j and toks[m + 1] == ('p', '=') \
                        and toks[m + 2][0] == 'str':
                    pending.append(toks[m + 2][1])
            i = j + 1
            continue
        if k == 'id' and v == 'mod' and i + 2 < n and toks[i + 1][0] == 'id' \
                and toks[i + 2][1] in (';', '{'):
            name = toks[i + 1][1]
            if toks[i + 2][1] == ';':
                found.append(('mod', name, tuple(s for s in stack if s is not None), list(pending)))
                pending = []
            else:
                # An inline module with #[path] adds that directory instead of its name.
                stack.append(pending[0] if pending else name)
                pending = []
            i += 3
            continue
        if k == 'id' and v in ('include', 'include_str', 'include_bytes') and i + 3 < n \
                and toks[i + 1] == ('p', '!') and toks[i + 3][0] == 'str':
            found.append(('include', toks[i + 3][1], tuple(s for s in stack if s is not None), []))
            i += 4
            continue
        if (k, v) == ('p', '{'):
            stack.append(None)
            pending = []
        elif (k, v) == ('p', '}'):
            if stack:
                stack.pop()
            pending = []
        elif (k, v) == ('p', ';'):
            pending = []
        i += 1
    return found


def manifest_roots(manifest, doc, files):
    """Absolute-from-repo-root paths of every build-target root a Cargo.toml names or implies."""
    d = os.path.dirname(manifest)

    def j(*p):
        return os.path.normpath(os.path.join(d, *p))
    roots = set()
    if 'lib' in doc and 'path' in doc['lib']:
        roots.add(j(doc['lib']['path']))
    for sect in ('bin', 'test', 'bench', 'example'):
        for t in doc.get(sect, []):
            if 'path' in t:
                roots.add(j(t['path']))
    for fixed in ('src/lib.rs', 'src/main.rs', 'build.rs'):
        roots.add(j(fixed))
    for sub in ('src/bin', 'tests', 'benches', 'examples'):
        prefix = j(sub) + '/'
        for f in files:
            if f.startswith(prefix):
                rest = f[len(prefix):]
                if '/' not in rest or re.fullmatch(r'[^/]+/main\.rs', rest):
                    roots.add(f)
    return roots


def orphans(files, read, manifests):
    """files: tracked .rs paths (repo-relative, '/'); read(path) -> text. Returns (orphans, unscoped)."""
    fileset = set(files)
    roots = set()
    packages = []
    for m in manifests:
        try:
            doc = tomllib.loads(read(m))
        except tomllib.TOMLDecodeError:
            doc = {}
        if 'package' not in doc:
            continue    # a virtual workspace manifest scopes nothing, or it would scope the tree
        packages.append(m)
        roots |= manifest_roots(m, doc, files)
    roots &= fileset
    reached = set()
    # (file, is_mod_rs): a crate root, a mod.rs and a #[path] target all resolve children from
    # their own directory; any other file resolves them from a directory named for its stem.
    work = [(r, True) for r in sorted(roots)]
    seen = set()
    while work:
        f, mod_rs = work.pop()
        if f in seen:
            continue
        seen.add(f)
        reached.add(f)
        d = os.path.dirname(f)
        own = d if mod_rs else os.path.join(d, os.path.splitext(os.path.basename(f))[0])
        for kind, name, inline, paths in declarations(read(f)):
            if kind == 'include':
                if name.endswith('.rs'):
                    t = os.path.normpath(os.path.join(d, *inline, name))
                    if t in fileset:
                        work.append((t, True))
                continue
            if paths:
                base = d if not inline else os.path.join(own, *inline)
                for p in paths:
                    t = os.path.normpath(os.path.join(base, p))
                    if t in fileset:
                        work.append((t, True))
                continue
            base = os.path.join(own, *inline)
            for cand, mr in ((os.path.join(base, name + '.rs'), False),
                             (os.path.join(base, name, 'mod.rs'), True)):
                cand = os.path.normpath(cand)
                if cand in fileset:
                    work.append((cand, mr))
    mdirs = [os.path.dirname(m) for m in packages]
    scoped, unscoped = [], []
    for f in files:
        (scoped if any((f.startswith(md + '/') if md else True) for md in mdirs) else unscoped).append(f)
    return sorted(f for f in scoped if f not in reached), unscoped


def run(root, listing):
    os.chdir(root)
    tracked = subprocess.run(['git', 'ls-files', '-z'], capture_output=True, text=True, check=True
                             ).stdout.split('\0')
    manifests = sorted(f for f in tracked if f == 'Cargo.toml' or f.endswith('/Cargo.toml'))
    files = sorted(f for f in tracked if f.endswith('.rs') and os.path.exists(f))

    def read(p):
        with open(p, encoding='utf-8', errors='replace') as fh:
            return fh.read()
    return orphans(files, read, manifests) + (len(files), len(manifests))


def main(argv):
    if argv[1:] == ['--selftest']:
        return selftest()
    t0 = time.time()
    here = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
    orph, unscoped, nfiles, nman = run(here, argv[1:] == ['--list'])
    dt = time.time() - t0
    bad = [f for f in orph if f not in ALLOWLIST]
    stale = [f for f in ALLOWLIST if f not in orph]
    if argv[1:] == ['--list']:
        for f in orph:
            print(('allowlisted ' if f in ALLOWLIST else 'ORPHAN      ') + f)
        print(f'{len(unscoped)} .rs files under no Cargo.toml (out of scope); {dt:.2f}s')
    if bad or stale:
        print('lint: Rust source that no module tree declares (rustc never compiles it):', file=sys.stderr)
        for f in bad:
            print('  ' + f, file=sys.stderr)
        for f in stale:
            print(f'  {f}: allowlisted in helpers/orphan_rust.py but not an orphan; drop the entry',
                  file=sys.stderr)
        print('  Declare it (`mod x;`), delete it, or allowlist it with the reason in '
              'helpers/orphan_rust.py. Commit d0b1ff821 dropped `mod proofs;` and the proofs went stale '
              'silently. Milestone 743, provisional.', file=sys.stderr)
        return 1
    print(f'orphan source: {nfiles} .rs files in {nman} manifests, every one declared '
          f'({len(ALLOWLIST)} allowlisted), {dt:.2f}s')
    return 0


def selftest():
    fixtures = {
        'k/Cargo.toml': '[package]\nname="k"\n[[bin]]\nname="tool"\npath="tools/t.rs"\n',
        'k/src/lib.rs': '// mod ghost;\nmod reached;\n#[cfg(kani)]\nmod proofs;\nmod nested;\n'
                        '#[path = "elsewhere/odd.rs"]\nmod odd;\n'
                        'mod inline { pub mod deep; }\n'
                        'const S: &str = "mod stringy;";\n'
                        'include!("gen/inc.rs");\n',
        'k/src/reached.rs': 'mod child;\n',
        'k/src/reached/child.rs': '',
        'k/src/proofs.rs': '',
        'k/src/nested/mod.rs': 'mod leaf;\n',
        'k/src/nested/leaf.rs': '',
        'k/src/elsewhere/odd.rs': 'mod sib;\n',
        'k/src/elsewhere/sib.rs': '',
        'k/src/inline/deep.rs': '',
        'k/src/gen/inc.rs': '',
        'k/tools/t.rs': '',
        'k/tests/it.rs': 'mod common;\n',
        'k/tests/common/mod.rs': '',
        'k/src/ghost.rs': '',
        'k/src/stringy.rs': '',
        'k/src/reached/unlisted.rs': '',
        'k/benches/b.rs': '',
        'loose/free.rs': '',
        'Cargo.toml': '[workspace]\nmembers=["k"]\n',
    }
    orph, unscoped = orphans(sorted(f for f in fixtures if f.endswith('.rs')), lambda p: fixtures[p], ['k/Cargo.toml', 'Cargo.toml'])
    want = ['k/src/ghost.rs', 'k/src/reached/unlisted.rs', 'k/src/stringy.rs']
    ok = orph == want and unscoped == ['loose/free.rs']
    if not ok:
        print(f'orphan selftest: got {orph}, {unscoped}; wanted {want}', file=sys.stderr)
    # The tokenizer's two traps: braces in strings and chars, lifetimes.
    d = declarations("fn f<'a>(x: &'a str) { let c = '{'; let s = \"}\"; }\nmod m;\n")
    ok = ok and d == [('mod', 'm', (), [])]
    print('orphan selftest: %s' % ('ok' if ok else 'FAILED'))
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv))
