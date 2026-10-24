# Three ratchets' debt

*Name: provisional, minted 2026-10-13 (UTC) by lane/ratchet-metrics, with the files and charts it
defines. Naming is an architect's.*

*(Appendix of [the register](../register-of-measures.md). Stem provisional, minted on 2026-10-13
(UTC) by lane/ratchet-metrics, as are the files `writing-standards.csv` and `comment-blocks.csv`,
their columns and their charts. `line-citations.csv`, its columns, its chart and
`line_citation_debt` are provisional too, minted 2026-10-14 (UTC) by
maintainer/line-citations-chart. `comment-by-directory.csv`, its columns, its three charts and
`comment_debt_by_directory` are provisional, minted 2026-10-23 (UTC) by
comments/929-metrics-per-directory. calef asked for these charts so the baselines' debt can be driven
down.)*

Three gates in `script/lint` hold a debt to a baseline that only falls. They enforce §213
(writing standards), §267 (a comment states the constraint as it is now) and §277 (cite a heading
or a symbol, never a line number). A baseline says what may not grow. These series say how much
is left, one row per ISO week in `notes/project-metrics/`, read by `script/metrics` from that
week's commit.

| file | column | what it is |
|---|---|---|
| `writing-standards.csv` | `writing_over_median` | documents whose median sentence is over 20 words |
| | `writing_over_longest` | documents whose longest sentence is over 40 words |
| | `writing_over_bold` | documents with more than 4 bold spans per 1,000 words |
| `comment-blocks.csv` | `comment_block_files_over_cap` | Rust files holding a comment block over 40 lines |
| | `comment_blocks_over_cap` | the blocks themselves, which is what a baseline row counts |
| | `comment_block_excess_lines` | each such block's lines past 40, summed |
| `comment-by-directory.csv` | `comment_cap_<dir>` | the cap the week's baseline puts the directory at; `other` is its lowest |
| | `comment_blocks_over_cap_<dir>` | blocks over that cap |
| | `comment_excess_lines_<dir>` | their lines past it, summed |
| | `comment_in_body_<dir>` | plain comment lines inside function bodies |
| | `comment_in_body_ceiling_<dir>` | the baseline's `in-body` row for the directory, blank without one |
| `line-citations.csv` | `line_citations_code` | citations by line number into code |
| | `line_citations_doc` | citations by line number into a Markdown document |
| | `line_citations_ceiling` | the week's own `CODE_CEILING`, blank before the gate existed |

## One measure, the gate's

Each series calls the gate's own functions on the week's text, so the chart and the gate cannot
disagree about a file:

- §213: `prose_ratchet.measured` measures each document and `prose_ratchet.over` judges it, over
  `prose_ratchet.documents`, the scope `prose_budget` already shares. The 200-word floor for the
  median and the bold density is the gate's. A document over two limits counts in both columns, so
  the chart groups the three bars rather than stacking them.
- §267: `comment_block_ratchet.blocks_over` over its `in_scope` files, every `.rs` outside
  `vendor/`. `comment-blocks.csv` judges every file at 40. `comment-by-directory.csv` judges each
  at `cap_of` its path under that week's `cap` rows and counts `in_body_lines`, for milestone 929
  (comments in the Linux style across first-party Rust). `<dir>` is `kernel`, `crates`,
  `components`, `system_tests`, `xtask`, or `other` for every other directory holding Rust.
- §277: `line_citations.file_hits` over its `in_scope` files, every text file but data, symlinks
  and `vendor/`. The ceiling is read from the week's own `helpers/line_citations.py`, so the chart
  draws the ceiling that week's gate held.

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

- `writing_over_bold` reads zero from 2026W41, where §213's bold backlog was cleared, and the gate
  refuses any bold ceiling. It stays a column so a regression would show.
- Today's measure applied to an old week can read a construct the old tree used differently. The
  roadmap's field tokens are one: `measured` reads both forms, so they do not inflate the median.
- The line-citation gate landed in 2026W41, so earlier weeks are today's counter on older text.
  They have no ceiling and no marked exceptions, and their Markdown citations were never held.
- The ceiling compares with the gate's count, which skips a marked code citation and this chart
  does not. They agree while no code citation is marked, as in 2026W42.
- `other` has no ceiling column, and a ceiling on a prefix that is not one of the five named
  directories (`kernel/src/`, `helpers/`) is not charted. Its directories would start their stages
  one at a time, so a partial sum would sit under the group's count and read as a breach.
- A change to a gate's measure restates nothing. `--update` rewrites the current week and the
  missing ones, so a series reads two definitions until someone runs `--backfill-measure`.
- None of these sees quality. A shorter sentence can be worse, and a comment block cut to 40 lines
  can lose a fact; §267's remedy is to move history to the commit message, not to delete it.
