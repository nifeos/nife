---
status: NOT-STARTED
raised: 2026-08-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 102. What a confined device's fault reaches

Raised 2026-08-04 from a sweep for work named in prose and owned by
nobody. This is the clearest instance of the shape: three separate documents defer to "a
fault-handling milestone" that has never existed.

What a production kernel does when a confined device faults is a fork with
three answers and different politics (print and continue, fault the driver process, disable the
device), and the block argues for the second without taking it. The small half, clearing the
overflow bit on drain, needs no decision and has a witness in milestone 29's flood. It should not
wait on this. The large half is
§163 (where a confined device's IOMMU fault
is delivered), written up 2026-09-19 by the lane of milestone 435 (forty-five milestones are gated
on a decision nobody wrote down). This gate had named no decision since
the block was raised, which is the same shape as the three documents deferring to a milestone that
did not exist.

**Ruled 2026-10-05 (calef, "B1"):** the kernel faults the driver process and its supervisor receives
`EVENT_FAULT` with the DMA address. The kernel quarantines the device's IOMMU entry (one write per driver,
re-confined on restart), per §163 (where a confined device's IOMMU fault is delivered).
The large half is now buildable. The small half (the overflow flag on aarch64 and riscv64; x86_64
already clears it) was never blocked.

**The finding.** An IOMMU stops a device that reaches outside its grant and records the attempt in a
fault queue. Milestone 16b built both drivers and proved the stop happens on both ISAs. Nothing
reads the queue in a real boot. `crate::iommu::take_fault` has five call sites and every one is a
test: `system_tests/src/user/display_tests.rs` (three, milestone 29's framebuffer work) and
`kernel/src/virtio.rs` (two, the DMA-escape test). So a confined device that faults during an
ordinary boot reports to nobody, and the kernel's evidence that its own hardware confinement fired
is discarded.

**Where it is already written down**, which is the argument that it is owed rather than merely
possible:

- `notes/iommu.md`, under "Honest limits": "No fault handler yet. `take_fault` is drained by
  the confinement test; routing IOMMU faults to a handler in a production boot is future work."
- `DECISIONS` §20 (IOMMU-backed DMA isolation) says the same in its own limits list.
- `notes/framebuffer-contract.md`, in the section now in
  `notes/framebuffer-contract/iommu-escape-test.md` ("A found limit: the RISC-V IOMMU's fault
  queue overflows silently"), mirrored into `DECISIONS` §29 (the framebuffer is a bigger grant,
  not an exemption): "What is left for a
  fault-handling milestone: clear the overflow bit when draining, and decide what a production
  kernel does when a confined device faults at all."

Three distinct claims across four sites, all pointing at a milestone number that does not exist.

The overflow bit is the small half, and it has a witness. Milestone 29 (a display terminal:
framebuffer, virtio-gpu, and a foreign component)'s escape test first attached a 4096-byte buffer,
which provoked a flood of faults. The *next* test in the suite then
observed no fault and correctly reported the IOMMU as not confining the device. A real regression
signal from a cause two tests away. The fix was to make the escape four bytes, so exactly one
translation and one fault, and to drain the queue afterwards. That leaves the queue's overflow flag
set by nobody's arrangement but the test's restraint. Today's correctness rests on no test ever
overflowing the queue, rather than on a drain that clears the condition. The flag exists on both
sides (the SMMUv3 event queue, and the RISC-V IOMMU's `fqcsr`, whose error bits the comment above `QUEUE_EN` in
`kernel/src/arch/riscv64/iommu.rs` already names), so clearing it is a two-driver change under rule 5.

The large half is a design fork, and it should be stated as one. "What does a production kernel
do when a confined device faults" has at least three answers with different politics:

1. Print and continue, which is what the kernel does with everything else it cannot handle, and
   which puts a security event on a console with no owner.
2. Fault the driver process. The supervision machinery already turns a thread's death into a
   message its supervisor holds (§26, milestone 22), so a device escaping its grant could arrive as
   the same kind of event, delivered to whoever holds the driver. That is the capability-shaped
   answer: the party that was granted the device is the party told that the device misbehaved.
3. Disable the device, which is the safest and takes a decision away from the holder.

Option 2 is the one that fits the model, and it is not free: the IOMMU's event queue is an interrupt
source neither driver currently registers, so this adds an interrupt handler per architecture before
it adds any policy.

## Scope note

Not a general device-fault framework. The subject is the IOMMU's fault queue on both boards, the
one place where hardware already knows something the system throws away. CPU page faults are a
different path with its own owner (§26, the fault endpoint).

The proof shape is milestone 16b (real hardware + IOMMU-backed driver isolation)'s, reused. Point a confined device at a frame outside its
domain, and assert the report arrives where the design says it should, rather than asserting a queue
entry exists. Milestone 29's flood is the reason to keep the provocation to a single translation.

## Index row

Three documents defer to "a fault-handling milestone" that has never existed. All five `take_fault` call sites are tests, so a confined device faulting in a real boot reports to nobody,
and the kernel discards its own evidence that hardware confinement fired. Also owed: clearing the
overflow bit on drain, whose absence already made one test misreport another
