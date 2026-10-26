"""The parent-index check: §282 (when a file is split, the parent becomes an index), held to a
baseline that only falls.

calef ruled §282 on 2026-10-11 (UTC), ruling fork 1 of #1959, the cut of milestone 872 (the
kernel's user-mode file is split along its seams): a parent that has been split holds module docs,
`mod` lines, `use` and re-exports, and nothing that can grow into a feature. This is that rule's
gate. It copies `helpers/file_length_ratchet.py`, §266 (a Rust source file stays under 2,000
lines)'s ratchet, on purpose: a committed list that only shrinks, a comparison against the merge
base, and a selftest that runs before the check.

    python3 helpers/parent_index.py --check          # what script/lint runs
    python3 helpers/parent_index.py --list           # every parent, and each item it holds
    python3 helpers/parent_index.py --bank           # lower the baseline to the tree; never raises
    python3 helpers/parent_index.py --selftest       # the rules, as fixtures
    python3 helpers/parent_index.py --initialize           # write the baseline anew (§282's lane only)

Name: provisional, minted 2026-10-11 by lane/split-parent-is-an-index, as is the baseline's,
`design/parent-index-baseline.tsv`, and the mark's wording. A shared python module under
`helpers/`, which `script/names` puts out of its own scope, so its provenance is this paragraph.
calef names things; expect these to change.

**What a parent is.** A Rust file that declares at least one child module out of line (`mod x;`,
any visibility, any `cfg`), other than a child named `tests`, `proofs` or `verification`. That one
definition covers the three shapes §282 names: a `foo.rs` beside a `foo/` directory, a `mod.rs`, and
a crate root that declares children. A file whose only children are its own tests or Kani harnesses
has had them moved out, not its code split, so it is not a parent. `verification` is the other name
the tree gives a harness module, and `.cargo/mutants.toml` excludes both files by name.

**What an index holds.** Inner docs (`//!`) and inner attributes (`#![..]`), `mod x;` lines, `use`
and `pub use` (with `extern crate`), and an inline module whose body holds only those, as
`std::collections` writes `pub mod hash_map { pub use super::hash::map::*; }`. Comments anywhere.

**What else may stay** (§282's exception, measured on 2026-10-11 and argued in the decision):

- An item of at most SMALL (3) code lines, when the parent holds at most HANDFUL (8) of them. A
  code line is a line on which a token that is not a comment begins, so an item is never charged
  for its documentation, a string literal costs only the lines its quotes sit on, and nobody can
  pass by deleting a comment. Three lines is a constant, a static, a type alias, a unit
  struct, or a function that is a signature, one expression and a closing brace. The fourth line
  is the first statement that is logic.
- More than HANDFUL small items is no longer a handful. Every small item then counts, because a
  table of forty constants is a feature (the system-initializer crate root's layout block, #1957).
- Any item at all, marked where it sits by a plain comment among its leading comments:
  `// Index exception (2026-10-11): <reason of three words or more>.` The date is UTC. A marked
  item is not counted against anything; every mark is printed by `--list`, so the count is
  visible. A comment that names an index exception in any other form fails, so a typo cannot
  quietly unmark an item.

**The rules**, the file-length ratchet's, over a different number. A parent's number is the code
lines of every item it holds that the above does not allow.

- An unlisted parent fails at any number above zero. A new split is unlisted, so §282 binds it on
  the day it lands.
- A listed parent fails if its number grew against the merge base, or exceeds its entry. That is
  how the rule binds an existing parent as it is touched: no new code lands in it.
- An entry is a ceiling `--bank` lowers and nothing raises. An entry fails when its file is gone,
  renamed, no longer a parent, or down to zero, so dead rows cannot accumulate.
- Against the merge base, a row may not be added and an entry may not rise. A rename moves its row
  in the same change, at the same number.

**What it cannot see.** A top-level `mod` declared by a macro expansion; a parent's items behind
`include!`. The lexer is the one this tree's other Rust checks use, adapted, and is not a parser:
a construct it misreads fails loud on the fixture set before it judges the tree.
"""

import os
import re
import subprocess
import sys

# --- the numbers, the baseline and the mark -------------------------------------------------------

SMALL = 3      # code lines: an item this size or smaller may stay unmarked
HANDFUL = 8    # how many such items a parent may hold before none of them may
TEST_CHILDREN = ('tests', 'proofs', 'verification')
BASELINE = 'design/parent-index-baseline.tsv'
COLUMN = 'code_lines'

MARK = re.compile(r'^//\s*Index exception \((\d{4}-\d{2}-\d{2})\):\s*(\S.*)$')
MARK_WORDS = re.compile(r'index[ -]exception', re.I)

HEADER = """\
# The parent-index check's baseline: §282 (when a file is split, the parent becomes an index).
# One row per parent that held more than an index when it was written: path, then the code lines
# (comments and literals blanked, blank lines dropped) of the items an index may not hold.
# A number is a ceiling that may only go DOWN: lower it with
# `python3 helpers/parent_index.py --bank`, which never raises and never adds. A row may be
# removed, in the same change that takes its parent to zero or stops it being a parent; a row may
# not be added and a number may not rise (script/lint compares against the merge base). A rename
# moves its row in the same change, at the same number. Moving an item into a child named for it
# is the remedy; a mark beside the item (`// Index exception (YYYY-MM-DD): reason.`) is the
# exception §282 allows, and it is reviewed like any other line.
"""

REMEDY = ('Move it into a child module named for it (§282), or, if it is one of the few small '
          'things every child uses and nothing here can grow into a feature, mark it where it '
          'sits: `// Index exception (YYYY-MM-DD): <reason>.`')


def fmt(n):
    return f'{n:,}'


# --- the lexer: tokens with line numbers, comments kept -------------------------------------------

def _ident_char(c):
    return c == '_' or c.isalnum()


def lex(src):
    """[(kind, text, line)]. kind is 'id', 'lit', 'life', 'p' (one punctuation char), 'idoc' (an
    inner doc comment), 'doc' (an outer one) or 'cmt' (any other comment)."""
    out = []
    i, n, line = 0, len(src), 1
    while i < n:
        c = src[i]
        if c == '\n':
            line += 1
            i += 1
            continue
        if c.isspace():
            i += 1
            continue
        if src.startswith('//', i):
            j = src.find('\n', i)
            j = n if j < 0 else j
            t = src[i:j]
            kind = ('idoc' if t.startswith('//!') else
                    'doc' if t.startswith('///') and not t.startswith('////') else 'cmt')
            out.append((kind, t, line))
            i = j
            continue
        if src.startswith('/*', i):
            depth, j, first = 1, i + 2, line
            while j < n and depth:
                if src.startswith('/*', j):
                    depth, j = depth + 1, j + 2
                elif src.startswith('*/', j):
                    depth, j = depth - 1, j + 2
                else:
                    line += src[j] == '\n'
                    j += 1
            t = src[i:j]
            kind = ('idoc' if t.startswith('/*!') else
                    'doc' if t.startswith('/**') and not t.startswith('/***') and t != '/**/'
                    else 'cmt')
            out.append((kind, t, first))
            i = j
            continue
        prev_ident = i > 0 and _ident_char(src[i - 1])
        m = re.match(r'(?:b|c)?r(#*)"', src[i:i + 300])
        if m and not prev_ident:
            close = '"' + m.group(1)
            j = src.find(close, i + m.end())
            j = n if j < 0 else j + len(close)
            out.append(('lit', src[i:j], line))
            line += src.count('\n', i, j)
            i = j
            continue
        if c == '"' or (c in 'bc' and src.startswith('"', i + 1) and not prev_ident):
            j = i + (1 if c == '"' else 2)
            while j < n and src[j] != '"':
                j += 2 if src[j] == '\\' else 1
            j += 1
            out.append(('lit', src[i:j], line))
            line += src.count('\n', i, j)
            i = j
            continue
        if c == "'" or (c == 'b' and src.startswith("'", i + 1) and not prev_ident):
            k = i + (1 if c == "'" else 2)
            m = re.match(r"(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^\\'\n])'", src[k:k + 16])
            if m:
                out.append(('lit', src[i:k + m.end()], line))
                i = k + m.end()
                continue
            m = re.match(r"'[A-Za-z_][A-Za-z0-9_]*", src[i:i + 200])
            if m:
                out.append(('life', m.group(), line))
                i += m.end()
                continue
        m = re.match(r'[A-Za-z_][A-Za-z0-9_]*|[0-9][0-9A-Za-z_.]*', src[i:i + 200])
        if m:
            out.append(('id', m.group(), line))
            i += m.end()
            continue
        out.append(('p', c, line))
        i += 1
    return out


# --- top-level items ------------------------------------------------------------------------------

KEYWORDS = ('fn', 'struct', 'enum', 'union', 'trait', 'impl', 'const', 'static', 'type', 'mod',
            'use', 'macro_rules')
QUALIFIERS = ('pub', 'crate', 'super', 'self', 'in', 'unsafe', 'async', 'default', 'auto',
              'extern')


def _bracket_end(toks, i):
    """Index of the `]` closing the `[` at toks[i]."""
    depth = 0
    for j in range(i, len(toks)):
        if toks[j][0] == 'p' and toks[j][1] == '[':
            depth += 1
        elif toks[j][0] == 'p' and toks[j][1] == ']':
            depth -= 1
            if depth == 0:
                return j
    return len(toks) - 1


def _impl_name(toks, i):
    """`impl<T> Trait for Type<T> where ..` reads as `Trait for Type`: the words after `impl`
    and its generics, up to the body or the `where`, with generic arguments left out."""
    j = i
    while j < len(toks) and toks[j][1] != 'impl':
        j += 1
    words, depth = [], 0
    for k, v, _ in toks[j + 1:]:
        if k == 'p' and v == '<':
            depth += 1
        elif k == 'p' and v == '>' and depth:
            depth -= 1
        elif (k == 'p' and v == '{') or (k, v) == ('id', 'where'):
            break
        elif k == 'id' and depth == 0:
            words.append(v)
    return ' '.join(words)


def _classify(toks, i):
    """(kind, name) for the item whose first non-attribute token is toks[i]."""
    words = []
    j = i
    while j < len(toks) and len(words) < 12:
        k, v, _ = toks[j]
        if k == 'id':
            words.append(v)
        elif k == 'lit':
            pass                                  # extern "C"
        elif (k, v) == ('p', '!'):
            words.append('!')
        elif (k, v) == ('p', ':') and j + 1 < len(toks) and toks[j + 1][1] == ':':
            words.append('::')
            j += 1
        elif k == 'p' and v in '({[;=':
            if v == '(' and words and words[-1] in ('pub',):
                depth = 0                         # pub(crate), pub(in path): skip the group
                while j < len(toks):
                    if toks[j][1] == '(':
                        depth += 1
                    elif toks[j][1] == ')':
                        depth -= 1
                        if depth == 0:
                            break
                    j += 1
                j += 1
                continue
            words.append(v)
            break
        j += 1
    if words[:2] == ['extern', 'crate']:
        return 'use', words[2] if len(words) > 2 else ''
    for idx, w in enumerate(words):
        if w in QUALIFIERS:
            if w == 'extern' and idx + 1 < len(words) and words[idx + 1] in ('{', ';'):
                return 'extern', ''
            continue
        if w == 'const' and idx + 1 < len(words) and words[idx + 1] in ('fn', 'unsafe', 'extern',
                                                                       'async', 'impl', 'trait'):
            continue
        if w == 'macro_rules':
            rest = [x for x in words[idx + 1:] if x != '!']
            return 'macro_rules', rest[0] if rest else ''
        if w in KEYWORDS:
            rest = words[idx + 1:]
            name = rest[0] if rest and rest[0] not in '({[;=<' else ''
            if w == 'impl':
                name = _impl_name(toks, i)
            return w, name
        # A macro call, its path included: `global_asm!`, `core::arch::global_asm!`.
        k = idx
        while k + 2 < len(words) and words[k + 1] == '::':
            k += 2
        if k + 1 < len(words) and words[k + 1] == '!':
            return 'macro', words[k]
        return 'other', w
    return 'other', ''


def items(src, toks=None):
    """Every top-level item, in order, as a dict: kind, name, first and last line (the first counts
    its attributes and outer docs), the plain comments before it, its attributes, and for an inline
    module the item list of its body."""
    toks = lex(src) if toks is None else toks
    found = []
    i, n = 0, len(toks)
    attrs, comments, first = [], [], None
    while i < n:
        k, v, line = toks[i]
        if k == 'idoc':
            i += 1
            continue
        if k in ('doc', 'cmt'):
            if k == 'cmt':
                comments.append((v, line))
            if first is None and k == 'doc':
                first = line
            i += 1
            continue
        if (k, v) == ('p', '#') and i + 2 < n and toks[i + 1][1] == '!' and toks[i + 2][1] == '[':
            i = _bracket_end(toks, i + 2) + 1    # an inner attribute belongs to the file
            attrs, comments, first = [], [], None
            continue
        if (k, v) == ('p', '#') and i + 1 < n and toks[i + 1][1] == '[':
            end = _bracket_end(toks, i + 1)
            attrs.append(' '.join(t[1] for t in toks[i:end + 1]))
            first = line if first is None else first
            i = end + 1
            continue
        if (k, v) == ('p', ';'):                  # a stray `;` after an item
            i += 1
            continue
        first = line if first is None else first
        kind, name = _classify(toks, i)
        # These end at their `;` even past a brace (`use a::{b, c};`, `const T: S = S { .. };`).
        # Every other item ends at the brace that closes its body, which is why an `=` is not a
        # signal: `struct Vec<T, A: Allocator = Global> { .. }` has one before its body.
        ends_at_semicolon = kind in ('use', 'const', 'static', 'type')
        depth, j, body = 0, i, None
        while j < n:
            tk, tv, _ = toks[j]
            if tk == 'p':
                if tv in '([{':
                    if depth == 0 and tv == '{' and body is None:
                        body = j
                    depth += 1
                elif tv in ')]}':
                    depth -= 1
                    if depth == 0 and tv == '}' and not ends_at_semicolon:
                        if j + 1 < n and toks[j + 1][:2] == ('p', ';'):
                            j += 1
                        break
                elif tv == ';' and depth == 0:
                    break
            j += 1
        j = min(j, n - 1)
        item = dict(kind=kind, name=name, first=first, last=toks[j][2], attrs=attrs,
                    comments=comments, body=None)
        if kind == 'mod' and body is not None:
            item['kind'] = 'inline_mod'
            item['body'] = items(src, toks[body + 1:j])
        found.append(item)
        attrs, comments, first = [], [], None
        i = j + 1
    return found


def code_lines(toks):
    """The lines on which a token that is not a comment begins. A literal spanning lines counts
    its first; the line its closing quote shares with a `;` counts through the `;`."""
    return {line for kind, _text, line in toks if kind not in ('idoc', 'doc', 'cmt')}


def is_index_item(item):
    if item['kind'] in ('mod', 'use'):
        return True
    if item['kind'] == 'inline_mod':
        return all(is_index_item(child) for child in item['body'])
    return False


def mark_of(item):
    """('ok', date, reason), ('bad', line, text) for a malformed mark, or None."""
    for text, line in item['comments']:
        m = MARK.match(text.strip())
        if m and len(m.group(2).split()) >= 3:
            return ('ok', m.group(1), m.group(2))
        if MARK_WORDS.search(text):
            return ('bad', line, text.strip())
    return None


# --- one file -------------------------------------------------------------------------------------

def judge(src):
    """None when `src` is not a parent. Otherwise a dict:

    held:   [(item, code_lines)] the items an index may not hold, unmarked
    marked: [(item, date, reason)]
    small:  [(item, code_lines)] small unmarked items the HANDFUL allowance let stand
    bad:    [(line, text)] malformed marks
    number: the code lines of `held`, the baseline's measure
    marked_lines: the code lines of `marked`, which the gate excuses
    """
    toks = lex(src)
    found = items(src, toks)
    if not any(it['kind'] == 'mod' and it['name'] not in TEST_CHILDREN for it in found):
        return None
    code = code_lines(toks)
    held, marked, small, bad = [], [], [], []
    marked_lines = 0
    for it in found:
        if is_index_item(it):
            continue
        size = sum(1 for row in range(it['first'], it['last'] + 1) if row in code)
        mark = mark_of(it)
        if mark and mark[0] == 'bad':
            bad.append(mark[1:])
            mark = None
        if mark:
            marked.append((it, mark[1], mark[2]))
            marked_lines += size
            continue
        (small if size <= SMALL else held).append((it, size))
    if len(small) > HANDFUL:
        held = sorted(held + small, key=lambda pair: pair[0]['first'])
        small = []
    return dict(held=held, marked=marked, small=small, bad=bad,
                number=sum(size for _it, size in held), marked_lines=marked_lines)


def describe(item):
    name = f' {item["name"]}' if item['name'] else ''
    return f'{item["kind"]}{name}'


# --- the baseline ---------------------------------------------------------------------------------

def read_baseline(text):
    rows = {}
    for line in text.splitlines():
        if not line.strip() or line.startswith('#'):
            continue
        cells = line.split('\t')
        if cells[0] == 'path':
            continue
        rows[cells[0]] = int(cells[1])
    return rows


def write_baseline(rows):
    out = [HEADER, 'path\t' + COLUMN + '\n']
    for path in sorted(rows):
        out.append(f'{path}\t{rows[path]}\n')
    with open(BASELINE, 'w') as f:
        f.write(''.join(out))


# --- git ------------------------------------------------------------------------------------------

def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


def in_scope(path):
    """A `.rs` path outside `vendor/`: the files the rules reach."""
    return path.endswith('.rs') and not path.startswith('vendor/')


def tracked_rust():
    """Every in-scope path: tracked, and untracked ones too."""
    files = git('ls-files', '-z', '--cached', '--others', '--exclude-standard') or ''
    return {f for f in files.split('\0') if f and in_scope(f) and os.path.exists(f)}


BASE_OVERRIDE = None  # `--check --base REV`: falsify the merge-base half against a chosen commit


def merge_base():
    if BASE_OVERRIDE:
        return (git('rev-parse', BASE_OVERRIDE) or '').strip() or None
    base = (git('merge-base', 'HEAD', 'origin/main') or '').strip()
    head = (git('rev-parse', 'HEAD') or '').strip()
    return base if base and base != head else None


def renamed_rust(base):
    """[(old, new)] for every `.rs` file git itself calls renamed between `base` and the tree."""
    pairs = []
    for line in (git('diff', '--name-status', '-M', base, '--', '*.rs') or '').splitlines():
        cells = line.split('\t')
        if cells[0].startswith('R') and len(cells) == 3:
            pairs.append((cells[1], cells[2]))
    return pairs


# --- the rules, pure so the selftest can drive them -----------------------------------------------

def problems(judged, rows, base_numbers=None, base_rows=None, renames=(), scanned=1):
    """Every failure in one tree.

    `judged` is {path: judge(text)} for every parent in the tree; `rows` the baseline's
    {path: ceiling}; `base_numbers` {path: number} at the merge base for the files that were
    parents there; `base_rows` the baseline there, None when the base has none (the change that
    first lands it); `renames` git's (old, new) `.rs` pairs since the base; `scanned` how many Rust
    files were read at all.
    """
    if not scanned or not judged:
        return ['the check judged no parent at all. A selector that matches nothing passes by '
                'checking nothing (design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md)']
    was_called = {new: old for old, new in renames}
    bad = []
    for path in sorted(judged):
        j = judged[path]
        for line, text in j['bad']:
            bad.append(f'{path}:{line}: a comment names an index exception but is not one: '
                       f'{text!r}. The form is `// Index exception (YYYY-MM-DD): <reason of three '
                       f'words or more>.`')
        if path in rows:
            continue
        for item, size in j['held']:
            why = (f'{size} code lines' if size > SMALL else
                   f'{size} code lines, but the parent holds more than {HANDFUL} small items')
            bad.append(f'{path}:{item["first"]}: {describe(item)} ({why}) in a split parent, '
                       f'which is not on the list. {REMEDY}')
    for path in sorted(rows):
        j = judged.get(path)
        ceiling = rows[path]
        if j is None:
            bad.append(f'{BASELINE}: {path} is not a parent in the tree (gone, renamed, or no '
                       f'longer split). Move its row with a rename, at the same number, or remove '
                       f'it; the list cannot go stale')
            continue
        if j['number'] == 0:
            bad.append(f'{BASELINE}: {path} is an index now. Remove its row; the list cannot go '
                       f'stale')
        elif j['number'] > ceiling:
            bad.append(f'{path}: {fmt(j["number"])} code lines outside the index, over its '
                       f'baseline ceiling of {fmt(ceiling)}. {REMEDY}')
        if base_numbers is not None:
            was = base_numbers.get(was_called.get(path, path))
            if was is not None and j['number'] > was:
                bad.append(f'{path}: grew from {fmt(was)} to {fmt(j["number"])} code lines outside '
                           f'the index against the merge base. A listed parent may only shrink. '
                           f'{REMEDY}')
    if base_rows is not None:
        for path in sorted(rows):
            prior = base_rows.get(path)
            if prior is None:
                prior = base_rows.get(was_called.get(path, ''))
            if prior is None:
                bad.append(f'{BASELINE}: {path} was added. The list only shrinks; a new split '
                           f'leaves an index outright')
            elif rows[path] > prior:
                bad.append(f'{BASELINE}: {path} raised its ceiling from {fmt(prior)} to '
                           f'{fmt(rows[path])}. An entry only goes down')
    return bad


def banked(rows, judged):
    """The baseline after banking: entries lowered to the tree, never raised, never added; a row
    whose parent is gone or is an index now drops out."""
    out = {}
    for path, ceiling in rows.items():
        j = judged.get(path)
        if j is None or j['number'] == 0:
            continue
        out[path] = min(j['number'], ceiling)
    return out


# --- the verbs ------------------------------------------------------------------------------------

def read(path):
    return open(path, encoding='utf-8', errors='replace').read()


# A cheap filter before the lexer: a file with no line that could declare a child out of line
# cannot be a parent. It only skips work; judge() still decides, so a match it should not have
# made costs time and nothing else.
MAYBE_PARENT = re.compile(r'\bmod\s+[A-Za-z_][A-Za-z0-9_]*\s*;')


def judge_tree():
    paths = tracked_rust()
    judged = {}
    for path in paths:
        text = read(path)
        j = judge(text) if MAYBE_PARENT.search(text) else None
        if j is not None:
            judged[path] = j
    return paths, judged


def gather():
    paths, judged = judge_tree()
    rows = read_baseline(read(BASELINE))
    base = merge_base()
    base_numbers, base_rows, renames = None, None, ()
    if base:
        renames = renamed_rust(base)
        wanted = set(rows) | {old for old, _new in renames}
        base_numbers = {}
        for path in sorted(wanted):
            text = git('show', f'{base}:{path}')
            j = judge(text) if text is not None else None
            if j is not None:
                base_numbers[path] = j['number']
        old = git('show', f'{base}:{BASELINE}')
        if old is not None:
            base_rows = read_baseline(old)
    return paths, judged, rows, base_numbers, base_rows, renames


def run_check():
    if not os.path.exists(BASELINE):
        print(f'parent-index check: {BASELINE} is missing. The gate and its list are one change; '
              f'deleting one deletes the gate', file=sys.stderr)
        return 1
    paths, judged, rows, base_numbers, base_rows, renames = gather()
    bad = problems(judged, rows, base_numbers, base_rows, renames, len(paths))
    if bad:
        print('parent-index check: a split parent holds more than an index (§282):',
              file=sys.stderr)
        for b in bad:
            print(f'  {b}', file=sys.stderr)
        print('\nMove the item into a child named for it, mark the exception where it sits, or '
              'bank a shrink with --bank. See design/decisions/'
              '0282-when-a-file-is-split-the-parent-becomes-an-index.md.', file=sys.stderr)
        return 1
    marks = sum(len(j['marked']) for j in judged.values())
    indexes = sum(1 for j in judged.values() if j['number'] == 0)
    print(f'parent-index check: {len(paths)} Rust files, {len(judged)} split parents, {indexes} '
          f'of them indexes; {len(rows)} held by the baseline; {marks} marked exceptions')
    return 0


def run_list():
    _paths, judged = judge_tree()
    for path in sorted(judged, key=lambda p: (-judged[p]['number'], p)):
        j = judged[path]
        print(f'{j["number"]:6}  {path}')
        for item, size in j['held']:
            print(f'          {item["first"]:>5}  {size:4}  {describe(item)}')
        for item, size in j['small']:
            print(f'          {item["first"]:>5}  {size:4}  {describe(item)}  (small)')
        for item, date, reason in j['marked']:
            print(f'          {item["first"]:>5}  mark  {describe(item)}  ({date}: {reason})')
    return 0


def bank(initialize=False):
    if initialize and os.path.exists(BASELINE):
        print(f'parent-index check: {BASELINE} already exists. --initialize wrote it once, for §282; it '
              f'is refused now, and --bank is the verb that never raises', file=sys.stderr)
        return 2
    if not initialize and not os.path.exists(BASELINE):
        print(f'parent-index check: {BASELINE} does not exist; there is nothing to bank',
              file=sys.stderr)
        return 2
    _paths, judged = judge_tree()
    if initialize:
        rows = {p: j['number'] for p, j in judged.items() if j['number']}
    else:
        rows = banked(read_baseline(read(BASELINE)), judged)
    write_baseline(rows)
    print(f'parent-index check: {len(rows)} rows written to {BASELINE}')
    return 0


# --- the selftest ---------------------------------------------------------------------------------

def selftest():
    """The rule as fixtures, and the lexer traps around it. Each case names what it proves."""
    cases = []

    def check(name, ok):
        cases.append((name, bool(ok)))

    INDEX = '''\
//! The parent's docs. A `fn` named in a doc is not an item: fn nope() { 1 }
#![no_std]

/// The first child.
pub mod alpha;
#[cfg(test)]
mod tests;
mod beta;
pub use alpha::{Alpha, AlphaError};
pub(crate) use beta::*;
extern crate alloc;
pub mod gamma {
    //! An inline index module, as std::collections writes hash_map.
    pub use super::beta::Gamma;
}
'''
    j = judge(INDEX)
    check('an index (docs, mod, use, extern crate, an inline module of uses) holds nothing',
          j is not None and j['number'] == 0 and not j['held'] and not j['small'])

    check('a file with no child is not a parent', judge('fn main() {\n    big();\n}\n') is None)
    check('a file whose only children are tests and proofs is not a parent',
          judge('fn f() {\n    a();\n    b();\n    c();\n}\n#[cfg(test)]\nmod tests;\n'
                '#[cfg(kani)]\nmod proofs;\n') is None)
    check('a file whose only child is its Kani `verification` module is not a parent',
          judge('fn f() {\n    a();\n    b();\n    c();\n}\n#[cfg(kani)]\nmod verification;\n')
          is None)
    check('a harness child beside a code child still makes a parent',
          judge('fn f() {\n    a();\n    b();\n    c();\n}\nmod widths;\n'
                '#[cfg(kani)]\nmod verification;\n') is not None)
    check('an inline `mod tests { .. }` does not make a parent',
          judge('mod tests {\n    fn t() {}\n}\n') is None)

    FOUR = 'pub fn four(x: u32) -> u32 {\n    let y = x + 1;\n    y * 2\n}\n'
    j = judge(INDEX + FOUR)
    check('a four-code-line function in a parent is held, at four',
          j['number'] == 4 and [describe(i) for i, _ in j['held']] == ['fn four'])
    THREE = 'pub fn three(x: u32) -> u32 {\n    x + 1\n}\n'
    j = judge(INDEX + THREE)
    check('a three-code-line function stays unmarked', j['number'] == 0 and len(j['small']) == 1)
    DOCS = '/// One.\n///\n/// Two, and fn fake() {}.\n// A plain comment.\n' + THREE
    check('an item is not charged for its docs or comments', judge(INDEX + DOCS)['number'] == 0)
    LIT = 'pub const S: &str = "a\nb\nc\nd";\n'
    check('a multi-line string literal is not code lines', judge(INDEX + LIT)['number'] == 0)

    # Literal numbers, not HANDFUL, so a change to the constant fails here until §282 says so too.
    consts = ''.join(f'pub const C{k}: u32 = {k};\n' for k in range(8))
    check('8 small items are a handful', judge(INDEX + consts)['number'] == 0)
    j = judge(INDEX + consts + 'pub const ONE_MORE: u32 = 9;\n')
    check('9 small items are not, and every one is held',
          j['number'] == 9 and len(j['held']) == 9)

    MARKED = ('// Index exception (2026-10-11): every child reads this table.\n'
              '#[derive(Debug)]\n' + FOUR)
    j = judge(INDEX + MARKED)
    check('a marked item is not held, and its mark is read',
          j['number'] == 0 and j['marked'] and j['marked'][0][1] == '2026-10-11')
    check('a marked item\'s code lines are still measured',
          j['marked_lines'] == 5)
    j = judge(INDEX + '// Index exception (2026-10-11): because.\n' + FOUR)
    check('a mark with a reason under three words is malformed, and the item is held',
          j['bad'] and j['number'] == 4)
    j = judge(INDEX + '// index exception: every child reads it.\n' + FOUR)
    check('a mark with no date is malformed', j['bad'] and j['number'] == 4)
    j = judge(INDEX + '/// Index exception (2026-10-11): docs are not marks here.\n' + FOUR)
    check('a doc comment is not a mark', j['number'] == 4 and not j['marked'])

    # The lexer: items whose shape a naive scanner gets wrong.
    TRICKY = '''\
mod child;
const TABLE: [u8; 2] = { [1, 2] };
impl<T> Thing<T> where T: Copy {
    fn a(&self) -> char { '}' }
    fn b(&self) -> &'static str { r#"}"# }
}
macro_rules! m { () => {}; }
core::arch::global_asm!("nop");
unsafe extern "C" {
    static x: u64;
}
pub(crate) const fn k() -> u32 {
    1
}
struct Tuple(u32);
pub struct Defaulted<T, A: Alloc = Global> {
    buf: T,
}
fn after() {}
const impl<T> Thing<T> {
    fn c(&self) {}
}
'''
    kinds = [describe(it) for it in items(TRICKY)]
    check('the lexer finds each item across braces in literals, lifetimes and paths',
          kinds == ['mod child', 'const TABLE', 'impl Thing', 'macro_rules m',
                    'macro global_asm', 'extern', 'fn k', 'struct Tuple',
                    'struct Defaulted', 'fn after', 'impl Thing'])
    check('an impl body is one item, and its code lines are counted',
          [(describe(it), s) for it, s in judge(TRICKY)['held'] if s > SMALL]
          == [('impl Thing', 4)])

    # The tree rules, over judged dicts.
    def parent(number):
        return dict(held=[], marked=[], small=[], bad=[], number=number)

    check('an unlisted parent that is an index passes', not problems({'x/a.rs': parent(0)}, {}))
    j = judge(INDEX + FOUR)
    msg = problems({'x/a.rs': j}, {})
    check('an unlisted parent holding a function fails, naming the item and the remedy',
          msg and 'fn four' in msg[0] and 'child module named for it' in msg[0]
          and 'Index exception' in msg[0])
    row = {'x/a.rs': 100}
    check('a listed parent unchanged passes',
          not problems({'x/a.rs': parent(90)}, row, {'x/a.rs': 90}, row))
    check('a listed parent that shrank passes',
          not problems({'x/a.rs': parent(80)}, row, {'x/a.rs': 90}, row))
    check('a listed parent that grew against the merge base fails, within its entry',
          problems({'x/a.rs': parent(91)}, row, {'x/a.rs': 90}, row))
    check('a listed parent over its entry fails',
          problems({'x/a.rs': parent(101)}, row, {'x/a.rs': 101}, row))
    check('an entry for a parent that is an index now fails',
          problems({'x/a.rs': parent(0)}, row, {'x/a.rs': 90}, row))
    check('an entry for a file that is no longer a parent fails',
          problems({'x/b.rs': parent(0)}, row, {}, row))
    check('a row added against the merge base fails',
          problems({'x/a.rs': parent(90), 'x/b.rs': parent(5)}, {'x/a.rs': 100, 'x/b.rs': 5},
                   {'x/a.rs': 90}, row))
    check('an entry raised against the merge base fails',
          problems({'x/a.rs': parent(90)}, {'x/a.rs': 110}, {'x/a.rs': 90}, row))
    check('a rename that moves its row at the same number passes',
          not problems({'x/new.rs': parent(90)}, {'x/new.rs': 100}, {'x/a.rs': 90}, row,
                       [('x/a.rs', 'x/new.rs')]))
    check('a rename that leaves its row behind fails',
          problems({'x/new.rs': parent(90)}, row, {'x/a.rs': 90}, row, ()))
    check('a malformed mark fails even in a listed parent',
          problems({'x/a.rs': dict(parent(90), bad=[(3, '// index exception')])}, row))
    check('a selector that matches nothing fails loudly', problems({}, {}, scanned=0))
    check('bank lowers, drops an index, never raises, never adds',
          banked({'a': 50, 'b': 40, 'c': 30}, {'a': parent(45), 'b': parent(0), 'c': parent(35),
                                              'd': parent(9)}) == {'a': 45, 'c': 30})

    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f'parent-index selftest: {name}', file=sys.stderr)
    if not failed:
        print(f'parent-index selftest: {len(cases)} cases pass')
    return 1 if failed else 0


def main(argv):
    root = git('rev-parse', '--show-toplevel')
    if root:
        os.chdir(root.strip())
    global BASE_OVERRIDE
    if len(argv) >= 3 and argv[0] == '--check' and argv[1] == '--base':
        BASE_OVERRIDE = argv[2]
        argv = argv[:1]
    if argv and argv[0] == '--selftest':
        return selftest()
    if not argv or argv[0] == '--check':
        return run_check()
    if argv[0] == '--list':
        return run_list()
    if argv[0] in ('--bank', '--initialize', '--init'):
        return bank(initialize=argv[0] in ('--initialize', '--init'))
    print(__doc__.split('\n\n')[2], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
