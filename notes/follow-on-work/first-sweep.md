# The first sweep, 2026-09-03: every finished block read

*Name: provisional, minted 2026-10-11 (UTC) by lane/final-longest-notes, after the parent's "The
sweep, 2026-09-03". calef names files.*

The evidence behind [`notes/follow-on-work.md`](../follow-on-work.md)'s sections "The sweep,
2026-09-03" and "What the sweep found, and which of it to promote first". The parent keeps the
conclusions and the five items worth promoting; this file keeps the table, the side findings and
the three proposals that turned out to be done already.

## How it was read

All 139 finished blocks were read, in twelve parallel lanes, roughly 159,000 words. Not skimmed:
each lane read its blocks in full, including `BUGS` sections and handoff prose. Each resolved
successors against the index rather than guessing.

605 dispositions were written. The shape of them is the interesting part:

| Disposition | Count | What it says about the tree |
|---|---|---|
| `Recorded.` | 291 | The `BUGS` convention is working. Most follow-on prose is an honest limitation sitting where a reader meets it |
| `Milestone N.` | 136 | The successor usually exists, and the block usually cited it by number |
| `Refused.` | 105 | Refusals with reasons, which this convention counts as successes |
| `Proposed.` | 38 | Work named, nobody holding it, now a file in `design/roadmap/proposals/` |
| `Decision.` | 24 | Forks written up under `design/decisions/` |
| `None.` | 7 | Almost all of them backfilled history blocks from milestones 1 to 11 |
| `Done.` | 3 | And all three were `Proposed.` first. See below: the word paid for itself within the hour |

## Two things the sweep found that were not in its brief

`Recorded.` bullets citing paths surfaced real rot in the roadmap's own prose. Milestone 136
(decision)'s pin still named `crates/regions` after §113 (identifier names move) renamed it to
`crates/memory_regions`. 136's free-site pin still named `kernel/src/untyped.rs` after milestone 135
(the region claim) moved it. And 124 still named `crates/slots`. Nothing had been checking a path
cited in a roadmap block.

And a block written after this convention existed is far cheaper to sweep than one written before
it. Blocks 196, 216 and 234 carried their follow-on work in a legible shape, usually citing the
successor by number. The older blocks name it only by implication, and cost several times the
reading per bullet. That is the case for the gate as well as any argument in the parent.

## The exemption list that lasted an hour

The first design let a block that finished before the sweep sit on a list in the parent instead of
carrying a section. A date cutoff meant nothing new could be parked. The sweep then reached every
block, and the list closed to zero. An empty list that permits parking is exactly the hiding place
its own `BUGS` entry had called a foot gun. It is gone rather than kept at zero.

## The three proposals that were already done

Three of the 42 were done. A lane told to write a proposal has to go and look at the tree, which
nothing had made anyone do before:

- Milestone 54 (network) asked for a roadmap status word meaning "built, then removed". calef
  minted `REMOVED` on 2026-08-30, the same day the code went, and the block's own status line reads
  it.
- Milestone 57 (partitioning) wanted partitioning on the target.
  `components/src/disk_partitioner.rs`, commit `4db2fd74`, holds a disk endpoint and an entropy
  endpoint. It draws four v4 GUIDs and writes both table copies.
- Milestone 57 wanted `mkfs` on the target, and called the `vendor/redoxfs` divergence calef's to
  take. It was taken and documented: `Header::new_with_uuid`, divergence 4 in `vendor/README.md`,
  carried as `patches/redoxfs-no-std-create-uuid.patch`.

Two of those had sat stale for a month inside a `## Follow-on` section. That is milestone 247
(follow-on work)'s own failure one layer in: the section says what happened to the work, and nothing
checks that it is still true. They are `Done.` now, with what carried each. `Done.` was added that
morning because four lanes said the word was missing, and it earned its place before the day was
out.
