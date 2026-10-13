---
status: BUILT
raised: 2026-10-08
built: 2026-10-08
milestone_dependencies: none
decision_dependencies: 266
machine_requirements: none
specific_machine: none
needs_person: no
---
# 841. A ratchet on Rust file length, and its dashboard row

*(The title and slug are provisional.)*

§266 (a Rust source file stays under 2,000 lines) sets a 2,000-line ceiling on every tracked
Rust file outside `vendor/`, held by a ratchet. This milestone builds the ratchet in `script/lint`
and puts the number on `notes/project-metrics.md`. It does not split any file. Milestone 840 (the
scheduler file is split along its seams) splits the largest.

calef ruled §266 on 2026-10-08 (UTC): the measure, the ceiling and the goals as written, and
option (b) for the gate's matching rule, a list entry that is a ceiling and only falls. This block
builds what that ruling says.

Reuse: `helpers/prose_ratchet.py` and `design/prose-baseline.tsv` are §212 (a prose budget)'s
ratchet, and this copies their pattern: a committed list that only falls, a comparison against the
merge base, and a selftest run before the check. The line count is the one `script/metrics` already
takes in its `Tree` class, which reads every `.rs` blob outside `vendor/` and `target/`. Nothing is
taken from outside the tree. rustc's `tidy` has a file-length check, but it counts non-comment
lines and exempts by a marker in the file, and §266 refuses both.

## Today

Measured on 2026-10-08 at `51ccfad65` with
`git ls-files -z '*.rs' | grep -zv '^vendor/' | xargs -0 wc -l`: 710 files, 369,902 lines. Twenty
files are over 2,000 lines and hold 21.6% of them. §266 lists all twenty. Nothing in `script/lint`
reads a source file's length.

## What to build

Every name here is provisional.

1. `helpers/file_length_ratchet.py`, with `--check`, `--bank` and `--selftest`, the same verbs
   `helpers/prose_ratchet.py` takes. Its header is its manual, as that file's is.
2. The list, `design/file-length-baseline.tsv`: one row per file over 2,000 lines, path and lines,
   with a header saying what the file is and how a row may change. `--init` writes it once, in this
   milestone's pull request, and is then removed or refused.
3. A `script/lint` step that runs `--selftest` and then `--check`, beside the prose ratchet's.
4. The dashboard series in `script/metrics`, described below.

## What the check enforces

- A file not on the list fails at 2,001 lines or more. A new file is not on the list.
- A listed file fails if it grew against the merge base.
- A list entry is a ceiling that `--bank` lowers and nothing raises (§266's option (b)). It fails
  when its file is gone, renamed, at or under 2,000 lines, or larger than the entry. §266 refused
  exact match, option (a), because two shrinking changes to one file in a merge group would each
  write a different number on the same row.
- A row may not be added and an entry may not rise, against the merge base. A rename moves its row
  in the same change, at the same number.
- The failure message names the file, its size and the ceiling. It says the remedy is to split the
  file along a seam and never to strip comments, quoting §266's measure section.

## The dashboard row

`script/metrics` gains a weekly series, `notes/project-metrics/file-size.csv`, one row per ISO week
read from that week's commit, like every other series. Columns: files over 2,000 lines, their share
of all Rust lines, the largest file and its size, the 95th percentile (nearest rank), and the share
of lines in files over 1,000.

`notes/project-metrics.md` is a deck (calef, 2026-09-24): a heading, a chart, and a line or two.
So the page gains a section with a chart of the share of lines in files over 2,000, which is the
headline because one split moves it. Under the chart, `script/metrics` writes one line between
markers, in this form, from that week's numbers:

> **File size:** 20 Rust files over 2,000 lines, holding 21.6% of all Rust lines; largest
> kernel/src/sched.rs (8,921); 95th percentile 1,511.

The 95th percentile in that example is nearest-rank. The request this was filed from quoted 1,484,
which is the 674th of 710 files rather than the 675th.

The definitions go in `notes/register-of-measures.md`, where the deck's rule sends everything
longer than a caption. That entry says the measure counts comments on purpose and links §266.

## Done when

1. Adding a new 2,001-line `.rs` file outside `vendor/` makes `script/lint` fail, and a 2,000-line
   one passes. The selftest carries both as fixtures.
2. Adding one line to a listed file makes `script/lint` fail. The selftest carries that too.
3. An entry left behind for a file that dropped to 2,000 lines, or was deleted, makes
   `script/lint` fail.
4. `script/lint` passes on the tree with the list as `--init` wrote it.
5. `script/metrics --update` writes the file-size series with a row for every week since 2026W29,
   and `notes/project-metrics.md` shows the chart and the one-line row for the current week.
6. `notes/register-of-measures.md` defines the series.

## BUGS

- The ratchet slows feature work in the twenty listed files, which took a quarter of the merges to
  `main` in the two weeks before this was filed. That is intended, and §266 records the number.
- The check cannot tell a split from a deleted comment. §266 makes that a review question.

## Follow-on

- **None.** The splits this gate exists to force are filed where the number is. Milestone 840 (the
  scheduler file is split along its seams) covers the largest file. §266 section 4 records that the
  2026-12-31 goals need a split milestone for each of the other six files over 4,000 lines, plus
  three of the thirteen between 2,000 and 4,000. Filing them is an architect's call each, not this
  block's.

## Index row

No Rust file may cross 2,000 lines, and the twenty already over it may only shrink, held by a
ratchet in `script/lint` copied from §212's prose ratchet. The share of Rust lines in files over
2,000 goes on the metrics deck as a weekly series. The ceiling, the measure and the gate's matching
rule come from §266.
