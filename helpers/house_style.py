"""The house-style ratchet: British spellings and imperial units, counted, and never allowed to grow.

calef, 2026-10-06 (UTC), two rulings on the same morning:

    "Lets standardize on American because I'm American."
    "Lets also standardize on metric measures, because imperial is stupid."

The tree's prose and names use American spelling, and its measurements are metric. Neither was
ruled before, and the tree had drifted British: on the ruling's day this counted 3,946 British
forms (2,134 in Markdown, 1,812 in code and configuration) and 5 imperial quantities. A sweep in
batches drives the counts to zero (the batch plan is in the pull request that landed this, #1735);
this gate makes sure nothing new arrives while it does. It is rung 2 of the AGENTS.md ladder. The
rule's record is a dated amendment to §213 (writing standards) and design/naming.md's spelling
section.

    python3 helpers/house_style.py --check      # what script/lint runs
    python3 helpers/house_style.py --list [PATH..]  # every counted hit, path:line: word
    python3 helpers/house_style.py --bank       # lower CEILINGS to the tree; never raises
    python3 helpers/house_style.py --selftest   # the traps, as fixtures

Name: provisional, minted by lane/american-spelling on 2026-10-06. A shared python module under
`helpers/`, which `script/names` puts out of its own scope, so its provenance is this paragraph.
calef names things; expect this to change.

**Two halves, the prose ratchet's shape.** A count over its ceiling in `CEILINGS` fails, which is
what guards `main`, where there is no merge base. A branch is also held to its merge base: summed
over the files it changed, no category may grow. That second half is the real ratchet. With
thousands of hits, ordinary edits delete British words all the time, and a ceiling alone would let
the next change spend that slack. Below the ceiling is silent, on purpose: failing there too (the
`agents-md-lines` shape) would fail every unrelated change that happened to delete a line with
one of these words in it. `--bank` lowers the ceilings, and each sweep batch runs it.

**British spelling is counted by `typos --locale en-us`**, the checker the spelling gate already
runs, so the word list is VarCon's (maintained upstream, a few thousand pairs) rather than one
written here. `typos` splits identifiers, so `FATAL_RISK_COLOURS` counts as a name: the ruling
covers names. `EXTRA_BRITISH` adds the forms VarCon leaves alone but American usage does not use.
`NOT_BRITISH` removes the two VarCon flags that American dictionaries accept as they stand.

**Imperial units are counted by regular expression**, and only as quantities: a unit word must
follow a number (digits, a number word, `a`, `an`, `half`), so "under the caller's feet", "foot
gun" and "the last mile" are idiom, not measurement. An abbreviation (`ft`, `lb`, `oz`, `yd`,
`mi`, `gal`) must follow digits. Of the inch marks only the double prime counts: a `"` after
digits closes a quoted string here (`"core 3"`) far more often than it measures anything. `in` is
never read as inches; "1 in 10" is far more common here than a length.

**Permanent exclusions, each correct for a stated reason, not a backlog:**

- A Markdown blockquote line (`>`). It is a quotation, and a quotation keeps its source's words,
  the em-dash gate's precedent (calef, 2026-09-22). An inline quotation still counts: put it on
  its own `>` line. Matching quotation marks instead lets anyone past by quoting themselves.
- An external name or title, in `EXTERNAL_NAMES`: a vulnerability's name, a paper's title. Each
  entry carries where it comes from. Add one only for a name somebody else coined.
- A URL, which is somebody else's address.
- An industry designation that is a product name rather than a measurement, in `DESIGNATIONS`:
  a 3.5-inch drive bay, a 19-inch rack, a screen's diagonal class. A metric gloss is welcome.
- Data files (`.csv`, `.tsv`, `.svg`): each is generated from the tree, so it follows its source,
  or records a past fact (a CI job's name on a date), which a sweep must not rewrite.
- Trees that are somebody else's text, as `_typos.toml` already excludes them: `vendor/`,
  `patches/`, lockfiles, `target/`, and board transcripts (`bench/**/*.log`).
- This file, which has to spell the words it counts.

**At zero, this file retires.** `locale = "en-us"` in `_typos.toml` then makes the spelling gate
reject British forms outright, the extras move into its `extend-words`, and the units half stays
here or moves to its own gate. That is a rung up from a counted ratchet to an outright refusal.

BUGS:
- VarCon is a dictionary, not a judgment about this tree. It flags a few words an American writer
  would accept (`NOT_BRITISH` holds the ones found so far) and misses some British ones
  (`EXTRA_BRITISH`). Both lists grow as the sweep meets cases.
- The imperial half reads quantities by pattern, so "a pound of" in a recipe-style idiom would
  count and "six-foot" written without its number would not, and `12"` for twelve inches is not
  read at all. None of the three occurred on the ruling's day.
"""

import bisect
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True

SELF = 'helpers/house_style.py'

# The counts on 2026-10-06, and since lowered by each sweep batch's `--bank`. Never raise one by
# hand: a new British form or imperial unit is fixed, not admitted.
CEILINGS = {
    'british-markdown': 199,
    'british-other': 1_812,
    'imperial': 4,
}

# --- what is out of scope -----------------------------------------------------------------------

EXCLUDED_PREFIXES = ('vendor/', 'patches/', 'target/')
# `.rec` and `.expected.txt` are the terminal replay harness's committed captures
# (crates/video_terminal/recordings/README.md): what a program painted on a screen on a date,
# the bench transcript's category, not prose anybody wrote.
EXCLUDED_SUFFIXES = ('.lock', '.csv', '.tsv', '.svg', '.rec', '.expected.txt')


def in_scope(path):
    if path == SELF or path.startswith(EXCLUDED_PREFIXES) or path.endswith(EXCLUDED_SUFFIXES):
        return False
    if path.startswith('bench/') and path.endswith('.log'):
        return False
    return True


# --- British spelling ---------------------------------------------------------------------------

# VarCon's flags that American dictionaries accept as written: Merriam-Webster lists "ax or axe",
# and "queueing" is the usual spelling of queueing theory.
NOT_BRITISH = {'axe', 'axes', 'queueing'}

# British forms VarCon leaves alone. Matched inside identifiers too, so
# `package_archive::CATALOGUE` and `catalogued_stem` count. Not here, on purpose: `analogue` and
# `acknowledgement`, which American dictionaries list beside `analog` and `acknowledgment`.
EXTRA_BRITISH = ['catalogue', 'catalogues', 'catalogued', 'cataloguing', 'grey', 'greys',
                 'greyed', 'greying', 'programme', 'programmes', 'whilst', 'practise',
                 'practises', 'practised', 'practising', 'spelt']


EXTRA = re.compile('|'.join(sorted(EXTRA_BRITISH, key=len, reverse=True)))


class _Match:
    """The slice of the original text a match in its lowered copy covers."""

    def __init__(self, text, span):
        self._text, self._span = text, span

    def span(self):
        return self._span

    def start(self):
        return self._span[0]

    def group(self, _=0):
        return self._text[self._span[0]:self._span[1]]


def extra_at(text, m):
    """Whether a case-blind EXTRA match is a whole word, an identifier part included.

    `catalogued_stem`, `CATALOGUE` and `ParseCatalogue` are words here; `greyhound` is not. One
    case-blind scan and this check, rather than a pattern per casing, because the pattern-per-casing
    form took half a minute over the tree.
    """
    a, b = m.span()
    word = m.group(0)
    before = text[a - 1] if a else ' '
    after = text[b] if b < len(text) else ' '
    if word.islower():
        return not before.isalpha() and not after.islower()
    if word[0].isupper() and word[1:].islower():
        return not before.isupper() and not after.islower()
    if word.isupper():
        return not before.isupper() and not after.isupper()
    return False

# Names somebody else coined, which keep their spelling. Each with its source.
EXTERNAL_NAMES = [
    'Spectre',  # the 2018 speculative-execution vulnerability (Kocher et al.)
    'What Have We Learnt in 20 Years of L4 Microkernels',  # Elphinstone and Heiser, SOSP 2013
]

URL = re.compile(r'https?://\S+')

# --- imperial units -----------------------------------------------------------------------------

_NUMBER = (r'(?:\d+(?:[.,]\d+)?|a|an|half|one|two|three|four|five|six|seven|eight|nine|ten|'
           r'eleven|twelve|fifteen|eighteen|twenty|thirty|forty|fifty|hundred)')
_UNIT = r'(?:inch|inches|foot|feet|mile|miles|yard|yards|pound|pounds|ounce|ounces|gallon|gallons)'
IMPERIAL = [
    # A unit word after a quantity. Not "foot gun", not "a pound sign": those are not lengths.
    re.compile(rf'(?i)\b{_NUMBER}[\s-]+{_UNIT}\b(?![\s-]*(?:guns?|sign|key|symbol)\b)'),
    re.compile(r'(?i)\bfahrenheit\b|°F\b'),
    # An abbreviation after digits that open a word, so `\x1b[?25lb` (an escape sequence) is not
    # twenty-five pounds.
    re.compile(r'(?<![^\s(])\d+(?:\.\d+)?\s?(?:ft|lbs?|oz|yd|mi|gal)\b'),
    # The double prime, the typographic inch mark. A `"` after digits is not read: in this tree
    # it closes a quoted string ("may run on core 3") far more often than it measures anything.
    re.compile(r'\d″'),
]

# Industry designations that are product names, not measurements. A metric gloss is welcome.
DESIGNATIONS = re.compile(
    r'(?i)\b\d+(?:\.\d+)?(?:[\s-]?inch(?:es)?|"|″)[\s-]+'
    r'(?:drive|disk|floppy|bay|bays|rack|racks|display|screen|monitor|laptop|tablet|panel|'
    r'form factor)\b')


def _spans(pattern, line):
    return [m.span() for m in pattern.finditer(line)]


def _inside(pos, spans):
    return any(a <= pos < b for a, b in spans)


# A cheap first pass: most files carry no unit word at all, and the patterns above are not cheap.
MAYBE_IMPERIAL = re.compile(
    rf'\b{_UNIT}\b|fahrenheit|°f|\d\s?(?:ft|lbs?|oz|yd|mi|gal)\b|\d″')


def imperial_hits(text):
    """[(offset, words)] for each imperial quantity in `text`."""
    if not MAYBE_IMPERIAL.search(text.lower()):
        return []
    exempt = _spans(DESIGNATIONS, text) + _spans(URL, text)
    return [(m.start(), m.group(0)) for pattern in IMPERIAL for m in pattern.finditer(text)
            if not _inside(m.start(), exempt)]


# --- counting -----------------------------------------------------------------------------------

def category(path):
    return 'british-markdown' if path.endswith('.md') else 'british-other'


def _typos(root, locale):
    config = os.path.join(REPO, '_typos.toml')
    out = subprocess.run(
        ['typos', '--locale', locale, '--format', 'json', '--config', config, '--hidden', '.'],
        cwd=root, capture_output=True, text=True)
    if out.returncode not in (0, 2):
        sys.stderr.write(out.stderr)
        raise SystemExit('house style: typos failed; run script/bootstrap (it installs it)')
    hits = set()
    for row in out.stdout.splitlines():
        r = json.loads(row)
        if r.get('type') != 'typo':
            continue
        p = r['path'][2:] if r['path'].startswith('./') else r['path']
        hits.add((p, r.get('line_num'), r['byte_offset'], r['typo']))
    return hits


def run_typos(root):
    """British forms in the tree at `root`: {path: [(line or None, offset, word)]}.

    What `typos --locale en-us` flags and plain `--locale en` does not, so an ordinary typo is the
    spelling gate's business and never counted here as British (found on #1732, where a FreeBSD
    driver's three-letter name would have been). A walk rather than a file list, because `typos` reads a
    walk in parallel and a list one file at a time (0.4 s against 4 s for this tree). The caller
    filters the paths. A hit with no line is in the file's name, which counts: a name is what the
    ruling covers.
    """
    hits = {}
    for p, line, offset, word in _typos(root, 'en-us') - _typos(root, 'en'):
        hits.setdefault(p, []).append((line, offset, word))
    return hits


def file_hits(path, text, typos_hits):
    """Every counted hit in one file: [(category, line or None, word)]."""
    lines = text.split('\n')
    markdown = path.endswith('.md')
    found = []

    def exempt_line(line):
        return markdown and re.match(r'\s*>', line)

    def exempt_at(line, offset):
        raw = line.encode()
        spans = [(m.start(), m.end()) for m in URL.finditer(line)]
        for name in EXTERNAL_NAMES:
            spans += [m.span() for m in re.finditer(re.escape(name), line)]
        char = len(raw[:offset].decode(errors='replace'))
        return _inside(char, spans)

    for lineno, offset, word in typos_hits:
        if word.lower() in NOT_BRITISH:
            continue
        if lineno is not None:
            line = lines[lineno - 1] if lineno - 1 < len(lines) else ''
            if exempt_line(line) or exempt_at(line, offset):
                continue
        found.append((category(path), lineno, word))
    # The two regular-expression passes read the whole text, so a phrase broken across a line
    # ("Two foot" then "guns") is read as written, and a file with none of the words costs one scan.
    starts = []

    def line_of(pos):
        if not starts:
            starts.append(0)
            for line in lines:
                starts.append(starts[-1] + len(line) + 1)
        return bisect.bisect_right(starts, pos)

    urls = _spans(URL, text)
    # Searched in a lowered copy: Python's case-blind alternation took three seconds over the tree,
    # and lowering keeps every offset as long as it keeps the length, which is checked.
    low = text.lower()
    for m in EXTRA.finditer(low if len(low) == len(text) else ''):
        m = _Match(text, m.span())
        n = line_of(m.start())
        if extra_at(text, m) and not exempt_line(lines[n - 1]) and not _inside(m.start(), urls):
            found.append((category(path), n, m.group(0)))
    for pos, word in imperial_hits(text):
        if not exempt_line(lines[line_of(pos) - 1]):
            found.append(('imperial', line_of(pos), word))
    name = os.path.basename(path)
    for m in EXTRA.finditer(name.lower()):
        m = _Match(name, m.span())
        if extra_at(name, m):
            found.append((category(path), None, m.group(0)))
    return found


def read(path):
    try:
        with open(path, encoding='utf-8') as f:
            return f.read()
    except (UnicodeDecodeError, OSError):
        return None


def census(root, paths):
    """{path: hits} for every readable text file in `paths`, read under `root`."""
    texts = {}
    for p in paths:
        t = read(os.path.join(root, p))
        if t is not None:
            texts[p] = t
    typos = run_typos(root)
    return {p: file_hits(p, t, typos.get(p, [])) for p, t in texts.items()}


def totals(by_path):
    t = {k: 0 for k in CEILINGS}
    for hits in by_path.values():
        for cat, _, _ in hits:
            t[cat] += 1
    return t


# --- git ----------------------------------------------------------------------------------------

def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True, text=True, cwd=REPO)
    return r.stdout if r.returncode == 0 else None


def tracked():
    files = git('ls-files', '-z', '--cached', '--others', '--exclude-standard') or ''
    return [f for f in files.split('\0') if f and in_scope(f) and os.path.isfile(f) and not os.path.islink(f)]


def merge_base():
    base = (git('merge-base', 'HEAD', 'origin/main') or '').strip()
    head = (git('rev-parse', 'HEAD') or '').strip()
    return base if base and base != head else None


def growth(base):
    """Net change per category, base to working tree, over the files this branch changed.

    Both sides are copied into one scratch tree and read by one `typos` run, so a file's name and
    its contents are counted the same way on each side.
    """
    old_paths, new_paths = set(), set()
    for line in (git('diff', '--name-status', '-M', base) or '').splitlines():
        cells = line.split('\t')
        status, paths = cells[0], cells[1:]
        if status.startswith('R') or status.startswith('C'):
            old_paths.add(paths[0])
            new_paths.add(paths[1])
        elif status == 'D':
            old_paths.add(paths[0])
        elif status == 'A':
            new_paths.add(paths[0])
        else:
            old_paths.add(paths[0])
            new_paths.add(paths[0])
    for f in (git('ls-files', '-z', '--others', '--exclude-standard') or '').split('\0'):
        if f:
            new_paths.add(f)
    old_paths = {p for p in old_paths if in_scope(p)}
    new_paths = {p for p in new_paths if in_scope(p) and os.path.isfile(p)}
    scratch = tempfile.mkdtemp(prefix='house-style-')
    try:
        listed = []
        for side, paths in (('base', old_paths), ('head', new_paths)):
            for p in paths:
                if side == 'base':
                    blob = subprocess.run(['git', 'show', f'{base}:{p}'], cwd=REPO,
                                          capture_output=True).stdout
                else:
                    with open(os.path.join(REPO, p), 'rb') as f:
                        blob = f.read()
                dest = os.path.join(scratch, side, p)
                os.makedirs(os.path.dirname(dest), exist_ok=True)
                with open(dest, 'wb') as f:
                    f.write(blob)
                listed.append(f'{side}/{p}')
        by_path = census(scratch, listed)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
    net = {k: 0 for k in CEILINGS}
    where = {}
    for p, hits in by_path.items():
        side, real = p.split('/', 1)
        sign = 1 if side == 'head' else -1
        for cat, _, _ in hits:
            net[cat] += sign
            where.setdefault((cat, real), 0)
            where[(cat, real)] += sign
    return net, where


# --- the commands -------------------------------------------------------------------------------

LABELS = {
    'british-markdown': 'British spellings in Markdown',
    'british-other': 'British spellings in code, scripts and configuration',
    'imperial': 'imperial units',
}


def check():
    bad = []
    now = totals(census(REPO, tracked()))
    for cat, n in now.items():
        if n > CEILINGS[cat]:
            # With no net growth on the branch, main itself is over: a change that landed before
            # this gate did (#1735 met exactly this in CI, one British word from #1728 or #1729).
            bad.append(f'{LABELS[cat]}: {n:,}, over the ceiling of {CEILINGS[cat]:,} in {SELF}. '
                       f'If this branch adds none, main moved past it: rebase, and set the ceiling '
                       f'to the merged count in this change, saying why in the commit')
    base = merge_base()
    if base:
        net, where = growth(base)
        for cat, n in net.items():
            if n > 0:
                grew = sorted(p for (c, p), d in where.items() if c == cat and d > 0)
                bad.append(f'{LABELS[cat]}: this branch adds {n} net, in {", ".join(grew)}')
    return now, bad


def bank():
    now = totals(census(REPO, tracked()))
    text = open(os.path.join(REPO, SELF)).read()
    for cat, n in now.items():
        lowered = min(n, CEILINGS[cat])
        text = re.sub(rf"(    '{cat}': )[\d_]+,", rf'\g<1>{lowered:_},', text, count=1)
    with open(os.path.join(REPO, SELF), 'w') as f:
        f.write(text)
    return now


def selftest():
    cases = [
        # (file name, text, expected {category: count})
        ('a.md', 'The colour of it.', {'british-markdown': 1}),
        ('a.md', '> The colour of it, quoted.', {}),
        ('a.md', 'See https://example.org/colour for it.', {}),
        ('a.md', 'Post-Spectre, the boundary got dearer.', {}),
        ('a.md', 'Queueing theory, and an axe.', {}),
        # Assembled, so the spelling gate does not read the typos as this file's own.
        ('a.md', 'An ordinary typo, ' + 't' + 'eh, and the `' + 'u' + 're` driver.', {}),
        ('a.rs', 'const CATALOGUE: u8 = 0; fn catalogued_stem() {} struct ParseCatalogue;',
         {'british-other': 3}),
        ('a.md', 'A greyhound and a programmer.', {}),
        ('a.md', 'A grey whilst it is spelt.', {'british-markdown': 3}),
        ('a_colour.md', 'Nothing here.', {'british-markdown': 1}),
        ('a.md', 'It sat three feet from the switch, and 1.5 ft of cable.', {'imperial': 2}),
        # calef, 2026-10-06 (UTC): "foot gun isn't imperial". `a` reads as a quantity, so this is
        # the case the `guns?` lookahead exists for, hyphenated or not.
        ('a.md', 'It is a foot gun, a foot-gun, under the caller\'s feet, on small feet.', {}),
        ('a.md', 'A 3.5-inch drive bay in a 19-inch rack and a 3.5" drive.', {}),
        ('a.md', 'A 12″ ruler, and a "core 3" quote.', {'imperial': 1}),
        ('a.md', 'Two foot\nguns, and an escape \\x1b[?25lb.', {}),
        ('a.md', 'Delegating "may run on core 3" is a copy.', {}),
        ('a.md', '1 in 10 runs, at 70 °F.', {'imperial': 1}),
        ('a.md', 'The milestone is a mile away.', {'imperial': 1}),
    ]
    scratch = tempfile.mkdtemp(prefix='house-style-selftest-')
    failed = 0
    try:
        for i, (name, text, want) in enumerate(cases):
            rel = f'{i}/{name}'
            os.makedirs(os.path.join(scratch, str(i)))
            with open(os.path.join(scratch, rel), 'w') as f:
                f.write(text + '\n')
            hits = census(scratch, [rel])[rel]
            got = {}
            for cat, _, _ in hits:
                got[cat] = got.get(cat, 0) + 1
            if got != want:
                failed += 1
                print(f'house style selftest: {text!r} counted {got}, wanted {want}',
                      file=sys.stderr)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
    if failed:
        return 1
    print(f'house style selftest: {len(cases)} fixtures')
    return 0


REPO = None


def main(argv):
    global REPO
    REPO = (subprocess.run(['git', 'rev-parse', '--show-toplevel'], capture_output=True,
                           text=True).stdout.strip() or os.getcwd())
    os.chdir(REPO)
    if argv and argv[0] == '--selftest':
        return selftest()
    if not argv or argv[0] == '--check':
        now, bad = check()
        if bad:
            print('house style: the tree gained British spelling or imperial units '
                  '(calef, 2026-10-06: American spelling, metric units):', file=sys.stderr)
            for b in bad:
                print(f'  {b}', file=sys.stderr)
            print(f'\nFix the words (`python3 {SELF} --list PATH` shows each). A quotation goes '
                  f'on its own `>` line; a name somebody else coined goes in EXTERNAL_NAMES with '
                  f'its source. See the header of {SELF}.', file=sys.stderr)
            return 1
        print('house style: ' + ', '.join(f'{n:,} {LABELS[c]}' for c, n in now.items())
              + ' (ceilings held; the sweep drives them to zero)')
        return 0
    if argv[0] == '--list':
        paths = argv[1:] or tracked()
        for p, hits in sorted(census(REPO, [x for x in paths if in_scope(x)]).items()):
            for cat, line, word in hits:
                print(f'{p}:{line or "name"}: {cat}: {word}')
        return 0
    if argv[0] == '--bank':
        now = bank()
        print('house style: ceilings now ' + ', '.join(f'{c} {min(n, CEILINGS[c]):,}'
                                                      for c, n in now.items()))
        return 0
    print(__doc__.split('\n\n')[2], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
