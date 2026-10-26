"""The file-length ratchet: §266 (a Rust source file stays under 2,000 lines), held to a baseline
that only falls.

Milestone 841 (a ratchet on Rust file length, and its dashboard row). calef ruled §266 on
2026-10-08 (UTC): the measure, the 2,000-line ceiling, the goals, and option (b) for the gate's
matching rule, a list entry that is a ceiling and only falls. This is that gate, and it copies
`helpers/prose_ratchet.py`'s pattern, §212 (a prose budget)'s ratchet, on purpose: a committed
list that only shrinks, a comparison against the merge base, and a selftest that runs before the
check.

    python3 helpers/file_length_ratchet.py --check          # what script/lint runs
    python3 helpers/file_length_ratchet.py --bank           # lower the baseline to the tree; never raises
    python3 helpers/file_length_ratchet.py --selftest       # the rules, as fixtures
    python3 helpers/file_length_ratchet.py --init           # write the baseline from scratch (milestone 841 only)

Name: provisional, minted by milestone 841's lane on 2026-10-08. A shared python module under
`helpers/`, which `script/names` puts out of its own scope, so its provenance is this paragraph
rather than a `Name:` block. calef names things; expect this to change. Refused
`file_size_ratchet`: §266 names a count of lines.

**The measure.** A file's size is its physical line count, `wc -l`: comments, blank lines and
inline `mod tests` counted, exactly §266 section 1. A reader or a model opening a file loads every
line of it, and in this tree the comments are 41% of the non-blank Rust, so a measure that skipped
them would hide most of what the reader pays. Every tracked `.rs` file outside `vendor/`, plus
untracked ones, so a lane's new file is judged before its first commit. A file of 2,000 lines is
allowed; 2,001 is over.

**The rules** (§266 section 3, option (b), ruled by calef over exact match):

- A file not on the list fails at 2,001 lines or more. A new file is not on the list.
- A listed file fails if it grew against the merge base, and if it is larger than its entry. The
  growth check is unconditional: an entry can sit above its file's real size because a shrink was
  never banked, and that slack is not room to grow back into.
- A list entry is a ceiling that `--bank` lowers and nothing raises. An entry fails when its file
  is gone, renamed, at or under 2,000 lines, or larger than the entry, so dead rows cannot
  accumulate.
- Against the merge base, a row may not be added and an entry may not rise. A rename moves its row
  in the same change, at the same number.
- Exceptions are not provided for. A file that must stay over 2,000 lines needs §266 amended, by
  an architect.

**Why a ceiling and not exact match** (§266 section 3, the measurement that argued it): 164 of the
664 merges in the fortnight to 2026-10-08 touched a listed file, so under exact match two
shrinking changes to one file in one merge group each write a different number on the same row and
the group fails even when the Rust merged cleanly. Under option (b) both pass; the cost is that an
entry can sit above its file's real size until someone banks it. Nothing but this gate reads
the list, so that staleness reaches no number anyone quotes.

**The failure message** names the file, its size and the ceiling, and says the remedy: §266
section 1 names the incentive a line ceiling creates ("a file at 2,050 lines can pass by deleting
50 lines of comments"), which would make the tree worse to buy a number. The remedy is to split
the file along a seam, and it is never to strip comments.
"""

import os
import subprocess
import sys

# --- the ceiling and the baseline -----------------------------------------------------------------

CEILING = 2000
BASELINE = 'design/file-length-baseline.tsv'
COLUMN = 'lines'

HEADER = """\
# The file-length ratchet's baseline: milestone 841 (a ratchet on Rust file length, and its
# dashboard row), for §266 (a Rust source file stays under 2,000 lines).
# One row per Rust file over the 2,000-line ceiling when it was written: path, then physical
# lines (wc -l, comments and blank lines counted, which is §266 section 1's measure).
# A number is a ceiling that may only go DOWN: lower it with
# `python3 helpers/file_length_ratchet.py --bank`, which never raises and never adds. A row may be
# removed, in the same change that takes its file to 2,000 lines or below, or deletes it; a row
# may not be added and a number may not rise (script/lint compares against the merge base). A
# rename moves its row in the same change, at the same number. A file that must exceed its row
# needs §266 amended by an architect, not an edit here.
"""

REMEDY = ('The remedy is to split the file along a seam; it is never to strip comments, join '
          'lines, or move a test module into a file nobody reads (§266 section 1, the measure).')


def fmt(n):
    return f'{n:,}'


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


def tracked_rust():
    """Every `.rs` path the rules reach: tracked outside `vendor/`, untracked ones too."""
    files = git('ls-files', '-z', '--cached', '--others', '--exclude-standard') or ''
    return {f for f in files.split('\0')
            if f and f.endswith('.rs') and not f.startswith('vendor/') and os.path.exists(f)}


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


def line_counts(paths, read):
    """{path: physical lines}, `wc -l` semantics: the count of newline characters."""
    return {p: read(p).count('\n') for p in paths}


# --- the rules, pure so the selftest can drive them ------------------------------------------------

def problems(counts, rows, base_counts=None, base_rows=None, renames=()):
    """Every failure in one tree. See the header for the rules.

    `counts` is the tree's {path: lines}; `rows` the baseline's {path: ceiling}; `base_counts` and
    `base_rows` the same at the merge base, where there was one; `renames` git's (old, new) `.rs`
    pairs since the base. The baseline comparison is skipped when `base_rows` is None, which is
    the base holding no baseline: the change that first lands one.
    """
    if not counts:
        return ['the check judged no Rust file at all. A selector that matches nothing passes by '
                'checking nothing (design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md)']
    was_called = {new: old for old, new in renames}
    bad = []

    # The tree: an unlisted file over the ceiling, and every way a listed one may not stand.
    for path in sorted(counts):
        n = counts[path]
        if n > CEILING and path not in rows:
            bad.append(f'{path}: {fmt(n)} lines against the {fmt(CEILING)}-line ceiling, and it '
                       f'is not on the list. A new file meets the ceiling outright. {REMEDY}')
    for path in sorted(rows):
        n = counts.get(path)
        if n is None:
            bad.append(f'{BASELINE}: {path} is not in the tree. Move its row with the rename, at '
                       f'the same number, or remove it; the list cannot go stale')
            continue
        ceiling = rows[path]
        if n <= CEILING:
            bad.append(f'{BASELINE}: {path} is at {fmt(n)} lines, at or under the '
                       f'{fmt(CEILING)}-line ceiling. Remove its row; the list cannot go stale')
        elif n > ceiling:
            bad.append(f'{path}: {fmt(n)} lines, over its baseline ceiling of {fmt(ceiling)}. '
                       f'A listed file may not exceed its entry. {REMEDY}')
        if base_counts is not None:
            was = base_counts.get(was_called.get(path, path))
            if was is not None and n > was:
                bad.append(f'{path}: grew from {fmt(was)} to {fmt(n)} lines against the merge '
                           f'base. A listed file may only shrink. {REMEDY}')

    # The list against its merge-base copy: no additions, no raises, renames at the same number.
    if base_rows is not None:
        for path in sorted(rows):
            prior = base_rows.get(path)
            if prior is None:
                prior = base_rows.get(was_called.get(path, ''))
            if prior is None:
                bad.append(f'{BASELINE}: {path} was added. The list only shrinks; a new file '
                           f'meets the ceiling outright')
            elif rows[path] > prior:
                bad.append(f'{BASELINE}: {path} raised its ceiling from {fmt(prior)} to '
                           f'{fmt(rows[path])}. An entry only goes down (§266 section 3, '
                           f'option (b))')
    return bad


def banked(rows, counts):
    """The baseline after banking: entries lowered to the tree, never raised, never added.

    A row whose file is gone or at or under the ceiling drops out, which is the rule `check`
    already enforces on the tree: banking is how a shrink or a split leaves the list in the same
    change.
    """
    out = {}
    for path, ceiling in rows.items():
        n = counts.get(path)
        if n is None or n <= CEILING:
            continue
        out[path] = min(n, ceiling)
    return out


# --- the verbs ------------------------------------------------------------------------------------

def gather():
    """(counts, rows, base_counts, base_rows, renames) for the tree and its merge base."""
    counts = line_counts(tracked_rust(),
                         lambda p: open(p, encoding='utf-8', errors='replace').read())
    rows = read_baseline(open(BASELINE).read())
    base = merge_base()
    base_counts, base_rows, renames = None, None, ()
    if base:
        renames = renamed_rust(base)
        wanted = set(rows) | {old for old, _new in renames}
        base_counts = {}
        for path in wanted:
            text = git('show', f'{base}:{path}')
            if text is not None:
                base_counts[path] = text.count('\n')
        old_text = git('show', f'{base}:{BASELINE}')
        if old_text is not None:
            base_rows = read_baseline(old_text)
    return counts, rows, base_counts, base_rows, renames


def run_check():
    if not os.path.exists(BASELINE):
        print(f'file-length ratchet: {BASELINE} is missing. The gate and its list are one change; '
              f'deleting one deletes the gate', file=sys.stderr)
        return 1
    counts, rows, base_counts, base_rows, renames = gather()
    bad = problems(counts, rows, base_counts, base_rows, renames)
    if bad:
        print('file-length ratchet: the tree breaks §266 against its baseline:', file=sys.stderr)
        for b in bad:
            print(f'  {b}', file=sys.stderr)
        print('\nSplit the file along a seam, bank the shrink with --bank, or amend §266 with an '
              'architect. See milestone 841\'s block, design/roadmap/'
              '0841-a-ratchet-on-rust-file-length-and-its-dashboard-row.md.', file=sys.stderr)
        return 1
    over = sum(1 for n in counts.values() if n > CEILING)
    print(f'file-length ratchet: {len(counts)} Rust files, {over} over the {fmt(CEILING)}-line '
          f'ceiling and held to {len(rows)} baseline rows')
    return 0


def bank(initialize=False):
    if initialize and os.path.exists(BASELINE):
        print(f'file-length ratchet: {BASELINE} already exists. --init wrote it once, in '
              f'milestone 841; it is refused now, and --bank is the verb that never raises',
              file=sys.stderr)
        return 2
    if not initialize and not os.path.exists(BASELINE):
        print(f'file-length ratchet: {BASELINE} does not exist; there is nothing to bank',
              file=sys.stderr)
        return 2
    counts = line_counts(tracked_rust(),
                         lambda p: open(p, encoding='utf-8', errors='replace').read())
    if initialize:
        rows = {p: n for p, n in counts.items() if n > CEILING}
    else:
        rows = banked(read_baseline(open(BASELINE).read()), counts)
    write_baseline(rows)
    print(f'file-length ratchet: {len(rows)} rows written to {BASELINE}')
    return 0


def selftest():
    """The block's `Done when` list as fixtures, and the traps around it."""
    RENAME = [('x/old.rs', 'x/renamed.rs')]

    def tree(**files):
        return {f'x/{name}.rs': n for name, n in files.items()}

    def rows(**files):
        return {f'x/{name}.rs': n for name, n in files.items()}

    cases = []

    def fails(name, bad):
        cases.append((name, bool(bad)))

    def passes(name, bad):
        cases.append((name, not bad))

    # Done-when 1: a new 2,001-line file fails and a 2,000-line one passes.
    fails('a new 2,001-line file fails', problems(tree(new=2001, small=100), {}))
    passes('a 2,000-line file passes', problems(tree(new=2000, small=100), {}))
    # Done-when 2: one line added to a listed file fails, grown against the merge base, both
    # within its entry and over it.
    base = tree(listed=2050)
    row = rows(listed=2100)
    fails('a listed file grown against the merge base fails',
          problems(tree(listed=2051), row, base, row))
    fails('a listed file over its entry fails',
          problems(tree(listed=2101), row, base, row))
    passes('a listed file unchanged passes', problems(base, row, base, row))
    passes('a listed file shrunk passes', problems(tree(listed=2020), row, base, row))
    # Done-when 3: an entry left behind for a file that dropped to 2,000 lines, or was deleted.
    fails('an entry for a file at 2,000 lines fails', problems(tree(listed=2000), row, base, row))
    fails('an entry for a deleted file fails', problems(tree(other=2100), row, {}, row))
    passes('a shrunk file passes once its row is gone',
           problems(tree(listed=1999), {}, base, row))
    # The list against its merge-base copy: rows are never added, entries never rise, and a
    # rename moves its row at the same number.
    fails('a row added against the merge base fails',
          problems(tree(listed=2100, fresh=2100), rows(listed=2100, fresh=2100),
                   tree(listed=2100), rows(listed=2100)))
    fails('an entry raised against the merge base fails',
          problems(tree(listed=2100), rows(listed=2200), tree(listed=2100), rows(listed=2100)))
    passes('a rename that moves its row at the same number passes',
           problems(tree(renamed=2100), rows(renamed=2100), tree(old=2100), rows(old=2100),
                    RENAME))
    fails('a rename that raises its number fails',
          problems(tree(renamed=2100), rows(renamed=2200), tree(old=2100), rows(old=2100),
                   RENAME))
    fails('a rename that leaves its row behind fails',
          problems(tree(renamed=2100), rows(old=2100), tree(old=2100), rows(old=2100), ()))
    # `wc -l` semantics, and a selector that matches nothing.
    if ('a\n' * 2001).count('\n') != 2001 or 'a\nb'.count('\n') != 1:
        cases.append(('the count is wc -l\'s: newline characters, a short last line uncounted',
                      False))
    fails('a selector that matches nothing fails loudly', problems({}, {}))
    # Banking lowers to the tree, never raises, never adds, and drops dead rows.
    if banked(rows(a=5000, b=3000, c=2100), tree(a=4000, b=3050, c=2000)) != rows(a=4000, b=3000):
        cases.append(('bank lowers an entry to the tree and drops a dead row', False))
    if banked(rows(a=4000), tree(a=4500)) != rows(a=4000):
        cases.append(('bank never raises an entry', False))
    if banked(rows(a=5000), tree(a=5000, fresh=2100)) != rows(a=5000):
        cases.append(('bank never adds a row', False))
    # The message names the file, its size and the ceiling, and carries §266's remedy.
    msg = problems(tree(new=2001), {})[0]
    if not all(s in msg for s in ('new.rs', '2,001', '2,000', 'split the file along a seam',
                                  'never to strip comments')):
        cases.append(('the failure message names the file, size, ceiling and remedy', False))

    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f'file-length ratchet selftest: {name}', file=sys.stderr)
    if not failed:
        print(f'file-length ratchet selftest: {len(cases)} cases pass')
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
    # The option keeps its command-line abbreviation, `--init`; the code behind it spells `initialize`.
    # The difference is deliberate (design/naming/precedents-set-by-example.md, "The command line
    # abbreviates and the code behind it does not").
    if argv[0] in ('--bank', '--init'):
        return bank(initialize=argv[0] == '--init')
    print(__doc__.split('\n\n')[1], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
