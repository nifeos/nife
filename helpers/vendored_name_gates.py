"""The naming gates nife runs on the template's words: what nife adds beside the vendored copies.

The template (nifeos/crate-workflows) owns, once for every repository, the words a name spells out
(`[spelled_out]`), the unit symbols a name never carries (`[units]`) and the Rust lexer that reads a
name. nife vendors them at the commit `vendor/crate-workflows.pin` names, into
`vendor/crate-workflows/helpers/`, and edits none of them: a change to a word goes to the template
and the pin moves to it. This file is the nife side. The template has no counterpart for what a
tree with a history needs, so these stay here:

- `CAP_CEILING` and `UNIT_CEILING`, and the net-growth half that holds a branch to its merge base;
- `CAP_KEPT`, `UNIT_KEPT`, `UNIT_KEPT_SHAPES` and `UNIT_STD`, the identifiers a rule does not reach;
- `UNIT_COUNTED`, the units the unit ratchet counts so far, a narrowing of the template's table;
- the check that design/naming.md and the template's word list say the same thing;
- `PROGRAM_KEPT`, `PROTOCOL_KEPT` and `PROSE_EXEMPT`, what the template's service-model and
  contract-name checks do not reach in this tree: the program and crate names, and the files, that
  keep a spelling on purpose, and the glob that proves the contract-crate check judged something.

    python3 helpers/vendored_name_gates.py cap --check      # what script/lint runs
    python3 helpers/vendored_name_gates.py cap --list [PATH..]  # every counted hit, path:line: identifier
    python3 helpers/vendored_name_gates.py cap --bank       # lower the ceiling to the tree; never raises
    python3 helpers/vendored_name_gates.py cap --selftest   # the traps, as fixtures
    python3 helpers/vendored_name_gates.py unit --check     # what script/lint runs
    python3 helpers/vendored_name_gates.py unit --count     # the hits by family
    python3 helpers/vendored_name_gates.py unit --list [PATH..]  # every counted hit, path:line: family: identifier
    python3 helpers/vendored_name_gates.py unit --unclear   # table units not yet counted, not counted
    python3 helpers/vendored_name_gates.py unit --bank      # lower the ceiling to the tree; never raises
    python3 helpers/vendored_name_gates.py unit --selftest  # the traps, as fixtures
    python3 helpers/vendored_name_gates.py words --check    # design/naming.md and the word list agree
    python3 helpers/vendored_name_gates.py words --selftest # that check, as fixtures
    python3 helpers/vendored_name_gates.py service --check  # program and crate names, and the prose
    python3 helpers/vendored_name_gates.py service --selftest # that check, as fixtures

**The cap ratchet** counts Rust identifiers with a `cap` or `caps` component, in any casing, as a
whole component: `cap`, `SEND_CAP`, `CapKind`, `MCap`. Components split at `_`, at digits and at
CamelCase boundaries, so `capacity`, `capture` and `keycaps` never match. Comments and string
literals are not identifiers. The ruling is that every identifier spells `capability` out, and the
ceiling is 0. Unlike the template's check, which judges the names a crate defines, this counts every
identifier the tree writes, a use as well as a definition, because the tree's history is a sweep of
all of them. The two words are the `cap` and `caps` rows of `[spelled_out]`.

**The unit ratchet** counts Rust identifiers that carry a unit symbol the rule refuses, the units in
`UNIT_COUNTED` and a bare `s` or `min` where the template's `[units_after_per]` and
`[units_after_number]` say a unit. The template's table is wider than the count: `ns`, `ms`, `hz`
and the byte and bit-rate symbols (`kib`, `mib`, `gib` among them) are refused by the rule and by
the template's own check, and this ratchet has not counted them yet, because counting them takes a
sweep of the names that carry them. `--unclear` lists them. Bringing a unit into `UNIT_COUNTED`
raises the count, so it comes with the sweep that brings the count back to zero.

**Two halves.** A count over the ceiling fails, which guards `main`. A branch is also held to its
merge base: summed over the `.rs` files it changed, the count may not grow.

**Permanent exclusions, each for a stated reason, not a backlog:**

- `vendor/`, `patches/` and `target/`: somebody else's code.
- `CAP_KEPT`: a `cap` that is the English word for a limit or a capacity, or a register a hardware
  specification named (VT-d's `CAP`, NVMe's `CAP`, the RISC-V IOMMU's `caps`, IEEE 802.3's ability
  bits). An entry is removed only by renaming what it names. Keyed by file, because `CAP` means a
  buffer's size in one file and a capability in another.
- `UNIT_STD`: Rust standard-library methods (`as_nanos`, `from_secs`) and the smoltcp time methods
  (`total_millis`), which keep their owner's spelling.
- `UNIT_KEPT`: an identifier where the component is not a unit, or mirrors an external
  specification. `US` is x86 paging's User/Supervisor bit, `sec` in `kernel/src/pci.rs` is a
  bridge's secondary bus, `sec1` is the SEC 1 curve encoding, `SEC` in a clock-and-reset name is
  security. `tv_sec`, `tv_nsec` and `tv_usec` are the POSIX `timespec` and `timeval` fields.
  `NANOS_PER_SEC`, `wall_nanos` and `offset_nanos` in `clock_contract` are the names the std overlay
  under `patches/` reads, and no sweep rewrites `patches/`.

Name: provisional. The ratchets are `cap_abbreviation.py` (2026-10-06, lane/no-cap-abbreviation) and
`unit_symbols.py` (2026-10-27, lane/unit-symbols-sweep-1) in one file, since the template took their
words and their lexer. A shared python module under `helpers/`, which `script/names` puts out of its
own scope, so this paragraph is the record. calef names things; expect this to change.

BUGS:
- `ncaps`, `cptr` and other contractions with no separator are one component and do not match.
- A lexer, not a parser (the template's, `vendor/crate-workflows/helpers/rust_names.py`). A macro
  that builds an identifier from a string is not read.
- `respell` still writes `nanos` as `ns` and `millis` as `ms`, which the rule refuses; it only
  recognizes a rename inside `helpers/moved_rows.py`, and a lane that sweeps those units fixes it.
- The template has no `proto` row, so design/naming.md's prose names a word the list does not hold
  (`PROSE_ONLY_WORDS`).
"""

import os
import re
import subprocess
import sys

sys.dont_write_bytecode = True

SELF = 'helpers/vendored_name_gates.py'
TEMPLATE_HELPERS = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                                'vendor', 'crate-workflows', 'helpers')
sys.path.insert(0, TEMPLATE_HELPERS)

import contract_names  # noqa: E402  (vendored; a contract crate is spelled `_contract`)
import rust_names  # noqa: E402  (vendored; the lexer and the word loader)
import service_model  # noqa: E402  (vendored; no Unix service model in a name or in prose)
import spelled_out  # noqa: E402  (vendored; the spelled-out check)
import unit_symbols as template_units  # noqa: E402  (vendored; the unit-symbol check)

WORDS = rust_names.load_words(os.path.join(TEMPLATE_HELPERS, 'naming_words.toml'))
SPELLED_OUT = WORDS['spelled_out']
UNITS = WORDS['units']
UNITS_AFTER_PER = WORDS['units_after_per']
UNITS_AFTER_NUMBER = WORDS['units_after_number']

# The template's `cap` and `caps` rows: the one word nife's `cap` ratchet counts.
CAP_WORDS = ('cap', 'caps')

REPO = None
EXCLUDED_PREFIXES = ('vendor/', 'patches/', 'target/')

# The counts on 2026-10-06 (cap, from 2,890) and 2026-10-27 (unit), lowered by each sweep's
# `--bank`. Never raise either by hand: a new `cap` or unit symbol is spelled out, not admitted.
CAP_CEILING = 0
UNIT_CEILING = 0


def in_scope(path):
    return path.endswith('.rs') and not path.startswith(EXCLUDED_PREFIXES)


# --- the template's words, read through nife's counting ---------------------------------------

def identifiers(text):
    """[(line, identifier)] for every identifier in Rust source, comments and strings skipped. A
    lifetime or a label (`'a`, `'outer`) is an identifier after its quote."""
    return [(t.line, t.text) for t in rust_names.tokens(text) if t.kind in ('ident', 'lifetime')]


def components(identifier):
    """`SEND_CAP` -> send, cap; `CapKind` -> cap, kind; `cap2` -> cap. Lowercase, and the digit runs
    the template keeps as components are dropped, as the ratchets always counted."""
    return [c for c in rust_names.components(identifier) if not c.isdigit()]


def spelled_out_hits(name):
    """[(word, replacement)] the template's spelled-out check refuses in `name`, in order."""
    return spelled_out.hits_of_words(WORDS)(rust_names.Definition(name, 'name', 1))


def abbreviates(identifier):
    return any(p in CAP_WORDS for p in components(identifier))


# --- the cap exclusions -----------------------------------------------------------------------

# Identifiers that keep `cap`, by file, each group with its reason. See the header before adding one.
_CEILING = 'cap is the English word for a limit or a capacity here, not an abbreviation'
_REGISTER = 'mirrors a register or field a hardware specification named'
CAP_KEPT = {
    'components/src/printenv.rs': ({'CAP'}, _CEILING),
    'crates/calendar/src/lib.rs': ({'FMT_CAP'}, _CEILING),
    'crates/calendar/src/tests.rs': ({'FMT_CAP'}, _CEILING),
    'kernel/src/stack.rs': ({'CAP'}, _CEILING),
    'kernel/src/arch/x86_64/timer.rs': ({'CALIBRATION_WINDOW_CAP'}, _CEILING),
    'redoxfs_server/src/bin/second_mount.rs': ({'cap', 'heap_cap', 'HIT_CAP', 'cap_mib'}, _CEILING),
    'crates/swish/src/say/tests.rs': ({'the_heap_cap_is_named_in_kibibytes'}, _CEILING),
    'crates/domain_name_system/tests/replies.rs':
        ({'addresses_past_the_cap_are_dropped_and_the_rest_kept'}, _CEILING),
    'crates/video_terminal/src/vt/tests.rs': ({'scrolls_report_movement_and_cap_at_the_screen'}, _CEILING),
    'xtask/src/board.rs': ({'cap_given'}, _CEILING),
    # `write_field` truncates to a capacity, the grant-argument test fills the page to its
    # `CAPACITY`, and the syscall fuzzer's sweep stops at a guest-time deadline.
    'crates/environment_contract/src/lib.rs': ({'cap'}, _CEILING),
    'crates/grant_plan/src/parsing/tests.rs': ({'cap'}, _CEILING),
    'system_tests/src/user/syscall_fuzzer_tests.rs': ({'cap'}, _CEILING),
    # `PRINT_CAP` bounds a diagnostic's lines; the test's `caps` is the verb "limits". Two files since
    # milestone 840 (the scheduler file is split along its seams) moved each out of `sched.rs`.
    'kernel/src/sched/canary.rs': ({'PRINT_CAP'}, _CEILING),
    'kernel/src/sched/tests.rs': ({'a_spawn_quota_caps_live_children_and_replenishes_on_reap'},
                                  _CEILING),
    # Intel VT-d, section 10.4.2: the Capability Register and its fields, and its value.
    'kernel/src/arch/x86_64/iommu.rs': ({'CAP', 'cap', 'CAP_FRO_MASK', 'CAP_FRO_SHIFT', 'CAP_ND_MASK',
                                         'CAP_PHMR', 'CAP_PLMR', 'CAP_RWBF', 'CAP_SAGAW_48BIT'},
                                        _REGISTER),
    # The RISC-V IOMMU specification's `capabilities` register, which it abbreviates `caps`.
    'kernel/src/arch/riscv64/iommu.rs': ({'CAPS', 'caps', 'CAP_MSI_FLAT', 'CAP_SV39'}, _REGISTER),
    # NVMe base specification, section 3.1: Controller Capabilities, `CAP`.
    'kernel/src/non_volatile_memory_express.rs': ({'CAP', 'Cap', 'cap'}, _REGISTER),
    'crates/non_volatile_memory_express/src/lib.rs':
        ({'CAP', 'Cap', 'cap', 'cap_fields_decode_from_their_spec_positions'}, _REGISTER),
    # IEEE 802.3 clause 28 ability bits, spelled as Linux's e1000e spells them.
    'crates/e1000e/src/platform_controller_hub/sequence.rs': ({'NWAY_AR_10T_HD_CAPS', 'NWAY_AR_10T_FD_CAPS',
                                           'NWAY_AR_100TX_HD_CAPS', 'NWAY_AR_100TX_FD_CAPS',
                                           'CR_1000T_HD_CAPS', 'CR_1000T_FD_CAPS'}, _REGISTER),
}


def cap_hits(text, path=''):
    kept = CAP_KEPT.get(path, (set(), ''))[0]
    return [(line, word) for line, word in identifiers(text)
            if abbreviates(word) and word not in kept]


# --- the unit exclusions and the units counted so far -----------------------------------------

# The template's table is wider than the count. These are the entries of `[units]` the ratchet
# counts today; every other entry is listed by `--unclear` until a sweep brings it in.
UNIT_COUNTED = ('nanos', 'millis', 'micros', 'us', 'usec', 'usecs', 'sec', 'secs', 'mins')
# Lookalikes outside the template's table that the ruling does not settle, listed by `--unclear`.
UNIT_UNSETTLED = {'ppm'}

UNIT_STD = {
    'as_secs', 'as_millis', 'as_micros', 'as_nanos', 'as_secs_f32', 'as_secs_f64',
    'subsec_millis', 'subsec_micros', 'subsec_nanos',
    'from_secs', 'from_millis', 'from_micros', 'from_nanos', 'from_secs_f32', 'from_secs_f64',
    'checked_as_nanos', 'abs_diff_secs',
    # smoltcp's time types
    'total_millis', 'total_micros',
}

_POSIX = 'a field of the POSIX timespec or timeval'
_SEC1 = 'SEC 1, the elliptic-curve point encoding'
_SECURITY = 'a security-domain clock or reset a vendor named'
_BUS = 'the secondary bus of a PCI bridge'
_OVERLAY = 'the field of clock_contract::Reading the std overlay reads under this name'
UNIT_KEPT = {
    'crates/paging/src/x86_64.rs': ({'US'}, 'the x86 paging User/Supervisor bit'),
    'kernel/src/pci.rs': ({'sec', 'sec_sts'}, _BUS),
    'c_library/src/lib.rs': ({'tv_sec', 'tv_nsec', 'tv_usec'}, _POSIX),
    'notes/filesystem-crates-2026-10-07/probes/btrfs-transaction.rs':
        ({'sec'}, 'a field of btrfs_disk::items::Timespec, an external crate'),
    'components/src/clock.rs': ({'offset_nanos'}, _OVERLAY),
    'components/src/date.rs': ({'offset_nanos'}, _OVERLAY),
    'components/src/timetable.rs': ({'offset_nanos'}, _OVERLAY),
    'kernel/src/user/clock_service.rs': ({'offset_nanos'}, _OVERLAY),
    'system_tests/src/user/clock_tests.rs': ({'offset_nanos'}, _OVERLAY),
    # The std overlay under patches/std-nife reads a generated copy of this file by these names, and
    # no sweep rewrites patches/. `offset_nanos` is the field of `Reading` it reads.
    'crates/clock_contract/src/lib.rs':
        ({'NANOS_PER_SEC', 'wall_nanos', 'offset_nanos', 'monotonic_nanos'},
         'the names the std overlay reads'),
    'crates/e1000e/src/platform_controller_hub/sequence.rs':
        ({'FEXTNVM3_PHY_CFG_COUNTER_50MSEC'}, 'a register field, spelled as Linux spells it'),
    'cryptography_provider/src/lib.rs': ({'Sec1Point', 'ToSec1Point', 'from_sec1_bytes', 'to_sec1_point', 'sec1'}, _SEC1),
    'cryptography_exerciser/src/main.rs': ({'Sec1Point', 'ToSec1Point', 'from_sec1_bytes', 'to_sec1_point', 'sec1'}, _SEC1),
}
# Names kept wherever they appear, by shape: SEC 1 and a vendor's security-domain clocks and resets.
UNIT_KEPT_SHAPES = (
    re.compile(r'[Ss]ec1'),
    re.compile(r'^(STGCLK|STGRST|VENDOR)_SEC_|^VENDOR_SEC_'),
    re.compile(r'^tv_(sec|nsec|usec)$'),
)


def family_of(identifier):
    """The family names a unit symbol this identifier uses, in order of appearance. A family is the
    template's replacement for the symbol."""
    comps = components(identifier)
    found = []
    for i, c in enumerate(comps):
        prev = comps[i - 1] if i else ''
        if c in UNIT_COUNTED:
            found.append(UNITS[c])
        elif c == 'min' and prev == 'per' and 'min' in UNITS_AFTER_PER:
            found.append(UNITS_AFTER_PER['min'])
        elif (c == 's' and i and (prev == 'per' or _after_number(identifier))
              and ('s' in UNITS_AFTER_PER or 's' in UNITS_AFTER_NUMBER)):
            found.append(UNITS_AFTER_PER.get('s') or UNITS_AFTER_NUMBER['s'])
    return found


def _after_number(identifier):
    """True when a bare `s` follows a digit run in the identifier, as `within_30_s` does."""
    parts = [p for p in identifier.split('_') if p != '']
    return any(parts[j].lower() == 's' and parts[j - 1].isdigit() for j in range(1, len(parts)))


# What a rename of a unit symbol looks like to helpers/moved_rows.py: a row follows its item to
# the respelled name. These are the respellings lanes made, not the rule's; see BUGS.
_RESPELLED = {'nanos': 'nanoseconds', 'millis': 'milliseconds', 'micros': 'microseconds',
              'us': 'microseconds', 'usec': 'microseconds', 'usecs': 'microseconds',
              'secs': 'seconds', 'sec': 'second', 'mins': 'minutes', 'ns': 'nanoseconds',
              'ms': 'milliseconds', 'hz': 'hertz', 'khz': 'kilohertz', 'mhz': 'megahertz',
              'ghz': 'gigahertz', 'kib': 'kibibytes', 'mib': 'mebibytes', 'gib': 'gibibytes'}
_COMPONENT = re.compile(r'[A-Z]+(?![a-z])|[A-Z]?[a-z]+|[A-Z]+')


def respell(identifier):
    """The identifier a rename of a unit symbol writes; the same string when none."""
    out, last = [], 0
    comps = [(m.start(), m.end(), m.group(0)) for m in _COMPONENT.finditer(identifier)]
    parts = [c[2].lower() for c in comps]
    for i, (a, b, text) in enumerate(comps):
        low, prev, new = text.lower(), (parts[i - 1] if i else ''), None
        if low in _RESPELLED:
            new = _RESPELLED[low]
        elif low == 'min' and prev == 'per':
            new = 'minute'
        elif low == 's' and i:
            if prev == 'per':
                new = 'second'
            elif _after_number(identifier):
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
    if identifier in UNIT_STD:
        return False
    if identifier in UNIT_KEPT.get(path, (set(), ''))[0]:
        return False
    return not any(p.search(identifier) for p in UNIT_KEPT_SHAPES)


def unit_hits(text, path=''):
    out = []
    for line, word in identifiers(text):
        fams = family_of(word)
        if fams and counts_as_hit(word, path):
            out.append((line, fams[0], word))
    return out


def unclear_hits(text):
    unclear = (set(UNITS) - set(UNIT_COUNTED)) | UNIT_UNSETTLED
    return [(line, word) for line, word in identifiers(text)
            if any(c in unclear for c in components(word))]


# --- counting, shared by both ratchets --------------------------------------------------------

def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True, cwd=REPO)
    return r.stdout.decode(errors='replace') if r.returncode == 0 else None


def read(path):
    try:
        with open(path, encoding='utf-8') as f:
            return f.read()
    except (UnicodeDecodeError, OSError):
        return None


def tracked():
    files = git('ls-files', '-z', '--cached', '--others', '--exclude-standard') or ''
    return [f for f in files.split('\0')
            if f and in_scope(f) and os.path.isfile(f) and not os.path.islink(f)]


def census(paths, file_hits):
    out = {}
    for p in paths:
        t = read(p)
        if t is not None:
            out[p] = file_hits(t, p)
    return out


def total(by_path):
    return sum(len(h) for h in by_path.values())


def merge_base():
    base = (git('merge-base', 'HEAD', 'origin/main') or '').strip()
    head = (git('rev-parse', 'HEAD') or '').strip()
    return base if base and base != head else None


def growth(base, file_hits):
    """Net change, base to working tree, over the `.rs` files this branch changed: (net, {path: d})."""
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
            where[p] = where.get(p, 0) + len(file_hits(read(p) or '', p))
    return sum(where.values()), where


def check(file_hits, ceiling, over):
    """(the count now, the failures): over the ceiling, or a branch that adds any net."""
    bad = []
    now = total(census(tracked(), file_hits))
    if now > ceiling:
        bad.append(over.format(now=now, ceiling=ceiling))
    base = merge_base()
    if base:
        net, where = growth(base, file_hits)
        if net > 0:
            grew = sorted(p for p, d in where.items() if d > 0)
            bad.append(f'this branch adds {net} net, in {", ".join(grew)}')
    return now, bad


def bank(file_hits, constant, ceiling):
    now = total(census(tracked(), file_hits))
    text = open(SELF).read()
    text = re.sub(rf'(?m)^({constant} = )[\d_]+$', rf'\g<1>{min(now, ceiling):_}', text, count=1)
    with open(SELF, 'w') as f:
        f.write(text)
    return now


# --- the cap ratchet --------------------------------------------------------------------------

def cap_selftest():
    cases = [
        # (Rust source, the identifiers that must count)
        ('fn f(cap: u64, caps: &[Cap]) -> CAP {}', ['cap', 'caps', 'Cap', 'CAP']),
        ('const SEND_CAP: u64 = 1; let interrupt_cap = cap_slot; struct CapKind; struct MCap;',
         ['SEND_CAP', 'interrupt_cap', 'cap_slot', 'CapKind', 'MCap']),
        ('let cap2 = x; let caps3 = y;', ['cap2', 'caps3']),
        # The false positives the ruling's wording invites.
        ('fn f(capacity: usize, capture: bool, escape: u8, capable: bool, keycaps: u8, '
         'Capability: u8, CAPACITY: u8, SdHighCapacity: u8, CappedHeap: u8, ncaps: u8) {}', []),
        # The English word for a ceiling, in a comment or a string, is not an identifier.
        ('// a cap on lane count\n/* the cap /* nested cap */ cap */ let x = "cap"; '
         'let y = r#"cap "quoted" cap"#; let z = b"cap"; let c = \'c\';', []),
        # A lifetime, a raw identifier, a char literal that looks like a lifetime.
        ("fn f<'cap>(r#cap: &'cap u8) { let q = '\\''; let cap = 1; }", ['cap', 'cap', 'cap', 'cap']),
        ('let n = 0x1cap; let m = 1..cap;', ['cap']),
    ]
    failed = 0
    for word in CAP_WORDS:
        if word not in SPELLED_OUT:
            failed += 1
            print(f'cap selftest: the template word list no longer holds `{word}`', file=sys.stderr)
    kept = [w for _, w in cap_hits('const CAP: usize = 96; let cap = 1;', 'components/src/printenv.rs')]
    if kept != ['cap']:
        failed += 1
        print(f'cap selftest: CAP_KEPT in printenv.rs counted {kept}, wanted [\'cap\']', file=sys.stderr)
    for src, want in cases:
        got = [w for _, w in cap_hits(src)]
        if got != want:
            failed += 1
            print(f'cap selftest: {src!r} counted {got}, wanted {want}', file=sys.stderr)
    lines = [ln for ln, _ in cap_hits('/* a\nb */\nlet s = "x\ny";\nlet cap = 1;')]
    if lines != [5]:
        failed += 1
        print(f'cap selftest: line numbers {lines}, wanted [5]', file=sys.stderr)
    if failed:
        return 1
    print(f'cap selftest: {len(cases) + 2} fixtures')
    return 0


def cap_main(argv):
    if argv and argv[0] == '--selftest':
        return cap_selftest()
    if not argv or argv[0] == '--check':
        now, bad = check(cap_hits, CAP_CEILING,
                         '{now:,} identifiers abbreviate capability as cap, over the ceiling of '
                         f'{{ceiling:,}} in {SELF}. If this branch adds none, main moved past it: '
                         'rebase, and set the ceiling to the merged count in this change, saying why')
        if bad:
            print('cap: the tree gained identifiers that abbreviate capability as cap '
                  '(calef, 2026-10-06: "I don\'t think we should abbreviate capability as cap."):',
                  file=sys.stderr)
            for b in bad:
                print(f'  {b}', file=sys.stderr)
            print(f'\nSpell it `capability` (`python3 {SELF} cap --list PATH` shows each hit). See '
                  f'design/naming/acronyms-and-abbreviations.md and the header of {SELF}.',
                  file=sys.stderr)
            return 1
        print(f'cap: {now:,} identifiers abbreviate capability as cap (ceiling held)')
        return 0
    if argv[0] == '--list':
        paths = argv[1:] or tracked()
        for p, hits in sorted(census([x for x in paths if in_scope(x)], cap_hits).items()):
            for line, word in hits:
                print(f'{p}:{line}: {word}')
        return 0
    if argv[0] == '--bank':
        now = bank(cap_hits, 'CAP_CEILING', CAP_CEILING)
        print(f'cap: ceiling now {min(now, CAP_CEILING):,}')
        return 0
    print(__doc__.split('\n\n')[2], file=sys.stderr)
    return 2


# --- the unit ratchet -------------------------------------------------------------------------

def unit_selftest():
    def names(src, path=''):
        return [w for _, _, w in unit_hits(src, path)]
    cases = [
        ('let now_us = 1; let secs = 2; let nanos = 3; let millis = 4; let micros = 5;',
         ['now_us', 'secs', 'nanos', 'millis', 'micros']),
        ('const NANOS_PER_SEC: u64 = 1; const SECS_PER_MIN: u64 = 60; let kib_per_s = 1;',
         ['NANOS_PER_SEC', 'SECS_PER_MIN', 'kib_per_s']),
        ('let within_30_s = 1; let TICKS_PER_SEC = 1; let mins = 2; let tv_usec = 0;',
         ['within_30_s', 'TICKS_PER_SEC', 'mins']),
        # The units the template refuses and the ratchet has not counted yet, the spelled-out names
        # and the std methods are never hits.
        ('let a_ns = 1; let b_ms = 2; let c_kib = 3; let hz = 4; let seconds = 5; let minutes = 6; '
         'let microseconds = 7; d.as_nanos(); d.as_secs(); Duration::from_millis(1);', []),
        # A bare `s`, a mode-bit prefix and a possessive are not units.
        ('let s = 1; const S_IFDIR: u32 = 1; let s_context = 2; let saw_s = false; let ds_s = 1;', []),
        # `min` is a minimum unless it follows `per`.
        ('let min = 1; const MIN_ALIGN: u64 = 1;', []),
        ('let (sec, sub) = f();', ['sec']),
        ('// secs in a comment\nlet x = "now_us";', []),
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
        print('unit selftest: UNIT_KEPT in kernel/src/pci.rs counted', file=sys.stderr)
    if names('const US: u64 = 1;', 'crates/paging/src/x86_64.rs'):
        failed += 1
        print('unit selftest: UNIT_KEPT US counted', file=sys.stderr)
    if [w for _, w in unclear_hits('let nsec = 1; let kb = 2; let ns = 3; let nanos = 4;')] != [
            'nsec', 'kb', 'ns']:
        failed += 1
        print('unit selftest: unclear lookalikes', file=sys.stderr)
    # The ratchet counts a subset of what the template refuses: a name it counts, the template's own
    # check refuses too.
    refuses = template_units.hits_of_words(WORDS)
    for name in ('now_us', 'SECS_PER_MIN', 'kib_per_s', 'within_30_s', 'NANOS_PER_SEC', 'mins'):
        if family_of(name) and not refuses(rust_names.Definition(name, 'name', 1)):
            failed += 1
            print(f'unit selftest: the ratchet counts {name} and the template check does not',
                  file=sys.stderr)
    for unit in UNIT_COUNTED:
        if unit not in UNITS:
            failed += 1
            print(f'unit selftest: the template word list no longer holds `{unit}`', file=sys.stderr)
    for table, unit in ((UNITS_AFTER_PER, 'min'), (UNITS_AFTER_PER, 's'),
                        (UNITS_AFTER_NUMBER, 's')):
        if unit not in table:
            failed += 1
            print(f'unit selftest: the template word list lost a `{unit}` row it counts after `per` '
                  f'or a number', file=sys.stderr)
    for old, new in (('NANOS_PER_SEC', 'NANOSECONDS_PER_SECOND'), ('TICK_HZ', 'TICK_HERTZ'), ('SECS_PER_MIN', 'SECONDS_PER_MINUTE'),
                     ('unix_nanos', 'unix_nanoseconds'), ('delay_us', 'delay_microseconds'),
                     ('within_30_s', 'within_30_seconds'), ('cpu_millis', 'cpu_milliseconds'),
                     ('hz', 'hertz'), ('fine', 'fine')):
        if respell(old) != new:
            failed += 1
            print(f'unit selftest: respell({old!r}) is {respell(old)!r}, wanted {new!r}', file=sys.stderr)
    if failed:
        return 1
    print(f'unit selftest: {len(cases) + 4} fixtures')
    return 0


def by_family(by_path):
    fam = {}
    for hits in by_path.values():
        for _, f, _ in hits:
            fam[f] = fam.get(f, 0) + 1
    return fam


def unit_main(argv):
    if argv and argv[0] == '--selftest':
        return unit_selftest()
    if not argv or argv[0] == '--check':
        now, bad = check(unit_hits, UNIT_CEILING,
                         '{now:,} identifiers use a unit symbol the naming ruling does not allow, '
                         f'over the ceiling of {{ceiling:,}} in {SELF}. If this branch adds none, '
                         'main moved past it: merge main, and set the ceiling to the merged count '
                         'in this change, saying why')
        if bad:
            print('unit: the tree gained identifiers with a unit symbol the ruling does not allow '
                  '(2026-10-27 UTC):', file=sys.stderr)
            for b in bad:
                print(f'  {b}', file=sys.stderr)
            print(f'\nSpell the unit out (`nanoseconds`, `milliseconds`, `seconds`, `minutes`); '
                  f'`python3 {SELF} unit --list PATH` shows each hit. See '
                  f'design/naming.md ("Units") and the header of {SELF}.',
                  file=sys.stderr)
            return 1
        print(f'unit: {now:,} identifiers use a disallowed unit symbol (ceiling held)')
        return 0
    if argv[0] == '--count':
        c = census(tracked(), unit_hits)
        fam = by_family(c)
        for f in sorted(fam, key=lambda k: -fam[k]):
            print(f'{f}\t{fam[f]}')
        print(f'total\t{total(c)}')
        return 0
    if argv[0] == '--list':
        paths = argv[1:] or tracked()
        for p, hits in sorted(census([x for x in paths if in_scope(x)], unit_hits).items()):
            for line, fam, word in hits:
                print(f'{p}:{line}: {fam}: {word}')
        return 0
    if argv[0] == '--unclear':
        for p in tracked():
            for line, word in unclear_hits(read(p) or ''):
                print(f'{p}:{line}: {word}')
        return 0
    if argv[0] == '--bank':
        now = bank(unit_hits, 'UNIT_CEILING', UNIT_CEILING)
        print(f'unit: ceiling now {min(now, UNIT_CEILING):,}')
        return 0
    print(__doc__.split('\n\n')[2], file=sys.stderr)
    return 2


# --- design/naming.md and the word list say the same thing -------------------------------------

NAMING = 'design/naming.md'
# Words the prose spells out that the template's list does not hold. An entry is a gap in the
# template, to be closed there; the check fails when the list gains the word, so this empties.
PROSE_ONLY_WORDS = {'proto': 'the template has no `proto` row'}
_BACKTICKED = re.compile(r'`([A-Za-z_]+)`')


def section_of(text, heading):
    """The lines of the `## heading` section of a markdown document, without the heading."""
    out, inside = [], False
    for line in text.split('\n'):
        if line.startswith('## '):
            inside = line[3:].strip() == heading
        elif inside:
            out.append(line)
    return '\n'.join(out)


def bullet_with(section, opening):
    """The bullet of `section` that opens with `opening`, continuation lines included."""
    lines, out = section.split('\n'), []
    for i, line in enumerate(lines):
        if line.startswith('- ' + opening):
            out = [line]
            for more in lines[i + 1:]:
                if more.startswith('- ') or not more.strip():
                    break
                out.append(more)
            break
    return ' '.join(s.strip() for s in out)


def base_words(table):
    """The words of a table, without a plural whose singular the table holds too."""
    return {w for w in table if not (w.endswith('s') and w[:-1] in table)}


def sentence_with(section, phrase):
    flat = ' '.join(section.split())
    return next((s for s in re.split(r'(?<=[.:])\s+', flat) if phrase in s), '')


def prose_problems(text, words):
    """What design/naming.md says that the template's word list does not, and the other way."""
    bad = []
    table, units = words['spelled_out'], words['units']
    section = section_of(text, 'Acronyms and abbreviations')
    spelled = bullet_with(section, 'Every name this tree authors')
    if not spelled:
        return [f'{NAMING} has no "Every name this tree authors" bullet under "Acronyms and '
                f'abbreviations"; the check reads the words from it']
    prose = set(_BACKTICKED.findall(spelled))
    named = {w.lower() for w in _BACKTICKED.findall(section)}
    for word in sorted(base_words(table) - named):
        bad.append(f'`{word}` is in the template word list ({table[word]}) and {NAMING} '
                   f'("Acronyms and abbreviations") never names it')
    for word in sorted(prose - set(table)):
        if word not in PROSE_ONLY_WORDS:
            bad.append(f'`{word}` is spelled out by {NAMING} and the template word list has no row '
                       f'for it. Add the row in nifeos/crate-workflows, or list it in '
                       f'PROSE_ONLY_WORDS with the reason')
    for word in sorted(set(PROSE_ONLY_WORDS) & set(table)):
        bad.append(f'PROSE_ONLY_WORDS lists `{word}`, which the template word list holds now. '
                   f'Remove the entry')
    for word in sorted(set(PROSE_ONLY_WORDS) - prose):
        bad.append(f'PROSE_ONLY_WORDS lists `{word}`, which {NAMING} no longer spells out. '
                   f'Remove the entry')
    spellings = {v.lower() for v in units.values()} | set(units)
    section = section_of(text, 'Units')
    for phrase in ('valid in a name', 'IEC binary'):
        sentence = sentence_with(section, phrase)
        if not sentence:
            bad.append(f'{NAMING} ("Units") has no sentence about "{phrase}"; the check compares it '
                       f'with the template word list')
        for token in _BACKTICKED.findall(sentence):
            if token.strip('_').lower() not in spellings:
                bad.append(f'{NAMING} ("Units") names `{token}` beside "{phrase}", and the template '
                           f'word list holds no such symbol or spelling')
    for word in sorted(set(units) & {w.lower() for w in _BACKTICKED.findall(section)}):
        stays = sentence_with(section, f'`{word}`')
        if re.search(r'\b(stay|stays|allowed|valid symbols?)\b', stays):
            bad.append(f'{NAMING} ("Units") says `{word}` stays, and the template word list refuses it')
    return bad


def words_selftest():
    words = {'spelled_out': {'op': 'operation', 'ops': 'operations', 'sdio': 'x', 'cap': 'capability'},
             'units': {'ns': 'nanoseconds', 'kib': 'kibibytes', 'mbps': 'megabits_per_second'}}
    good = ('## Acronyms and abbreviations\n\n'
            '- Every name this tree authors spells these out: `op`, `cap` and `proto`.\n'
            '  It continues here.\n'
            '- These stay: `pci`. `SDIO` is not accepted.\n\n'
            '## Units\n\n'
            'No SI symbol is valid in a name (`_NS`, `mbps`). The IEC binary prefixes (`kib`) are '
            'symbols too: write `kibibytes`.\n')
    ok = prose_problems(good, words)
    failed = 0

    def expect(label, text, fragment=None, table=words):
        nonlocal failed
        got = prose_problems(text, table)
        hit = (not got) if fragment is None else any(fragment in b for b in got)
        if not hit:
            failed += 1
            print(f'words selftest: {label}: {got}', file=sys.stderr)
    held = dict(words, spelled_out=dict(words['spelled_out'], proto='protocol'))
    if not any('PROSE_ONLY_WORDS lists `proto`' in b for b in prose_problems(good, held)):
        failed += 1
        print('words selftest: a word the list holds now still sits in PROSE_ONLY_WORDS',
              file=sys.stderr)
    if ok:
        failed += 1
        print(f'words selftest: the good prose fails: {ok}', file=sys.stderr)
    expect('a list word the prose never names',
           good.replace('`op`, ', ''), '`op` is in the template word list')
    expect('a prose word the list lacks',
           good.replace('`op`,', '`op`, `zzz`,'), '`zzz` is spelled out')
    expect('an exception that went stale', good.replace(' and `proto`', ''),
           'PROSE_ONLY_WORDS lists `proto`')
    expect('a unit that stays', good.replace('are symbols too', 'are allowed, `kib` stays'),
           'says `kib` stays')
    expect('an unknown symbol beside the rule', good.replace('`mbps`', '`wibble`'), '`wibble`')
    expect('no bullet', '## Acronyms and abbreviations\n\n- Other.\n', 'no "Every name')
    if failed:
        return 1
    print('words selftest: 8 fixtures')
    return 0


def words_main(argv):
    if argv and argv[0] == '--selftest':
        return words_selftest()
    if argv and argv[0] != '--check':
        print(__doc__.split('\n\n')[2], file=sys.stderr)
        return 2
    bad = prose_problems(read(NAMING) or '', WORDS)
    if bad:
        print(f'words: {NAMING} and the template word list '
              f'(vendor/crate-workflows/helpers/naming_words.toml) disagree:', file=sys.stderr)
        for b in bad:
            print(f'  {b}', file=sys.stderr)
        print('\nThe list is the template\'s: change a word there, never in the vendored copy, and '
              f'move the pin. The prose is {NAMING}.', file=sys.stderr)
        return 1
    print(f'words: {NAMING} names all {len(base_words(SPELLED_OUT))} words of the template list '
          f'and its units agree')
    return 0


# --- the service model and the contract names, over this tree's programs and crates -----------

# Program and crate names the template's service-model check refuses and this tree keeps, each with
# why. An entry is removed by renaming what it names.
PROGRAM_KEPT = {
    "uuid": "RFC 9562's own term for the object; the d is the last letter of identifier, and the "
            "program draws sixteen bytes, prints one line and exits (milestone 111 (a shell that can endow a child with entropy))",
    "report_child": "ends in the word child, which calef ruled for it on 2026-10-18 (UTC); built "
                    "by its builder, reports once and exits",
    "device_child": "as report_child",
    "interrupt_child": "as report_child",
}
# The one crate that keeps `_protocol`: the NTPv4 standard's own name (calef, 2026-10-18 UTC).
PROTOCOL_KEPT = {"network_time_protocol": "the NTPv4 standard's own name"}
# Files the prose check does not read. `design/` holds the argument about the word and the naming
# conventions, and script/lint names it to explain the rule; the CI log baseline records branch
# names as they were on their dates.
PROSE_EXEMPT = ('vendor/', 'design/', 'script/lint', 'notes/project-metrics/ci-log-baseline.csv')


def program_names(root):
    """Every program and crate name the tree defines, as the lint check read them: a source file
    or directory under components/src and fixtures/src, a `[[bin]]` name in either manifest, and a
    directory under crates/."""
    names = set()
    for src in ('components/src', 'fixtures/src'):
        names |= {re.sub(r'\.rs$', '', n) for n in os.listdir(os.path.join(root, src))}
    for crate in ('components', 'fixtures'):
        text = read(os.path.join(root, crate, 'Cargo.toml')) or ''
        names |= set(re.findall(r'^\[\[bin\]\]\nname = "(.*)"$', text, re.M))
    names |= set(os.listdir(os.path.join(root, 'crates')))
    return sorted(names)


def name_problems(names, words=WORDS):
    """Failure lines for the names the template's service-model and contract-name checks refuse and
    the kept tables do not excuse."""
    service = service_model.hits_of_words(words)
    contract = contract_names.hits_of_words(words)
    out = []
    for name in names:
        definition = rust_names.Definition(name, 'program', 1)
        if name not in PROGRAM_KEPT:
            out += [service_model.phrase(w, why) + f' ({name})' for w, why in service(definition)]
        if name not in PROTOCOL_KEPT:
            out += [contract_names.phrase(w, why) + f' ({name})'
                    for w, why in contract(definition)]
    return out


def prose_files():
    files = git('ls-files', '-z') or ''
    return [f for f in files.split('\0')
            if f and not f.startswith(PROSE_EXEMPT) and os.path.isfile(f) and not os.path.islink(f)]


def service_selftest():
    failed = 0
    bad = name_problems(['netd', 'uuid', 'kbd', 'storage_proto', 'network_time_protocol',
                         'fscontract', 'storage_contract', 'report_child', 'keyboard_driver'])
    names = sorted(line.rsplit('(', 1)[1].rstrip(')') for line in bad)
    if names != ['fscontract', 'netd', 'storage_proto']:
        failed += 1
        print(f'service selftest: refused {names}', file=sys.stderr)
    for kept in list(PROGRAM_KEPT) + list(PROTOCOL_KEPT):
        if not os.path.exists(os.path.join(REPO, 'crates', kept)) and \
                kept not in program_names(REPO):
            failed += 1
            print(f'service selftest: {kept} is kept but no program or crate has the name',
                  file=sys.stderr)
    if failed:
        return 1
    print('service selftest: ok')
    return 0


def service_main(argv):
    if argv and argv[0] == '--selftest':
        return service_selftest()
    if argv != ['--check']:
        print(__doc__.split('\n\n')[2], file=sys.stderr)
        return 2
    names = program_names(REPO)
    contracts = [n for n in os.listdir(os.path.join(REPO, 'crates')) if n.endswith('contract')]
    if not contracts:
        print('service: crates/ holds no crate ending in contract, so the contract-crate spelling '
              'check judged nothing. The suffix moved again; move the check with it '
              '(design/naming.md).', file=sys.stderr)
        return 1
    bad = name_problems(names)
    files = prose_files()
    for path in files:
        data = read(path)
        if data is not None:
            bad += service_model.prose_problems(path, data)
    if bad:
        for line in bad:
            print(f'service: {line}', file=sys.stderr)
        print('\nName it for what it is (DECISIONS §39 (a component is named for what it is), '
              'design/naming.md). A name that keeps its '
              'spelling on purpose goes in PROGRAM_KEPT or PROTOCOL_KEPT in '
              'helpers/vendored_name_gates.py, with the reason.', file=sys.stderr)
        return 1
    print(f'service: {len(names)} program and crate names ({len(contracts)} contract crates) and '
          f'{len(files)} files read, none claims a Unix service model')
    return 0


def main(argv):
    global REPO
    REPO = (subprocess.run(['git', 'rev-parse', '--show-toplevel'], capture_output=True,
                           text=True).stdout.strip() or os.getcwd())
    os.chdir(REPO)
    gates = {'cap': cap_main, 'unit': unit_main, 'words': words_main, 'service': service_main}
    if not argv or argv[0] not in gates:
        print(__doc__.split('\n\n')[2], file=sys.stderr)
        return 2
    return gates[argv[0]](argv[1:])


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
