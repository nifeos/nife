"""The `cap` ratchet: Rust identifiers that abbreviate capability as `cap`, counted, never allowed to grow.

calef, 2026-10-06 (UTC):

    "I don't think we should abbreviate capability as cap."

and, on how to apply it: "Record the ruling. We've almost got a quiet tree so we can run that
shortly." New code spells `capability` out. Existing code is renamed in one sweep when the tree is
quiet, from the worklist in design/naming/capability-worklist.md; this gate makes sure nothing new arrives
before then. It is rung 2 of the AGENTS.md ladder, and the ruling's record is in
design/naming/spelled-out-rulings.md.

    python3 helpers/cap_abbreviation.py --check     # what script/lint runs
    python3 helpers/cap_abbreviation.py --list [PATH..]  # every counted hit, path:line: identifier
    python3 helpers/cap_abbreviation.py --bank      # lower CEILING to the tree; never raises
    python3 helpers/cap_abbreviation.py --selftest  # the traps, as fixtures

Name: provisional, minted by lane/no-cap-abbreviation on 2026-10-06. A shared python module under
`helpers/`, which `script/names` puts out of its own scope, so its provenance is this paragraph.
calef names things; expect this to change.

**What counts.** An identifier in a `.rs` file that has `cap` or `caps` as a whole component, in
any casing: `cap`, `Cap`, `CAP`, `caps`, `cap_slot`, `SEND_CAP`, `irq_cap`, `CapKind`, `MCap`.
Components split at `_`, at digits and at CamelCase boundaries, so `capacity`, `capture`, `escape`,
`capable` and `keycaps` are one component each and never match. Comments and string literals are
stripped first: the ruling covers identifiers, and "a cap on lane count" in a comment is the
English word for a ceiling, which nobody abbreviated.

**Two halves, house_style.py's shape.** A count over `CEILING` fails, which guards `main`, where
there is no merge base. A branch is also held to its merge base: summed over the `.rs` files it
changed, the count may not grow. That second half is the real ratchet, because an ordinary edit
that deletes a `cap` would otherwise leave slack for the next change to spend. The sweep runs
`--bank` to lower the ceiling.

**Permanent exclusions, each for a stated reason, not a backlog:**

- `vendor/`, `patches/` and `target/`: somebody else's code.
- `KEPT`, identifiers in a named file where `cap` is not this tree's abbreviation of capability.
  Two kinds, each entry saying which. A ceiling: `cap` is the English word for a limit (`heap_cap`,
  a print budget), or `CAP` a buffer's capacity, and nothing abbreviated capability. A register a
  hardware specification named (VT-d's `CAP`, NVMe's `CAP`, the PCI capability pointer, IEEE
  802.3's ability bits): the `operation` ruling's precedent, that an identifier mirroring an
  external API keeps its spelling. Both are proposals in the worklist until calef rules, and an
  entry may only be removed by renaming what it names. Keyed by file, because `CAP` means a
  buffer's size in one file and a capability in another.

BUGS:
- `ncaps`, `cptr` and other contractions with no separator are one component and do not match.
  The worklist lists the ones found; the sweep renames them by hand.
- The lexer is a lexer, not a parser. It knows comments (nested), strings (raw and byte), char
  literals and lifetimes, which is what this tree uses. A macro that builds an identifier from a
  string (`concat_idents!`, `paste!`) is not read.
"""

import os
import re
import subprocess
import sys

sys.dont_write_bytecode = True

SELF = 'helpers/cap_abbreviation.py'

# The count on 2026-10-06, and since lowered by the sweep's `--bank`. Never raise it by hand: a new
# `cap` is spelled out, not admitted.
CEILING = 2_890

# Identifiers that keep `cap`, by file, each group with its reason. See the header before adding one.
_CEILING = 'cap is the English word for a limit or a capacity here, not an abbreviation'
_REGISTER = 'mirrors a register or field a hardware specification named'
_CALLS = 'a call of an existing tree API whose name is on the sweep worklist, not a name minted here'
KEPT = {
    'components/src/printenv.rs': ({'CAP'}, _CEILING),
    'crates/calendar/src/lib.rs': ({'FMT_CAP'}, _CEILING),
    'kernel/src/stack.rs': ({'CAP'}, _CEILING),
    'kernel/src/arch/x86_64/timer.rs': ({'CALIBRATION_WINDOW_CAP'}, _CEILING),
    'redoxfs_server/src/bin/second_mount.rs': ({'cap', 'heap_cap', 'HIT_CAP', 'cap_mib'}, _CEILING),
    'crates/swish/src/lib.rs': ({'the_heap_cap_is_named_in_kib'}, _CEILING),
    'crates/domain_name_system/tests/replies.rs':
        ({'addresses_past_the_cap_are_dropped_and_the_rest_kept'}, _CEILING),
    'crates/video_terminal/src/lib.rs': ({'scrolls_report_movement_and_cap_at_the_screen'}, _CEILING),
    'xtask/src/board.rs': ({'cap_given'}, _CEILING),
    # `PRINT_CAP` bounds a diagnostic's lines; the test's `caps` is the verb "limits".
    'kernel/src/sched.rs': ({'PRINT_CAP', 'a_spawn_quota_caps_live_children_and_replenishes_on_reap'},
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
    'crates/e1000e/src/pch/sequence.rs': ({'NWAY_AR_10T_HD_CAPS', 'NWAY_AR_10T_FD_CAPS',
                                           'NWAY_AR_100TX_HD_CAPS', 'NWAY_AR_100TX_FD_CAPS',
                                           'CR_1000T_HD_CAPS', 'CR_1000T_FD_CAPS'}, _REGISTER),
    # PCI Local Bus 3.0 (`PCI_CAP_PTR`, `PCI_STATUS_CAP_LIST`, `PCI_CAP_ID_*`) and virtio 1.x
    # section 4.1.4 (`VIRTIO_PCI_CAP_*_CFG`). The tree's own names for them are on the worklist.
    'crates/pci/src/lib.rs': ({'CAP_PTR', 'STATUS_CAP_LIST', 'CAP_ID_MSI', 'CAP_ID_MSIX',
                               'CAP_ID_VENDOR', 'VIRTIO_CAP_COMMON', 'VIRTIO_CAP_NOTIFY',
                               'VIRTIO_CAP_ISR', 'VIRTIO_CAP_DEVICE'}, _REGISTER),
    'kernel/src/pci.rs': ({'VIRTIO_CAP_COMMON', 'VIRTIO_CAP_NOTIFY', 'VIRTIO_CAP_ISR'}, _REGISTER),
    # Milestone 800 (a non-Anthropic model attacks the confinement claim), 2026-10-07 (UTC): the
    # outsider pass's fixture and test call these APIs by their existing names and mint no
    # cap-named identifier of their own.
    'components/src/socket_squatter.rs': ({'send_cap'}, _CALLS),
    'system_tests/src/user/net_confinement_tests.rs':
        ({'cap', 'memory_region_cap', 'rendezvous_cap'}, _CALLS),
    # The same pass's chatty reshape: `spawn_swapper`'s two new slots call the same family.
    'system_tests/src/user/live_swap_tests.rs':
        ({'cap', 'device_frame_cap', 'memory_region_root_cap', 'notification_cap', 'rendezvous_cap',
          'thread_control_block_insert_cap', 'timer_cap'}, _CALLS),
    # Milestone 812 (`std::thread::spawn` runs real threads in one address space), 2026-10-10
    # (UTC): the thread-pointer and futex tests call these APIs by their existing names and mint
    # none, and the `ThreadControlBlock` methods moved out of `kernel/src/syscall.rs` unchanged
    # (the split §266 (a Rust source file stays under 2,000 lines) asks for), names and all.
    'system_tests/src/user/futex_tests.rs':
        ({'cap', 'address_space_cap', 'thread_control_block_insert_cap', 'delete_current_cap'},
         _CALLS),
    'system_tests/src/user/thread_pointer_tests.rs':
        ({'cap', 'address_space_cap', 'thread_control_block_cap', 'thread_control_block_insert_cap',
          'current_cap', 'delete_current_cap'}, _CALLS),
    'kernel/src/syscall/thread_control_block.rs':
        ({'cap', 'CAP_INSERT', 'thread_control_block_cap_insert', 'current_cap',
          'delete_current_cap', 'thread_control_block_delegate_cap'}, _CALLS),
}

EXCLUDED_PREFIXES = ('vendor/', 'patches/', 'target/')


def in_scope(path):
    return path.endswith('.rs') and not path.startswith(EXCLUDED_PREFIXES)


# --- lexing -------------------------------------------------------------------------------------

IDENT_START = re.compile(r'[A-Za-z_]')
IDENT = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')
RAW_STRING = re.compile(r'(?:b|c)?r(#*)"')
CHAR_LITERAL = re.compile(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^\\'\n])'")


def identifiers(text):
    """[(line, identifier)] for every identifier in Rust source, comments and strings skipped."""
    out = []
    i, n, line = 0, len(text), 1
    while i < n:
        c = text[i]
        if c == '\n':
            line += 1
            i += 1
        elif text.startswith('//', i):
            j = text.find('\n', i)
            i = n if j < 0 else j
        elif text.startswith('/*', i):
            depth, i = 1, i + 2
            while i < n and depth:
                if text.startswith('/*', i):
                    depth, i = depth + 1, i + 2
                elif text.startswith('*/', i):
                    depth, i = depth - 1, i + 2
                else:
                    line += text[i] == '\n'
                    i += 1
        elif c == '"':
            i += 1
            while i < n and text[i] != '"':
                if text[i] == '\\':
                    i += 1
                line += i < n and text[i] == '\n'
                i += 1
            i += 1
        elif c == "'":
            m = CHAR_LITERAL.match(text, i)
            # A lifetime or a label (`'a`, `'outer`) is an identifier after its quote.
            i = m.end() if m else i + 1
        elif c.isdigit():
            m = re.compile(r'[0-9A-Za-z_]*').match(text, i + 1)
            i = m.end()
        elif IDENT_START.match(c):
            raw = RAW_STRING.match(text, i)
            if raw and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == '_')):
                close = '"' + raw.group(1)
                j = text.find(close, raw.end())
                j = n if j < 0 else j + len(close)
                line += text.count('\n', i, j)
                i = j
                continue
            if c in 'bc' and i + 1 < n and text[i + 1] in '"\'':
                i += 1  # a byte or C string, or a byte char: the quote branch reads it
                continue
            m = IDENT.match(text, i)
            word = m.group(0)
            if word == 'r' and text.startswith('#', m.end()) and m.end() + 1 < n \
                    and IDENT_START.match(text[m.end() + 1]):
                m = IDENT.match(text, m.end() + 1)  # a raw identifier, r#type
                word = m.group(0)
            out.append((line, word))
            i = m.end()
        else:
            i += 1
    return out


COMPONENT = re.compile(r'[A-Z]+(?![a-z])|[A-Z]?[a-z]+|[A-Z]+')


def components(identifier):
    """`SEND_CAP` -> SEND, CAP; `CapKind` -> Cap, Kind; `MCap` -> M, Cap; `cap2` -> cap."""
    parts = []
    for chunk in re.split(r'[_0-9]+', identifier):
        parts += COMPONENT.findall(chunk)
    return parts


def abbreviates(identifier):
    return any(p.lower() in ('cap', 'caps') for p in components(identifier))


def file_hits(text, path=''):
    kept = KEPT.get(path, (set(), ''))[0]
    return [(line, word) for line, word in identifiers(text)
            if abbreviates(word) and word not in kept]


# --- counting -----------------------------------------------------------------------------------

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


def census(paths):
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


def growth(base):
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


# --- the commands -------------------------------------------------------------------------------

def check():
    bad = []
    now = total(census(tracked()))
    if now > CEILING:
        bad.append(f'{now:,} identifiers abbreviate capability as cap, over the ceiling of '
                   f'{CEILING:,} in {SELF}. If this branch adds none, main moved past it: rebase, '
                   f'and set the ceiling to the merged count in this change, saying why')
    base = merge_base()
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
    cases = [
        # (Rust source, the identifiers that must count)
        ('fn f(cap: u64, caps: &[Cap]) -> CAP {}', ['cap', 'caps', 'Cap', 'CAP']),
        ('const SEND_CAP: u64 = 1; let irq_cap = cap_slot; struct CapKind; struct MCap;',
         ['SEND_CAP', 'irq_cap', 'cap_slot', 'CapKind', 'MCap']),
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
    kept = [w for _, w in file_hits('const CAP: usize = 96; let cap = 1;', 'components/src/printenv.rs')]
    if kept != ['cap']:
        failed += 1
        print(f'cap selftest: KEPT in printenv.rs counted {kept}, wanted [\'cap\']', file=sys.stderr)
    for src, want in cases:
        got = [w for _, w in file_hits(src)]
        if got != want:
            failed += 1
            print(f'cap selftest: {src!r} counted {got}, wanted {want}', file=sys.stderr)
    lines = [ln for ln, _ in file_hits('/* a\nb */\nlet s = "x\ny";\nlet cap = 1;')]
    if lines != [5]:
        failed += 1
        print(f'cap selftest: line numbers {lines}, wanted [5]', file=sys.stderr)
    if failed:
        return 1
    print(f'cap selftest: {len(cases) + 2} fixtures')
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
            print('cap: the tree gained identifiers that abbreviate capability as cap '
                  '(calef, 2026-10-06: "I don\'t think we should abbreviate capability as cap."):',
                  file=sys.stderr)
            for b in bad:
                print(f'  {b}', file=sys.stderr)
            print(f'\nSpell it `capability` (`python3 {SELF} --list PATH` shows each hit). See '
                  f'design/naming/spelled-out-rulings.md and the header of {SELF}.',
                  file=sys.stderr)
            return 1
        print(f'cap: {now:,} identifiers abbreviate capability as cap (ceiling held; the sweep '
              f'in design/naming/capability-worklist.md drives it to zero)')
        return 0
    if argv[0] == '--list':
        paths = argv[1:] or tracked()
        for p, hits in sorted(census([x for x in paths if in_scope(x)]).items()):
            for line, word in hits:
                print(f'{p}:{line}: {word}')
        return 0
    if argv[0] == '--bank':
        now = bank()
        print(f'cap: ceiling now {min(now, CEILING):,}')
        return 0
    print(__doc__.split('\n\n')[2], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
