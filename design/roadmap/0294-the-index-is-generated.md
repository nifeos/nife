---
status: BUILT
raised: 2026-09-14
built: 2026-09-14
---
# 294. `design/roadmap/README.md`'s index is generated, not hand-maintained

There is no index table: calef, 2026-09-21, *"I don't think the milestone index table is worth
building any more. It has too many rows to be digestible as a table."* The derivation this milestone
built is what made that cost nothing: every cell was a rendering of text living in a milestone's own
file, so deleting the table deleted no writing. What survives of the generator is `script/roadmap --index`,
which assembles the same five columns as a feed for `script/audits` and `script/fatal-risks`; the
vocabularies that surrounded the table are in `notes/roadmap.md`. The status stays **BUILT** rather
than becoming REMOVED, because what this milestone built was the *derivation*, and the derivation
is what made the deletion free.
*(Number provisional until the merge queue lands it.)*

**The defect was structural, not bad luck.** `script/lint` check 4b requires every lane to touch
its own milestone's roadmap block. Every milestone also needed a row in one sorted table. So every
milestone in flight collided with every other milestone in flight, always, in that one file, and
`main` moving once re-conflicted every open lane. Pull requests #836, #843, #847, #849, #850, #851,
#854 and #855 all conflicted in `design/roadmap/README.md`'s index table and nowhere else, several
of them more than once, and the resolution was mechanical every time: take both sides' rows and
sort by number.

AGENTS.md names `kernel/src/user/tests.rs` as the tree's merge hotspot, measured on 2026-08-16. It
is the wrong file to fear. Only some lanes wire a test; **every** lane adds a row.

## The ruling

calef, 2026-09-14, choosing between three options: generate the index from the block files.

## Where the row now lives, and why it had to move rather than be derived

Each block carries a `## Index row` section, and the table's five columns come from four places in
the milestone's own file:

| Column | Comes from |
|---|---|
| `#` | the filename's number |
| `Status` | the `**Status: <TOKEN>` line, which `script/roadmap` already read |
| `Milestone` | the H1, which `script/roadmap` already read |
| `Why it matters` | the `## Index row` section's one summary paragraph |
| `Built` | that section's `**Built:** <YYYY-MM-DD>` line |

**Two of the five were not derivable from anything the blocks already said**, and measuring that
is what decided the shape of this milestone rather than discovering it late.

The summary is a hand-written precis, median 369 characters and up to 1832. Deriving it from
the block's own opening paragraph was the option that needed no migration, and it was measured
before it was refused: across 288 milestones the row's summary scored a median similarity of
0.07 against the block's first body paragraph, above 0.4 for three milestones and above 0.6 for
none. An opening paragraph is written to open a document. Taking it would have silently rewritten
what the index said about 288 milestones, in one commit, and each of those cells is the tree's own
account of its history.

The Built date was worse. 129 of 288 blocks did not state it anywhere a parser could find, and
the ones that did used at least eight spellings: `**Status: BUILT.**` with the date in a later
sentence, `**Status: BUILT, 2026-08-26.**`, `**Status: BUILT (2026-08-01).**`, `**Status: BUILT**
on 2026-08-18 (PR #320)`, `**Status: BUILT**, 2026-08-27, as ...`. Canonicalising 129 status lines
would have been rewriting prose in 129 blocks to serve a parser, which is the wrong direction.

So both were moved, verbatim. The migration cut each cell's text and pasted it into the block;
the only transformations were unescaping `\|` (an artifact of living in a table cell, where a bare
pipe would end it) and line wrapping. Nothing was regenerated, paraphrased or shortened, and the
reconstruction below is the proof.

## The proof: reconstruction, not inspection

Generating the table from the migrated blocks and diffing it against the committed one reproduced
271 of 288 rows byte for byte. All 17 differences were pre-existing drift in the
hand-maintained table, none of them a defect in the generator:

- 12 whitespace. Eight rows carried a doubled space in an empty `Built` cell (131, 224, 258,
  260, 261, 262, 263, 265) and four padded the date (135, 196, 202, 208). Invisible in rendering,
  invisible to the old gate, which anchored on the first four fields.
- 5 titles, where the row had drifted from its own block's H1. Resolved in favor of the
  block, because there is now one title per milestone rather than two:

  | # | The row said | The block's H1 says |
  |---|---|---|
  | 16 | Real hardware + IOMMU-backed driver isolation, **RISC-V first** | Real hardware + IOMMU-backed driver isolation (recast 2026-07-27: RISC-V first) |
  | 29 | A display terminal (framebuffer, virtio-gpu) | A display terminal: framebuffer, virtio-gpu, and a foreign component |
  | 47 | Navigation and naming: cd, pwd, ls, mkdir, rm, paths, and environment | Navigation and naming: `cd`, `pwd`, `ls`, `mkdir`, `rm`, paths, and environment |
  | 48 | Job control: jobs, wait, kill, fg, bg, and a stopped state | Job control: `jobs`, `wait`, `kill`, `fg`, `bg`, and a stopped state |
  | 121 | `ripgrep`: enumeration as a capability, and what the walk costs | `ripgrep` on nife: enumeration as a capability, and what the walk costs |

  Row title versus file title is checked, not left out on the ground that rows abbreviate on
  purpose: two of the five abbreviations dropped backticks off command names, which is a loss
  rather than an abbreviation.

## What a lane does differently

**A lane never edits `design/roadmap/README.md`.** It writes its block, including the
`## Index row` section, and stops. The row appears when the index is regenerated.

`script/lint` check 4b **did not change and did not need to**: it already required a
`milestone/N-*` branch to touch `design/roadmap/N-*.md`, the block, and never the index. The
requirement to edit the index came from `script/roadmap`, which failed a block with no row. That
is the check this milestone inverted.

## What the gate does now, and the one thing it deliberately stopped doing

The blocks are the source. `script/roadmap` reads them for status, gates, follow-on dispositions
and tree-wide citation resolution, where all four used to key on the index table. `--index` prints
the generated table, `--write` rewrites it between the markers in the README, and `--check`
validates the blocks.

A committed row that disagrees with its block no longer fails the build. That was the
milestone-69 check, and it was a real defect *because both records were hand-maintained and both
looked authoritative*, so a reader could not tell which was lying. Once the row is derived there is
nothing to decide: a table that disagrees with a block is stale, and the fix is one command. So
`--check` reports the staleness, with the count and the command, and passes.

It has to pass, and the reason is the whole milestone. Failing would force every lane whose
status moves to regenerate, which means editing the table, which is the hotspot coming straight
back. The cost of not failing is in BUGS below, named rather than hidden.

## Follow-on

- **Recorded.** The committed table can lag the blocks between a merge and a regeneration; detection
  is rung two and the action is rung four. Recorded in this block's `BUGS` and, while the table
  existed, beside it in `notes/roadmap.md`'s predecessor; the table was retired on 2026-09-21 and
  the staleness went with it.
- **Recorded.** `## Index row` is a provisional name, like every name a lane ships. It sits beside
  `## Follow-on` and `## BUGS` as a section `script/roadmap` reads, and the refusals are in
  `notes/roadmap.md`.
- **Milestone 412.**
  This lane's first `script/test` failed on `uefi-boot`'s `smp: 2 core(s) online` assertion, from a
  tree whose diff `cargo xtask` does not read, and passed on the next two runs. The tree already
  records that flake and bounds it at `-smp 3` and above, which the tour's own comment reasons from;
  it reaches two.
- **Done.** The migration itself: 288 blocks, text moved rather than rewritten, verified by
  reconstruction. The method and the script are in `notes/roadmap.md` so the next
  column-shaped migration does not have to re-derive it.

## BUGS

- The committed table can be stale, and nothing fails when it is. A lane does not regenerate it,
  so between a merge and the integrator running `script/roadmap --write` the table in the README is
  behind the blocks. `--check` prints the count and the command on every run, which is rung two for
  detection and rung four for action, and that is an exception AGENTS.md's ladder asks to be
  declared rather than left to read as a design. Three higher rungs were looked at and each cost
  more than the staleness does: a bot pushing the regeneration to `main` would race the merge queue
  and can evict it, and this tree's own convention (`.github/workflows/metrics.yml`) is that a job
  must never push to `main`; a `merge=union` attribute on the table would auto-resolve the append
  conflicts and leave the rows mis-sorted, which trades a visible conflict for a quiet wrong answer;
  and failing the gate on a stale table brings the hotspot back. A stale *derived* artifact is also
  a much smaller defect than the one it replaces, because nobody has to work out which record is
  right.
- A lane still cannot see another lane's rows, and now it cannot see their blocks either. Two
  lanes minting the same milestone number still collide, exactly as before; what changed is that
  the collision is now two files claiming one number (which `script/roadmap` has always called
  fatal) rather than two rows in one table.
- The five title corrections are the generator asserting the block over the row, and one of
  them reads worse in a narrow table than what it replaced (milestone 16's parenthetical recast).
  It was not reworded, because rewording a block's H1 is a naming decision and this lane had
  permission to move text and nothing else.

## Index row

Eight merge conflicts in one session were all in `design/roadmap/README.md`'s index table and
nowhere else. Structural rather than unlucky: check 4b
makes every lane touch its own block, every milestone also needed a row in one sorted table, so
every milestone in flight collided with every other. The row is now derived from the block. Two of
the five columns had to move rather than be derived, and measuring that is what decided the shape:
the summary scored a median similarity of 0.07 against the block's own opening paragraph (an
opening paragraph opens a document rather than summarizing one), and 129 of 288 blocks did not
state their Built date in any parseable position, in at least eight spellings. Both moved verbatim
into a `## Index row` section; the proof is the reconstruction, which reproduced 271 of 288 rows
byte for byte, with all 17 differences pre-existing drift in the hand-maintained table (12
whitespace, 5 titles the row had abbreviated away from its own H1, two of them dropping backticks
off command names). A lane now never edits the index. Check 4b did not change: it always asked for
the block.
