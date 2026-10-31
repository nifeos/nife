#!/usr/bin/env python3
"""A pull request that moves a magic or a number moves its version constant too.

    python3 helpers/format_values_check.py            # judge the tree against the merge base
    python3 helpers/format_values_check.py --selftest # the rules, as fixtures plus a scratch repo

The enforcement of the shape calef ruled on 2026-10-29 (UTC, PR #2852, question 5), built by
milestone 956 (the linked interfaces carry checkable versions) step 7. The pinned-values test in
`crates/abi` catches a renumber that skips the test; this catches the other half, a value that
moves with its test edited and its version constant left alone. It reads the pull request's
changed lines (the merge base against the tree) and fails in two cases:

1. A `MAGIC` constant's value changed in a crate, and no `VERSION` or `REVISION` constant's value
   changed in that crate. A magic is a format's identity, and a format never changes in place, so
   a new magic is a new version.
2. A public number in `crates/abi` changed or was removed (a `pub const` of any module, or an
   `Error` discriminant), and `ABI_VERSION` did not change. The syscall surface is one version;
   adding a number is an addition and owes nothing.

It is the diff-grep the ruling names, not the rustdoc diff, which waits for a consumer outside the
tree. It sees what a line says: a constant whose value moves through another constant it names is
read by its text, and a semantic change behind an unchanged number is a break this cannot see (the
blind spot milestone 610 records for the weekly series, too). A test module's constants are not a
format and are skipped, as is any file under a `tests` path.

Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions. A
shared module under `helpers/`, which `script/names` puts out of its own scope, so its provenance
is this paragraph.
"""
import os
import re
import subprocess
import sys
import tempfile

# A public constant, joined across continuation lines up to its semicolon.
CONST = re.compile(r'^\s*pub\s+const\s+([A-Z][A-Z0-9_]*)\s*:[^=]+=\s*([^;]+);', re.M)
# An enum discriminant line: `    NoSuchSlot = -1,`.
DISCRIMINANT = re.compile(r'^\s+([A-Z][A-Za-z0-9]*)\s*=\s*(-?\d+)\s*,', re.M)
MAGIC = re.compile(r'(^|_)MAGIC($|_)')
VERSION = re.compile(r'(^|_)(VERSION|REVISION)($|_)')
ABI_CRATE = 'crates/abi/'


def values(text, path):
    """{key: normalized value} of the public constants (and, in abi, the discriminants)."""
    found = {}
    # Drop the test module: everything from `#[cfg(test)]` on is not a public format.
    text = text.split('#[cfg(test)]', 1)[0]
    for name, value in CONST.findall(text):
        found[('const', name)] = ' '.join(value.split())
    if path.startswith(ABI_CRATE):
        for name, value in DISCRIMINANT.findall(text):
            found[('discriminant', name)] = value
    return found


def judged(path):
    return (path.startswith('crates/') and path.endswith('.rs')
            and '/tests' not in path and not path.endswith('/tests.rs'))


def crate_of(path):
    return '/'.join(path.split('/')[:2])


def problems(files):
    """`files` is {path: (base text or None, head text or None)}; returns the failure messages."""
    moved = {}  # crate -> {'magic': [...], 'version': [...], 'abi': [...]}
    for path, (old, new) in sorted(files.items()):
        if not judged(path):
            continue
        before = values(old or '', path)
        after = values(new or '', path)
        crate = moved.setdefault(crate_of(path), {'magic': [], 'version': [], 'abi': []})
        for key in sorted(set(before) | set(after)):
            kind, name = key
            was, now = before.get(key), after.get(key)
            if was == now:
                continue
            changed = was is not None  # a changed or removed value; a new one is an addition
            if not changed:
                continue
            label = f'{path}: {name} {was} -> {now if now is not None else "(removed)"}'
            if kind == 'const' and MAGIC.search(name):
                crate_bucket = 'magic'
            elif kind == 'const' and VERSION.search(name) and now is not None:
                crate_bucket = 'version'
            else:
                crate_bucket = None
            if crate_bucket:
                crate and moved[crate_of(path)][crate_bucket].append(label)
            if path.startswith(ABI_CRATE):
                if name == 'ABI_VERSION':
                    moved[crate_of(path)]['version'].append(label)
                else:
                    moved[crate_of(path)]['abi'].append(label)
    out = []
    for crate, buckets in sorted(moved.items()):
        if buckets['magic'] and not buckets['version']:
            out.append(f'{crate}: a magic moved and no version constant moved with it:\n    '
                       + '\n    '.join(buckets['magic']))
        if buckets['abi'] and not any('ABI_VERSION' in v for v in buckets['version']):
            out.append(f'{crate}: a public number moved or went away and ABI_VERSION did not '
                       'change:\n    ' + '\n    '.join(buckets['abi']))
    return out


def git(*args, cwd=None):
    run = subprocess.run(['git', *args], capture_output=True, text=True, cwd=cwd)
    return run.stdout if run.returncode == 0 else None


def changed_files(base, cwd=None):
    names = git('diff', '--name-only', '--no-renames', base, '--', 'crates', cwd=cwd) or ''
    files = {}
    for path in names.split():
        if not judged(path):
            continue
        old = git('show', f'{base}:{path}', cwd=cwd)
        head = os.path.join(cwd or '.', path)
        new = open(head, errors='replace').read() if os.path.exists(head) else None
        files[path] = (old, new)
    return files


def merge_base():
    override = os.environ.get('FORMAT_VALUES_BASE')
    if override:
        return (git('rev-parse', override) or '').strip() or None
    base = (git('merge-base', 'HEAD', 'origin/main') or '').strip()
    head = (git('rev-parse', 'HEAD') or '').strip()
    return base if base and base != head else None


def selftest():
    def fails(what, files, expect):
        got = problems(files)
        assert bool(got) == expect, f'{what}: {got}'

    magic_old = 'pub const MAGIC: u64 = 1;\npub const VERSION: u32 = 1;\n'
    fails('a magic moved alone fails',
          {'crates/x/src/lib.rs': (magic_old, 'pub const MAGIC: u64 = 2;\npub const VERSION: u32 = 1;\n')}, True)
    fails('a magic moved with its version passes',
          {'crates/x/src/lib.rs': (magic_old, 'pub const MAGIC: u64 = 2;\npub const VERSION: u32 = 2;\n')}, False)
    fails('a version moved with the magic in another file of the crate passes',
          {'crates/x/src/lib.rs': (magic_old, 'pub const MAGIC: u64 = 2;\npub const VERSION: u32 = 1;\n'),
           'crates/x/src/v.rs': ('pub const SCHEMA_VERSION: u32 = 1;\n', 'pub const SCHEMA_VERSION: u32 = 2;\n')},
          False)
    fails('a new magic is an addition',
          {'crates/x/src/lib.rs': ('', 'pub const MAGIC: u64 = 2;\n')}, False)
    fails('a removed magic fails',
          {'crates/x/src/lib.rs': (magic_old, 'pub const VERSION: u32 = 1;\n')}, True)
    fails('a magic in a test file is not a format',
          {'crates/x/src/tests.rs': ('pub const MAGIC: u64 = 1;\n', 'pub const MAGIC: u64 = 2;\n')}, False)
    fails('a magic in a test module is not a format',
          {'crates/x/src/lib.rs': ('#[cfg(test)]\nmod t { pub const MAGIC: u64 = 1; }\n',
                                    '#[cfg(test)]\nmod t { pub const MAGIC: u64 = 2; }\n')}, False)
    fails('a multi-line value is read whole',
          {'crates/x/src/lib.rs': ('pub const MAGIC: u64 =\n    1;\n', 'pub const MAGIC: u64 =\n    2;\n')}, True)

    abi = 'pub const ABI_VERSION: u32 = 1;\npub const SYS_EXIT: u64 = 0;\n'
    fails('a renumbered syscall without the version fails',
          {'crates/abi/src/lib.rs': (abi, 'pub const ABI_VERSION: u32 = 1;\npub const SYS_EXIT: u64 = 9;\n')}, True)
    fails('a renumbered syscall with the version passes',
          {'crates/abi/src/lib.rs': (abi, 'pub const ABI_VERSION: u32 = 2;\npub const SYS_EXIT: u64 = 9;\n')}, False)
    fails('a number in a module of abi without the version fails',
          {'crates/abi/src/timer.rs': ('pub const ARM: u64 = 0;\n', 'pub const ARM: u64 = 1;\n'),
           'crates/abi/src/lib.rs': (abi, abi)}, True)
    fails('an added syscall is an addition',
          {'crates/abi/src/lib.rs': (abi, abi + 'pub const SYS_NEW: u64 = 5;\n')}, False)
    fails('a removed number fails',
          {'crates/abi/src/lib.rs': (abi, 'pub const ABI_VERSION: u32 = 1;\n')}, True)
    fails('a renumbered error without the version fails',
          {'crates/abi/src/lib.rs': ('    NoSuchSlot = -1,\n', '    NoSuchSlot = -2,\n')}, True)
    fails('a constant outside abi is not an abi number',
          {'crates/x/src/lib.rs': ('pub const SIZE: u64 = 1;\n', 'pub const SIZE: u64 = 2;\n')}, False)

    # The same rules over a scratch repository: a synthetic pull request that moves a value.
    with tempfile.TemporaryDirectory() as scratch:
        def sh(*args):
            subprocess.run(['git', '-c', 'user.name=t', '-c', 'user.email=t@t', *args], cwd=scratch,
                           check=True, capture_output=True)
        sh('init', '-q')
        os.makedirs(os.path.join(scratch, 'crates/x/src'))
        lib = os.path.join(scratch, 'crates/x/src/lib.rs')
        open(lib, 'w').write(magic_old)
        sh('add', '.')
        sh('commit', '-q', '-m', 'base')
        base = git('rev-parse', 'HEAD', cwd=scratch).strip()
        open(lib, 'w').write('pub const MAGIC: u64 = 2;\npub const VERSION: u32 = 1;\n')
        sh('commit', '-qam', 'moves a magic alone')
        assert problems(changed_files(base, cwd=scratch)), 'the scratch pull request must fail'
        open(lib, 'w').write('pub const MAGIC: u64 = 2;\npub const VERSION: u32 = 2;\n')
        sh('commit', '-qam', 'and its version')
        assert not problems(changed_files(base, cwd=scratch)), 'with its version it passes'
    print('format_values_check selftest: ok')


def main():
    if '--selftest' in sys.argv:
        selftest()
        return 0
    base = merge_base()
    if base is None:
        print('format values: no merge base (on main, or origin/main is missing); nothing to compare')
        return 0
    bad = problems(changed_files(base))
    for message in bad:
        print(f'format values: {message}', file=sys.stderr)
    if bad:
        print('format values: a magic or a public ABI number moved without its version constant. '
              'Bump it in the same change, with the dated ledger entry (design/roadmap/0979).',
              file=sys.stderr)
        return 1
    print('format values: every moved magic or ABI number moved with its version constant')
    return 0


if __name__ == '__main__':
    sys.exit(main())
