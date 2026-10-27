"""The unit-symbol ratchet: Rust identifiers with a unit symbol the naming ruling does not allow.

Ruled 2026-10-27 (UTC), recorded in design/naming/spelled-out-rulings.md under "Unit symbols in
names". A name uses a unit symbol only from the allowed list: `ns` and `ms`; `kib`, `mib` and
`gib`; `hz`, `khz` and `mhz`. A symbol that reads two ways is spelled out (`microseconds`, not
`us`; `seconds`, not `s` or `sec`; `minutes`; `meters`), and `bytes` stays a word. `nanos` becomes
`ns` and `millis` becomes `ms`. New code spells the unit out; this gate makes sure nothing new
arrives. It is rung 2 of the AGENTS.md ladder.

    python3 helpers/unit_symbols.py --check     # what script/lint runs
    python3 helpers/unit_symbols.py --count     # the hits by family
    python3 helpers/unit_symbols.py --list [PATH..]  # every counted hit, path:line: family: identifier
    python3 helpers/unit_symbols.py --unclear   # lookalikes the ruling does not settle, not counted
    python3 helpers/unit_symbols.py --bank      # lower CEILING to the tree; never raises
    python3 helpers/unit_symbols.py --selftest  # the traps, as fixtures

Name: provisional, minted by lane/unit-symbols-sweep-1 on 2026-10-27. A shared python module under
`helpers/`, which `script/names` puts out of its own scope, so its provenance is this paragraph.
calef names things; expect this to change.

**What counts.** An identifier in a `.rs` file that has one of these as a whole component, in any
casing (components split at `_`, at digits and at CamelCase boundaries, as in
helpers/cap_abbreviation.py, whose lexer this reads through):

- `nanos` (family `nanos`, becomes `ns`) and `millis` (family `millis`, becomes `ms`);
- `micros`, `us` and `usec` (family `micros`, becomes `microseconds`);
- `sec` and `secs` (family `sec`, becomes `seconds`);
- `mins` (family `mins`, becomes `minutes`), and `min` straight after `per` (`SECS_PER_MIN`);
- a bare `s` straight after `per` or a number (`kib_per_second`, `within_30_s`; family `s`, becomes
  `seconds`). A bare `s` anywhere else is a variable, a mode-bit prefix or a possessive, and the
  ruling does not reach it.

Comments and string literals are stripped first: the ruling covers identifiers.

**Permanent exclusions, each for a stated reason, not a backlog:**

- `vendor/`, `patches/` and `target/`: somebody else's code.
- `STD`: Rust standard-library methods (`as_nanos`, `from_secs`), which keep their spelling.
- `KEPT`: an identifier where the component is not a unit, or mirrors an external specification.
  `US` is x86 paging's User/Supervisor bit, `sec` in `kernel/src/pci.rs` is a bridge's secondary
  bus, `sec1` is the SEC 1 curve encoding, `SEC` in a clock-and-reset name is security. `tv_sec`,
  `tv_nsec` and `tv_usec` are the POSIX `timespec` and `timeval` fields.

**Not counted, listed by `--unclear`:** `nsec`, `msec`, `kb`, `mb`, `gb`, `tb`, `bps`, `ppm`. The
ruling does not settle them, so the sweep left them and reported them.

**Two halves, house_style.py's shape.** A count over `CEILING` fails, which guards `main`. A branch
is also held to its merge base: summed over the `.rs` files it changed, the count may not grow.

BUGS:
- `ns` is allowed by the ruling, and a `nanos` renamed to `ns` can collide with an existing `ns` in
  the same scope; the compiler catches a duplicate binding but not a silent shadow.
- The lexer is a lexer, not a parser. A macro that builds an identifier from a string is not read.
"""

import os
import re
import subprocess
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import cap_abbreviation as lexer  # noqa: E402

SELF = 'helpers/unit_symbols.py'

# The count on 2026-10-27, lowered by the sweep's `--bank`. Never raise it by hand: a new unit
# symbol is spelled out, not admitted.
CEILING = 289

FAMILY = {
    'nanos': 'nanos', 'millis': 'millis',
    'micros': 'micros', 'us': 'micros', 'usec': 'micros', 'usecs': 'micros',
    'sec': 'sec', 'secs': 'sec',
    'mins': 'mins',
}
UNCLEAR = {'nsec', 'nsecs', 'msec', 'msecs', 'kb', 'mb', 'gb', 'tb', 'bps', 'kbps', 'mbps', 'gbps', 'ppm'}

STD = {
    'as_secs', 'as_millis', 'as_micros', 'as_nanos', 'as_secs_f32', 'as_secs_f64',
    'subsec_millis', 'subsec_micros', 'subsec_nanos',
    'from_secs', 'from_millis', 'from_micros', 'from_nanos', 'from_secs_f32', 'from_secs_f64',
    'checked_as_nanos', 'abs_diff_secs',
}

_POSIX = 'a field of the POSIX timespec or timeval'
_SEC1 = 'SEC 1, the elliptic-curve point encoding'
_SECURITY = 'a security-domain clock or reset a vendor named'
_BUS = 'the secondary bus of a PCI bridge'
KEPT = {
    'crates/paging/src/x86_64.rs': ({'US'}, 'the x86 paging User/Supervisor bit'),
    'kernel/src/pci.rs': ({'sec', 'sec_sts'}, _BUS),
    'c_library/src/lib.rs': ({'tv_sec', 'tv_nsec', 'tv_usec'}, _POSIX),
    'crates/e1000e/src/pch/sequence.rs':
        ({'FEXTNVM3_PHY_CFG_COUNTER_50MSEC'}, 'a register field, spelled as Linux spells it'),
    'cryptography_provider/src/lib.rs': ({'Sec1Point', 'ToSec1Point', 'from_sec1_bytes', 'to_sec1_point', 'sec1'}, _SEC1),
    'cryptography_exerciser/src/main.rs': ({'Sec1Point', 'ToSec1Point', 'from_sec1_bytes', 'to_sec1_point', 'sec1'}, _SEC1),
}
# Names kept wherever they appear, by shape: SEC 1 and a vendor's security-domain clocks and resets.
KEPT_SHAPES = (
    re.compile(r'[Ss]ec1'),
    re.compile(r'^(STGCLK|STGRST|VENDOR)_SEC_|^VENDOR_SEC_'),
    re.compile(r'^tv_(sec|nsec|usec)$'),
)

EXCLUDED_PREFIXES = ('vendor/', 'patches/', 'target/')


def in_scope(path):
    return path.endswith('.rs') and not path.startswith(EXCLUDED_PREFIXES)


def family_of(identifier):
    """The family names a unit symbol this identifier uses, in order of appearance."""
    comps = [c.lower() for c in lexer.components(identifier)]
    found = []
    for i, c in enumerate(comps):
        prev = comps[i - 1] if i else ''
        if c in FAMILY:
            found.append(FAMILY[c])
        elif c == 'min' and prev == 'per':
            found.append('mins')
        elif c == 's' and i and (prev == 'per' or _after_number(identifier, c, i)):
            found.append('s')
    return found


def _after_number(identifier, comp, index):
    """True when the component at `index` follows a digit run, as `within_30_s` does."""
    parts = [p for p in re.split(r'_', identifier) if p != '']
    for j in range(1, len(parts)):
        if parts[j].lower() == 's' and parts[j - 1].isdigit():
            return True
    return False


SPELLED = {'nanos': 'ns', 'millis': 'ms', 'micros': 'microseconds', 'us': 'microseconds',
           'usec': 'microseconds', 'usecs': 'microseconds', 'secs': 'seconds', 'sec': 'second',
           'mins': 'minutes'}
_COMPONENT = re.compile(r'[A-Z]+(?![a-z])|[A-Z]?[a-z]+|[A-Z]+')


def respell(identifier):
    """The identifier the ruling asks for in place of this one; the same string when none."""
    out, last = [], 0
    comps = [(m.start(), m.end(), m.group(0)) for m in _COMPONENT.finditer(identifier)]
    parts = [c[2].lower() for c in comps]
    for i, (a, b, text) in enumerate(comps):
        low, prev, new = text.lower(), (parts[i - 1] if i else ''), None
        if low in SPELLED:
            new = SPELLED[low]
        elif low == 'min' and prev == 'per':
            new = 'minute'
        elif low == 's' and i:
            if prev == 'per':
                new = 'second'
            elif _after_number(identifier, text, i):
                new = 'seconds'
        if new:
            if text.isupper():
                new = new.upper()
            elif text[0].isupper():
                new = new.capitalize()
            out.append(identifier[last:a] + new)
            last = b
    out.append(identifier[last:])
    return ''.join(out)


def counts_as_hit(identifier, path=''):
    if identifier in STD:
        return False
    if identifier in KEPT.get(path, (set(), ''))[0]:
        return False
    if any(p.search(identifier) for p in KEPT_SHAPES):
        return False
    return True


def file_hits(text, path=''):
    out = []
    for line, word in lexer.identifiers(text):
        fams = family_of(word)
        if fams and counts_as_hit(word, path):
            out.append((line, fams[0], word))
    return out


def unclear_hits(text):
    return [(line, word) for line, word in lexer.identifiers(text)
            if any(c.lower() in UNCLEAR for c in lexer.components(word))]


# --- counting -----------------------------------------------------------------------------------

REPO = None


def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True, cwd=REPO)
    return r.stdout.decode(errors='replace') if r.returncode == 0 else None


def tracked():
    return [f for f in lexer.tracked() if in_scope(f)]


def census(paths):
    out = {}
    for p in paths:
        t = lexer.read(p)
        if t is not None:
            out[p] = file_hits(t, p)
    return out


def total(by_path):
    return sum(len(h) for h in by_path.values())


def by_family(by_path):
    fam = {}
    for hits in by_path.values():
        for _, f, _ in hits:
            fam[f] = fam.get(f, 0) + 1
    return fam


def growth(base):
    """Net change, base to working tree, over the `.rs` files this branch changed."""
    old_paths, new_paths = set(), set()
    for row in (git('diff', '--name-status', '-M', base) or '').splitlines():
        cells = row.split('\t')
        status, paths = cells[0], cells[1:]
        if status[0] in 'RC':
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
    where = {}
    for p in old_paths:
        if in_scope(p):
            where[p] = where.get(p, 0) - len(file_hits(git('show', f'{base}:{p}') or '', p))
    for p in new_paths:
        if in_scope(p) and os.path.isfile(p):
            where[p] = where.get(p, 0) + len(file_hits(lexer.read(p) or '', p))
    return sum(where.values()), where


def check():
    bad = []
    now = total(census(tracked()))
    if now > CEILING:
        bad.append(f'{now:,} identifiers use a unit symbol the naming ruling does not allow, over '
                   f'the ceiling of {CEILING:,} in {SELF}. If this branch adds none, main moved '
                   f'past it: merge main, and set the ceiling to the merged count in this change, '
                   f'saying why')
    base = lexer.merge_base()
    if base:
        net, where = growth(base)
        if net > 0:
            grew = sorted(p for p, d in where.items() if d > 0)
            bad.append(f'this branch adds {net} net, in {", ".join(grew)}')
    return now, bad


def bank():
    now = total(census(tracked()))
    text = open(SELF).read()
    text = re.sub(r'(?m)^(CEILING = )[\d_]+$', rf'\g<1>{min(now, CEILING):_}', text, count=1)
    with open(SELF, 'w') as f:
        f.write(text)
    return now


def selftest():
    def names(src, path=''):
        return [w for _, _, w in file_hits(src, path)]
    cases = [
        ('let now_microseconds = 1; let secs = 2; let nanos = 3; let millis = 4; let micros = 5;',
         ['now_microseconds', 'secs', 'nanos', 'millis', 'micros']),
        ('const NANOS_PER_SEC: u64 = 1; const SECS_PER_MIN: u64 = 60; let kib_per_second = 1;',
         ['NANOS_PER_SEC', 'SECS_PER_MIN', 'kib_per_second']),
        ('let within_30_s = 1; let TICKS_PER_SEC = 1; let mins = 2; let tv_usec = 0;',
         ['within_30_s', 'TICKS_PER_SEC', 'mins']),
        # The allowed list, the spelled-out names and the std methods are never hits.
        ('let a_ns = 1; let b_ms = 2; let c_kib = 3; let hz = 4; let seconds = 5; let minutes = 6; '
         'let microseconds = 7; d.as_nanos(); d.as_secs(); Duration::from_millis(1);', []),
        # A bare `s`, a mode-bit prefix and a possessive are not units.
        ('let s = 1; const S_IFDIR: u32 = 1; let s_context = 2; let saw_s = false; let ds_s = 1;', []),
        # `min` is a minimum unless it follows `per`.
        ('let min = 1; const MIN_ALIGN: u64 = 1;', []),
        ('let (sec, sub) = f();', ['sec']),
        ('// secs in a comment\nlet x = "now_microseconds";', []),
        ('let sec1 = 1; const STGCLK_SEC_AHB: u32 = 1; let tv_sec = 1; let Sec1Point = 2;', []),
    ]
    failed = 0
    for src, want in cases:
        got = names(src)
        if got != want:
            failed += 1
            print(f'unit selftest: {src!r} counted {got}, wanted {want}', file=sys.stderr)
    if names('let sec = 1; let sec_sts = 2;', 'kernel/src/pci.rs'):
        failed += 1
        print('unit selftest: KEPT in kernel/src/pci.rs counted', file=sys.stderr)
    if names('const US: u64 = 1;', 'crates/paging/src/x86_64.rs'):
        failed += 1
        print('unit selftest: KEPT US counted', file=sys.stderr)
    if [w for _, w in unclear_hits('let nsec = 1; let kb = 2; let ns = 3;')] != ['nsec', 'kb']:
        failed += 1
        print('unit selftest: unclear lookalikes', file=sys.stderr)
    for old, new in (('NANOS_PER_SEC', 'NS_PER_SECOND'), ('SECS_PER_MIN', 'SECONDS_PER_MINUTE'),
                     ('unix_nanos', 'unix_ns'), ('delay_microseconds', 'delay_microseconds'),
                     ('within_30_s', 'within_30_seconds'), ('cpu_millis', 'cpu_ms'),
                     ('hz', 'hz')):
        if respell(old) != new:
            failed += 1
            print(f'unit selftest: respell({old!r}) is {respell(old)!r}, wanted {new!r}', file=sys.stderr)
    if failed:
        return 1
    print(f'unit selftest: {len(cases) + 4} fixtures')
    return 0


def main(argv):
    global REPO
    REPO = (subprocess.run(['git', 'rev-parse', '--show-toplevel'], capture_output=True,
                           text=True).stdout.strip() or os.getcwd())
    os.chdir(REPO)
    lexer.REPO = REPO
    if argv and argv[0] == '--selftest':
        return selftest()
    if not argv or argv[0] == '--check':
        now, bad = check()
        if bad:
            print('unit: the tree gained identifiers with a unit symbol the ruling does not allow '
                  '(2026-10-27 UTC):', file=sys.stderr)
            for b in bad:
                print(f'  {b}', file=sys.stderr)
            print(f'\nSpell it out (`seconds`, `microseconds`, `minutes`) or use `ns`, `ms`; '
                  f'`python3 {SELF} --list PATH` shows each hit. See '
                  f'design/naming/spelled-out-rulings.md and the header of {SELF}.',
                  file=sys.stderr)
            return 1
        print(f'unit: {now:,} identifiers use a disallowed unit symbol (ceiling held)')
        return 0
    if argv[0] == '--count':
        c = census(tracked())
        fam = by_family(c)
        for f in sorted(fam, key=lambda k: -fam[k]):
            print(f'{f}\t{fam[f]}')
        print(f'total\t{total(c)}')
        return 0
    if argv[0] == '--list':
        paths = argv[1:] or tracked()
        for p, hits in sorted(census([x for x in paths if in_scope(x)]).items()):
            for line, fam, word in hits:
                print(f'{p}:{line}: {fam}: {word}')
        return 0
    if argv[0] == '--unclear':
        for p in tracked():
            t = lexer.read(p)
            for line, word in unclear_hits(t or ''):
                print(f'{p}:{line}: {word}')
        return 0
    if argv[0] == '--bank':
        now = bank()
        print(f'unit: ceiling now {min(now, CEILING):,}')
        return 0
    print(__doc__.split('\n\n')[2], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
