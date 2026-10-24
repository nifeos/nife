# The IOMMU escape test: what it found on the way

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.*

An appendix to [notes/framebuffer-contract.md](../framebuffer-contract.md), moved there on
2026-10-11 (UTC) under §212 (a prose budget). It records why the test reads the fault queue rather
than the device, and the RISC-V fault-queue overflow it ran into.

## A surprise worth recording: the device's "OK" is not evidence

The first version of the escape test asserted that the device *refused* the command. It did not.
QEMU's DMA layer answers a translation failure by handing the device a bounce buffer rather than
failing the mapping. So `RESOURCE_ATTACH_BACKING` returns `OK_NODATA`, while the bytes the device
actually gets are not the victim frame's. The confinement held; only the error reporting did not
survive the trip.

So the test asserts on the IOMMU's fault queue, which is the hardware's own account. The
response code is only printed for the record. A test that had trusted the response code would have
reported a hole that was not there. Worse, in the other direction, a test that trusted an `OK`
as "the device read it" would have reported a read that did not happen.

An earlier iteration also aimed the escape at "the frame just past my region", which was a bad
guess. The kernel's shadow page is allocated immediately after the DMA region, and that frame is in
the domain. The kernel now picks the victim frame and hands it to the attacker in `arg2`, the same
way milestone 16b's confinement test picks its own. The test has to know the exact address to look
for in the fault queue.

## A found limit: the RISC-V IOMMU's fault queue overflows silently

Worth its own heading because it will matter more to whoever routes faults to a production handler
than it did here. The RISC-V IOMMU driver gives the fault queue 128 records
(`FQ_LOG2 = 7` in `kernel/src/arch/riscv64/iommu.rs`) and never clears the queue's overflow bit.
So a flood of faults latches the overflow and, from then on, *no further faults are recorded at all*.

This was found the way such things should be. The escape test first attached a 4096-byte backing,
which produced a flood. The next test in the suite (milestone 16b (real hardware)'s
`the_iommu_faults_a_dma_that_escapes_the_domain`) then observed no fault, and correctly reported the
IOMMU as not confining the device. A real regression signal, from a cause two tests away.

Two mitigations, both local to milestone 29 because the driver's overflow handling is not this
milestone's lane:

- the escape attaches four bytes, so it provokes exactly one translation and one fault. That asks
  the same security question ("can this device reach an address outside its grant?") without
  flooding;
- the test drains the queue when it is done, leaving it as it found it.

What is left for a fault-handling milestone is two things. Clear the overflow bit when draining.
And decide what a production kernel does when a confined device faults at all. Today the queue is
drained only by tests, and DECISIONS §20 (IOMMU-backed DMA isolation) already records fault routing
as future work. aarch64's SMMUv3 event queue has the same
shape of limit and was not exercised into it here.
