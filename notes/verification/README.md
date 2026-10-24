# Verification: the appendices to notes/verification.md

[`notes/verification.md`](../verification.md) is the page to read: what a Kani proof is, what a green
check means, one row per proved crate, and the rules. The files here hold each crate's harness table
and the reasoning behind it, at a length the main page's prose budget cannot hold.

- [`core-harnesses.md`](core-harnesses.md): capabilities, untyped regions, paging, frames, the
  device-tree leaves, IPC, the generational table, the intrusive queue and ASIDs.
- [`parser-harnesses.md`](parser-harnesses.md): ELF, nifefs and PCI.
- [`direct-memory-access-validator.md`](direct-memory-access-validator.md): the DMA-confinement validator, its bounds and
  `kani::cover!`.
- [`iommu-domain.md`](iommu-domain.md): the IOMMU domain, and what the DMA proof does not establish.
- [`paging-blocks.md`](paging-blocks.md): 2 MiB and 1 GiB leaves.
- [`calendar-proofs.md`](calendar-proofs.md): the calendar, and where BMC's cost actually is.
- [`elf-parser-wall.md`](elf-parser-wall.md): where BMC hit a wall.
- [`glob-proofs.md`](glob-proofs.md): the glob matcher.
- [`ci-timings.md`](ci-timings.md): suite timings, sharding and `--affected-since`.

Name: provisional, for the directory and every stem in it. The directory follows the `notes/<stem>/`
appendix convention §212 (a prose budget) set, so its name is the parent's stem. Each file is named
for its content. calef names directories and files, and `script/names --unratified` lists each stem.
