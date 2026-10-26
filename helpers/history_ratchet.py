"""The history ratchet: phrases that only a document's own history uses, counted, never allowed to grow.

calef, 2026-10-13 (UTC):

    "We do not need the history in the text. That is what revision control is for."

A document states what is true and in force now, and how it got there goes in commit messages and
pull request bodies: §298 (a document states what is in force now), which is rule 5 of §213
(writing standards). This gate is rung 2 of the AGENTS.md ladder for that rule. It cannot see every
way to narrate history, so it counts the phrases that almost never mean anything else, and holds
them to a ratchet.

    python3 helpers/history_ratchet.py --check          # what script/lint runs
    python3 helpers/history_ratchet.py --list [PATH..]  # every counted hit, path:line: kind: text
    python3 helpers/history_ratchet.py --bank           # lower CEILING to the tree; never raises
    python3 helpers/history_ratchet.py --selftest       # the traps, as fixtures

Name: provisional. A shared python module under `helpers/`, which `script/names` puts out of its
own scope, so its provenance is this paragraph. calef names things; expect this to change.

**What counts**, in Markdown outside fenced code, inline code, HTML comments, frontmatter and `>`
quotation lines:

- `supersede` in any form, and `supersession`, unless a modal or `to` precedes it ("a later
  decision can supersede this" is about the future). An all-capitals `SUPERSEDED` is a status
  value, not prose, and is not counted.
- `used to` followed by a verb of saying or being: `used to say`, `used to be`, `used to read`.
- `was` or `were`, then within the sentence `until` and a milestone, a section, a pull request or
  a date: "The lint job was named `clippy` until 2026-10-08."
- `renamed from`, `was renamed`, `formerly`.
- `moved from` (or `moved here from`, `moved whole from`) naming a document: a link, a `.md`
  path, a milestone, a section, or "its section".
- A heading that names a history: `Timeline`, `Status history`, `BUGS history`, `How it got here`,
  `The history`, `Its history`.
- A file in an appendix directory whose name ends in `history`, `timeline` or `changelog`.

**Two halves, the shape of house_style.py and cap_abbreviation.py.** A count over `CEILING` fails,
which guards `main`, where there is no merge base. A branch is also held to its merge base: summed
over the files it changed, the count may not grow. That second half is the ratchet. Deleting a
history sentence leaves slack, and the next change may not spend it. `--bank` lowers the ceiling.

**Not counted, each for a stated reason:**

- A document whose subject is an event, where the dates and their order are the content: a
  correction of error under §210 (a correction of error), in `notes/coes/` and
  `notes/corrections/`, an audit report, and any
  file whose path carries a date (`measured-2026-08-16.md`, a bench session).
- A document carrying `<!-- history: exception. Reason: ... -->`, which says out loud that it is
  one, as the ladder requires of an exception.
- `vendor/`, `patches/` and `target/`: somebody else's text.

BUGS:
- Markdown only. Rust and script comments carry history too; §267 (a comment states the constraint
  as it is now) governs them, and its gate holds comment length rather than phrasing.
- Phrases only. "The gate first held bold as two counts" is history and matches nothing here.
  Review is the mechanism for the rest, and the count is a floor.
- About one counted line in fifteen is not history: "a property that used to hold" (a regression),
  a CI push "superseding" an older one, a decision's table naming the rename it rules. Rewording
  costs a sentence; an exception costs a marked comment.
"""

import os
import re
import subprocess
import sys

sys.dont_write_bytecode = True

SELF = 'helpers/history_ratchet.py'

# The tree's count when this gate was added. Never raise it by hand: new history is moved to the
# commit message, not admitted.
CEILING = 42

EXCLUDED_PREFIXES = ('vendor/', 'patches/', 'target/', 'notes/coes/', 'notes/corrections/',
                     'design/audit-reports/')
DATED = re.compile(r'\d{4}-\d\d-\d\d')
EXCEPTION = re.compile(r'<!--\s*history:\s*exception\b[^>]*\bReason:', re.S)


def in_scope(path):
    return (path.endswith('.md') and not path.startswith(EXCLUDED_PREFIXES)
            and not DATED.search(path))


# --- what counts --------------------------------------------------------------------------------

PHRASES = {
    'supersede': re.compile(r'(?i)\bsupersed(?:e|ed|es|ing)\b|\bsupersession\b'),
    'used to': re.compile(r'(?i)\bused to (?:be|say|read|live|hold|carry|mean|sit|name|call|count|'
                          r'list|state|claim)\b'),
    'was until': re.compile(r'(?i)\b(?:was|were)\b[^.]{0,80}?\buntil (?:milestone \d|§\d|#\d|'
                            r'\d{4}-\d\d-\d\d)'),
    'renamed': re.compile(r'(?i)\brenamed from\b|\bwas renamed\b|\bformerly\b'),
    'moved from': re.compile(r'(?i)\bmoved (?:whole |here )?from (?:its (?:section|BUGS)|\[|'
                             r'milestone \d|§\d|\S+\.md\b)'),
}
MODAL = re.compile(r'(?i)\b(?:could|would|can|may|might|will|to)\s+$')
HEADING = re.compile(r'^#{1,6} .*\b(?i:timeline|status history|bugs history|'
                     r'how (?:it|this|we) got here|the history|its history)\b')
HISTORY_FILE = re.compile(r'(?:^|-)(?:history|timeline|changelog)\.md$')
BLOCK = re.compile(r'(?:design/(?:roadmap|decisions)/)\d{4}-[^/]*\.md$')
CODE = re.compile(r'`[^`\n]*`')
COMMENT = re.compile(r'<!--.*?-->', re.S)


def line_hits(line):
    """[(kind, phrase)] in one line of prose."""
    if re.match(r'\s*>', line):
        return []
    if HEADING.match(line):
        return [('heading', line.strip())]
    text = CODE.sub('``', line)
    out = []
    for kind, pattern in PHRASES.items():
        for m in pattern.finditer(text):
            if m.group(0).isupper():
                continue
            if kind == 'supersede' and MODAL.search(text[:m.start()]):
                continue
            out.append((kind, m.group(0)))
    return out


def file_hits(text, path=''):
    """[(line, kind, phrase)] for a Markdown document."""
    if EXCEPTION.search(text):
        return []
    hits = []
    if path and HISTORY_FILE.search(os.path.basename(path)) and not BLOCK.search(path):
        hits.append((0, 'history file', os.path.basename(path)))
    # Comments go, keeping their newlines, so a line number still points at the line.
    text = COMMENT.sub(lambda m: '\n' * m.group(0).count('\n'), text)
    lines = text.split('\n')
    start = 0
    if lines and lines[0] == '---':
        for i in range(1, len(lines)):
            if lines[i] == '---':
                start = i + 1
                break
    fence = None
    for n in range(start, len(lines)):
        line = lines[n]
        mark = re.match(r'\s*(```|~~~)', line)
        if mark:
            fence = None if fence == mark.group(1) else (fence or mark.group(1))
            continue
        if fence:
            continue
        hits += [(n + 1, kind, phrase) for kind, phrase in line_hits(line)]
    return hits


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
        if t is not None:
            out[p] = file_hits(t, p)
    return out


def total(by_path):
    return sum(len(h) for h in by_path.values())


def merge_base():
    base = (git('merge-base', 'HEAD', 'origin/main') or '').strip()
    head = (git('rev-parse', 'HEAD') or '').strip()
    return base if base and base != head else None


def growth(base):
    """Net change, base to working tree, over the files this branch changed: (net, {path: d})."""
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
    where = {}
    for p in old_paths:
        if in_scope(p):
            where[p] = where.get(p, 0) - len(file_hits(git('show', f'{base}:{p}') or '', p))
    for p in new_paths:
        if in_scope(p) and os.path.isfile(p):
            where[p] = where.get(p, 0) + len(file_hits(read(p) or '', p))
    return sum(where.values()), where


# --- the commands -------------------------------------------------------------------------------

def check():
    bad = []
    now = total(census(tracked()))
    if now > CEILING:
        bad.append(f'{now:,} history phrases in the tree, over the ceiling of {CEILING:,} in '
                   f'{SELF}. If this branch adds none, main moved past it: rebase, and set the '
                   f'ceiling to the merged count in this change, saying why')
    base = merge_base()
    if base:
        net, where = growth(base)
        if net > 0:
            grew = sorted(p for p, d in where.items() if d > 0)
            bad.append(f'this branch adds {net} net, in {", ".join(grew)}')
    return now, bad


def bank():
    now = total(census(tracked()))
    text = open(SELF).read()
    text = re.sub(r'(?m)^(CEILING = )[\d_]+$', rf'\g<1>{min(now, CEILING):_}', text, count=1)
    with open(SELF, 'w') as f:
        f.write(text)
    return now


def selftest():
    cases = [
        # (Markdown, the kinds that must count)
        ('This supersedes the 2026-09-19 ruling.', ['supersede']),
        ('Section 105 is superseded. It was superseded on a second pass.', ['supersede', 'supersede']),
        ('This bullet used to say the row was missing.', ['used to']),
        ('It was called init until #1500.', ['was until']),
        ('The lint job was named `clippy` until 2026-10-08 (UTC).', ['was until']),
        ('`rank::INTERRUPT_CONTROLLER` (renamed from `rank::GIC`); nife, formerly cricker-os.',
         ['renamed', 'renamed']),
        ('An appendix of [x](../x.md), moved from its section "Y" on 2026-10-12.', ['moved from']),
        ('Moved whole from [`notes/a.md`](../a.md).', ['moved from']),
        ('## Timeline', ['heading']),
        ('### The status history', ['heading']),
        # What is not history.
        ('status: SUPERSEDED, and `superseded_by: 148`.', []),
        ('A later decision can supersede this, and one may supersede it.', []),
        ('> "It supersedes the old one," he said.', []),
        ('The tool is used to build the image. It was slow until warmed.', []),
        ('`PHYS_START` moved from 1 MiB to 32 MiB; the choice moved from the wait site.', []),
        ('# 657. A live history search at the prompt', []),
        ('```\nThis supersedes the 2026-09-19 ruling.\n```', []),
        ('<!-- This supersedes it. -->', []),
        ('---\nstatus: DECIDED\nnote: superseded\n---\nBody.', []),
        ('<!-- history: exception. Reason: a dated run. -->\nThis supersedes it.', []),
    ]
    failed = 0
    for src, want in cases:
        got = [k for _, k, _ in file_hits(src)]
        if got != want:
            failed += 1
            print(f'history selftest: {src!r} counted {got}, wanted {want}', file=sys.stderr)
    files = [
        ('notes/pipes/bugs-history.md', ['history file']),
        ('design/roadmap/0161-x86-64-kernel-port/history.md', ['history file']),
        ('design/roadmap/0679-the-prompt-suggests-from-history.md', []),
    ]
    for path, want in files:
        got = [k for _, k, _ in file_hits('Nothing here.', path)]
        if got != want:
            failed += 1
            print(f'history selftest: {path} counted {got}, wanted {want}', file=sys.stderr)
    scope = [('notes/coes/x.md', False), ('notes/merge-queue/measured-2026-08-16.md', False),
             ('notes/a.md', True), ('notes/a.rs', False)]
    for path, want in scope:
        if in_scope(path) != want:
            failed += 1
            print(f'history selftest: in_scope({path}) is {not want}, wanted {want}',
                  file=sys.stderr)
    lines = [n for n, _, _ in file_hits('A\n\n<!-- a\nb -->\nIt was renamed.')]
    if lines != [5]:
        failed += 1
        print(f'history selftest: line numbers {lines}, wanted [5]', file=sys.stderr)
    if failed:
        return 1
    print(f'history selftest: {len(cases) + len(files) + len(scope) + 1} fixtures')
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
        now, bad = check()
        if bad:
            print('history: the tree gained text about its own history (calef, 2026-10-13: "We do '
                  'not need the history in the text. That is what revision control is for."):',
                  file=sys.stderr)
            for b in bad:
                print(f'  {b}', file=sys.stderr)
            print(f'\nState what is true now, and put how it got there in the commit message '
                  f'(`python3 {SELF} --list PATH` shows each hit). See the header of {SELF}.', file=sys.stderr)
            return 1
        print(f'history: {now:,} history phrases in Markdown (ceiling held; paid down by lanes)')
        return 0
    if argv[0] == '--list':
        paths = argv[1:] or tracked()
        for p, hits in sorted(census([x for x in paths if in_scope(x)]).items()):
            for line, kind, phrase in hits:
                print(f'{p}:{line}: {kind}: {phrase}')
        return 0
    if argv[0] == '--bank':
        now = bank()
        print(f'history: ceiling now {min(now, CEILING):,}')
        return 0
    print(__doc__.split('\n\n')[2], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
