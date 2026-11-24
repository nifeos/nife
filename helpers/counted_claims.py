"""Reading a counted claim: the number a `<!--count...:NAME-->` marker vouches for.

`script/lint` checks every marker against the tree; `script/metrics` reads one back as the line a
chart draws (the unsafe chart's ceiling is `unsafe-density-outside-arch`'s). Both read it here, so
a chart cannot draw a ceiling the gate is not holding. partsfoundry/record:notes/counted-claims.md is the convention.

Name: provisional, minted by metrics/unsafe-chart-matches-lint on 2026-10-19 (UTC), after the
document it reads for. A shared python module under `helpers/`, which `script/names` puts out of
its own scope, so its provenance is this paragraph.
"""

import re

# THREE relations, and neither of the other two is equality, because equality is the wrong one for
# any quantity that moves on its own.
#
#   <!--count:NAME-->           the number must EQUAL the tree. A census.
#   <!--count-at-least:NAME-->  the number must be a FLOOR. Fires only when the count goes DOWN.
#   <!--count-at-most:NAME-->   the number must be a CEILING. Fires only when the count goes UP.
#
# Why the floor (2026-08-17): `kani-harnesses` was marked in four claim sites and the count moved
# three times in one day, so every lane that added a harness had to edit four files, one of them
# README.md, which nearly every other lane touches too. That is a merge hotspot manufactured by the
# convention rather than by the tree. Reading what each site actually claimed split them cleanly:
# two are a census and want equality; two were conveying **scale** ("121 Kani harnesses across 21
# crates: the capability model, IPC, MMU isolation, ..."), and for a monotonically growing quantity
# a floor is the *true* claim while an exact equality is a maintenance tax that buys nothing.
#
# The ratchet is intact, which is the only test that matters here: a floor that goes false still
# fails the build. Deleting harnesses below a stated floor is exactly the event worth catching, and
# it is the one an equality check would have drowned in noise.
#
# **Why the ceiling (milestone 134, 2026-08-18), and it is the floor's mirror rather than a third
# idea.** A floor suits a quantity where MORE IS BETTER and the bad event is a deletion. Some
# quantities are the other way up, and unsafe is the tree's clearest one: nobody wants more of it,
# every addition should be a deliberate act with a written reason, and the bad event is a drift
# upward that no single commit looks responsible for. calef raised it as one question -- "is that
# something we should be monitoring and driving in a particular direction over time?" -- and a
# direction is exactly what the other two relations cannot express. Equality says "this is the
# number"; a floor says "at least"; only a ceiling says "and it should not grow".
#
# **What a ceiling is NOT.** It is not zero-slack, and this was measured before it was chosen. Over
# the 14 days to 2026-08-18, 38 non-merge commits changed the number of `unsafe {` outside
# kernel/src/arch/. A ceiling written at exactly the tree's value would have fired on all 38 and
# sent every one of them to edit the same line of the same note, which is precisely the merge
# hotspot the paragraph above records the convention manufacturing once already. A ceiling is
# written ABOVE the tree, with the headroom stated, so it fires on drift rather than on a lane.
#
# The exception is a population small and consequential enough that every addition deserves the
# stop: `unsafe-thread-safety-claims` is written at the tree's exact value for that reason, and it
# moved twice in three weeks rather than 38 times in two.
MARKER = re.compile(r'<!--count(-at-least|-at-most)?:([a-z0-9-]+)-->')

# Prose explaining this convention has to be able to spell it, the same exemption `script/lint`'s
# vocabulary check carries for the documents that argue about a word. A marker inside a backtick
# span is being QUOTED, not asserted, so blank the spans before looking. Blanked rather than
# deleted so that column positions still line up with the number-before-marker search.
CODE_SPAN = re.compile(r'`[^`]*`')

# The number a marker vouches for is the LAST number on the line before the marker, commas allowed
# so that `1,303 commits` works. Last rather than first, because a sentence usually reaches its
# number at the end ("across 21 crates") and because a line with two counts wants each marker to
# bind to the number just before it -- partsfoundry/record:notes/unsafe-obligations.md is the live case, and it says
# crates before harnesses, the opposite order to everywhere else.
#
# **A count spelled in words counts too**, added by the 2026-08-17 documentation sweep, which
# needed it: the claim it went to gate was "the surface is three calls", and small counts in prose
# about a design are written as words far more often than as digits. partsfoundry/record:notes/counted-claims.md
# recorded "digits or nothing" as a limitation, and this is that limitation closed rather than a
# second mechanism -- one regex and the same opt-in marker.
#
# Only small cardinals, and deliberately no composition ("twenty-one" is not read). A count large
# enough to want composing is one a person writes in digits anyway, and a parser that accepts
# "one hundred and twenty" would be guessing at prose. If a marked word is not in this table the
# gate says so rather than skipping the claim, which is the loud direction.
#
# This changes nothing about the ratchet: only a MARKED number is ever read, so admitting words
# cannot produce a false positive on the ordinary prose that says "the three of them".
WORD_VALUES = {
    'one': 1, 'two': 2, 'three': 3, 'four': 4, 'five': 5, 'six': 6, 'seven': 7, 'eight': 8,
    'nine': 9, 'ten': 10, 'eleven': 11, 'twelve': 12, 'thirteen': 13, 'fourteen': 14,
    'fifteen': 15, 'sixteen': 16, 'seventeen': 17, 'eighteen': 18, 'nineteen': 19, 'twenty': 20,
    'thirty': 30, 'forty': 40, 'fifty': 50, 'sixty': 60, 'seventy': 70, 'eighty': 80,
    'ninety': 90, 'hundred': 100,
}
# Longest first, so `fourteen` is never read as `four`. The word boundaries make that safe already;
# the ordering means it stays safe if a boundary is ever relaxed.
NUMBER = re.compile(
    r'\d[\d,]*|\b(?:' + '|'.join(sorted(WORD_VALUES, key=len, reverse=True)) + r')\b',
    re.I,
)


def claimed_value(text):
    """The last number-like token in `text` as an int, or None if there is none.

    Returns None for a word this table does not carry as well as for no number at all; the caller
    reports both, because a marker whose number cannot be read is a claim nobody is checking and
    that is the thing this gate exists to refuse.
    """
    token = None
    for hit in NUMBER.finditer(text):
        token = hit.group(0)
    if token is None:
        return None
    if token[0].isdigit():
        return int(token.replace(',', ''))
    return WORD_VALUES.get(token.lower())


# Markdown fences: a marker inside ``` is an EXAMPLE of the convention, not a claim
# about the tree: the roadmap block and partsfoundry/record:notes/counted-claims.md both show one, and both would
# otherwise be checked against numbers they were never asserting. Fenced blocks only; an indented
# code block is not recognised, which is a limitation partsfoundry/record:notes/counted-claims.md records rather than
# hides. Markdown only, because a fence means nothing in any other file.
def claims(path, text):
    """(line number, relation, name, number) for each marker `text` asserts, in order.

    `relation` is `None` for a census, `-at-least` or `-at-most`. `number` is None when no number
    the table can read comes before the marker, which a caller must report, never skip.
    """
    inside = False
    for lineno, line in enumerate(text.split('\n'), 1):
        if path.endswith('.md') and line.lstrip().startswith('```'):
            inside = not inside
            continue
        if inside or '<!--count' not in line:
            continue
        line = CODE_SPAN.sub(lambda m: ' ' * len(m.group(0)), line)
        # A marker's own name must not donate a numeral to the marker after it:
        # `count:one-shot-reply` would read as the number 1 for the next claim on the line. Blanked
        # rather than deleted, so column positions still line up with the number-before-marker
        # search, the same trick CODE_SPAN uses. Searched from `line`, and blanked into a parallel
        # string, so the markers are still found.
        numbers_only = MARKER.sub(lambda m: ' ' * len(m.group(0)), line)
        for match in MARKER.finditer(line):
            yield (lineno, match.group(1), match.group(2),
                   claimed_value(numbers_only[:match.start()]))


def ceiling(name, files):
    """The ceiling the tree holds `name` to: the lowest `-at-most` marker for it across `files`,
    (path, text) pairs, or None where no marker does. Lowest, because each marker is checked on its
    own and the lowest is the one that fires first."""
    held = [number for path, text in files
            for _line, relation, marker, number in claims(path, text)
            if marker == name and relation == '-at-most' and number is not None]
    return min(held) if held else None
