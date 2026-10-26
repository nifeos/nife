# Fatal risk colors, by week

*An appendix to [the register of measures](../register-of-measures.md). The stem is provisional.*

## What is counted

One row per ISO week in `notes/project-metrics/fatal-risks-colors.csv`: how many of the nine risks
carry each verdict color that week. Three count columns, green, amber and red, and `fatal_risks_total`, every risk in that week's
summary, so the chart reads one file. The chart draws
gray for the total minus the three, which is a risk with no verdict rendered.

## Where the color lives

calef commissioned this on 2026-09-29, built as milestone 625 (fatal risk colors, counted by
week), whose number is provisional:
[`design/roadmap/0625-fatal-risk-colors-by-week.md`](../../design/roadmap/0625-fatal-risk-colors-by-week.md).
Each appendix under `design/fatal-risks/` carries frontmatter: `risk:`, `color:`, `updated:`. The color is the verdict word, green, amber,
red or none, and not a hex. The word is the architect's verdict, the one the README states in
prose; the hue is a rendering choice. `none` is the fourth value because five of nine risks have
no verdict at all. It counts in no column and draws gray. `updated:` is the UTC date that color
last changed on `main`. The hues live in one mapping in `script/metrics`, so changing one is one
edit rather than nine. They are calef's to change.

Where both sources exist they must agree, and a disagreement fails the count, naming the risk. That
cross-check is the only thing that gates an appendix: `script/fatal-risks` parses the README alone,
and its own BUGS says so.

## The backfill, and its limits

History is backfilled once from git history, at each week's recorded tip, by the same status-line
walk the status chart uses. It keys on the first capitalized color word after each entry's status
marker. Capitalization is the discriminator, because risk 2's line says AMBER and then, lowercased,
names the red half of its finding. Prose is not data, which is the argument for the frontmatter.
Weeks before 2026W36 have no file and count nothing. A verdict shows the week its word landed on
`main`: risk 9's green is 2026W39, though its status line names 2026-09-17.

`MEASURED` and `AUDITED` are prose qualifiers about how well an experiment was done, and never a
color, per the proposal's own rule.

## BUGS

- The backfill reads the first color word, so a color word in the wrong place in an old blob
  would be read as a verdict. None is known of.
- Which risk turned is not in this series: a week where one risk turned amber moves a bar by one,
  and the README names the risk. This chart is the glance, not the record.
- Red has never occurred, so the red series is zero throughout and draws nothing; it joins the
  chart the week a risk first turns red.
