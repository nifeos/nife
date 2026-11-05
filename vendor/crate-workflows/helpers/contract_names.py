#!/usr/bin/env python3
"""A crate that holds a wire or service contract is named `..._contract`, not `..._protocol`.

    python3 contract_names.py --manifest Cargo.toml
    python3 contract_names.py --selftest

One spelling for one idea: a second habit is how the first one rots. The package name and each
`[[bin]]` name must not end in `proto` or `protocol` as their last word, and must not end in
`contract` glued to another word (`fscontract`); the spelling is `<what>_contract`. A network
standard whose own name ends in `protocol` keeps it by saying so: the crate root's docs open with
`(<name>, <source>)`, as rust_names.py describes for a package.

A crate that keeps the word `protocol` for a different reason (it implements the standard, not a
contract of its own) is the kept case; a crate that defines its own contract takes the suffix.
There is no baseline: the ceiling is 0.

The rule follows the naming rules page of the operating system project this template serves
(design/naming.md there). The file's name is provisional.
"""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import rust_names  # noqa: E402

RULE = 'a contract crate is spelled `_contract`'


def hits_of_words(_words):
    def hits(definition):
        if definition.kind not in ('package', 'program'):
            return []
        name = definition.name
        parts = rust_names.components(name)
        if parts and parts[-1] in ('proto', 'protocol') and len(parts) > 1:
            return [(parts[-1], f'`{name[:name.lower().rfind(parts[-1])].rstrip("_-")}_contract`')]
        if name.endswith('contract') and not name.endswith('_contract') and name != 'contract':
            return [('contract', f'`{name[:-len("contract")]}_contract`')]
        return []
    return hits


def phrase(word, instead):
    return f'`{word}` ends the name of a contract crate: spell it {instead}'


def selftest():
    hits = hits_of_words({})
    failed = 0
    for kind, name, want in (('package', 'storage_contract', False),
                             ('package', 'storage_proto', True),
                             ('package', 'storage_protocol', True),
                             ('program', 'fscontract', True),
                             ('package', 'protocol', False),
                             ('package', 'protocols_table', False),
                             ('fn', 'storage_protocol', False)):
        got = bool(hits(rust_names.Definition(name, kind, 1)))
        if got != want:
            failed += 1
            print(f'contract_names selftest: {kind} {name} refused={got}, wanted {want}',
                  file=sys.stderr)
    named = hits(rust_names.Definition('storage_protocol', 'package', 1))
    if named != [('protocol', '`storage_contract`')]:
        failed += 1
        print(f'contract_names selftest: advice {named}', file=sys.stderr)
    kept = rust_names.Definition('network_time_protocol', 'package', 1,
                                 ['The standard (network_time_protocol, RFC 5905).'])
    if rust_names.judge('Cargo.toml', [kept], hits, phrase):
        failed += 1
        print('contract_names selftest: a standard keeps its own name by stating its source',
              file=sys.stderr)
    if failed:
        return 1
    print('contract_names selftest: ok')
    return 0


if __name__ == '__main__':
    argv = sys.argv[1:]
    if argv and argv[0] != '--selftest' and '--manifest' not in argv:
        print(__doc__.split('\n\n')[0], file=sys.stderr)
        sys.exit(2)
    sys.exit(rust_names.run(argv, 'contract_names', RULE, hits_of_words, selftest, phrase))
