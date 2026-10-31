"""Function length in code lines, read from source text the way clippy's `too_many_lines` reads it.

Milestone 888 (a ratchet on function length). `script/lint`'s lint ratchet reads clippy's saved diagnostics, which are
the authority, but no clippy pass here compiles every file in scope: `patches/`, `c_library/`, the
UEFI loader and others build only for nife's std targets or in a separate step. This module is the
text derivation that reaches them, and `helpers/lint_ratchet.py` compares it with clippy on every
function clippy reports, failing on any difference. That is
`helpers/rust_source.py`'s answer for the harness count, applied here: two derivations, and a gate
that says when they drift.

**The count is clippy's, line for line.** The scan below is written from memory of clippy's
`too_many_lines` and checked by measurement, not by reading its source: on 2026-10-13 (UTC), clippy
0.1.101 at a threshold of 0 over the lint ratchet's eight passes reported 11,217 functions in scope,
and the two agreed on all but 8, each a `fn` written inside a `macro_rules!` body (below). Take the body's source text, drop its outer braces, trim it, and count each line
that has code on it after leading whitespace and block comments are skipped. A line that opens with
`//` does not count; a line with code before a `//` does, and a `//` inside a string literal ends
the line's code there too. Because clippy reads source text, a block a `#[cfg]` removes from one
target still counts on every target, so the configurations agree on every function. Closures are not functions here: their lines count in the function around them.

**Which functions.** Every `fn` with a body in every tracked `.rs` file outside `vendor/`: the Rust
that runs on nife, host tools, `xtask`, build scripts, tests, benches, fixtures and items under
`#[cfg(test)]` alike. That is §283 (a function is held to 100 code lines by a lint ratchet), as
calef widened it on 2026-10-13 (UTC): "Yes, widen the gate". `in_scope` is the one scope function,
and the lint ratchet's gate reads it. A `fn` written inside a `macro_rules!` body is skipped. Clippy reports such a function at its span in
the macro, which this scan does not try to follow; all 8 today are under 10 code lines, and the
gate's comparison fails if one ever passes 100.

**A function's name** is its row key: `Type::name` inside an `impl` (the self type, generics
dropped), `Trait::name` for a trait's default body, `outer::name` for a `fn` inside a `fn`, and the
bare name otherwise. A name that occurs twice in one file (two `cfg` arms of one function, say)
takes ` #2`, ` #3` in source order.

Name: provisional, minted by milestone 888's lane on 2026-10-11 (UTC). A shared module under
`helpers/`, which `script/names` puts out of its own scope, so its provenance is this paragraph.
"""

import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rust_source as rs  # noqa: E402

THRESHOLD = 100

# --- the scope ------------------------------------------------------------------------------------

def in_scope(path):
    """Every Rust file outside `vendor/`, test code included."""
    return path.endswith('.rs') and not path.startswith('vendor/')


def scoped(files):
    """{path: (text, stripped)} for the in-scope files among `files`, a {path: text} map.

    `stripped` has comments and literals blanked, with line structure kept.
    """
    return {path: (files[path], rs.strip_non_code(files[path]))
            for path in sorted(files) if in_scope(path)}


# --- clippy's count -------------------------------------------------------------------------------

def code_lines(body):
    """`too_many_lines`' count over a body's source text, outer braces included in `body`."""
    if body.startswith('{') and body.endswith('}'):
        body = body[1:-1]
    count, in_comment = 0, False
    for line in body.strip().splitlines():
        code = False
        while True:
            line = line.lstrip()
            if not line:
                break
            if in_comment:
                i = line.find('*/')
                if i >= 0:
                    line = line[i + 2:]
                    in_comment = False
                    continue
            else:
                multi = line.find('/*')
                multi = len(line) if multi < 0 else multi
                single = line.find('//')
                single = len(line) if single < 0 else single
                code |= multi > 0 and single > 0
                if multi < single:
                    line = line[multi + 2:]
                    in_comment = True
                    continue
            break
        count += code
    return count


def code_line_set(body, first_line):
    """The 1-based line numbers `code_lines` counts, for a body starting on `first_line`."""
    lines, in_comment = set(), False
    for k, line in enumerate(body.split('\n')):
        if k == 0 and line.startswith('{'):
            line = line[1:]
        code = False
        while True:
            line = line.lstrip()
            if not line:
                break
            if in_comment:
                i = line.find('*/')
                if i >= 0:
                    line = line[i + 2:]
                    in_comment = False
                    continue
            else:
                multi = line.find('/*')
                multi = len(line) if multi < 0 else multi
                single = line.find('//')
                single = len(line) if single < 0 else single
                code |= multi > 0 and single > 0
                if multi < single:
                    line = line[multi + 2:]
                    in_comment = True
                    continue
            break
        if code:
            lines.add(first_line + k)
    # The closing brace's line is the body's own, not a line of code in it, unless code precedes it.
    last = first_line + body.count('\n')
    if last in lines and body.split('\n')[-1].strip() == '}':
        lines.discard(last)
    return lines


# --- finding the functions ------------------------------------------------------------------------

_FN = re.compile(r'\bfn\s+(r#)?([A-Za-z_]\w*)')
_QUALIFIERS = re.compile(
    r'(?:\bpub(?:\s*\([^)]*\))?|\bconst|\basync|\bunsafe|\bsafe|\bdefault|\bextern(?:\s+"[^"]*")?)\s*$')
_SCOPE = re.compile(r'\b(impl|trait|mod)\b')


def _match_brace(code, i):
    depth = 0
    for k in range(i, len(code)):
        c = code[k]
        if c == '{':
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0:
                return k
    return len(code) - 1


def _body_open(code, start):
    """Index of the `{` that opens a fn body, or None for a declaration ending in `;`."""
    depth = 0
    k = start
    while k < len(code):
        c = code[k]
        if c in '([':
            depth += 1
        elif c in ')]':
            depth -= 1
        elif depth == 0 and c == ';':
            return None
        elif depth == 0 and c == '{':
            return k
        k += 1
    return None


def _impl_name(header):
    """The self type of an `impl` or the name of a `trait`, from its header text."""
    header = ' '.join(header.split())
    m = re.match(r'(?:unsafe\s+)?trait\s+(\w+)', header)
    if m:
        return m.group(1)
    header = re.sub(r'^(?:unsafe\s+)?impl\s*', '', header)
    header = re.sub(r'\bwhere\b.*$', '', header)
    # Drop a leading generic parameter list.
    if header.startswith('<'):
        depth = 0
        for k, c in enumerate(header):
            depth += (c == '<') - (c == '>')
            if depth == 0:
                header = header[k + 1:]
                break
    if re.search(r'\bfor\b', header):
        header = re.split(r'\bfor\b', header, maxsplit=1)[1]
    header = header.strip().lstrip('&').strip()
    header = re.sub(r"^'\w+\s+", '', header)
    header = re.sub(r'^(?:mut|dyn)\s+', '', header)
    name = re.split(r'[<\s{(]', header, maxsplit=1)[0]
    return name.split('::')[-1] or '?'


def functions(text, code):
    """[(name, start_line, n, lines)] for every fn with a body in one file.

    `text` is the source and `code` the same with comments and literals blanked.
    `start_line` is the 1-based line of the item's first qualifier, which is where clippy's span
    starts; `n` is `code_lines` over the body; `lines` is `code_line_set`'s set.
    """
    # Regions that name a scope: impl and trait blocks, and macro_rules bodies, which are skipped.
    scopes = []  # (open, close, kind, name)
    for m in re.finditer(r'\bmacro_rules!\s*\w+\s*([{(\[])', code):
        o = m.end() - 1
        closer = {'{': '}', '(': ')', '[': ']'}[code[o]]
        depth = 0
        for k in range(o, len(code)):
            depth += (code[k] == code[o]) - (code[k] == closer)
            if depth == 0:
                scopes.append((o, k, 'macro', ''))
                break
    for m in _SCOPE.finditer(code):
        kind = m.group(1)
        if kind == 'mod':
            continue
        # Only an item-position `impl`/`trait`: at the start of a line after qualifiers.
        line_start = code.rfind('\n', 0, m.start()) + 1
        before = code[line_start:m.start()]
        if before.strip() not in ('', 'unsafe', 'pub', 'pub(crate)', 'pub unsafe', 'default'):
            continue
        o = _body_open(code, m.end())
        if o is None:
            continue
        name = _impl_name(code[m.start():o])
        scopes.append((o, _match_brace(code, o), kind, name))

    found, seen = [], {}
    fns = []
    for m in _FN.finditer(code):
        o = _body_open(code, m.end())
        if o is None:
            continue
        if any(a < m.start() < b for a, b, kind, _ in scopes if kind == 'macro'):
            continue
        close = _match_brace(code, o)
        fns.append((m, o, close))
    for m, o, close in fns:
        owner = None
        for a, b, kind, name in scopes:
            if kind != 'macro' and a < m.start() < b and (owner is None or a > owner[0]):
                owner = (a, name)
        outer = None
        for m2, o2, c2 in fns:
            if o2 < m.start() < c2 and (outer is None or o2 > outer[0]):
                outer = (o2, m2.group(2))
        name = m.group(2)
        if outer and (owner is None or outer[0] > owner[0]):
            name = f'{outer[1]}::{name}'
        elif owner:
            name = f'{owner[1]}::{name}'
        seen[name] = seen.get(name, 0) + 1
        if seen[name] > 1:
            name = f'{name} #{seen[name]}'
        # The item's first qualifier: walk back over `pub`, `unsafe`, `extern "C"` and the like.
        s = m.start()
        while True:
            q = _QUALIFIERS.search(text[:s])
            if not q or q.end() != s and text[q.end():s].strip():
                break
            if q.start() == s:
                break
            s = q.start()
        start_line = text.count('\n', 0, s) + 1
        body = text[o:close + 1]
        first = text.count('\n', 0, o) + 1
        found.append((name, start_line, code_lines(body), code_line_set(body, first)))
    return found


def measure(files):
    """{(path, name): (start_line, n, lines)} over the in-scope files of a {path: text} map."""
    out = {}
    for path, (text, code) in scoped(files).items():
        for name, start, n, lines in functions(text, code):
            out[(path, name)] = (start, n, lines)
    return out


def working_tree():
    """{path: text} for every tracked or untracked-unignored `.rs` file in the working tree."""
    names = subprocess.run(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard',
                            '*.rs'], capture_output=True, text=True, check=True).stdout
    out = {}
    for p in names.split('\0'):
        if p and os.path.exists(p):
            with open(p, encoding='utf-8', errors='replace') as f:
                out[p] = f.read()
    return out


if __name__ == '__main__':
    root = subprocess.run(['git', 'rev-parse', '--show-toplevel'], capture_output=True, text=True)
    os.chdir(root.stdout.strip())
    fns = measure(working_tree())
    for (path, name), (start, n, _lines) in sorted(fns.items(), key=lambda kv: -kv[1][1]):
        if n > THRESHOLD or '--all' in sys.argv:
            print(f'{n}\t{path}:{start}\t{name}')
