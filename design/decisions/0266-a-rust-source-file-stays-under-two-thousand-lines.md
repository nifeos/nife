---
status: DECIDED
raised: 2026-10-08
decided: 2026-10-08
ratified_by: calef
---

# 266. A Rust source file stays under 2,000 lines, held by a ratchet that only falls

Raised 2026-10-08 (UTC). calef approved filing a proposal to bring source file sizes down the same
day, after milestone 840 (the scheduler file is split along its seams) found `kernel/src/sched.rs`
at 8,921 lines. Written by lane `file-size-budget`. *(Section number provisional until the merge
queue lands it. Every script and file name below is provisional too.)*

The build is [milestone 841 (a ratchet on Rust file length, and its dashboard row)](../roadmap/0841-a-ratchet-on-rust-file-length-and-its-dashboard-row.md).
Nothing is built on this section yet.

## The ruling

calef ruled on 2026-10-08 at 21:20 (UTC), in the maintainer session, and the maintainer recorded it
on [PR #1867](https://github.com/nifeos/nife/pull/1867#issuecomment-6070003952) the same evening.
Two questions, two answers:

| Question | calef's words | What it decides |
|---|---|---|
| The measure, the 2,000-line ceiling, and the goals with their dates | *"Yes."* | Sections 1, 2 and 4 hold as written. |
| The gate's matching rule, (a) or (b) | *"B"* | Option (b): a list entry is a ceiling that only falls, as the prose ratchet works. Option (a) is refused; section 3 gives the reason. |

## What this decides

1. The measure: what counts as the size of a file.
2. The ceiling: 2,000 lines.
3. The gate: a ratchet in `script/lint`.
4. The goals, with their dates.

§212 (a prose budget) made this decision for prose on 2026-09-23. This section makes it for Rust,
and copies §212's shape on purpose: a cap, a ratchet so the files already over it do not all go
red, and a chart so the trend is visible.

## The evidence

Measured on 2026-10-08 at `51ccfad65`. This branch's base, `bb2be83ec`, changes no Rust file
since then, so the figures hold there. The method is every tracked `.rs`
file outside `vendor/`, then `wc -l`:

```console
$ git ls-files -z '*.rs' | grep -zv '^vendor/' | xargs -0 wc -l
```

That is 710 files and 369,902 lines. One tracked file has "vendor" in its name,
`crates/machine_discovery/tests/riscv64_jh7110_vendor.rs`, and it is ours, so it is counted.

| statistic | lines |
|---|---|
| median | 293 |
| 95th percentile | 1,511 |
| 99th percentile | 3,885 |
| largest, `kernel/src/sched.rs` | 8,921 |

Percentiles are nearest-rank: the 95th is the 675th of 710 files, sorted by size.

| files over | count | share of all Rust lines |
|---|---|---|
| 1,000 | 94 | 48.3% |
| 2,000 | 20 | 21.6% |
| 4,000 | 7 | 11.7% |

The twenty over 2,000, largest first:

| file | lines |
|---|---|
| `kernel/src/sched.rs` | 8,921 |
| `crates/grant_plan/src/lib.rs` | 7,080 |
| `crates/system_initializer/src/lib.rs` | 6,358 |
| `components/src/swish.rs` | 5,980 |
| `crates/file_system_contract/src/lib.rs` | 5,744 |
| `redoxfs_server/src/lib.rs` | 4,785 |
| `xtask/src/swish_check.rs` | 4,576 |
| `crates/swish/src/lib.rs` | 3,885 |
| `kernel/src/user.rs` | 3,867 |
| `system_tests/src/user/tests.rs` | 3,533 |
| `crates/video_terminal/src/lib.rs` | 3,242 |
| `crates/machine_discovery/src/acpi.rs` | 3,009 |
| `kernel/src/lib.rs` | 2,984 |
| `crates/timetable/src/lib.rs` | 2,468 |
| `kernel/src/user/fs_service.rs` | 2,407 |
| `crates/line_editor/src/lib.rs` | 2,290 |
| `components/src/login.rs` | 2,287 |
| `kernel/src/arch/x86_64/mmu.rs` | 2,192 |
| `kernel/src/bench.rs` | 2,110 |
| `crates/component_plan/src/lib.rs` | 2,094 |

The trend, from the first-parent commit of `main` before each date, by the same method:

| date | commit | files | lines | over 2,000 | their share | largest |
|---|---|---|---|---|---|---|
| 2026-08-08 | `f6fd09748` | 274 | 126,117 | 8 | 21.5% | 4,746 |
| 2026-09-08 | `3e0bb9092` | 401 | 216,560 | 12 | 23.6% | 11,127 |
| 2026-09-24 | `31638e815` | 519 | 258,490 | 12 | 17.2% | 6,874 |
| 2026-10-08 | `51ccfad65` | 710 | 369,902 | 20 | 21.6% | 8,921 |

The share has sat between 17% and 24% for two months while the tree tripled. Files get split
when somebody notices, which is rung zero. Of the twenty, 17 were at the same path 14 days
earlier. Sixteen of those grew and one shrank, and seven of them crossed 2,000 inside that window.

## 1. The measure: physical lines, comments included

The size of a file is its physical line count, `wc -l`, with comments, blank lines and inline
`mod tests` counted.

The reason is the reader. A person or a model opening a file loads every line of it, comments
included, and the comments are most of what makes a file here readable. This tree comments on
purpose. `notes/project-metrics/lines.csv` for 2026W41 counts 139,983 comment lines against 201,975
code lines, so comments are 41% of the non-blank Rust.

That choice creates an incentive, and it has to be named where the rule is. A file at 2,050 lines
can pass by deleting 50 lines of comments. That would make the tree worse to buy a number, and it
is the opposite of what this section is for. The remedy for a file over the ceiling is to split
it along a seam. It is never to strip comments, join lines, or move a test module into a file
nobody reads. A review that sees comments removed in a change that also moves a file under 2,000
lines should send it back. No gate can tell a deleted comment that was stale from one that was
load-bearing, so this is a review question, the same as §212's completeness rule.

## 2. The ceiling: 2,000 lines

Two reasons, one checked and one a judgment.

The checked one. Claude Code's `Read` tool loads 2,000 lines by default; its own description in
this session says "Reads up to 2000 lines by default". A file over that cannot be seen in one read.
An agent either pages through it or works from the part it saw, and the second is how a lane edits
a function whose caller is 3,000 lines further down. Most of this tree is written by agents, so the
tool's window is a real constraint on correctness, not a preference. Other agent tools' defaults
were not checked.

The judgment. 2,000 lines is about what a person can hold in mind for one file. Nobody measured
that, and the BUGS section says so.

A file of 2,000 lines is allowed. A file of 2,001 is over.

## 3. The gate: a ratchet in `script/lint`

- No file outside the list may exceed 2,000 lines. A new file meets the ceiling outright.
- The files over 2,000 lines when the gate lands go on a list, each with its size on that day. A
  listed file may only shrink.
- A list entry that no longer matches its file fails. The list therefore cannot go stale.
- Exceptions are not provided for. A file that must stay over 2,000 lines needs this section
  amended, by an architect.

What "matches" means was put to calef as two options, because the measurement below argues
against the simplest reading. He ruled (b).

(a) Exact match, refused. Each entry records the file's current size, and any difference fails. The list
is always the truth. The cost is that every change to a listed file edits the list too. In the 14
days to 2026-10-08, 164 of the 664 first-parent merges to `main` touched at least one of the
twenty files, and 47 touched `kernel/src/sched.rs`. Two such pull requests in one merge group
(the queue builds groups of up to five, per `notes/merge-queue.md`) each write a different number
on the same line. The group then fails even when the Rust merged cleanly, and one is ejected.

(b) A ceiling that only falls, decided. Each entry is a ceiling. A listed file may not grow against the
merge base and may not exceed its entry. An entry fails if its file is gone, renamed, or at or under
2,000 lines, so dead rows cannot accumulate. A banking command lowers entries to the tree and never
raises one. This is how `helpers/prose_ratchet.py` and `design/prose-baseline.tsv` already work for
§212. Two shrinking changes in one group both pass. The cost is that an entry can sit above its
file's real size until someone banks it. The dashboard row reads the tree, not the list, so that
staleness reaches no number anyone quotes.

The decision is (b), and (a) is refused. The reason is the merge-group failure under (a), which
holds at equal cost, so the choice is not about effort. The request this section was written from
said (a); the proposal departed from it on purpose, the pull request named that, and calef ruled
(b) on 2026-10-08.

## 4. The goals

| when | goal | measured now |
|---|---|---|
| when milestone 841 lands | the ratchet is on | off |
| by 2026-12-31 | no file over 4,000 lines | 7 |
| by 2026-12-31 | at most 10 files over 2,000 lines | 20 |
| long-term | no file over 2,000 lines | 20 |
| long-term | under 25% of all Rust lines in files over 1,000 | 48.3% |

Only the first is a gate. The others are read off the dashboard row.

The 2026-12-31 goals are about 12 weeks of work, and only one file has a milestone.
Milestone 840 (the scheduler file is split along its seams) covers `kernel/src/sched.rs`. The
other six files over 4,000 lines have none. Meeting the 2026-12-31 goals means filing a split
milestone for each of them, plus at least three of the thirteen between 2,000 and 4,000. This
section does not file them. Each split is its own design question: milestone 840 found four
architect rulings inside one file.

The headline number is the share of lines in files over 2,000, because one split moves it. The
count of files over 2,000 moves only when a file crosses the line, and the 95th percentile barely
moves at all.

## The seven questions

1. What else was considered.
   - Non-comment lines, which is what rustc's `tidy` counts. Refused: a reader loads the comments,
     and in this tree they are 41% of the non-blank lines.
   - Bytes or tokens, closer to a model's context cost. Refused: the window that matters, the
     `Read` default, is in lines, and so is a person's sense of a long file.
   - A cliff, failing all twenty on the day the gate lands. Refused for §212's reason: a gate cannot
     split a file, so the tree goes red and nothing improves.
   - A marked exception in the file, tidy's `// ignore-tidy-filelength`. Refused: an exception
     marker never shrinks and nothing counts them. A list is one file, and its length is the debt.
   - A ceiling of 1,000 lines (94 over) or 1,500 (36 over). Both are worth considering later.
     Neither has the one-read reason behind it.
   - A ceiling of 3,000, tidy's number (12 over). Refused: it loses the one-read property.
   - A per-function limit, clippy's `too_many_lines` (from memory, not read). A different problem,
     and not ruled out.
2. What this tree already does. §212 is the same decision for prose, with a ratchet and a chart.
   The unsafe ceiling in `script/lint` is a ratchet on a count. Nothing in `script/lint` checks a
   source file's length today; `script/boot-file-size-check` is about a boot image, not source.
3. Prior art, read on 2026-10-08. rustc's `src/tools/tidy/src/style.rs` sets `const LINES: usize =
   3000` and counts only lines that do not start with `//`. A file opts out with an
   `ignore-tidy-filelength` comment. ESLint's `max-lines` defaults to 300 lines. It counts comments
   and blank lines unless `skipComments` or `skipBlankLines` is set.
4. Is the premise true. The `Read` default was checked against the tool's own description. That
   large files hurt correctness is an inference: no lane's failure has yet been traced to a file it
   could not see whole.
5. What it costs. The check is one `git ls-files` and one `wc -l`, which took 14 milliseconds over
   the whole tree. The real cost is in the lanes. A quarter of recent merges touched a listed file,
   and under the ratchet each must be net non-growing or split the file first. That is the point
   of the gate, and it will slow some feature work in those twenty files.
6. How reversible. Entirely. A lint and a list file, and nobody outside the tree acts on either.
   The dashboard number is internal. The one thing that would be hard to undo is a habit of
   deleting comments to pass, which is why section 1 names it.
7. Would we choose this at equal cost. Yes. Option (b) over (a) is about merge-group failures, not
   effort.

## What this unblocks

Milestone 841 (a ratchet on Rust file length, and its dashboard row) waited on this ruling and can
now start. Milestone 840 never waited on it; it splits one file whatever the ceiling is.

## BUGS

- 2,000 lines as what a person can hold in mind is a judgment, not a measurement. The `Read`
  default is the only checked reason, and it belongs to one tool. If that default changes, the
  ceiling's strongest argument goes with it.
- A line ceiling rewards long lines. `script/fmt` holds width, so this is bounded, but a file can
  pass by moving code into a macro or a generated file. Review is the only defense.
- Splitting a file can make a reader open three files where one would do. Splitting along a seam
  is what makes that cheaper than one long file, and no gate can tell a seam from a cut.
- Inline `mod tests` counts toward its file. `kernel/src/sched.rs` holds 2,090 lines of tests. A
  split that moves tests out is a real split, since the reader of the code no longer loads them, but
  it is also the cheapest one, and it can leave the code itself no shorter.
