# Confining DMA with an IOMMU: the appendices to notes/iommu.md

[`notes/iommu.md`](../iommu.md) carries the seam, the drivers, the confinement test, what is
proved, and the VT-d driver with its BUGS. These files carry the detail its prose budget has no
room for.

- [`interrupt-remapping.md`](interrupt-remapping.md): how VT-d remaps MSIs and IO APIC messages,
  and what the unit refuses.

*Name: provisional, minted 2026-11-11 (UTC) by milestone/1068-vtd-interrupt-remapping, which split
the parent under §212 (a prose budget). The directory follows §212's siting default, `notes/X/`
beside `notes/X.md`.*
