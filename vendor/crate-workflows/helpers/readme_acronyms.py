#!/usr/bin/env python3
"""Report an acronym that a README or a crate-level doc comment uses before the text defines it.

    python3 readme_acronyms.py [--crate-dir DIR] [--accepted .github/accepted-words]
                               [--ceiling .github/readme-acronyms-ceiling] [--summary FILE]
    python3 readme_acronyms.py --selftest

A reader opens the README, or the crate's front page on docs.rs, and meets each acronym in reading
order. The first use of each one is defined there or earlier, or the reader is left to guess.
`prose_acronyms.py` already fails a crate whose files leave an acronym bare; this check reads the
two front pages more closely, and reports.

**What it reads.** The documents `readme_documents.py` names: the README, and the crate root's `//!`
lines as a page of their own (unless the root includes the README). Each is judged alone, in reading
order, headings included. Inline code, fenced code, addresses and HTML comments are not read.

**What is a token.** Everything `prose_acronyms.py` counts (two or more capitals, then letters or
digits), and two forms it does not: capitals after a lowercase prefix (`exFAT`, `eMMC`) and single
capitals joined by slashes (`I/O`, read as `IO`). Its exemptions hold here too: a plural `s`, hex,
capitals used as emphasis or as a label, and a width (`FAT32`) once its acronym (`FAT`) is defined.

**What is not an acronym.** A unit (`MHz`, `GHz`), a part number (capitals, then four digits or
more: `YT8531`), and a word of five letters or more in capitals that is an English word in
`english_words.txt` (`IDENTIFY`, a command's name in its specification).

**What defines a token.** Any one of these, at or before the first use:

- the parenthesized forms `prose_acronyms.py` accepts, at the first use: `File Allocation Table
  (FAT)` or `FAT (File Allocation Table)`;
- the words in the same sentence as the first use, in prose: `FAT, the File Allocation Table,` or
  `the file allocation table, which FAT names`. The initials of a run of words spell the whole
  token, small words skipped, as `prose_acronyms.py` counts them;
- after the token, in parentheses, words its letters are cut from in order, the first letter first
  (`AON (always-on)`);
- the words in Title Case anywhere earlier in the document (`the File Allocation Table ... FAT`);
- the token is accepted: `allow` or `words` in `prose_acronyms.toml`, an `[accepted]` word in
  `naming_words.toml` (`pci`, `usb`, `efi`), or a word in the repository's `.github/accepted-words`.

**What it prints.** One line per finding, `file:line: TOKEN is used before the text defines it`,
with `(defined later, at line N)` when a definition comes after the first use. Then a total.
`--summary FILE` appends the report as Markdown, for `$GITHUB_STEP_SUMMARY`.

**Report only.** It exits 0 whatever it finds, and 2 on a malformed configuration (an accepted-words
line without a reason, a ceiling file that does not hold one whole number). The ceiling file is read
and the total says whether it is over; nothing enforces it yet.

Name: provisional, from the README gates lane.
"""
import argparse
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import abbreviations  # noqa: E402
import prose_acronyms  # noqa: E402
import readme_documents  # noqa: E402
import rust_names  # noqa: E402
from prose_text import markdown_lines  # noqa: E402

LABEL = 'readme_acronyms'
CEILING_FILE = '.github/readme-acronyms-ceiling'
ACCEPTED_FILE = '.github/accepted-words'
TOKEN = re.compile(r'\b[a-z]*[A-Z]{2,}[A-Za-z0-9]*\b|\b[A-Z](?:/[A-Z])+\b')
WORD = re.compile(r"[A-Za-z][A-Za-z0-9'-]*")
SENTENCE_END = re.compile(r'[.!?](?=\s+[A-Z`*\[(])')


def canonical(token):
    """The token as the lists hold it: no slashes, no plural `s`."""
    return prose_acronyms.normal(token.replace('/', ''))


def letters(token):
    """The letters the expansion's initials spell, in lowercase."""
    return re.sub(r'[^A-Za-z]', '', token).lower()


def word_initials(word):
    """What one word gives: each part of a hyphenated word, each capital of a CamelCase one."""
    return prose_acronyms.initials([w for w in word.split('-') if w])


def spelled_runs(token, words, title_case=False):
    """Does a run of `words` spell exactly the token's letters, small words skipped?"""
    want = letters(token)
    for start in range(len(words)):
        if words[start].lower() in prose_acronyms.STOP:
            continue
        got = ''
        for w in words[start:]:
            if w.lower() in prose_acronyms.STOP:
                continue
            if title_case and not w[0].isupper():
                break
            if TOKEN.search(w):
                break  # an acronym is not part of a prose expansion (`FAT16's` spells no FAT)
            got += word_initials(w)
            if len(got) >= len(want):
                break
        if got == want and len(want) >= 2:
            return True
    return False


class Page:
    """One document's prose as a stream, with each offset's line and sentence."""

    def __init__(self, text):
        self.stream, self.marks, self.starts = '', [], set()
        self.paragraphs = []
        last = None
        for n, line, heading in markdown_lines(text):
            new_block = (last is None or n != last + 1 or heading or
                         re.match(r'\s*([-*+]|\d+[.)])\s', line))
            if new_block and self.stream:
                self.paragraphs.append(len(self.stream))
            self.marks.append((len(self.stream), n))
            self.starts.add(len(self.stream))
            self.stream += line + (' \n ' if heading else ' ')
            last = n
        self.paragraphs.append(len(self.stream))

    def line(self, offset):
        return [n for off, n in self.marks if off <= offset][-1]

    def sentence(self, offset):
        """(start, end) of the sentence around an offset, inside its paragraph."""
        start = max([0] + [p for p in self.paragraphs if p <= offset])
        end = min(p for p in self.paragraphs if p > offset)
        for m in SENTENCE_END.finditer(self.stream, start, end):
            if m.end() <= offset:
                start = m.end()
            else:
                end = m.end()
                break
        return start, end


def cut_from(token, text):
    """Are the token's letters cut, in order, from the words, starting at the first one?
    `AON (always-on)` and `IDMAC (internal direct memory access controller)` are."""
    want, got = letters(token), re.sub(r'[^a-z]', '', text.lower())
    rest = iter(got)
    return len(want) >= 2 and got[:1] == want[:1] and all(c in rest for c in want)


def defined_at(page, m, token):
    """Is the use at match m defined by its own sentence or by its parentheses?"""
    if prose_acronyms.expanded(page.stream, m.start(), m.end(), letters(token).upper()):
        return True
    after = re.match(r'\s*\(([^)]*)\)', page.stream[m.end():])
    if after and cut_from(token, after.group(1)):
        return True
    start, end = page.sentence(m.start())
    before = WORD.findall(page.stream[start:m.start()])
    after = WORD.findall(page.stream[m.end():end])
    return spelled_runs(token, before) or spelled_runs(token, after)


def defined_earlier(page, m, token):
    return spelled_runs(token, WORD.findall(page.stream[:m.start()]), title_case=True)


UNIT = re.compile(r'^[kKMGT]?Hz$|^[KMGT]B$')
PART_NUMBER = re.compile(r'^[A-Z]{2,}[0-9]{4,}[A-Z0-9]*$')


def not_an_acronym(token, english):
    """A unit (`MHz`), a part number (`YT8531`), or an English word in capitals (`IDENTIFY`)."""
    if UNIT.match(token) or PART_NUMBER.match(token):
        return True
    return token.isalpha() and token.isupper() and len(token) >= 5 and token.lower() in english


def family(token):
    """The acronym a token is a form of: `FAT32` and `exFAT` are forms of `FAT`, `macOS` of `OS`."""
    width = prose_acronyms.WIDTH.match(token)
    if width:
        return width.group(1)
    prefixed = re.match(r'^[a-z]+([A-Z]{2,}[A-Za-z0-9]*)$', token)
    return family(prefixed.group(1)) if prefixed else token


def findings(text, accepted, english=frozenset()):
    """[(line, token, later line or None)] for each acronym whose first use is not defined.

    The forms of one acronym (`FAT`, `FAT32`, `exFAT`) share one first use: a reader who has the
    acronym has its widths, and a width read first is where the acronym is first met."""
    page = Page(text)
    out, seen, settled = [], set(), set()
    uses = {}
    for m in TOKEN.finditer(page.stream):
        uses.setdefault(family(canonical(m.group(0))), []).append(m)
    for m in TOKEN.finditer(page.stream):
        token = canonical(m.group(0))
        if prose_acronyms.is_hex(token) or not_an_acronym(token, english):
            continue
        root = family(token)
        if token.upper() in accepted or root.upper() in accepted or root in settled:
            continue
        if root in seen:
            continue
        seen.add(root)
        if token.isupper() and prose_acronyms.is_word_or_label(page.stream, m, token, page.starts):
            continue
        if any(defined_at(page, m, t) or defined_earlier(page, m, t) for t in {token, root}):
            settled |= {token, root}
            continue
        later = next((page.line(u.start()) for u in uses[root][1:]
                      if defined_at(page, u, canonical(u.group(0))) or defined_at(page, u, root)), None)
        out.append((page.line(m.start()), m.group(0), later))
    return out


def load_accepted(accepted_path=None):
    """Every accepted spelling, in capitals: the prose allowlist and the naming checks' words."""
    words = rust_names.load_words()
    accepted = {w.upper() for w in prose_acronyms.load_allow()}
    accepted |= {w.upper() for w in abbreviations.table(words, 'accepted')}
    if accepted_path:
        accepted |= {w.upper() for w in abbreviations.read_accepted_file(accepted_path)}
    return accepted


def line_of(path, finding):
    line, token, later = finding
    tail = f' (defined later, at line {later})' if later else ''
    return f'{path}:{line}: {token} is used before the text defines it{tail}'


def run(argv):
    parser = argparse.ArgumentParser(prog='readme_acronyms.py')
    parser.add_argument('--crate-dir', default='.')
    parser.add_argument('--accepted', help=f'the repository\'s accepted words (default {ACCEPTED_FILE}'
                        ' when it exists)')
    parser.add_argument('--ceiling', default=CEILING_FILE)
    parser.add_argument('--summary')
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args(argv)
    if args.selftest:
        return selftest()
    accepted_path = args.accepted or (ACCEPTED_FILE if Path(ACCEPTED_FILE).is_file() else None)
    try:
        accepted = load_accepted(accepted_path)
        ceiling = readme_documents.read_ceiling(args.ceiling)
    except (abbreviations.Malformed, readme_documents.Malformed, OSError) as error:
        print(f'{LABEL}: malformed configuration: {error}', file=sys.stderr)
        return 2
    docs = readme_documents.documents(args.crate_dir)
    english = abbreviations.read_word_list(abbreviations.ENGLISH)
    lines = [line_of(path, f) for path, text in docs for f in findings(text, accepted, english)]
    lines.append(readme_documents.total_line(LABEL, len(lines), 'acronyms used before their definition',
                                             len(docs), ceiling))
    print('\n'.join(lines))
    if args.summary:
        readme_documents.write_summary(args.summary, 'Acronyms before their definition', lines)
    return 0


ACCEPTED = {'API', 'CPU', 'PCI', 'IO', 'OS'}


def selftest():
    import contextlib
    import io
    import tempfile
    cases = [
        # (source, wanted [(token, later)])
        ('The EFI loads.\n', [('EFI', None)]),
        ('Extensible Firmware Interface (EFI) loads. Later EFI.\n', []),
        ('The EFI loads. Extensible Firmware Interface (EFI) is later.\n', [('EFI', 1)]),
        ('FAT, the file allocation table, holds clusters.\n', []),
        ('The file allocation table, which FAT names, holds clusters.\n', []),
        ('The File Allocation Table holds clusters.\n\nFAT has no journal.\n', []),
        ('The file allocation table holds clusters.\n\nFAT has no journal.\n', [('FAT', None)]),
        ('File Allocation Table (FAT) volumes: FAT12, FAT16 and FAT32.\n', []),
        ('# The FAT heading\n\nFile Allocation Table (FAT) after.\n', [('FAT', 3)]),
        ('The exFAT format and eMMC cards on macOS.\n', [('exFAT', None), ('eMMC', None)]),
        ('File Allocation Table (FAT), and its sibling exFAT.\n', []),
        ('A FAT12, FAT16 and FAT32 volume. File Allocation Table (FAT).\n', [('FAT12', 1)]),
        ('No I/O, a PCI bus, an API.\n', []),
        ('The `EFI` span and\n```\nMMIO\n```\n', []),
        ('THIS is emphasis, and this proves it.\nSAFETY: a label.\n', []),
        ('The PRPs and the PRP list.\n', [('PRPs', None)]),
        ('A byte 0xFF, FF and AA55.\n', []),
        ('Some Wrong Words (EFI) pair.\n', [('EFI', None)]),
        ('Non-Volatile Memory Express (NVMe) and PCI Express (PCIe).\n', []),
        ('The Basic Multilingual Plane is one plane. A reader of\nthe BMP knows it.\n', []),
        ('The Fast path. Access is Tight.\n\nFAT later.\n', [('FAT', None)]),
        ('# c\n\n[![CI](https://x.example/TRNG.svg)](https://x.example)\n\nThe EFI.\n', [('EFI', None)]),
    ]
    english = frozenset({'identify', 'fat'})
    cases += [
        ('A 100 MHz clock and the YT8531 chip and AXP15060.\n', []),
        ('The IDENTIFY command. The FAT.\n', [('FAT', None)]),
        ('The AON (always-on) domain.\n', []),
        ('A FAT12 volume, then a full directory except FAT16\'s root.\n', [('FAT12', None)]),
        ('The AON (never-off) domain.\n', [('AON', None)]),
    ]
    failed = []
    for src, want in cases:
        got = [(t, later) for _, t, later in findings(src, ACCEPTED, english)]
        if got != want:
            failed.append(f'{src!r} gave {got}, wanted {want}')
    with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stdout(io.StringIO()) as out, \
            contextlib.redirect_stderr(io.StringIO()):
        crate = Path(scratch, 'crate')
        (crate / 'src').mkdir(parents=True)
        (crate / 'README.md').write_text('# crate\n\nThe TRNG and the API.\n')
        (crate / 'src' / 'lib.rs').write_text('//! The TRNG.\npub fn f() {}\n')
        none = str(Path(scratch, 'absent'))
        code = run(['--crate-dir', str(crate), '--ceiling', none])
        report = out.getvalue()
        if code != 0 or report.count('TRNG is used before') != 2:
            failed.append(f'the README and the crate doc are judged alone: exit {code}, {report!r}')
        if 'API' in report:
            failed.append('an accepted acronym is reported')
        ceiling = Path(scratch, 'ceiling')
        ceiling.write_text('none\n')
        if run(['--crate-dir', str(crate), '--ceiling', str(ceiling)]) != 2:
            failed.append('a malformed ceiling file is not refused')
        accepted = Path(scratch, 'accepted-words')
        accepted.write_text('trng\n')
        if run(['--crate-dir', str(crate), '--ceiling', none, '--accepted', str(accepted)]) != 2:
            failed.append('an accepted-words line without a reason is not refused')
        accepted.write_text('trng  the chip manual and every reader say TRNG\n')
        out.truncate(0)
        run(['--crate-dir', str(crate), '--ceiling', none, '--accepted', str(accepted)])
        if 'TRNG is used' in out.getvalue():
            failed.append('a word in the repository\'s accepted words is reported')
    for message in failed:
        print(f'{LABEL} selftest: FAIL {message}', file=sys.stderr)
    print(f'{LABEL} selftest: %s' % ('FAILED' if failed else 'ok'))
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(run(sys.argv[1:]))
