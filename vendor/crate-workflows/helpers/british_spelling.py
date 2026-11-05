#!/usr/bin/env python3
"""A name uses American spelling: `color`, `initialize`, `canceled`.

    python3 british_spelling.py [--manifest Cargo.toml] [--report] FILE...
    python3 british_spelling.py --selftest

The house spelling is American. A name the crate defines (helpers/rust_names.py says which), and
the package name with `--manifest`, holds no component that is a British form; the failure message
names the American one. The forms are in helpers/naming_words.toml: a table of single words
(`[british]`) and three families that generate their forms from a stem and its endings
(`[british_ise]`, `[british_our]`, `[british_re]`), so `initialise`, `initialisation` and
`initialised` are one line. A stem not listed is never read, so `rise`, `promise` and `exercise`
pass. Comments, strings and `#[doc(alias)]` strings are not judged.

The list is a word list, not a dictionary: it holds the forms that turn up in code, and a miss is
added to the toml in the change that meets it. A name that mirrors a datasheet, a specification or
the standard library keeps its spelling in the kept form rust_names.py describes, at the item. There
is no baseline: the ceiling is 0.

The rule follows the naming rules page of the operating system project this template serves
(design/naming.md there, the spelling section). The file's name is provisional.
"""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import rust_names  # noqa: E402

RULE = 'a name uses American spelling'


def forms(words):
    """`{british: american}` from the explicit table and the three generated families."""
    table = dict(words.get('british', {}))
    ise = words.get('british_ise', {})
    for stem in ise.get('stems', ()):
        for ending in ise.get('endings', ()):
            table.setdefault(f'{stem}is{ending}', f'{stem}iz{ending}')
    our = words.get('british_our', {})
    for stem in our.get('stems', ()):
        for ending in our.get('endings', ()):
            table.setdefault(stem + ending, stem + ending.replace('our', 'or', 1))
    re_ = words.get('british_re', {})
    for stem in re_.get('stems', ()):
        for ending in re_.get('endings', ()):
            table.setdefault(stem + ending, stem + ending.replace('re', 'er', 1))
    return table


def hits_of_words(words):
    table = forms(words)

    def hits(definition):
        out = []
        for part in rust_names.components(definition.name):
            if part in table and (part, table[part]) not in out:
                out.append((part, table[part]))
        return out
    return hits


def phrase(word, instead):
    return f'`{word}` is a British spelling: write `{instead}`'


def selftest():
    words = rust_names.load_words()
    table = forms(words)
    failed = 0
    for british, american in (('initialise', 'initialize'), ('initialisation', 'initialization'),
                              ('colour', 'color'), ('behavioural', 'behavioral'),
                              ('centre', 'center'), ('centred', 'centered'),
                              ('cancelled', 'canceled'), ('serialiser', 'serializer')):
        if table.get(british) != american:
            failed += 1
            print(f'british_spelling selftest: {british} -> {table.get(british)}, wanted {american}',
                  file=sys.stderr)
    for fine in ('rise', 'promise', 'exercise', 'color', 'center', 'canceled', 'for', 'four',
                 'hour', 'your', 'spectre', 'real'):
        if fine in table:
            failed += 1
            print(f'british_spelling selftest: {fine} must pass', file=sys.stderr)
    hits = hits_of_words(words)
    src = '''
pub struct ColourMap { grey_level: u8 }
pub fn initialise_all(cancelled: bool) {}
pub const FAVOURITE_COLOUR: u8 = 0;
/// The tint (tintre, Example Specification 1).
#[doc(alias = "tint")]
pub const TINTRE: u8 = 0;
/// A grey table; the doc and the alias may say grey.
#[doc(alias = "grey")]
pub fn gray_table() { let promise = 1; }
pub fn exercise_rise() {}
'''
    bad = rust_names.judge('x.rs', rust_names.definitions(src), hits, phrase)
    names = [line.split(': ')[1] for line in bad]
    want = ['struct ColourMap', 'field grey_level', 'fn initialise_all', 'parameter cancelled',
            'const FAVOURITE_COLOUR']
    if names != want:
        failed += 1
        print(f'british_spelling selftest: refused {names}, wanted {want}', file=sys.stderr)
    if not bad or 'write `color`' not in bad[0]:
        failed += 1
        print(f'british_spelling selftest: message {bad[:1]}', file=sys.stderr)
    if failed:
        return 1
    print('british_spelling selftest: ok')
    return 0


if __name__ == '__main__':
    sys.exit(rust_names.run(sys.argv[1:], 'british_spelling', RULE, hits_of_words, selftest, phrase))
