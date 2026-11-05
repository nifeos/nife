#!/usr/bin/env python3
"""A name claims no Unix service model: no `daemon`, and no crate or program named `...d`.

    python3 service_model.py [--manifest Cargo.toml] [--report] FILE...
    python3 service_model.py --prose FILE...
    python3 service_model.py --selftest

A name is a claim made before a reader sees a line of code, and a crate named `netd` claims a
long-running Unix service with a supervisor, a socket and a pid file. Most programs here are
components that do one job and exit, or services reached through a contract, and the name says
which. Three rules, one per mode:

- The names the crate defines (helpers/rust_names.py says which) hold no `daemon` component, and
  none ends in a lone `d` component (`net_d`).
- With `--manifest`, the package name and each `[[bin]]` name of four or more characters do not end
  in `d` (`netd`, `compd`). Three characters or fewer is too little room to claim a service model
  (`kbd`, `pwd`). A name that really ends in `d` (`uuid`) keeps it by saying so: the crate root's
  docs open with `(<name>, <source>)`, as rust_names.py describes for a package.
- `--prose` reads text files (README, docs, comments, manifests) and refuses the word `daemon` on
  any line. A line that quotes a source keeps it by holding `(daemon, <source>)`. The vocabulary
  is component, service and contract.

A Rust name kept from a datasheet, a specification or the standard library uses the kept form at the
item: the doc comment's first paragraph opens with `(<name>, <source>)` and `#[doc(alias)]` carries
a plain-English name. There is no baseline: the ceiling is 0.

The rule follows the naming rules page of the operating system project this template serves
(design/naming.md there). The file's name is provisional.
"""

import os
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import rust_names  # noqa: E402

RULE = 'a name claims no Unix service model'

DAEMON = re.compile(r'\bdaemons?\b', re.I)
KEPT_LINE = re.compile(r'\(\s*`?daemons?`?\s*,\s*[^)]*[A-Za-z][^)]*\)', re.I)
ADVICE = 'name it for what it is: a component, a service or a contract'


def hits_of_words(_words):
    def hits(definition):
        name = definition.name
        parts = rust_names.components(name)
        out = []
        if any(p in ('daemon', 'daemons') for p in parts):
            out.append(('daemon', ADVICE))
        elif len(parts) > 1 and parts[-1] == 'd':
            out.append(('d', ADVICE))
        elif definition.kind in ('package', 'program') and len(name) >= 4 and name.endswith('d'):
            out.append(('d', ADVICE))
        return out
    return hits


def phrase(word, advice):
    return f'`{word}` claims a Unix service model: {advice}'


def prose_problems(path, text):
    out = []
    for number, line in enumerate(text.splitlines(), 1):
        if DAEMON.search(line) and not KEPT_LINE.search(line):
            out.append(f'{path}:{number}: the word "daemon" claims a Unix service model; the '
                       f'vocabulary is component, service and contract. A quotation keeps it with '
                       f'`(daemon, <source>)` on the line.')
    return out


def prose(files):
    bad, read = [], 0
    for path in files:
        try:
            data = Path(path).read_bytes()
        except OSError:
            continue  # a tracked symlink to a directory, or a file deleted in the working tree
        if b'\0' in data:
            continue
        read += 1
        bad += prose_problems(path, data.decode('utf-8', errors='replace'))
    for line in bad:
        print(line, file=sys.stderr)
    if len(bad) <= rust_names.CEILING:
        print(f'service_model --prose: {read} files read, {len(bad)} lines refused '
              f'(ceiling {rust_names.CEILING})')
        return 0
    print(f'\nservice_model --prose: {len(bad)} lines over the ceiling of {rust_names.CEILING}.',
          file=sys.stderr)
    return 1


def selftest():
    hits = hits_of_words({})
    src = '''
pub struct DaemonState;
pub fn run_daemon() {}
pub fn net_d() {}
pub const fn standard() {}
/// The hosts (daemons, Example Specification 1).
#[doc(alias = "background service")]
pub fn daemons() {}
pub fn thread() { let spawned = 1; }
'''
    bad = rust_names.judge('x.rs', rust_names.definitions(src), hits, phrase)
    names = [line.split(': ')[1] for line in bad]
    want = ['struct DaemonState', 'fn run_daemon', 'fn net_d']
    failed = 0
    if names != want:
        failed += 1
        print(f'service_model selftest: refused {names}, wanted {want}', file=sys.stderr)
    for kind, name, refused in (('package', 'netd', True), ('program', 'compd', True),
                                ('package', 'kbd', False), ('package', 'uuid', True),
                                ('package', 'keyboard_driver', False),
                                ('fn', 'thread', False)):
        got = bool(hits(rust_names.Definition(name, kind, 1)))
        if got != refused:
            failed += 1
            print(f'service_model selftest: {kind} {name} refused={got}, wanted {refused}',
                  file=sys.stderr)
    kept = rust_names.Definition('uuid', 'package', 1, ['The identifier (uuid, RFC 9562).'])
    if rust_names.judge('Cargo.toml', [kept], hits, phrase):
        failed += 1
        print('service_model selftest: a package keeps `uuid` by stating its source', file=sys.stderr)
    lines = prose_problems('x.md', 'a daemon here\nthe Daemons there\nundaemonized ok\n'
                           'the (daemon, RFC 1) word\n')
    if len(lines) != 2 or not lines[0].startswith('x.md:1:') or not lines[1].startswith('x.md:2:'):
        failed += 1
        print(f'service_model selftest: prose {lines}', file=sys.stderr)
    if failed:
        return 1
    print('service_model selftest: ok')
    return 0


if __name__ == '__main__':
    if sys.argv[1:2] == ['--prose']:
        sys.exit(prose(sys.argv[2:]))
    sys.exit(rust_names.run(sys.argv[1:], 'service_model', RULE, hits_of_words, selftest, phrase))
