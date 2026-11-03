"""The line-citation gate: cite a document by heading and code by symbol, never by line number.

§277 (cite a heading or a symbol, never a line number), calef, 2026-10-11 (UTC):

    "Can we establish a standard that we don't link to line numbers but instead section headers
    in documents?"

and, asked the same day whether citations into source code are folded in: "Yes". A line number
moves with every edit above it, and the same day's §212 (a prose budget) splits left every `notes/X.md:NNN` that
pointed into sixty notes stale at once. A heading anchor or a symbol name moves with its text.

    python3 helpers/line_citations.py --check          # what script/lint runs
    python3 helpers/line_citations.py --list [PATH..]  # every counted citation, path:line: target
    python3 helpers/line_citations.py --bank           # lower CODE_CEILING to the tree; never raises
    python3 helpers/line_citations.py --selftest       # the traps, as fixtures

Name: provisional, minted by maintainer/no-line-links-into-docs on 2026-10-11 (UTC). A shared
python module under `helpers/`, which `script/names` puts out of its own scope, so its provenance
is this paragraph. calef names things; expect this to change.

**What counts.** In any tracked text file outside the exclusions below, a path followed by a line
number: `notes/net.md:63`, `notes/x.md:105-115`, `kernel/src/sched.rs:123`, `sched.rs:12-40`, or a
URL fragment `x.md#L63` / `x.rs#L12-L40`. A path is anything ending in a known source or document
extension (`TARGET_EXTENSIONS`), or an extensionless file under `script/`, `helpers/` or
`.githooks/`.

**Two halves, two rules.**

- A citation into a Markdown document is held at zero. The tree had 38 when the gate landed: 35
  were repointed to a heading in the same change, two carry the marker below, and one was quoted
  output in an indented code block. There is nothing to ratchet.
- A citation into code is held by a ratchet, the shape of helpers/cap_abbreviation.py: the tree
  total may not exceed `CODE_CEILING`, which guards `main`, and a branch may not add any net over
  the files it changed against its merge base. `--bank` lowers the ceiling as a sweep repoints
  them; nothing raises it.

**What is not a citation, decided structurally, each for a stated reason:**

- `path:LINE:COLUMN`. That is the shape of a compiler diagnostic, a panic message and a grep
  hit: quoted tool output, not a pointer a writer chose.
- Anything inside a code block in a Markdown file, fenced or indented four spaces after a blank
  line (not a list item's continuation). Code blocks hold pasted output and the `EXAMPLES` a
  brief or note shows; a citation a writer means sits in prose.
- A GitHub `blob/<commit sha>/...#L12` permalink. It names an immutable file, so it cannot drift.
- Files that are output or data, not writing (`DATA_EXTENSIONS`: logs, TSV and CSV tables, patches,
  JSON), and `vendor/`, `patches/`, `target/`, which are somebody else's text.
- This file, whose fixtures are the patterns it looks for.

**An exception is marked at the citation.** `line-citation: exception. Reason: ...` on the same
line or the line directly above it, in an HTML comment in Markdown or a comment in code. Use it for
quoted output that is not in a fence, and say why in the reason. An unmarked exception would read
as a citation style, and the next writer would copy it.

BUGS:
- Prose that names a line in words ("line 88 of notes/scheduler.md") is not read. The form the
  tree actually used was `path:N`, which is what this matches.
- `path:LINE: text` with a trailing colon is the shape of grep and lint output, but it is counted,
  because a writer can put a colon after a citation. Pasted output of that shape outside a fence
  needs the marker.
- A citation into code inside a code comment and a citation in a string literal are counted
  alike. A test fixture that holds `foo.rs:12` as data needs the marker or a fence.
"""

import os
import re
import subprocess
import sys

sys.dont_write_bytecode = True

SELF = 'helpers/line_citations.py'
MARKER = 'line-citation: exception'

# Citations into code on 2026-10-11 (UTC), 835 in 161 files, and since lowered by `--bank`. Never raise it by hand: a
# new citation names a symbol, not a line.
CODE_CEILING = 664

DOC_EXTENSIONS = ('md',)
CODE_EXTENSIONS = ('rs', 'S', 's', 'py', 'sh', 'c', 'h', 'toml', 'yml', 'yaml', 'ld', 'lds', 'x',
                   'awk', 'jq', 'js', 'ts', 'nix')
TARGET_EXTENSIONS = DOC_EXTENSIONS + CODE_EXTENSIONS
DATA_EXTENSIONS = ('.log', '.tsv', '.csv', '.patch', '.diff', '.json', '.svg', '.txt', '.lock')
EXCLUDED_PREFIXES = ('vendor/', 'patches/', 'target/')

_PATH = (r'(?:[A-Za-z0-9_./][A-Za-z0-9_.\-/]*\.(?:' + '|'.join(TARGET_EXTENSIONS) + r')'
         r'|(?:script|helpers|\.githooks)/[A-Za-z0-9_\-]+)')
CITATION = re.compile(
    r'(?<![A-Za-z0-9_.\-/])(?P<path>' + _PATH + r')'
    r'(?:(?P<colon>:(?P<line>\d+)(?:-\d+)?)|(?P<frag>#L\d+(?:-L?\d+)?))'
    r'(?![A-Za-z0-9_])')
PERMALINK = re.compile(r'/blob/[0-9a-f]{7,40}/\S*$')
FENCE = re.compile(r'^\s{0,3}(```|~~~)')
LIST_ITEM = re.compile(r'^\s*(?:[-*+]|\d+[.)])\s')


def in_scope(path):
    if path.startswith(EXCLUDED_PREFIXES) or path == SELF:
        return False
    return not path.endswith(DATA_EXTENSIONS)


def kind(target):
    return 'doc' if target.endswith('.md') else 'code'


def file_hits(text, path='', honor_marks=True):
    """[(line, target, kind)] for every line-number citation in one file's text.

    `honor_marks=False` reads the text as if no marker were there, so the citations a marker excuses
    are the difference (helpers/exception_markers.py)."""
    out = []
    markdown = path.endswith('.md')
    fenced = None
    indented = False   # inside an indented code block
    in_list = False    # the last prose line was a list item or its continuation
    previous = ''
    for n, line in enumerate(text.split('\n'), 1):
        if markdown:
            f = FENCE.match(line)
            if f and fenced is None:
                fenced = f.group(1)
                previous = line
                continue
            if fenced is not None:
                if f and f.group(1) == fenced:
                    fenced = None
                previous = line
                continue
            if not line.strip():
                previous = line
                continue
            if line.startswith('    ') and (indented or (not previous.strip() and not in_list)):
                indented = True
                previous = line
                continue
            indented = False
            in_list = bool(LIST_ITEM.match(line)) or (in_list and line.startswith(' '))
        marked = MARKER in line or MARKER in previous
        previous = line
        if marked and honor_marks:
            continue
        for m in CITATION.finditer(line):
            # `path:LINE:COLUMN` is a diagnostic or a panic message, quoted.
            if m.group('colon') and re.match(r':\d', line[m.end():]):
                continue
            if m.group('frag') and PERMALINK.search(line[:m.start('frag')]):
                continue
            out.append((n, m.group(0), kind(m.group('path'))))
    return out


# --- counting -----------------------------------------------------------------------------------

def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True, cwd=REPO)
    return r.stdout.decode(errors='replace') if r.returncode == 0 else None


def read(path):
    try:
        with open(path, encoding='utf-8') as f:
            return f.read()
    except (UnicodeDecodeError, OSError):
        return None


def tracked():
    files = git('ls-files', '-z', '--cached', '--others', '--exclude-standard') or ''
    return [f for f in files.split('\0')
            if f and in_scope(f) and os.path.isfile(f) and not os.path.islink(f)]


def census(paths):
    out = {}
    for p in paths:
        t = read(p)
        if t is not None and '\0' not in t:
            hits = file_hits(t, p)
            if hits:
                out[p] = hits
    return out


def count(by_path, which):
    return sum(1 for hits in by_path.values() for h in hits if h[2] == which)


def merge_base():
    base = (git('merge-base', 'HEAD', 'origin/main') or '').strip()
    head = (git('rev-parse', 'HEAD') or '').strip()
    return base if base and base != head else None


def growth(base):
    """Net code citations, base to working tree, over the files this branch changed."""
    old_paths, new_paths = set(), set()
    for row in (git('diff', '--name-status', '-M', base) or '').splitlines():
        cells = row.split('\t')
        status, paths = cells[0], cells[1:]
        if status[0] in 'RC':
            old_paths.add(paths[0])
            new_paths.add(paths[1])
        elif status == 'D':
            old_paths.add(paths[0])
        elif status == 'A':
            new_paths.add(paths[0])
        else:
            old_paths.add(paths[0])
            new_paths.add(paths[0])
    for f in (git('ls-files', '-z', '--others', '--exclude-standard') or '').split('\0'):
        if f:
            new_paths.add(f)

    def code(text, p):
        return sum(1 for h in file_hits(text or '', p) if h[2] == 'code')

    where = {}
    for p in old_paths:
        if in_scope(p):
            where[p] = where.get(p, 0) - code(git('show', f'{base}:{p}'), p)
    for p in new_paths:
        if in_scope(p) and os.path.isfile(p):
            where[p] = where.get(p, 0) + code(read(p), p)
    return sum(where.values()), where


# --- the commands -------------------------------------------------------------------------------

def check():
    bad = []
    found = census(tracked())
    docs = [(p, h) for p, hits in sorted(found.items()) for h in hits if h[2] == 'doc']
    for p, (line, target, _) in docs:
        bad.append(f'{p}:{line}: cites {target}; cite the heading instead '
                   f'(`X.md#the-heading`, or quote the heading text)')
    code = count(found, 'code')
    if code > CODE_CEILING:
        bad.append(f'{code:,} line-number citations into code, over the ceiling of '
                   f'{CODE_CEILING:,} in {SELF}. If this branch adds none, main moved past it: '
                   f'rebase, and set the ceiling to the merged count in this change, saying why')
    base = merge_base()
    if base:
        net, where = growth(base)
        if net > 0:
            grew = sorted(p for p, d in where.items() if d > 0)
            bad.append(f'this branch adds {net} net line-number citation(s) into code, in '
                       f'{", ".join(grew)}; cite the function, type or test by name')
    return code, bad


def bank():
    now = count(census(tracked()), 'code')
    text = open(SELF).read()
    text = re.sub(r'(?m)^(CODE_CEILING = )[\d_]+$', rf'\g<1>{min(now, CODE_CEILING):_}', text,
                  count=1)
    with open(SELF, 'w') as f:
        f.write(text)
    return now


def selftest():
    md = 'notes/x.md'
    cases = [
        # (file the text is in, text, the targets that must count)
        (md, 'See `notes/net.md:63` and notes/x.md:105-115.', ['notes/net.md:63', 'notes/x.md:105-115']),
        (md, 'A fragment [here](../notes/net.md#L63) and [there](x.rs#L12-L40).',
         ['../notes/net.md#L63', 'x.rs#L12-L40']),
        (md, '`kernel/src/sched.rs:123`, sched.rs:12-40, script/lint:99.',
         ['kernel/src/sched.rs:123', 'sched.rs:12-40', 'script/lint:99']),
        # A heading anchor, a path with no number, a time of day, a ratio, a version.
        (md, '[x](notes/net.md#the-inbound-half), notes/net.md, at 10:30, 3:1, rust 1.80:', []),
        # Quoted tool output: line and column.
        (md, "panicked at system_tests/src/user/confined_fuzzer_tests.rs:1244:60:", []),
        # A commit-pinned permalink cannot drift; a branch one can.
        (md, 'https://github.com/o/r/blob/4af8ff980/kernel/src/sched.rs#L12', []),
        (md, 'https://github.com/o/r/blob/main/kernel/src/sched.rs#L12',
         ['//github.com/o/r/blob/main/kernel/src/sched.rs#L12']),
        # A fence holds output, and is skipped until it closes with its own marker.
        (md, '```\nnotes/net.md:63: lint said\n~~~\nstill fenced x.rs:3\n```\nnotes/net.md:9',
         ['notes/net.md:9']),
        # An indented code block is output too, blank lines inside it included; a list item's
        # indented continuation is prose.
        (md, 'Text.\n\n    $ lint\n    notes/net.md:63: said\n\n    notes/net.md:64\nnotes/net.md:65',
         ['notes/net.md:65']),
        (md, '- item\n\n    notes/net.md:63 continues it', ['notes/net.md:63']),
        # The marker, on the line or the line above.
        (md, '<!-- line-citation: exception. Reason: quoted output -->\nnotes/net.md:63: x\n'
             'notes/net.md:64', ['notes/net.md:64']),
        # In code, a fence means nothing; the marker still does.
        ('kernel/src/a.rs', '// ```\n// see notes/net.md:63\n// line-citation: exception. Reason: '
                            'example output\n//   notes/net.md:64: said', ['notes/net.md:63']),
        # A longer word that merely contains a path is not one.
        (md, 'foo.mdx:3 and xfoo.rs_bak:4', []),
    ]
    failed = 0
    for path, text, want in cases:
        got = [t for _, t, _ in file_hits(text, path)]
        if got != want:
            failed += 1
            print(f'line-citations selftest: {text!r} counted {got}, wanted {want}',
                  file=sys.stderr)
    kinds = [k for _, _, k in file_hits('a.md:1 b.rs:2 script/lint:3', md)]
    if kinds != ['doc', 'code', 'code']:
        failed += 1
        print(f'line-citations selftest: kinds {kinds}', file=sys.stderr)
    for p, want in (('vendor/x.md', False), ('bench/a.log', False), ('notes/d.tsv', False),
                    (SELF, False), ('notes/x.md', True), ('script/lint', True)):
        if in_scope(p) != want:
            failed += 1
            print(f'line-citations selftest: in_scope({p!r}) is {not want}', file=sys.stderr)
    if failed:
        return 1
    print(f'line-citations selftest: {len(cases) + 7} fixtures')
    return 0


REPO = None


def main(argv):
    global REPO
    REPO = (subprocess.run(['git', 'rev-parse', '--show-toplevel'], capture_output=True,
                           text=True).stdout.strip() or os.getcwd())
    os.chdir(REPO)
    if argv and argv[0] == '--selftest':
        return selftest()
    if not argv or argv[0] == '--check':
        code, bad = check()
        if bad:
            print('line-citations: cite a document by heading and code by symbol, never by line '
                  'number (§277, calef, 2026-10-11):', file=sys.stderr)
            for b in bad:
                print(f'  {b}', file=sys.stderr)
            print(f'\nQuoted tool output belongs in a fence, or carries `{MARKER}. Reason: ...`. '
                  f'See the header of {SELF}.', file=sys.stderr)
            return 1
        print(f'line-citations: none into a Markdown document; {code:,} into code (ceiling '
              f'{CODE_CEILING:,}, may only fall)')
        return 0
    if argv[0] == '--list':
        paths = argv[1:] or tracked()
        for p, hits in sorted(census([x for x in paths if in_scope(x)]).items()):
            for line, target, k in hits:
                print(f'{p}:{line}: {k} {target}')
        return 0
    if argv[0] == '--bank':
        now = bank()
        print(f'line-citations: code ceiling now {min(now, CODE_CEILING):,}')
        return 0
    print(__doc__.split('\n\n')[3], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
