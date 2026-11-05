#!/usr/bin/env python3
"""A name spells out the words the naming rules spell out: `receive`, not `rx`.

    python3 spelled_out.py [--manifest Cargo.toml] [--report] FILE...
    python3 spelled_out.py --selftest

The words and what replaces each are `[spelled_out]` in helpers/naming_words.toml, and the
failure message names the replacement. The names judged are the ones the crate defines
(helpers/rust_names.py says which), plus the package name with `--manifest`. A component matches
whole and in any casing, so `rx_ring`, `RxRing` and `RX_RING` all fail, while `capacity`, `maxima`
and `initial` do not. Comments, strings and `#[doc(alias)]` strings are never judged: the alias is
where the source's spelling belongs.

A name that mirrors a datasheet, a specification or the standard library keeps its spelling in the
kept form rust_names.py describes, at the item. There is no baseline: a new crate repository starts
at a ceiling of 0, and every finding fails.

The rule is the naming rules page's spelled-out rulings and its acronym test, in the operating
system project this template serves (design/naming.md there). This file holds the check, and the
word list holds the words; neither needs any other repository's files. The file's name is
provisional.
"""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import rust_names  # noqa: E402

RULE = 'a name spells out the words the naming rules spell out'


def hits_of_words(words):
    table = words.get('spelled_out', {})
    before = words.get('spelled_out_before', {})
    after_per = words.get('units_after_per', {})

    def hits(definition):
        parts = rust_names.components(definition.name)
        out = []
        for k, part in enumerate(parts):
            if part not in table:
                continue
            following = parts[k + 1] if k + 1 < len(parts) else None
            if following in before.get(part, ()):
                continue
            if k and parts[k - 1] == 'per' and part in after_per:
                continue  # `SECONDS_PER_MIN`: a unit, which unit_symbols.py judges
            if (part, table[part]) not in out:
                out.append((part, table[part]))
        return out
    return hits


def selftest():
    hits = hits_of_words(rust_names.load_words())
    src = '''
/// A ring of receive descriptors.
pub struct RxRing { pub tx_head: u32, capacity: usize }
pub const MAX_FRAME_BYTES: usize = 1518;
pub fn init(config: u8) { let initial = 0; let maxima = 1; }
/// The opt in switch.
pub fn opt_in() {}
pub const SECONDS_PER_MIN: u64 = 60;
#[doc(alias = "rx")]
#[doc(alias = "fifo")]
/// The receive queue; the alias and this doc mention rx and fifo.
pub struct ReceiveQueue;
/// **Controller Capabilities** (CAP, NVM Express Base Specification 2.0, section 3.1.1).
#[doc(alias = "controller capabilities")]
pub const CAP: u32 = 0;
#[doc(alias = "controller capabilities")]
pub const CAPS: u32 = 0;
// let rx = 1; is a comment
pub fn quoted() { let s = "irq"; }
'''
    found = rust_names.definitions(src)
    bad = rust_names.judge('x.rs', found, hits)
    names = [line.split(': ')[1] for line in bad]
    want = ['struct RxRing', 'field tx_head', 'const MAX_FRAME_BYTES', 'fn init', 'parameter config',
            'const CAPS']
    failed = 0
    if names != want:
        failed += 1
        print(f'spelled_out selftest: refused {names}, wanted {want}', file=sys.stderr)
    msg = {line.split(': ')[1]: line for line in bad}
    for name, needle in (('struct RxRing', '`rx` is spelled out: write `receive`'),
                         ('const MAX_FRAME_BYTES', 'write `maximum`'),
                         ('const CAPS', 'states no source'),
                         ('fn init', 'open its doc comment with `(init, <source>)`')):
        if needle not in msg.get(name, ''):
            failed += 1
            print(f'spelled_out selftest: {name} message lacks {needle!r}: {msg.get(name)}',
                  file=sys.stderr)
    if 'parameter config' in msg and 'doc comment' in msg['parameter config']:
        failed += 1
        print('spelled_out selftest: a parameter cannot be kept, so its message offers no kept form',
              file=sys.stderr)
    if hits(rust_names.Definition('my_nvme_crate', 'package', 1)) != [
            ('nvme', 'non_volatile_memory_express')]:
        failed += 1
        print('spelled_out selftest: a package name is judged', file=sys.stderr)
    if failed:
        return 1
    print('spelled_out selftest: ok')
    return 0


if __name__ == '__main__':
    sys.exit(rust_names.run(sys.argv[1:], 'spelled_out', RULE, hits_of_words, selftest))
