# NVMe: the appendix to notes/non-volatile-memory-express.md

[`notes/non-volatile-memory-express.md`](../non-volatile-memory-express.md) carries the driver's
shape, the protocol, the confinement story and its tests. This directory carries what its prose
budget has no room for.

- [`the-dma-escape-test.md`](the-dma-escape-test.md): the test where a confined server aims the
  controller outside its region, the attacker role that does it, and why the proof is the IOMMU
  fault and a canary.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/`
beside `notes/X.md`. calef names directories.*
