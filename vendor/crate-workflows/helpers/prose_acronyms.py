"""An acronym is expanded at its first use in a file, or it is on the allowlist.

    python3 prose_acronyms.py FILE...
    python3 prose_acronyms.py --selftest

A reader meets EFI, MMIO or LBA cold unless the prose says what the letters stand for. In a crate's
README, CHANGELOG, `docs/*.md` and Rust doc comments (`///` and `//!`), a run of two or more capital
letters (mixed forms such as `FSInfo`, `PCIe` and `IPv4` included) must be expanded where the file
first uses it, or be listed in `prose_acronyms.toml`. Ceiling 0, no baseline.

**What is a token.** Two or more capitals, then any letters or digits, as a whole word; a trailing
lowercase `s` after capitals is a plural (`LBAs` is `LBA`). A constant (`MAX_SIZE`), a hex literal
(`0xFF`), a bare byte (`FF`) and a hex word with a digit (`AA55`) are not tokens.

**What is not an acronym.** A word of four or more letters in capitals is emphasis or a label, and
passes, when its lowercase form appears in the same file (`THIS` beside "this", `BUGS` beside "bugs")
or when it opens a line and ends in a colon (`SAFETY:`). Document and section names (`README`,
`DECISIONS`), rule words and short English words (`OR`, `NOT`) are the `words` list in
`prose_acronyms.toml`.

**What is not read.** Inline code, fenced code, addresses, HTML comments, front matter and
headings. Rust is read only in doc comments; a `//` comment and code are not prose here.

**Widths.** A token that is an expanded acronym followed by digits (`FAT32` once `FAT` is expanded in the
same file, or in the same crate for doc comments) is covered. A single word the letters are cut from
is an expansion (`null (NUL)`).

**Where "first use" is.** Markdown is judged per file: the README, the CHANGELOG and each
`docs/*.md` expand an acronym at that file's first use. Rust doc comments are judged once per crate,
because a reader does not need the same expansion in every source file: the README's expansions
count for all source files, and so does the first use in the crate root (`lib.rs` or `main.rs`,
then the other files in path order). All files named in one run are one crate.

**The expansion pattern the gate accepts.** The first use of the token is one of

- `Extensible Firmware Interface (EFI)`: the words, then the token in parentheses; or
- `EFI (Extensible Firmware Interface)`: the token, then the words in parentheses.

The initials of the words must spell the token, in order. Small words (of, the, and, for, to, in, on,
or, a, an, with, by) do not count, a hyphenated word counts as two (`Non-Volatile Memory Express`
is `NVMe`), a word that is itself an acronym gives all its letters (`PCI Express` is `PCIe`), and a CamelCase word gives each capital (`OpenID Connect` is `OIDC`), and
the words may be a prefix of the token (`File System Information (FSInfo)`). A line break inside
the words is fine. Expanding a token later in the file does not excuse an earlier bare use.

**Allowlist.** `prose_acronyms.toml` beside this file, key `allow`: acronyms every reader of a Rust
crate knows. A name that is not on it is expanded, not added.

Name: provisional, from the prose gates lane.
"""
import re
import sys
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from prose_text import prose_lines  # noqa: E402

TOKEN = re.compile(r'\b[A-Z]{2,}[A-Za-z0-9]*\b')
STOP = {'of', 'the', 'and', 'for', 'to', 'in', 'on', 'or', 'a', 'an', 'with', 'by'}
WORD = re.compile(r"[A-Za-z][A-Za-z0-9']*")
HEX = re.compile(r'^[0-9A-F]+$')


def normal(token):
    """The token without a plural s."""
    if token.endswith('s') and token[:-1].isupper() and len(token) > 3:
        return token[:-1]
    return token


def is_hex(token):
    return bool(HEX.match(token)) and (len(token) == 2 or any(c.isdigit() for c in token))


def initials(words):
    out = ''
    for w in words:
        if w.lower() in STOP:
            continue
        if w.isupper() and len(w) >= 2:
            out += w.lower()
        else:
            # A CamelCase word gives each capital (`OpenID` is O, I, D).
            out += w[0].lower() + ''.join(c.lower() for c in w[1:] if c.isupper())
    return out


def spells(token, words):
    """Do the words' initials spell the token (or its leading part)?"""
    want = re.sub(r'[0-9]', '', token).lower()
    got = initials(words)
    if len(words) == 1 and len(want) >= 3 and words[0][0].lower() == want[0]:
        # One word the letters are cut from, as `null (NUL)`.
        rest = iter(words[0].lower())
        if all(c in rest for c in want):
            return True
    return len(got) >= 2 and want.startswith(got)


def expanded(stream, start, end, token):
    """Is the use at stream[start:end] in an accepted expansion pattern?"""
    before = stream[:start].rstrip()
    if before.endswith('(') and stream[end:].startswith(')'):
        words = WORD.findall(before[:-1])[-8:]
        return any(spells(token, words[-k:]) for k in range(1, len(words) + 1))
    after = re.match(r'\s*\(([^)]*)\)', stream[end:])
    if after:
        return spells(token, WORD.findall(after.group(1)))
    return False


WIDTH = re.compile(r'^([A-Z]{2,})[0-9]+$')


def scan(path, text, allow, known, known_expanded=()):
    """([(line, token)], expanded, seen) for one file.

    `known` tokens were used earlier in the crate and `known_expanded` were expanded there. A token
    that is an expanded acronym and a width (`FAT32` after `FAT`) is covered."""
    stream, marks, starts = '', [], set()
    for n, line, heading in prose_lines(path, text):
        if heading:
            continue
        marks.append((len(stream), n))
        starts.add(len(stream))
        stream += line + ' '
    out, seen, settled = [], set(known), set(known_expanded)
    for m in TOKEN.finditer(stream):
        token = normal(m.group(0))
        if token in seen:
            continue
        seen.add(token)
        if token in allow or is_hex(token) or is_word_or_label(stream, m, token, starts):
            continue
        width = WIDTH.match(token)
        if width and width.group(1) in settled:
            continue
        if expanded(stream, m.start(), m.end(), token):
            settled.add(token)
            continue
        line = [n for off, n in marks if off <= m.start()][-1]
        out.append((line, m.group(0)))
    return out, settled, seen


def is_word_or_label(stream, m, token, starts):
    """Capitals used as emphasis or as a label, not as an abbreviation."""
    if len(token) < 4 or not token.isalpha():
        return False
    if re.search(r'\b%s\b' % token.lower(), stream):
        return True
    return m.start() in starts and stream[m.end():m.end() + 1] == ':'


def violations(path, text, allow, known=()):
    """[(line, token)] for each acronym whose first use in the file is not expanded."""
    return scan(path, text, allow, known)[0]


def crate_violations(files, allow):
    """[(path, line, token)] for a crate's files, [(path, text)] in any order.

    A Markdown file is judged on its own. Rust doc comments are judged once per crate: the README's
    expansions count for every source file, and so does the first use in the crate root (`lib.rs`
    or `main.rs`), then the other files in path order."""
    out, known, known_expanded = [], set(), set()
    mds = [f for f in files if f[0].endswith('.md')]
    rss = [f for f in files if f[0].endswith('.rs')]
    for path, text in sorted(mds, key=lambda f: (not Path(f[0]).name.upper().startswith('README'), f[0])):
        hits, settled, seen = scan(path, text, allow, ())
        out += [(path, n, t) for n, t in hits]
        if Path(path).name.upper().startswith('README'):
            known_expanded |= settled
            known |= settled
    def order(f):
        return (Path(f[0]).name not in ('lib.rs', 'main.rs'), f[0].count('/'), f[0])
    for path, text in sorted(rss, key=order):
        hits, settled, seen = scan(path, text, allow, known, known_expanded)
        out += [(path, n, t) for n, t in hits]
        known |= seen
        known_expanded |= settled
    return out


def load_allow():
    path = Path(__file__).resolve().with_name('prose_acronyms.toml')
    table = tomllib.loads(path.read_text(encoding='utf-8'))
    return set(table['allow']) | set(table['words'])



ALLOW = {'API', 'CPU'}


def selftest():
    cases = [
        # (source, wanted tokens)
        ('The EFI loads the image.\n', ['EFI']),
        ('Extensible Firmware Interface (EFI) loads the image. Later EFI again.\n', []),
        ('EFI (Extensible Firmware Interface) loads the image. Later EFI again.\n', []),
        ('The Logical Block Addressing (LBA) rules. The LBAs are 512 bytes.\n', []),
        ('Later EFI use. Then Extensible Firmware Interface (EFI) after the first use.\n', ['EFI']),
        ('Read the FSInfo sector, then the FSInfo sector again.\n', ['FSInfo']),
        ('File System Information (FSInfo) sector. The FSInfo again.\n', []),
        ('The API and the CPU are allowed.\n', []),
        ('The `EFI` code span and `MMIO` are exempt.\n', []),
        ('```\nEFI MMIO\n```\n', []),
        ('# EFI heading\nPlain text.\n', []),
        ('The PRP list and the CNS value.\n', ['PRP', 'CNS']),
        ('A byte 0xFF, then FF and AA55 and MAX_SIZE.\n', []),
        ('Controller or Namespace Structure (CNS) value. A Wrong Words (CNS) twist.\n', []),
        ('Some Wrong Words (EFI) pair.\n', ['EFI']),
        ('OpenID Connect (OIDC) and PCI Express (PCIe) and Non-Volatile Memory Express (NVMe).\n', ['PCI']),
    ]
    failed = 0
    for src, want in cases:
        got = [t for _, t in violations('a.md', src, ALLOW)]
        if got != want:
            failed += 1
            print(f'FAIL {src!r} gave {got}, wanted {want}', file=sys.stderr)
    rs = '/// The EFI table.\nfn f() {} // EFI in code\n'
    got = [t for _, t in violations('a.rs', rs, ALLOW)]
    if got != ['EFI']:
        failed += 1
        print(f'FAIL rust doc gave {got}', file=sys.stderr)
    good = '/// Extensible Firmware Interface (EFI) table.\nfn f() {} // MMIO here\n'
    if violations('a.rs', good, ALLOW):
        failed += 1
        print('FAIL rust good fixture failed', file=sys.stderr)
    word_cases = [
        ('File Allocation Table (FAT) and FAT32 and FAT12. The FAT16 too.\n', []),
        ('The FAT32 volume. File Allocation Table (FAT) later.\n', ['FAT32']),
        ('The FAT32 volume.\n', ['FAT32']),
        ('The null (NUL) byte. The NUL again.\n', []),
        ('The word (EFI) is not an expansion.\n', ['EFI']),
        ('Input and output (IO) and then IO.\n', []),
        ('The decisions are listed. See DECISIONS.\n', []),
        ('See DECISIONS and QEMU.\n', ['DECISIONS', 'QEMU']),
        ('SAFETY: the pointer is valid.\nNOTE: more.\n', []),
        ('A SAFETY: label after text is still a hit? Mid SAFETY: line.\n', ['SAFETY']),
        ('THIS is not an abbreviation, and this is the lowercase proof. ELF is.\n', ['ELF']),
        ('A CRC and a TRNG, both short, with crc in lowercase.\n', ['CRC', 'TRNG']),
    ]
    for src, want in word_cases:
        got = [t for _, t in violations('a.md', src, ALLOW)]
        if got != want:
            failed += 1
            print(f'FAIL {src!r} gave {got}, wanted {want}', file=sys.stderr)
    crate_cases = [
        # (files, wanted [(path, token)])
        ([('src/lib.rs', '/// The EFI.\n'), ('src/a.rs', '/// The EFI again.\n')], [('src/lib.rs', 'EFI')]),
        ([('src/a.rs', '/// The EFI again.\n'), ('src/lib.rs', '/// Extensible Firmware Interface (EFI).\n')], []),
        ([('src/lib.rs', '/// File Allocation Table (FAT).\n'), ('src/a.rs', '/// The FAT32 volume.\n')], []),
        ([('README.md', 'File Allocation Table (FAT).\n'), ('src/a.rs', '/// The FAT16 volume.\n')], []),
        ([('src/a.rs', '/// The EFI.\n'), ('README.md', 'Extensible Firmware Interface (EFI).\n')], []),
        ([('README.md', 'The EFI.\n'), ('docs/a.md', 'The EFI.\n')], [('README.md', 'EFI'), ('docs/a.md', 'EFI')]),
        ([('README.md', 'The EFI.\n'), ('src/lib.rs', '/// The EFI.\n')], [('README.md', 'EFI'), ('src/lib.rs', 'EFI')]),
    ]
    for files, want in crate_cases:
        got = [(p, t) for p, _, t in crate_violations(files, ALLOW)]
        if got != want:
            failed += 1
            print(f'FAIL crate {files} gave {got}, wanted {want}', file=sys.stderr)
    print('prose_acronyms selftest: %s' % ('FAILED' if failed else 'ok'))
    return 1 if failed else 0


def main(argv):
    if '--selftest' in argv:
        return selftest()
    if not argv[1:]:
        print(__doc__, file=sys.stderr)
        return 2
    allow = load_allow()
    files = []
    for f in argv[1:]:
        try:
            files.append((f, open(f, encoding='utf-8').read()))
        except (UnicodeDecodeError, OSError):
            continue
    bad = 0
    for path, line, token in crate_violations(files, allow):
        print(f'prose_acronyms: {path}:{line}: {token} is not expanded at its first use', file=sys.stderr)
        bad += 1
    if bad:
        print("prose_acronyms: write the words first, then the acronym in parentheses "
              "('Extensible Firmware Interface (EFI)'), or the acronym first with the words in "
              "parentheses. Words every reader knows go in helpers/prose_acronyms.toml.",
              file=sys.stderr)
    else:
        print(f'==> acronyms: all expanded in {len(files)} files')
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
