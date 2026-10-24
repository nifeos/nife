"""The comment-block ratchet: §267 (a comment states the constraint as it is now), held to a
baseline that only falls.

calef ruled the ruling this gate holds on 2026-10-07 (UTC), deciding the forks of
`notes/comment-cost-2026-10-07.md` (PR #1800, fork (c)), and ratified its sharper form the same
day: "ratify #1800 Fork (c) in the sharper form". Milestone 860 (comments state the constraint as
it is now) files the sweep and builds this gate. It copies `helpers/file_length_ratchet.py`'s
pattern, §266 (a Rust source file stays under 2,000 lines)'s ratchet, which itself copies
`helpers/prose_ratchet.py`'s, §212 (a prose budget)'s: a committed list that only shrinks, a
comparison against the merge base, and a selftest that runs before the check.

    python3 helpers/comment_block_ratchet.py --check          # what script/lint runs
    python3 helpers/comment_block_ratchet.py --bank           # lower the baseline to the tree; never raises
    python3 helpers/comment_block_ratchet.py --step kernel/ 10  # lower a directory's cap a step, listing what it reaches
    python3 helpers/comment_block_ratchet.py --selftest       # the rules, as fixtures
    python3 helpers/comment_block_ratchet.py --init           # write the baseline from scratch (milestone 860 only)

Name: provisional, minted by milestone 860's lane on 2026-10-09 (UTC). A shared python module
under `helpers/`, which `script/names` puts out of its own scope, so its provenance is this
paragraph rather than a `Name:` block. calef names things; expect this to change. Refused
`comment_ratchet.py`: §267 caps a *block*, not comment density, which the ruling keeps on purpose
(the tree stays commented far more heavily than production code, deliberately), and a name that
read as capping comments would say the opposite of the decision.

**The measure.** A block is a run of adjacent comment lines of one kind, doc (`///`, `//!`,
`/** */`, `/*! */`) or plain (`//`, `/* */`); its size is its line count. A trailing comment, one
that begins after code on its line, is not part of any block: it labels the line it sits on, and
that code line ends the run around it, so a table whose every row carries `// 0x41 A` holds no
block at all. Every tracked `.rs` file outside `vendor/`, plus untracked ones, so a lane's new file
is judged before its first commit. A block of 40 lines is allowed; 41 is over.

**The cap: 40 lines**, calef's "about 40" read off the measured distribution rather than chosen
before it. Measured 2026-10-09 (UTC) at `2d59ddde1` with the note's method, which counts a trailing
comment as a one-line block: 28,950 blocks, median 2 lines, mean 4.77. Blocks over 40 hold
19.8% of comment lines (the note, one day earlier, measured 20%); over 30 holds 24.5% and over 50
holds 17.3%, so 40 is where the long tail's bulk sits, and it is the number calef named.

**Caps per directory, and what the cap does not count** (milestone 929 (comments in the Linux style
across first-party Rust)). A `cap` row in the baseline sets a directory's cap; the longest prefix
that holds a file wins, and a directory with no row is at 40. A cap is 40, 20 or 10 and only falls.
`--step DIR CAP` takes a directory down a step: it writes the row, lists every block over the new
cap at its size, and starts the directory's in-body ceiling. That one change may add rows for that
directory, each at no more than its block's size at the merge base; no other change may. A block's
size leaves out two kinds of line §267 keeps whole: a `// SAFETY:` comment from its marker to the
block's end, and a `Name:` paragraph (design/naming.md) from its marker to the next empty doc line.

**The in-body count** (milestone 929). An `in-body` row holds the plain whole-line comment lines
inside `fn` bodies under one directory, nested closures and items included and `// SAFETY:`
comments left out, to a ceiling. The ceiling only falls, `--bank` lowers it, and once written it may
not be removed. Bodies are found by the lexer: a `fn` outside brackets starts
a signature, and the next `{` outside brackets and the signature's `<>` opens its body unless a
`;` or `}` comes first, or a `,` outside `<>` before any `where`, or a `>` closing generics the
`fn` sits in (a trait method, a `fn` pointer type).

**The rules** (§266 section 3 option (b)'s shape, ruled there by calef over exact match, applied
one level down):

- A block not on the list fails at 41 lines or more. A new file's blocks meet the cap outright.
- A listed block fails if it grew against the merge base, and if it is larger than its entry. The
  growth check is unconditional: an entry can sit above its block's real size because a shrink was
  never banked, and that slack is not room to grow back into.
- A list entry is a ceiling that `--bank` lowers and nothing raises. An entry fails when its block
  is gone (its file is gone, or the file holds fewer over-cap blocks than the list holds rows), so
  dead rows cannot accumulate.
- Against the merge base, a row may not be added and an entry may not rise. Rows are matched file
  by file in line order, so a shrink that shifts later blocks down moves their rows in the same
  change, the way a rename moves a row. A rename moves its rows in the same change, at the same
  numbers.
- A split carries a row the same way (§267's amendment of 2026-10-11, UTC). A listed block that
  leaves its file and turns up, unchanged or only cut, in a file with no rows at the merge base
  keeps its row under the new path, at a ceiling no higher than before. The match is by content,
  not by a declared list: the block's lines, each stripped of its indentation, must be the base
  block's lines or a subsequence of them (lines deleted, none added or edited), and nothing in the
  old file may still be that block. So a moved block that grew is unlisted and fails, a copy left
  behind carries nothing, and each base block carries at most one row.
- Exceptions are not provided for. A block that must stay over the cap needs §267 amended, by an
  architect.

**Why a ceiling and not exact match** (§266 section 3's measurement, which holds here too): of the
664 merges in the fortnight to 2026-10-08, 164 touched a file the file-length ratchet lists; two
shrinking changes to one file in one merge group each write a different number on the same row
under exact match and the group fails even when the Rust merged cleanly. Under a ceiling both pass;
the cost is that an entry can sit above its block's real size until someone banks it.

**The failure message** names the file, the block's first line, its size and the ceiling, and says
the remedy: history goes in the commit message, findings go in a note, and the block keeps the
constraint as it is now (§267). The remedy is never to delete a fact nobody has recorded.
"""

import os
import re
import subprocess
import sys

# --- the ceiling and the baseline -----------------------------------------------------------------

CEILING = 40
BASELINE = 'design/comment-block-baseline.tsv'

HEADER = """\
# The comment-block ratchet's baseline: milestone 860 (comments state the constraint as it is now),
# for §267 (a comment states the constraint as it is now), and milestone 929 (comments in the Linux
# style across first-party Rust).
# A `cap` row lowers one directory's cap from 40 to 20 or 10, and an `in-body` row holds its plain
# comment lines inside function bodies to a ceiling; both only fall (`--step`, then `--bank`).
# One row per comment block over its directory's cap when it was written: the file and the block's
# first line (with `#2`, `#3`... only where two over-cap blocks would start at one line), then the
# block's lines. A number is a ceiling that may only go DOWN: lower it with
# `python3 helpers/comment_block_ratchet.py --bank`, which never raises and never adds. A row goes,
# in the same change that takes its block to 40 lines or below, or deletes it; a row may not be
# added and a number may not rise (script/lint compares against the merge base, matching rows in
# line order per file, so a shrink that shifts later blocks moves their rows in the same change).
# A rename moves its rows in the same change, at the same numbers, and so does a split: a listed block
# moved unchanged (or only cut) into a file with no rows keeps its row under its new path, which
# `--bank` writes for you. A block that must exceed its row needs §267 amended by an architect.
"""

REMEDY = ('The remedy is the sweep\'s move: history goes in the commit message, findings go in a '
          'note, and the block keeps the constraint as it is now (§267). It is never to delete a '
          'fact nobody has recorded.')


def fmt(n):
    return f'{n:,}'


def read_baseline(text):
    """{(path, first_line, occurrence): ceiling}, the occurrence starting at 1."""
    return read_table(text)[0]


def read_table(text):
    """(rows, caps, in_body) from a baseline: the block rows as `read_baseline` gives them, the
    directory caps {prefix: cap} from `cap` rows, and the in-body ceilings {prefix: lines} from
    `in-body` rows."""
    rows, caps, in_body = {}, {}, {}
    for line in text.splitlines():
        if not line.strip() or line.startswith('#'):
            continue
        cells = line.split('\t')
        if cells[0] == 'block':
            continue
        if cells[0] in ('cap', 'in-body') and len(cells) == 3:
            (caps if cells[0] == 'cap' else in_body)[cells[1]] = int(cells[2])
            continue
        key = parse_key(cells[0])
        if key:
            rows[key] = int(cells[1])
    return rows, caps, in_body


def cap_of(path, caps):
    """The block cap for `path`: the longest directory prefix in `caps` that holds it, else 40."""
    best = None
    for prefix in caps:
        if path.startswith(prefix) and (best is None or len(prefix) > len(best)):
            best = prefix
    return caps[best] if best is not None else CEILING


def parse_key(cell):
    """`path:line` or `path:line#n` -> (path, line, n), or None when the cell is not one."""
    m = re.fullmatch(r'(.+):(\d+)(?:#(\d+))?', cell)
    if not m:
        return None
    return (m.group(1), int(m.group(2)), int(m.group(3) or 1))


def write_baseline(rows, caps=None, in_body=None):
    out = [HEADER]
    for prefix in sorted(caps or {}):
        out.append(f'cap\t{prefix}\t{caps[prefix]}\n')
    for prefix in sorted(in_body or {}):
        out.append(f'in-body\t{prefix}\t{in_body[prefix]}\n')
    out.append('block\tlines\n')
    for key in sorted(rows, key=lambda k: (k[0], k[1], k[2])):
        path, line, occ = key
        suffix = f'#{occ}' if occ > 1 else ''
        out.append(f'{path}:{line}{suffix}\t{rows[key]}\n')
    with open(BASELINE, 'w') as f:
        f.write(''.join(out))


# --- the lexer -------------------------------------------------------------------------------------
#
# Adapted from notes/comment-cost-2026-10-07/rustlex.py (lane/comment-cost-measurement,
# 2026-10-07 UTC), which stays untouched as that note's evidence. It handles line comments, nested
# block comments, doc versus plain, strings, raw strings, byte strings, and char literals versus
# lifetimes; "good enough to decide" is its own bar and the note's. It departs from the note's lexer
# in one place: a comment that begins after code on its line is kind 'trailing', which the note
# counted as a one-line span of its markers' kind.

def lex_comment_spans(src):
    """[(kind, start_line, end_line)], 1-based, in source order. kind is 'doc' or 'plain', or
    'trailing' for a comment that begins after code on its line, whatever its markers."""
    i, n = 0, len(src)
    spans = []
    line = 1
    code = False  # whether this line has held anything but whitespace and comments so far
    while i < n:
        c = src[i]
        if c == '\n':
            line += 1
            code = False
            i += 1
            continue
        if c in ' \t\r':
            i += 1
            continue
        trailing = code
        code = True
        if src.startswith('//', i):
            j = src.find('\n', i)
            if j < 0:
                j = n
            text = src[i:j]
            doc = (text.startswith('///') and not text.startswith('////')) or text.startswith('//!')
            spans.append(('trailing' if trailing else 'doc' if doc else 'plain', line, line))
            code = trailing
            i = j
            continue
        if src.startswith('/*', i):
            depth = 0
            j = i
            sl = line
            while j < n:
                if src.startswith('/*', j):
                    depth += 1
                    j += 2
                    continue
                if src.startswith('*/', j):
                    depth -= 1
                    j += 2
                    if depth == 0:
                        break
                    continue
                if src[j] == '\n':
                    line += 1
                j += 1
            text = src[i:j]
            doc = ((text.startswith('/**') and not text.startswith('/***') and text != '/**/')
                   or text.startswith('/*!'))
            spans.append(('trailing' if trailing else 'doc' if doc else 'plain', sl, line))
            code = trailing and sl == line
            i = j
            continue
        # Raw strings r"..", r#".."#, br#".."#, c#".."#: skip whole, newlines counted.
        m = re.match(r'(b|c)?r(#*)"', src[i:i + 300])
        if m and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == '_')):
            end = '"' + m.group(2)
            j = src.find(end, i + m.end())
            if j < 0:
                j = n
            else:
                j = j + len(end)
            line += src[i:j].count('\n')
            i = j
            continue
        if c == '"' or (c in 'bc' and i + 1 < n and src[i + 1] == '"'
                        and not (i > 0 and (src[i - 1].isalnum() or src[i - 1] == '_'))):
            j = i + (1 if c == '"' else 2)
            while j < n and src[j] != '"':
                if src[j] == '\\':
                    j += 1
                j += 1
            j += 1
            line += src[i:j].count('\n')
            i = j
            continue
        if c == "'" or (c == 'b' and i + 1 < n and src[i + 1] == "'"):
            k = i + (1 if c == "'" else 2)
            m = re.match(r"(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^\\'\n])'", src[k:k + 16])
            if m:
                i = k + m.end()
                continue
            m = re.match(r"'[A-Za-z_][A-Za-z0-9_]*", src[i:])
            if m:
                i = i + m.end()
                continue
            i += 1
            continue
        m = re.match(r'[A-Za-z_][A-Za-z0-9_]*|[0-9][0-9A-Za-z_.]*', src[i:i + 200])
        if m:
            i = i + m.end()
            continue
        i += 1
    return spans


def blocks(src):
    """[(first_line, last_line, size)] for every block, in line order.

    A block is a run of adjacent comment spans of one kind whose line ranges are consecutive. A
    trailing comment labels the code line it sits on, so it neither begins nor joins a block, and
    its code line ends the run around it. `size` is the block's lines less the two kinds §267
    keeps whole (`exempt_lines`), so it can be smaller than the block's span.
    """
    lines = src.split('\n')
    out = []
    cur = None
    for kind, a, b in lex_comment_spans(src) + [('end', 0, 0)]:
        if kind == 'trailing':
            continue
        if cur and kind == cur[2] and a == cur[1] + 1:
            cur = [cur[0], b, kind]
            continue
        if cur:
            span = [line.strip() for line in lines[cur[0] - 1:cur[1]]]
            out.append((cur[0], cur[1], len(span) - exempt_lines(span)))
        cur = [a, b, kind]
    return out


NAME_LINE = re.compile(r'//[/!]\s?Name:')


def exempt_lines(span):
    """How many of a block's lines the cap does not count: a `// SAFETY:` comment from its marker
    to the block's end (Linux demands that comment, and §267 keeps the `SAFETY` discipline), and a
    `Name:` paragraph from its marker to the next empty doc line or the block's end (calef,
    2026-10-22 UTC: "Approved assuming naming provenance comments remain.")."""
    exempt, i = 0, 0
    while i < len(span):
        if span[i].startswith('// SAFETY:'):
            return exempt + len(span) - i
        if NAME_LINE.match(span[i]):
            j = i + 1
            while j < len(span) and span[j] not in ('///', '//!'):
                j += 1
            exempt += j - i
            i = j
            continue
        i += 1
    return exempt


def blocks_over(src, ceiling=CEILING):
    """[(first_line, lines)] for every block over `ceiling`, in line order."""
    return [(first, size) for first, _last, size in blocks(src) if size > ceiling]


def block_texts(src, ceiling=CEILING):
    """[(first_line, lines, text)] for every block over `ceiling`, `text` a tuple of its source
    lines with indentation stripped, so a block that moved out of an inline `mod x { }` into its own
    file, one indent shallower, still reads as the same block."""
    lines = src.split('\n')
    return [(first, size, tuple(line.strip() for line in lines[first - 1:last]))
            for first, last, size in blocks(src) if size > ceiling]


# --- the in-body count -----------------------------------------------------------------------------
#
# Milestone 929 (comments in the Linux style across first-party Rust): a comment inside a function
# body states a constraint the code cannot show. What it counts is plain whole-line comment lines
# inside a `fn` body, nested items and closures included; a `// SAFETY:` comment, from its marker
# to the end of its block, is not counted. Each started directory holds the count to a ceiling that
# only falls.

def fn_body_ranges(src):
    """[(open_line, close_line)] of every outermost `fn` body. A `fn` at bracket depth 0 starts a
    signature; the next `{` at depth 0 outside its generics opens its body, and a `;` or `}` first
    means it had none (a trait method, a `fn` pointer type). So does a `,` outside the signature's
    `<>` and before any `where`, or a `>` closing generics the `fn` sits inside (a `fn` pointer
    type as a struct field, `Option<fn(u8) -> u8>`). `->` is not a `>`, and a `{` inside `<>` is a
    const generic expression, whose `>=` or `>>` is at depth. Lexing is the ratchet's: comments and
    literals are skipped whole."""
    out = []
    i, n, line = 0, len(src), 1
    depth = 0        # () and [] nesting outside any body, and {} inside a signature's <>
    pending = False  # a `fn` seen, its body not yet opened
    angles = 0       # the pending signature's <> nesting at depth 0
    where = False    # the pending signature has reached its `where` clause
    body = None      # (open_line, brace depth) while inside a body
    braces = 0
    while i < n:
        c = src[i]
        if c == '\n':
            line += 1
            i += 1
            continue
        if src.startswith('//', i):
            j = src.find('\n', i)
            i = n if j < 0 else j
            continue
        if src.startswith('/*', i):
            d, j = 0, i
            while j < n:
                if src.startswith('/*', j):
                    d, j = d + 1, j + 2
                elif src.startswith('*/', j):
                    d, j = d - 1, j + 2
                    if d == 0:
                        break
                else:
                    line += src[j] == '\n'
                    j += 1
            i = j
            continue
        ident_before = i > 0 and (src[i - 1].isalnum() or src[i - 1] == '_')
        m = None if ident_before else re.match(r'(?:b|c)?r(#*)"', src[i:i + 300])
        if m:
            end = '"' + m.group(1)
            j = src.find(end, i + m.end())
            j = n if j < 0 else j + len(end)
            line += src[i:j].count('\n')
            i = j
            continue
        if c == '"' or (c in 'bc' and src[i + 1:i + 2] == '"' and not ident_before):
            j = i + (1 if c == '"' else 2)
            while j < n and src[j] != '"':
                j += 2 if src[j] == '\\' else 1
            line += src[i:j + 1].count('\n')
            i = j + 1
            continue
        if c == "'" or (c == 'b' and src[i + 1:i + 2] == "'" and not ident_before):
            k = i + (1 if c == "'" else 2)
            m = re.match(r"(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^\\'\n])'", src[k:k + 16])
            if m:
                i = k + m.end()
                continue
        m = re.match(r"'?[A-Za-z_][A-Za-z0-9_]*", src[i:i + 200])
        if m:
            if body is None and m.group(0) == 'fn' and depth == 0 and not pending:
                pending, angles, where = True, 0, False
            elif pending and m.group(0) == 'where' and depth == 0 and angles == 0:
                where = True
            i += m.end()
            continue
        if body is not None:
            if c == '{':
                braces += 1
            elif c == '}':
                braces -= 1
                if braces == 0:
                    out.append((body, line))
                    body = None
        elif c in '([' or (pending and c == '{' and (depth or angles)):
            depth += 1
        elif c in ')]' or (pending and c == '}' and depth):
            depth = max(0, depth - 1)
        elif pending and depth == 0 and src.startswith('->', i):
            i += 1
        elif pending and depth == 0 and c == '<':
            angles += 1
        elif pending and depth == 0 and c == '>':
            angles -= 1
            pending = angles >= 0
        elif pending and depth == 0 and c == '{':
            body, braces, pending = line, 1, False
        elif pending and depth == 0 and (c in ';}' or (c == ',' and not angles and not where)):
            pending = False
        i += 1
    return out


def in_body_lines(src):
    """The plain whole-line comment lines inside `fn` bodies, `// SAFETY:` comments left out."""
    ranges = fn_body_ranges(src)
    if not ranges:
        return 0
    lines = src.split('\n')
    inside = lambda a: any(o < a < c for o, c in ranges)
    count, cur = 0, None
    for kind, a, b in lex_comment_spans(src) + [('end', 0, 0)]:
        if kind == 'plain' and inside(a) and cur and a == cur[1] + 1:
            cur = [cur[0], b]
            continue
        if cur:
            span = [line.strip() for line in lines[cur[0] - 1:cur[1]]]
            safety = next((k for k, t in enumerate(span) if t.startswith('// SAFETY:')), len(span))
            count += safety
        cur = [a, b] if kind == 'plain' and inside(a) else None
    return count


def block_counts(paths, read, caps=None):
    """{path: [(first_line, lines)]} for the over-cap blocks of each path, at its directory's cap."""
    return {p: blocks_over(read(p), cap_of(p, caps or {})) for p in paths}


def in_body_counts(paths, read, prefixes):
    """{prefix: in-body lines} over the paths under each started prefix."""
    return {prefix: sum(in_body_lines(read(p)) for p in paths if p.startswith(prefix))
            for prefix in prefixes}


def rows_from_counts(counts):
    """{(path, first_line, occurrence): lines}, the occurrence separating same-line collisions."""
    rows = {}
    seen = {}
    for path, blocks in counts.items():
        for line, size in blocks:
            occ = seen.get((path, line), 0) + 1
            seen[(path, line)] = occ
            rows[(path, line, occ)] = size
    return rows


# --- git ------------------------------------------------------------------------------------------

def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


def in_scope(path):
    """A `.rs` path outside `vendor/`: the files the rules reach, here and in `script/metrics`."""
    return path.endswith('.rs') and not path.startswith('vendor/')


def tracked_rust():
    """Every in-scope path: tracked, and untracked ones too."""
    files = git('ls-files', '-z', '--cached', '--others', '--exclude-standard') or ''
    return {f for f in files.split('\0') if f and in_scope(f) and os.path.exists(f)}


BASE_OVERRIDE = None  # `--check --base REV`, for falsifying the merge-base half against a chosen commit


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


def is_cut_from(new, old):
    """True when `new` is `old` with zero or more whole lines deleted and nothing added."""
    rest = iter(old)
    return all(line in rest for line in new)


def moved_blocks(base_texts, tree_texts, renames=()):
    """[(old_path, old_line, new_path, new_line)]: the listed blocks a split carried elsewhere.

    `base_texts` is {path: [(first_line, lines, text)]} for every file with rows at the merge base,
    `tree_texts` the same for every tree file with an over-cap block. A tree block is a carried
    block when its file had no rows at the base, its text is a listed base block's text or a cut of
    it (`is_cut_from`), and the base block's own file (under its new name, if git calls it renamed)
    no longer holds anything that could be that block. Exact matches pair first, so a block moved
    unchanged is never claimed by a cut copy of itself; each base block carries at most once.
    """
    became = dict(renames)
    was_called = {new: old for old, new in renames}
    home = {}
    for old, blocks in base_texts.items():
        now = tree_texts.get(became.get(old, old), [])
        for line, _size, text in blocks:
            home[(old, line)] = any(is_cut_from(t, text) for _l, _s, t in now)
    targets = [(path, line, text) for path in sorted(tree_texts)
               if was_called.get(path, path) not in base_texts
               for line, _size, text in tree_texts[path]]
    sources = [(old, line, text) for old in sorted(base_texts)
               for line, _size, text in base_texts[old] if not home[(old, line)]]
    used, taken, moves = set(), set(), []
    for exact in (True, False):
        for path, line, text in targets:
            if (path, line) in taken:
                continue
            for old, old_line, old_text in sources:
                if (old, old_line) in used:
                    continue
                if text == old_text if exact else is_cut_from(text, old_text):
                    used.add((old, old_line))
                    taken.add((path, line))
                    moves.append((old, old_line, path, line))
                    break
    return sorted(moves)


def carry(moves, base_counts, base_rows, renames=()):
    """`base_counts` and `base_rows` as though each moved block had sat in its new file at the base.

    Every later rule then holds the moved block exactly as it holds a renamed one: its new file's
    rows may not outnumber what came with it, its ceiling may not rise, and it may not grow.
    """
    if not moves:
        return base_counts, base_rows
    was_called = {new: old for old, new in renames}
    counts = {path: list(blocks) for path, blocks in (base_counts or {}).items()}
    rows = dict(base_rows or {})
    by_file = {}
    for (path, line, occ), ceiling in (base_rows or {}).items():
        by_file.setdefault(path, []).append((line, occ, ceiling))
    for old, old_line, new, new_line in moves:
        blocks = (base_counts or {}).get(old, [])
        index = next(i for i, (line, _size) in enumerate(blocks) if line == old_line)
        size = blocks[index][1]
        counts[old] = [b for b in counts.get(old, []) if b[0] != old_line]
        dest = was_called.get(new, new)
        counts[dest] = sorted(counts.get(dest, []) + [(new_line, size)])
        listed = sorted(by_file.get(old, []))
        if index < len(listed):
            line, occ, ceiling = listed[index]
            rows.pop((old, line, occ), None)
            occ = 1
            while (dest, new_line, occ) in rows:
                occ += 1
            rows[(dest, new_line, occ)] = ceiling
    return ({p: b for p, b in counts.items() if b},
            rows if base_rows is not None else None)


# --- the rules, pure so the selftest can drive them ------------------------------------------------

def problems(counts, rows, base_counts=None, base_rows=None, renames=(), moves=(), caps=None,
             stepped=()):
    """Every failure in one tree. See the header for the rules.

    `counts` is the tree's {path: [(first_line, lines)]}; `rows` the baseline's
    {(path, first_line, occurrence): ceiling}; `base_counts` and `base_rows` the same at the merge
    base, where there was one; `renames` git's (old, new) `.rs` pairs since the base; `moves` the
    blocks a split carried, from `moved_blocks`. The baseline comparison is skipped when
    `base_rows` is None, which is the base holding no baseline: the change that first lands one.
    `caps` is the tree's {directory: cap}; `stepped` the directories whose cap this change lowered,
    where rows may be added at no more than each block's size at the base.
    """
    if not counts:
        return ['the check judged no Rust file at all. A selector that matches nothing passes by '
                'checking nothing (design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md)']
    base_counts, base_rows = carry(moves, base_counts, base_rows, renames)
    was_called = {new: old for old, new in renames}
    bad = []
    total_blocks = sum(len(blocks) for blocks in counts.values())

    # The tree: an unlisted block over the cap, and every way a listed one may not stand. Rows and
    # blocks are matched file by file in line order, so a shrink that shifts later blocks down has
    # moved its rows rather than deleting and adding them.
    by_file = {}
    for (path, line, occ), ceiling in rows.items():
        by_file.setdefault(path, []).append((line, occ, ceiling))
    for path in sorted(counts):
        blocks = counts[path]
        listed = sorted(by_file.get(path, []))
        if len(listed) < len(blocks):
            for line, size in blocks[len(listed):]:
                bad.append(f'{path}:{line}: a {fmt(size)}-line comment block against the '
                           f'{fmt(cap_of(path, caps or {}))}-line cap, and it is not on the list. A new block meets '
                           f'the cap outright; a block moved from another file keeps its row only '
                           f'when it moved unchanged or cut. {REMEDY}')
        for (line, size), (_row_line, _occ, ceiling) in zip(blocks, listed):
            if size > ceiling:
                bad.append(f'{path}:{line}: a {fmt(size)}-line comment block, over its baseline '
                           f'ceiling of {fmt(ceiling)}. A listed block may not exceed its entry. '
                           f'{REMEDY}')
    for path in sorted(by_file):
        listed = sorted(by_file[path])
        blocks = counts.get(path)
        if blocks is None:
            old = was_called.get(path, path)
            if counts.get(old) is not None:
                continue  # the rename check below carries it
            bad.append(f'{BASELINE}: {path} is not in the tree. Move its rows with the rename, at '
                       f'the same numbers, or remove them; the list cannot go stale')
        elif len(listed) > len(blocks):
            for line, occ, _ceiling in listed[len(blocks):]:
                suffix = f'#{occ}' if occ > 1 else ''
                bad.append(f'{BASELINE}: {path}:{line}{suffix} has no over-cap block behind it. '
                           f'Remove its row; the list cannot go stale')

    # The tree against its merge base: a listed block may not grow, unconditionally.
    if base_counts is not None:
        for path in sorted(counts):
            old = was_called.get(path, path)
            was = base_counts.get(old)
            if not was:
                continue
            now = counts[path]
            for (line, size), (_was_line, was_size) in zip(now, was):
                if size > was_size:
                    bad.append(f'{path}:{line}: grew from {fmt(was_size)} to {fmt(size)} lines '
                               f'against the merge base. A listed block may only shrink. {REMEDY}')

    # The list against its merge-base copy: no additions, no raises, renames at the same numbers.
    if base_rows is not None:
        base_by_file = {}
        for (path, line, occ), ceiling in base_rows.items():
            base_by_file.setdefault(path, []).append((line, occ, ceiling))
        for path in sorted(by_file):
            listed = sorted(by_file[path])
            old = was_called.get(path, path)
            was = sorted(base_by_file.get(old) or base_by_file.get(path) or [])
            if any(path.startswith(d) for d in stepped):
                # A step lists the blocks the lower cap newly reaches. Each row may stand no
                # higher than its block stood at the base, so the step adds no room to grow.
                sizes = [size for _line, size in (base_counts or {}).get(old, [])]
                if len(listed) > len(sizes):
                    bad.append(f'{BASELINE}: {path} lists {len(listed)} rows for {len(sizes)} '
                               f'blocks over its new cap at the merge base. A step lists only '
                               f'blocks that were already there')
                for (line, occ, ceiling), size in zip(listed, sizes):
                    if ceiling > size:
                        bad.append(f'{BASELINE}: {path}:{line} lists {fmt(ceiling)} lines for a '
                                   f'block of {fmt(size)} at the merge base. A step lists a block '
                                   f'at its size')
                continue
            if not was:
                bad.append(f'{BASELINE}: {path} gained rows. The list only shrinks; a new block '
                           f'meets the cap outright')
                continue
            if len(listed) > len(was):
                bad.append(f'{BASELINE}: {path} gained a row against the merge base '
                           f'({len(listed)} rows over {len(was)}). The list only shrinks; a new '
                           f'block meets the cap outright')
            for (line, occ, ceiling), (_was_line, _was_occ, was_ceiling) in zip(listed, was):
                if ceiling > was_ceiling:
                    suffix = f'#{occ}' if occ > 1 else ''
                    bad.append(f'{BASELINE}: {path}:{line}{suffix} raised its ceiling from '
                               f'{fmt(was_ceiling)} to {fmt(ceiling)}. An entry only goes down '
                               f'(§266 section 3, option (b), applied one level down)')
    return bad


STEPS = (40, 20, 10)


def table_problems(caps, base_caps, in_body, base_in_body, in_body_counts):
    """Every failure in the cap and in-body rows, against the tree and the merge base.

    A cap is one of the steps, 40, 20 or 10, and may only fall; a cap row may not go, since that
    lifts the directory back to 40. An in-body ceiling holds its directory's count and may only
    fall; one may be added, which starts a directory's stage, and none may go. `base_caps` and
    `base_in_body` are None when the base had no baseline."""
    bad = []
    for prefix, cap in sorted(caps.items()):
        if cap not in STEPS:
            bad.append(f'{BASELINE}: cap {prefix} is {cap}. A cap steps 40, 20, 10 (milestone 929)')
        if not prefix.endswith('/'):
            bad.append(f'{BASELINE}: cap {prefix} names no directory; write it with a trailing /')
    for prefix, ceiling in sorted(in_body.items()):
        now = in_body_counts.get(prefix, 0)
        if now > ceiling:
            bad.append(f'{prefix}: {fmt(now)} plain comment lines inside function bodies, over the '
                       f'ceiling of {fmt(ceiling)}. A comment in a body states a constraint the '
                       f'code cannot show; narrative goes in the item\'s doc comment or its note '
                       f'(§267)')
    if base_caps is not None:
        for prefix, was in sorted(base_caps.items()):
            if prefix not in caps:
                bad.append(f'{BASELINE}: cap {prefix} is gone. A cap only falls')
            elif caps[prefix] > was:
                bad.append(f'{BASELINE}: cap {prefix} rose from {was} to {caps[prefix]}. A cap '
                           f'only falls')
        for prefix, cap in sorted(caps.items()):
            if prefix not in base_caps and cap == CEILING:
                bad.append(f'{BASELINE}: cap {prefix} is 40, which every directory already has')
    if base_in_body is not None:
        for prefix, was in sorted(base_in_body.items()):
            if prefix not in in_body:
                bad.append(f'{BASELINE}: in-body {prefix} is gone. A ceiling only falls')
            elif in_body[prefix] > was:
                bad.append(f'{BASELINE}: in-body {prefix} rose from {fmt(was)} to '
                           f'{fmt(in_body[prefix])}. A ceiling only falls')
    return bad


def stepped_dirs(caps, base_caps):
    """The directories whose cap this change lowered."""
    return {p for p, cap in caps.items() if cap < (base_caps or {}).get(p, CEILING)}


def banked(rows, counts):
    """The baseline after banking: entries lowered to the tree, never raised, never added.

    A row whose block is gone or at or under the cap drops out, which is the rule `check` already
    enforces on the tree: banking is how a sweep leaves the list in the same change.
    """
    out = {}
    by_file = {}
    for (path, line, occ), ceiling in rows.items():
        by_file.setdefault(path, []).append((line, occ, ceiling))
    for path, listed in by_file.items():
        for (line, occ, ceiling), (block_line, block_size) in zip(sorted(listed), counts.get(path, [])):
            out[(path, block_line, occ)] = min(block_size, ceiling)
    return out


def carried_rows(moves, base_counts, base_rows, rows, renames=()):
    """`rows` with each split's rows moved to their blocks' new files, for `--bank`.

    Only a file whose rows still read as they did at the base is carried from, since a lane that
    already moved its rows by hand has done this, and moving them twice would mislabel them.
    """
    if not moves or base_rows is None:
        return rows
    untouched = {old for old, _line, _new, _new_line in moves
                 if rows_of(rows, {old}) == rows_of(base_rows, {old})}
    moves = [m for m in moves if m[0] in untouched]
    if not moves:
        return rows
    _counts, moved = carry(moves, base_counts, rows_of(base_rows, untouched), renames)
    return {**{k: v for k, v in rows.items() if k[0] not in untouched}, **moved}


def rows_of(table, paths):
    return {k: v for k, v in table.items() if k[0] in paths}


# --- the verbs ------------------------------------------------------------------------------------

def read_file(path):
    return open(path, encoding='utf-8', errors='replace').read()


def gather():
    """The tree and its merge base, as the rules take them: a dict of counts, rows, caps, in-body
    ceilings and counts, and each at the base where there is one."""
    rows, caps, in_body = read_table(open(BASELINE).read())
    paths = tracked_rust()
    g = {'counts': block_counts(paths, read_file, caps), 'rows': rows, 'caps': caps,
         'in_body': in_body, 'in_body_counts': in_body_counts(paths, read_file, in_body),
         'base_counts': None, 'base_rows': None, 'base_caps': None, 'base_in_body': None,
         'renames': (), 'moves': ()}
    base = merge_base()
    if not base:
        return g
    g['renames'] = renamed_rust(base)
    old_text = git('show', f'{base}:{BASELINE}')
    if old_text is not None:
        g['base_rows'], g['base_caps'], g['base_in_body'] = read_table(old_text)
    # The base's listed files are read too, not only the tree's, so a file whose every row a split
    # carried away is still there to carry from. Each is measured at the tree's cap for its
    # directory, so rows pair with the same blocks on both sides of a step.
    wanted = ({path for (path, _line, _occ) in rows} | {new for _old, new in g['renames']}
              | {path for (path, _line, _occ) in (g['base_rows'] or {})})
    base_counts, base_texts = {}, {}
    for path in sorted(wanted):
        text = git('show', f'{base}:{path}')
        if text is not None:
            over = block_texts(text, cap_of(path, caps))
            if over:
                base_counts[path] = [(line, size) for line, size, _text in over]
                base_texts[path] = over
    g['base_counts'] = base_counts
    if g['base_rows'] is not None:
        listed = {path for (path, _line, _occ) in g['base_rows']}
        tree_texts = {path: block_texts(read_file(path), cap_of(path, caps))
                      for path in sorted(g['counts']) if g['counts'][path]}
        g['moves'] = moved_blocks({p: t for p, t in base_texts.items() if p in listed},
                                  tree_texts, g['renames'])
    return g


def all_problems(g):
    return (problems(g['counts'], g['rows'], g['base_counts'], g['base_rows'], g['renames'],
                     g['moves'], g['caps'], stepped_dirs(g['caps'], g['base_caps']))
            + table_problems(g['caps'], g['base_caps'], g['in_body'], g['base_in_body'],
                             g['in_body_counts']))


def run_check():
    if not os.path.exists(BASELINE):
        print(f'comment-block ratchet: {BASELINE} is missing. The gate and its list are one change; '
              f'deleting one deletes the gate', file=sys.stderr)
        return 1
    g = gather()
    bad = all_problems(g)
    if bad:
        print('comment-block ratchet: the tree breaks §267 against its baseline:', file=sys.stderr)
        for b in bad:
            print(f'  {b}', file=sys.stderr)
        print('\nMove the history to a commit message and the findings to a note, bank the shrink '
              'with --bank, or amend §267 with an architect. See milestone 860\'s block, '
              'design/roadmap/0860-comments-state-the-constraint-as-it-is-now.md, and milestone '
              '929\'s, design/roadmap/0929-kernel-comments-in-the-linux-style.md.', file=sys.stderr)
        return 1
    over = sum(len(blocks) for blocks in g['counts'].values())
    carried = f', {len(g["moves"])} carried by a split' if g['moves'] else ''
    caps = ''.join(f', {p} capped at {c}' for p, c in sorted(g['caps'].items()))
    bodies = ''.join(f'; {p} holds {fmt(g["in_body_counts"][p])} in-body comment lines against '
                     f'{fmt(c)}' for p, c in sorted(g['in_body'].items()))
    print(f'comment-block ratchet: {len(g["counts"])} Rust files, {fmt(over)} comment blocks over '
          f'the cap (40{caps}), all held to {len(g["rows"])} baseline rows{carried}{bodies}')
    return 0


def bank(init=False, step=None):
    """Write the baseline. `--bank` lowers rows and in-body ceilings to the tree, never raising
    or adding. `--step DIR CAP` is a bank that also lowers DIR's cap, lists every block over the
    new cap at its size, and starts DIR's in-body ceiling at its count if it has none."""
    if init and os.path.exists(BASELINE):
        print(f'comment-block ratchet: {BASELINE} already exists. --init wrote it once, in '
              f'milestone 860; it is refused now, and --bank is the verb that never raises',
              file=sys.stderr)
        return 2
    if not init and not os.path.exists(BASELINE):
        print(f'comment-block ratchet: {BASELINE} does not exist; there is nothing to bank',
              file=sys.stderr)
        return 2
    if init:
        counts = block_counts(tracked_rust(), read_file)
        write_baseline(rows_from_counts(counts))
        print(f'comment-block ratchet: {len(counts)} files measured into {BASELINE}')
        return 0
    rows, caps, in_body = read_table(open(BASELINE).read())
    if step:
        prefix, cap = step
        if cap not in STEPS or cap >= cap_of(prefix, caps) or not prefix.endswith('/'):
            print(f'comment-block ratchet: --step {prefix} {cap}: a step names a directory with a '
                  f'trailing / and lowers its cap ({cap_of(prefix, caps)}) to the next of '
                  f'{", ".join(map(str, STEPS))}', file=sys.stderr)
            return 2
        caps[prefix] = cap
        paths = tracked_rust()
        in_body.setdefault(prefix, in_body_counts(paths, read_file, [prefix])[prefix])
        # The directory's rows are rewritten at the tree's sizes. A listed block was already at or
        # under its row (the check holds that), so no row rises.
        counts = block_counts([p for p in paths if p.startswith(prefix)], read_file, caps)
        rows = {k: v for k, v in rows.items() if not k[0].startswith(prefix)}
        rows.update(rows_from_counts(counts))
        write_baseline(rows, caps, in_body)
    g = gather()
    rows = banked(carried_rows(g['moves'], g['base_counts'], g['base_rows'], g['rows'],
                               g['renames']), g['counts'])
    in_body = {p: min(c, g['in_body_counts'][p]) for p, c in g['in_body'].items()}
    write_baseline(rows, g['caps'], in_body)
    print(f'comment-block ratchet: {len(rows)} rows written to {BASELINE}')
    return 0


def selftest():
    """The rules as fixtures: the cap, the ratchet's every refusal, banking, and the traps."""
    RENAME = [('x/old.rs', 'x/renamed.rs')]

    def tree(**files):
        return {f'x/{name}.rs': blocks for name, blocks in files.items()}

    def rows(path, *pairs):
        return {(path, line, occ): size for line, occ, size in pairs}

    cases = []

    def fails(name, bad):
        cases.append((name, bool(bad)))

    def passes(name, bad):
        cases.append((name, not bad))

    BIG = [(100, 50)]
    BIGGER = [(100, 60)]

    # The cap: a new 41-line block fails; the measure never hands `problems` one at 40 or below,
    # so a block that left the cap simply stops being counted.
    fails('a new 41-line block fails', problems(tree(a=[(10, 41)]), {}))
    passes('a block at the cap is not counted at all', problems(tree(a=[]), {}))
    # A listed block: over its entry fails, growth against the merge base fails even within it.
    row = rows('x/a.rs', (100, 1, 55))
    base = tree(a=BIG)
    fails('a listed block over its entry fails', problems(tree(a=BIGGER), row, base, row))
    fails('a listed block grown against the merge base fails',
          problems(tree(a=[(100, 52)]), rows('x/a.rs', (100, 1, 60)), base, row))
    passes('a listed block unchanged passes', problems(base, row, base, row))
    passes('a listed block shrunk passes', problems(tree(a=[(100, 45)]), row, base, row))
    # A vanished block's row must go; a file's rows cannot outnumber its over-cap blocks.
    fails('a row whose block left the cap fails', problems(tree(a=[]), row, base, row))
    fails('a row for a deleted file fails', problems(tree(b=BIG), row, {}, row))
    passes('a shrunk block passes once its row is gone', problems(tree(a=[]), {}, base, row))
    # Two blocks in one file pair in line order; the second's row moves with a shift above it.
    two = tree(a=[(100, 50), (300, 60)])
    two_rows = rows('x/a.rs', (100, 1, 50), (300, 1, 60))
    passes('two listed blocks pass', problems(two, two_rows, two, two_rows))
    shifted = tree(a=[(100, 45), (290, 60)])
    shifted_rows = rows('x/a.rs', (100, 1, 45), (290, 1, 60))
    passes('a shrink that shifts the second block moves its row',
           problems(shifted, shifted_rows, two, two_rows))
    fails('a shifted row raised against the merge base fails',
          problems(shifted, rows('x/a.rs', (100, 1, 45), (290, 1, 61)), two, two_rows))
    # The list against its merge-base copy: rows are never added, entries never rise.
    fails('a row added against the merge base fails',
          problems(two, rows('x/a.rs', (100, 1, 50), (300, 1, 60), (500, 1, 45)),
                   tree(a=BIG), row))
    fails('an entry raised against the merge base fails',
          problems(base, rows('x/a.rs', (100, 1, 56)), base, row))
    fails('dropping a row whose block is still over the cap fails',
          problems(base, {}, base, row))
    # Renames move rows in the same change, at the same numbers.
    passes('a rename that moves its rows at the same numbers passes',
           problems(tree(renamed=BIG), rows('x/renamed.rs', (100, 1, 50)),
                    tree(old=BIG), rows('x/old.rs', (100, 1, 50)), RENAME))
    fails('a rename that leaves its rows behind fails',
          problems(tree(renamed=BIG), rows('x/old.rs', (100, 1, 50)),
                   tree(old=BIG), rows('x/old.rs', (100, 1, 50)), RENAME))
    # A split carries a row the way a rename does (§267, amended 2026-10-11 UTC): lib.rs holds two
    # listed blocks at the base and the split moves the second into a new file, fs.rs.
    lib_base = tree(lib=[(100, 50), (300, 60)])
    lib_rows = rows('x/lib.rs', (100, 1, 50), (300, 1, 60))
    MOVE = [('x/lib.rs', 300, 'x/fs.rs', 20)]
    split_rows = {**rows('x/lib.rs', (100, 1, 50)), **rows('x/fs.rs', (20, 1, 60))}
    passes('a block moved unchanged keeps its row under its new path',
           problems(tree(lib=[(100, 50)], fs=[(20, 60)]), split_rows, lib_base, lib_rows, (), MOVE))
    fails('without the carry the same split fails',
          problems(tree(lib=[(100, 50)], fs=[(20, 60)]), split_rows, lib_base, lib_rows))
    fails('a moved block whose row stays under the old path fails',
          problems(tree(lib=[(100, 50)], fs=[(20, 60)]), lib_rows, lib_base, lib_rows, (), MOVE))
    fails('a moved block grown past its base size fails',
          problems(tree(lib=[(100, 50)], fs=[(20, 61)]),
                   {**rows('x/lib.rs', (100, 1, 50)), **rows('x/fs.rs', (20, 1, 61))},
                   lib_base, lib_rows, (), MOVE))
    fails('a carried row raised above its base ceiling fails',
          problems(tree(lib=[(100, 50)], fs=[(20, 60)]),
                   {**rows('x/lib.rs', (100, 1, 50)), **rows('x/fs.rs', (20, 1, 61))},
                   lib_base, lib_rows, (), MOVE))
    passes('a block moved and cut passes, its row at the old ceiling until banked',
           problems(tree(lib=[(100, 50)], fs=[(20, 55)]), split_rows, lib_base, lib_rows, (), MOVE))
    passes('a block moved and cut passes, its row lowered',
           problems(tree(lib=[(100, 50)], fs=[(20, 55)]),
                    {**rows('x/lib.rs', (100, 1, 50)), **rows('x/fs.rs', (20, 1, 55))},
                    lib_base, lib_rows, (), MOVE))
    fails('an unlisted over-cap block in a new file still fails',
          problems(tree(lib=[(100, 50), (300, 60)], fs=[(20, 45)]), lib_rows, lib_base, lib_rows,
                   (), MOVE[:0]))
    fails('a second over-cap block in the new file, beside the carried one, fails',
          problems(tree(lib=[(100, 50)], fs=[(20, 60), (200, 45)]), split_rows, lib_base,
                   lib_rows, (), MOVE))
    passes('a split beside a rename carries its row from the old name',
           problems(tree(mod=[(100, 50)], fs=[(20, 60)]),
                    {**rows('x/mod.rs', (100, 1, 50)), **rows('x/fs.rs', (20, 1, 60))},
                    lib_base, lib_rows, [('x/lib.rs', 'x/mod.rs')], MOVE))
    # The match is by content. Lines are compared without their indentation.
    old = tuple(f'// line {i}' for i in range(60))
    base_text = {'x/lib.rs': [(100, 50, tuple(f'// a {i}' for i in range(50))), (300, 60, old)]}
    keep = {'x/lib.rs': [base_text['x/lib.rs'][0]]}
    if moved_blocks(base_text, {**keep, 'x/fs.rs': [(20, 60, old)]}) != MOVE:
        cases.append(('a block moved unchanged is found by its content', False))
    cut = tuple(l for i, l in enumerate(old) if i % 12)
    if moved_blocks(base_text, {**keep, 'x/fs.rs': [(20, len(cut), cut)]}) != MOVE:
        cases.append(('a block moved and cut is found by its content', False))
    grown = old[:30] + ('// added',) + old[30:]
    if moved_blocks(base_text, {**keep, 'x/fs.rs': [(20, 61, grown)]}):
        cases.append(('a block moved and grown carries nothing', False))
    edited = old[:30] + ('// edited',) + old[31:]
    if moved_blocks(base_text, {**keep, 'x/fs.rs': [(20, 60, edited)]}):
        cases.append(('a block moved with a line edited carries nothing', False))
    if moved_blocks(base_text, {**base_text, 'x/fs.rs': [(20, 60, old)]}):
        cases.append(('a copy that leaves the block at home carries nothing', False))
    two = moved_blocks(base_text, {**keep, 'x/fs.rs': [(20, 60, old)], 'x/verb.rs': [(5, 60, old)]})
    if len(two) != 1:
        cases.append(('a block copied into two new files carries one row, not two', False))
    if moved_blocks(base_text, {**keep, 'x/fs.rs': [(20, 60, old)]}, [('x/lib.rs', 'x/fs.rs')]):
        cases.append(('a block in a renamed file is the rename\'s, not a split\'s', False))
    if moved_blocks(base_text, {'x/lib.rs': [base_text['x/lib.rs'][0], (900, 60, old)],
                                'x/fs.rs': []}):
        cases.append(('a block that moved within its own file carries nothing', False))
    # --bank writes the carried row under the new path, lowered to the tree.
    if banked(carried_rows(MOVE, lib_base, lib_rows, lib_rows), tree(lib=[(100, 50)], fs=[(20, 55)])) \
            != {**rows('x/lib.rs', (100, 1, 50)), **rows('x/fs.rs', (20, 1, 55))}:
        cases.append(('bank carries a split\'s row to its new path and lowers it', False))
    if carried_rows(MOVE, lib_base, lib_rows, split_rows) != split_rows:
        cases.append(('bank leaves rows a lane already moved by hand', False))
    # A selector that matches nothing.
    fails('a selector that matches nothing fails loudly', problems({}, {}))
    # Banking lowers to the tree, never raises, never adds, and drops dead rows.
    if banked(rows('x/a.rs', (100, 1, 55), (300, 1, 60)),
              tree(a=[(100, 50)])) != rows('x/a.rs', (100, 1, 50)):
        cases.append(('bank lowers an entry to the tree and drops a dead row', False))
    if banked(rows('x/a.rs', (100, 1, 50)), tree(a=[(100, 55)])) != rows('x/a.rs', (100, 1, 50)):
        cases.append(('bank never raises an entry', False))
    if banked(rows('x/a.rs', (100, 1, 50)), tree(a=[(100, 50), (400, 45)])) != \
            rows('x/a.rs', (100, 1, 50)):
        cases.append(('bank never adds a row', False))
    # The lexer: the block definition, doc versus plain, strings that hold comment markers.
    src = '//! a\n//! b\nlet x = 1;\n// c\n/* d\ne */\nlet s = "// not a comment";\n'
    if blocks_over('//! a\n' * 41) != [(1, 41)] or blocks_over('//! a\n' * 40) != []:
        cases.append(('a run of 41 doc lines is one block over the cap, 40 is not', False))
    if lex_comment_spans(src) != \
            [('doc', 1, 1), ('doc', 2, 2), ('plain', 4, 4), ('plain', 5, 6)]:
        cases.append(('adjacent spans of one kind merge, kinds differ, strings are skipped', False))
    if blocks_over('let s = r#"// not\na comment"#; // trailing\n// next\n'):
        cases.append(('raw strings do not start blocks', False))
    # Trailing comments: crates/bitmap_font's glyph table, 128 rows each ending `// 0x41 A`, read
    # as one 128-line block under the note's lexer.
    table = ''.join(f'    [0x00, 0x7e], // 0x{k:02x}\n' for k in range(128))
    if blocks_over(table, 0) != []:
        cases.append(('a table annotated on every row holds no block of any size', False))
    real = '// a\n' * 41
    if blocks_over('let x = 1;\n' + real + table) != [(2, 41)]:
        cases.append(('a 41-line // block beside an annotated table is still one block over the '
                      'cap', False))
    fails('a 41-line // block from the lexer still fails the cap',
          problems({'x/a.rs': blocks_over(real)}, {}))
    if blocks_over('// a\n' * 30 + 'f(); // b\n' + '// a\n' * 30, 0) != [(1, 30), (32, 30)]:
        cases.append(('an annotated code line ends the run around it', False))
    if lex_comment_spans('/* a */ f(); /* b */ // c\n/* d */ // e\n') != \
            [('plain', 1, 1), ('trailing', 1, 1), ('trailing', 1, 1), ('plain', 2, 2),
             ('plain', 2, 2)]:
        cases.append(('a comment after code trails, a comment after only comments does not', False))
    # Milestone 929: a cap per directory, stepped 40, 20, 10, and a step that lists its blocks.
    if cap_of('kernel/src/a.rs', {'kernel/': 20, 'kernel/src/arch/': 10}) != 20 or \
            cap_of('kernel/src/arch/x.rs', {'kernel/': 20, 'kernel/src/arch/': 10}) != 10 or \
            cap_of('crates/a.rs', {'kernel/': 20}) != 40:
        cases.append(('a path takes its longest directory prefix\'s cap, else 40', False))
    K = {'x/': 20}
    msg = problems(tree(a=[(10, 25)]), {}, caps=K)
    fails('a block over a lowered cap and unlisted fails', msg)
    if msg and '20-line cap' not in msg[0]:
        cases.append(('the failure names the directory\'s cap', False))
    at_base = tree(a=[(10, 25), (90, 30)])
    step_rows = rows('x/a.rs', (10, 1, 25), (90, 1, 30))
    passes('a step lists the blocks its cap reaches, at their sizes',
           problems(at_base, step_rows, at_base, {}, caps=K, stepped={'x/'}))
    fails('a step that lists a block above its base size fails',
          problems(at_base, rows('x/a.rs', (10, 1, 26), (90, 1, 30)), at_base, {}, caps=K,
                   stepped={'x/'}))
    fails('a step that lists more rows than blocks fails',
          problems(at_base, {**step_rows, ('x/a.rs', 200, 1): 22}, at_base, {}, caps=K,
                   stepped={'x/'}))
    fails('a block grown in the step\'s own change fails',
          problems(tree(a=[(10, 27), (90, 30)]), rows('x/a.rs', (10, 1, 27), (90, 1, 30)),
                   at_base, {}, caps=K, stepped={'x/'}))
    fails('rows added with no step fail', problems(at_base, step_rows, at_base, {}, caps=K))
    passes('a cap that falls a step passes', table_problems({'x/': 20}, {}, {}, {}, {}))
    fails('a cap that is not a step fails', table_problems({'x/': 25}, {}, {}, {}, {}))
    fails('a cap that names no directory fails', table_problems({'x': 20}, {}, {}, {}, {}))
    fails('a cap that rises fails', table_problems({'x/': 20}, {'x/': 10}, {}, {}, {}))
    fails('a cap row that goes fails', table_problems({}, {'x/': 20}, {}, {}, {}))
    if stepped_dirs({'x/': 20, 'y/': 10}, {'y/': 10}) != {'x/'}:
        cases.append(('a step is a cap below its base value', False))
    # The in-body ceiling: held, only falls, may start, may not go.
    passes('an in-body count at its ceiling passes', table_problems({}, {}, {'x/': 9}, {}, {'x/': 9}))
    fails('an in-body count over its ceiling fails', table_problems({}, {}, {'x/': 9}, {}, {'x/': 10}))
    fails('an in-body ceiling that rises fails',
          table_problems({}, {}, {'x/': 10}, {'x/': 9}, {'x/': 9}))
    fails('an in-body ceiling that goes fails', table_problems({}, {}, {}, {'x/': 9}, {}))
    passes('an in-body ceiling may start', table_problems({}, {}, {'x/': 9}, {}, {'x/': 9}))
    # What the cap does not count: a SAFETY comment from its marker, a Name: paragraph.
    safety = '// why\n' * 5 + '// SAFETY: the lock is held.\n' + '// more of the argument\n' * 30
    if blocks_over(safety, 10) or blocks_over('// why\n' * 11 + '// SAFETY: x\n', 10) != [(1, 11)]:
        cases.append(('a SAFETY comment counts only the lines before its marker', False))
    named = '//! A crate.\n//!\n//! Name: provisional, 2026-10-22.\n' + '//! refused x\n' * 20 + \
            '//!\n' + '//! more doc\n' * 8
    if blocks_over(named, 10) != [(1, 11)] or blocks_over(named, 11):
        cases.append(('a Name: paragraph is not counted, the doc around it is', False))
    if block_texts(named, 10)[0][2][-1] != '//! more doc':
        cases.append(('a block\'s text spans its exempt lines, so a split still finds it', False))
    # The in-body count: plain lines in fn bodies, nested closures in, SAFETY out, signatures and
    # fn-pointer types are not bodies, and a { inside a string is not a brace.
    body = ('/// doc\nfn a(x: fn(u8) -> u8) -> impl Fn() {\n    // one\n    // two\n'
            '    let s = "{ // no";\n    // SAFETY: fine\n    // still the argument\n'
            '    move || {\n        // three\n    }\n}\ntrait T { fn b(&self); }\n'
            'struct S { f: fn(u8), }\nimpl S {\n    fn c(&self) -> [u8; { 2 }] {\n'
            '        /* four\n           five */\n        [0; 2]\n    }\n}\n// outside\n')
    if fn_body_ranges(body) != [(2, 11), (15, 19)] or in_body_lines(body) != 5:
        cases.append(('the in-body count finds bodies and counts their plain lines', False))
    # A comma inside a signature's <> or where clause does not end the fn; a `fn` pointer type
    # inside generics does, at the `>` that closes them, and `->`, `>=`, `>>`, `<<` are not <>.
    bodied = {
        'a comma in a return type\'s generics': 'fn f() -> Result<A, B> {\n    // c\n}\n',
        'a comma in the fn\'s own generics': 'fn f<A, B>(a: A) -> B {\n    // c\n}\n',
        'a comma in a where clause': 'fn f<T>() where T: Clone, T: Copy {\n    // c\n}\n',
        'an arrow inside generics': 'fn f() -> Box<dyn Fn(u8, u8) -> u8> {\n    // c\n}\n',
        'a shift in a const generic expression':
            'fn f<const N: usize>() -> Foo<{ N >> 1 }, { N << 1 }> {\n    // c\n}\n',
        'a comparison in a where clause':
            'fn f<const N: usize>() where Foo<{ N >= 1 }>: Is, [(); N >> 1]: Sized {\n'
            '    // c\n}\n',
        'a fn pointer inside generics in a parameter':
            'fn f(g: Option<fn(u8) -> u8>, h: Vec<Vec<u8>>) -> Vec<Vec<u8>> {\n    // c\n}\n',
        'a fn pointer inside generics in a where clause':
            'fn f<F>() where F: Into<fn(u8) -> u8>, {\n    // c\n}\n',
    }
    for name, src in bodied.items():
        if fn_body_ranges(src) != [(1, 3)] or in_body_lines(src) != 1:
            cases.append((f'{name} does not end the fn', False))
    fields = ('struct S { f: Option<fn(u8) -> u8>, g: HashMap<u8, fn()>, }\n'
              'type P = Vec<fn(u8, u8) -> Result<A, B>>;\nfn h() -> Result<A, B> {\n    // c\n}\n')
    if fn_body_ranges(fields) != [(3, 5)] or in_body_lines(fields) != 1:
        cases.append(('a fn pointer type inside generics has no body', False))

    # The message names the file, the line, the sizes and the remedy.
    msg = problems(tree(a=[(10, 41)]), {})[0]
    if not all(s in msg for s in ('a.rs:10', '41', '40', 'commit message', 'note', 'never to delete')):
        cases.append(('the failure message names the file, line, sizes and remedy', False))

    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f'comment-block ratchet selftest: {name}', file=sys.stderr)
    if not failed:
        print(f'comment-block ratchet selftest: {len(cases)} cases pass')
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
    if argv[0] in ('--bank', '--init'):
        return bank(init=argv[0] == '--init')
    if argv[0] == '--step' and len(argv) == 3 and argv[2].isdigit():
        return bank(step=(argv[1], int(argv[2])))
    print(__doc__.split('\n\n')[1], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
