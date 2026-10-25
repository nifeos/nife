"""The prose ratchet: §212's word cap and §213's density rules, held to a baseline that only falls.

Milestone 586 (a prose ratchet in lint). calef ratified both decisions on 2026-09-23 with a ratchet
as their enforcement and no gate built, and the budget slipped twice on 2026-09-24 because nothing
checked it: a lane moved facts into notes already over the cap and each note grew. This is the gate.

    python3 helpers/prose_ratchet.py --check          # what script/lint runs
    python3 helpers/prose_ratchet.py --report PATH..  # one document's measures, and why
    python3 helpers/prose_ratchet.py --bank           # lower the baseline to the tree; never raises
    python3 helpers/prose_ratchet.py --init           # write the baseline from scratch (milestone 586 only)
    python3 helpers/prose_ratchet.py --remeasure      # once, in the change that bumps MEASURE (see there)
    python3 helpers/prose_ratchet.py --init-passive   # write the passive baseline (milestone 952 only)
    python3 helpers/prose_ratchet.py --remeasure-passive  # once, with a PASSIVE_MEASURE bump

Name: provisional, minted by milestone 586's lane on 2026-09-24. A shared python module under
`helpers/`, which `script/names` puts out of its own scope, so its provenance is this paragraph
rather than a `Name:` block. calef names things; expect this to change. Refused `prose_budget`,
which is §212's half alone.

**What it measures, per document.** Four things, each against a ratified limit:

| measure | limit | decision |
|---|---|---|
| words, whole file (`budget_words`) | 3,000 | §212 (a prose budget) |
| median sentence, in words | 20 | §213 (writing standards) rule 1 |
| longest sentence, in words | 40 | §213 rule 2 |
| bold spans per 1,000 words | 4 | §213 rule 3, held as two counts: line-opening and inline |
| passive constructions per 1,000 words | 2 | §213 rule 6, held in its own baseline |

**The ratchet.** A measure at or under its limit passes. A measure over it passes only if the
committed baseline (`design/prose-baseline.tsv`) records the document as already over on that
measure and the document has not got worse than the baseline says. A document missing from the
baseline, which is every new one, meets the limits outright. Against the merge base as well: a
document that shrank on `main` without anyone banking the shrink may not grow back into the slack,
because §212's rule is "may not grow", not "may not exceed a number written down once".

**The passive-voice half** (milestone 952, §213 rule 6, calef 2026-10-25 UTC: "Lets implement
A."). `helpers/passive_voice.py` counts passives over the prose the sentence limits read (blocks,
quotes out, inline code one word). A document of at least 200 prose words over 2 per 1,000 fails,
unless `design/passive-baseline.tsv` holds a row for it, and then its count may not rise above the
row nor above its count at the merge base. That file is its own: `--init-passive` wrote it once,
`--bank` lowers it, and nothing raises it. No touch rule applies, unlike bold: ruling A holds an
existing document to a count that only falls, and a document gains no allowance by being edited.

**Exceptions** are HTML comments in the document, where a reader meets them, and they need a date
and a `Reason:` so that an exception says out loud that it is one (AGENTS.md's ladder):

    <!-- prose-budget: exception. ... Ratified by calef on 2026-09-24 ... Reason: ... -->
    <!-- writing-standards: exception. ... on 2026-09-24 ... Reason: ... -->

`prose-budget` marks a §212 exception and `writing-standards` its §213 twin. Name: ratified
2026-10-19 (calef, PR #2425), each named for the decision it waives. An exception exempts its decision's measures entirely, which is what both decisions
say it does ("passes only if it did not get worse, or carries a marked exception").

**The orphan check.** An appendix is a file under `X/` beside a document `X.md` (§212's default
siting), and it must be linked from `X.md` or from `X/README.md`. `X/README.md` itself is the
directory's provenance page, not an appendix, and is exempt. A thematic appendix directory
(§212's permitted exception, `design/tenets/` today) is named in `THEMATIC_APPENDIX_DIRS` below and
its files must be linked from its own `README.md`. An appendix nothing links to is a lost document.

**What is counted, exactly**, because §213 records a first measurement that was wrong by ten words
of median and a gate built on it would fail documents that pass:

- §212's words are whitespace-separated tokens over the whole file, code, comments, frontmatter and
  tables included, less a generated table: `budget_words` (calef, 2026-10-13 UTC, "Gate counts every
  word").
- For §213's measures, fenced code, HTML comments and YAML frontmatter are not prose and are
  stripped before anything. Its 200-word floor and its bold density count the prose words left.
- Sentences are split on block boundaries first (blank lines, headings, list-item starts, table
  rows, blockquotes), and only then within a block, on `.`, `!` or `?` followed by whitespace and
  anything that can open a sentence, unless the word before the stop is an abbreviation. §213's
  splitter asked for a capital; this tree starts sentences with `calef` and `xenon`, so that one
  ran them together. Headings and table rows are not sentences (§213's own method stripped
  tables). These choices put the corpus lower than §213 measured: 17 words for the median
  document's median sentence, against 20.
- Quoted text is exempt from both sentence limits (586's design note of 2026-09-24): a verbatim
  quote cannot be rewrapped without changing what the speaker said. So a blockquote is skipped and
  a `"..."` span is removed from its sentence before the sentence is measured; the lead-in around
  it is still prose and still counts.
- Inline code is one word in a sentence, since it is verbatim, and all its words in the word count.
  A link counts as its text.
- Bold is `**...**` outside code and outside table rows (§213's table-cell precedent). It opens a
  line when nothing but indentation, a list marker or a blockquote marker precedes it. Bold inside
  a quote still counts: bold is the writer's markup, not the speaker's. Bold a script parses
  (`**Status:`, `**Built:**`, a Follow-on tag) is syntax and is not counted; see `derived_markers`.
- The median and the bold density are only asked of documents of at least 200 words, the floor
  §213's own per-document statistics used. A median of five sentences or a density over 90 words is
  a coin toss, and a new three-line README with one bold word would otherwise fail at 11 per 1,000.
  The longest-sentence limit applies to every document.

**Bold is judged on density in any document a change touches** (calef, 2026-09-26 UTC: "4 bolds
per 1000 is the right ratio for our written prose. That it was previously written without density
is irrelevant. Bold should be rare."). A document whose content differs from the merge base must
be at or under 4 bold per 1,000 words afterwards, and its baseline bold columns grant it nothing.
This was built first as two counts, because density is bold over words and a lane that condenses a
document raises it. The answer now is that whoever condenses a document removes its bold too.

Untouched documents passed on their baseline bold counts, so the tree did not go red on the day
of the ruling. That allowance is spent: the backlog reached zero on 2026-10-06 (UTC), the baseline's
bold columns are empty, and `check` refuses any bold ceiling there, so every document meets the
limit whether or not a change touches it. "Touched" is the diff against the merge base this module already takes for its
tight half, read with renames: a pure rename (100% similar) is not a touch, and an edit is.

**A mechanical rename is not a touch either** (calef, 2026-09-27T05:23Z, ruling on the
architect-role-census sweep, PR #1289: "a mechanical rename that adds no new sentence does not
count as touching a document for the §212/§213 prose ratchet"). That sweep's own substitution
adds a word almost everywhere it lands ("calef's" to "an architect's"), which both the word-count
ceiling and the bold-density touch rule read as growth though nobody wrote a new sentence.
`RENAME_PAIRS` names the forms this sweep produces, each mapped back to the form it replaced, and
`rename_masked` reads a line through that mapping before a document is measured or diffed: a line
whose only difference from its old version is one of these substitutions is read as unchanged; a
line that also carries a new sentence still differs after the mapping, and still counts. This is
the general mechanism the class exception asked for, so a future rename of this shape needs a new
pair in the list, not a new per-file exception.
"""

import os
import re
import subprocess
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import passive_voice  # noqa: E402
import roadmap_block  # noqa: E402

# --- a counted claim's number is not a touch --------------------------------------------------
#
# `script/lint`'s counted-claims check (notes/counted-claims.md) makes a lane that adds, say, a Kani
# harness bump every `<!--count:...-->`-marked number in the tree, and the touch rule below then
# held two 10,000-word notes to 4 bold per 1,000 words for a one-digit edit the other gate forced
# (found 2026-09-26 by the lane for milestone 23 (a capability-routed component OS with live
# replacement), adding one harness). So digits on a line carrying a count marker are masked before
# deciding whether a document was touched. Any other edit on that line, or anywhere else, still is.
COUNT_MARKER = re.compile(r'<!--count:[a-z0-9-]+-->')


def count_masked(text):
    return '\n'.join(re.sub(r'\d', '#', line) if COUNT_MARKER.search(line) else line
                     for line in text.split('\n'))

# --- a mechanical rename is not a touch ---------------------------------------------------------
#
# calef ruled 2026-09-27T05:23Z, on the architect-role-census sweep (PR #1289): "a mechanical rename
# that adds no new sentence does not count as touching a document for the §212/§213 prose ratchet."
# That sweep's own substitution ("calef's" to "an architect's", and the forms below) adds a word
# almost everywhere it lands, which the ceiling check and the touch rule both read as growth even
# though nobody wrote a new sentence. `RENAME_PAIRS` is the explicit list the ruling asks for: a
# form this sweep produced, mapped back to the form it replaced. `rename_masked` is applied
# wherever a document is measured or diffed against its merge-base copy, so a line whose only
# difference from its old version is one of these substitutions reads as unchanged; a line that
# also carries a new sentence still differs after the mapping, and still counts.
#
# Matched with `\s+` between a form's own words, not a literal string: the rewrap a longer
# replacement forces can land the break inside the phrase itself ("and those are an\n  architect's."
# was found this way, on 117's own sweep), and a literal match would miss exactly the case this
# exists for.
RENAME_PAIRS = (
    # The fatal-risks move of 2026-09-29 (calef's ruling: the summary becomes the directory's
    # README). Git pairs that rename only across the two commits that perform it, so the endpoint
    # diff this module reads sees none, and the sweep rewrites the path in a hundred other
    # documents. Masked like any mechanical rename: a repointed path is not a new sentence.
    ("design/fatal-risks.md", "design/fatal-risks/README.md"),
    ("calef has not ratified the package name itself", "no architect has ratified the package name itself"),
    ("calef has not ratified this instance", "no architect has ratified this instance"),
    ("calef has not ratified any of them", "no architect has ratified any of them"),
    ("calef has not ratified it", "no architect has ratified it"),
    ("Not ratified by calef.", "Not ratified by an architect."),
    ("waiting on calef", "waiting on an architect"),
    ("Calef names", "An architect names"),
    ("calef names", "an architect names"),
    ("Calef's", "An architect's"),
    ("calef's", "an architect's"),
    # The GitHub organization rename of 2026-10-03 (crickertech to nifeos): links, `gh` paths and
    # repo slugs in hundreds of dated records. Not a new sentence, so not a touch. The bare org
    # name is deliberately not a pair: prose that names it is a human edit.
    ("github.com/crickertech", "github.com/nifeos"),
    ("crickertech/nife", "nifeos/nife"),
    ("crickertech/basalt", "nifeos/basalt"),
    ("repos/crickertech", "repos/nifeos"),
    ("orgs/crickertech", "orgs/nifeos"),
    ("organizations/crickertech", "organizations/nifeos"),
    # The fixture rename of 2026-10-05 (calef, §185 (what carries the claim that userspace composes
    # a process from an authority you can count on one hand)). A path is navigation and is fixed
    # even in a BUILT block (design/naming.md, performing a ratified rename, step 7); the name in
    # that block's account stays.
    ("fixtures/src/address_space_witness.rs", "fixtures/src/process_composition_witness.rs"),
    # The bare form last: every longer phrase above is tried first, so by the time this one
    # runs, an "an architect" left in the text is not part of one of them, whichever case
    # sentence position gave it.
    ("Calef", "An architect"),
    ("calef", "an architect"),
)

_RENAME_RES = tuple(
    (re.compile(r'\s+'.join(re.escape(w) for w in new.split())), old)
    for old, new in RENAME_PAIRS)


# A promotion from `design/roadmap/proposals/` (2026-10-03, UTC) rewrites, in documents that cite
# the proposal, the path (a number is added and the directory dropped) and the `**Proposed.**`
# disposition (to `**Milestone N.**`, with the one-time gloss `script/citations` asks for). No
# sentence is new, and without this mask a promotion of the pile read as a touch of 40 baselined
# documents and put each under the bold rule. The path form is narrow on purpose: a number and a
# hyphen under `roadmap/`, two to four digits, since block numbers were padded to four on 2026-10-07
# (UTC) and a base older than that holds two- and three-digit names, which the same mask lets the
# padding rename compare equal. The label form folds every `**Milestone N.**` bullet label, glossed
# or not, to `**Proposed.**` on both sides of a comparison, so it hides a label edit and nothing
# else.
_PROMOTION_RES = (
    (re.compile(r'\*\*Milestone \d+\.\*\*(?: Milestone \d+ \([^)\n]*\)\.)?'), '**Proposed.**'),
    (re.compile(r'(roadmap/)\d{2,4}-'), r'\1proposals/'),
)


def promotion_masked(text):
    for pattern, old in _PROMOTION_RES:
        text = pattern.sub(old, text)
    return text


def rename_masked(text):
    for pattern, old in _RENAME_RES:
        text = pattern.sub(old, text)
    return promotion_masked(text)


def _flat(text):
    """Whitespace-insensitive, for comparing two copies of a paragraph a rename rewrapped: a word
    moved to a different line, at the same column limit, is not a new word."""
    return re.sub(r'\s+', ' ', text).strip()

# --- scope -------------------------------------------------------------------------------------
#
# The document scope, which `script/metrics`' writing-standards chart reads from here so the gate
# and the chart cannot drift: every `.md` directly under these directories, plus
# `AGENTS.md`. Directly under, so `design/audit-reports/` and `design/journeys/` are out, as the
# ratified figures had them.
#
# `design/roadmap/proposals/` is named outright (calef, 2026-10-06, UTC: "Lets ensure proposals
# have the 3000 word limit and eliminate the exemption"). It was out, so a proposal could grow
# past every limit and its promotion then needed a marked exception to land as a block. Those
# exceptions are gone with it: a proposal meets the limits when it is written, and promotion is a
# rename that the ratchet already reads as no new prose (`promotion_masked`).
#
# Appendices are added to it, because §212 puts them "under the same cap". Directly under alone
# would miss every one: `notes/benchmarks/` and `design/fatal-risks/` sit one directory down.
#
# `notes/coes/` is named outright (2026-10-03, UTC): a correction-of-error record is a document in
# its own right with no `notes/coes.md` parent, so the appendix rule never reached it and a 3,291-word
# COE passed lint.
PROSE_DIRS = ('design/', 'design/decisions/', 'design/roadmap/', 'design/roadmap/proposals/', 'notes/',
              'notes/coes/', 'briefs/')
PROSE_ROOT_FILES = ('AGENTS.md',)
PROSE_CAP = 3000

# §212's permitted exception to parent-named siting: a thematic directory whose appendices are
# independently citable. Its parent and the reason are in its own README.md, where §212 says the
# exception is marked; this is the machine-readable half of that, and the orphan check needs it
# because a thematic directory cannot be checked by a path rule.
THEMATIC_APPENDIX_DIRS = {'design/tenets/': 'AGENTS.md'}

LIMITS = {'words': 3000, 'median': 20, 'longest': 40, 'bold_per_1000': 4}
SMALL_DOCUMENT = 200  # words; below this, median and density are not asked (see the header)

BASELINE = 'design/prose-baseline.tsv'
COLUMNS = ('words', 'median', 'longest', 'bold_lead', 'bold_inline')
FAMILY = {'words': 'prose-budget', 'median': 'writing-standards', 'longest': 'writing-standards',
          'bold_lead': 'writing-standards', 'bold_inline': 'writing-standards'}


def _direct(path):
    return path in PROSE_ROOT_FILES or (
        '/' in path and path.rsplit('/', 1)[0] + '/' in PROSE_DIRS)


def appendix_parent(path, all_paths):
    """The document `path` is an appendix of, or None. `all_paths` is a set of tracked paths."""
    for d, parent in THEMATIC_APPENDIX_DIRS.items():
        if path.startswith(d):
            return parent
    parts = path.split('/')
    for i in range(len(parts) - 1, 0, -1):
        candidate = '/'.join(parts[:i]) + '.md'
        if candidate in all_paths and (_direct(candidate) or appendix_parent(candidate, all_paths)):
            return candidate
    return None


def documents(all_paths):
    """Every markdown path the prose rules apply to: the directories above plus appendices."""
    paths = set(all_paths)
    out = []
    for p in sorted(paths):
        if not p.endswith('.md') or p.startswith(('vendor/', 'target/')):
            continue
        if _direct(p) or appendix_parent(p, paths):
            out.append(p)
    return out


# --- reading a document ------------------------------------------------------------------------

FENCE = re.compile(r'^\s*(```|~~~)')
COMMENT = re.compile(r'<!--.*?-->', re.S)
FRONTMATTER = re.compile(r'\A---\n.*?\n---\n', re.S)
HEADING = re.compile(r'^\s{0,3}#{1,6}(\s|$)')
LIST_ITEM = re.compile(r'^\s*([-*+]|\d+[.)])\s+')
QUOTE_LINE = re.compile(r'^\s*>')
TABLE_ROW = re.compile(r'^\s*\|')
CODE_SPAN = re.compile(r'(`+)(.+?)\1', re.S)
LINK = re.compile(r'!?\[([^\]]*)\]\([^)]*\)')
BOLD = re.compile(r'\*\*(?=\S)(.+?)(?<=\S)\*\*', re.S)
QUOTED = re.compile(r'[*_]*["“][^"“”]*["”][*_]*')
SENTENCE_END = re.compile(r'[.!?][*_)\]"\'\u201d\u2019]*\s+(?=[A-Za-z0-9`"*\'\u201c(\[_\u00a7])')


# Tables a script writes, keyed by the heading that anchors them, and skipped like fenced code.
# `script/decisions` regenerated the table under `## The decisions` in design/decisions/README.md
# from every section's frontmatter, one row per section. Counting it made every new section a
# prose-budget failure on a document nobody had edited: the row was the only change, and no lane
# could cut words to pay for it without cutting someone else's prose. Found 2026-09-25 (UTC), the
# day the ratchet landed, when #1273 and #1278 each minted a section and each failed by one row.
#
# calef retired that table on 2026-10-12 (UTC), and its heading stays here for history:
# `script/metrics --backfill` measures each past week's own tree through this set, and a week that
# carried the table should not suddenly count its rows as prose.
#
# The six below are notes/project-metrics/baseline-drift.md's, written by `script/metrics` for
# milestone 415 (sub-tripwire drift accumulates across baseline saves). The appendix grows by one
# row per baseline save. helpers/baseline_drift.py's selftest fails if one of its headings is missing here.
GENERATED_TABLES = {'## The decisions'} | {
    '## %s: %s' % (arch, what)
    for arch in ('aarch64', 'riscv64', 'x86_64')
    for what in ('drift per row since the anchor', 'every save since the anchor, and its reason')}


def without_generated_tables(text):
    """`text` minus the rows of every table in GENERATED_TABLES, which `budget_words` counts
    through."""
    out, generated = [], False
    for line in text.split('\n'):
        if line.strip() in GENERATED_TABLES:
            generated = True
        elif generated and (not line.strip() or TABLE_ROW.match(line)):
            continue
        else:
            generated = False
        out.append(line)
    return '\n'.join(out)


def budget_words(text):
    """§212's word count: whitespace-separated tokens over the whole file, generated tables out.

    This gate's `words` measure (calef, 2026-10-13 UTC, "Gate counts every word"; §212 records
    it). §213's sentence and bold measures
    read prose only (`prose_lines`).

    Name: provisional, minted 2026-10-13 (UTC) by lane/prose-gate-whole-file. calef names things.
    """
    return len(without_generated_tables(text).split())


def prose_lines(text):
    """(line, in_code) for the text with frontmatter and comments removed. Code lines are dropped."""
    text = FRONTMATTER.sub('', text)
    # Keep the line count stable across a multi-line comment, so nothing downstream miscounts.
    text = COMMENT.sub(lambda m: '\n' * m.group(0).count('\n'), text)
    fence = None
    generated = False
    for line in text.split('\n'):
        if line.strip() in GENERATED_TABLES:
            generated = True
            yield line
            continue
        if generated:
            if not line.strip() or TABLE_ROW.match(line):
                continue
            generated = False
        m = FENCE.match(line)
        if fence:
            if m and m.group(1) == fence:
                fence = None
            continue
        if m:
            fence = m.group(1)
            continue
        yield line


def _code_to_word(s):
    # An inline code span is verbatim, like a quote: it cannot be rewrapped, so in a sentence it is
    # one word, whatever it holds. `Code` is capitalised so a sentence that opens with code still
    # starts a new sentence, which is what the backtick in §213's splitter was for.
    return CODE_SPAN.sub('Code', s)


# Lowercase after a full stop is a sentence start in this tree, because the architect's name is
# `calef` and the boards are `radon`, `argon` and `xenon`. §213's splitter asked for a capital and
# so ran "... has the experiment happened. calef ratified ..." together into one 56-word sentence.
# Any start is accepted unless the word before the stop is an abbreviation.
ABBREVIATIONS = {'e.g.', 'i.e.', 'etc.', 'vs.', 'cf.', 'al.', 'approx.', 'no.', 'fig.', 'ch.',
                 'p.', 'pp.', 'resp.', 'viz.', 'ca.'}


def blocks(lines):
    """Prose blocks, split on block boundaries BEFORE any sentence is split (§213's trap)."""
    cur = []
    for line in lines:
        if not line.strip() or HEADING.match(line) or TABLE_ROW.match(line) or QUOTE_LINE.match(line):
            if cur:
                yield ' '.join(cur)
            cur = []
            continue  # headings, tables and quotes are not sentences
        if LIST_ITEM.match(line):
            if cur:
                yield ' '.join(cur)
            cur = [LIST_ITEM.sub('', line, count=1).strip()]
            continue
        cur.append(line.strip())
    if cur:
        yield ' '.join(cur)


def unquoted(block):
    """A block as the writer's own words: links as their text, inline code as one word, and each
    `"..."` quote as a `\0` marker.

    Quotes come out BEFORE the split, because a verbatim quote may hold sentence ends of its own
    ("... It is more honest ...") and splitting inside it would measure half a quote as prose. A
    quote that closed its sentence leaves the stop behind so the lead-in still ends there.
    """
    s = LINK.sub(r'\1', _code_to_word(block))
    return QUOTED.sub(lambda m: ' \0' + ('. ' if re.search(r'[.!?]\W*$', m.group(0)) else ' '), s)


def sentences(block, s=None):
    """Each sentence's length in words. `s` is `unquoted(block)`, for a caller that has it."""
    s = unquoted(block) if s is None else s
    out, start = [], 0
    for m in SENTENCE_END.finditer(s):
        before = s[:m.start() + 1].split()
        if before and before[-1].lower().lstrip('(') in ABBREVIATIONS:
            continue
        out.append(s[start:m.end()])
        start = m.end()
    out.append(s[start:])
    lengths = []
    for sent in out:
        n = sum(1 for w in sent.split() if not w.startswith('\0'))  # the lead-in counts
        if n:
            lengths.append(n)
    return lengths


def passive_count(texts):
    """Passive constructions in the prose `sentences` measures, given each block as `unquoted`
    returns it: no heading, table row or blockquote; a link as its text; inline code as one word;
    a `"..."` quote removed, since a writer cannot rewrite a speaker into the active voice. The
    counter is `passive_voice.count`."""
    return sum(passive_voice.count(t) for t in texts)


# --- bold a script parses is syntax, not emphasis --------------------------------------------
#
# The maintainer's ruling of 2026-09-26 (UTC), on #1311, after the lane for §219 (how the shell
# names an installed program to the spawner) found it: `**Status: …`, `**Built:**`, `**Gate: …**`,
# the Follow-on and Revisit tags and fatal-risks' experiment lead-ins are read by scripts, and in a short block that bold alone is over 4 per 1,000 words, so the touch
# rule would have made those documents uneditable. Parsed bold is not counted at all.
#
# The set is DERIVED from the parsers, not listed here: every `re.compile`/`re.match`/`re.search`/
# `re.finditer`/`re.findall` whose pattern is a literal containing `\*\*`, and every
# `.startswith("**X…")`, in the Python of `script/*` (the heredoc a shell wrapper runs) and
# `helpers/*.py`. A parser that adds a marker is honoured the day it lands, with nothing to update.
# A single shared module was considered and not built: the parsers disagree about their own
# markers (five spellings of the Status line across six scripts), so sharing one definition means
# either changing what eight gates accept, which is its own decision, or re-homing each variant
# verbatim, which shares nothing. Deriving reads each variant where it lives.
#
# The failure mode of deriving is a generic pattern (one that matches any bold) exempting everything.
# `selftest` guards it: no derived marker may match ordinary bold, and a pattern that does must be
# named in NOT_MARKERS with its reason, or the lint fails.
PARSER_ROOTS = ('script', 'helpers')
NOT_MARKERS = {
    # Reads a running-order TABLE cell's leading verdict; table rows are never counted here anyway,
    # and on prose it would match any bold that opens with a capital word ("**A run ...").
    ('script/fatal-risks', r'^\*\*([A-Z][A-Z-]*)\b'),
    # Milestone 791 (bold that a script reads), 2026-10-07 UTC. Reads the `Reuse:` line, and reads it
    # plain as well, so the bold is emphasis a writer chose and is counted like any other. §46 (thin
    # primitives or whole subsystems) spells the line plain, and notes/roadmap.md's template now
    # writes it so.
    ('script/roadmap', r'(?:\*\*Reuse:\*\*|Reuse:)\s*(.*)$'),
    # Milestone 791, 2026-10-07 UTC. `script/decisions`' `^\*\*Status:` is a ban (it refuses a status
    # line in a decision), not a key, so a bold `**Status:` line elsewhere is emphasis and is counted.
    # It was exempt, marked as an exception here, until the 25 free-form lines in notes and design
    # documents lost their bold.
    ('script/decisions', r'^\*\*Status:'),
}
ORDINARY_BOLD = ('**Hello world.** Then prose.', '**A claim that opens.** More.', '**I think so.**',
                 '- **Operations.** The rest.', 'Inline **names provisional** here.',
                 '**It is fixed, 2026-09-23.** Then.', '**NOT** this.')
_markers = None


def _python_of(path, text):
    if path.endswith('.py'):
        return [text]
    return [m.group(2) for m in re.finditer(r"<<-?\s*'(\w+)'\n(.*?)\n\1\n", text, re.S)]


def _flags(node):
    import ast
    flags = 0
    for n in ast.walk(node):
        if isinstance(n, ast.Attribute) and isinstance(n.value, ast.Name) and n.value.id == 're':
            flags |= {'M': re.M, 'MULTILINE': re.M, 'I': re.I, 'IGNORECASE': re.I,
                      'S': re.S, 'DOTALL': re.S}.get(n.attr, 0)
    return flags


def derived_markers(root=None):
    """[(file, pattern, flags)] for every bold marker a script or helper parses. See above."""
    import ast
    root = root or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    out = []
    for d in PARSER_ROOTS:
        base = os.path.join(root, d)
        if not os.path.isdir(base):
            continue
        for name in sorted(os.listdir(base)):
            rel = f'{d}/{name}'
            if rel == 'helpers/prose_ratchet.py':
                continue  # the counter itself: its BOLD matches every span by design
            path = os.path.join(base, name)
            if not os.path.isfile(path):
                continue
            try:
                text = open(path, encoding='utf-8').read()
            except (UnicodeDecodeError, OSError):
                continue
            if '**' not in text:
                continue
            for src in _python_of(rel, text):
                try:
                    tree = ast.parse(src)
                except SyntaxError:
                    continue
                for node in ast.walk(tree):
                    if not isinstance(node, ast.Call) or not node.args:
                        continue
                    f, a0 = node.func, node.args[0]
                    if not (isinstance(a0, ast.Constant) and isinstance(a0.value, str)):
                        continue
                    if (isinstance(f, ast.Attribute) and isinstance(f.value, ast.Name)
                            and f.value.id == 're'
                            and f.attr in ('compile', 'match', 'search', 'finditer', 'findall')
                            and '\\*\\*' in a0.value):
                        flags = _flags(node.args[1]) if len(node.args) > 1 else 0
                        for kw in node.keywords:
                            if kw.arg == 'flags':
                                flags |= _flags(kw.value)
                        out.append((rel, a0.value, flags))
                    elif (isinstance(f, ast.Attribute) and f.attr == 'startswith'
                          and a0.value.startswith('**') and len(a0.value.strip('*')) > 0):
                        out.append((rel, '^' + re.escape(a0.value), 0))
    return out


# --- the readers derivation cannot see ----------------------------------------------------------
#
# Derivation (above) reads a literal pattern handed straight to `re.*`, or a literal `.startswith`.
# A parser written any other way is invisible to it, and an invisible parser is a live hazard: the
# bold it needs is counted as emphasis, and a sweep that removes the emphasis removes the key. Three
# shapes were found by hand on 2026-10-05 UTC (design/roadmap/0791-bold-keys-to-frontmatter.md,
# provisional): a pattern built by `+` or an f-string (`script/roadmap` RESTATED), a bold written
# with a repeat count or a character class instead of two escaped stars, and a reader in a language
# this file does not parse (shell, awk, jq, Rust, a workflow). `opaque_readers` finds the first two
# by AST and the third by text. `selftest` fails on any it cannot account for in OPAQUE_OK, keyed by
# file and the name nearest above the match. The value is the record: which derived marker already
# covers the same spans, or why the match is not a key.
OPAQUE_OK = {
    ('script/roadmap', 'RESTATED'):
        'built from VOCAB by `+`; it matches the same three shapes as roadmap_block STATUS_TOKEN, '
        'BUILT_TOKEN and GATE_TOKEN, which are derived, so the spans are already exempt',
}
# `\*\*`, `\*{2}` and `[*]{2}`: the three spellings of "two literal stars" in a regex.
_BOLD_TEXT = re.compile(r'\\\*\\\*|\\\*\{2\}|\[\*\]\{2\}')
OTHER_LANGUAGE_ROOTS = ('script', 'helpers', 'xtask', '.github')
_RE_CALLS = ('compile', 'match', 'search', 'finditer', 'findall', 'sub', 'split')


def _enclosing_name(tree, target):
    """The assignment target or function a node sits in, for a stable OPAQUE_OK key."""
    import ast
    best = None
    for node in ast.walk(tree):
        if isinstance(node, (ast.Assign, ast.FunctionDef)) and any(n is target for n in ast.walk(node)):
            if isinstance(node, ast.FunctionDef):
                best = node.name
            elif isinstance(node.targets[0], ast.Name) and best is None:
                best = node.targets[0].id
    return best


def opaque_readers(root=None):
    """[(file, name or line)] for every bold reader `derived_markers` cannot see. See above."""
    import ast
    root = root or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    parsed, out = set(), []
    for d in PARSER_ROOTS:
        base = os.path.join(root, d)
        for name in sorted(os.listdir(base)) if os.path.isdir(base) else []:
            rel, path = f'{d}/{name}', os.path.join(base, name)
            if rel == 'helpers/prose_ratchet.py' or not os.path.isfile(path):
                continue
            try:
                text = open(path, encoding='utf-8').read()
            except (UnicodeDecodeError, OSError):
                continue
            for src in _python_of(rel, text):
                try:
                    tree = ast.parse(src)
                except SyntaxError:
                    continue
                parsed.add(rel)
                for node in ast.walk(tree):
                    if not (isinstance(node, ast.Call) and node.args):
                        continue
                    f, a0 = node.func, node.args[0]
                    if not (isinstance(f, ast.Attribute) and isinstance(f.value, ast.Name)
                            and f.value.id == 're' and f.attr in _RE_CALLS):
                        continue
                    consts = [n.value for n in ast.walk(a0)
                              if isinstance(n, ast.Constant) and isinstance(n.value, str)]
                    if not any(_BOLD_TEXT.search(c) for c in consts):
                        continue
                    # A bare literal with two escaped stars is derived; anything else is not.
                    derived = (isinstance(a0, ast.Constant) and '\\*\\*' in a0.value
                               and f.attr in ('compile', 'match', 'search', 'finditer', 'findall'))
                    if not derived:
                        out.append((rel, _enclosing_name(tree, node) or f'line {node.lineno}'))
    for d in OTHER_LANGUAGE_ROOTS:
        for dirpath, _, names in os.walk(os.path.join(root, d)):
            for name in sorted(names):
                path = os.path.join(dirpath, name)
                rel = os.path.relpath(path, root)
                if rel in parsed or rel == 'helpers/prose_ratchet.py':
                    continue
                try:
                    lines = open(path, encoding='utf-8').read().split('\n')
                except (UnicodeDecodeError, OSError):
                    continue
                for n, line in enumerate(lines, 1):
                    if _BOLD_TEXT.search(line) and not line.lstrip().startswith(('#', '//')):
                        out.append((rel, f'line {n}'))
    return out


def unaccounted_readers(root=None):
    """[(file, name or line)] for opaque readers not in OPAQUE_OK. `selftest` wants none."""
    return [(rel, name) for rel, name in opaque_readers(root) if (rel, name) not in OPAQUE_OK]


def key_spans(text):
    """[(line, span, parsers)] for each bold span a script reads, with the files that read it.

    The answer to "which bold here is a key and who needs it", for someone cutting bold from a
    document. Same paragraph reading as `bold_counts`, so what is listed is exactly what is exempt.
    """
    by_pattern = {}
    for rel, pat, flags in derived_markers():
        if (rel, pat) in NOT_MARKERS:
            continue
        try:
            by_pattern.setdefault((pat, flags), []).append((re.compile(pat, flags | re.M), rel))
        except re.error:
            continue
    out, para, first = [], [], 0

    def flush():
        if not para:
            return
        body = '\n'.join(para)
        for group in by_pattern.values():
            for rx, rel in group:
                for m in rx.finditer(body):
                    j = body.find('**', m.start())
                    if 0 <= j < m.end():
                        span = BOLD.match(body, j)
                        if span:
                            line = first + body.count('\n', 0, j)
                            out.append((line, span.group(0)[:50], rel))
        para.clear()

    fenced = False
    for n, line in enumerate(text.split('\n'), 1):
        if line.lstrip().startswith('```'):
            fenced = not fenced
        if fenced or not line.strip() or TABLE_ROW.match(line) or HEADING.match(line):
            flush()
            continue
        if LIST_ITEM.match(line) or QUOTE_LINE.match(line):
            flush()
        if not para:
            first = n
        para.append(CODE_SPAN.sub('code', line))
    flush()
    merged = {}
    for line, span, rel in out:
        merged.setdefault((line, span), set()).add(rel)
    return [(l, sp, sorted(r)) for (l, sp), r in sorted(merged.items())]


def markers():
    """The derived markers, compiled for use on a paragraph (always multiline), NOT_MARKERS out."""
    global _markers
    if _markers is None:
        _markers = []
        seen = set()
        for rel, pat, flags in derived_markers():
            if (rel, pat) in NOT_MARKERS or (pat, flags) in seen:
                continue
            seen.add((pat, flags))
            try:
                _markers.append(re.compile(pat, flags | re.M))
            except re.error:
                continue
    return _markers


def bold_counts(lines):
    """(line-opening, inline) bold spans. A span may wrap onto the next line of its paragraph, which
    is how a bold lead-in sentence is usually written, so paragraphs are read whole: a line-by-line
    count missed every wrapped span, and wrapped lead-ins are the commonest bold in this tree."""
    lead = inline = 0
    para = []

    def flush():
        nonlocal lead, inline
        if not para:
            return
        text = '\n'.join(para)
        starts = []
        pos = 0
        for line in para:
            m = re.match(r'^\s*(>\s*)*([-*+]\s+|\d+[.)]\s+)?', line)
            starts.append(pos + m.end())
            pos += len(line) + 1
        parsed = set()
        for marker in markers():
            for m in marker.finditer(text):
                j = text.find('**', m.start())
                if 0 <= j < m.end():
                    parsed.add(j)
        for span in BOLD.finditer(text):
            if span.start() in parsed:
                continue  # syntax a script reads, not emphasis
            if span.start() in starts:
                lead += 1
            else:
                inline += 1
        para.clear()

    for line in lines:
        if not line.strip() or TABLE_ROW.match(line) or HEADING.match(line):
            flush()
            continue
        if LIST_ITEM.match(line) or QUOTE_LINE.match(line):
            flush()
        para.append(CODE_SPAN.sub('code', line))
    flush()
    return lead, inline


def exceptions(text):
    """{family: problem-or-None} for every exception marker in the text."""
    found = {}
    # A marker inside code is being quoted, not asserted (milestone 586's own block spells one).
    text = CODE_SPAN.sub('', re.sub(r'^\s*(```|~~~).*?^\s*\1', '', text, flags=re.S | re.M))
    for m in re.finditer(r'<!--\s*(prose-budget|writing-standards):\s*exception\.(.*?)-->', text, re.S):
        body = m.group(2)
        problem = None
        if not re.search(r'\d{4}-\d{2}-\d{2}', body):
            problem = 'names no date'
        elif 'Reason:' not in body:
            problem = 'gives no `Reason:`'
        found[m.group(1)] = problem
    return found


def granted_words(text):
    """The word count a `prose-budget` exception marker records, or None.

    Milestone 586's design note: the marker's number is the grant, and a file past it has grown
    without a grant. It is the first number before `words` in the marker, compared with the file's
    `wc -w`, marker included, because that is how every marker in the tree was measured, and
    frontmatter excluded (`counted_words`), because a record's fields are not its prose.
    """
    text = CODE_SPAN.sub('', re.sub(r'^\s*(```|~~~).*?^\s*\1', '', text, flags=re.S | re.M))
    m = re.search(r'<!--\s*prose-budget:\s*exception\.(.*?)-->', text, re.S)
    n = re.search(r'([\d,]+)\s+words\b', m.group(1)) if m else None
    return int(n.group(1).replace(',', '')) if n else None


# --- a roadmap block's field tokens are syntax (milestone 596) ---------------------------------
#
# `**Status: BUILT.**`, `**Gate: NONE.**`, a `**Built:**` line and the tag opening a Follow-on or
# Revisit bullet are fields `script/roadmap` parses, written in a sentence's clothes. Counted as
# sentences they held down the median of every block that carried one (a two-word "sentence" per
# block), so milestone 596 (the roadmap blocks get frontmatter too), which moves them into
# frontmatter, read as 118 blocks getting worse. The tokens come from `helpers/roadmap_block.py`, the
# module the parser reads them with, so nothing here lists them. Kept a separate function from the
# bold counting on purpose: #1311 changes that, and the two should not share a hunk.
#
# MEASURE numbers the measurement. The baseline records the one it was written with, and a baseline
# written with a newer one may re-measure rows upward once, in the same change as the code that
# moved (see `check`). That is a re-measurement of the same prose, not a relaxed limit.
#
# REMEASURED names the columns the current MEASURE moved: `check` lets those cells rise, or a row
# holding only them appear, in the change that bumps MEASURE and nowhere else. MEASURE 3 made
# `words` the whole-file count (`budget_words`).
MEASURE = 3
REMEASURED = ('words',)


def measured(path, text):
    """`measure`, with a roadmap document's field tokens read as syntax rather than sentences, and
    a mechanical rename (`RENAME_PAIRS`) read as the form it replaced, per calef's 2026-09-27
    ruling. Both readings apply to §213's measures only. `words` is `budget_words` of the file as
    written, the graph's count. A rename that adds words is not a touch for the bold rule (`check`),
    but its words count against the cap."""
    whole = text
    if path.startswith('design/roadmap/'):
        text = roadmap_block.without_field_tokens(promotion_masked(text))
    m = measure(rename_masked(text))
    m['words'] = budget_words(whole)
    return m


def only_fields_moved(path, base):
    """True when a roadmap document's only change since `base` is milestone 596's: its field tokens
    moved into frontmatter (`roadmap_block.without_fields`), and not a word of prose changed. Such a
    document is not touched for the bold rule, because nobody edited its prose."""
    if not path.startswith('design/roadmap/') or not os.path.exists(path):
        return False
    old = at(base, path)
    if old is None:
        return False

    def prose(text):
        return [line.rstrip() for line in text.split('\n') if line.strip()]
    return prose(roadmap_block.without_fields(old)) == prose('\n'.join(roadmap_block.body(
        open(path).read())))


def renamed_paths(base):
    """Every file git calls renamed between `base` and the working tree, as (old, new) path pairs."""
    pairs = []
    for line in (git('diff', '--name-status', '-M', base) or '').splitlines():
        cells = line.split('\t')
        if cells[0].startswith('R') and len(cells) == 3:
            pairs.append((cells[1], cells[2]))
    return pairs


def only_paths_renamed(path, base, renamed):
    """True when a document's only change since `base` is a path that git itself reports as renamed
    in the same diff, rewritten from its old spelling to its new one (milestone 609 (the system
    tests leave the kernel crate)). Moving 66 files would otherwise put forty notes under the bold
    rule for a repointed path each, which is `only_fields_moved`'s case in a different rewrite:
    nobody edited the prose."""
    if not renamed or not os.path.exists(path):
        return False
    old = at(base, path)
    if old is None:
        return False
    before, after = old.split('\n'), open(path).read().split('\n')
    if len(before) != len(after):
        return False

    def repointed(line):
        for was, now in renamed:
            line = line.replace(was, now)
        return line
    # Line by line, so a record that repoints only the one path a gate resolves still qualifies.
    return all(a == b or repointed(a) == b for a, b in zip(before, after))


def counted_words(text):
    """The word count a prose-budget grant is held to: the file's `wc -w`, frontmatter excluded.

    Not `budget_words`: a grant is the number calef wrote into a marker, which was measured this
    way, and reading it any other way would fail a document for words nobody added. The marker
    itself exempts the `words` measure.

    Frontmatter is a record's fields, which `prose_lines` already leaves out of the main-body count;
    counting it here meant a block that moved its status into frontmatter grew against its grant.
    """
    return len(FRONTMATTER.sub('', text).split())


def median(xs):
    xs = sorted(xs)
    n = len(xs)
    if not n:
        return 0
    return xs[n // 2] if n % 2 else (xs[n // 2 - 1] + xs[n // 2]) / 2


def measure(text):
    """`words` is §212's whole-file count (`budget_words`). `prose_words` is the prose alone, which
    §213 asks about: the 200-word floor and bold per 1,000 words are a density of prose."""
    lines = list(prose_lines(text))
    prose = sum(len(line.split()) for line in lines)
    prose_blocks = [(b, unquoted(b)) for b in blocks(lines)]
    lengths = [n for b, s in prose_blocks for n in sentences(b, s)]
    lead, inline = bold_counts(lines)
    return {'words': budget_words(text), 'prose_words': prose, 'median': median(lengths),
            'longest': max(lengths, default=0), 'bold_lead': lead, 'bold_inline': inline,
            'sentences': len(lengths), 'passives': passive_count(s for _b, s in prose_blocks)}


def bold_allowed(words):
    """The most bold spans a document of `words` words may carry: 4 per 1,000, rounded down."""
    return words * LIMITS['bold_per_1000'] // 1000


def bold_excess(m):
    """Bold spans over the density limit, 0 for a document under the 200-word floor: what a document
    the gate refuses has to remove. Both read prose words, not §212's whole-file count."""
    if m['prose_words'] < SMALL_DOCUMENT:
        return 0
    return max(0, m['bold_lead'] + m['bold_inline'] - bold_allowed(m['prose_words']))


def over(m):
    """The measures of `m` that are over their limit, as the baseline records them."""
    out = {}
    if m['words'] > LIMITS['words']:
        out['words'] = m['words']
    big = m['prose_words'] >= SMALL_DOCUMENT
    if big and m['median'] > LIMITS['median']:
        out['median'] = m['median']
    if m['longest'] > LIMITS['longest']:
        out['longest'] = m['longest']
    if big and (m['bold_lead'] + m['bold_inline']) * 1000 > LIMITS['bold_per_1000'] * m['prose_words']:
        out['bold_lead'] = m['bold_lead']
        out['bold_inline'] = m['bold_inline']
    return out


# --- the baseline ------------------------------------------------------------------------------

HEADER = """\
# The prose ratchet's baseline: milestone 586 (a prose ratchet in lint), for §212 (a prose budget)
# and §213 (writing standards).
# One row per document over at least one limit when it was written. `-` means that measure was
# within its limit, and so may not cross it. A number is a ceiling that may only go DOWN: lower it
# with `python3 helpers/prose_ratchet.py --bank`, which never raises and never adds. A row may be
# removed; a row may not be added and a number may not rise (script/lint compares against the
# merge base). A document that must exceed its row gets a marked exception, not an edit here.
# The one exception is a change of measurement: the `measure:` line below says which one wrote
# these rows, and a change that bumps it re-measures the rows it moved (`--remeasure`), once.
# Limits: words 3000, median sentence 20, longest sentence 40, bold 4 per 1,000 words (held as the
# two counts). Median and bold are not asked of documents under 200 words.
"""


def fmt(v):
    if v is None:
        return '-'
    return str(int(v)) if float(v).is_integer() else f'{v:.1f}'


def read_baseline(text):
    rows = {}
    for line in text.splitlines():
        if not line.strip() or line.startswith('#'):
            continue
        cells = line.split('\t')
        if cells[0] == 'path':
            continue
        rows[cells[0]] = {c: (None if v == '-' else float(v)) for c, v in zip(COLUMNS, cells[1:])}
    return rows


def baseline_measure(text):
    """The MEASURE a baseline was written with; 1 for a baseline older than the line."""
    m = re.search(r'^# measure: (\d+)$', text or '', re.M)
    return int(m.group(1)) if m else 1


def write_baseline(rows):
    out = [HEADER, f'# measure: {MEASURE}\n', 'path\t' + '\t'.join(COLUMNS) + '\n']
    for path in sorted(rows):
        out.append(path + '\t' + '\t'.join(fmt(rows[path].get(c)) for c in COLUMNS) + '\n')
    with open(BASELINE, 'w') as f:
        f.write(''.join(out))



# --- the passive-voice baseline (milestone 952) -----------------------------------------------
#
# Its own file, so the prose baseline's MEASURE and REMEASURED stay about the prose baseline's own
# columns: adding this measure moved none of them, and needs no bump there. PASSIVE_MEASURE numbers
# this file's measurement the same way. A change to `passive_voice` that raises counts (an entry
# leaving NOT_PASSIVE, a participle joining IRREGULAR) bumps it, runs `--remeasure-passive`, and
# `check` lets rows rise or appear in that change alone. A change that only lowers counts needs no
# bump: the rows are ceilings, and `--bank` lowers them.

PASSIVE_BASELINE = 'design/passive-baseline.tsv'
PASSIVE_PER_1000 = 2
PASSIVE_MEASURE = 1
PASSIVE_FAMILY = 'writing-standards'

PASSIVE_HEADER = """\
# The passive-voice baseline: milestone 952 (a passive-voice ratchet in lint), for rule 6 of §213
# (writing standards): at most 2 passive constructions per 1,000 prose words.
# One row per document over that limit when `python3 helpers/prose_ratchet.py --init-passive`
# wrote this file. A count is a ceiling that only goes DOWN: `--bank` lowers it and drops a row
# whose document has reached the limit. A row may be removed; a row may not be added and a count
# may not rise (script/lint compares against the merge base). A document under 200 prose words is
# not held to the density. A document absent from this file meets the limit outright, or carries
# a marked writing-standards exception. helpers/passive_voice.py is the counter.
"""


def passive_over(m):
    """True when a measured document is over 2 passives per 1,000 prose words, and big enough to
    be asked (the 200-word floor the bold density uses)."""
    return (m['prose_words'] >= SMALL_DOCUMENT
            and m['passives'] * 1000 > PASSIVE_PER_1000 * m['prose_words'])


def passive_allowed(words):
    """The most passives a document of `words` prose words may carry: 2 per 1,000, rounded down."""
    return words * PASSIVE_PER_1000 // 1000


def read_passive_baseline(text):
    rows = {}
    for line in (text or '').splitlines():
        if not line.strip() or line.startswith('#'):
            continue
        cells = line.split('\t')
        if cells[0] != 'path' and len(cells) >= 2:
            rows[cells[0]] = int(cells[1])
    return rows


def write_passive_baseline(rows):
    out = [PASSIVE_HEADER, f'# measure: {PASSIVE_MEASURE}\n', 'path\tpassives\n']
    out += [f'{path}\t{rows[path]}\n' for path in sorted(rows)]
    with open(PASSIVE_BASELINE, 'w') as f:
        f.write(''.join(out))


def passive_verdict(path, m, row, was):
    """The refusal for one document's passive count, or None when it passes.

    `m` is its measure now, `row` its baseline count (None without a row), and `was` its count at
    the merge base (None for a document the branch did not change). The caller has already let a
    `writing-standards` exception through. Pure, so `selftest` drives it with fixtures.
    """
    if not passive_over(m):
        return None
    n, words = m['passives'], m['prose_words']
    have = (f'{path}: {n} passive constructions in {words:,} words of prose, over 2 per 1,000 '
            f'(§213 rule 6), which here allows {passive_allowed(words)}')
    if row is None:
        return (f'{have}. A document not in {PASSIVE_BASELINE} meets the limit outright: put '
                f'{n - passive_allowed(words)} into the active voice')
    if n > row:
        return (f'{have}, and its baseline holds it to {row}. A document already over may not '
                f'get worse: put {n - row} into the active voice')
    if was is not None and n > was:
        return (f'{have}. It went from {was} to {n} on this branch, and the baseline ({row}) has '
                f'room only because a shrink was never banked')
    return None


def passive_baseline_problems(base, renames):
    """The passive baseline may only shrink: no row added and no count raised against the merge
    base's copy, outside a change that bumps PASSIVE_MEASURE."""
    if not base or not os.path.exists(PASSIVE_BASELINE):
        return []
    old_text = at(base, PASSIVE_BASELINE)
    if old_text is None:
        return []  # the change that writes the file once (`--init-passive`)
    now_text = open(PASSIVE_BASELINE).read()
    if baseline_measure(old_text) < PASSIVE_MEASURE == baseline_measure(now_text):
        return []  # a re-measurement, once, with the counter change that moved it
    old, bad = read_passive_baseline(old_text), []
    for path, count in read_passive_baseline(now_text).items():
        prior = old.get(path, old.get(renames.get(path, '')))
        if prior is None:
            bad.append(f'{PASSIVE_BASELINE}: {path} was added. The baseline only shrinks; a new '
                       f'document meets 2 passives per 1,000 words outright')
        elif count > prior:
            bad.append(f'{PASSIVE_BASELINE}: {path} raised its passives from {prior} to {count}. '
                       f'The baseline only goes down')
    return bad

# --- git ---------------------------------------------------------------------------------------

def git(*args):
    r = subprocess.run(('git',) + args, capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


def tracked():
    # Untracked files too (not ignored ones): a lane's new note is judged before its first commit.
    files = git('ls-files', '-z', '--cached', '--others', '--exclude-standard') or ''
    return {f for f in files.split('\0') if f and os.path.exists(f)}


BASE_OVERRIDE = None  # `--base REV`, for falsifying the merge-base half against a chosen commit


def merge_base():
    if BASE_OVERRIDE:
        return (git('rev-parse', BASE_OVERRIDE) or '').strip() or None
    base = (git('merge-base', 'HEAD', 'origin/main') or '').strip()
    head = (git('rev-parse', 'HEAD') or '').strip()
    return base if base and base != head else None


def at(rev, path):
    return git('show', f'{rev}:{path}')


# --- the checks --------------------------------------------------------------------------------

LINK_TARGET = re.compile(r'\]\(([^)#\s]+)')


def links_from(path):
    try:
        text = open(path).read()
    except OSError:
        return set()
    here = os.path.dirname(path)
    out = set()
    for t in LINK_TARGET.findall(text):
        if '://' in t:
            continue
        target = os.path.normpath(os.path.join(here, t))
        out.add(target)
        out.add(target + '/README.md')  # a link to a directory is a link to its README
    return out


def orphans(paths):
    bad = []
    all_paths = set(paths)
    for p in documents(all_paths):
        parent = appendix_parent(p, all_paths)
        if not parent:
            continue
        thematic = next((d for d in THEMATIC_APPENDIX_DIRS if p.startswith(d)), None)
        if thematic:
            readme = thematic + 'README.md'
            if p == readme:
                sources = {parent}
            else:
                sources = {readme}
        else:
            if p == parent[:-3] + '/README.md':
                # The directory's own README is its provenance page (every documentation directory
                # carries its Name: block there since d449f8442), reached by opening the directory.
                # It is not an appendix and is not held to linking.
                continue
            sources = {parent, parent[:-3] + '/README.md'}
        if not any(p in links_from(s) for s in sources):
            bad.append(f'{p}: an appendix nothing links to. Link it from {" or ".join(sorted(sources))} '
                       f'(§212: an unlinked appendix is a lost document)')
    return bad


NAMES = {'words': 'words (whole file)', 'median': 'median sentence', 'longest': 'longest sentence',
         'bold_lead': 'line-opening bold spans', 'bold_inline': 'inline bold spans'}
LIMIT_TEXT = {'words': 'the 3,000-word cap (§212)', 'median': 'the 20-word median (§213 rule 1)',
              'longest': 'the 40-word limit (§213 rule 2)',
              'bold_lead': '4 bold per 1,000 words (§213 rule 3)',
              'bold_inline': '4 bold per 1,000 words (§213 rule 3)'}


def check():
    paths = tracked()
    docs = documents(paths)
    base = merge_base()
    baseline = read_baseline(open(BASELINE).read()) if os.path.exists(BASELINE) else {}
    bad = []

    # The baseline may only go down. Rows are compared with the merge base's copy of the file.
    if base:
        old_text = at(base, BASELINE)
        # A change that moves MEASURE re-measures the columns it moved (REMEASURED) once, with the
        # code that moved them: those cells may rise, and a row holding only them may appear, in
        # that change and in no other.
        remeasured = (old_text is not None and baseline_measure(old_text) < MEASURE
                      and baseline_measure(open(BASELINE).read()) == MEASURE)
        if old_text is not None:
            old = read_baseline(old_text)
            renames = {}
            for line in (git('diff', '--name-status', '-M', base, '--', '*.md') or '').splitlines():
                cells = line.split('\t')
                if cells[0].startswith('R') and len(cells) == 3:
                    renames[cells[2]] = cells[1]
            for path, row in baseline.items():
                prior = old.get(path) or old.get(renames.get(path, ''))
                held_cells = {c for c in COLUMNS if row.get(c) is not None}
                if prior is None and remeasured and held_cells <= set(REMEASURED):
                    continue
                if prior is None:
                    bad.append(f'{BASELINE}: {path} was added. The baseline only shrinks; a new '
                               f'document meets the limits outright, or carries a marked exception')
                    continue
                for c in COLUMNS:
                    if remeasured and c in REMEASURED:
                        continue
                    was, now = prior.get(c), row.get(c)
                    if now is not None and (was is None or now > was):
                        bad.append(f'{BASELINE}: {path} raised {NAMES[c]} from {fmt(was)} to '
                                   f'{fmt(now)}. The baseline only goes down; an exception belongs '
                                   f'in the document, marked, with its reason')
    # The bold backlog reached zero on 2026-10-06 (UTC), and calef asked that it stop being
    # tracked (lane/bold-backlog-final). Its weekly chart went from script/metrics; this is what
    # replaced it. No baseline row may hold a bold ceiling, so a document over 4 bold per 1,000
    # words fails outright unless it carries a marked writing-standards exception. A raise is
    # already refused above; this also refuses one written by `--remeasure`.
    for path, row in baseline.items():
        for c in ('bold_lead', 'bold_inline'):
            if row.get(c) is not None:
                bad.append(f'{BASELINE}: {path} holds a ceiling for {NAMES[c]} ({fmt(row[c])}). '
                           f'The bold backlog is zero and that column stays empty: the document '
                           f'meets 4 bold per 1,000 words, or carries a marked exception')
    for path in baseline:
        if path not in paths:
            bad.append(f'{BASELINE}: {path} is not in the tree. Remove its row (or move it, if the '
                       f'document was renamed)')

    # The passive-voice baseline, by the same two rules: it only shrinks against the merge base,
    # a renamed document's row is compared with its old path's, and every row names a document.
    passive_rows = (read_passive_baseline(open(PASSIVE_BASELINE).read())
                    if os.path.exists(PASSIVE_BASELINE) else {})
    if base:
        renames = {}
        for line in (git('diff', '--name-status', '-M', base, '--', '*.md') or '').splitlines():
            cells = line.split('\t')
            if cells[0].startswith('R') and len(cells) == 3:
                renames[cells[2]] = cells[1]
        bad += passive_baseline_problems(base, renames)
    for path in passive_rows:
        if path not in paths:
            bad.append(f'{PASSIVE_BASELINE}: {path} is not in the tree. Remove its row (or move it, '
                       f'if the document was renamed)')

    changed = set()
    touched = set()  # content differs from the merge base; a 100%-similar rename is not a touch
    if base:
        changed = set((git('diff', '--name-only', base, '--', '*.md') or '').split())
        for line in (git('diff', '--name-status', '-M', base, '--', '*.md') or '').splitlines():
            cells = line.split('\t')
            if cells[0] != 'R100' and cells[0] != 'D':
                path = cells[-1]
                old_text = at(base, cells[1] if cells[0].startswith('R') else path)
                new_text = open(path).read() if os.path.exists(path) else None
                if old_text is not None and new_text is not None and \
                        count_masked(old_text) == count_masked(new_text):
                    continue  # only a counted claim's number moved: `script/lint` made it do so
                if old_text is not None and new_text is not None and \
                        rename_masked(_flat(old_text)) == rename_masked(_flat(new_text)):
                    continue  # only a mechanical rename (and the rewrap it forced) moved: calef's
                              # 2026-09-27 ruling
                touched.add(path)
        touched = {p for p in touched if not only_fields_moved(p, base)}
        renamed = renamed_paths(base)
        touched = {p for p in touched if not only_paths_renamed(p, base, renamed)}

    excused = 0
    held = 0
    passive_held = 0
    for path in docs:
        text = open(path).read()
        exc = exceptions(text)
        for family, problem in exc.items():
            if problem:
                bad.append(f'{path}: its {family} exception {problem}. An exception has to say '
                           f'when it was granted and why, or it reads as a design')
        grant = granted_words(text)
        counted = counted_words(rename_masked(text))
        if grant is not None and counted > grant:
            bad.append(f'{path}: {counted:,} words (wc -w, frontmatter excluded, a mechanical '
                       f'rename read as the form it replaced) against the {grant:,} its '
                       f'prose-budget exception grants. Cut it back; raising the grant is calef\'s')
        m = measured(path, text)
        if passive_over(m):
            if PASSIVE_FAMILY in exc:
                excused += 1
            else:
                prior = None
                if path in changed:
                    old_text = at(base, path)
                    prior = measured(path, old_text)['passives'] if old_text is not None else None
                problem = passive_verdict(path, m, passive_rows.get(path), prior)
                if problem:
                    bad.append(problem)
                else:
                    passive_held += 1
        now_over = over(m)
        if not now_over:
            continue
        row = baseline.get(path, {})
        # The merge base's measures, for a document this branch changed: the tight half of the
        # ratchet, which closes the slack an unbanked shrink leaves in the baseline.
        was = None
        if path in changed:
            old_text = at(base, path)
            was = measured(path, old_text) if old_text is not None else None
        doc_held = False
        if path in touched and 'bold_lead' in now_over and FAMILY['bold_lead'] not in exc:
            spans = m['bold_lead'] + m['bold_inline']
            bad.append(f'{path}: {spans} bold spans in {m["prose_words"]:,} words of prose, and this '
                       f'change touches it. A touched document meets 4 bold per 1,000 words (calef, '
                       f'2026-09-26), which here is {bold_allowed(m["prose_words"])}: remove '
                       f'{bold_excess(m)} (promote a '
                       f'label to a heading, or drop the bold)')
            now_over = {c: v for c, v in now_over.items() if c not in ('bold_lead', 'bold_inline')}
        for c, v in now_over.items():
            if FAMILY[c] in exc:
                excused += 1
                continue
            ceiling = row.get(c)
            if ceiling is None:
                if path not in baseline:
                    why = 'a document not in the baseline meets the limits outright'
                else:
                    why = ('its baseline row has no ceiling for this measure (it was within the '
                           'limit, or excused by an exception, when the row was written), so it '
                           'may not cross')
                bad.append(f'{path}: {NAMES[c]} is {fmt(v)}, over {LIMIT_TEXT[c]}; {why}')
                continue
            if v > ceiling:
                bad.append(f'{path}: {NAMES[c]} is {fmt(v)}, over its baseline of {fmt(ceiling)} and '
                           f'{LIMIT_TEXT[c]}. A document already over may not get worse')
                continue
            if was is not None and v > was[c]:
                bad.append(f'{path}: {NAMES[c]} went from {fmt(was[c])} to {fmt(v)} on this branch, '
                           f'over {LIMIT_TEXT[c]}. The baseline ({fmt(ceiling)}) has room only '
                           f'because a shrink was never banked, and a document over a limit may not '
                           f'get worse')
                continue
            doc_held = True
        held += doc_held

    bad += orphans(paths)
    return docs, held, passive_held, excused, bad


def remeasure():
    """Raise or add the baseline cells a new MEASURE moved, and touch no other cell.

    Only the columns in REMEASURED are written, and only where a document now measures over its
    limit there, at what it now measures; no marked exception's measure is written. Every other cell
    is copied as it stands, so this cannot lower or loosen anything the measurement did not reach.
    Run once, in the change that bumps MEASURE; `check` refuses the raise anywhere else. MEASURE 3
    moved `words` for nearly every document, so every document is visited.
    """
    docs = documents(tracked())
    rows = read_baseline(open(BASELINE).read()) if os.path.exists(BASELINE) else {}
    moved = []
    for path in docs:
        text = open(path).read()
        new = measured(path, text)
        exc = exceptions(text)
        row = dict(rows.get(path, {}))
        changed = False
        for c, v in over(new).items():
            if FAMILY[c] in exc or c not in REMEASURED:
                continue
            if row.get(c) is None or v > row[c]:
                row[c] = v
                changed = True
        if changed:
            rows[path] = row
            moved.append(path)
    write_baseline(rows)
    return moved


def bank_passive(init=False):
    """Write the passive baseline. `init` writes every document over the limit, once; otherwise
    each row falls to the document's count, a row at or under the limit goes, and none is added."""
    old = {} if init else read_passive_baseline(
        open(PASSIVE_BASELINE).read() if os.path.exists(PASSIVE_BASELINE) else '')
    rows = {}
    for path in documents(tracked()):
        text = open(path).read()
        m = measured(path, text)
        if not passive_over(m) or PASSIVE_FAMILY in exceptions(text):
            continue
        if init:
            rows[path] = m['passives']
        elif path in old:
            rows[path] = min(m['passives'], old[path])
    write_passive_baseline(rows)
    return rows


def remeasure_passive():
    """Raise or add passive rows to what a new PASSIVE_MEASURE counts, and lower none. Run once,
    in the change that bumps PASSIVE_MEASURE; `check` refuses the raise anywhere else."""
    rows = read_passive_baseline(
        open(PASSIVE_BASELINE).read() if os.path.exists(PASSIVE_BASELINE) else '')
    moved = []
    for path in documents(tracked()):
        text = open(path).read()
        m = measured(path, text)
        if passive_over(m) and PASSIVE_FAMILY not in exceptions(text) \
                and m['passives'] > rows.get(path, -1):
            rows[path] = m['passives']
            moved.append(path)
    write_passive_baseline(rows)
    return moved


def bank(init=False):
    docs = documents(tracked())
    old = {} if init or not os.path.exists(BASELINE) else read_baseline(open(BASELINE).read())
    rows = {}
    for path in docs:
        text = open(path).read()
        exc = exceptions(text)
        o = {c: v for c, v in over(measured(path, text)).items() if FAMILY[c] not in exc}
        if not o:
            continue
        if init:
            rows[path] = o
            continue
        if path not in old:
            continue  # never adds
        prior = old[path]
        kept = {}
        for c, v in o.items():
            if prior.get(c) is not None:
                kept[c] = min(v, prior[c])
        if kept:
            rows[path] = kept
    write_baseline(rows)
    return rows


def selftest():
    """The reader's traps, each as a document that a wrong reader would measure wrongly."""
    long = ' '.join(['word'] * 30)
    cases = [
        # §213's recorded trap: a heading and an unpunctuated bullet must not join the sentence
        # before them. A naive splitter reads this as one 64-word sentence.
        ('block boundaries', f'{long} ends here\n## A heading of five\n- {long} bullet\n- next',
         lambda m: m['longest'] <= 33),
        # A verbatim quote is exempt and may hold its own sentence ends; the lead-in still counts.
        ('quotes exempt', f'He said: *"{long}. {long} {long}."* And that was all.',
         lambda m: m['longest'] <= 4),
        ('inline code is one word', f'Run `{long}` now.', lambda m: m['longest'] == 3),
        ('lowercase starts split', f'{long} one. calef ruled {long}.', lambda m: m['longest'] == 32),
        ('abbreviations do not split', 'Use a tool, e.g. the linter, here.', lambda m: m['sentences'] == 1),
        ('code fences are not prose', f'Short.\n```\n{long} {long}\n```\n', lambda m: m['prose_words'] == 1),
        ('comments are not prose', f'Short.\n<!-- {long} -->\n', lambda m: m['prose_words'] == 1),
        # §212's count is the whole file (MEASURE 3, calef 2026-10-13): code, comments and all.
        ('the word budget counts code and comments',
         f'Short.\n```\n{long}\n```\n<!-- {long} -->\n', lambda m: m['words'] == 65),
        ('a generated table is not prose',
         '## The decisions\n\n| # | Status | Decision |\n|---|---|---|\n| 1 | DECIDED | [A](a.md) |\n',
         lambda m: m['words'] == 3),
        # Decision files open with YAML frontmatter since #1195; their status is a field, not prose.
        ('frontmatter is not prose', '---\nstatus: DECIDED\nraised: 2026-09-23\n---\n\n# T\n\nShort.\n',
         lambda m: m['prose_words'] == 3 and m['words'] == 9),
        ('line-opening and inline bold', '**Lead.** text **inline** more\n- **Item** x\n> **Q** y',
         lambda m: (m['bold_lead'], m['bold_inline']) == (3, 1)),
        ('a bold span may wrap', '**A lead-in that\nwraps.** Then prose.\n\nMore **wrapped\ninline** text.',
         lambda m: (m['bold_lead'], m['bold_inline']) == (1, 1)),
        ('tables hold no sentences or bold', f'| **{long}** | {long} |\n', lambda m: (
            m['longest'], m['bold_lead'] + m['bold_inline']) == (0, 0)),
        # 250 words allow one span (4 per 1,000 rounds down), so three are two over.
        ('bold excess rounds the allowance down',
         ' '.join(['w'] * 247) + ' **a** **b** **c**', lambda m: bold_excess(m) == 2),
        ('no bold excess under the 200-word floor', '**a** **b** **c** short', lambda m: bold_excess(m) == 0),
        ('parsed bold is not counted',
         '**Status: BUILT.** Done.\n\n**Built:** 2026-09-01\n\n- **Recorded.** x\n\n**Real.** y',
         lambda m: (m['bold_lead'], m['bold_inline']) == (1, 0)),
    ]
    failed = [name for name, text, ok in cases if not ok(measure(text))]
    # The gate's `words` is `budget_words`, the count of the file as written, whatever `measured` reads
    # through for §213: a roadmap block's field tokens and a mechanical rename both still count.
    block = "---\nstatus: BUILT\n---\n\n**Status: BUILT.** an architect's call.\n"
    if measured('design/roadmap/0001-x.md', block)['words'] != budget_words(block):
        failed.append('the gate\'s word count differs from budget_words')
    # A counted claim's number moving is not a touch; a word moving on the same line is.
    # The marker is assembled, so `script/lint`'s counted-claims reader does not take it for a claim.
    a = 'We carry **188 widgets** <!-' + '-count:widgets--> today.\nOther **bold** here.'
    if count_masked(a) != count_masked(a.replace('188', '189')):
        failed.append('a counted claim\'s number moving reads as a touch')
    if count_masked(a) == count_masked(a.replace('today', 'now')):
        failed.append('a word moving on a counted-claim line reads as no touch')
    # A mechanical rename from RENAME_PAIRS is not a touch; the same line plus a new sentence is.
    old_line = "The gate is calef's, not a lane's, and the fork is still open."
    renamed_only = "The gate is an architect's, not a lane's, and the fork is still open."
    renamed_plus_sentence = renamed_only + " A second sentence nobody wrote before."
    if rename_masked(old_line) != rename_masked(renamed_only):
        failed.append('a mechanical rename reads as a touch')
    if rename_masked(old_line) == rename_masked(renamed_plus_sentence):
        failed.append('a mechanical rename plus a new sentence reads as no touch')
    if measure(rename_masked(old_line))['words'] != measure(rename_masked(renamed_only))['words']:
        failed.append('a mechanical rename changes the measured word count')
    # A promotion (a path gains its number, `**Proposed.**` becomes `**Milestone N.**` with a gloss)
    # is not a touch; the same bullet plus a new sentence is.
    proposed = "- **Proposed.** Do x. `design/roadmap/proposals/some-slug.md` has it."
    ms = 'Mile' + 'stone 9999'  # assembled, so the citation gates do not read it as a citation
    promoted = f"- **{ms}.** {ms} (some slug). Do x. `design/roadmap/9999-some-slug.md` has it."
    if rename_masked(_flat(proposed)) != rename_masked(_flat(promoted)):
        failed.append('a promotion reads as a touch')
    if rename_masked(_flat(proposed)) == rename_masked(_flat(promoted + " A new sentence.")):
        failed.append('a promotion plus a new sentence reads as no touch')
    # The rewrap a rename forces (an extra word pushes a line past the column limit) is not a
    # second touch: `check()` compares the flattened, rename-masked text, so a word moving to the
    # next line reads the same as the word staying put.
    rewrapped = "The gate is an architect's, not a lane's,\nand the fork is still open."
    if rename_masked(_flat(old_line)) != rename_masked(_flat(rewrapped)):
        failed.append('a mechanical rename\'s forced rewrap reads as a touch')
    rewrapped_plus_sentence = rewrapped + "\nA second sentence nobody wrote before."
    if rename_masked(_flat(old_line)) == rename_masked(_flat(rewrapped_plus_sentence)):
        failed.append('a rewrap plus a new sentence reads as no touch')
    # The rewrap can split a seeded phrase itself across the line break (found on 117's own
    # sweep: "and those are an\n  architect's."), so flattening has to run BEFORE the phrase is
    # matched, not after; the reverse order leaves the split phrase unmatched.
    split_mid_phrase = "The gate is an\narchitect's, not a lane's, and the fork is still open."
    if rename_masked(_flat(old_line)) != rename_masked(_flat(split_mid_phrase)):
        failed.append('a rewrap that splits a seeded phrase reads as a touch')
    # The derived-marker guard: a parser pattern that matches ordinary bold would exempt all of it.
    found = derived_markers()
    if not found:
        failed.append('no parser markers derived at all, so the reader of script/ is broken')
    for rel, pat, flags in found:
        if (rel, pat) in NOT_MARKERS:
            continue
        try:
            rx = re.compile(pat, flags | re.M)
        except re.error:
            continue
        for sample in ORDINARY_BOLD:
            if rx.search(sample):
                failed.append(f'{rel}\'s pattern {pat!r} matches ordinary bold ({sample!r}), so the '
                              f'ratchet would stop counting it. Tighten it, or name it in NOT_MARKERS '
                              f'with the reason')
                break
    # Both registries name entries by what derivation finds, so an entry whose pattern was edited or
    # removed would sit there quietly naming nothing. Fail instead, so the record stays true.
    found_keys = {(rel, pat) for rel, pat, _ in found}
    for key in NOT_MARKERS:
        if key not in found_keys:
            failed.append(f'helpers/prose_ratchet.py: NOT_MARKERS names {key!r}, which no parser '
                          f'carries any more; remove the entry or update its pattern')
    live_opaque = set(opaque_readers())
    for key in OPAQUE_OK:
        if key not in live_opaque:
            failed.append(f'helpers/prose_ratchet.py: OPAQUE_OK names {key!r}, which is no longer an '
                          f'opaque reader; remove the entry')
    ks = key_spans('**Status: BUILT.** Done.\n\n- **Recorded.** x\n\n**Real emphasis.** y\n')
    if [k[0] for k in ks] != [1, 3] or any(not k[2] for k in ks):
        failed.append('key_spans (which bold a script reads, and who)')
    for rel, name in unaccounted_readers():
        failed.append(f'{rel} ({name}) reads bold in a way the marker derivation cannot see. '
                      f'Pass a literal pattern with two escaped stars straight to re.*, or name it in '
                      f'OPAQUE_OK with the reason its spans are already exempt')
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        os.makedirs(os.path.join(tmp, 'helpers'))
        with open(os.path.join(tmp, 'helpers', 'x.py'), 'w') as fh:
            fh.write("import re\nA = re.compile(r'\\*\\*Key: ' + 'v')\nB = re.compile(r'^[*]{2}Key')\n"
                     "C = re.compile(r'\\*\\*Fine')\n")
        if len(opaque_readers(tmp)) != 2:
            failed.append('helpers/x.py: the opaque-reader guard missed a concatenated or `[*]{2}` pattern')
    grant = '<!-- prose-budget: exception. 1,234 words against a 3,000-word cap. 2026-09-24. Reason: x -->'
    if granted_words(grant) != 1234 or granted_words('`' + grant + '`') is not None:
        failed.append('the marker\'s granted word count')
    exc_ok = exceptions('<!-- prose-budget: exception. 2026-09-24. Reason: x -->')
    exc_bad = exceptions('<!-- writing-standards: exception. because -->')
    if exceptions('Spell it `<!-- prose-budget: exception. x -->` in prose.'):
        failed.append('a quoted marker is not an exception')
    if exc_ok != {'prose-budget': None} or exc_bad.get('writing-standards') is None:
        failed.append('exception markers')
    # The passive count (milestone 952): the counter's own cases, then the prose it reads.
    failed += [f'helpers/passive_voice.py: {f}' for f in passive_voice.selftest()]
    passive_cases = [
        ('a passive in prose counts', 'The row is checked by lint.', 1),
        ('a passive in inline code is not prose', 'Run `it is checked` now.', 0),
        ('a passive in a fence is not prose', 'Short.\n```\nit is checked\n```\n', 0),
        ('a passive in a comment is not prose', 'Short.\n<!-- it is checked -->\n', 0),
        ('a passive in a heading is not prose', '## It is checked\n\nShort.', 0),
        ('a passive in a table row is not prose', '| it is checked | x |\n', 0),
        ('a passive in a blockquote is a speaker\'s', '> It was decided.\n\nShort.', 0),
        ('a passive in a quote is a speaker\'s', 'He said "it was decided" and left.', 0),
        ('a link counts as its text', 'See [what is built](x.md) here.', 1),
        ('the ladder\'s rung is a noun', 'That is rung two.', 0),
    ]
    for name, text, want in passive_cases:
        if measure(text)['passives'] != want:
            failed.append(f'{name} ({measure(text)["passives"]} passives, want {want})')
    # A roadmap block's field tokens are syntax, so `**Status: BUILT.**` is no passive.
    if measured('design/roadmap/0001-x.md', '**Status: BUILT.** Done.\n')['passives'] != 0:
        failed.append('a roadmap field token is counted as a passive')
    # The verdict, on a 500-word document, which 2 per 1,000 allows one passive.
    def doc(n, words=500):
        return {'passives': n, 'prose_words': words}
    verdicts = [
        ('at the limit passes', doc(1), None, None, False),
        ('over the limit with no row fails', doc(2), None, None, True),
        ('under the 200-word floor passes', doc(3, 150), None, None, False),
        ('held by its row passes', doc(5), 5, None, False),
        ('over its row fails', doc(6), 5, None, True),
        ('grown since the merge base fails', doc(5), 9, 4, True),
        ('shrunk since the merge base passes', doc(4), 9, 5, False),
    ]
    for name, m, row, was, fails in verdicts:
        if (passive_verdict('x.md', m, row, was) is not None) != fails:
            failed.append(f'the passive verdict: {name}')
    for name in failed:
        if name.startswith(('script/', 'helpers/', 'no parser')):
            print(f'prose ratchet selftest: {name}', file=sys.stderr)
        else:
            print(f'prose ratchet selftest: {name} is measured wrongly', file=sys.stderr)
    return 1 if failed else 0


def main(argv):
    root = git('rev-parse', '--show-toplevel')
    if root:
        os.chdir(root.strip())
    global BASE_OVERRIDE
    if len(argv) >= 3 and argv[0] == '--check' and argv[1] == '--base':
        BASE_OVERRIDE = argv[2]
        argv = argv[:1]
    if argv and argv[0] == '--selftest':
        return selftest()
    if not argv or argv[0] == '--check':
        docs, held, passive_held, excused, bad = check()
        if bad:
            print('prose ratchet: the tree breaks §212 or §213 against its baseline:', file=sys.stderr)
            for b in bad:
                print(f'  {b}', file=sys.stderr)
            print('\nFix the document (cut restatement, split a sentence, drop or promote a bold '
                  'lead-in), or mark an exception in it with its date and reason. See milestone '
                  '586\'s block, design/roadmap/0586-a-prose-ratchet-in-lint.md.', file=sys.stderr)
            return 1
        print(f'prose ratchet: {len(docs)} documents; {held} over a limit and held to the baseline, '
              f'{passive_held} over the passive limit and held to theirs, {excused} measures '
              f'excused by a marked exception, every appendix linked')
        return 0
    if argv[0] == '--report':
        for path in argv[1:]:
            m = measured(path, open(path).read())
            print(path, ' '.join(f'{k}={fmt(v)}' for k, v in m.items()),
                  'over:', ','.join(over(m)) or 'none')
        return 0
    if argv[0] == '--keys':
        for path in argv[1:]:
            for line, span, readers in key_spans(open(path).read()):
                print(f'{path}:{line}: {span}  <- {", ".join(readers)}')
        return 0
    if argv[0] == '--remeasure':
        moved = remeasure()
        print(f'prose ratchet: re-measured {len(moved)} rows under measure {MEASURE}')
        return 0
    if argv[0] in ('--bank', '--init'):
        rows = bank(init=argv[0] == '--init')
        print(f'prose ratchet: {len(rows)} rows written to {BASELINE}')
        if argv[0] == '--bank' and os.path.exists(PASSIVE_BASELINE):
            rows = bank_passive()
            print(f'prose ratchet: {len(rows)} rows written to {PASSIVE_BASELINE}')
        return 0
    if argv[0] == '--init-passive':
        # Once, by milestone 952. A second run would reset every ceiling to today's count, which
        # is the raise the baseline exists to refuse.
        if os.path.exists(PASSIVE_BASELINE):
            print(f'prose ratchet: {PASSIVE_BASELINE} exists, and it is written once; '
                  f'--bank lowers it', file=sys.stderr)
            return 1
        rows = bank_passive(init=True)
        print(f'prose ratchet: {len(rows)} rows written to {PASSIVE_BASELINE}')
        return 0
    if argv[0] == '--remeasure-passive':
        moved = remeasure_passive()
        print(f'prose ratchet: re-measured {len(moved)} passive rows under measure {PASSIVE_MEASURE}')
        return 0
    print(__doc__.split('\n\n')[1], file=sys.stderr)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
