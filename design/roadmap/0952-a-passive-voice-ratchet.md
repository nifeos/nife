---
status: BUILT
built: 2026-10-25
raised: 2026-10-25
milestone_dependencies: none
decision_dependencies: 213
machine_requirements: none
specific_machine: none
needs_person: no
---
# 952. A passive-voice ratchet in lint, and its metric

*(Title and slug provisional.)*

calef ruled on 2026-10-25 (UTC), choosing option A of three:

> "Lets implement A. I think we also want a metric graph of the passive voice in the tree so that
> we can drive it down over time."

Option A: new prose carries at most 2 passive constructions per 1,000 prose words, and an existing
document keeps a count that only falls. [§213 (writing standards)](../decisions/0213-writing-standards.md)
states it as rule 6, and its appendix
[`passive-voice.md`](../decisions/0213-writing-standards/passive-voice.md) records the two options
calef refused. This block builds the gate and the chart.

## What it is

- One counter. `helpers/passive_voice.py` matches a form of "be", then a word ending in "ed" or an
  irregular past participle: Vale's `write-good.Passive` rule, adopted with its MIT notice.
  `NOT_PASSIVE` lists three words the rule misreads in this tree, each with its reason.
- One prose. `helpers/prose_ratchet.py` runs the counter over the blocks its sentence limits read.
  Code, comments, frontmatter, headings, table rows and quotes stay out, and a roadmap block's
  field tokens read as syntax.
- One limit. A document of at least 200 prose words fails over 2 per 1,000, the floor and the
  rounding the bold budget uses. A `writing-standards` exception marker exempts it.
- One baseline. `design/passive-baseline.tsv` holds a count per document over the limit on the day
  this landed. `--init-passive` wrote it and refuses to run again; `--bank` lowers it. `check`
  refuses a row added or raised against the merge base, a count over its row, and a count above
  its merge-base count. A renamed document keeps its old path's row.
- One chart pair. `script/metrics` writes `passive-voice.csv` each week through
  `prose_ratchet.measured`, so the chart and the gate count the same sentences.
  `notes/project-metrics.md` draws the total, and the rate against the limit.

Unlike the bold rule, an edit does not put a document under the limit at once. Ruling A holds an
old document to its count, and the count falls as people rewrite.

Reuse: the word list and the regular expression come from `errata-ai/write-good`, MIT, after
Brian Ford's `btford/write-good`. Copying them adopts code and takes no dependency (AGENTS.md rule
6), so Vale stays out of the tree and out of CI. The prose reader, the merge-base comparison and
the chart writer already existed here.

## Built 2026-10-25 (UTC)

Lane `milestone/952-a-passive-voice-ratchet`.

- `python3 helpers/prose_ratchet.py --selftest` drives the counter's eleven cases, ten prose
  fixtures and seven verdicts. A passive in code, a comment, a heading, a table or a quote does not
  count; a document over its row, or grown since the merge base, fails.
- `--init-passive` wrote 2,357 rows. `--check` passes on the tree and reports them held.
- `script/metrics --backfill-measure passive-voice` filled 2026W29 to 2026W43 in 4 minutes 40
  seconds. At 2026W43 the scope holds 27,447 passives in 3,292,192 prose words, 8.34 per 1,000.
- `script/lint` passes.

## BUGS

- A word between the auxiliary and the participle hides a passive: "is not used", "was then
  built". write-good's rule has the same gap, and closing it means an adverb list.
- A stative adjective reads as a passive: "is unchanged", "is untouched", "is unmeasured". These
  three alone match 468 times. An adjective cannot go on `NOT_PASSIVE` when its verb exists
  ("is unmapped" is a true passive), so they count.
- Only Markdown in the prose ratchet's scope counts. Rust doc comments and other Markdown sit
  outside, and a later milestone would widen the scope rather than this counter.
- The 12-hit sample behind the rule's precision came from Vale on the whole tree, not from this
  counter on the scope; nobody has sampled this counter's hits by hand.
- `--remeasure-passive` has no caller yet. It exists for the first change that raises counts, an
  entry leaving `NOT_PASSIVE` for one.

## Follow-on

- **Recorded.** Rust doc comments and Markdown outside the ratchet's scope stay uncounted, in this
  block's `BUGS` (`design/roadmap/0952-a-passive-voice-ratchet.md`).
- **Recorded.** The adverb gap and the stative adjectives, in the same `BUGS` and in
  `helpers/passive_voice.py`'s header.

## Index row

§213 rule 6 holds new prose to 2 passive constructions per 1,000 words and holds every older
document to a count that only falls. `script/lint` gates it through the prose ratchet, and a weekly
chart shows the tree's rate, 8.34 per 1,000 when it landed, so the backlog falls without a sweep.
