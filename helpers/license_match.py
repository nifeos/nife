#!/usr/bin/env python3
"""A crate's manifest `license` field matches the licenses its sources adapt.

The rule is in milestone 887 (crate repositories share one CI template, adapted from nife's gates)
and was ruled 2026-11-05 (UTC): the field is the crate's own `MIT OR Apache-2.0` joined with `AND`
to the SPDX identifier of every license of source the crate adapts, as in
`(MIT OR Apache-2.0) AND ISC`. Each adapted file's notice opens with an `SPDX-License-Identifier:`
line, and this check reads those lines from the crate's text files and compares the set with the
`AND` terms of the field, both ways. A term with no notice and a notice with no term each fail,
naming the crate and the file or the term.

Usage: helpers/license_match.py [--selftest] [crates-dir]
"""
import re
import sys
import tomllib
from pathlib import Path

OWN = 'MIT OR Apache-2.0'
OWN_IDS = {'MIT', 'Apache-2.0'}
TEXT = ('.rs', '.md', '.toml', '.txt')
MARKER = re.compile(r'SPDX-License-Identifier:([^`\n]*)')
ID = re.compile(r'[A-Za-z0-9][A-Za-z0-9.+-]*')
OPERATORS = {'AND', 'OR', 'WITH'}


def ids_in(text):
    """The license identifiers named on `SPDX-License-Identifier:` lines in `text`."""
    found = set()
    for m in MARKER.finditer(text):
        found |= {t for t in ID.findall(m.group(1)) if t not in OPERATORS}
    return found


def carried(crate_dir):
    """{identifier: first file naming it} over the crate's text files, minus the crate's own."""
    out = {}
    for path in sorted(crate_dir.rglob('*')):
        parts = path.relative_to(crate_dir).parts
        if (not path.is_file() or path.suffix not in TEXT or parts[0] in ('target', 'fuzz')
                or path.name.startswith('LICENSE')):
            continue
        for ident in sorted(ids_in(path.read_text(errors='replace')) - OWN_IDS):
            out.setdefault(ident, str(path.relative_to(crate_dir)))
    return out


def field_of(manifest, workspace_license):
    lic = manifest.get('package', {}).get('license')
    if isinstance(lic, dict) and lic.get('workspace'):
        return workspace_license
    return lic


def terms_of(field):
    """The adapted-license terms of `field`, or None when it is not in the ruled form."""
    if field == OWN:
        return set()
    head, sep, rest = (field or '').partition(f'({OWN}) AND ')
    if head or not sep:
        return None
    return set(rest.split(' AND '))


def findings(crates_dir, workspace_license):
    """[message] for every crate whose field and carried notices disagree."""
    bad = []
    for manifest_path in sorted(crates_dir.glob('*/Cargo.toml')):
        crate = manifest_path.parent
        field = field_of(tomllib.loads(manifest_path.read_text()), workspace_license)
        terms = terms_of(field)
        if terms is None:
            bad.append(f'{crate.name}: license field {field!r} is not {OWN!r} or '
                       f'({OWN}) AND <adapted licenses>')
            continue
        notices = carried(crate)
        for ident, where in notices.items():
            if ident not in terms:
                bad.append(f'{crate.name}: {where} carries {ident}, which the license field '
                           f'{field!r} does not name')
        for term in sorted(terms - set(notices)):
            bad.append(f'{crate.name}: license field names {term}, but no file carries an '
                       f'SPDX-License-Identifier: {term} notice')
    return bad


def workspace_license(root):
    ws = tomllib.loads((root / 'Cargo.toml').read_text())
    return ws.get('workspace', {}).get('package', {}).get('license')


def _crate(base, name, field, files):
    d = base / 'crates' / name
    (d / 'src').mkdir(parents=True)
    (d / 'Cargo.toml').write_text(f'[package]\nname = "{name}"\nlicense = "{field}"\n')
    for rel, text in files.items():
        (d / rel).write_text(text)


def selftest():
    import tempfile
    pair = OWN
    isc = f'({OWN}) AND ISC'
    notice = '//! ```text\n//! SPDX-License-Identifier: ISC\n//!\n//! Copyright (c) someone\n//! ```\n'
    with tempfile.TemporaryDirectory() as tmp:
        base = Path(tmp)
        _crate(base, 'plain', pair, {'src/lib.rs': '//! nothing adapted\n'})
        _crate(base, 'adapts', isc, {'src/lib.rs': notice})
        _crate(base, 'own_marker', pair, {'src/lib.rs': '//! SPDX-License-Identifier: MIT OR Apache-2.0\n'})
        _crate(base, 'prose_marker', f'({OWN}) AND BSD-2-Clause AND BSD-3-Clause',
               {'src/lib.rs': '//! are `SPDX-License-Identifier: BSD-2-Clause AND BSD-3-Clause`, with the\n'})
        assert findings(base / 'crates', None) == [], findings(base / 'crates', None)
    cases = {
        'notice missing from the field': ('adapts_unnamed', pair, {'src/lib.rs': notice}, 'carries ISC'),
        'term with no notice': ('named_unused', isc, {'src/lib.rs': '//! plain\n'}, 'names ISC'),
        'wrong term': ('wrong', f'({OWN}) AND BSD-2-Clause', {'src/lib.rs': notice}, 'carries ISC'),
        'field not in the ruled form': ('shape', 'MIT', {'src/lib.rs': ''}, 'is not'),
        'notice only in a note': ('note', pair, {'src/lib.rs': '', 'NOTES.md': 'SPDX-License-Identifier: ISC\n'}, 'NOTES.md carries ISC'),
    }
    for label, (name, field, files, want) in cases.items():
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            _crate(base, name, field, files)
            got = findings(base / 'crates', None)
            assert got and want in got[0], (label, got)
            if label != 'wrong term':
                assert len(got) == 1, (label, got)
    with tempfile.TemporaryDirectory() as tmp:
        base = Path(tmp)
        _crate(base, 'inherits', '', {'src/lib.rs': notice})
        (base / 'crates/inherits/Cargo.toml').write_text(
            '[package]\nname = "inherits"\nlicense.workspace = true\n')
        assert len(findings(base / 'crates', pair)) == 1
        assert findings(base / 'crates', isc) == []
    print('license_match selftest: ok')


def main(argv):
    if '--selftest' in argv:
        selftest()
        return 0
    args = [a for a in argv if not a.startswith('-')]
    root = Path(args[0]) if args else Path('.')
    bad = findings(root / 'crates', workspace_license(root))
    for line in bad:
        print(f'license-match: {line}', file=sys.stderr)
    if bad:
        print("license-match: a manifest's license is the crate's own `MIT OR Apache-2.0` joined "
              'with AND to the license of every source it adapts (milestone 887); each adapted '
              "notice opens with an SPDX-License-Identifier: line.", file=sys.stderr)
        return 1
    n = len(list((root / 'crates').glob('*/Cargo.toml')))
    print(f'license-match: all {n} crate manifests match the licenses their sources carry')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
