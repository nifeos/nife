"""The file-length check: §266 (a Rust source file stays under 2,000 lines).

Milestone 841 (a ratchet on Rust file length, and its dashboard row). calef ruled §266 on
2026-10-08 (UTC): the measure and the 2,000-line ceiling. The check began as a ratchet with a
baseline of files already over; the baseline emptied and was removed, so the ceiling is now
absolute, with no list of exceptions.

    python3 helpers/file_length_ratchet.py --check          # what script/lint runs
    python3 helpers/file_length_ratchet.py --selftest       # the rules, as fixtures

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

**The rule.** A file over the ceiling fails. Exceptions are not provided for. A file that must stay
over 2,000 lines needs §266 amended, by an architect.

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

REMEDY = ('The remedy is to split the file along a seam; it is never to strip comments, join '
          'lines, or move a test module into a file nobody reads (§266 section 1, the measure).')


def fmt(n):
    return f'{n:,}'


# --- git ------------------------------------------------------------------------------------------

def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


def tracked_rust():
    """Every `.rs` path the rules reach: tracked outside `vendor/`, untracked ones too."""
    files = git('ls-files', '-z', '--cached', '--others', '--exclude-standard') or ''
    return {f for f in files.split('\0')
            if f and f.endswith('.rs') and not f.startswith('vendor/') and os.path.exists(f)}


def line_counts(paths, read):
    """{path: physical lines}, `wc -l` semantics: the count of newline characters."""
    return {p: read(p).count('\n') for p in paths}


# --- the rule, pure so the selftest can drive it --------------------------------------------------

def problems(counts):
    """Every file over the ceiling, and a judgment of nothing at all."""
    if not counts:
        return ['the check judged no Rust file at all. A selector that matches nothing passes by '
                'checking nothing (nifeos/record:design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md)']
    return [f'{path}: {fmt(n)} lines against the {fmt(CEILING)}-line ceiling. {REMEDY}'
            for path, n in sorted(counts.items()) if n > CEILING]


# --- the verbs ------------------------------------------------------------------------------------

def run_check():
    counts = line_counts(tracked_rust(),
                         lambda p: open(p, encoding='utf-8', errors='replace').read())
    bad = problems(counts)
    if bad:
        print('file-length check: the tree breaks §266:', file=sys.stderr)
        for b in bad:
            print(f'  {b}', file=sys.stderr)
        print('\nSplit the file along a seam, or amend §266 with an architect. See milestone '
              '841\'s block, nifeos/record:design/roadmap/'
              '0841-a-ratchet-on-rust-file-length-and-its-dashboard-row.md.', file=sys.stderr)
        return 1
    print(f'file-length check: {len(counts)} Rust files, none over the {fmt(CEILING)}-line ceiling')
    return 0


def selftest():
    """The rule as fixtures, and the traps around it."""
    def tree(**files):
        return {f'x/{name}.rs': n for name, n in files.items()}

    cases = []
    cases.append(('a 2,001-line file fails', bool(problems(tree(new=2001, small=100)))))
    cases.append(('a 2,000-line file passes', not problems(tree(new=2000, small=100))))
    # `wc -l` semantics, and a selector that matches nothing.
    cases.append(("the count is wc -l's: newline characters, a short last line uncounted",
                  ('a\n' * 2001).count('\n') == 2001 and 'a\nb'.count('\n') == 1))
    cases.append(('a selector that matches nothing fails loudly', bool(problems({}))))
    # The message names the file, its size and the ceiling, and carries §266's remedy.
    msg = problems(tree(new=2001))[0]
    cases.append(('the failure message names the file, size, ceiling and remedy',
                  all(s in msg for s in ('new.rs', '2,001', '2,000', 'split the file along a seam',
                                         'never to strip comments'))))

    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f'file-length check selftest: {name}', file=sys.stderr)
    if not failed:
        print(f'file-length check selftest: {len(cases)} cases pass')
    return 1 if failed else 0


def main(argv):
    root = git('rev-parse', '--show-toplevel')
    if root:
        os.chdir(root.strip())
    if argv and argv[0] == '--selftest':
        return selftest()
    if not argv or argv[0] == '--check':
        return run_check()
    print(__doc__.split('\n\n')[1], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
