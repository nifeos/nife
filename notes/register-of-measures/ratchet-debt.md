# Two ratchets' debt

*Name: provisional, minted 2026-10-13 (UTC) by lane/ratchet-metrics, with the files and charts it
defines. Naming is an architect's.*

*(Appendix of [the register](../register-of-measures.md). Stem provisional, minted on 2026-10-13
(UTC) by lane/ratchet-metrics. `line-citations.csv`, its columns, its chart and
`line_citation_debt` are provisional too, minted 2026-10-14 (UTC) by
maintainer/line-citations-chart. calef asked for these charts so the baselines' debt can be driven
down.)*

Two gates in `script/lint` hold a debt to a baseline that only falls. They enforce §213
(writing standards) and §277 (cite a heading or a symbol, never a line number). A baseline says what may not grow. These series say how much
is left, one row per ISO week in `notes/project-metrics/`, read by `script/metrics` from that
week's commit.

| file | column | what it is |
|---|---|---|
| `passive-voice.csv` | `passives_total` | passive constructions in the prose of every document |
| | `passive_prose_words` | the prose words they sit in |
| | `passives_per_1000` | the first over the second, per 1,000 words |
| `line-citations.csv` | `line_citations_code` | citations by line number into code |
| | `line_citations_doc` | citations by line number into a Markdown document |
| | `line_citations_ceiling` | the week's own `CODE_CEILING`, blank before the gate existed |

## One measure, the gate's

Each series calls the gate's own functions on the week's text, so the chart and the gate cannot
disagree about a file:

- §277: `line_citations.file_hits` over its `in_scope` files, every text file but data, symlinks and
  `vendor/`. `script/metrics` reads the ceiling from the week's own `helpers/line_citations.py`, so
  the chart draws the ceiling that week's gate held.

## Passive voice

`passive-voice.csv` and its two charts answer calef's ask of 2026-10-25 (UTC), "a metric graph of
the passive voice in the tree so that we can drive it down over time". `passive_voice_counts` sums
`prose_ratchet.measured`'s `passives` and `prose_words` over the same scope, so the chart counts what
the gate counts. Every document counts, the short and the exempted too, and the rate pools the
words rather than averaging documents. `--backfill-measure passive-voice` filled every week from
2026W29. The file, its columns and its charts are provisional, minted by
milestone/952-a-passive-voice-ratchet.

## Counted from the tree, exemptions included

The series never read a baseline's block rows; the directory series read its `cap` and `in-body` rows only. A baseline holds ceilings, and a ceiling can sit above
what the tree measures until someone banks the shrink; the tree is the debt. A `writing-standards`
exception marker and a `line-citation: exception` marker excuse an item at the gate and not here: a chart that subtracted what the
gate excuses would hide the debt it exists to show.

## Every week, from 2026W29

`--backfill-measure` restated each series from 2026W29, each week from its own commit. The helpers
are today's, applied to old text, so a week from before a gate existed reads as the debt that gate
would have found then. That is every row's rule here, and it is why no week is blank.

## BUGS

- Today's measure applied to an old week can read a construct the old tree used differently. The
  roadmap's field tokens are one: `measured` reads both forms, so they do not inflate the median.
- The line-citation gate landed in 2026W41, so earlier weeks are today's counter on older text.
  They have no ceiling and no marked exceptions, and their Markdown citations were never held.
- The ceiling compares with the gate's count, which skips a marked code citation and this chart does
  not. They agree while no code citation carries a mark, as in 2026W42.
- `other` has no ceiling column, and a ceiling on a prefix that is not one of the five named
  directories (`kernel/src/`, `helpers/`) is not charted. Its directories would start their stages
  one at a time, so a partial sum would sit under the group's count and read as a breach.
- A change to a gate's measure restates nothing. `--update` rewrites the current week and the
  missing ones, so a series reads two definitions until someone runs `--backfill-measure`.
- None of these sees quality. A shorter sentence can be worse, and a comment block cut to 40 lines
  can lose a fact; §267's remedy is to move history to the commit message, not to delete it.
