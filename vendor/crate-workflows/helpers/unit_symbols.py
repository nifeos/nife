#!/usr/bin/env python3
"""A name spells its unit: `_MILLISECONDS`, `_HERTZ`, `_BYTES`, never `_MS`, `_HZ` or `_KB`.

    python3 unit_symbols.py [--manifest Cargo.toml] [--report] FILE...
    python3 unit_symbols.py --selftest

An uppercase constant erases the case a unit symbol depends on (`MHZ` is also `mHz`, and `_MS`
could be megasiemens), and the micro sign has no ASCII form, so no symbol is a valid suffix. The
symbols and what replaces each are `[units]`, `[units_after_per]` and `[units_after_number]` in
helpers/naming_words.toml. A bare `s` counts only straight after `per` or a number (`kib_per_s`,
`within_30_s`, never the possessive in `fat32s`), and `min` only straight after `per` (`SECONDS_PER_MIN`); elsewhere they are a
variable and a minimum.

The names judged are the ones the crate defines (helpers/rust_names.py says which), so a call to
`Duration::as_nanos` or `from_millis` is never a finding: the standard library defined those. A
name the crate defines to mirror a source keeps its spelling in the kept form rust_names.py
describes. There is no baseline: the ceiling is 0, and every finding fails.

The rule is the naming rules page's unit ruling, in the operating system project this template
serves (design/naming.md there). The file's name is provisional.
"""

import os
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import rust_names  # noqa: E402

RULE = 'a name spells its unit'


def hits_of_words(words):
    units = words.get('units', {})
    after_per = words.get('units_after_per', {})
    after_number = words.get('units_after_number', {})

    def hits(definition):
        parts = rust_names.components(definition.name)
        out = []
        for k, part in enumerate(parts):
            previous = parts[k - 1] if k else ''
            if part in units:
                hit = (part, units[part])
            elif previous == 'per' and part in after_per:
                hit = (part, after_per[part])
            elif previous.isdigit() and part in after_number and re.search(
                    rf'(?:^|_)[0-9]+_{part}(?:_|$)', definition.name, re.I):
                # Its own part after a number part (`within_30_s`); `gmac0s` and `fat32s` are
                # possessives, not seconds.
                hit = (part, after_number[part])
            else:
                continue
            if hit not in out:
                out.append(hit)
        return out
    return hits


def selftest():
    hits = hits_of_words(rust_names.load_words())
    src = '''
pub const TIMEOUT_MS: u32 = 10;
pub const CLOCK_MHZ: u32 = 24;
pub fn delay_us(us: u32) { let within_30_s = 1; let kib_per_s = 2; }
pub const SECONDS_PER_MIN: u64 = 60;
pub const NANOS_PER_SEC: u64 = 1;
pub const BUFFER_KB: usize = 4;
fn gmac0s_bits_and_fat32s_fields() {}
pub fn fine(d: core::time::Duration) -> u128 { let s = 1; let min = 2; d.as_nanos() }
pub const TIMEOUT_MILLISECONDS: u32 = 10;
pub const REFERENCE_HERTZ: u32 = 1;
/// The bus clock; the alias and this doc say MHz.
#[doc(alias = "mhz")]
pub const BUS_MEGAHERTZ: u32 = 1;
/// **Timeout Value** (TO_MS, a vendor databook, register table 4).
#[doc(alias = "timeout value")]
pub const TO_MS: u32 = 0;
/// **Timeout Value** (TimeoutMs, a vendor databook, register table 4).
#[doc(alias = "timeout value")]
pub struct TimeoutMs;
// pub const IN_A_COMMENT_MS: u32 = 0;
'''
    found = rust_names.definitions(src)
    bad = rust_names.judge('x.rs', found, hits)
    names = [line.split(': ')[1] for line in bad]
    want = ['const TIMEOUT_MS', 'const CLOCK_MHZ', 'fn delay_us', 'parameter us',
            'binding within_30_s', 'binding kib_per_s', 'const SECONDS_PER_MIN',
            'const NANOS_PER_SEC', 'const BUFFER_KB', 'const TO_MS']
    failed = 0
    if names != want:
        failed += 1
        print(f'unit_symbols selftest: refused {names}, wanted {want}', file=sys.stderr)
    msg = {line.split(': ')[1]: line for line in bad}
    for name, needle in (('const TIMEOUT_MS', 'write `milliseconds`'),
                         ('const CLOCK_MHZ', 'write `megahertz`'),
                         ('const SECONDS_PER_MIN', 'write `minute`'),
                         ('binding within_30_s', 'write `seconds`'),
                         ('const NANOS_PER_SEC', '`sec` is spelled out: write `seconds`'),
                         ('const TO_MS', 'integer constant')):
        if needle not in msg.get(name, ''):
            failed += 1
            print(f'unit_symbols selftest: {name} message lacks {needle!r}: {msg.get(name)}',
                  file=sys.stderr)
    if failed:
        return 1
    print('unit_symbols selftest: ok')
    return 0


if __name__ == '__main__':
    sys.exit(rust_names.run(sys.argv[1:], 'unit_symbols', RULE, hits_of_words, selftest))
