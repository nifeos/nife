# The census and the backfill

*Name: provisional.*

An appendix to [`notes/citations.md`](../citations.md), which states the conclusion.

## The census: the population nothing had ever counted

`script/citations --census` counts the citations that carry no gloss at all, which `--check`
cannot see, because `--check` reads only the parentheticals that exist. The first run was 2026-09-19
and it corrected the framing of milestone 444 (a citation says what it cites)'s own brief:

```
$ script/citations --census
                    files  citations   named  unnamed  no record
roadmap proposals      23        196       5      101          0
roadmap blocks        439       5224     212     2351          0
decisions             188       1944      66      984          0
design/ (other)        25        444      38      224          0
notes/                175       3408     131     1660          0
Rust                  395       5867      33     2738          0
scripts                64        716      12      399          0
manifests              55        370       7      260          0
CI workflows           13         64       0       44          0
everything else        70        284       1      217          0
total                1447      18517     505     8978          0
```

Read the last two columns first. 18,517 citations, in 1,447 files, and 505 of the 9,483
(file, scheme, number) pairs are named: 5.3%. The roadmap-after-the-split proposal reported the
tree as "83% split-proof by habit", and that number is true of the 560 citations that carry a gloss.
Over the whole population it is 5%. Both numbers are honest and they are about different sets; the
second is the one that prices the backfill, and nobody had it before.

The zero is the other finding, and it was re-checked before being believed. Not one cited number
in the tree fails to resolve to a block or a section. That is `script/roadmap --check` and
`script/decisions --check` doing exactly their job, for 18,517 citations. The check on the zero
matters because a `git grep` for citations does turn up six-digit numbers that resolve to nothing.
They are segment timestamps in the filenames under `bench/radon-2026-09-04/`, binary captures, and
they arrive through the trap `script/decisions` records, where `git grep` prints *"Binary file ...
matches"* into the same stream as the matches. The
census skips a file containing a NUL byte and excludes `*.log`, so the zero survived the re-check.

The unit is the pair, not the occurrence, because that is the unit of work: one gloss in a file
answers every mention of that number in it. `--census --list` prints the worklist, one line per pair,
which is how the backfill below chose what to do.

## The backfill, and the rule that chose it

Milestone 444 glossed 25 pairs across three files: `README.md`, `CONTRIBUTING.md` and
`SECURITY.md`. That is 0.3% of the backlog, chosen rather than sampled, and the rule is one
question.

Gloss where the citation is the reader's only route to what is meant. `SECURITY.md` is the
worked case and it is why those three files were picked: a person reporting a vulnerability met
*"(DECISIONS §20, §23, §30; `crates/dma_validator`, notes/iommu.md)"* and had no way to know what
those three sections claim without cloning the repository and opening three files. They now read
*"§20 (IOMMU-backed DMA isolation), §23 (multi-queue DMA confinement), §30 (the DMA boundary is
proved for descriptors)"*. These are the documents a stranger meets before they have a checkout,
which is principle 3 (a newcomer must be able to succeed without asking anyone) applied to a
footnote.

Do not gloss where the surrounding text already says what the target is. Two cases were looked
at and deliberately left:

- `notes/scripts.md`, 50 pairs. Its citations sit in a table whose row already describes the
  script. The `script/icount` row says it is the instruction-count instrument and what it boots, and
  the parenthetical records only which milestone built it. That number is provenance, not a pointer,
  and a gloss would add a clause to a dense table row saying what the next clause already says.
- `notes/README.md`, 128 pairs. The notes index, same shape: each row links the note and
  describes it, and the milestone number records which milestone wrote it.

And do not gloss a dated account, a quotation, or a block's own history, which is
`design/naming.md`'s rule that a dated record keeps its words. A gloss inserted into a sentence
written in August rewrites what that sentence said.

The remainder, 8,953 pairs, is recorded as work in milestone 444's block rather than carried as an
intention: `script/citations --census --list` regenerates the worklist at any time, so nothing about
it has to be kept in a note that will go stale.
