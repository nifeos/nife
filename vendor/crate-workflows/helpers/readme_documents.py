#!/usr/bin/env python3
"""The two documents a reader meets first, shared by the report-only README checks.

    python3 readme_documents.py --selftest

A reader meets a crate in one of two places: its README (on the repository's page and on
crates.io) and its crate-level doc comment (the `//!` lines of `src/lib.rs`, or of `src/main.rs`
for a program, on docs.rs). Each is read cold, so each is judged on its own. A crate root that
carries `#![doc = include_str!("../README.md")]` shows the README there, and its `//!` lines are
read as part of that same page.

The helpers that import this module (`readme_acronyms.py`, `inflated_wording.py` and
`readme_sections.py`) report and never fail on a finding. They share three things here: which
documents a crate has, the ceiling file (one whole number, read and reported, not enforced), and
the report's total line.

Name: provisional, from the README gates lane.
"""
import re
import sys
from pathlib import Path

INCLUDE = re.compile(r'#!\[doc\s*=\s*include_str!\(\s*"[./]*README[^"]*"\s*\)\s*\]')


class Malformed(Exception):
    """The configuration cannot be read; the message says where."""


def readme_path(crate_dir):
    """The crate's README at its root, or None."""
    found = sorted(p for p in Path(crate_dir).glob('*')
                   if p.is_file() and p.name.upper().startswith('README'))
    return found[0] if found else None


def crate_root(crate_dir):
    """`src/lib.rs`, else `src/main.rs`, else None."""
    for name in ('lib.rs', 'main.rs'):
        path = Path(crate_dir, 'src', name)
        if path.is_file():
            return path
    return None


def crate_doc_markdown(text):
    """The `//!` lines of a crate root as Markdown, each on its own line number, the rest blank."""
    out = []
    for line in text.split('\n'):
        m = re.match(r'\s*//!(?:\s?)(.*)$', line)
        out.append(m.group(1) if m else '')
    return '\n'.join(out)


def documents(crate_dir):
    """[(path, Markdown text)] for the README and the crate-level doc comment, each judged alone.

    A crate root that includes the README adds its own `//!` lines to the README's page rather than
    making a second page; one with no `//!` lines adds nothing."""
    out = []
    readme = readme_path(crate_dir)
    if readme is not None:
        out.append((str(readme), readme.read_text(encoding='utf-8', errors='replace')))
    root = crate_root(crate_dir)
    if root is not None:
        text = root.read_text(encoding='utf-8', errors='replace')
        doc = crate_doc_markdown(text)
        if doc.strip() and not (INCLUDE.search(text) and readme is not None):
            out.append((str(root), doc))
    return out


def read_ceiling(path):
    """The number a ceiling file holds, or None when there is no file."""
    if not Path(path).is_file():
        return None
    lines = [raw.split('#', 1)[0].strip()
             for raw in Path(path).read_text(encoding='utf-8').splitlines()]
    lines = [line for line in lines if line]
    if len(lines) != 1 or not lines[0].isdigit():
        raise Malformed(f'{path}: a ceiling file holds one whole number')
    return int(lines[0])


def total_line(label, count, what, documents_read, ceiling):
    """`label: N <what> in M documents (report only; ...)`."""
    noun = 'document' if documents_read == 1 else 'documents'
    total = f'{label}: {count} {what} in {documents_read} {noun} (report only'
    if ceiling is None:
        return total + '; no ceiling file)'
    if count > ceiling:
        return total + f'; {count - ceiling} over the ceiling of {ceiling}, not enforced)'
    return total + f'; within the ceiling of {ceiling}, not enforced)'


def write_summary(path, title, lines):
    """Append the report to a Markdown job summary: the total, then the findings in a block."""
    with open(path, 'a', encoding='utf-8') as out:
        out.write(f'### {title} (report only)\n\n{lines[-1]}\n\n')
        if lines[:-1]:
            out.write('```\n' + '\n'.join(lines[:-1]) + '\n```\n')


def selftest():
    import tempfile
    failed = []
    with tempfile.TemporaryDirectory() as scratch:
        crate = Path(scratch, 'crate')
        (crate / 'src').mkdir(parents=True)
        (crate / 'README.md').write_text('# crate\n')
        (crate / 'src' / 'lib.rs').write_text('#![no_std]\n//! The crate.\n//!\n//! More.\nfn f() {}\n')
        got = [(Path(p).name, t) for p, t in documents(crate)]
        if got != [('README.md', '# crate\n'), ('lib.rs', '\nThe crate.\n\nMore.\n\n')]:
            failed.append(f'documents gave {got}')
        (crate / 'src' / 'lib.rs').write_text('#![doc = include_str!("../README.md")]\n//! Extra.\n')
        got = [Path(p).name for p, _ in documents(crate)]
        if got != ['README.md']:
            failed.append(f'an included README is one page, gave {got}')
        (crate / 'src' / 'lib.rs').write_text('fn f() {}\n')
        if [Path(p).name for p, _ in documents(crate)] != ['README.md']:
            failed.append('a crate root without `//!` lines adds no document')
        ceiling = Path(scratch, 'ceiling')
        for text, want in (('3\n', 3), ('# why\n0\n', 0)):
            ceiling.write_text(text)
            if read_ceiling(ceiling) != want:
                failed.append(f'ceiling {text!r}')
        for text in ('three\n', '1\n2\n'):
            ceiling.write_text(text)
            try:
                read_ceiling(ceiling)
                failed.append(f'malformed ceiling {text!r} was read')
            except Malformed:
                pass
        if read_ceiling(Path(scratch, 'absent')) is not None:
            failed.append('an absent ceiling file is no ceiling')
    if not total_line('x', 2, 'findings', 1, 1).endswith('1 over the ceiling of 1, not enforced)'):
        failed.append('a total over the ceiling says so')
    if not total_line('x', 0, 'findings', 2, None).endswith('in 2 documents (report only; no ceiling file)'):
        failed.append('a total without a ceiling file says so')
    for message in failed:
        print(f'readme_documents selftest: {message}', file=sys.stderr)
    print('readme_documents selftest: %s' % ('FAILED' if failed else 'ok'))
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(selftest() if '--selftest' in sys.argv else 2)
