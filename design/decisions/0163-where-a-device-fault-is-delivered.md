---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-05
ratified_by: calef
---

# 163. Where a confined device's IOMMU fault is delivered

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote down)'s lane, which found milestone 102 (what a confined device's fault reaches) gated on
`DECISION` with no decision anywhere a reader can open. The block has stated the fork since it was
raised on 2026-08-04, and three separate documents defer to a fault-handling milestone that did not
exist when they were written. 

## What is being decided

**What a production kernel does when a confined device reaches outside its grant.** The IOMMU
already stops it and already records it; the question is who is told.

**The small half is not part of this.** Clearing the event queue's overflow bit on drain needs no
decision, has a witness, and is a two-driver change under the architectural-parity rule. It is
named here only so the decision is not read as gating it.

## The ruling, 2026-10-05 UTC

calef ruled in one word: **"B1"**. On an IOMMU fault, the kernel faults the driver process. Its
supervisor receives the existing death message, `EVENT_FAULT` (`crates/moho_abi/src/lib.rs:984`). The
message's `addr` is set to the faulting DMA address, and the driver's label is in argument
register 5 (§148 (a supervisor restarts by asking, and resolves by asking the kernel), as amended
2026-10-04; #1659). That label is milestone 105 (the two forks named and left)'s. There is no new
event code and no wire change. It mirrors §26 (the fault endpoint: thread death becomes a message a
supervisor holds): a thread whose message is sent never runs again.

**Widened the same day (2026-10-05 UTC), calef answering "Yes" to "B1 plus quarantine at the IOMMU,
written as part of the ruling":** besides faulting the driver, the kernel switches that device's
IOMMU entry to block everything. A dead driver's device then cannot keep issuing DMA and faulting. The mechanism, per driver at origin/main:

| architecture | quarantine write | where the field is |
|---|---|---|
| aarch64 | the device's stream table entry set to abort (`CONFIG` bits `[3:1]` = `0b000`) | `kernel/src/arch/aarch64/iommu.rs:351-358` (entry layout), `:188` (an unusable entry aborts and records `C_BAD_STE`) |
| x86_64 | the device's VT-d context entry marked not-present (`CTX_ENTRY_P` cleared) | `kernel/src/arch/x86_64/iommu.rs:282` |
| riscv64 | the device's riscv-iommu device context marked invalid (`DC_TC_V` cleared) | `kernel/src/arch/riscv64/iommu.rs:101`, written at `:394` |

The supervisor's restart path re-confines the device for the new driver, so quarantine is undone by
the same `confine` that first granted the device. The quarantine touches only IOMMU state the
kernel already owns through `confine`, never device registers.

Refused as the form: PCI bus-master-off. It exists only on PCI, so virtio-mmio devices on
aarch64 and riscv64 would be missed, which breaks §19 (architectural parity is a tenet; the targets
are aarch64, riscv64, and x86_64).

## Whether the premise is true, measured 2026-09-19, rechecked 2026-10-05

It is. Every call site of `crate::iommu::take_fault` outside the driver definitions is a test. The
sites are `kernel/src/virtio.rs:1049,1595,1603` (the DMA-escape test) and
`system_tests/src/user/display_tests.rs:196,219,247` (milestone 29 (a display terminal: framebuffer,
virtio-gpu, and a foreign component), framebuffer work). That file moved out of `kernel/src/user/`
when the system tests left the kernel crate. The aarch64 and riscv64 drivers say so in their own
comments. *"take_fault (the confinement test); no production fault handler yet."* There are three
drivers, not two: x86_64 now has VT-d (`kernel/src/arch/x86_64/iommu.rs`), and VT-d and
riscv-iommu-pci signal faults by message-signaled interrupt (MSI), as the xHCI driver already does.
Today no driver registers a fault interrupt, so a confined device that faults during an ordinary
boot reports to nobody, and the kernel discards its own evidence that hardware confinement fired.

The overflow flag is partly done. x86_64 already clears its flag in `take_fault`
(`kernel/src/arch/x86_64/iommu.rs:918-924`). aarch64 and riscv64 do not, which is milestone 102's
small half.

Three documents already say it is owed, which is the argument that this is a gap rather than a
possibility. They are `notes/iommu.md`'s honest limits, [§20 (IOMMU-backed DMA
isolation)](0020-iommu-dma-isolation.md) (IOMMU-backed DMA isolation)'s own limits list, and
`notes/framebuffer-contract.md` mirrored into [§29 (the framebuffer is a bigger
grant)](0029-framebuffer-grant.md) (the framebuffer is a bigger grant, not an exemption).

## What this tree already does in the analogous case

A thread's death is already a message its supervisor holds. [§26](0026-fault-endpoint.md) (thread
death becomes a message a supervisor holds) built exactly this shape for CPU faults: the kernel
does not print, it delivers, and the party that holds the relationship is the party told.
[§32](0032-reap-without-build.md) then made the supervision relationship the unit of authority rather
than a rights bit. A device escaping its grant is the same event one layer down, and the tree
already has the vocabulary for it.

The counter-precedent is real and should be weighed: the kernel prints and continues for
everything it cannot handle. That is the right default for a kernel with nobody to tell. It is the
wrong one here precisely because there *is* somebody to tell.

## The options, and what each lost to

| | what happens | verdict |
|---|---|---|
| A | Print and continue. | Refused: the evidence is kept and nobody decides anything. |
| B1 | Fault the driver process; the supervisor gets `EVENT_FAULT` with the DMA address. | Ruled. The party granted the device is told, through a channel it already reads. |
| B2 | A new non-death event, driver keeps running. | Refused, reasons above. |
| C | Disable the device. | Refused: nobody decides here either, the kernel does. It would stop a fault storm, which the quarantine now covers. |

What B1 costs, measured from the tree 2026-10-05. Two pieces of kernel work come before any
policy:

- A fault interrupt per driver, three of them (VT-d, riscv-iommu-pci, SMMUv3), none of which
  exists.
- A kernel map from DMA requester id to the driver to fault, written at `confine` time. No
  DMA-domain capability exists to hang this on, so the map is new state. `confine` is called from
  bring-up code (`kernel/src/non_volatile_memory_express.rs:475`, `kernel/src/e1000e.rs:199`, and
  also `kernel/src/extensible_host_controller_interface.rs:119` and `kernel/src/virtio.rs:746`), so
  each call site must also say whose driver it is.

## How reversible it is

B1 reuses a message shape two parties already agree on, so the expensive category is untouched. The interrupt registration and the requester-id map are ordinary kernel work. Nobody
outside this tree has acted on either, so the cost today is only the writing.

## What is blocked until this is answered

Milestone 102's large half, which is now buildable. Its small half (clearing the overflow bit on drain, still owed on aarch64 and riscv64) was never
blocked and should not wait. Today's correctness rests on no test ever overflowing the queue rather than on
a drain that clears the condition, and that already made one test misreport another.

And the proof shape is settled, so nothing else is waiting. Point a confined device at a frame
outside its domain and assert the report arrives where the design says it should, keeping the
provocation to a single translation so the flood milestone 29 hit cannot recur.

## BUGS

Cost this adds to milestone 102: one quarantine write per driver (three), plus re-confine on the
supervisor's restart path. The proof shape should assert the device's next transaction is blocked,
not only that the supervisor was told.
