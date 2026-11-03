"""The layout of a row baseline that many pull requests shrink at once, so git merges their removals.

Two ratchets keep a committed list of rows that only shrinks: `helpers/module_block_ratchet.py`
(`design/module-block-baseline.txt`) and `helpers/name_checks.py` (`design/naming-rule-baseline.txt`).
Naming pull requests each clear a run of rows, and rows sort by path, so two pull requests that clear
adjacent rows touched adjacent lines. Git reports adjacent changed lines as a conflict even
though the two changes never overlap, and a merge queue that meets one removes every entry behind it.
Measured on the 28 open naming pull requests of 2026-11-03: 243 of the 351 pairs that edited the
module-block baseline cleared rows on the same or adjacent lines.

    python3 helpers/baseline_layout.py --selftest      # the rules, as fixtures, and git's own merge

**The layout.**

- A row is a line that is neither blank nor a `#` comment. Rows are sorted, byte for byte as Python
  sorts the strings, and a row may repeat (the list is a multiset).
- A blank line follows every row. Two rows are never on adjacent lines, so two changes that clear
  different rows never touch adjacent lines, and git merges them.
- A change that clears a row deletes that row's line and leaves the blank line after it. Blank lines
  are slots: nothing removes them, `--bank` included, and a run of them is fine. A change that adds a
  row (a rename re-keying its rows) writes the row and a blank line after it, at its sorted place.
- Nothing is derived into a baseline: no total, no count. The gate counts the rows it reads.

**What the gate checks.** `problems(text, label)` fails a list whose rows are not sorted or whose
rows touch. The ratchets run it beside their own rules, so a hand edit that deletes a blank line, or
a `sort` of the file that closes the gaps, fails with the line to fix.

Name: provisional, minted 2026-11-03 (UTC) by lane/names-note-and-baselines-rework. A shared helper
under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the record.
`baseline_layout` for what it holds, a layout, as `moved_rows` holds moved rows. calef has not named
it.
"""

import collections
import os
import subprocess
import sys
import tempfile


def is_row(line):
    return bool(line.strip()) and not line.startswith('#')


def rows_of(text):
    """The rows of a baseline as a multiset; blank lines and comments are not rows."""
    return collections.Counter(line.strip() for line in text.splitlines() if is_row(line))


def problems(text, label):
    """Every way `text` departs from the layout, as messages that name the line to fix."""
    bad = []
    previous = None
    touching = 0
    first_touch = None
    unsorted = None
    for number, line in enumerate(text.splitlines(), 1):
        if not is_row(line):
            continue
        row = line.strip()
        if previous is not None:
            if row < previous[1] and unsorted is None:
                unsorted = (number, row, previous)
            if previous[0] == number - 1:
                touching += 1
                first_touch = first_touch or number
        previous = (number, row)
    if unsorted:
        number, row, (before, earlier) = unsorted
        bad.append(f'{label}: line {number} ({row}) sorts before line {before} ({earlier}). Rows are '
                   f'sorted; move the row, and do not re-sort the file, which closes the blank lines')
    if touching:
        bad.append(f'{label}: {touching} rows touch the row before them (first at line {first_touch}). '
                   f'A blank line follows every row, so two changes that clear different rows never '
                   f'touch adjacent lines; put the blank line back (helpers/baseline_layout.py)')
    return bad


def rewrite(old_text, header, rows):
    """`old_text` with `header` for its leading comments and `rows` for its rows, layout kept.

    A row `old_text` holds and `rows` no longer does has its line deleted and its blank line left.
    A row `rows` holds and `old_text` does not is written at its sorted place with a blank line
    after it. The same multiset in the same order comes back byte for byte.
    """
    left = collections.Counter(rows)
    body = old_text.splitlines()
    while body and body[0].startswith('#'):
        body.pop(0)
    kept = []
    for line in body:
        if is_row(line):
            row = line.strip()
            if left[row] <= 0:
                continue
            left[row] -= 1
        kept.append(line)
    # What is left over is new. `kept` is sorted, so each new row goes before the first row above it.
    for row in sorted(left.elements()):
        at = next((i for i, line in enumerate(kept) if is_row(line) and line.strip() > row), len(kept))
        kept[at:at] = [row, '']
    return header + ''.join(f'{line}\n' for line in kept)


def spaced(header, rows):
    """A fresh baseline: every row sorted, a blank line after each."""
    return header + ''.join(f'{row}\n\n' for row in sorted(rows.elements()))


# --- the selftest ---------------------------------------------------------------------------------

def selftest():
    failed = []

    def case(name, ok):
        if not ok:
            failed.append(name)

    header = '# header\n'
    good = '# header\na\n\nb\n\nc\n\n'
    case('a spaced sorted list passes', not problems(good, 'f'))
    case('two rows on adjacent lines fail', bool(problems('# h\na\nb\n\n', 'f')))
    case('rows out of order fail', bool(problems('# h\nb\n\na\n\n', 'f')))
    case('a run of blank lines passes', not problems('# h\na\n\n\n\nb\n\n', 'f'))
    case('a repeated row passes', not problems('# h\na\n\na\n\n', 'f'))
    case('the message names the line', 'line 3' in ' '.join(problems('# h\na\nb\n\n', 'f')))

    rows = collections.Counter
    case('clearing a row leaves its blank line',
         rewrite(good, header, rows(['a', 'c'])) == '# header\na\n\n\nc\n\n')
    case('the same rows come back byte for byte', rewrite(good, header, rows(['a', 'b', 'c'])) == good)
    case('a new row lands at its sorted place with a blank line',
         rewrite(good, header, rows(['a', 'b', 'bb', 'c'])) == '# header\na\n\nb\n\nbb\n\nc\n\n')
    case('a new row below every row goes last',
         rewrite(good, header, rows(['a', 'b', 'c', 'z'])) == good + 'z\n\n')
    case('a cleared list still passes its own gate',
         not problems(rewrite(good, header, rows(['a'])), 'f'))
    case('a repeated row is cleared one at a time',
         rewrite('# h\na\n\na\n\n', header, rows(['a'])) == '# header\na\n\n\n')

    # The point of the layout, against git's own merge: two changes clear adjacent rows.
    many = ''.join(f'row{n:02}\n\n' for n in range(10))
    mine = rewrite(many, header, rows(f'row{n:02}' for n in range(10) if n != 3))
    theirs = rewrite(many, header, rows(f'row{n:02}' for n in range(10) if n != 4))
    flat = ''.join(f'row{n:02}\n' for n in range(10))
    flat_mine = ''.join(f'row{n:02}\n' for n in range(10) if n != 3)
    flat_theirs = ''.join(f'row{n:02}\n' for n in range(10) if n != 4)
    case('git merges adjacent clears in the spaced layout', merges(many, mine, theirs))
    case('git conflicts on adjacent clears in the flat layout (the old layout)',
         not merges(flat, flat_mine, flat_theirs))
    case('git merges a whole run cleared beside a adjacent row',
         merges(many, rewrite(many, header, rows(f'row{n:02}' for n in range(10) if n not in (1, 2, 3))),
                rewrite(many, header, rows(f'row{n:02}' for n in range(10) if n != 4))))
    for name in failed:
        print(f'baseline_layout selftest FAILED: {name}', file=sys.stderr)
    if not failed:
        print('baseline_layout selftest: all cases pass')
    return 1 if failed else 0


def merges(base, a, b):
    """True when `git merge-file` merges `a` and `b`, two edits of `base`, without a conflict."""
    with tempfile.TemporaryDirectory() as tmp:
        paths = []
        for name, text in (('a', a), ('base', base), ('b', b)):
            path = os.path.join(tmp, name)
            with open(path, 'w') as f:
                f.write(text)
            paths.append(path)
        return subprocess.run(['git', 'merge-file', '-p'] + paths, capture_output=True).returncode == 0


if __name__ == '__main__':
    if sys.argv[1:] == ['--selftest']:
        sys.exit(selftest())
    print(__doc__.split('\n\n')[2], file=sys.stderr)
    sys.exit(2)
