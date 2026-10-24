# Confining DMA: the appendices to notes/dma.md

[`notes/dma.md`](../dma.md) carries the hole, why there was no IOMMU, the fix, what is proved and
what is only mitigated, the validator, BUGS, the tradeoff, and the IOMMU that arrived. These files
carry the detail its prose budget has no room for.

- [`shadow-ring.md`](shadow-ring.md): the two features that defeated the in-place check, the
  time-of-check race that remained, and the shadow descriptor ring that closes all three.
- [`directions.md`](directions.md): why the validator needed no change for block writes or a NIC's
  receive queue, what kill-mid-write found, and the multi-queue layout.
- [`validator.md`](validator.md): the validator crate's one predicate, and what each of its Kani
  harnesses covers.

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes, which split the parent
under §212 (a prose budget). The directory follows §212's siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

Each stem below is named for its content, and calef names files.

- `shadow-ring` Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.
- `directions` Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.
- `validator` Name: provisional, minted 2026-10-24 (UTC) by comments/929-crates-11.
