# The DMA escape test

*Name: provisional. For the NVMe server's DMA escape test and its attacker role. calef names files.*

The appendix to [`notes/non-volatile-memory-express.md`](../non-volatile-memory-express.md), whose
section "The confinement story, which is the interesting part" carries why the server is confined.
This file carries the test that attacks that confinement.

`a_confined_el0_server_cannot_dma_outside_its_region` is the storage twin of
`display_tests::a_backing_outside_the_grant_is_refused_by_the_iommu` and of `block_driver`'s two
virtio attacker roles. The headline test asserts `confined_by_iommu`, which is a claim the boot
made rather than one it attacked: the decorative assertion milestone 202 (every confinement test is
a ritual until somebody breaks the confinement and watches it fail) was minted against. Here an EL0
server points the controller at a frame outside its region, in both directions. Milestone 202's
convention holds: a claim, a test and a replayable falsification, whose patch widens the
controller's IOMMU domain over the victim so the escape lands.

The proof is the IOMMU fault and an untouched canary, not the controller's completion status.
QEMU's NVMe model may answer a DMA it could not perform with an error or by dropping it, and the
GPU and virtio escape tests anchor on the fault for the same reason. A CPU read of the victim
bypasses the IOMMU, so the kernel can read the canary while the device's view of that frame is
unmapped. The canary is the assertion the patch flips.

The attacker is this exact server differing by one field, the physical address it puts in a PRP, so
the attack tests the IOMMU rather than a straw man (milestone 261 (the NVMe driver leaves the
kernel)). `start_dma_escape` in the kernel's wiring writes a magic word and a victim's physical
address into the transfer buffer before the server runs, and the server reads them before its
readiness transfer reuses that page. The victim is a raw number read out of shared memory, exactly
the authority an escaping driver would fabricate from nothing. The role is compiled only with the
`confinement_attackers` feature, which only `cargo xtask test` turns on, so a shipped server has
neither the check nor the attack.
