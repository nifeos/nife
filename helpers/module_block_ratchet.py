"""The module-block ratchet: every module and document carries a `Name:` block, held to a list that only shrinks.

A module is a file (`helpers/name_census.py`'s `is_module_file`): every `.rs` file under a `src/`
directory outside vendored code, whether or not anyone wrote its block. So is a document directly
under `design/`, `notes/` or `briefs/` (`is_top_document`), whose block is its own entry in that
directory's README. That is what lets `script/names` and `script/metrics` count every one, and it
means a name with no block is visible, so it can be held to something. This holds it, with
`helpers/file_length_ratchet.py`'s pattern on purpose: a committed list that only shrinks, a
comparison against the merge base, and a selftest that runs before the check.

    python3 helpers/module_block_ratchet.py --check      # what script/lint runs
    python3 helpers/module_block_ratchet.py --bank       # drop rows whose module gained a block or went
    python3 helpers/module_block_ratchet.py --selftest   # the rules, as fixtures

Name: provisional, minted 2026-10-18 (UTC) by lane maintainer/names-count-every-module. A shared
helper under `helpers/`, which `script/names` puts out of its own scope, so this paragraph is the
record. `module_block` for what it holds each module to, `ratchet` for the family beside
`file_length_ratchet`. calef has not named it.

**The rules.**

- A module or document with no block that is not on the list fails. A new file is not on the
  list, so it arrives with its block: `//! Name: provisional, minted <date> (UTC) by <lane>.` at
  the top of a module, or a README entry for a document, is enough.
- A listed file that now carries a block, or is gone, fails until its row is removed, so dead
  rows cannot accumulate. `--bank` removes them.
- Against the merge base a row may not be added. A rename moves its row in the same change: a file
  git calls renamed may take its old row's place, so moving a file does not oblige a block.
- A selection that matches no module fails, because a gate that judges nothing passes.

**What it does not hold.** Items: an item is counted only where it carries a block, until "item"
has a definition a tree can be enumerated by (`helpers/name_census.py` marks that exception). And
it checks presence, never status: a `provisional` block passes, as `script/names --check` does.
"""

import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import name_census  # noqa: E402

BASELINE = 'design/module-block-baseline.txt'

HEADER = """\
# The module-block ratchet's baseline: helpers/module_block_ratchet.py, whose header is the manual.
# One path per line: a module (every .rs file under a src/ directory) or a document directly under
# design/, notes/ or briefs/ that carried no `Name:` block when this list was written. The list only shrinks. Remove a row in the change that writes
# its block or deletes its file (`python3 helpers/module_block_ratchet.py --bank` does it); a row
# may not be added (script/lint compares against the merge base), and a rename moves its row.
"""

def remedy(path):
    if path.endswith('.md'):
        return (f'give it an entry in {os.path.dirname(path)}/README.md, a paragraph '
                f'`` `{os.path.basename(path)}`: Name: provisional, minted <date> (UTC) by <lane>. ``')
    return ('write its block, `//! Name: provisional, minted <date> (UTC) by <lane>.`, at the top '
            'of the file (design/naming.md)')


# --- the baseline and git -------------------------------------------------------------------------

def read_baseline(text):
    return {line.strip() for line in text.splitlines()
            if line.strip() and not line.startswith('#')}


def write_baseline(rows):
    with open(BASELINE, 'w') as f:
        f.write(HEADER + ''.join(f'{path}\n' for path in sorted(rows)))


def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


def merge_base():
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


def tree_modules():
    """`(modules, unblocked)` for the working tree, untracked files included, out of the census."""
    listed = git('ls-files', '-z', '--cached', '--others', '--exclude-standard') or ''
    paths = [p for p in listed.split('\0') if p and os.path.isfile(p)]

    def read(rel):
        try:
            return open(rel, encoding='utf-8', errors='replace').read()
        except OSError:
            return None

    _records, _problems, _missing, unblocked = name_census.census(paths, read)
    judged = {p for p in paths if name_census.is_module_file(p) or name_census.is_top_document(p)}
    return judged, set(unblocked)


# --- the rules, pure so the selftest can drive them ------------------------------------------------

def problems(modules, unblocked, rows, base_rows=None, renames=()):
    """Every failure in one tree. `base_rows` is None where the merge base holds no baseline."""
    if not modules:
        return ['the check judged no file at all. A selector that matches nothing passes by '
                'checking nothing (design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md)']
    bad = []
    for path in sorted(unblocked - rows):
        bad.append(f'{path}: no `Name:` block, and it is not on the list. A new file carries its '
                   f'block; {remedy(path)}')
    for path in sorted(rows - unblocked):
        why = 'carries a block now' if path in modules else 'is not a module or document here'
        bad.append(f'{BASELINE}: {path} {why}. Remove its row (--bank); the list cannot go stale')
    if base_rows is not None:
        was_called = {new: old for old, new in renames}
        for path in sorted(rows - base_rows):
            if was_called.get(path) not in base_rows:
                bad.append(f'{BASELINE}: {path} was added. The list only shrinks; {remedy(path)}')
    return bad


# --- the verbs ------------------------------------------------------------------------------------

def run_check():
    if not os.path.exists(BASELINE):
        print(f'module-block ratchet: {BASELINE} is missing. The gate and its list are one change; '
              f'deleting one deletes the gate', file=sys.stderr)
        return 1
    modules, unblocked = tree_modules()
    rows = read_baseline(open(BASELINE).read())
    base, base_rows, renames = merge_base(), None, ()
    if base:
        renames = renamed_rust(base)
        old = git('show', f'{base}:{BASELINE}')
        if old is not None:
            base_rows = read_baseline(old)
    bad = problems(modules, unblocked, rows, base_rows, renames)
    if bad:
        print('module-block ratchet: the tree breaks its baseline:', file=sys.stderr)
        for b in bad:
            print(f'  {b}', file=sys.stderr)
        return 1
    print(f'module-block ratchet: {len(modules)} modules and documents, {len(unblocked)} with no block, held to '
          f'{len(rows)} baseline rows')
    return 0


def bank():
    if not os.path.exists(BASELINE):
        print(f'module-block ratchet: {BASELINE} does not exist; there is nothing to bank',
              file=sys.stderr)
        return 2
    _modules, unblocked = tree_modules()
    rows = read_baseline(open(BASELINE).read()) & unblocked
    write_baseline(rows)
    print(f'module-block ratchet: {len(rows)} rows written to {BASELINE}')
    return 0


def selftest():
    def m(*names):
        return {f'x/src/{n}.rs' for n in names}

    cases = [
        ('a new module with no block fails', bool(problems(m('a', 'new'), m('new'), set()))),
        ('a new module with a block passes', not problems(m('a', 'new'), set(), set())),
        ('a listed module with no block passes', not problems(m('a'), m('a'), m('a'), m('a'))),
        ('a listed module that gained a block fails', bool(problems(m('a'), set(), m('a')))),
        ('a row for a deleted module fails', bool(problems(m('b'), set(), m('a')))),
        ('a row added against the merge base fails',
         bool(problems(m('a', 'b'), m('a', 'b'), m('a', 'b'), m('a')))),
        ('a rename that moves its row passes',
         not problems(m('new'), m('new'), m('new'), m('old'), [('x/src/old.rs', 'x/src/new.rs')])),
        ('a rename that leaves its row behind fails',
         bool(problems(m('new'), m('new'), m('old'), m('old'), [('x/src/old.rs', 'x/src/new.rs')]))),
        ('a selector that matches nothing fails loudly', bool(problems(set(), set(), set()))),
        ('a file under src/ is a module', name_census.is_module_file('crates/a/src/b/c.rs')),
        ('a file under tests/ is not', not name_census.is_module_file('crates/a/tests/c.rs')),
        ('build.rs is not', not name_census.is_module_file('kernel/build.rs')),
        ('vendored code is not', not name_census.is_module_file('vendor/a/src/b.rs')),
        ('code quoted in a note is not', not name_census.is_module_file('notes/a/src/b.rs')),
        ('a top-level note is a document', name_census.is_top_document('notes/a.md')),
        ('a numbered record is not', not name_census.is_top_document('design/0001-a.md')),
        ('a README is not', not name_census.is_top_document('notes/README.md')),
        ('a nested document is not (its directory gates it)',
         not name_census.is_top_document('notes/a/b.md')),
    ]
    msg = problems(m('a', 'new'), m('new'), set())[0]
    cases.append(('the failure names the file and the block to write',
                  'x/src/new.rs' in msg and '//! Name: provisional' in msg))
    failed = [name for name, ok in cases if not ok]
    for name in failed:
        print(f'module-block ratchet selftest: {name}', file=sys.stderr)
    if not failed:
        print(f'module-block ratchet selftest: {len(cases)} cases pass')
    return 1 if failed else 0


def main(argv):
    root = git('rev-parse', '--show-toplevel')
    if root:
        os.chdir(root.strip())
    if argv and argv[0] == '--selftest':
        return selftest()
    if not argv or argv[0] == '--check':
        return run_check()
    if argv[0] == '--bank':
        return bank()
    if argv[0] == '--init' and not os.path.exists(BASELINE):
        write_baseline(tree_modules()[1])
        print(f'module-block ratchet: wrote {BASELINE}')
        return 0
    print(__doc__.split('\n\n')[1], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
