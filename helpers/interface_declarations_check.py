#!/usr/bin/env python3
"""A program's `carry_interfaces!` list names exactly the run-time formats it links.

    python3 helpers/interface_declarations_check.py            # judge the tree
    python3 helpers/interface_declarations_check.py --selftest # the rules, as fixtures

Milestone 956 (the linked interfaces carry checkable versions) makes condition 4 of milestone 756
(`process-tools`, `core-tools` and `disk-tools` are the first code to leave this repository) hold:
the moved packages declare what they link. Ruled 2026-11-01 (UTC) on pull request #3022: the
condition covers only the crates that define a run-time format exchanged between separately
released parts. Every other linked crate is a Rust API, which its Cargo version and a semver check
guard, and it is declared nowhere.

A program declares its formats with `capability_manifest_note::carry_interfaces!`, a list written
by hand (the ruling of 2026-10-31 UTC leaves deriving it at package build to the first package that
leaves the tree). What a program links is the workspace crates its source names (`crate::` paths,
the list's own entries aside), closed over their normal dependencies in `cargo metadata`, less the
crates of its own package. Each linked crate is in exactly one of three places: a module of it
carries `pub const INTERFACE_ID` (a format, declared), it is in `OWED` (a format still owed its id
and version), or it is in `NOT_A_FORMAT`.

An interface is the module that holds an `INTERFACE_ID`: the crate itself for `src/lib.rs`, or
`crate::module` for a module file, so one crate may hold one interface per wire format. A program
that links such a crate declares each interface module its source names; naming none of them, it
declares them all. The one table of assignments is the "Interface ids" section of
milestone 956's block (`TABLE`), and a new interface takes the next unused number there.

The check fails when:

1. A linked crate is in none of the three places, or in two.
2. An `INTERFACE_ID` in the tree is not the number the table assigns its interface, two interfaces
   share a number, a retired number returns, or the table assigns one that the tree does not hold.
3. A program of a package in `CUT` carries no `carry_interfaces!` list.
4. A list misses an interface the program links, or names one it does not.
5. An entry's version is not read from the same module as its id, or names a constant that module
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

# The one table of interface ids: a row is `| 6 | `crate` |`, `| 14 | `crate::module` |`, or
# `| 3 | retired, `crate` |` for a number no interface takes again.
TABLE = 'design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md'

# Linked crates that define a run-time format exchanged between separately released parts, still
# owed an `INTERFACE_ID` and a version. A crate leaves this table in the change that gives it both.
OWED = {}

# Linked crates that define no run-time format another released part reads: a Rust API only.
NOT_A_FORMAT = {
    'calendar': 'arithmetic on seconds and dates, with no peer',
    'domain_name_system': 'RFC 1035, a format nife does not own, exchanged with name servers',
    'glob': 'a pattern matcher on two byte strings, with no peer',
    'globally_unique_identifier_partition_table':
        'UEFI\'s on-disk table, whose revision the header carries; nife does not own the format',
}

PATH = r'[a-z_][a-z0-9_]*(?:::[a-z_][a-z0-9_]*)*'
INTERFACE_ID = re.compile(r'^\s*pub\s+const\s+INTERFACE_ID\s*:\s*u64\s*=\s*([^;]+);', re.M)
VERSION_CONST = re.compile(r'^\s*pub\s+const\s+([A-Z0-9_]*VERSION[A-Z0-9_]*)\s*:\s*u32\s*=', re.M)
CARRY = re.compile(r'carry_interfaces!\s*\(\s*&\[(.*?)\]\s*\)', re.S)
DECLARED = re.compile(
    r'Declared\s*\{\s*id\s*:\s*(' + PATH + r')::INTERFACE_ID\s*,'
    r'\s*version\s*:\s*(' + PATH + r')::([A-Z][A-Z0-9_]*)\s*,?\s*\}')
ROW = re.compile(r'^\|\s*(\d+)\s*\|\s*(retired,\s*)?`(' + PATH + r')`\s*\|', re.M)


def strip_comments(text):
    """The source with every `//` comment line removed, so a crate named in prose is not a link."""
    return '\n'.join(line for line in text.splitlines() if not line.lstrip().startswith('//'))


def code_of(source):
    """The source a link is read from: no comments, and the `carry_interfaces!` list's entries
    replaced by the macro's own crate, or a declaration would justify itself."""
    return CARRY.sub('capability_manifest_note::', strip_comments(source))


def named_crates(source, candidates):
    """The crates among `candidates` that `source` names by a `name::` path."""
    code = code_of(source)
    return {c for c in candidates if re.search(r'(?<![A-Za-z0-9_:])' + re.escape(c) + r'::', code)}


def named_modules(source, crate):
    """The `crate::module` paths `source` names, directly or in a `crate::{a, b}` group."""
    code = code_of(source)
    lead = r'(?<![A-Za-z0-9_:])' + re.escape(crate) + r'::'
    out = {f'{crate}::{m}' for m in re.findall(lead + r'([a-z_][a-z0-9_]*)\b(?!\s*\()', code)}
    for group in re.findall(lead + r'\{([^}]*)\}', code):
        out |= {f'{crate}::{m}' for m in re.findall(r'(?<![A-Za-z0-9_:])([a-z_][a-z0-9_]*)\b', group)}
    return out


def required(source, linked, interfaces):
    """The interfaces a program must declare: `interfaces` maps a crate to its interface paths."""
    out = set()
    for crate in linked:
        paths = interfaces.get(crate, ())
        modules = {p for p in paths if p != crate}
        out |= {p for p in paths if p == crate}
        named = modules & named_modules(source, crate)
        out |= named or modules
    return out


def declarations(source):
    """The `(id path, version path, version constant)` entries of the one list, or None."""
    lists = CARRY.findall(strip_comments(source))
    if len(lists) != 1:
        return None if not lists else 'many'
    return DECLARED.findall(lists[0])


def problems(program, source, needed, versions):
    """What is wrong with one program's list. `needed` is the set of interface paths it must
    declare; `versions` maps each interface path to its public `u32` version constants."""
    found = declarations(source)
    if found is None:
        return [f'{program}: carries no carry_interfaces! list, and it links {sorted(needed)}']
    if found == 'many':
        return [f'{program}: carries more than one carry_interfaces! list']
    out = []
    named = {path for path, _, _ in found}
    for path in sorted(needed - named):
        out.append(f'{program}: links {path} but does not declare it')
    for path in sorted(named - needed):
        out.append(f'{program}: declares {path} but does not link it')
    for path, vpath, vconst in found:
        if vpath != path:
            out.append(f'{program}: the entry for {path} reads its version from {vpath}')
        elif vconst not in versions.get(path, ()):
            out.append(f'{program}: {path}::{vconst} is not a public u32 version of {path}')
    return out


def classification_problems(linked, with_id):
    """Each crate in `linked` that is not in exactly one of: carries an id, `OWED`,
    `NOT_A_FORMAT`."""
    out = []
    for crate in sorted(linked):
        places = [crate in with_id, crate in OWED, crate in NOT_A_FORMAT]
        if sum(places) == 0:
            out.append(f'{crate}: linked by the first cut and not classified; give it an '
                       'INTERFACE_ID, or add it to OWED or NOT_A_FORMAT with its reason')
        elif sum(places) > 1:
            out.append(f'{crate}: classified twice (an id, OWED, NOT_A_FORMAT: {places})')
    return out


def table_rows(text):
    """The table's rows, as `(number, path, retired)`, in the order the table gives them."""
    return [(int(n), path, bool(retired)) for n, retired, path in ROW.findall(text)]


def id_problems(found, rows):
    """`found` maps each interface path in the tree to its `INTERFACE_ID` text; `rows` is the
    table. Every number names one interface, once, and the tree and the table agree."""
    out = []
    numbers = [n for n, _, _ in rows]
    for n in sorted({n for n in numbers if numbers.count(n) > 1}):
        out.append(f'the table assigns {n} more than once')
    assigned = {path: n for n, path, retired in rows if not retired}
    retired = {n: path for n, path, gone in rows if gone}
    owners = {}
    for path, value in sorted(found.items()):
        text = ' '.join(value.split())
        owners.setdefault(text, []).append(path)
        if not text.isdigit():
            out.append(f'{path}: INTERFACE_ID is {text}, not a literal number')
        elif int(text) in retired:
            out.append(f'{path}: INTERFACE_ID {text} is retired (it named {retired[int(text)]})')
        elif path not in assigned:
            out.append(f'{path}: INTERFACE_ID {text} has no row in {TABLE}; take the next unused '
                       'number there')
        elif assigned[path] != int(text):
            out.append(f'{path}: INTERFACE_ID {text}, but the table assigns {assigned[path]}')
    for text, paths in sorted(owners.items()):
        if len(paths) > 1:
            out.append(f'INTERFACE_ID {text} is declared by {", ".join(paths)}')
    for path, n in sorted(assigned.items()):
        if path not in found:
            out.append(f'the table assigns {n} to {path}, which carries no INTERFACE_ID')
    return out


def module_path(crate, src, file):
    """`crate` for `src/lib.rs`, `crate::a::b` for `src/a/b.rs` or `src/a/b/mod.rs`."""
    parts = os.path.relpath(file, src)[:-3].split(os.sep)
    if parts[-1] in ('lib', 'mod'):
        parts = parts[:-1]
    return '::'.join([crate] + parts)


def crate_interfaces(crate, src):
    """`{path: (id text, versions)}` for every non-test module file under `src`."""
    out = {}
    for root, dirs, files in os.walk(src):
        dirs[:] = [d for d in dirs if d != 'tests']
        for name in files:
            if not name.endswith('.rs') or name == 'tests.rs':
                continue
            file = os.path.join(root, name)
            text = open(file, encoding='utf-8').read().split('#[cfg(test)]', 1)[0]
            ids = INTERFACE_ID.findall(text)
            if ids:
                out[module_path(crate, src, file)] = (ids[0], set(VERSION_CONST.findall(text)))
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

    found, versions, interfaces = {}, {}, {}
    for name, i in by_name.items():
        src = os.path.join(os.path.dirname(package[i]['manifest_path']), 'src')
        if os.path.isdir(src):
            for path, (value, consts) in crate_interfaces(name, src).items():
                found[path] = value
                versions[path] = consts
                interfaces.setdefault(name, set()).add(path)

    out = id_problems(found, table_rows(open(TABLE, encoding='utf-8').read()))

    bins = {}
    for i in members:
        for t in package[i]['targets']:
            if 'bin' in t['kind']:
                bins.setdefault(t['name'], t['src_path'])

    reached = set()
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
            reached |= linked
            out += problems(program, source, required(source, linked, interfaces), versions)
    return classification_problems(reached, set(interfaces)) + out


def selftest():
    versions = {'abi': {'ABI_VERSION'}, 'clock': {'VERSION'}, 'fsc::blk': {'VERSION'},
                'fsc::fs': {'VERSION'}}

    def entry(path, vpath=None, vconst='VERSION'):
        return (f'capability_manifest_note::Declared {{ id: {path}::INTERFACE_ID, '
                f'version: {vpath or path}::{vconst} }},')

    def carry(*entries):
        return 'capability_manifest_note::carry_interfaces!(&[\n' + '\n'.join(entries) + '\n]);\n'

    good = carry(entry('abi', vconst='ABI_VERSION'), entry('clock'))
    cases = [
        ('a complete list passes', good, {'abi', 'clock'}, []),
        ('no list fails', 'fn main() {}\n', {'abi'}, ['carries no carry_interfaces!']),
        ('two lists fail', good + good, {'abi', 'clock'}, ['more than one']),
        ('a missing interface fails', carry(entry('abi', vconst='ABI_VERSION')), {'abi', 'clock'},
         ['links clock but does not declare it']),
        ('an extra interface fails', good, {'abi'}, ['declares clock but does not link it']),
        ('a version from another module fails',
         carry(entry('abi', vconst='ABI_VERSION'), entry('clock', 'abi', 'ABI_VERSION')),
         {'abi', 'clock'}, ['reads its version from abi']),
        ('an unknown version constant fails', carry(entry('abi', vconst='VERSION'), entry('clock')),
         {'abi', 'clock'}, ['abi::VERSION is not a public u32 version']),
        ('a list in a comment does not count', '// ' + good.replace('\n', '\n// '), {'abi'},
         ['carries no carry_interfaces!']),
        ('a module interface passes', carry(entry('fsc::fs')), {'fsc::fs'}, []),
        ('a module version from its sibling fails', carry(entry('fsc::fs', 'fsc::blk')),
         {'fsc::fs'}, ['reads its version from fsc::blk']),
    ]
    for what, source, needed, expect in cases:
        got = problems('p', source, needed, versions)
        if not expect:
            assert got == [], f'{what}: {got}'
        else:
            assert len(got) == len(expect) and all(e in g for e, g in zip(expect, got)), \
                f'{what}: {got}'

    assert named_crates('use abi::x;\n// glob::y\nlet z = grant_plan::P;', {'abi', 'glob', 'grant_plan'}) \
        == {'abi', 'grant_plan'}
    assert named_crates('foo_abi::x; self::abi::y', {'abi'}) == set()
    assert named_crates(good, {'abi', 'clock', 'capability_manifest_note'}) \
        == {'capability_manifest_note'}

    interfaces = {'fsc': {'fsc::blk', 'fsc::fs'}, 'abi': {'abi'}}
    assert required('use fsc::{dir, fs, grant};', {'fsc', 'abi'}, interfaces) == {'fsc::fs', 'abi'}
    assert required('use fsc::blk;\nfsc::req::x();', {'fsc'}, interfaces) == {'fsc::blk'}
    assert required('use other::x;', {'fsc'}, interfaces) == {'fsc::blk', 'fsc::fs'}
    assert required('fsc::helper(1);', {'fsc'}, interfaces) == {'fsc::blk', 'fsc::fs'}

    assert classification_problems({'abi', 'calendar'}, {'abi'}) == []
    assert 'not classified' in classification_problems({'unknown_crate'}, set())[0]
    assert 'classified twice' in classification_problems({'calendar'}, {'calendar'})[0]

    rows = table_rows('| id | interface |\n|---|---|\n| 1 | `abi` |\n| 3 | retired, `glob` |\n'
                      '| 14 | `fsc::blk` |\n| 15 | `fsc::fs` |\n')
    assert rows == [(1, 'abi', False), (3, 'glob', True), (14, 'fsc::blk', False),
                    (15, 'fsc::fs', False)]
    agree = {'abi': '1', 'fsc::blk': '14', 'fsc::fs': '15'}
    assert id_problems(agree, rows) == []

    def fires(what, found, rows, expect):
        got = id_problems(found, rows)
        assert any(expect in g for g in got), f'{what}: {got}'

    fires('two interfaces sharing a number fail', {**agree, 'clock': '1'},
          rows + [(1, 'clock', False)], 'declared by abi, clock')
    fires('a duplicate the table does not list still fails', {**agree, 'fsc::fs': '14'},
          [r for r in rows if r[1] != 'fsc::fs'], 'declared by fsc::blk, fsc::fs')
    fires('a retired number fails', {**agree, 'clock': '3'}, rows, 'is retired')
    fires('a number the table does not record fails', {**agree, 'clock': '16'}, rows,
          'has no row')
    fires('a number other than the table\'s fails', {**agree, 'fsc::fs': '16'}, rows,
          'the table assigns 15')
    fires('a table row the tree lacks fails', {'abi': '1', 'fsc::blk': '14'}, rows,
          'carries no INTERFACE_ID')
    fires('a number the table assigns twice fails', agree, rows + [(15, 'clock', False)],
          'assigns 15 more than once')
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
    print('interface declarations: every interface id matches the table, every crate the first '
          'cut links is classified, and every program declares exactly the interfaces it links')
    return 0


if __name__ == '__main__':
    sys.exit(main())
