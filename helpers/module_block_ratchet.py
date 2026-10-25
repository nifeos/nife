"""The module-block ratchet: every module, document and `pub` item carries a `Name:` block, held to a list that only shrinks.

A module is a file (`helpers/name_census.py`'s `is_module_file`): every `.rs` file under a `src/`
directory outside vendored code, whether or not anyone wrote its block. So is a document directly
under `design/`, `notes/` or `briefs/` (`is_top_document`), whose block is a paragraph of its own
opening `Name:`, and so is every `pub` item in a module (`pub_items`; constants and statics
included, calef 2026-10-18 UTC). That is what lets `script/names` and `script/metrics` count every one, and it
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

- A row is a path (a module or document) or `path::name` (an item), and the list is a multiset: a
  file with three unblocked `pub fn new` holds three identical rows.
- A name with no block beyond what the list holds fails. A new file or a new `pub` item is not on
  the list, so it arrives with its block: `//! Name: provisional, minted <date> (UTC) by <lane>.`
  at the top of a module, `/// Name: ...` in an item's doc comment, or `*Name: ...*` under a
  document's title, is enough.
- A row with no unblocked name behind it (the name gained a block, or is gone) fails until it is
  removed, so dead rows cannot accumulate. `--bank` removes them.
- Against the merge base a row may not be added. A rename moves its rows in the same change: a file
  git calls renamed may take its old rows' place, so moving a file does not oblige a block. So does
  a split, which git calls no rename: an item that leaves one file for another in the same crate,
  keeping its name and kind, takes its row along (`helpers/moved_rows.py`), and `--bank` re-keys it.
- A selection that matches no module fails, because a gate that judges nothing passes.

**What it does not hold.** Status: a `provisional` block passes, as `script/names --check` does.
An item a macro generates is invisible to `pub_items`, so it is never held.
"""

import collections
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import moved_rows  # noqa: E402
import name_census  # noqa: E402

BASELINE = 'design/module-block-baseline.txt'
# The baseline's format. A merge base whose baseline lacks this line holds a list of a smaller
# scope (format 1 had no items), so the change that widens the scope is compared with nothing, as
# the change that first lands a baseline is. This is an exception and a foot gun: bumping the
# format skips the no-additions rule for that one change, so a bump needs a widened scope behind it
# and says so in its pull request.
FORMAT = '# format 2: modules, documents and pub items'

HEADER = FORMAT + """
# The module-block ratchet's baseline: helpers/module_block_ratchet.py, whose header is the manual.
# One row per name that carried no `Name:` block when this list was written: a module (every .rs
# file under a src/ directory) or a document directly under design/, notes/ or briefs/ by its path,
# a `pub` item as path::name, once per item. The list only shrinks. Remove a row in the change that
# writes its block or deletes it (`python3 helpers/module_block_ratchet.py --bank` does it); a row
# may not be added (script/lint compares against the merge base), and a rename moves its rows.
"""

def path_of(row):
    return row.split('::', 1)[0]


def remedy(path):
    if '::' in path:
        return ('write `/// Name: provisional, minted <date> (UTC) by <lane>.` as a paragraph of its '
                'doc comment (design/naming.md)')
    if path.endswith('.md'):
        return ('write `*Name: provisional, minted <date> (UTC) by <lane>.*` as the paragraph under '
                'its title (design/naming.md)')
    return ('write its block, `//! Name: provisional, minted <date> (UTC) by <lane>.`, at the top '
            'of the file (design/naming.md)')


# --- the baseline and git -------------------------------------------------------------------------

def read_baseline(text):
    return collections.Counter(line.strip() for line in text.splitlines()
                               if line.strip() and not line.startswith('#'))


def write_baseline(rows):
    with open(BASELINE, 'w') as f:
        f.write(HEADER + ''.join(f'{row}\n' for row in sorted(rows.elements())))


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
    return judged, collections.Counter(unblocked)


# --- the rules, pure so the selftest can drive them ------------------------------------------------

def problems(modules, unblocked, rows, base_rows=None, renames=(), before=None, after=None):
    """Every failure in one tree. `base_rows` is None where the merge base holds no baseline.

    `before` and `after` read a file at the merge base and now; without them no row moves.
    """
    if not modules:
        return ['the check judged no file at all. A selector that matches nothing passes by '
                'checking nothing (design/roadmap/0401-a-gate-that-selects-the-set-it-judges.md)']
    bad = []
    for row, n in sorted((unblocked - rows).items()):
        bad.append(f'{row}: no `Name:` block, and {"it is" if n == 1 else f"{n} more are"} not on '
                   f'the list. A new name carries its block; {remedy(row)}')
    for row, n in sorted((rows - unblocked).items()):
        gone = path_of(row) not in modules or '::' in row
        why = 'has no unblocked name behind it' if gone else 'carries a block now'
        bad.append(f'{BASELINE}: {row} {why}{f" ({n} rows)" if n > 1 else ""}. Remove it (--bank); '
                   f'the list cannot go stale')
    if base_rows is not None:
        was_called = {new: old for old, new in renames}

        def as_it_was(row):
            path = path_of(row)
            old = was_called.get(path)
            return row if old is None else old + row[len(path):]

        moved = collections.Counter()
        for row, n in rows.items():
            moved[as_it_was(row)] += n
        added = moved - base_rows
        if added and before is not None:
            added -= moved_rows.arrivals(moved_rows.moves(added, base_rows - moved, before, after))
        for row, n in sorted(added.items()):
            bad.append(f'{BASELINE}: {row} was added. The list only shrinks; {remedy(row)}')
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
    before = None
    if base:
        def before(path):
            return git('show', f'{base}:{path}')
        renames = renamed_rust(base)
        old = git('show', f'{base}:{BASELINE}')
        if old is not None and FORMAT in old.splitlines():
            base_rows = read_baseline(old)
    bad = problems(modules, unblocked, rows, base_rows, renames, before, read_now)
    if bad:
        print('module-block ratchet: the tree breaks its baseline:', file=sys.stderr)
        for b in bad:
            print(f'  {b}', file=sys.stderr)
        return 1
    print(f'module-block ratchet: {len(modules)} module and document files, '
          f'{sum(unblocked.values())} names with no block, held to {sum(rows.values())} baseline rows')
    return 0


def read_now(path):
    try:
        return open(path, encoding='utf-8', errors='replace').read()
    except OSError:
        return None


def bank():
    if not os.path.exists(BASELINE):
        print(f'module-block ratchet: {BASELINE} does not exist; there is nothing to bank',
              file=sys.stderr)
        return 2
    _modules, unblocked = tree_modules()
    base = merge_base() or 'HEAD'
    renames = renamed_rust(base)
    was_called = {old: new for old, new in renames}
    old = collections.Counter()
    for row, n in read_baseline(open(BASELINE).read()).items():
        path = path_of(row)
        old[was_called.get(path, path) + row[len(path):]] += n

    def before(path):
        return git('show', f'{base}:{path}')
    pairs = moved_rows.moves(unblocked - old, old - unblocked, before, read_now)
    rows = moved_rows.banked(old, unblocked, pairs)
    write_baseline(rows)
    print(f'module-block ratchet: {sum(rows.values())} rows written to {BASELINE}')
    return 0


def selftest():
    def m(*names):
        return collections.Counter(f'x/src/{n}.rs' for n in names)

    def i(*names):
        return collections.Counter(f'x/src/a.rs::{n}' for n in names)

    files = {'x/src/a.rs'}

    cases = [
        ('a new module with no block fails', bool(problems(m('a', 'new'), m('new'), collections.Counter()))),
        ('a new module with a block passes', not problems(m('a', 'new'), collections.Counter(), collections.Counter())),
        ('a listed module with no block passes', not problems(m('a'), m('a'), m('a'), m('a'))),
        ('a listed module that gained a block fails', bool(problems(m('a'), collections.Counter(), m('a')))),
        ('a row for a deleted module fails', bool(problems(m('b'), collections.Counter(), m('a')))),
        ('a row added against the merge base fails',
         bool(problems(m('a', 'b'), m('a', 'b'), m('a', 'b'), m('a')))),
        ('a rename that moves its row passes',
         not problems(m('new'), m('new'), m('new'), m('old'), [('x/src/old.rs', 'x/src/new.rs')])),
        ('a rename that leaves its row behind fails',
         bool(problems(m('new'), m('new'), m('old'), m('old'), [('x/src/old.rs', 'x/src/new.rs')]))),
        ('a selector that matches nothing fails loudly',
         bool(problems(set(), collections.Counter(), collections.Counter()))),
        ('a new pub item with no block fails', bool(problems(files, i('f', 'g'), i('f')))),
        ('a third unblocked `new` beside two listed fails',
         bool(problems(files, i('new', 'new', 'new'), i('new', 'new')))),
        ('two listed `new`, one now blocked, leaves a stale row',
         bool(problems(files, i('new'), i('new', 'new')))),
        ('an item row added against the merge base fails',
         bool(problems(files, i('f', 'g'), i('f', 'g'), i('f')))),
        ('a rename carries its item rows',
         not problems({'x/src/b.rs'}, collections.Counter({'x/src/b.rs::f': 1}),
                      collections.Counter({'x/src/b.rs::f': 1}), i('f'),
                      [('x/src/a.rs', 'x/src/b.rs')])),
        ('pub constants, statics, methods and qualified fns are items',
         name_census.pub_items('pub const A: u8 = 0;\npub static mut B: u8 = 0;\n'
                               '    pub fn new() {}\npub const unsafe fn c() {}\n'
                               'pub struct D;\npub trait E {}') == ['A', 'B', 'new', 'c', 'D', 'E']),
        ('pub(crate), mod and private items are not',
         name_census.pub_items('pub(crate) fn a() {}\npub mod b;\nfn c() {}\n') == []),
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
    # A split git calls no rename: a.rs stays as the index, and two of its items move to b.rs.
    files = {'x/src/a.rs', 'x/src/b.rs'}
    was = {'x/src/a.rs': 'pub fn f() {}\npub fn new() {}\npub fn new() {}\npub struct S;\n'}
    now = {'x/src/a.rs': 'mod b;\npub fn new() {}\npub struct S;\n',
           'x/src/b.rs': '//! Name: provisional.\npub fn f() {}\npub fn new() {}\n'}
    base = i('f', 'new', 'new', 'S')
    tree = collections.Counter(['x/src/b.rs::f', 'x/src/a.rs::new', 'x/src/b.rs::new',
                                'x/src/a.rs::S'])
    rows = moved_rows.banked(base, tree, moved_rows.moves(tree - base, base - tree, was.get, now.get))
    grown = dict(now, **{'x/src/b.rs': now['x/src/b.rs'] + 'pub fn g() {}\n'})
    new = tree + collections.Counter(['x/src/b.rs::g'])
    retyped = dict(now, **{'x/src/b.rs': now['x/src/b.rs'].replace('fn f()', 'static f: u8 = 0;')})
    cases += [
        ('--bank re-keys the rows a split moved, and keeps their count',
         rows == tree and sum(rows.values()) == sum(base.values())),
        ('a split that moves items with their rows passes',
         not problems(files, tree, rows, base, (), was.get, now.get)),
        ('a split without the sources to prove it moves nothing',
         bool(problems(files, tree, rows, base))),
        ('--bank never admits a new name that arrived with a split',
         moved_rows.banked(base, new, moved_rows.moves(new - base, base - new, was.get,
                                                       grown.get)) == tree),
        ('a new name in a split fails',
         [b.split(' was added')[0] for b in problems(files, new, new, base, (), was.get, grown.get)]
         == [f'{BASELINE}: x/src/b.rs::g']),
        ('an item whose kind changed as it moved is not a move',
         [b.split(' was added')[0] for b in problems(files, tree, tree, base, (), was.get, retyped.get)]
         == [f'{BASELINE}: x/src/b.rs::f']),
    ]
    # A crate that ships alone keeps its blocks in a sidecar beside its milestone (name_census).
    tree_files = {
        'crates/a/src/lib.rs': 'pub fn f() {}\n',
        'crates/a/src/b.rs': 'pub fn g() {}\npub fn h() {}\n',
        'design/roadmap/0001-a/names.md':
            '# Names\n\n`crates/a/src/lib.rs` Name: provisional, minted 2026-10-25.\n\n'
            '`crates/a/src/b.rs` Name: provisional, minted 2026-10-25.\n\n'
            '`crates/a/src/b.rs::g` Name: provisional, minted 2026-10-25.\n\n'
            '`crates/a/src/b.rs::gone` Name: provisional, minted 2026-10-25.\n',
    }
    recs, probs, _miss, unblocked = name_census.census(list(tree_files), tree_files.get)
    kinds = {(r['kind'], r['name']) for r in recs}
    cases += [
        ('a sidecar block counts for the crate, the module and the item',
         {('crate', 'a'), ('module', 'b'), ('item', 'g')} <= kinds),
        ('names a sidecar does not cover stay unblocked',
         sorted(unblocked) == ['crates/a/src/b.rs::h', 'crates/a/src/lib.rs::f']),
        ('a sidecar key that names no item is a problem',
         any('crates/a/src/b.rs::gone' in p for p in probs)),
    ]
    msg = problems(set(m('a', 'new')), m('new'), collections.Counter())[0]
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
