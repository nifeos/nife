---
status: BUILT
raised: 2026-09-24
built: 2026-10-15
promoted_from: a-quoted-phrase-after-a-path-must-be-in-that-file
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 644. A phrase quoted after a path must still be in that file

*Title and slug are drafts.*
Raised by the 2026-09-24 documentation audit
(`design/audit-reports/2026-09-24-split-documents-read-from-the-inbound-side.md`), whose lens was
the day's document splits read from the side of the files that cite them. **Name provisional**, this
file's and nothing else's.

No hardware, no decision, no dependency. It extends `script/citations`, which
already checks the same thing for two other shapes.

## The claim

The tree cites a section of a document in one common shape: a path, then the section's heading or a
phrase from it, in quotation marks. `notes/interrupts.md, "Testing it with no device"` is one.
Nothing checks that the quoted phrase is still in the file the path names.

`script/citations` already checks two neighbors of this shape. An attributed block quote
(`> -- path`) must still exist in the file it names, and a gloss after `milestone N` or `§N` must
match the target. The path-then-quote form is the third, and it is the one a document split breaks,
because a split moves the section and leaves the path pointing at the main page.

## The measurement

On 2026-09-24, at the audit's base `edfb4cd6b`, the tree held 121 citations in this shape whose path
resolves to a file. 14 quoted a phrase the file does not contain:

- 9 were rot, and the audit fixed them. Five were headings that moved into an appendix in that
  day's splits, and four were headings in code comments that the note had since renamed or never
  had.
- 5 were not rot: code syntax the pattern caught (`", title: "`), a relative path the scan resolved
  to the wrong README, and three quotes that record an old wording on purpose.

A further 37 citations named a section by date or by a bare noun ("the 2026-09-19 section", "the
multi-hart section"). No script can check that shape without false positives, so it stays with the
documentation sweep.

## What it would be

A `--ratchet` rule in `script/citations`: a line a branch adds that carries `path`, then a quoted
phrase of six characters or more, fails if the phrase is not in that file after whitespace and
markup are normalized. It covers added lines only, like the gloss ratchet, so the five deliberate
exceptions above never fire. An intentional old quotation is marked the way the attributed-quote
form already allows.

## What would make it not worth doing

If a lane's added lines produce more false positives than the 5 in 14 measured over the whole
tree, the rule costs more attention than it saves. The first week's failures are the measurement.

## Built 2026-10-15 (UTC): the third shape is a ratchet rule

Written by the milestone 644 lane. The proposal above is kept as written, with one exception
recorded here. Its example line quoted a phrase a later split of `notes/interrupts.md` had divided
in two, which is the defect this milestone exists for. The example now quotes the heading the file
has today.

`script/citations --ratchet` judges every line a branch adds for the shape: a path that resolves
from the repository root, then a comma, then a quoted phrase of six characters or more. It fails
when the phrase is not in the file the path names, matched on the same normalization as the
attributed quotations (whitespace collapsed, emphasis and double-quote punctuation dropped). A
quotation that is stale on purpose carries `quotation: exception. Reason: ...` on its line or the
one above, the reach `helpers/line_citations.py` allows its own marker. The false-positive shapes
the audit's looser scan caught stay out on resolution. `runs.txt, "<run_id> <lane>"` in a usage
template and `kani_lib.c, "machine model macos"` in a list of search strings name no file from the
root, so they are never judged. The possessive (`AGENTS.md's "old wording"`, the historical form)
has no comma, so the scanner never sees it.

Measured at the lane's base `8fad33799`: 128 citations in the shape, 125 with a path that resolves,
12 quoting a phrase the file does not hold.

- 10 were stale, and this lane fixed them. Six QEMU pointers and the lane-count pointer aimed at
  text the skills split had moved out of AGENTS.md and CLAUDE.md. A comment in
  `crates/hot_swap_contract` quoted a sentence `notes/hung-component.md` no longer has. A census row
  quoted milestone 74 (cycle counters) with "M-74's" where the heading says "milestone 74's". And
  this block's example quoted a heading a later split had divided in two.
  The census row is also the ratchet's third excluded record, after the baseline-drift page. An
  append-only record whose evidence column quotes headings verbatim cannot carry glosses, so
  `notes/dependency-census/dependencies.tsv` is out of the corpus with that reason beside the
  exclusion in the script.
- 1 was not rot: design/roadmap/0610 quotes "What stable would mean" and the note's heading is
  `## What "stable" would mean`. Normalization now drops double-quote punctuation, which its
  docstring had claimed all along, and the citation passes with no edit.
- 1 is routed, not fixed: design/decisions/0196 quotes design/roadmap/0198's "Rescoped
  2026-09-19", gone from that block. A developer lane does not edit `design/decisions/`; the
  finding is in the pull request for the maintainer to land or drop.
- 1 is unchecked by construction: `notes/kani-upstream/pull-request-4913-review.md` cites `kani-upstream.md`
  for `notes/kani-upstream.md`, a walk-up shorthand that resolves neither from the root nor from
  the citing file's directory. That is a BUGS entry, not a false positive.

The gate's first week starts from that zero. Proven on this branch before the docs landed, by
planting one line at a time and committing it. A stale quotation failed with its line and phrase. A
current phrase passed. The marker above the line exempted a planted deliberate quotation. The gloss
ratchet still fired on the same plant for its own reason, a bare `milestone 644` with no gloss. The
two rules compose rather than compete.

Against the proposal's bar for not worth doing: the audit's five non-rot cases do not match the
landed shape, so the measured false-positive rate on added lines starts at zero.

## Follow-on

- **Recorded.** `design/decisions/0196-nife-carries-tls-and-builds-the-provider.md` quotes
  design/roadmap/0198's "Rescoped 2026-09-19", which
  that block no longer says. A developer lane does not edit `design/decisions/`, so the one-line
  fix travels in this lane's pull request for the maintainer to land or to rule accepted, as the
  2026-09-24 audit's accepted findings were.
- **Recorded.** A citation whose path is a walk-up shorthand (`kani-upstream.md` meaning
  `notes/kani-upstream.md`, written from a sibling directory) resolves nowhere and is skipped. The
  limit lives in the BUGS of `notes/citations.md`. The audit's wrong-README finding is why
  resolution does not guess.

## Index row

`script/citations --ratchet` checks three citation shapes; the third, since 2026-10-15, is that a
phrase quoted after a path must still be in the file the path names, on lines a branch adds. A
deliberate old quotation carries `quotation: exception. Reason: ...` on its line or the one above.
Normalization drops double-quote punctuation, so a heading that scare-quotes its term still
matches.
