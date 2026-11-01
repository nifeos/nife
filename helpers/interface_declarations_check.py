#!/usr/bin/env python3
"""A program's `carry_interfaces!` list names exactly the versioned interfaces it links.

    python3 helpers/interface_declarations_check.py            # judge the tree
    python3 helpers/interface_declarations_check.py --selftest # the rules, as fixtures

Milestone 956 (the linked interfaces carry checkable versions) makes condition 4 of milestone 756
(`process-tools`, `core-tools` and `disk-tools` are the first code to leave this repository) hold:
the moved packages declare what they link. A program declares it with
`capability_manifest_note::carry_interfaces!`, a list written by hand (the ruling of 2026-10-31
UTC leaves deriving it at package build to the first package that leaves the tree). This check is
what keeps a hand-written list from rotting. It fails when:

1. A program of a package in `CUT` carries no `carry_interfaces!` list.
2. A list misses an interface the program links, or names one it does not. What a program links is
   the workspace crates its source names (`crate::` paths, the list's own entries aside), closed
   over their normal dependencies in `cargo metadata`, less the crates of its own package; the
   versioned ones are those whose `lib.rs` carries `pub const INTERFACE_ID`. The list's macro
   counts: `capability_manifest_note` reaches `glob` through `grant_plan`, so every carrier
   declares `glob`.
3. An entry's version is not read from the same crate as its id, or names a constant that crate
   does not export as a public `u32` version.

It reads text and the dependency graph, not the linked bytes: a crate a program reaches only through
a macro it names by another crate's path is read as that other crate's. The compiler already
refuses an unsorted list.

Name: provisional, minted 2026-11-01 (UTC) by lane
milestone/956-the-linked-interfaces-carry-checkable-versions. A shared module under `helpers/`,
which `script/names` puts out of its own scope, so its provenance is this paragraph.
"""
import json
import os
import re
import subprocess
import sys
import tomllib

# The packages of milestone 756's first cut, whose every program declares what it links.
CUT = ('process-tools', 'core-tools', 'disk-tools')

INTERFACE_ID = re.compile(r'^\s*pub\s+const\s+INTERFACE_ID\s*:\s*u64\s*=', re.M)
VERSION_CONST = re.compile(r'^\s*pub\s+const\s+([A-Z0-9_]*VERSION[A-Z0-9_]*)\s*:\s*u32\s*=', re.M)
CARRY = re.compile(r'carry_interfaces!\s*\(\s*&\[(.*?)\]\s*\)', re.S)
DECLARED = re.compile(
    r'Declared\s*\{\s*id\s*:\s*([a-z_][a-z0-9_]*)::INTERFACE_ID\s*,'
    r'\s*version\s*:\s*([a-z_][a-z0-9_]*)::([A-Z][A-Z0-9_]*)\s*,?\s*\}')


def strip_comments(text):
    """The source with every `//` comment line removed, so a crate named in prose is not a link."""
    return '\n'.join(line for line in text.splitlines() if not line.lstrip().startswith('//'))


def named_crates(source, candidates):
    """The crates among `candidates` that `source` names by a `name::` path. The
    `carry_interfaces!` list's own entries do not count, or a declaration would justify itself;
    the macro's crate does, since the note's encoder is a dependency of the program."""
    code = CARRY.sub('capability_manifest_note::', strip_comments(source))
    return {c for c in candidates if re.search(r'(?<![A-Za-z0-9_:])' + re.escape(c) + r'::', code)}


def declarations(source):
    """The `(id crate, version crate, version constant)` entries of the one list, or None."""
    lists = CARRY.findall(strip_comments(source))
    if len(lists) != 1:
        return None if not lists else 'many'
    return DECLARED.findall(lists[0])


def problems(program, source, linked, versions):
    """What is wrong with one program's list. `linked` is the set of versioned crates it links;
    `versions` maps each versioned crate to its public `u32` version constants."""
    found = declarations(source)
    if found is None:
        return [f'{program}: carries no carry_interfaces! list, and it links {sorted(linked)}']
    if found == 'many':
        return [f'{program}: carries more than one carry_interfaces! list']
    out = []
    named = {crate for crate, _, _ in found}
    for crate in sorted(linked - named):
        out.append(f'{program}: links {crate} but does not declare it')
    for crate in sorted(named - linked):
        out.append(f'{program}: declares {crate} but does not link it')
    for crate, vcrate, vconst in found:
        if vcrate != crate:
            out.append(f'{program}: the entry for {crate} reads its version from {vcrate}')
        elif vconst not in versions.get(crate, ()):
            out.append(f'{program}: {crate}::{vconst} is not a public u32 version of {crate}')
    return out


def metadata():
    out = subprocess.run(['cargo', 'metadata', '--format-version', '1', '--offline'],
                         check=True, capture_output=True, text=True).stdout
    return json.loads(out)


def tree_problems():
    meta = metadata()
    members = set(meta['workspace_members'])
    package = {p['id']: p for p in meta['packages']}
    by_name = {package[i]['name']: i for i in members}
    edges = {n['id']: [d['pkg'] for d in n['deps']
                       if any(k['kind'] is None for k in d['dep_kinds'])]
             for n in meta['resolve']['nodes']}

    def closure(names):
        seen, stack = set(), [by_name[n] for n in names]
        while stack:
            i = stack.pop()
            if i not in seen:
                seen.add(i)
                stack.extend(edges.get(i, ()))
        return {package[i]['name'] for i in seen if i in members}

    versions = {}
    for name, i in by_name.items():
        lib = os.path.join(os.path.dirname(package[i]['manifest_path']), 'src', 'lib.rs')
        if os.path.exists(lib):
            text = open(lib, encoding='utf-8').read()
            if INTERFACE_ID.search(text):
                versions[name] = set(VERSION_CONST.findall(text))

    bins = {}
    for i in members:
        for t in package[i]['targets']:
            if 'bin' in t['kind']:
                bins.setdefault(t['name'], t['src_path'])

    out = []
    for name in CUT:
        with open(f'packages/{name}.package.toml', 'rb') as f:
            pkg = tomllib.load(f)
        own = set(pkg.get('crates', []))
        for program in pkg.get('programs', []):
            if program not in bins:
                out.append(f'{program}: in packages/{name}.package.toml but no binary target')
                continue
            source = open(bins[program], encoding='utf-8').read()
            linked = closure(named_crates(source, by_name)) - own
            out += problems(program, source, linked & set(versions), versions)
    return out


def selftest():
    versions = {'abi': {'ABI_VERSION'}, 'glob': {'VERSION'}}

    def entry(crate, vcrate=None, vconst='VERSION'):
        return (f'capability_manifest_note::Declared {{ id: {crate}::INTERFACE_ID, '
                f'version: {vcrate or crate}::{vconst} }},')

    def carry(*entries):
        return 'capability_manifest_note::carry_interfaces!(&[\n' + '\n'.join(entries) + '\n]);\n'

    good = carry(entry('abi', vconst='ABI_VERSION'), entry('glob'))
    cases = [
        ('a complete list passes', good, {'abi', 'glob'}, []),
        ('no list fails', 'fn main() {}\n', {'abi'}, ['carries no carry_interfaces!']),
        ('two lists fail', good + good, {'abi', 'glob'}, ['more than one']),
        ('a missing interface fails', carry(entry('abi', vconst='ABI_VERSION')), {'abi', 'glob'},
         ['links glob but does not declare it']),
        ('an extra interface fails', good, {'abi'}, ['declares glob but does not link it']),
        ('a version from another crate fails',
         carry(entry('abi', vconst='ABI_VERSION'), entry('glob', 'abi', 'ABI_VERSION')),
         {'abi', 'glob'}, ['reads its version from abi']),
        ('an unknown version constant fails', carry(entry('abi', vconst='VERSION'), entry('glob')),
         {'abi', 'glob'}, ['abi::VERSION is not a public u32 version']),
        ('a list in a comment does not count', '// ' + good.replace('\n', '\n// '), {'abi'},
         ['carries no carry_interfaces!']),
    ]
    for what, source, linked, expect in cases:
        got = problems('p', source, linked, versions)
        if not expect:
            assert got == [], f'{what}: {got}'
        else:
            assert len(got) == len(expect) and all(e in g for e, g in zip(expect, got)), \
                f'{what}: {got}'
    assert named_crates('use abi::x;\n// glob::y\nlet z = grant_plan::P;', {'abi', 'glob', 'grant_plan'}) \
        == {'abi', 'grant_plan'}
    assert named_crates('foo_abi::x; self::abi::y', {'abi'}) == set()
    assert named_crates(good, {'abi', 'glob', 'capability_manifest_note'}) \
        == {'capability_manifest_note'}
    print('interface_declarations_check selftest: ok')


def main():
    if '--selftest' in sys.argv:
        selftest()
        return 0
    found = tree_problems()
    for p in found:
        print(f'interface declarations: {p}', file=sys.stderr)
    if found:
        return 1
    print('interface declarations: every program of the first cut declares exactly the versioned '
          'interfaces it links')
    return 0


if __name__ == '__main__':
    sys.exit(main())
