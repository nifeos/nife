"""The passive-voice counter: one definition, which the prose ratchet and `script/metrics` share.

Milestone 952 (a passive-voice ratchet in lint), for rule 6 of §213 (writing standards): new prose
carries at most 2 passives per 1,000 prose words (calef, 2026-10-25 UTC). `helpers/prose_ratchet.py`
decides which words are prose and calls `count` on them; `script/metrics` reaches this file only
through `prose_ratchet.measured`, so the gate and the chart cannot disagree about a sentence.

Name: provisional, minted 2026-10-25 (UTC) by milestone/952-a-passive-voice-ratchet. A module under
`helpers/`, which `script/names` leaves out of its scope, so this paragraph is its provenance. calef
names things.

What it matches. A form of "be" (am, are, is, was, were, be, been, being), whitespace, then a word
ending in "ed" or one of the irregular past participles below, as whole words, in any case. That is
the `write-good.Passive` rule of the Vale style `errata-ai/write-good`, adopted here rather than
depended on (AGENTS.md rule 6: copying a word list and a regex adopts code, it takes no dependency,
and Vale stays out of the tree and out of CI). The style is itself based on Brian Ford's
`btford/write-good`. Its license, which travels with the copied list:

    The MIT License (MIT)

    Copyright (c) 2014 Brian Ford

    Permission is hereby granted, free of charge, to any person obtaining a copy of this software
    and associated documentation files (the "Software"), to deal in the Software without
    restriction, including without limitation the rights to use, copy, modify, merge, publish,
    distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in all copies or
    substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
    BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
    NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
    DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
    OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

What it cannot see, and says so (design/roadmap/0952-a-passive-voice-ratchet.md, "BUGS"): a word
between the auxiliary and the participle ("is not used", "was then built") hides a passive, and
an adjective ending in "ed" after "is" ("is unchanged") reads as one. `NOT_PASSIVE` removes the
false readings that this tree repeats often enough to move a count.
"""

import re

AUXILIARIES = ('am', 'are', 'were', 'being', 'is', 'been', 'was', 'be')

# write-good's irregular past participles, in its order, less two British forms the house style
# refuses (helpers/house_style.py). Their American forms, learned and dreamed, end in "ed".
IRREGULAR = (
    'awoken', 'beat', 'become', 'been', 'begun', 'bent', 'beset', 'bet', 'bid', 'bidden', 'bitten',
    'bled', 'blown', 'born', 'bought', 'bound', 'bred', 'broadcast', 'broken', 'brought', 'built',
    'burnt', 'burst', 'cast', 'caught', 'chosen', 'clung', 'come', 'cost', 'crept', 'cut', 'dealt',
    'dived', 'done', 'drawn', 'driven', 'drunk', 'dug', 'eaten', 'fallen', 'fed', 'felt',
    'fit', 'fled', 'flown', 'flung', 'forbidden', 'foregone', 'forgiven', 'forgotten', 'forsaken',
    'fought', 'found', 'frozen', 'given', 'gone', 'gotten', 'ground', 'grown', 'heard', 'held',
    'hidden', 'hit', 'hung', 'hurt', 'kept', 'knelt', 'knit', 'known', 'laid', 'lain', 'leapt',
    'led', 'left', 'lent', 'let', 'lighted', 'lost', 'made', 'meant', 'met', 'misspelt',
    'mistaken', 'mown', 'overcome', 'overdone', 'overtaken', 'overthrown', 'paid', 'pled', 'proven',
    'put', 'quit', 'read', 'rid', 'ridden', 'risen', 'run', 'rung', 'said', 'sat', 'sawn', 'seen',
    'sent', 'set', 'sewn', 'shaken', 'shaven', 'shed', 'shod', 'shone', 'shorn', 'shot', 'shown',
    'shrunk', 'shut', 'slain', 'slept', 'slid', 'slit', 'slung', 'smitten', 'sold', 'sought',
    'sown', 'sped', 'spent', 'spilt', 'spit', 'split', 'spoken', 'spread', 'sprung', 'spun',
    'stolen', 'stood', 'stridden', 'striven', 'struck', 'strung', 'stuck', 'stung', 'stunk', 'sung',
    'sunk', 'swept', 'swollen', 'sworn', 'swum', 'swung', 'taken', 'taught', 'thought', 'thrived',
    'thrown', 'thrust', 'told', 'torn', 'trodden', 'understood', 'upheld', 'upset', 'wed', 'wept',
    'withheld', 'withstood', 'woken', 'won', 'worn', 'wound', 'woven', 'written', 'wrung',
)

# Words the pattern matches that are not a participle in this tree, each with the reason. Measured
# on 2026-10-25 (UTC) over the prose ratchet's 2,688 documents: these three account for 776 of
# 28,226 matches, and a sample of each read as no passive at all. Keep it short: an entry here
# exempts every use of the word, so it must be wrong almost never as a verb.
NOT_PASSIVE = {
    'rung': 'the noun: a rung of AGENTS.md\'s ladder ("is rung two"), 349 matches, never the bell',
    'red': 'the color of a risk or a run ("is red on main"), 97 matches; "red" is no participle',
    'gone': '"go" takes no object, so it has no passive; "is gone" states that a thing left, '
            '330 matches',
}

# write-good's rule is one regex, `\b(am|are|...)\b\s*` then `\w+ed` or a word of IRREGULAR. This
# is the same match in two steps, for speed: the pattern finds an auxiliary and peeks at the next
# word without consuming it, and `_participle` tests that word. Peeking keeps "is is checked"
# finding its passive, as the one regex does.
PATTERN = re.compile(r'\b(?:%s)\s+(?=(\w+))' % '|'.join(AUXILIARIES), re.I)
_IRREGULAR = frozenset(IRREGULAR)


def _participle(word):
    word = word.lower()
    return (len(word) > 2 and word.endswith('ed') or word in _IRREGULAR) and word not in NOT_PASSIVE


def matches(text):
    """Every passive construction in `text`, as `re.Match` objects, `NOT_PASSIVE` left out.

    `text` is prose already: the caller strips code, comments, quotes and markup first
    (`prose_ratchet.passive_count`). Group 1 is the participle.
    """
    return [m for m in PATTERN.finditer(text) if _participle(m.group(1))]


def count(text):
    """How many passive constructions `text` holds. See `matches`."""
    return len(matches(text))


def selftest():
    """[failure] for each reading the counter gets wrong; empty when every case holds."""
    cases = [
        ('a regular participle', 'The row is checked by lint.', 1),
        ('an irregular participle', 'It was written in July and is built daily.', 2),
        ('any case', 'IS CHECKED and Was Built', 2),
        ('a line break between the two words', 'The gate is\nrefused.', 1),
        ('active voice', 'Lint checks the row. calef wrote it.', 0),
        ('a word between hides it, a known gap', 'It is not used.', 0),
        ('"is" inside a longer word', 'This isolated case was fine.', 0),
        ('the ladder\'s rung', 'That is rung two of the ladder.', 0),
        ('a color', 'The run is red on main.', 0),
        ('an intransitive verb', 'The file is gone.', 0),
        ('every auxiliary', 'am told, are told, were told, being told, is told, been told, '
                            'was told, be told', 8),
    ]
    return ['%s: counted %d, want %d' % (name, count(text), want)
            for name, text, want in cases if count(text) != want]
