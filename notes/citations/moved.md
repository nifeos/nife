# `--moved`: why it tells, how it was shaped, and what it costs

*Name: provisional. Named after the `--moved` mode it is about.*

An appendix to [`notes/citations.md`](../citations.md), which states the conclusion.

## Why it tells rather than obliges

DECISIONS §207 (the roadmap is a graph, and the block says so in fields a script can walk) ruled on
this exact coupling from the other direction, and cites milestone 385 by number as its evidence.
§207 retired `Gate: MILESTONE N` because the rule obliged an edit in every dependent block when
a dependency landed, made by somebody in another lane who was not looking. So a milestone being
satisfied turned unrelated branches red on a line they never touched.

What §207 removed is an obligation to *edit* on a status flip. `--moved` is a duty to *tell*
somebody on a status flip, addressed to the one lane already in the file. It asks for no edit, it
is read and dismissed in a sentence, and no other branch goes red. A version of this that demanded
edits would reintroduce what §207 refused three weeks after it was decided, which is why it reports.

## The four measurements that decided the design

1. A worklist is three files, not thirty. Across `notes/`, 194 milestones are cited by at least
   one note, median 3 and mean 4.4. Three files is a thing a lane reads.
2. It fires per branch, not over the tree. Sixty-eight rows flipped to `BUILT` in fourteen days.
   Tree-wide that is fifteen note-reads a day owned by nobody, which is how a check becomes noise.
3. It keys on the bare `milestone N`. Only 4% of milestone citations in `notes/` carry a gloss,
   so reading the glossed form `--check` reads would find one citation in twenty-five. It uses the
   same regex `--census` uses, for the same reason.
4. It reads the block file and nothing else. 385's own `BUGS` feared a status moving without the
   pull request touching the index, and asked the implementation to read both. There is no longer a
   both: `design/roadmap/README.md` was generated from the blocks by
   milestone 294 (`design/roadmap/README.md`'s index is generated, not hand-maintained) and retired
   on 2026-09-21, so the `Status:` line in the block is the single record.

## What it costs, measured over history

Over the 150 most recent merges into `main` on 2026-09-23, 50 moved at least one status and the
other 100 printed a single line saying nothing moved. A lane that edits its own block without
changing its status, which `script/lint-fleet` 4b makes every lane do, sees that line and nothing else.

Of the 50, the median is 4 citing files. Those 50 merges hold 262 individual flips, and per
flip the median is 1 citing file and the mean 3.4.

The tail is real and is worth naming rather than averaging away, because it is where a reader would
decide this is noise:

| merge | flips | citing files |
|---|---|---|
| #970, milestone 433 (drain the proposal pile to zero, and keep it there) | 113 | 363 |
| #1018, milestone 448 (a refusal gets a number, a status, and a condition that would change it) | 43 | 103 |
| #992, milestone 161 (the x86_64 kernel port: bring up the HAL's third architecture) | 1 | 48 |

The first two are sweeps that added or restatused dozens of blocks at once, which is the same
outlier the ratchet's own measurement found in #970. The third is one flip on a heavily cited
number. The worst single milestone in the window is 433 at 108 citing files, for the same reason:
a milestone that reorganized the roadmap is cited by the roadmap.
