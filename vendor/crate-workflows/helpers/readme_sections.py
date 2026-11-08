#!/usr/bin/env python3
"""Report a README whose sections are not in the one order every crate shares.

    python3 readme_sections.py [--crate-dir DIR] [--ceiling .github/readme-sections-ceiling]
                               [--summary FILE]
    python3 readme_sections.py --selftest

A reader who has read one crate's README knows where to look in the next one. The order:

| # | section | |
|---|---|---|
| 1 | `# <package name>`, then a summary paragraph | required |
| 2 | `## What it does`, or `## The contract` for a crate that holds one | optional |
| 3 | `## EXAMPLES` | required |
| 4 | `## What is proven` | required |
| 5 | any section of the crate's own (`## The two device trees`, `## Hardware evidence`) | optional |
| 6 | `## BUGS` | required |
| 7 | `## How it differs from <other crates>` | optional |
| 8 | `## Minimum Rust version` | required |
| 9 | `## License` | required |

**What it reads.** The README at the crate root, its `#` and `##` headings only (`###` and deeper
belong to the section above them); headings inside fenced code are not headings. The title is the
package name from the crate's `Cargo.toml`, as plain text.

**What it reports.** A title that is not the package name, no summary before the first section, a
required section that is missing, a section that comes before one it follows in the table, a
section listed twice, and a known section under another spelling (`## Usage` or `## Example` for
`## EXAMPLES`, `## Limitations` for `## BUGS`), which is judged in its canonical place.

**Report only.** It exits 0 whatever it finds, and 2 on a ceiling file that does not hold one whole
number. The ceiling is read and reported, not enforced. `standalone.py` already fails a README
without `## EXAMPLES` or `## BUGS`.

Name: provisional, from the README gates lane.
"""
import argparse
import re
import sys
import tomllib
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import readme_documents  # noqa: E402
from prose_text import markdown_lines  # noqa: E402

LABEL = 'readme_sections'
CEILING_FILE = '.github/readme-sections-ceiling'
OWN = 'a section of the crate\'s own'
# (slot, canonical heading, required, other spellings)
ORDER = [
    (2, 'What it does', False, ()),
    (2, 'The contract', False, ()),
    (3, 'EXAMPLES', True, ('Example', 'Examples', 'Usage')),
    (4, 'What is proven', True, ()),
    (6, 'BUGS', True, ('Limitations', 'Known limitations')),
    (7, 'How it differs from', False, ()),
    (8, 'Minimum Rust version', True, ('MSRV', 'Minimum supported Rust version')),
    (9, 'License', True, ('Licensing',)),
]
OWN_SLOT = 5


def classify(heading):
    """(slot, canonical heading, other spelling or None) for one `##` heading's text."""
    for slot, canonical, _, others in ORDER:
        if canonical == 'How it differs from':
            if heading.startswith(canonical + ' '):
                return slot, heading, None
            continue
        if heading == canonical:
            return slot, canonical, None
        if heading.lower() == canonical.lower() or heading.lower() in (o.lower() for o in others):
            return slot, canonical, heading
    return OWN_SLOT, heading, None


def package_name(crate_dir):
    try:
        manifest = tomllib.loads(Path(crate_dir, 'Cargo.toml').read_text(encoding='utf-8'))
    except (OSError, tomllib.TOMLDecodeError):
        return None
    return manifest.get('package', {}).get('name')


def findings(text, name=None):
    """[(line, message)] for one README."""
    lines = markdown_lines(text)
    out = []
    titles = [(n, t) for n, t, h in lines if h and re.match(r'# ', t)]
    if not lines or not (lines[0][2] and lines[0][1].startswith('# ')):
        out.append((1, 'the README does not open with a `# <package name>` title'))
    else:
        title = text.split('\n')[lines[0][0] - 1][2:].strip()
        if name and title != name:
            out.append((lines[0][0], f'the title is {title!r}; write the package name, `# {name}`, as plain text'))
    for n, t in titles[1:]:
        out.append((n, f'a second `#` title, {t[2:].strip()!r}; a section is a `##` heading'))
    sections = [(n, t[3:].strip()) for n, t, h in lines if h and t.startswith('## ')]
    first = sections[0][0] if sections else None
    if not any(not h and (first is None or n < first) for n, _, h in lines):
        out.append((lines[0][0] if lines else 1, 'no summary paragraph before the first section'))
    seen, latest = {}, (0, None)
    for n, heading in sections:
        slot, canonical, spelled = classify(heading)
        if spelled:
            out.append((n, f'`## {spelled}` is spelled `## {canonical}` in every crate'))
        key = canonical if slot != OWN_SLOT else (OWN_SLOT, canonical)
        if key in seen:
            out.append((n, f'`## {canonical}` appears twice (first at line {seen[key]})'))
        seen.setdefault(key, n)
        if slot < latest[0]:
            what = OWN if slot == OWN_SLOT else f'`## {canonical}`'
            if latest[0] == OWN_SLOT:
                out.append((n, f'{what} comes after `## {latest[1]}`, {OWN}; that goes after '
                               '`## What is proven` and before `## BUGS`'))
            else:
                where = 'after `## What is proven` and before `## BUGS`' if slot == OWN_SLOT else 'earlier'
                out.append((n, f'{what} ({heading!r}) comes after `## {latest[1]}`; it goes {where}'))
        else:
            latest = (slot, canonical)
    present = {classify(h)[0] for _, h in sections}
    for slot, canonical, required, _ in ORDER:
        if required and slot not in present:
            out.append((first or 1, f'no `## {canonical}` section'))
    return sorted(out)


def run(argv):
    parser = argparse.ArgumentParser(prog='readme_sections.py')
    parser.add_argument('--crate-dir', default='.')
    parser.add_argument('--ceiling', default=CEILING_FILE)
    parser.add_argument('--summary')
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args(argv)
    if args.selftest:
        return selftest()
    try:
        ceiling = readme_documents.read_ceiling(args.ceiling)
    except readme_documents.Malformed as error:
        print(f'{LABEL}: malformed configuration: {error}', file=sys.stderr)
        return 2
    readme = readme_documents.readme_path(args.crate_dir)
    lines, read = [], 0
    if readme is not None:
        read = 1
        text = readme.read_text(encoding='utf-8', errors='replace')
        lines = [f'{readme}:{n}: {message}' for n, message in findings(text, package_name(args.crate_dir))]
    lines.append(readme_documents.total_line(LABEL, len(lines), 'section-order findings', read, ceiling))
    print('\n'.join(lines))
    if args.summary:
        readme_documents.write_summary(args.summary, 'README section order', lines)
    return 0


GOOD = '''# example

One line.

## What it does

Text.

## EXAMPLES

```rust
// ## not a heading
```

## What is proven

Text.

### A subsection

## The two device trees

Text.

## BUGS

- None.

## How it differs from other_crate

Text.

## Minimum Rust version

Rust 1.88.

## License

Text.
'''


def selftest():
    import contextlib
    import io
    import tempfile
    failed = []

    def expect(label, text, want, name='example'):
        got = [message for _, message in findings(text, name)]
        if len(got) != len(want) or not all(any(w in g for g in got) for w in want):
            failed.append(f'{label}: gave {got}, wanted {want}')

    expect('the canonical order', GOOD, [])
    expect('the minimum shape', '# example\n\nOne line.\n\n## EXAMPLES\n\n## What is proven\n\n'
           '## BUGS\n\n## Minimum Rust version\n\n## License\n', [])
    swapped = GOOD.replace('## EXAMPLES', '## TEMP').replace('## What is proven', '## EXAMPLES') \
        .replace('## TEMP', '## What is proven')
    expect('proofs before examples', swapped, ["`## EXAMPLES` ('EXAMPLES') comes after `## What is proven`"])
    expect('a section of its own before the proofs',
           GOOD.replace('## What is proven', '## Build\n\nText.\n\n## What is proven'),
           ["`## What is proven` comes after `## Build`, a section of the crate's own"])
    expect('other spellings', GOOD.replace('## EXAMPLES', '## Usage').replace('## BUGS', '## Limitations'),
           ['`## Usage` is spelled `## EXAMPLES`', '`## Limitations` is spelled `## BUGS`'])
    expect('missing sections', '# example\n\nOne line.\n\n## EXAMPLES\n\n## BUGS\n',
           ['no `## What is proven`', 'no `## Minimum Rust version`', 'no `## License`'])
    expect('a title in code', GOOD.replace('# example', '# `example`'), ["the title is '`example`'"])
    expect('no summary', GOOD.replace('One line.\n', ''), ['no summary paragraph'])
    expect('a section twice', GOOD + '\n## BUGS\n\nMore.\n',
           ['`## BUGS` appears twice', "`## BUGS` ('BUGS') comes after `## License`"])
    with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stdout(io.StringIO()) as out, \
            contextlib.redirect_stderr(io.StringIO()):
        crate = Path(scratch)
        (crate / 'Cargo.toml').write_text('[package]\nname = "other"\n')
        (crate / 'README.md').write_text(GOOD)
        ceiling = crate / 'ceiling'
        ceiling.write_text('0\n')
        code = run(['--crate-dir', str(crate), '--ceiling', str(ceiling)])
        if code != 0 or "write the package name, `# other`" not in out.getvalue():
            failed.append(f'the package name is read from Cargo.toml, and a finding never fails: {code}')
        ceiling.write_text('x\n')
        if run(['--crate-dir', str(crate), '--ceiling', str(ceiling)]) != 2:
            failed.append('a malformed ceiling file is not refused')
    for message in failed:
        print(f'{LABEL} selftest: FAIL {message}', file=sys.stderr)
    print(f'{LABEL} selftest: %s' % ('FAILED' if failed else 'ok'))
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(run(sys.argv[1:]))
