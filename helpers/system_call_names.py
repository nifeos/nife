"""`syscall` is spelled `system_call`: the gate on the identifiers and paths this tree authors.

    python3 helpers/system_call_names.py --check      # what script/lint runs
    python3 helpers/system_call_names.py --list [PATH..]  # every counted hit, path:line: identifier
    python3 helpers/system_call_names.py --bank       # lower the ceiling to the tree; never raises
    python3 helpers/system_call_names.py --selftest   # the traps, as fixtures

design/naming/acronyms-and-abbreviations.md spells `syscall` out (ruled 2026-11-20 UTC): snake case
`system_call` and `raw_system_call`, CamelCase `SystemCall`, constants `SYSTEM_CALL_...`, and the
same in a module, crate, file, directory, script, test or variable name. This check counts what
still breaks that:

- every Rust identifier with a `syscall` or `syscalls` component, in any casing (`syscall`,
  `SYSCALL_EXIT`, `NullSyscall`, `redox_syscall`), read by the template's lexer
  (`vendor/crate-workflows/helpers/rust_names.py`), so comments and string literals are not read;
- every Python identifier with such a component, read by Python's own tokenizer, in `.py` files and
  in an extensionless file whose first line runs Python;
- every tracked path with such a component, one hit per path (`kernel/src/syscall.rs`,
  `notes/syscall-boundary.md`).

Components split at `_`, `-`, `.`, `/`, digits and CamelCase boundaries, so `vsyscall` (Linux's
page) is one component and never matches. Unlike the template's spelled-out check, which judges the
names a crate defines, this counts every identifier the tree writes, a use as well as a definition,
since a use of a renamed item breaks the build anyway and a use of a foreign one is listed below.

**Two halves**, as the `cap` ratchet beside it (`helpers/vendored_name_gates.py`): a count over
`CEILING` fails, which guards `main`, and a branch is held to its merge base, so the count summed
over the files it changed may not grow.

**The boundary is `KEPT`, and only `KEPT`.** A name that arrives across another project's interface
keeps its owner's spelling (design/naming.md, "Acronyms and abbreviations"), and only the identifier
the owner wrote keeps it. Each entry is keyed by file, holds the identifiers (or the path itself)
that keep `syscall`, and says whose word it is. An entry is removed by renaming what it names; a
public item that keeps the word carries `#[doc(alias = "syscall")]` beside its expansion. Out of
scope for a stated reason, not a backlog: `vendor/` and `target/`, which are somebody else's code or
no code at all, and `patches/*.patch`, which are diffs of somebody else's code. The std overlay under
`patches/std-nife/overlay/` is this tree's own and is read.

Name: provisional. A shared helper under `helpers/`, which `script/names` puts out of its own
scope, so this paragraph is the record. It takes `system_call` for the word it guards and `names`
for what it reads. calef has not named it.

BUGS:
- Shell, YAML, TOML, linker scripts and assembly are not read, so an environment variable or a job
  name spelled with `syscall` there is not counted. Prose and comments are not read either, though
  the ruling covers them: a reader catches those.
- A lexer, not a parser. A macro that builds an identifier from a string is not followed.
"""

import io
import os
import re
import subprocess
import sys
import tokenize

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import vendored_name_gates  # noqa: E402

SELF = 'helpers/system_call_names.py'
WORDS = ('syscall', 'syscalls')
EXCLUDED_PREFIXES = ('vendor/', 'target/')

# The count when this check landed, lowered by each sweep's `--bank`. Never raise it by hand: a new
# `syscall` is spelled `system_call`, or it is a foreign name and goes in KEPT with its owner.
CEILING = 0

# The identifiers and paths that keep `syscall`, by file, each group with whose word it is. See the
# header before adding one.
_REDOX = ("the `redox_syscall` crate's own name, and `syscall`, the name its library is imported "
          "under")
_LINUX = "the C library's `syscall` function, Linux's `syscall(2)` and its macOS twin"
KEPT = {
    # RedoxFS's error type comes from `redox_syscall`, whose library is imported as `syscall`.
    'redoxfs_server/src/bin/mkfs.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/bin/redoxfs_server.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/bin/second_mount.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/block.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/crash.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/dispatch.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/lib.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/server.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/tests/directory_capability.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/tests/entries.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/tests/handles.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/tests/name_window.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/src/walk_model.rs': ({'syscall'}, _REDOX),
    'redoxfs_server/tests/crash_consistency.rs': ({'syscall'}, _REDOX),
    'tools/redoxfs_host/src/attributes.rs': ({'syscall'}, _REDOX),
    'tools/redoxfs_host/src/lib.rs': ({'syscall'}, _REDOX),
    'tools/redoxfs_host/src/volume.rs': ({'syscall'}, _REDOX),
    # The host benchmarks call the C library's `syscall` function, which Linux and macOS name.
    'bench/host/linux_all.rs': ({'syscall'}, _LINUX),
    'bench/host/linux_fs.rs': ({'syscall'}, _LINUX),
    'bench/host/null_system_call.rs': ({'syscall'}, _LINUX),
}


def _scope(path):
    if path.startswith(EXCLUDED_PREFIXES):
        return False
    return not (path.startswith('patches/') and path.endswith('.patch'))


def _is_python(path, text):
    if path.endswith('.py'):
        return True
    if '.' in os.path.basename(path):
        return False
    return text.split('\n', 1)[0].startswith('#!') and 'python' in text.split('\n', 1)[0]


def components(word):
    """`NullSyscall` -> null, syscall; `notes/syscall-boundary.md` -> notes, syscall, boundary, md."""
    out = []
    for part in re.split(r'[-./]', word):
        out += vendored_name_gates.components(part)
    return out


def counts(word):
    return any(c in WORDS for c in components(word))


def python_identifiers(text):
    out = []
    try:
        for tok in tokenize.generate_tokens(io.StringIO(text).readline):
            if tok.type == tokenize.NAME:
                out.append((tok.start[0], tok.string))
    except (tokenize.TokenError, IndentationError, SyntaxError):
        pass
    return out


def hits(text, path=''):
    """[(line, identifier)] that keep `syscall` in `text`, read as the file at `path`, plus the path
    itself at line 0 when it carries the word. KEPT's entry for the file is subtracted."""
    kept = KEPT.get(path, (set(), ''))[0]
    found = []
    if path and counts(path):
        found.append((0, path))
    if path.endswith('.rs'):
        found += vendored_name_gates.identifiers(text)
    elif _is_python(path, text):
        found += python_identifiers(text)
    return [(line, word) for line, word in found
            if (line == 0 or counts(word)) and word not in kept]


def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True)
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
            if f and _scope(f) and os.path.isfile(f) and not os.path.islink(f)]


def census(paths):
    out = {}
    for p in paths:
        t = read(p)
        out[p] = hits(t if t is not None else '', p)
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
        if _scope(p):
            where[p] = where.get(p, 0) - len(hits(git('show', f'{base}:{p}') or '', p))
    for p in new_paths:
        if _scope(p) and os.path.isfile(p) and not os.path.islink(p):
            where[p] = where.get(p, 0) + len(hits(read(p) or '', p))
    return sum(where.values()), where


def selftest():
    cases = [
        # (path, source, the identifiers that must count)
        ('a.rs', 'fn syscall(n: u64) -> Syscall { SYSCALL_EXIT; raw_syscall(); syscalls }',
         ['syscall', 'Syscall', 'SYSCALL_EXIT', 'raw_syscall', 'syscalls']),
        ('a.rs', 'struct NullSyscall; const NIFE_SYSCALL_FUZZ_SEEDS: u8 = 0; let syscall5 = 1;',
         ['NullSyscall', 'NIFE_SYSCALL_FUZZ_SEEDS', 'syscall5']),
        # Comments, strings and the instruction inside an `asm!` string are not identifiers.
        ('a.rs', '// a syscall\n/* syscall */ let x = "syscall"; asm!("syscall"); let s = r#"syscall"#;',
         []),
        # The lookalikes: Linux's page, and the words that contain the letters.
        ('a.rs', 'let vsyscall = 1; let system_call = 2; let SystemCall = 3; let syscallish = 4;',
         []),
        ('b.py', 'def rule_syscall_error():\n    # a syscall\n    return "syscall"\n',
         ['rule_syscall_error']),
        ('notes/syscall-boundary.md', 'the syscall boundary', ['notes/syscall-boundary.md']),
        ('kernel/src/syscall/tests.rs', 'fn f() {}', ['kernel/src/syscall/tests.rs']),
        ('notes/system-call-boundary.md', 'syscall', []),
    ]
    failed = 0
    for path, src, want in cases:
        got = [w for _, w in hits(src, path)]
        if got != want:
            failed += 1
            print(f'system_call selftest: {path} {src!r} counted {got}, wanted {want}', file=sys.stderr)
    KEPT['k.rs'] = ({'redox_syscall', 'k.rs'}, 'fixture')
    got = [w for _, w in hits('use redox_syscall; let syscall = 1;', 'k.rs')]
    del KEPT['k.rs']
    if got != ['syscall']:
        failed += 1
        print(f'system_call selftest: KEPT counted {got}, wanted [\'syscall\']', file=sys.stderr)
    lines = [ln for ln, _ in hits('/* a\nb */\nlet s = "x\ny";\nlet syscall = 1;', 'c.rs')]
    if lines != [5]:
        failed += 1
        print(f'system_call selftest: line numbers {lines}, wanted [5]', file=sys.stderr)
    if not _is_python('script/x', '#!/usr/bin/env python3\n') or _is_python('script/x', '#!/bin/sh\n'):
        failed += 1
        print('system_call selftest: a script\'s shebang was misread', file=sys.stderr)
    if failed:
        return 1
    print(f'system_call selftest: {len(cases) + 3} fixtures')
    return 0


def main(argv):
    root = (git('rev-parse', '--show-toplevel') or '').strip() or os.getcwd()
    os.chdir(root)
    if argv and argv[0] == '--selftest':
        return selftest()
    if not argv or argv[0] == '--check':
        bad = []
        now = total(census(tracked()))
        if now > CEILING:
            bad.append(f'{now:,} identifiers and paths spell syscall, over the ceiling of '
                       f'{CEILING:,} in {SELF}. If this branch adds none, main moved past it: merge '
                       f'main, and set the ceiling to the merged count in this change, saying why')
        base = merge_base()
        if base:
            net, where = growth(base)
            if net > 0:
                grew = sorted(p for p, d in where.items() if d > 0)
                bad.append(f'this branch adds {net} net, in {", ".join(grew)}')
        if bad:
            print('system_call: the tree gained identifiers or paths that spell syscall '
                  '(design/naming/acronyms-and-abbreviations.md spells it `system_call`):',
                  file=sys.stderr)
            for b in bad:
                print(f'  {b}', file=sys.stderr)
            print(f'\nSpell it `system_call`, `SystemCall` or `SYSTEM_CALL` (`python3 {SELF} --list '
                  f'PATH` shows each hit). A foreign name goes in KEPT in {SELF}, with its owner.',
                  file=sys.stderr)
            return 1
        print(f'system_call: {now:,} identifiers and paths spell syscall (ceiling held)')
        return 0
    if argv[0] == '--list':
        paths = argv[1:] or tracked()
        for p, found in sorted(census([x for x in paths if _scope(x)]).items()):
            for line, word in found:
                print(f'{p}:{line}: {word}')
        return 0
    if argv[0] == '--bank':
        now = total(census(tracked()))
        text = open(SELF).read()
        text = re.sub(r'(?m)^(CEILING = )[\d_]+$', rf'\g<1>{min(now, CEILING):_}', text, count=1)
        with open(SELF, 'w') as f:
            f.write(text)
        print(f'system_call: ceiling now {min(now, CEILING):,}')
        return 0
    print(__doc__.split('\n\n')[1], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
