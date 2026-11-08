#!/usr/bin/env python3
"""Report every component of a Rust name that is not a word: `gens`, `val`, `cnt`, `idx`.

    python3 abbreviations.py [--manifest Cargo.toml] [--accepted .github/accepted-words]
                             [--ceiling .github/abbreviations-ceiling] [--summary FILE] FILE...
    python3 abbreviations.py --selftest

The rule: a name spells out what it holds. An abbreviation stays only where nobody says its
expansion (`pci`, `elf`, `usb`, `cpu`, `id`, `io`), and a name that mirrors a datasheet, a
specification or the standard library keeps its owner's spelling and says so where the reader meets
it. spelled_out.py refuses the abbreviations it lists; this check finds the ones nobody listed.

**What it reads.** The names helpers/rust_names.py says a crate defines (items, fields, variants,
parameters, locals, constants, test and proof harness names, file names, and the package name with
`--manifest`), split into components at `_`, at digits and at CamelCase boundaries, in lowercase.
Comments, strings and `#[doc(alias)]` strings are never read. A name kept from its source in the
form rust_names.py describes (`(<name>, <source>)` in its doc comment, plus `#[doc(alias)]`) is
skipped whole; an integer constant is never kept, so a register, bit or command constant is always
read.

**What counts as a word.** A component passes when it is one of these:

- an English word in helpers/english_words.txt (SCOWL, levels 10 to 50, American; its source and
  license are in its header and helpers/english_words.LICENSE), unless `[abbreviations]` in
  helpers/naming_words.toml names it (`err`, `info`: words in a dictionary, abbreviations in a name);
- a technical word in helpers/technical_words.txt (`mutex`, `kani`, `doorbell`);
- an accepted short word in `[accepted]` in helpers/naming_words.toml (`id`, `pci`, `len`), each
  with its reason;
- a word the repository accepts in `.github/accepted-words`: one word per line, then its reason
  (`lba  the specifications and every reader say LBA`); `#` starts a comment;
- any of those with a plural `s` or `es`, a number, or a single letter (a loop index or a
  coordinate is a different question from an abbreviation).

**What it prints.** One line per finding, `file:line: name: component 'x' is not a word`, with
`(did you mean 'y'?)` when a suggestion is cheap: `[abbreviations]` or `[expansions]` in
helpers/naming_words.toml, the spelled-out table, or else the one word among the crate's own
components that starts with the component or holds its letters in order with the same first and
last letter (`gens` beside `generation` suggests `generations`). Then a count per file, and a
total. `--summary FILE` appends the same report as Markdown, for `$GITHUB_STEP_SUMMARY`.

**Report only.** It exits 0 whatever it finds. It exits 2 when its configuration is malformed: a
word list line that is not lowercase letters, an `[accepted]` entry without a reason, an
accepted-words line without a reason, or a ceiling file that does not hold one whole number. The
ceiling file (`.github/abbreviations-ceiling`, one number) is read and the report says whether the
total is over it; nothing enforces it yet.

To rebuild the English list from the SCOWL 2020.12.07 release (`scowl-2020.12.07.tar.gz`, sha256 in
the list's header): concatenate `final/english-words.{10,20,35,40,50}` and
`final/american-words.{10,20,35,40,50}`, convert from Latin-1, keep the lines matching `^[a-z]+$`,
and sort them with `LC_ALL=C sort -u`.

BUGS:
- A component the English list holds is a word even where a name uses it as an abbreviation
  (`sec`, `dev`); `[abbreviations]` lists the ones known.
- The file's name, the table names and `.github/accepted-words` are provisional.
"""

import os
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import rust_names  # noqa: E402

HERE = Path(__file__).parent
ENGLISH = HERE / 'english_words.txt'
TECHNICAL = HERE / 'technical_words.txt'
ACCEPTED_FILE = '.github/accepted-words'
CEILING_FILE = '.github/abbreviations-ceiling'
WORD = re.compile(r'[a-z]+')
LABEL = 'abbreviations'


class Malformed(Exception):
    """The configuration cannot be read; the message says where."""


def read_word_list(path):
    """The words in a list of one lowercase word per line, `#` lines and blank lines skipped."""
    out = set()
    for number, raw in enumerate(Path(path).read_text(encoding='utf-8').splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith('#'):
            continue
        if not WORD.fullmatch(line):
            raise Malformed(f'{path}:{number}: {line!r} is not one lowercase word')
        out.add(line)
    return out


def read_accepted_file(path):
    """The words of a repository's accepted-words file: `word reason`, one per line."""
    out = {}
    for number, raw in enumerate(Path(path).read_text(encoding='utf-8').splitlines(), 1):
        line = raw.split('#', 1)[0].strip()
        if not line:
            continue
        word, _, reason = line.replace('\t', ' ').partition(' ')
        if not WORD.fullmatch(word):
            raise Malformed(f'{path}:{number}: {word!r} is not one lowercase word')
        if not reason.strip():
            raise Malformed(f'{path}:{number}: {word!r} carries no reason; write `{word} <why>`')
        out[word] = reason.strip()
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


def table(words, name):
    """A `[name]` table of word = text from naming_words.toml, checked."""
    found = words.get(name, {})
    if not isinstance(found, dict):
        raise Malformed(f'naming_words.toml: [{name}] is not a table')
    for word, text in found.items():
        if not WORD.fullmatch(word):
            raise Malformed(f'naming_words.toml: [{name}] key {word!r} is not one lowercase word')
        if not isinstance(text, str) or not text.strip():
            raise Malformed(f'naming_words.toml: [{name}] {word} needs a text')
    return found


class Vocabulary:
    """What counts as a word, and what to suggest for a component that is not one."""

    def __init__(self, words, english, technical, repository=None):
        self.accepted = table(words, 'accepted')
        self.abbreviations = table(words, 'abbreviations')
        self.suggested = {**table(words, 'expansions'), **table(words, 'spelled_out')}
        self.suggested.update(self.abbreviations)
        self.repository = repository or {}
        self.words = (english - set(self.abbreviations)) | technical
        self.words |= set(self.accepted) | set(self.repository)

    def is_word(self, part):
        if part.isdigit() or len(part) == 1 or part in self.words:
            return True
        if part in self.abbreviations:
            return False
        for ending in ('s', 'es'):
            stem = part.removesuffix(ending)
            if stem != part and len(stem) > 1 and stem in self.words:
                return True
        return False

    def suggest(self, part, crate_words):
        """A cheap suggestion for `part`, or None: a listed one, else the one crate word that
        extends it. Two letters say too little to search the crate's words with (`pm` would find
        `platform`)."""
        if part in self.suggested:
            return self.suggested[part]
        if len(part) < 3:
            return None
        found = _extensions(part, crate_words)
        if len(found) == 1:
            return found[0]
        stem = part.removesuffix('s')
        if stem != part and len(stem) > 2 and not found:
            found = _extensions(stem, crate_words)
            if len(found) == 1:
                return found[0] if found[0].endswith('s') else found[0] + 's'
        return None


def _skeleton(part, word):
    """Whether `part` is `word` with some of its vowels dropped, its first letter kept: `cnt` is
    `count`'s skeleton and `lmt` is `limit`'s, while `stg` is not `selecting`'s."""
    return part[:1] == word[:1] and _drops_vowels(part[1:], word[1:])


def _drops_vowels(part, word):
    if not word:
        return not part
    if part[:1] == word[:1] and _drops_vowels(part[1:], word[1:]):
        return True
    return word[0] in 'aeiou' and _drops_vowels(part, word[1:])


def _extensions(part, crate_words):
    """The crate words that start with `part` or hold its skeleton, longer than it; singulars only
    when the plural is also there."""
    found = sorted(w for w in crate_words
                   if len(w) > len(part) and (w.startswith(part) or _skeleton(part, w)))
    return [w for w in found if not (w.endswith('s') and w[:-1] in found)]


def load_vocabulary(words_path=rust_names.WORDS, accepted_path=None):
    import tomllib
    try:
        words = rust_names.load_words(words_path)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise Malformed(f'{words_path}: {error}') from error
    repository = read_accepted_file(accepted_path) if accepted_path else {}
    return Vocabulary(words, read_word_list(ENGLISH), read_word_list(TECHNICAL), repository)


def findings(path, found, vocabulary, crate_words):
    """(path, line, name, component, suggestion) for each component of each definition that is not a
    word; a kept definition is skipped whole."""
    out = []
    for d in found:
        parts = [p for p in rust_names.components(d.name) if not vocabulary.is_word(p)]
        if not parts or rust_names.kept(d)[0]:
            continue
        for part in dict.fromkeys(parts):
            out.append((path, d.line, d.name, part, vocabulary.suggest(part, crate_words)))
    return out


def line_of(finding):
    path, line, name, part, suggestion = finding
    if not suggestion:
        tail = ''
    elif re.fullmatch(r'[a-z_]+', suggestion):
        tail = f" (did you mean '{suggestion}'?)"
    else:
        tail = f' (write {suggestion})'  # a listed hint that is a phrase, not one word
    return f"{path}:{line}: {name}: component '{part}' is not a word{tail}"


def report(results, judged, ceiling):
    """The report's lines: the findings, a count per file and the total."""
    lines = [line_of(f) for f in results]
    per_file = {}
    for f in results:
        per_file[f[0]] = per_file.get(f[0], 0) + 1
    lines += [f'{path}: {count} components are not words' for path, count in per_file.items()]
    total = (f'{LABEL}: {judged} names judged, {len(results)} components are not words, in '
             f"{len(per_file)} {'file' if len(per_file) == 1 else 'files'} (report only")
    if ceiling is None:
        total += '; no ceiling file)'
    elif len(results) > ceiling:
        total += f'; {len(results) - ceiling} over the ceiling of {ceiling}, not enforced)'
    else:
        total += f'; within the ceiling of {ceiling}, not enforced)'
    return lines + [total]


def run(argv):
    import argparse
    parser = argparse.ArgumentParser(prog='abbreviations.py')
    parser.add_argument('files', nargs='*')
    parser.add_argument('--manifest', help="the crate's Cargo.toml, whose package name is judged too")
    parser.add_argument('--words', default=str(rust_names.WORDS))
    parser.add_argument('--accepted', help=f'the repository\'s accepted words (default {ACCEPTED_FILE}'
                        ' when it exists)')
    parser.add_argument('--ceiling', default=CEILING_FILE, help='a file holding the ceiling')
    parser.add_argument('--summary', help='a Markdown file to append the report to')
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args(argv)
    if args.selftest:
        return selftest()
    accepted = args.accepted or (ACCEPTED_FILE if Path(ACCEPTED_FILE).is_file() else None)
    try:
        vocabulary = load_vocabulary(args.words, accepted)
        ceiling = read_ceiling(args.ceiling)
    except (Malformed, OSError) as error:
        print(f'{LABEL}: malformed configuration: {error}', file=sys.stderr)
        return 2
    per_path = []
    if args.manifest:
        per_path.append((args.manifest, rust_names.manifest_definitions(args.manifest)))
    for path in args.files:
        if path.endswith('.rs'):
            text = Path(path).read_text(encoding='utf-8', errors='replace')
            per_path.append((path, rust_names.file_definitions(path, text)))
    crate_words = {p for _, found in per_path for d in found
                   for p in rust_names.components(d.name) if p in vocabulary.words}
    results = [f for path, found in per_path for f in findings(path, found, vocabulary, crate_words)]
    judged = sum(len(found) for _, found in per_path)
    lines = report(results, judged, ceiling)
    print('\n'.join(lines))
    if args.summary:
        with open(args.summary, 'a', encoding='utf-8') as out:
            out.write(f'### Abbreviations (report only)\n\n{lines[-1]}\n\n')
            if results:
                out.write('```\n' + '\n'.join(lines[:-1]) + '\n```\n')
    return 0


def selftest():
    import contextlib
    import io
    import tempfile
    failed = []
    with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stdout(io.StringIO()), \
            contextlib.redirect_stderr(io.StringIO()):
        accepted = Path(scratch, 'accepted-words')
        accepted.write_text('# The repository\'s own words.\nlba  every reader says LBA\n')
        vocabulary = load_vocabulary(accepted_path=accepted)
        src = '''
pub struct Table { gens: Vec<u32>, slot_count: usize, generation: u32 }
pub fn kani_proof(id: u32, pci_len: usize) { let val = 0; let cnt = 1; let idx = 2; }
pub fn walk() { for item in iter_slots() { let lba_first = 0; let mutexes = 1; } }
/// The number of generations a slot has seen.
pub const GENS_SEEN: u32 = 0;
/// **Receive Descriptor Tail** (Rdt, Intel 82574 datasheet, section 10.2.5.9).
#[doc(alias = "receive descriptor tail")]
pub struct Rdt;
/// **ACMD41, SD send operation conditions** (SD_SEND_OP_COND, the SD specification).
#[doc(alias = "SD send operation conditions")]
pub const SD_SEND_OP_COND: u8 = 41;
// let tmp = 1; is a comment
pub fn quoted() { let s = "ctx"; let i = 0; let x2 = 0; }
'''
        found = rust_names.definitions(src)
        crate_words = {p for d in found for p in rust_names.components(d.name)
                       if p in vocabulary.words}
        got = [(name, part, hint) for _, _, name, part, hint in
               findings('x.rs', found, vocabulary, crate_words)]
        want = [('gens', 'gens', 'generations'), ('val', 'val', 'value'), ('cnt', 'cnt', 'count'),
                ('idx', 'idx', 'index'), ('GENS_SEEN', 'gens', 'generations'),
                ('SD_SEND_OP_COND', 'op', 'operation'), ('SD_SEND_OP_COND', 'cond', None)]
        if got != want:
            failed.append(f'findings {got}, wanted {want}')
        for word in ('slot', 'kani', 'generation', 'id', 'pci', 'crc', 'len', 'iter', 'lba',
                     'mutexes'):
            if not vocabulary.is_word(word):
                failed.append(f'{word} is reported, and it is a word')
        if vocabulary.is_word('err') or vocabulary.suggest('err', set()) != 'error':
            failed.append('err is a dictionary word that a name still spells out as error')
        if vocabulary.suggest('pm', {'platform'}) is not None:
            failed.append('a two-letter component is not searched for in the crate words')
        if vocabulary.suggest('frob', {'frobnicate', 'frobnicator'}) is not None:
            failed.append('two crate words extend frob, so there is no cheap suggestion')
        if vocabulary.suggest('frobs', {'frobnicator'}) != 'frobnicators':
            failed.append('a plural component suggests the plural of the one crate word')
        line = line_of(('x.rs', 3, 'gens', 'gens', 'generations'))
        if line != "x.rs:3: gens: component 'gens' is not a word (did you mean 'generations'?)":
            failed.append(f'line form {line!r}')
        line = line_of(('x.rs', 3, 'irq', 'irq', 'interrupt or interrupt_line'))
        if not line.endswith("is not a word (write interrupt or interrupt_line)"):
            failed.append(f'a phrase hint reads as a phrase: {line!r}')
        if not report([], 4, 0)[-1].endswith('within the ceiling of 0, not enforced)'):
            failed.append(f'total line {report([], 4, 0)[-1]!r}')
        if 'over the ceiling of 1' not in report([('a.rs', 1, 'v', 'v', None)] * 3, 3, 1)[-1]:
            failed.append('a total over the ceiling says so')
        for name, text in (('no-reason', 'lba\n'), ('two-words', 'Lba-x reason\n')):
            bad = Path(scratch, name)
            bad.write_text(text)
            if run(['--accepted', str(bad), '--ceiling', str(Path(scratch, 'none'))]) != 2:
                failed.append(f'the malformed accepted-words file {name} is not refused')
        ceiling = Path(scratch, 'ceiling')
        for text, want_code in (('12\n', 0), ('# the ceiling\n3\n', 0), ('twelve\n', 2), ('1\n2\n', 2)):
            ceiling.write_text(text)
            code = run(['--accepted', str(accepted), '--ceiling', str(ceiling)])
            if code != want_code:
                failed.append(f'ceiling file {text!r}: exit {code}, wanted {want_code}')
        words = Path(scratch, 'words.toml')
        words.write_text('[accepted]\nid = ""\n')
        if run(['--words', str(words), '--ceiling', str(ceiling)]) != 2:
            failed.append('an [accepted] entry without a reason is not refused')
        rust = Path(scratch, 'a.rs')
        rust.write_text('fn f() { let cnt = 0; }\n')
        ceiling.write_text('0\n')
        if run(['--accepted', str(accepted), '--ceiling', str(ceiling), str(rust)]) != 0:
            failed.append('a finding over the ceiling fails the run, and the check reports only')
    for message in failed:
        print(f'{LABEL} selftest: {message}', file=sys.stderr)
    if failed:
        return 1
    print(f'{LABEL} selftest: ok')
    return 0


if __name__ == '__main__':
    sys.exit(run(sys.argv[1:]))
