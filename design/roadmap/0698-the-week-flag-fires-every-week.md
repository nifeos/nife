---
status: NOT-STARTED
raised: 2026-09-29
promoted_from: the-week-flag-fires-every-week
milestone_dependencies: 623
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 698. The week flag fires every week

*Title and slug are drafts.*
Raised by milestone 623 (bullet under the chart explains a cliff), which built the flag calef
commissioned.
The threshold came from the proposal verbatim. The build measured what it does on real data, and
the measurement belongs in front of whoever rules on the constant.

## What the build measured

`script/metrics --flags` over the ten charted weeks 2026W31 to 2026W40, each week compared with
the week before it in the series, seventy-three series charted. Variants were measured by editing
one constant and one comparison in copies of the built flag. The table is therefore the
implementation's own arithmetic and not a re-derivation:

| variant | series flagging, by week (min to max) | weeks with nothing flagged |
| --- | --- | --- |
| one tenth, both directions (as built) | 16 to 44 | 0 of 10 |
| one tenth, drops only | 4 to 18 | 0 of 10 |
| one quarter, both directions | 10 to 33 | 0 of 10 |
| one quarter, drops only | 3 to 16 | 0 of 10 |

The cause is structural, not a bad week. Most charted series are counts or churn flows, and a
young project grows and churns them faster than any of these bars. Under every variant the
2026W40 kernel cliff flags (kernel code fell 29%). Under every variant something flags every
week, so no flat bar fully separates a cliff from ordinary motion on this data.

A drift that moves a series a twentieth each week never flags. The original proposal already
recorded that as the threshold's other blind spot.

## Options

| option | what it is | verdict |
| --- | --- | --- |
| A. Keep one tenth, both directions | the rule as built | Refused as the answer. Measured: 16 to 44 series a week flag, and the one-line report is ten lines most weeks |
| B. Drops only, one tenth | flag only a series that fell more than a tenth | **Recommended.** Cliffs read as drops: the motivating case, and 2026W38's proposals falling 83 to 2, are drops, while growth is this project's normal direction. Min to max falls to 4 to 18 |
| C. One quarter, both directions | raise the bar on either direction | Refused. Measured 10 to 33. It buys less than B and buries small real drops, a 15% fall included |
| D. Drops only, one quarter | B with a higher bar | Refused. Measured 3 to 16, the quietest, but it would miss a 20% fall in a series that had earned a reader's trust |
| E. A per-series band | flag a move outside the series' own trailing range | Refused by cost: seventy-three bespoke bands are a second dashboard to maintain |

The recommendation is B: one word of rule added to the constant already in `script/metrics`. The
honest caveat is that even then a week with nothing worth explaining will sometimes flag, because
this tree's counts move. An architect rules with the table above in hand. The constant lives beside
`CHART_WEEKS` either way.

Name provisional.

## Index row

The week-over-week flag calef commissioned flagged 16 to 44 of 73 series every week for ten weeks. Proposed: rule on a threshold that flags something rarer, with the measured variants in front of the ruling.
