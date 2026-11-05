#!/usr/bin/env python3
"""A function that answers yes or no is named like a question: `is_empty`, not `empty`.

    python3 question_shape.py [--manifest Cargo.toml] [--report] FILE...
    python3 question_shape.py --selftest

A function is judged when it returns `bool` and takes no `&mut` argument. Its name passes when it
starts with `is`, `has`, `can`, `was`, `will`, `all` or `any`, or reads as a finite verb (the first
or last word ends in `s` and not `ss`, as in `contains` and `grant_allows`), or holds an inner `is`,
`has`, `can` or `was` (`machine_has_no_rtc`). Two shapes are never questions and pass: a function
that takes `&mut` acts and reports whether it worked (`insert`), and `take_` clears a flag and
reports it. The check reads function names the crate defines (helpers/rust_names.py says which); a
trait method in an `impl Trait for Type` block is the trait's name and is skipped.

A name that mirrors a datasheet, a specification or the standard library keeps its spelling in the
kept form rust_names.py describes, at the item: the doc comment's first paragraph opens with
`(<name>, <source>)` and `#[doc(alias)]` carries a plain-English name. There is no baseline: the
ceiling is 0.

Known false pass: a plural noun as the first or last word reads as a finite verb, because the check
sees spelling and not grammar. A known false stop: a function that acts and reports whether it
worked, but takes its state through a shared reference or a global instead of `&mut`; it is kept in
the form above.

The rule follows the naming rules page of the operating system project this template serves
(design/naming.md there, "a question has the shape of a question"). The file's name is provisional.
"""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import rust_names  # noqa: E402

RULE = 'a function answering yes or no is named like a question'

QUESTION_FIRST = {'is', 'has', 'can', 'was', 'will', 'all', 'any'}
QUESTION_INNER = {'is', 'has', 'can', 'was'}
ACTS_FIRST = {'take'}
ADVICE = ('returns `bool` and is not named like a question: start with `is_`, `has_`, `can_`, '
          '`was_`, `will_`, `all_` or `any_`, or use a finite verb such as `contains`')


def is_question(words):
    first, last = words[0], words[-1]
    return (first in QUESTION_FIRST or first in ACTS_FIRST
            or (first.endswith('s') and not first.endswith('ss'))
            or (len(words) > 1 and last.endswith('s') and not last.endswith('ss'))
            or any(w in QUESTION_INNER for w in words[1:]))


def hits_of_words(_words):
    def hits(definition):
        if definition.kind != 'fn' or not definition.returns_bool or definition.takes_mut:
            return []
        words = rust_names.components(definition.name)
        if not words or is_question(words):
            return []
        return [(definition.name, ADVICE)]
    return hits


def phrase(_name, advice):
    return advice


def selftest():
    hits = hits_of_words({})
    src = '''
pub fn empty(&self) -> bool { true }
pub fn is_empty(&self) -> bool { true }
pub fn contains(&self, x: u8) -> bool { true }
pub fn grant_allows(&self) -> bool { true }
pub fn machine_has_no_rtc() -> bool { true }
pub fn take_pending(&self) -> bool { true }
pub fn insert(&mut self, x: u8) -> bool { true }
pub fn insert_lifetime<'a>(&'a mut self) -> bool { true }
pub fn ready(&self) -> bool { true }
pub fn class(&self) -> bool { true }
pub fn count(&self) -> u8 { 0 }
pub fn wrapped(&self) -> Option<bool> { None }
pub fn path() -> bool::Output { 0 }
pub const fn valid(flags: u8) -> bool where u8: Copy { true }
pub trait Probe { fn present(&self) -> bool; }
impl Probe for Thing { fn present(&self) -> bool { true } }
/// The enable bit (enable, Example Datasheet 1.0, section 2).
#[doc(alias = "enable bit")]
pub fn enable(&self) -> bool { true }
'''
    found = rust_names.definitions(src)
    bad = rust_names.judge('x.rs', found, hits, phrase)
    names = [line.split(': ')[1] for line in bad]
    want = ['fn empty', 'fn ready', 'fn class', 'fn valid', 'fn present']
    failed = 0
    if names != want:
        failed += 1
        print(f'question_shape selftest: refused {names}, wanted {want}', file=sys.stderr)
    if bad and 'start with `is_`' not in bad[0]:
        failed += 1
        print(f'question_shape selftest: message lacks the advice: {bad[0]}', file=sys.stderr)
    if failed:
        return 1
    print('question_shape selftest: ok')
    return 0


if __name__ == '__main__':
    sys.exit(rust_names.run(sys.argv[1:], 'question_shape', RULE, hits_of_words, selftest, phrase))
