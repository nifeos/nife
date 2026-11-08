#!/usr/bin/env python3
"""Report rhetorical or inflated wording in a README or a crate-level doc comment.

    python3 inflated_wording.py [--crate-dir DIR] [--ceiling .github/inflated-wording-ceiling]
                                [--summary FILE]
    python3 inflated_wording.py --selftest

A word that claims quality ("robust") asks the reader to trust what the text does not show, and a
rhetorical figure makes the reader decode a sentence before the fact arrives. Each entry in
`inflated_wording.toml` is one such phrase with the plain text to write instead. The list is data
so that it grows; `prose_figures.py` fails on its own, shorter list, and an entry here moves there
once no crate uses it.

**What it reads.** The documents `readme_documents.py` names: the README and the crate root's `//!`
lines. Headings are read. Inline code, fenced code, addresses and HTML comments are not. The lines
of one paragraph are read as one text, so a phrase broken across a line still matches.

**The list.** `[[wording]]` entries with `phrase`, a regular expression matched in any case
between word boundaries, and `instead`. An entry without both is a malformed configuration.

**What it prints.** One line per finding, `file:line: "phrase as written": write <instead>`, then a
total. `--summary FILE` appends the report as Markdown, for `$GITHUB_STEP_SUMMARY`.

**Report only.** It exits 0 whatever it finds, and 2 on a malformed list or a ceiling file that does
not hold one whole number. The ceiling is read and reported, not enforced.

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

LABEL = 'inflated_wording'
CEILING_FILE = '.github/inflated-wording-ceiling'
WORDING = Path(__file__).resolve().with_name('inflated_wording.toml')


def load_wording(path=WORDING):
    """[(compiled pattern, instead)] from the list; Malformed when an entry lacks a part."""
    try:
        entries = tomllib.loads(Path(path).read_text(encoding='utf-8')).get('wording', [])
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise readme_documents.Malformed(f'{path}: {error}') from error
    out = []
    for number, entry in enumerate(entries, 1):
        phrase, instead = entry.get('phrase'), entry.get('instead')
        if not isinstance(phrase, str) or not phrase or not isinstance(instead, str) or not instead:
            raise readme_documents.Malformed(f'{path}: entry {number} needs a phrase and an instead')
        try:
            pattern = re.compile(r'\b(?:%s)\b' % phrase.replace(' ', r'\s+'), re.I)
        except re.error as error:
            raise readme_documents.Malformed(f'{path}: entry {number}: {error}') from error
        out.append((pattern, instead))
    return out


def findings(text, wording):
    """[(line, phrase as written, instead)] in reading order."""
    stream, marks, last = '', [], None
    for n, line, heading in markdown_lines(text):
        if last is not None and (n != last + 1 or heading):
            stream += '\n\n'
        marks.append((len(stream), n))
        stream += line + ('\n\n' if heading else '\n')
        last = n
    out = []
    for pattern, instead in wording:
        for m in pattern.finditer(stream):
            if '\n\n' in m.group(0):
                continue
            line = [n for off, n in marks if off <= m.start()][-1]
            out.append((line, ' '.join(m.group(0).split()), instead))
    return sorted(out)


def run(argv):
    parser = argparse.ArgumentParser(prog='inflated_wording.py')
    parser.add_argument('--crate-dir', default='.')
    parser.add_argument('--wording', default=str(WORDING))
    parser.add_argument('--ceiling', default=CEILING_FILE)
    parser.add_argument('--summary')
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args(argv)
    if args.selftest:
        return selftest()
    try:
        wording = load_wording(args.wording)
        ceiling = readme_documents.read_ceiling(args.ceiling)
    except readme_documents.Malformed as error:
        print(f'{LABEL}: malformed configuration: {error}', file=sys.stderr)
        return 2
    docs = readme_documents.documents(args.crate_dir)
    lines = [f'{path}:{line}: "{phrase}": write {instead}'
             for path, text in docs for line, phrase, instead in findings(text, wording)]
    lines.append(readme_documents.total_line(LABEL, len(lines), 'inflated phrases', len(docs), ceiling))
    print('\n'.join(lines))
    if args.summary:
        readme_documents.write_summary(args.summary, 'Inflated wording', lines)
    return 0


def selftest():
    import contextlib
    import io
    import tempfile
    failed = []
    with tempfile.TemporaryDirectory() as scratch:
        listed = Path(scratch, 'wording.toml')
        listed.write_text('[[wording]]\nphrase = "robust(ly)?"\ninstead = "say what it handles"\n'
                          '[[wording]]\nphrase = "it is worth noting"\ninstead = "drop it"\n'
                          '[[wording]]\nphrase = "owes? (the|a) (\\\\w+ )?nothing"\ninstead = "plain"\n')
        wording = load_wording(listed)
        cases = [
            ('A robust parser, robustly built.\n', [(1, 'robust'), (1, 'robustly')]),
            ('It is worth\nnoting that it parses.\n', [(1, 'It is worth noting')]),
            ('The crate owes the driver nothing.\n', [(1, 'owes the driver nothing')]),
            ('A `robust` span and\n```\nrobust\n```\n', []),
            ('# A robust heading\n\nPlain.\n', [(1, 'robust')]),
            ('Robustness is another word here.\n', []),
            ('It is worth\n\nnoting across paragraphs.\n', []),
        ]
        for src, want in cases:
            got = [(n, p) for n, p, _ in findings(src, wording)]
            if got != want:
                failed.append(f'{src!r} gave {got}, wanted {want}')
        try:
            load_wording()
        except readme_documents.Malformed as error:
            failed.append(f'the shipped list is malformed: {error}')
        bad = Path(scratch, 'bad.toml')
        bad.write_text('[[wording]]\nphrase = "robust"\n')
        none = str(Path(scratch, 'absent'))
        with contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()):
            if run(['--wording', str(bad), '--ceiling', none]) != 2:
                failed.append('an entry without an instead is not refused')
            crate = Path(scratch, 'crate')
            (crate / 'src').mkdir(parents=True)
            (crate / 'README.md').write_text('# crate\n\nA robust crate.\n')
            (crate / 'src' / 'lib.rs').write_text('//! A robust crate.\n')
            ceiling = Path(scratch, 'ceiling')
            ceiling.write_text('0\n')
            code = run(['--crate-dir', str(crate), '--wording', str(listed), '--ceiling', str(ceiling)])
            if code != 0 or 'over the ceiling of 0' not in out.getvalue() or out.getvalue().count('"robust"') != 2:
                failed.append(f'findings over the ceiling are reported and never fail: {code}, {out.getvalue()!r}')
    for message in failed:
        print(f'{LABEL} selftest: FAIL {message}', file=sys.stderr)
    print(f'{LABEL} selftest: %s' % ('FAILED' if failed else 'ok'))
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(run(sys.argv[1:]))
