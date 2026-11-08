#!/usr/bin/env python3
"""Register, mask, address and size names: the naming rules that read a constant's shape.

    python3 register_names.py [--manifest Cargo.toml] [--report] FILE...
    python3 register_names.py --selftest

Four rules, each judged on the names the crate defines (helpers/rust_names.py says which):

1. **A register offset ends `_REGISTER`.** A constant with a plain integer literal for its value
   that either has `offset` in its name or sits in a module named for registers or offsets
   (`registers`, `register_offsets`, `offsets`), inline or as a file (`src/registers.rs`,
   `src/registers/mod.rs`, anything under `src/registers/`), ends `_REGISTER`, and says which register:
   `CONTROL_REGISTER`, never `CTRL` or `CONTROL_OFFSET`. A name that ends `_MASK`, `_SHIFT`,
   `_VALUE`, `_BYTES` or `_BITS` is a different quantity and is not read as an offset.
2. **A mask of several bits ends `_MASK`.** A constant with `mask` anywhere but last in its name
   fails: `STATUS_MASK`, never `MASK_STATUS`. A register named for the mask it holds ends
   `_MASK_REGISTER` (`INTERRUPT_MASK_REGISTER`), which passes.
3. **An address is virtual or physical.** A name with `address` (or `addr`) in it has `virtual` or
   `physical` straight before it (`FRAME_PHYSICAL_ADDRESS`), or a word from `[address_qualifiers]`
   that says it is not a location in memory (`MAC_ADDRESS`, `LOGICAL_BLOCK_ADDRESS`); a qualifier
   other than `virtual` or `physical` may stand one word further back (`RELATIVE_CARD_ADDRESS`).
   `address` used as a verb is not read: after a word from `[address_verb_before]`
   (`can_address_blocks`), or leading a function name before a word from `[address_verb_after]`
   (`address_and_select`). `addressed` and `addressing` are other words and are never read.
4. **A size carries its unit.** A constant, static or field with `size`, `len` or `length` in its
   name also holds a word from `[size_units]` (`BUFFER_SIZE_BYTES`, never `BUFFER_SIZE`). A count
   of things names the things (`MAXIMUM_COLUMNS`). An integer constant whose first word is in
   `[command_verbs]` names a command, not a quantity (`SET_BLOCK_LENGTH`), and is not read.

5. **A register constant is spelled out, and the source spelling is the alias.** An integer
   constant whose doc comment opens with a source for its own name, `(CAP, <source>)`, mirrors a
   datasheet's identifier and fails: rename it to say what it is and keep the source's spelling in
   `#[doc(alias = "CAP")]`. A constant that states a source for a different spelling carries that
   spelling as an alias, or it fails. A parenthesis is a source only when the text after its comma
   holds a word from `[source_words]` (`OpenBSD`, `databook`, `specification`), a file name
   (`dwqereg.h`) or a section number: "(`FBE`, `DU` or `CES`)" lists names and cites nothing.

Rules 1, 2 and 5 have no exception: an offset, a mask and a register constant are renamed. Rules 3
and 4 take the kept form rust_names.py describes (a name that mirrors a specification or the
standard library opens its doc comment with `(<name>, <source>)` and carries `#[doc(alias)]`),
except on an integer constant, which rust_names.py never keeps.
There is no baseline: the ceiling is 0, and every finding fails.

The rules are the naming rules page's "Registers and constants" and "Units" sections in the
operating system project this template serves (design/naming.md there). The file's name is
provisional.

BUGS:
- A lexer, not a parser. Whether a constant is a register offset is read from its module and its
  value, so an offset written as an expression (`BASE + 0x10`) in a module with another name is
  missed.
- A mask is recognized by the word `mask` in its name. A multi-bit constant that never says so
  (`0xFF00`) is not read; a reviewer reads those.
- Whether a name spells out what a register is stays a reader's judgment: this check refuses the
  spec's own abbreviation only where the doc comment states it as the source.
"""

import os
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import rust_names  # noqa: E402

INTEGERS = rust_names.INTEGERS
REGISTER_MODULES = {'register', 'registers', 'offset', 'offsets'}
OTHER_QUANTITIES = {'mask', 'shift', 'value', 'bytes', 'bits'}
NUMBER = re.compile(r'0[xXbBoO][0-9A-Fa-f_]+|[0-9][0-9_]*')


def _words(table, words):
    return set(words.get(table, {}).get('words', ()))


def is_plain_literal(definition):
    """Whether the initializer is one integer literal (an offset), not a shift or a mask."""
    value = definition.value
    return len(value) == 1 and bool(NUMBER.fullmatch(value[0]))


def file_modules(path):
    """The module names a file's path gives its definitions: the path below the last `src`
    directory, without `lib`, `main` or `mod` (`src/registers.rs` -> registers,
    `src/registers/control.rs` -> registers, control). A path with no `src` gives its stem."""
    parts = Path(path).with_suffix('').parts
    if 'src' in parts:
        parts = parts[len(parts) - parts[::-1].index('src'):]
    else:
        parts = parts[-1:]
    return tuple(p for p in parts if p not in ('lib', 'main', 'mod'))


def in_register_module(definition, path=''):
    modules = file_modules(path) + tuple(definition.module) if path else tuple(definition.module)
    return any(c in REGISTER_MODULES for m in modules for c in rust_names.components(m))


CITATION = re.compile(r'\(\s*`?([A-Za-z_][A-Za-z0-9_]*)`?\s*,\s*([^)]*[A-Za-z][^)]*)\)')
FILE_NAME = re.compile(r'[A-Za-z0-9_./-]+\.(?:h|c|rs|dts|dtsi|py|txt)\b')
SECTION_NUMBER = re.compile(r'(?:§|\bsection\s+)\d')


def cites(text, words):
    """Whether `text`, the part of a parenthesis after its comma, names a source."""
    lower = text.lower()
    if FILE_NAME.search(text) or SECTION_NUMBER.search(lower):
        return True
    return any(re.search(r'(?<![a-z0-9])' + re.escape(w) + r'(?![a-z0-9])', lower)
               for w in _words('source_words', words))


def stated_source(definition, words):
    """The spelling the doc comment's first paragraph states as the source, or None. A parenthesis
    whose text after the comma names no source (`[source_words]`, a file, a section) is not one."""
    paragraph = '\n'.join(definition.docs).split('\n\n')[0].replace('\n', ' ')
    for m in CITATION.finditer(paragraph):
        if cites(m.group(2), words):
            return m.group(1)
    return None


def address_findings(definition, parts, words):
    """The address rule's finding for `definition`, or None."""
    qualifiers = _words('address_qualifiers', words)
    verb_before = _words('address_verb_before', words)
    verb_after = _words('address_verb_after', words)
    for k, part in enumerate(parts):
        if part not in ('address', 'addresses', 'addr', 'addrs'):
            continue
        before = parts[k - 1] if k else None
        if before in qualifiers:
            continue
        if k > 1 and parts[k - 2] in qualifiers - {'virtual', 'physical'}:
            continue  # `RELATIVE_CARD_ADDRESS`: the card's relative address
        if before in verb_before:
            continue  # `can_address_blocks`: a verb
        if k == 0 and definition.kind == 'fn' and len(parts) > 1 and parts[1] in verb_after:
            continue  # `address_and_select`: a function named for what it does
        spelled = 'address' if part.startswith('addr') and not part.startswith('address') \
            else part
        return (f'an address is `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS` in every '
                f'location; write the {spelled} as one of them', True)
    return None


def findings(definition, words, path=''):
    """`[(message, keepable)]` for each rule `definition` breaks; `path` is its file."""
    out = []
    parts = rust_names.components(definition.name)
    kind = definition.kind
    constant = kind in ('const', 'static')
    integer = definition.type_name in INTEGERS
    if constant and integer and is_plain_literal(definition) \
            and not set(parts[-1:]) & OTHER_QUANTITIES and parts[-1:] != ['register']:
        if 'offset' in parts or 'offsets' in parts or in_register_module(definition, path):
            out.append(('a register offset ends `_REGISTER` and names the register, as '
                        '`CONTROL_REGISTER`; a bit in a register module is written as a shift '
                        '(`1 << 0`), and another quantity ends `_MASK`, `_SHIFT`, `_VALUE`, '
                        '`_BYTES` or `_BITS`', False))
    # A register named for the mask it holds, `INTERRUPT_MASK_REGISTER`, ends `_MASK_REGISTER`.
    masked = parts[:-1] if parts[-1:] == ['register'] else parts
    if constant and ('mask' in masked or 'masks' in masked) and masked[-1] not in ('mask', 'masks'):
        out.append(('a mask of several bits ends `_MASK`', False))
    address = address_findings(definition, parts, words)
    if address:
        out.append(address)
    sized = ('size', 'sizes', 'len', 'length', 'lengths')
    command = constant and integer and parts[:1] and parts[0] in _words('command_verbs', words)
    if kind in ('const', 'static', 'field') and any(p in sized for p in parts) and not command \
            and not set(parts) & _words('size_units', words):
        out.append(('a size, a length or a limit carries its unit: end it `_BYTES`, `_BITS`, '
                    '`_WORDS` or another unit from `[size_units]`', True))
    if constant and integer:
        source = stated_source(definition, words)
        aliases = [a.strip().lower() for a in definition.aliases]
        if source == definition.name:
            out.append((f'a register, field or bit constant is spelled out: name what it is and '
                        f'carry the source spelling `{source}` in #[doc(alias = "{source}")]',
                        False))
        elif source and source.lower() not in aliases:
            out.append((f'its doc comment states the source spelling `{source}`, so it carries '
                        f'#[doc(alias = "{source}")]', False))
    return out


def judge(path, found, words):
    bad = []
    for d in found:
        for message, keepable in findings(d, words, path):
            if keepable and rust_names.kept(d)[0]:
                continue
            tail = ''
            if keepable and d.kind not in ('parameter', 'binding') \
                    and not rust_names.is_integer_constant(d):
                tail = (f' If it mirrors a specification or the standard library, keep it and say '
                        f'so at the item: open its doc comment with `({d.name}, <source>)` and add '
                        f'#[doc(alias = "<plain-English name>")].')
            bad.append(f'{path}:{d.line}: {d.kind} {d.name}: {message}.{tail}')
    return bad


def run(argv):
    import argparse
    parser = argparse.ArgumentParser(prog='register_names.py')
    parser.add_argument('files', nargs='*')
    parser.add_argument('--manifest', help='accepted for symmetry with the other naming checks')
    parser.add_argument('--words', default=str(rust_names.WORDS))
    parser.add_argument('--report', action='store_true')
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args(argv)
    if args.selftest:
        return selftest()
    words = rust_names.load_words(args.words)
    bad, judged = [], 0
    for path in args.files:
        if not path.endswith('.rs'):
            continue
        found = rust_names.file_definitions(path, Path(path).read_text(encoding='utf-8',
                                                                       errors='replace'))
        judged += len(found)
        bad += judge(path, found, words)
    for line in bad:
        print(line, file=sys.stdout if args.report else sys.stderr)
    if args.report or len(bad) <= rust_names.CEILING:
        print(f'register_names: {judged} names judged, {len(bad)} refused '
              f'(ceiling {rust_names.CEILING})')
        return 0
    print(f'\nregister_names: {len(bad)} names break the register naming rules, over the ceiling '
          f'of {rust_names.CEILING}.', file=sys.stderr)
    return 1


def selftest():
    words = rust_names.load_words()
    src = '''
pub mod registers {
    pub const CONTROL_REGISTER: u64 = 0x00;
    pub const STATUS: u64 = 0x04;
    pub const STATUS_MASK: u32 = 0xF0;
    pub const RESET_VALUE: u32 = 0;
}
pub const DATA_OFFSET: usize = 8;
pub const IDENTIFIER_REGISTER: usize = 0x30;
pub const MASK_STATUS: u32 = 0x3;
pub const STATUS_MASK_BITS: u32 = 0x3;
pub const FRAME_PHYSICAL_ADDRESS: u64 = 0;
pub const BASE_ADDRESS: u64 = 0;
pub const GATEWAY_IP_ADDRESS: u32 = 0;
pub const BUFFER_SIZE: usize = 4096;
pub const BUFFER_SIZE_BYTES: usize = 4096;
pub const PAGE_SIZE_SHIFT: u32 = 12;
pub struct Header { pub len: u16, pub payload_length_bytes: u16, pub dest_addr: u32 }
/// **Controller Capabilities** (CAP, NVM Express Base Specification 2.0, section 3.1.1).
#[doc(alias = "controller capabilities")]
pub const CAP: u32 = 0;
/// **Controller Capabilities** (CAP, NVM Express Base Specification 2.0, section 3.1.1).
#[doc(alias = "CAP")]
pub const CONTROLLER_CAPABILITIES_REGISTER: u64 = 0;
/// **Controller Capabilities** (CAP, NVM Express Base Specification 2.0, section 3.1.1).
pub const CONTROLLER_CAPABILITIES_MASK: u32 = 0xFF;
/// **Page size** (PAGE_SIZE, a datasheet).
#[doc(alias = "page size")]
pub const PAGE_SIZE: usize = 4096;
pub fn count(len: usize) -> usize { let size = len; size }
'''
    found = rust_names.definitions(src)
    bad = judge('x.rs', found, words)
    got = sorted({line.split(': ')[1] for line in bad})
    want = sorted(['const STATUS', 'const DATA_OFFSET', 'const MASK_STATUS', 'const STATUS_MASK_BITS',
                   'const BASE_ADDRESS', 'const BUFFER_SIZE', 'field len', 'field dest_addr',
                   'const CAP', 'const CONTROLLER_CAPABILITIES_MASK', 'const PAGE_SIZE'])
    failed = 0
    if got != want:
        failed += 1
        print(f'register_names selftest: refused {got}, wanted {want}', file=sys.stderr)
    text = '\n'.join(bad)
    for needle in ('a register offset ends `_REGISTER`', 'a mask of several bits ends `_MASK`',
                   'is `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS`', 'carries its unit',
                   'is spelled out', 'carries #[doc(alias = "CAP")]'):
        if needle.replace('carries ', 'so it carries ') not in text and needle not in text:
            failed += 1
            print(f'register_names selftest: no message holds {needle!r}', file=sys.stderr)
    failed += selftest_file_and_false_positives(words)
    if failed:
        return 1
    print('register_names selftest: ok')
    return 0


def selftest_file_and_false_positives(words):
    """A file named for registers is a register module; the address, source, mask and command
    readings that are not findings."""
    failed = 0
    in_file = '''
pub const CONTROL: u32 = 0x000;
pub const CONTROL_REGISTER: u32 = 0x000;
pub const RESET_BITS: u32 = 0x7;
mod inner { pub const STATUS: u32 = 0x04; }
'''
    for path, want in (('src/registers.rs', ['const CONTROL', 'const STATUS']),
                       ('crate/src/registers/mod.rs', ['const CONTROL', 'const STATUS']),
                       ('src/host/registers.rs', ['const CONTROL', 'const STATUS']),
                       ('src/register_offsets/control.rs', ['const CONTROL', 'const STATUS']),
                       ('src/host.rs', []),
                       ('/checkout/registers/src/host.rs', [])):
        bad = judge(path, rust_names.file_definitions(path, in_file), words)
        got = sorted(line.split(': ')[1] for line in bad)
        if got != want:
            failed += 1
            print(f'register_names selftest: {path} refused {got}, wanted {want}', file=sys.stderr)
    src = '''
pub const SEND_RELATIVE_ADDRESS: u8 = 3;
pub struct Card { relative_card_address: u16, first_logical_block_address: u64 }
pub fn block_address_argument(logical_block_address: u64) -> u32 { 0 }
pub fn can_address_blocks() -> bool { true }
pub fn address_and_select() {}
fn a_byte_addressed_card_is_read_by_addressing_it() {}
pub const PHYSICAL_FRAME_ADDRESS: u64 = 0;
pub const RELATIVE_ADDRESS_OFFSETS: u64 = 0;
pub fn bus_address() -> u64 { 0 }
pub fn address() -> u64 { 0 }
/// **Interrupt Mask Register** (OpenBSD's `SDMMC_INTMASK`).
#[doc(alias = "INTMASK")]
pub const INTERRUPT_MASK_REGISTER: u32 = 0x024;
pub const MASK_STATUS_REGISTER: u32 = 0x028;
/// **CMD16, set block length** (`SET_BLOCKLEN` in both specifications).
#[doc(alias = "SET_BLOCKLEN")]
pub const SET_BLOCK_LENGTH: u8 = 16;
pub const READ_LENGTH: usize = 512;
/// **Abnormal interrupt summary**: any of the abnormal bits (`FBE`, `DU` or `CES`).
#[doc(alias = "abnormal interrupt summary")]
pub const ABNORMAL_INTERRUPT_SUMMARY: u32 = 1 << 9;
/// **Card detect** (`CDETECT`, OpenBSD's `SDMMC_CDETECT`).
pub const CARD_DETECT_REGISTER: u32 = 0x050;
/// **Card detect** (`CDETECT`, `dwmmc.h`).
pub const CARD_DETECTED: u32 = 0x050;
/// **Card detect** (`CDETECT`, section 7.2).
pub const CARD_PRESENT: u32 = 0x050;
/// **Card detect** (`CDETECT`, the databook).
#[doc(alias = "CDETECT")]
pub const CARD_DETECT_BIT: u32 = 1;
'''
    bad = judge('src/sd.rs', rust_names.definitions(src), words)
    got = sorted(line.split(': ')[1] for line in bad)
    want = sorted(['const PHYSICAL_FRAME_ADDRESS', 'const RELATIVE_ADDRESS_OFFSETS',
                   'fn bus_address', 'fn address', 'const MASK_STATUS_REGISTER',
                   'const READ_LENGTH', 'const CARD_DETECT_REGISTER', 'const CARD_DETECTED',
                   'const CARD_PRESENT'])
    if got != want:
        failed += 1
        print(f'register_names selftest: false positives: refused {got}, wanted {want}',
              file=sys.stderr)
    by_name = {line.split(': ')[1]: line for line in bad}
    if 'If it mirrors' in by_name.get('const READ_LENGTH', 'If it mirrors'):
        failed += 1
        print('register_names selftest: an integer constant is never offered the kept form',
              file=sys.stderr)
    if 'If it mirrors' not in by_name.get('fn bus_address', ''):
        failed += 1
        print('register_names selftest: a function is offered the kept form', file=sys.stderr)
    return failed


if __name__ == '__main__':
    sys.exit(run(sys.argv[1:]))
