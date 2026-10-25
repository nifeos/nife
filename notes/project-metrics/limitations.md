# Project metrics: known limitations

*Name: provisional, minted 2026-10-19 (UTC) by lane/names-chart-w43-note. calef names things.*

The `BUGS` section of [notes/project-metrics.md](../project-metrics.md): what its series and charts get wrong or cannot see.

## BUGS

- `script/metrics --check` is deliberately not in `script/lint`. Any commit changes `HEAD`, and
  the current week's row records the commit it was taken at, so a gate on it would fail every pull
  request that touched anything. The workflow is the mechanism; this is rung two of `AGENTS.md`'s ladder declining to be rung one,
  said out loud.
- A newly added column can go blank across history if a CSV merge conflict is resolved with
  `--update`, which restates only the current week and past weeks not at their last commit,
  instead of `--backfill`. Merging two branches' concurrently added columns needs a `--backfill`
  afterward, or the older rows carry empty cells (the register's *The weekly series* has the one
  case). Empty, not zero, is the tell: run `git diff` on `notes/project-metrics/` after merging two
  metrics branches.
- `milestones_built_this_week` will not equal the week-on-week change in the `Built` stock, in
  any week. The gap is the lag between finishing a milestone and flipping its row, and the register
  argues it; nothing gates the two against each other and nothing can.
- A milestone dated in a week the series has no row for is dropped, which is milestone 1 (boot
  to Rust on QEMU `virt`, and print to the PL011 UART) and 2026W28. `script/metrics` prints a stderr
  line naming it since 2026-09-23; before that it was silent. Summing the chart gives one fewer than the tree's dated blocks.
- Two of the three definitions are shared, and the third is checked (milestone 236 (three derivations are
  copied between scripts), 2026-09-03). The `unsafe` census and the comment-and-literal strip the code and comment line split
  is built on live in `helpers/rust_source.py`, which `script/lint` and this script both import, so
  there is one definition and nothing left to drift. The harness count could not be collapsed the
  same way: `script/lint` and `script/falsifications` attribute each harness to a workspace package
  out of `cargo metadata`, and this script reads blobs at revisions nobody has checked out and
  cannot run cargo against them. `script/lint` runs all three derivations and fails on a disagreement, the weaker answer.
- A line inside a multi-line string literal counts as a comment line: wrong in principle,
  negligible here.
- The charts follow the reader's operating system color preference, not GitHub's theme toggle.
  GitHub serves the SVG as an `<img>`, so a media query inside it cannot see the page theme.
- `patches/` is outside the `unsafe` census, inherited from `script/lint` along with its reason.
  That code does run on the machine, so it is a real hole rather than a boundary, and
  `notes/register-of-measures.md` records the blocks it leaves uncounted.
- `names_total` is not the sum of the naming columns, which is the one place a column here
  breaks the pattern the milestone and decision columns set. It is every named thing in that week's
  tree; the four statuses count the ones whose header carries a block that parses. The gap is drawn as a band, explained in the register, and not folded into `Unrecorded` because
  silence is not a claim.
- The naming columns count signatures, not names. A name can be ratified and bad, or provisional
  and perfect. Nothing here reads a name, and `script/names`' own `BUGS` is the longer version: it
  cannot check that a recorded reason is still true, that a date is right, or that a `recorded`
  citation leads anywhere.
- An item is a `pub` line, found by a regular expression rather than by the compiler
  (`name_census.pub_items`). An item a macro generates is missed, and a `pub` line inside a block
  comment or raw string is counted. `pub(crate)` items are not public and are not counted unless
  they carry a block. 2026W42 has 7,419 `pub` items, 2,955 of them constants. `helpers/` helpers
  are outside `script/names`' surfaces, and design/naming.md's
  `BUGS` carries what that leaves uncovered.
- `proposals_unnumbered` is a net count and cannot see the flow. Five proposals have left the
  directory and 81 remain; a flat line fits both a stalled pile and one draining as fast as it fills. The measurement that would tell them apart is the age of the oldest,
  which `script/roadmap --check` prints on every lint run and this column does not carry.
- `merged_pull_requests` can only see GitHub's default merge subject. A merge made any other way
  is not counted and cannot be distinguished from an ordinary merge commit afterwards. The total matched a maintainer's hand count on 2026-09-21, evidence and not proof.
- A band that goes to zero across the whole chart window changes the colors of the bands after
  it. `series_of` drops an all-zero series and the palette is indexed over what survives, so when
  a band leaves the ten-week window every band after it shifts one hue, as the by-model legend's did
  when 2026W30 left. The legend is redrawn with it, so nothing is mislabelled, but a color moves between screenshots.
- The cost and context columns carry seven limitations of their own, from a capture that can
  stop silently to a price [ledger](ledger.md) that re-prices history. They are listed where those columns are
  argued: [what this project costs, *Known limitations*](../register-of-measures/project-cost.md#known-limitations).
- Four series were restated on 2026-09-24, and an older screenshot will disagree with each.
  Opus 5.5 got its own column (`opus_5_5`), so this week's commits left *other models*, and Opus 4.8
  now draws inside that band because the palette has eight hues. The fatal-risks chart (since removed in favor of the colors chart, which now stores the total itself) gained
  `fatal_risks_total` and an *unclassified* band, so 2026W36 to 2026W38 read nine rather than five,
  four and five. The coverage floor was backfilled from each week's own tree.
- Nothing here is audited outside this project, as the page states at the top.
